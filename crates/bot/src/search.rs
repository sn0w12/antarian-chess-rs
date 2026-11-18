use crate::evaluation::{evaluate, has_major_piece};
use crate::tt::{TTEntry, TTFlag, TranspositionTable, zobrist_key};
use chess_engine::board::{coord_to_index, index_to_coord};
use chess_engine::{Board, Color, Move, PieceKind};
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
    /// Nodes visited by the thread that produced this result.
    pub nodes: u64,
}

pub const MATE_SCORE: i32 = 90_000;
const INF: i32 = 999_999;
const MAX_PLY: usize = 128;

type SharedBest = Arc<Mutex<Option<(Move, i32, u32, u64)>>>;

/// Full-move distance to mate when `score` is a forced-mate score.
pub fn mate_distance(score: i32) -> Option<u32> {
    let abs = score.abs();
    if abs >= MATE_SCORE - MAX_PLY as i32 {
        Some(((MATE_SCORE - abs + 1) / 2) as u32)
    } else {
        None
    }
}

// Null-move reduction: R = 3 + depth/4.
fn null_move_r(depth: u32) -> u32 {
    3 + depth / 4
}

// Max possible positional gain in one quiet move, by remaining depth.
fn futility_margin(depth: u32) -> i32 {
    match depth {
        0 => 0,
        1 => 120,
        2 => 260,
        3 => 420,
        4 => 600,
        5 => 800,
        _ => 1000,
    }
}

// Quiet moves searched before the rest are discarded at shallow depth.
fn late_move_threshold(depth: u32) -> u32 {
    if depth <= 4 {
        5 + depth * depth
    } else {
        u32::MAX
    }
}

/// Late move reduction. With 30 to 60 legal moves in this variant, anything
/// past the first handful is nearly always worse, so late quiet moves lose a
/// ply of depth every couple of move indices. Capped at `depth - 1`, which
/// sends the very last moves straight to quiescence.
fn lmr_reduction(depth: u32, moves: u32) -> u32 {
    if depth < 2 || moves < 2 {
        return 0;
    }
    (1 + moves.saturating_sub(2) / 2).min(depth - 1)
}

fn see_score(mv: &Move, board: &Board) -> i32 {
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
}

fn cheapest_attacker(board: &Board, mask: u64) -> Option<(usize, i32)> {
    let mut best: Option<(usize, i32)> = None;
    let mut bits = mask;
    while bits != 0 {
        let sq = bits.trailing_zeros() as usize;
        bits &= bits - 1;
        let value = board.get(sq).map(|(_, k)| k.value()).unwrap_or(0);
        if best.is_none_or(|(_, v)| value < v) {
            best = Some((sq, value));
        }
    }
    best
}

/// Slider of `color` uncovered on the far side of `removed`, looking outward
/// from `square`. Captures only reveal attackers on the line they travel.
fn xray_attacker(board: &Board, square: usize, removed: usize, color: Color, occupied: u64) -> u64 {
    let (tf, tr) = index_to_coord(square);
    let (rf, rr) = index_to_coord(removed);
    let df = (rf - tf).signum();
    let dr = (rr - tr).signum();
    if df == 0 && dr == 0 {
        return 0;
    }

    let orthogonal = dr == 0;
    let diagonal = df != 0 && dr != 0;
    let mut file = rf + df;
    let mut rank = rr + dr;
    while let Some(idx) = coord_to_index(file, rank) {
        if occupied & (1u64 << idx) != 0 {
            let Some((pc, kind)) = board.get(idx) else {
                return 0;
            };
            if pc != color {
                return 0;
            }
            let slides = match kind {
                PieceKind::Empress => true,
                PieceKind::Priest => diagonal,
                PieceKind::Dragon => orthogonal,
                _ => false,
            };
            return if slides { 1u64 << idx } else { 0 };
        }
        file += df;
        rank += dr;
    }
    0
}

