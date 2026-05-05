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

    fn find_king(&self, color: Color) -> Option<usize> {
        (0..64).find(|&sq| self.squares[sq] == Some((color, PieceKind::Emperor)))
    }

    /// Returns `true` if `color`'s emperor is attacked by any opponent piece.
    pub fn is_in_check(&self, color: Color) -> bool {
        let king_sq = match self.find_king(color) {
            Some(sq) => sq,
            None => return false,
        };
        let opp = color.opposite();
        for sq in 0..64 {
            if let Some((pc, kind)) = self.squares[sq]
                && pc == opp
                && self.attacks(sq, kind, pc, king_sq)
            {
                return true;
            }
        }
        false
    }

    /// Returns `true` if an enemy slider has line-of-sight to the king through
    /// a friendly piece — i.e. a pin exists. When there are no pin threats,
    /// `generate_legal_moves` can skip the expensive `make_move`+`is_in_check`
    /// filter for most moves.
    fn has_pin_threat(&self, king_sq: usize, color: Color) -> bool {
        let (kf, kr) = index_to_coord(king_sq);
        const DIRS: [(i8, i8); 8] = [
            (-1, -1), (-1, 0), (-1, 1),
            (0, -1),           (0, 1),
            (1, -1),  (1, 0),  (1, 1),
        ];
        for &(df, dr) in &DIRS {
            let mut cf = kf + df;
            let mut cr = kr + dr;
            let mut found_friendly = false;
            while let Some(idx) = coord_to_index(cf, cr) {
                match self.squares[idx] {
                    None => {
                        cf += df;
                        cr += dr;
                        continue;
                    }
                    Some((pc, kind)) => {
                        if pc == color {
                            if found_friendly {
                                break;
                            }
                            found_friendly = true;
                        } else {
                            if found_friendly {
                                // king → friendly → enemy: could be a pin
                                let slides = match kind {
                                    PieceKind::Empress => true,
                                    PieceKind::Priest => df.abs() == dr.abs(),
                                    PieceKind::Dragon => df == 0 || dr == 0,
                                    _ => false,
                                };
                                if slides {
                                    return true;
                                }
                            }
                            break;
                        }
                    }
                }
                cf += df;
                cr += dr;
            }
        }
        false
    }

    /// Fast targeted attack check — does a piece of `kind` at `from` attack `to`?
    fn attacks(&self, from: usize, kind: PieceKind, color: Color, to: usize) -> bool {
        if from == to {
            return false;
        }
        match kind {
            PieceKind::Emperor => {
                let (ff, fr) = index_to_coord(from);
                let (tf, tr) = index_to_coord(to);
                (ff - tf).abs() <= 1 && (fr - tr).abs() <= 1
            }
            PieceKind::Empress => self.sliding_attack(from, to, |df, dr| df == 0 || dr == 0 || df.abs() == dr.abs()),
            PieceKind::Priest => {
                if self.sliding_attack(from, to, |df, dr| df.abs() == dr.abs()) {
                    return true;
                }
                let (ff, fr) = index_to_coord(from);
                let (tf, tr) = index_to_coord(to);
                let df = (ff - tf).abs();
                let dr = (fr - tr).abs();
                (df == 1 && dr == 2) || (df == 2 && dr == 1)
            }
            PieceKind::Paladin => {
                let (ff, fr) = index_to_coord(from);
                let (tf, tr) = index_to_coord(to);
                if (ff - tf).abs() <= 1 && (fr - tr).abs() <= 1 {
                    return true;
                }
                let df = tf - ff;
                let dr = tr - fr;
                if (df.abs() == 2 && dr == 0) || (df == 0 && dr.abs() == 2) {
                    let mf = ff + df.signum();
                    let mr = fr + dr.signum();
                    if let Some(idx) = coord_to_index(mf, mr) {
                        match self.squares[idx] {
                            None => true,
                            Some((pc, _)) => pc == color,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            PieceKind::Dragon => {
                let (ff, fr) = index_to_coord(from);
                let (tf, tr) = index_to_coord(to);
                let df = tf - ff;
                let dr = tr - fr;
                if df != 0 && dr != 0 {
                    return false;
                }
                if df == 0 && dr == 0 {
                    return false;
                }
                let sf = df.signum();
                let sr = dr.signum();
                let mut cf = ff + sf;
                let mut cr = fr + sr;
                while (cf, cr) != (tf, tr) {
                    if let Some(idx) = coord_to_index(cf, cr) {
                        match self.squares[idx] {
                            None => {}
                            Some((pc, _)) => {
                                if pc != color {
                                    return false;
                                }
                            }
                        }
                    }
                    cf += sf;
                    cr += sr;
                }
                true
            }
            PieceKind::Knight => {
                let (ff, fr) = index_to_coord(from);
                let (tf, tr) = index_to_coord(to);
                let fwd = if color == Color::White { 1 } else { -1 };
                tr - fr == fwd && (tf - ff).abs() == 1
            }
        }
    }

    fn sliding_attack(
        &self,
        from: usize,
        to: usize,
        direction_match: fn(i8, i8) -> bool,
    ) -> bool {
        let (ff, fr) = index_to_coord(from);
        let (tf, tr) = index_to_coord(to);
        let df = tf - ff;
        let dr = tr - fr;
        if !direction_match(df, dr) {
            return false;
        }
        if df == 0 && dr == 0 {
            return false;
        }
        let sf = df.signum();
        let sr = dr.signum();
        let mut cf = ff + sf;
        let mut cr = fr + sr;
        while (cf, cr) != (tf, tr) {
            if let Some(idx) = coord_to_index(cf, cr) {
                if self.squares[idx].is_some() {
                    return false; // blocked
                }
            }
            cf += sf;
            cr += sr;
        }
        true
    }

    /// Count pseudo-legal moves without allocating.
    pub fn count_all_moves(&self, color: Color) -> u32 {
        let mut n = 0u32;
        for sq in 0..64 {
            if let Some((pc, _)) = self.squares[sq]
                && pc == color
            {
                n += self.generate_moves_for(sq, false).len() as u32;
            }
        }
        n
    }

    /// Count pseudo-legal captures without allocating.
    pub fn count_captures(&self, color: Color) -> u32 {
        let mut n = 0u32;
        for sq in 0..64 {
            if let Some((pc, _)) = self.squares[sq]
                && pc == color
            {
                n += self.generate_moves_for(sq, true).len() as u32;
            }
        }
        n
    }

    /// Pseudolegal moves that do **not** leave the moving side's own emperor in check.
    ///
    /// Optimised with pin detection: when no opponent slider can discover a check,
    /// only the emperor's own moves need the full legality check.
    pub fn generate_legal_moves(&self, color: Color) -> Vec<Move> {
        let pseudo = self.generate_all_moves(color);
        let king_sq = match self.find_king(color) {
            Some(sq) => sq,
            None => return pseudo,
        };
        let in_check = self.is_in_check(color);
        let has_pins = in_check || self.has_pin_threat(king_sq, color);

        if !has_pins {
            // No pins: only the emperor's own moves need checking.
            pseudo
                .into_iter()
                .filter(|mv| {
                    if mv.from as usize == king_sq {
                        let child = self.make_move(mv);
                        !child.is_in_check(color)
                    } else {
                        true
                    }
                })
                .collect()
        } else {
            // In check or a pin exists: check every move.
            pseudo
                .into_iter()
                .filter(|mv| {
                    let child = self.make_move(mv);
                    !child.is_in_check(color)
                })
                .collect()
        }
    }

    /// Legal capture‑only moves (for quiescence search).
    pub fn generate_legal_captures(&self, color: Color) -> Vec<Move> {
        let pseudo = self.generate_captures(color);
        if self.find_king(color).is_none() {
            return pseudo;
        }
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
