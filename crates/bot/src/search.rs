use crate::evaluation::evaluate;
use crate::tt::{TTEntry, TTFlag, TranspositionTable, zobrist_key};
use chess_engine::{Board, Color, Move};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// Result of a completed search.
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    pub best_move: Move,
    /// Score in centipawns (positive = good for the side to move).
    pub score: i32,
    /// How many plies (half-moves) of iterative deepening were completed.
    pub depth: u32,
}

const INF: i32 = 999_999;
const MATE_SCORE: i32 = 90_000;
const MAX_PLY: usize = 128;

// Null-move reduction: R = 2 + depth/6  (less aggressive → fewer blunders)
fn null_move_r(depth: u32) -> u32 {
    2 + depth / 6
}

// Futility margin: max possible position gain in one quiet move
fn futility_margin(depth: u32) -> i32 {
    match depth {
        0 => 0,
        1 => 100,
        2 => 250,
        3 => 450,
        _ => 700,
    }
}

/// Negamax-aware evaluation: score from the current player's perspective.
fn eval_relative(board: &Board) -> i32 {
    evaluate(board) * board.turn.multiplier()
}

// TT mate score adjustment: mate scores are ply-relative, but TT stores
// them distance-from-root so they remain valid across different search depths.
fn tt_store_score(score: i32, ply: usize) -> i32 {
    if score >= MATE_SCORE - MAX_PLY as i32 {
        score + ply as i32
    } else if score <= -(MATE_SCORE - MAX_PLY as i32) {
        score - ply as i32
    } else {
        score
    }
}

fn tt_retrieve_score(stored: i32, ply: usize) -> i32 {
    if stored >= MATE_SCORE - MAX_PLY as i32 {
        stored - ply as i32
    } else if stored <= -(MATE_SCORE - MAX_PLY as i32) {
        stored + ply as i32
    } else {
        stored
    }
}

// ---------------------------------------------------------------------------
// SearchThread — per-thread state (killers, history, shared TT & stop flag)
// ---------------------------------------------------------------------------

pub struct SearchThread {
    tt: Arc<TranspositionTable>,
    stop: Arc<AtomicBool>,
    best_result: Arc<Mutex<Option<(Move, i32, u32)>>>,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
    nodes: u64,
}

impl SearchThread {
    pub fn new(
        tt: Arc<TranspositionTable>,
        stop: Arc<AtomicBool>,
        best_result: Arc<Mutex<Option<(Move, i32, u32)>>>,
    ) -> Self {
        Self {
            tt,
            stop,
            best_result,
            killers: [[None; 2]; MAX_PLY],
            history: [[0i32; 64]; 64],
            nodes: 0,
        }
    }

    /// Full iterative deepening loop for one Lazy SMP thread.
    pub fn iterative_deepen(&mut self, board: &Board, thread_id: u32) {
        let color = board.turn;
        let mut depth = 1u32;
        let mut last_score = 0;

        loop {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }

            // Slightly perturb search depth per thread to diversify
            let thread_depth = depth + (thread_id % 3);

            let (score, best_mv) = self.search_root(board, thread_depth, color);

            if !self.stop.load(Ordering::Relaxed) {
                last_score = score;

                // Update global best result
                if let Ok(mut guard) = self.best_result.lock() {
                    match guard.as_ref() {
                        Some((_, _, d)) if thread_depth > *d => {
                            *guard = Some((best_mv, score, thread_depth));
                        }
                        None => {
                            *guard = Some((best_mv, score, thread_depth));
                        }
                        _ => {}
                    }
                }
            }

            depth += 1;

            // Early exit on forced mate
            if score.abs() >= MATE_SCORE - 100 {
                break;
            }
        }