/// Static exchange evaluation: material the mover nets on `mv.to` when both
/// sides keep recapturing with their cheapest piece, with each side allowed to
/// stop once recapturing costs more than it wins.
fn see(board: &Board, mv: &Move) -> i32 {
    let to = mv.to as usize;
    let from = mv.from as usize;
    let Some((_, victim)) = board.get(to) else {
        return 0;
    };

    let opponent = board.turn.opposite();
    let defenders = board.attackers_to(to, opponent);
    if defenders == 0 {
        return victim.value();
    }

    let mover_is_white = board.turn == Color::White;
    let mover = usize::from(!mover_is_white);
    let mut occupied = board.occupied();
    occupied &= !(1u64 << to);
    occupied &= !(1u64 << from);

    let mut attackers = [board.attackers_to(to, Color::White), defenders];
    attackers[mover] &= !(1u64 << from);
    attackers[mover] |= xray_attacker(board, to, from, board.turn, occupied);

    let mut sequence = [0i32; 16];
    sequence[0] = board.get(from).map(|(_, k)| k.value()).unwrap_or(0);
    let mut len = 1;
    let mut side = 1 - mover;
    while len < sequence.len() {
        let Some((sq, value)) = cheapest_attacker(board, attackers[side]) else {
            break;
        };
        let side_color = if side == 0 {
            Color::White
        } else {
            Color::Black
        };
        attackers[side] &= !(1u64 << sq);
        occupied &= !(1u64 << sq);
        attackers[side] |= xray_attacker(board, to, sq, side_color, occupied);
        sequence[len] = value;
        len += 1;
        side = 1 - side;
    }

    let mut reply = 0;
    for i in (1..len).rev() {
        reply = (sequence[i - 1] - reply).max(0);
    }
    victim.value() - reply
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
    best_result: SharedBest,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
    counter: [[Option<Move>; 64]; 6],
    played: [Option<Move>; MAX_PLY],
    root_prev: Vec<(Move, i32)>,
    root_depth: u32,
    aborted: bool,
    nodes: u64,
}

