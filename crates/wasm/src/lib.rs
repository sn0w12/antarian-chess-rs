//! WebAssembly entry points for the engine and bot.
//!
//! The module keeps one [`Searcher`] (and therefore one transposition table)
//! alive for the lifetime of the page, plus a set of game states addressed by
//! integer handles. Everything crosses the boundary as UTF-8 JSON in a shared
//! output buffer, except board strings which are 65 ASCII bytes: 64 squares
//! from a1 to h8, then the side to move (`w` or `b`). Pieces are `E Q B P R N`
//! for white and lowercase for black, `.` for an empty square.

use chess_bot::Searcher;
use chess_engine::{Board, Color, GameResult, Move, PieceKind, compute_zobrist};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    static SEARCHER: RefCell<Searcher> = RefCell::new(Searcher::new());
    static GAMES: RefCell<HashMap<u32, Game>> = RefCell::new(HashMap::new());
    static NEXT_GAME_ID: Cell<u32> = const { Cell::new(1) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

struct Game {
    board: Board,
    history: Vec<(Board, Option<Move>)>,
    last_move: Option<Move>,
}

impl Game {
    fn new() -> Self {
        Self {
            board: Board::initial(),
            history: Vec::new(),
            last_move: None,
        }
    }

    fn play(&mut self, from: u32, to: u32, promotion: Option<PieceKind>) -> Option<Move> {
        let from = u8::try_from(from).ok().filter(|square| *square < 64)?;
        let to = u8::try_from(to).ok().filter(|square| *square < 64)?;
        let mv = self
            .board
            .generate_legal_moves(self.board.turn)
            .into_iter()
            .find(|mv| {
                mv.from == from
                    && mv.to == to
                    && promotion.is_none_or(|kind| mv.promotion == Some(kind))
            })?;
        self.history.push((self.board.clone(), self.last_move));
        self.board = self.board.make_move(&mv);
        self.last_move = Some(mv);
        Some(mv)
    }

    fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some((board, last_move)) => {
                self.board = board;
                self.last_move = last_move;
                true
            }
            None => false,
        }
    }
}

/// Allocate `len` bytes for the caller to write a board string into.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buffer = vec![0u8; len];
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// Release a buffer returned by [`alloc`].
///
/// # Safety
/// `pointer` must come from [`alloc`] with the same `len`, and must not be used
/// afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(pointer: *mut u8, len: usize) {
    if !pointer.is_null() {
        drop(unsafe { Vec::from_raw_parts(pointer, len, len) });
    }
}

/// Pointer to the JSON written by the last call that returned a length.
#[unsafe(no_mangle)]
pub extern "C" fn result_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

/// Create a game in the starting position and return its handle.
#[unsafe(no_mangle)]
pub extern "C" fn game_create() -> u32 {
    let id = NEXT_GAME_ID.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1).max(1));
        id
    });
    GAMES.with(|games| games.borrow_mut().insert(id, Game::new()));
    id
}

/// Drop a game and its history.
#[unsafe(no_mangle)]
pub extern "C" fn game_destroy(id: u32) {
    GAMES.with(|games| games.borrow_mut().remove(&id));
}

/// Reset a game to the starting position.
#[unsafe(no_mangle)]
pub extern "C" fn game_reset(id: u32) -> usize {
    with_game(id, |game| {
        *game = Game::new();
        state_json(game)
    })
}

/// Load a 65-byte board string, replacing the current position and history.
///
/// # Safety
/// `pointer` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_load(id: u32, pointer: *const u8, len: usize) -> usize {
    let input = unsafe { std::slice::from_raw_parts(pointer, len) };
    match parse_board(input) {
        Some(board) => with_game(id, |game| {
            game.board = board;
            game.history.clear();
            game.last_move = None;
            state_json(game)
        }),
        None => set_output("{\"error\":\"invalid board\"}".to_string()),
    }
}

/// Current position as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn game_state(id: u32) -> usize {
    with_game(id, |game| state_json(game))
}

/// Legal moves as JSON. Pass `from = 255` for every move on the board.
#[unsafe(no_mangle)]
pub extern "C" fn game_legal_moves(id: u32, from: u32) -> usize {
    with_game(id, |game| {
        let moves = game.board.generate_legal_moves(game.board.turn);
        let mut out = String::with_capacity(moves.len() * 40 + 2);
        out.push('[');
        let mut first = true;
        for mv in moves {
            if from != 255 && mv.from as u32 != from {
                continue;
            }
            if !first {
                out.push(',');
            }
            first = false;
            push_move(&mut out, &mv);
        }
        out.push(']');
        out
    })
}