        if last_score.abs() >= MATE_SCORE - 100 {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    // ----- Root search with aspiration windows -----

    fn search_root(&mut self, board: &Board, depth: u32, color: Color) -> (i32, Move) {
        let mut moves = board.generate_legal_moves(color);
        if moves.is_empty() {
            return (0, Move::NULL);
        }

        // Move ordering: score captures by MVV-LVA, quiet by history
        self.score_moves(&mut moves, board, 0, None);

        // Aspiration window
        let mut alpha = -INF;
        let mut beta = INF;
        let mut delta = 75;

        let mut best_score;
        let mut best_move;

        // First move with full window for PVS
        {
            let first_mv = moves[0];
            let child = board.make_move(&first_mv);
            best_score = -self.alpha_beta(&child, depth - 1, -beta, -alpha, 1);
            best_move = first_mv;
        }

        if self.stop.load(Ordering::Relaxed) {
            return (best_score, best_move);
        }

        // PVS on remaining moves with aspiration sizing
        alpha = best_score - delta;
        beta = best_score + delta;

        for &mv in &moves[1..] {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }

            let child = board.make_move(&mv);

            // Zero-window search first (PVS)
            let mut score = -self.alpha_beta(&child, depth - 1, -alpha - 1, -alpha, 1);

            // Fail-high: re-search with full window
            if score > alpha && score < beta {
                score = -self.alpha_beta(&child, depth - 1, -beta, -alpha, 1);
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
            }

            if score >= beta {
                break;
            }

            // Widen aspiration window if needed
            if score <= alpha {
                alpha = (alpha - delta).max(-INF);
                delta += delta / 2;
                // Re-search this move with wider window
                score = -self.alpha_beta(&child, depth - 1, -beta, -alpha, 1);
                if score > best_score {
                    best_score = score;
                    best_move = mv;
                }
            }

            alpha = best_score - delta;
            beta = best_score + delta;
        }

        (best_score, best_move)
    }

    // ----- Alpha-beta with PVS, null-move, futility pruning -----

