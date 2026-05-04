use crate::pieces;
use crate::types::{Color, GameResult, Move, PieceKind};
use std::sync::OnceLock;

pub const ZOBRIST_SIDE: u64 = 0x9D39247E_33776D41;

static ZOBRIST: OnceLock<[[[u64; 64]; 2]; 6]> = OnceLock::new();

fn get_zobrist() -> &'static [[[u64; 64]; 2]; 6] {
    ZOBRIST.get_or_init(|| {
        // SplitMix64-style PRNG for deterministic zobrist keys
        let mut state: u64 = 0xCAFEBABE_DEADBEEF;
        let mut table = [[[0u64; 64]; 2]; 6];
        for ki_table in &mut table {
            for ci_table in ki_table {
                for sq_val in ci_table {
                    state = state.wrapping_add(0x9E3779B97F4A7C15);
                    let mut z = state;
                    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
                    z ^= z >> 31;
                    *sq_val = z;
                }
            }
        }
        table
    })
}

fn zobrist_piece_square(color: Color, kind: PieceKind, sq: usize) -> u64 {
    get_zobrist()[kind.index()][color_index(color)][sq]
}

fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

/// Compute full zobrist hash for a board from scratch.
pub fn compute_zobrist(squares: &[Option<(Color, PieceKind)>; 64], turn: Color) -> u64 {
    let table = get_zobrist();
    let mut key = 0u64;
    for sq in 0..64 {
        if let Some((color, kind)) = squares[sq] {
            key ^= table[kind.index()][color_index(color)][sq];
        }
    }
    if turn == Color::Black {
        key ^= ZOBRIST_SIDE;
    }
    key
}

#[derive(Clone)]
pub struct Board {
    squares: [Option<(Color, PieceKind)>; 64],
    pub turn: Color,
    pub zobrist: u64,
}

impl Board {
    pub fn empty() -> Self {
        Board {
            squares: [None; 64],
            turn: Color::White,
            zobrist: 0,
        }
    }

    pub fn initial() -> Self {
        let mut board = Self::empty();

        let back = [
            PieceKind::Dragon,
            PieceKind::Priest,
            PieceKind::Paladin,
            PieceKind::Emperor,
            PieceKind::Empress,
            PieceKind::Paladin,
            PieceKind::Priest,
            PieceKind::Dragon,
        ];
        for (file, &kind) in back.iter().enumerate() {
            board.set(file, Some((Color::White, kind)));
        }
        for file in 0..8 {
            board.set(8 + file, Some((Color::White, PieceKind::Knight)));
        }
        for file in 0..8 {
            board.set(48 + file, Some((Color::Black, PieceKind::Knight)));
        }
        for (file, &kind) in back.iter().enumerate() {
            board.set(56 + file, Some((Color::Black, kind)));
        }

        board.zobrist = compute_zobrist(&board.squares, board.turn);
        board
    }

    pub fn set(&mut self, square: usize, piece: Option<(Color, PieceKind)>) {
        self.squares[square] = piece;
    }

    pub fn get(&self, square: usize) -> Option<(Color, PieceKind)> {
        self.squares[square]
    }

    /// Get a reference to the squares array.
    pub fn squares(&self) -> &[Option<(Color, PieceKind)>; 64] {
        &self.squares
    }

    /// Make a move, returning a new board. Zobrist is updated incrementally.
    pub fn make_move(&self, mv: &Move) -> Self {
        let mut new = self.clone();
        let piece = new.squares[mv.from as usize].take();

        // Remove piece from 'from' square in zobrist
        if let Some((color, kind)) = piece {
            new.zobrist ^= zobrist_piece_square(color, kind, mv.from as usize);
        }

        // Remove captured piece from zobrist
        if let Some((cap_color, cap_kind)) = new.squares[mv.to as usize] {
            new.zobrist ^= zobrist_piece_square(cap_color, cap_kind, mv.to as usize);
        }

        new.squares[mv.to as usize] = piece;

        // Add piece to 'to' square in zobrist
        if let Some((color, kind)) = piece {
            new.zobrist ^= zobrist_piece_square(color, kind, mv.to as usize);
        }

        // Flip side to move
        new.zobrist ^= ZOBRIST_SIDE;
        new.turn = self.turn.opposite();
        new
    }