/// Play a move by coordinates. `promotion` is 0 for none or the choice code
/// (1 empress, 2 priest, 3 paladin, 4 dragon). Returns `{ ok, move, state }`
/// or an error.
#[unsafe(no_mangle)]
pub extern "C" fn game_play(id: u32, from: u32, to: u32, promotion: u32) -> usize {
    with_game(id, |game| {
        match game.play(from, to, promotion_from_code(promotion)) {
            Some(mv) => {
                let mut out = String::from("{\"ok\":true,\"move\":");
                push_move(&mut out, &mv);
                out.push_str(",\"state\":");
                out.push_str(&state_json(game));
                out.push('}');
                out
            }
            None => "{\"ok\":false,\"error\":\"illegal move\"}".to_string(),
        }
    })
}

/// Take back the last move. Returns `{ ok, state }`.
#[unsafe(no_mangle)]
pub extern "C" fn game_undo(id: u32) -> usize {
    with_game(id, |game| {
        if game.undo() {
            let mut out = String::from("{\"ok\":true,\"state\":");
            out.push_str(&state_json(game));
            out.push('}');
            out
        } else {
            "{\"ok\":false,\"error\":\"nothing to undo\"}".to_string()
        }
    })
}

/// Search the current position without playing the move.
#[unsafe(no_mangle)]
pub extern "C" fn game_best_move(id: u32, depth: u32) -> usize {
    with_game(id, |game| match search(game, depth) {
        Some(result) => {
            let mut out = String::from("{\"move\":");
            push_move(&mut out, &result.best_move);
            out.push_str(&format!(
                ",\"score\":{},\"depth\":{},\"nodes\":{}}}",
                result.score, result.depth, result.nodes
            ));
            out
        }
        None => "{\"move\":null,\"error\":\"no legal moves\"}".to_string(),
    })
}

/// Search and play the resulting move.
#[unsafe(no_mangle)]
pub extern "C" fn game_play_best(id: u32, depth: u32) -> usize {
    with_game(id, |game| match search(game, depth) {
        Some(result) => {
            let Some(mv) = game.play(
                result.best_move.from as u32,
                result.best_move.to as u32,
                result.best_move.promotion,
            ) else {
                return "{\"ok\":false,\"error\":\"search returned an illegal move\"}".to_string();
            };
            let mut out = String::from("{\"ok\":true,\"move\":");
            push_move(&mut out, &mv);
            out.push_str(&format!(
                ",\"score\":{},\"depth\":{},\"nodes\":{},\"state\":",
                result.score, result.depth, result.nodes
            ));
            out.push_str(&state_json(game));
            out.push('}');
            out
        }
        None => "{\"ok\":false,\"error\":\"no legal moves\"}".to_string(),
    })
}

/// Stateless search for a 65-byte board string.
///
/// # Safety
/// `pointer` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bot_move(pointer: *const u8, len: usize, depth: u32) -> usize {
    let input = unsafe { std::slice::from_raw_parts(pointer, len) };
    match parse_board(input) {
        Some(board) => match SEARCHER.with(|searcher| {
            searcher
                .borrow()
                .find_best_move_depth(&board, depth.clamp(1, 30))
        }) {
            Some(result) => {
                let mut out = String::from("{\"move\":");
                push_move(&mut out, &result.best_move);
                out.push_str(&format!(
                    ",\"score\":{},\"depth\":{},\"nodes\":{}}}",
                    result.score, result.depth, result.nodes
                ));
                set_output(out)
            }
            None => set_output("{\"error\":\"no legal moves\"}".to_string()),
        },
        None => set_output("{\"error\":\"invalid board\"}".to_string()),
    }
}

fn search(game: &Game, depth: u32) -> Option<chess_bot::SearchResult> {
    SEARCHER.with(|searcher| {
        searcher
            .borrow()
            .find_best_move_depth(&game.board, depth.clamp(1, 30))
    })
}

fn with_game(id: u32, action: impl FnOnce(&mut Game) -> String) -> usize {
    GAMES.with(|games| {
        let mut games = games.borrow_mut();
        match games.get_mut(&id) {
            Some(game) => set_output(action(game)),
            None => set_output("{\"error\":\"unknown game\"}".to_string()),
        }
    })
}