impl SearchThread {
    pub fn new(
        tt: Arc<TranspositionTable>,
        stop: Arc<AtomicBool>,
        best_result: SharedBest,
    ) -> Self {
        Self {
            tt,
            stop,
            best_result,
            killers: [[None; 2]; MAX_PLY],
            history: [[0i32; 64]; 64],
            counter: [[None; 64]; 6],
            played: [None; MAX_PLY],
            root_prev: Vec::new(),
            root_depth: 0,
            aborted: false,
            nodes: 0,
        }
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Full iterative deepening loop for one Lazy SMP thread.
    pub fn iterative_deepen(&mut self, board: &Board, thread_id: u32) {
        let color = board.turn;
        // Stagger the starting depth so threads do not duplicate the same
        // iterations in lockstep.
        let mut depth = 1 + thread_id % 4;
        let mut last_score = 0;

        loop {
            if self.stopped() {
                break;
            }

            let thread_depth = depth;
            self.root_depth = thread_depth;

            // Search around the previous score; a miss widens the window and
            // repeats instead of paying for a full-width search up front.
            let mut delta = 40;
            let mut alpha = if thread_depth >= 5 {
                last_score - delta
            } else {
                -INF
            };
            let mut beta = if thread_depth >= 5 {
                last_score + delta
            } else {
                INF
            };

            let (score, best_mv) = loop {
                let (score, best_mv) = self.search_root(board, thread_depth, color, alpha, beta);
                if self.aborted || self.stopped() || best_mv == Move::NULL {
                    break (score, best_mv);
                }
                if score <= alpha {
                    alpha = (alpha - delta).max(-INF);
                    delta = delta.saturating_mul(2);
                } else if score >= beta {
                    beta = (beta + delta).min(INF);
                    delta = delta.saturating_mul(2);
                } else {
                    break (score, best_mv);
                }
            };

            if self.aborted || self.stopped() || best_mv == Move::NULL {
                break;
            }

            last_score = score;

            // Update global best result
            if let Ok(mut guard) = self.best_result.lock() {
                match guard.as_ref() {
                    Some((_, _, d, _)) if thread_depth > *d => {
                        *guard = Some((best_mv, score, thread_depth, self.nodes));
                    }
                    None => {
                        *guard = Some((best_mv, score, thread_depth, self.nodes));
                    }
                    _ => {}
                }
            }

            depth += 1;

            // Early exit on forced mate
            if score.abs() >= MATE_SCORE - MAX_PLY as i32 {
                break;
            }
        }

        if !self.aborted && last_score.abs() >= MATE_SCORE - MAX_PLY as i32 {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    // ----- Root search with PVS -----

    fn search_root(
        &mut self,
        board: &Board,
        depth: u32,
        color: Color,
        mut alpha: i32,
        beta: i32,
    ) -> (i32, Move) {
        let mut moves = board.generate_legal_moves(color);
        if moves.is_empty() {
            return (0, Move::NULL);
        }

        // Later iterations order by the previous scores, which are a much
        // better predictor than the static heuristics.
        self.played[0] = None;
        if self.root_prev.is_empty() {
            let hash_move = self.tt.get(board.zobrist).and_then(|entry| entry.best_move);
            self.score_moves(&mut moves, board, 0, hash_move.as_ref());
        } else {
            let mut ranked: Vec<(Move, i32)> = moves
                .iter()
                .map(|&mv| {
                    let prev = self
                        .root_prev
                        .iter()
                        .find(|(m, _)| m.from == mv.from && m.to == mv.to)
                        .map(|(_, s)| *s)
                        .unwrap_or(-INF);
                    (mv, prev)
                })
                .collect();
            ranked.sort_by_key(|(_, score)| -*score);
            moves = ranked.into_iter().map(|(mv, _)| mv).collect();
        }

        let mut best_score = alpha;
        let mut best_move = moves[0];
        let mut scored = Vec::with_capacity(moves.len());

        for (index, &mv) in moves.iter().enumerate() {
            if self.stopped() {
                self.aborted = true;
                return (best_score, best_move);
            }

            let child = board.make_move(&mv);

            // The first move gets the window; later moves get a null window
            // and are re-searched at full width when they beat the best so far.
            let mut score = if index == 0 {
                -self.alpha_beta(&child, depth - 1, -beta, -alpha, 1)
            } else {
                -self.alpha_beta(&child, depth - 1, -best_score - 1, -best_score, 1)
            };

            if score > best_score && index > 0 {
                score = -self.alpha_beta(&child, depth - 1, -beta, -alpha, 1);
            }

            if self.aborted {
                return (best_score, best_move);
            }

            scored.push((mv, score));

            if score > best_score {
                best_score = score;
                alpha = score;
                best_move = mv;
            }

            if score >= beta {
                // The caller widens the window and repeats; the remaining moves
                // would be searched against a bound that cannot improve.
                return (score, mv);
            }
        }

        self.root_prev = scored;
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
        if self.nodes & 1023 == 0 && self.stopped() {
            self.aborted = true;
        }
        if self.aborted {
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

        // Nodes without a TT move search one ply shallower; the extra ply is
        // rarely worth its cost when the move ordering has nothing to start
        // from.
        let mut depth = depth;
        if hash_move.is_none() && depth >= 4 {
            depth -= 1;
        }

        // Check for check (used by null-move and checkmate detection)
        let in_check = board.is_in_check(board.turn);
        let static_eval = if in_check { 0 } else { eval_relative(board) };
        let window_is_null = beta - alpha <= 1;
        let mate_window = beta.abs() < MATE_SCORE - MAX_PLY as i32;

        // ----- Reverse futility pruning -----
        // A null-window node whose static score already beats beta is unlikely
        // to be improved by searching; the margin scales with the depth.
        if !in_check && window_is_null && mate_window && depth <= 8 {
            let margin = 100 * depth as i32;
            if static_eval - margin >= beta {
                return static_eval;
            }
        }

        // ----- Razoring -----
        // Far below alpha at shallow depth: quiescence decides whether the
        // position is really that bad.
        if !in_check && window_is_null && depth <= 3 {
            let margin = 300 * depth as i32;
            if static_eval + margin < alpha {
                let score = self.quiescence(board, alpha, alpha + 1, ply);
                if self.aborted {
                    return 0;
                }
                if score <= alpha {
                    return score;
                }
            }
        }

        // ----- Null-move pruning -----
        // Positions with no piece material are zugzwang-prone, so the null move
        // is not a reliable bound there.
        if !in_check
            && window_is_null
            && depth >= 3
            && has_major_piece(board, board.turn)
            && has_major_piece(board, board.turn.opposite())
        {
            let r = null_move_r(depth) + u32::from(static_eval >= beta);
            let null_depth = depth.saturating_sub(1 + r);
            if null_depth > 0 {
                let null_board = board.null_move();
                let score = -self.alpha_beta(&null_board, null_depth, -beta, -beta + 1, ply + 1);
                if self.aborted {
                    return 0;
                }
                if score >= beta {
                    return score;
                }
            }
        }

        // ----- Check extension -----
        // Bounded so a run of checks cannot extend the tree without limit.
        let mut ext = 0u32;
        if in_check && (ply as u32) < 2 * self.root_depth {
            ext = 1;
        }

        // Leaf node => quiescence
        if depth + ext == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let mut moves = board.generate_legal_moves_with_check(board.turn, in_check);
        if moves.is_empty() {
            return if in_check {
                -(MATE_SCORE - ply as i32)
            } else {
                0
            };
        }

        self.score_moves(&mut moves, board, ply, hash_move.as_ref());

        let futility_base = if depth + ext <= 6 && !in_check {
            Some(static_eval)
        } else {
            None
        };
        let lmp_threshold = late_move_threshold(depth + ext);

        let mut best_score = -INF;
        let mut best_move = None;
        let mut flag = TTFlag::UpperBound;
        let mut moves_searched = 0u32;

        for &mv in &moves {
            if self.stopped() {
                self.aborted = true;
                break;
            }

            let child = board.make_move(&mv);
            let gives_check = child.is_in_check(child.turn);
            self.played[ply] = Some(mv);

            // ----- Bad capture pruning (shallow, non-checking) -----
            if !in_check && !gives_check && mv.capture && moves_searched > 0 && depth + ext <= 6 {
                let victim = board
                    .get(mv.to as usize)
                    .map(|(_, k)| k.value())
                    .unwrap_or(0);
                let attacker = board
                    .get(mv.from as usize)
                    .map(|(_, k)| k.value())
                    .unwrap_or(0);
                if victim <= attacker && see(board, &mv) < -75 * (depth + ext) as i32 {
                    continue;
                }
            }

            // ----- Futility and late move pruning (empty window, shallow) -----
            if let Some(base) = futility_base
                && window_is_null
                && !mv.capture
                && !gives_check
                && moves_searched > 0
                && (base + futility_margin(depth + ext) <= alpha || moves_searched >= lmp_threshold)
            {
                continue;
            }

            // ----- Late Move Reductions (LMR) -----
            let mut reduction = 0u32;
            if depth + ext >= 2 && moves_searched >= 2 && !in_check {
                reduction = lmr_reduction(depth + ext, moves_searched);
                if mv.capture {
                    reduction = reduction.saturating_sub(1);
                } else if gives_check || self.is_killer(&mv, ply) {
                    reduction = 0;
                }
                reduction = reduction.min(depth + ext - 1);
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

            if self.aborted {
                return 0;
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
                    if ply > 0
                        && let Some(prev) = self.played[ply - 1]
                        && let Some((_, kind)) = board.get(prev.to as usize)
                    {
                        self.counter[kind.index()][prev.to as usize] = Some(mv);
                    }
                }
                break;
            }

            if score > alpha {
                alpha = score;
                flag = TTFlag::Exact;
            }
        }

        if self.aborted {
            return best_score;
        }

        // Nothing was searched, so there is no bound worth storing.
        if best_move.is_none() {
            return eval_relative(board);
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

        if self.nodes & 1023 == 0 && self.stopped() {
            self.aborted = true;
        }
        if self.aborted {
            return 0;
        }

        if ply >= MAX_PLY {
            return eval_relative(board);
        }

        let key = zobrist_key(board);
        if let Some(entry) = self.tt.get(key) {
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

        let original_alpha = alpha;
        let in_check = board.is_in_check(board.turn);

        let mut best_score;
        let mut stand_pat = -INF;
        let mut moves;

        if in_check {
            // A checked side cannot stand pat: every evasion must be searched
            // or the node scores as if the check did not exist.
            moves = board.generate_legal_moves_with_check(board.turn, true);
            if moves.is_empty() {
                return -(MATE_SCORE - ply as i32);
            }
            self.score_moves(&mut moves, board, ply, None);
            best_score = -INF;
        } else {
            // Standing pat
            stand_pat = eval_relative(board);
            if stand_pat >= beta {
                return stand_pat;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
            best_score = stand_pat;

            moves = board.generate_legal_captures_with_check(board.turn, false);
            self.score_captures(&mut moves, board);
        }

        let mut cutoff = false;

        if in_check {
            for &mv in &moves {
                let child = board.make_move(&mv);
                let score = -self.quiescence(&child, -beta, -alpha, ply + 1);
                if self.aborted {
                    return 0;
                }
                if score > best_score {
                    best_score = score;
                }
                if score >= beta {
                    cutoff = true;
                    break;
                }
                if score > alpha {
                    alpha = score;
                }
            }
        } else {
            for &mv in &moves {
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
                if !child.is_in_check(child.turn) {
                    let attacker = board
                        .get(mv.from as usize)
                        .map(|(_, k)| k.value())
                        .unwrap_or(0);
                    if victim_value <= attacker && see(board, &mv) < 0 {
                        continue;
                    }
                }

                let score = -self.quiescence(&child, -beta, -alpha, ply + 1);
                if self.aborted {
                    return 0;
                }

                if score > best_score {
                    best_score = score;
                }

                if score >= beta {
                    cutoff = true;
                    break;
                }
                if score > alpha {
                    alpha = score;
                }
            }
        }

        let flag = if cutoff {
            TTFlag::LowerBound
        } else if best_score > original_alpha {
            TTFlag::Exact
        } else {
            TTFlag::UpperBound
        };
        self.tt.store(
            key,
            TTEntry {
                score: tt_store_score(best_score, ply),
                depth: 0,
                flag,
                best_move: None,
            },
        );

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
            return see_score(mv, board);
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

        // Counter move: the reply that worked against the previous move.
        if ply > 0
            && let Some(prev) = self.played[ply - 1]
            && let Some((_, kind)) = board.get(prev.to as usize)
            && self.counter[kind.index()][prev.to as usize]
                .is_some_and(|c| c.from == mv.from && c.to == mv.to)
        {
            return 350_000;
        }

        // History heuristic
        let hist = self.history[mv.from as usize][mv.to as usize];
        if hist > 0 {
            return hist;
        }

        0
    }

    fn score_moves(&self, moves: &mut [Move], board: &Board, ply: usize, hash_move: Option<&Move>) {
        const BUF: usize = 96;
        if moves.len() > BUF {
            moves.sort_by(|a, b| {
                let sa = self.move_score(a, board, ply, hash_move);
                let sb = self.move_score(b, board, ply, hash_move);
                sb.cmp(&sa)
            });
            return;
        }

        let mut keyed = [(0i32, Move::NULL); BUF];
        for (slot, &mv) in keyed.iter_mut().zip(moves.iter()) {
            *slot = (self.move_score(&mv, board, ply, hash_move), mv);
        }
        let n = moves.len();
        keyed[..n].sort_by(|a, b| b.0.cmp(&a.0));
        for (slot, &(_, mv)) in moves.iter_mut().zip(keyed[..n].iter()) {
            *slot = mv;
        }
    }

    fn score_captures(&self, captures: &mut [Move], board: &Board) {
        const BUF: usize = 96;
        if captures.len() > BUF {
            captures.sort_by(|a, b| see_score(b, board).cmp(&see_score(a, board)));
            return;
        }

        let mut keyed = [(0i32, Move::NULL); BUF];
        for (slot, &mv) in keyed.iter_mut().zip(captures.iter()) {
            *slot = (see_score(&mv, board), mv);
        }
        let n = captures.len();
        keyed[..n].sort_by(|a, b| b.0.cmp(&a.0));
        for (slot, &(_, mv)) in captures.iter_mut().zip(keyed[..n].iter()) {
            *slot = mv;
        }
    }

    fn is_killer(&self, mv: &Move, ply: usize) -> bool {
        self.killers[ply][0].is_some_and(|k| k.from == mv.from && k.to == mv.to)
            || self.killers[ply][1].is_some_and(|k| k.from == mv.from && k.to == mv.to)
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

/// Reusable search state. The transposition table is kept between calls so
/// later moves build on the subtrees the previous search already resolved.
#[derive(Clone, Default)]
pub struct Searcher {
    tt: Arc<TranspositionTable>,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            tt: Arc::new(TranspositionTable::new()),
        }
    }

    pub fn clear(&self) {
        self.tt.clear();
    }

    pub fn find_best_move(&self, board: &Board, time_limit: Duration) -> Option<SearchResult> {
        let tt = self.tt.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let best_result: SharedBest = Arc::new(Mutex::new(None));

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
            if let Some(&(mv, score, depth, nodes)) = guard.as_ref()
                && mv != Move::NULL
            {
                return Some(SearchResult {
                    best_move: mv,
                    score,
                    depth,
                    nodes,
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
                nodes: 0,
            })
    }
}

/// One-shot search with a fresh transposition table.
pub fn find_best_move(board: &Board, time_limit: Duration) -> Option<SearchResult> {
    Searcher::new().find_best_move(board, time_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_engine::PieceKind;

    fn board_with(pieces: &[(usize, Color, PieceKind)], turn: Color) -> Board {
        let mut board = Board::empty();
        for &(sq, color, kind) in pieces {
            board.set(sq, Some((color, kind)));
        }
        board.turn = turn;
        board
    }

    #[test]
    fn see_scores_the_recapture_chain() {
        let free_capture = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),
                (12, Color::White, PieceKind::Empress),
                (20, Color::Black, PieceKind::Priest),
                (63, Color::Black, PieceKind::Emperor),
            ],
            Color::White,
        );
        assert_eq!(see(&free_capture, &Move::new(12, 20, true)), 650);

        let defended_by_slider = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),
                (12, Color::White, PieceKind::Empress),
                (20, Color::Black, PieceKind::Knight),
                (60, Color::Black, PieceKind::Dragon),
                (63, Color::Black, PieceKind::Emperor),
            ],
            Color::White,
        );
        assert_eq!(see(&defended_by_slider, &Move::new(12, 20, true)), -800);

        let equal_trade = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),
                (13, Color::White, PieceKind::Knight),
                (20, Color::Black, PieceKind::Knight),
                (60, Color::Black, PieceKind::Empress),
                (63, Color::Black, PieceKind::Emperor),
            ],
            Color::White,
        );
        assert_eq!(see(&equal_trade, &Move::new(13, 20, true)), 0);
    }

    #[test]
    fn see_sees_through_the_capturing_piece() {
        // The empress on e1 only defends e3 once the dragon leaves e2.
        let board = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),
                (4, Color::White, PieceKind::Empress),
                (12, Color::White, PieceKind::Dragon),
                (20, Color::Black, PieceKind::Knight),
                (27, Color::Black, PieceKind::Priest),
                (63, Color::Black, PieceKind::Emperor),
            ],
            Color::White,
        );
        assert_eq!(see(&board, &Move::new(12, 20, true)), 100);
    }
}