    fn alpha_beta(
        &mut self,
        board: &Board,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        ply: usize,
    ) -> i32 {
        self.nodes += 1;

        // Check stop flag periodically
        if self.nodes & 2047 == 0 && self.stop.load(Ordering::Relaxed) {
            return 0;
        }

        if ply >= MAX_PLY {
            return eval_relative(board);
        }

        // TT lookup
        let key = zobrist_key(board);
        let hash_move = if let Some(entry) = self.tt.get(key) {
            if entry.depth >= depth {
                let adjusted = tt_retrieve_score(entry.score, ply);
                match entry.flag {
                    TTFlag::Exact => return adjusted,
                    TTFlag::LowerBound => {
                        if adjusted >= beta {
                            return adjusted;
                        }
                    }
                    TTFlag::UpperBound => {
                        if adjusted <= alpha {
                            return adjusted;
                        }
                    }
                }
            }
            entry.best_move
        } else {
            None
        };

        // Check for check (used by null-move and checkmate detection)
        let in_check = board.is_in_check(board.turn);

        // ----- Null-move pruning -----
        if !in_check && depth >= 3 {
            let r = null_move_r(depth);
            let null_depth = depth.saturating_sub(1 + r);
            if null_depth > 0 {
                let null_board = board.null_move();
                let score = -self.alpha_beta(&null_board, null_depth, -beta, -beta + 1, ply + 1);
                if score >= beta {
                    return beta;
                }
            }
        }

        // ----- Check extension -----
        let mut ext = 0u32;
        if in_check {
            ext = 1;
        }

        // Leaf node => quiescence
        if depth + ext == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let mut moves = board.generate_legal_moves(board.turn);
        if moves.is_empty() {
            return if in_check {
                -(MATE_SCORE - ply as i32)
            } else {
                0
            };
        }

        self.score_moves(&mut moves, board, ply, hash_move.as_ref());

        let mut best_score = -INF;
        let mut best_move = None;
        let mut flag = TTFlag::UpperBound;
        let mut moves_searched = 0u32;

        for &mv in &moves {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }

            let child = board.make_move(&mv);

            // ----- Futility pruning (depth <= 3, quiet moves) -----
            if depth + ext <= 3 && !in_check && !mv.capture && moves_searched > 0 {
                let stand_pat = eval_relative(board);
                if stand_pat + futility_margin(depth + ext) <= alpha {
                    continue;
                }
            }

            // ----- Late Move Reductions (LMR) -----
            let mut reduction = 0u32;
            if moves_searched >= 4 && depth >= 3 && !in_check && !mv.capture {
                reduction = 1 + (moves_searched - 4) / 5;
                reduction = reduction.min(depth - 1);
            }

            let search_depth = (depth + ext).saturating_sub(1 + reduction);
            let mut score;

            if moves_searched == 0 {
                // First move: full window
                score = -self.alpha_beta(&child, search_depth, -beta, -alpha, ply + 1);
            } else {
                // PVS: zero-window search
                score = -self.alpha_beta(&child, search_depth, -alpha - 1, -alpha, ply + 1);

                // If fail-high and we reduced, do a reduced re-search first
                if score > alpha && reduction > 0 {
                    score = -self.alpha_beta(
                        &child,
                        search_depth + reduction,
                        -alpha - 1,
                        -alpha,
                        ply + 1,
                    );
                }

                // Full re-search if PV-node candidate
                if score > alpha && score < beta {
                    score = -self.alpha_beta(&child, depth + ext - 1, -beta, -alpha, ply + 1);
                }
            }

            moves_searched += 1;

            if score > best_score {
                best_score = score;
                best_move = Some(mv);
            }

            if score >= beta {
                flag = TTFlag::LowerBound;

                // Store killer move
                if !mv.capture {
                    self.store_killer(mv, ply);
                    self.history[mv.from as usize][mv.to as usize] +=
                        (depth as i32 * depth as i32).min(400);
                    // Decay history to prevent saturation
                    if self.history[mv.from as usize][mv.to as usize] > 100_000 {
                        for row in &mut self.history {
                            for h in row {
                                *h /= 2;
                            }
                        }
                    }
                }
                break;
            }

            if score > alpha {
                alpha = score;
                flag = TTFlag::Exact;
            }
        }

        // Store in TT
        self.tt.store(
            key,
            TTEntry {
                score: tt_store_score(best_score, ply),
                depth: depth + ext,
                flag,
                best_move,
            },
        );