fn set_output(json: String) -> usize {
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.clear();
        output.extend_from_slice(json.as_bytes());
        output.len()
    })
}

fn state_json(game: &Game) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("{\"squares\":[");
    for square in 0..64 {
        if square > 0 {
            out.push(',');
        }
        match game.board.get(square) {
            Some((color, kind)) => push_piece(&mut out, color, kind),
            None => out.push_str("null"),
        }
    }
    out.push_str("],\"turn\":\"");
    out.push_str(color_name(game.board.turn));
    out.push_str("\",\"status\":");
    push_status(&mut out, &game.board);
    out.push_str(",\"moveNumber\":");
    out.push_str(&game.history.len().to_string());
    out.push_str(",\"lastMove\":");
    match game.last_move {
        Some(mv) => push_move(&mut out, &mv),
        None => out.push_str("null"),
    }
    out.push('}');
    out
}

fn push_piece(out: &mut String, color: Color, kind: PieceKind) {
    out.push_str("{\"color\":\"");
    out.push_str(color_name(color));
    out.push_str("\",\"kind\":\"");
    out.push_str(kind_name(kind));
    out.push_str("\"}");
}

fn push_move(out: &mut String, mv: &Move) {
    out.push_str(&format!(
        "{{\"from\":{},\"to\":{},\"capture\":{},\"promotion\":",
        mv.from, mv.to, mv.capture
    ));
    match mv.promotion {
        Some(kind) => {
            out.push('"');
            out.push_str(kind.name());
            out.push('"');
        }
        None => out.push_str("null"),
    }
    out.push('}');
}

fn promotion_from_code(code: u32) -> Option<PieceKind> {
    match code {
        1 => Some(PieceKind::Empress),
        2 => Some(PieceKind::Priest),
        3 => Some(PieceKind::Paladin),
        4 => Some(PieceKind::Dragon),
        _ => None,
    }
}

fn push_status(out: &mut String, board: &Board) {
    match board.game_result() {
        GameResult::Ongoing => {
            out.push_str("{\"kind\":\"ongoing\",\"winner\":null,\"inCheck\":");
            out.push_str(if board.is_in_check(board.turn) {
                "true}"
            } else {
                "false}"
            });
        }
        GameResult::Checkmate { winner } => {
            out.push_str("{\"kind\":\"checkmate\",\"winner\":\"");
            out.push_str(color_name(winner));
            out.push_str("\",\"inCheck\":true}");
        }
        GameResult::Stalemate => {
            out.push_str("{\"kind\":\"stalemate\",\"winner\":null,\"inCheck\":false}");
        }
    }
}

fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Black => "black",
    }
}

fn kind_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Emperor => "emperor",
        PieceKind::Empress => "empress",
        PieceKind::Priest => "priest",
        PieceKind::Paladin => "paladin",
        PieceKind::Dragon => "dragon",
        PieceKind::Knight => "knight",
    }
}

fn parse_board(input: &[u8]) -> Option<Board> {
    if input.len() != 65 {
        return None;
    }

    let mut board = Board::empty();
    for (square, &byte) in input[..64].iter().enumerate() {
        let piece = match byte {
            b'.' => continue,
            b'E' => (Color::White, PieceKind::Emperor),
            b'Q' => (Color::White, PieceKind::Empress),
            b'B' => (Color::White, PieceKind::Priest),
            b'P' => (Color::White, PieceKind::Paladin),
            b'R' => (Color::White, PieceKind::Dragon),
            b'N' => (Color::White, PieceKind::Knight),
            b'e' => (Color::Black, PieceKind::Emperor),
            b'q' => (Color::Black, PieceKind::Empress),
            b'b' => (Color::Black, PieceKind::Priest),
            b'p' => (Color::Black, PieceKind::Paladin),
            b'r' => (Color::Black, PieceKind::Dragon),
            b'n' => (Color::Black, PieceKind::Knight),
            _ => return None,
        };
        board.set(square, Some(piece));
    }

    board.turn = match input[64] {
        b'w' => Color::White,
        b'b' => Color::Black,
        _ => return None,
    };
    board.zobrist = compute_zobrist(board.squares(), board.turn);
    Some(board)
}