    /// Make a null move — just flip turn. Used for null-move pruning.
    pub fn null_move(&self) -> Self {
        let mut new = self.clone();
        new.zobrist ^= ZOBRIST_SIDE;
        new.turn = self.turn.opposite();
        new
    }

    pub fn generate_moves_for(&self, square: usize, captures_only: bool) -> Vec<Move> {
        if let Some((color, kind)) = self.squares[square] {
            match kind {
                PieceKind::Emperor => {
                    pieces::generate_emperor_moves(self, square, color, captures_only)
                }
                PieceKind::Empress => {
                    pieces::generate_empress_moves(self, square, color, captures_only)
                }
                PieceKind::Priest => {
                    pieces::generate_priest_moves(self, square, color, captures_only)
                }
                PieceKind::Paladin => {
                    pieces::generate_paladin_moves(self, square, color, captures_only)
                }
                PieceKind::Dragon => {
                    pieces::generate_dragon_moves(self, square, color, captures_only)
                }
                PieceKind::Knight => {
                    pieces::generate_knight_moves(self, square, color, captures_only)
                }
            }
        } else {
            Vec::new()
        }
    }

    /// Generate all legal moves for a color.
    pub fn generate_all_moves(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::with_capacity(128);
        for sq in 0..64 {
            if let Some((pc, _)) = self.squares[sq]
                && pc == color
            {
                moves.extend(self.generate_moves_for(sq, false));
            }
        }
        moves
    }

    /// Generate only capture moves for a color. Faster than generating all+filtering.
    pub fn generate_captures(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::with_capacity(32);
        for sq in 0..64 {
            if let Some((pc, _)) = self.squares[sq]
                && pc == color
            {
                moves.extend(self.generate_moves_for(sq, true));
            }
        }
        moves
    }

    // ---- Legal‑move filtering (king safety) ----

    /// Returns `true` if `color`'s emperor is attacked by any opponent piece.
    pub fn is_in_check(&self, color: Color) -> bool {
        let emperor_sq =
            match (0..64).find(|&sq| self.squares[sq] == Some((color, PieceKind::Emperor))) {
                Some(sq) => sq,
                None => return false,
            };
        let opp = color.opposite();
        for sq in 0..64 {
            if self.squares[sq].is_some_and(|(pc, _)| pc == opp)
                && self
                    .generate_moves_for(sq, true)
                    .iter()
                    .any(|mv| mv.to == emperor_sq as u8)
            {
                return true;
            }
        }
        false
    }

    /// Pseudolegal moves that do **not** leave the moving side's own emperor in check.
    pub fn generate_legal_moves(&self, color: Color) -> Vec<Move> {
        let pseudo = self.generate_all_moves(color);
        pseudo
            .into_iter()
            .filter(|mv| {
                let child = self.make_move(mv);
                !child.is_in_check(color)
            })
            .collect()
    }

    /// Legal capture‑only moves (for quiescence search).
    pub fn generate_legal_captures(&self, color: Color) -> Vec<Move> {
        let pseudo = self.generate_captures(color);
        pseudo
            .into_iter()
            .filter(|mv| {
                let child = self.make_move(mv);
                !child.is_in_check(color)
            })
            .collect()
    }

    /// Determine the game state from the current player's perspective.
    pub fn game_result(&self) -> GameResult {
        let turn = self.turn;
        let has_legal = !self.generate_legal_moves(turn).is_empty();
        if has_legal {
            return GameResult::Ongoing;
        }
        if self.is_in_check(turn) {
            GameResult::Checkmate {
                winner: turn.opposite(),
            }
        } else {
            GameResult::Stalemate
        }
    }
}

pub fn index_to_coord(idx: usize) -> (i8, i8) {
    ((idx % 8) as i8, (idx / 8) as i8)
}

pub fn coord_to_index(file: i8, rank: i8) -> Option<usize> {
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Some((rank * 8 + file) as usize)
    } else {
        None
    }
}