        best_score
    }

    // ----- Quiescence with delta pruning and standing pat -----

    fn quiescence(&mut self, board: &Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;

        if self.nodes & 1023 == 0 && self.stop.load(Ordering::Relaxed) {
            return eval_relative(board);
        }

        if ply >= MAX_PLY {
            return eval_relative(board);
        }

        // Standing pat
        let stand_pat = eval_relative(board);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        // Generate captures only
        let mut captures = board.generate_legal_captures(board.turn);

        // Score and sort captures by MVV-LVA (best first)
        self.score_captures(&mut captures, board);

        let mut best_score = stand_pat;

        for &mv in captures.iter().rev() {
            // Delta pruning: skip captures that can't possibly raise alpha
            let victim_value = board
                .get(mv.to as usize)
                .map(|(_, k)| k.value())
                .unwrap_or(0);

            // MVV-LVA delta margin
            if stand_pat + victim_value + 200 <= alpha {
                continue;
            }

            let child = board.make_move(&mv);
            let score = -self.quiescence(&child, -beta, -alpha, ply + 1);

            if score > best_score {
                best_score = score;
            }

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        best_score
    }

    // ----- Move ordering -----

    fn move_score(&self, mv: &Move, board: &Board, ply: usize, hash_move: Option<&Move>) -> i32 {
        // Hash move gets highest priority
        if hash_move.is_some_and(|hm| hm.from == mv.from && hm.to == mv.to) {
            return 1_000_000;
        }

        // Captures ordered by a simple SEE approximation:
        //   winning trades → near hash-move priority
        //   equal trades   → above killers
        //   losing trades  → above history but below killers
        if mv.capture {
            let victim = board
                .get(mv.to as usize)
                .map(|(_, k)| k.value())
                .unwrap_or(0);
            let attacker = board
                .get(mv.from as usize)
                .map(|(_, k)| k.value())
                .unwrap_or(0);
            let see = victim - attacker;
            if see > 0 {
                return 800_000 + see * 10;
            } else if see == 0 {
                return 500_000;
            } else {
                return (200_000 + see * 10).max(0);
            }
        }

        // Killer moves
        if ply < MAX_PLY {
            if self.killers[ply][0].is_some_and(|k| k.from == mv.from && k.to == mv.to) {
                return 400_000;
            }
            if self.killers[ply][1].is_some_and(|k| k.from == mv.from && k.to == mv.to) {
                return 390_000;
            }
        }

        // History heuristic
        let hist = self.history[mv.from as usize][mv.to as usize];
        if hist > 0 {
            return hist;
        }

        0
    }

    fn score_moves(&self, moves: &mut [Move], board: &Board, ply: usize, hash_move: Option<&Move>) {
        moves.sort_by(|a, b| {
            let sa = self.move_score(a, board, ply, hash_move);
            let sb = self.move_score(b, board, ply, hash_move);
            sb.cmp(&sa)
        });
    }

    fn score_captures(&self, captures: &mut [Move], board: &Board) {
        captures.sort_by_key(|mv| {
            let victim = board
                .get(mv.to as usize)
                .map(|(_, k)| k.value())
                .unwrap_or(0);
            let attacker = board
                .get(mv.from as usize)
                .map(|(_, k)| k.value())
                .unwrap_or(0);
            let see = victim - attacker;
            if see > 0 {
                800_000 + see * 10
            } else if see == 0 {
                500_000
            } else {
                (200_000 + see * 10).max(0)
            }
        });
    }

    fn store_killer(&mut self, mv: Move, ply: usize) {
        if ply >= MAX_PLY {
            return;
        }

        // Don't store same move twice
        if self.killers[ply][0].is_some_and(|k| k.from == mv.from && k.to == mv.to) {
            return;
        }

        // Shift: killer[0] -> killer[1], new -> killer[0]
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = Some(mv);
    }
}

// ---------------------------------------------------------------------------
// Public API: find_best_move with Lazy SMP
// ---------------------------------------------------------------------------

pub fn find_best_move(board: &Board, time_limit: Duration) -> Option<SearchResult> {
    let tt = Arc::new(TranspositionTable::new());
    let stop = Arc::new(AtomicBool::new(false));
    let best_result: Arc<Mutex<Option<(Move, i32, u32)>>> = Arc::new(Mutex::new(None));

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(1);

    // Timeout thread
    let stop_clone = stop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(time_limit);
        stop_clone.store(true, Ordering::SeqCst);
    });

    // Search threads
    let mut handles = Vec::with_capacity(num_threads);
    for thread_id in 0..num_threads {
        let tt = tt.clone();
        let stop = stop.clone();
        let best = best_result.clone();
        let b = board.clone();

        handles.push(std::thread::spawn(move || {
            let mut searcher = SearchThread::new(tt, stop, best);
            searcher.iterative_deepen(&b, thread_id as u32);
        }));
    }

    // Wait for all search threads to finish
    for h in handles {
        let _ = h.join();
    }

    {
        let guard = best_result.lock().unwrap();
        if let Some(&(mv, score, depth)) = guard.as_ref() {
            return Some(SearchResult {
                best_move: mv,
                score,
                depth,
            });
        }
    }
    board
        .generate_legal_moves(board.turn)
        .first()
        .copied()
        .map(|mv| SearchResult {
            best_move: mv,
            score: 0,
            depth: 0,
        })
}
