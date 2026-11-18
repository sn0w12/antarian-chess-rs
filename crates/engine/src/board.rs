use crate::pieces;
use crate::rays;
use crate::types::{Color, GameResult, Move, PieceKind};
use std::sync::OnceLock;

pub const ZOBRIST_SIDE: u64 = 0x9D39247E_33776D41;

const EMPEROR_OFFSETS: [(i8, i8); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

const ALL_DIRS: [(i8, i8); 8] = EMPEROR_OFFSETS;

const DIAG_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

const KNIGHT_OFFSETS: [(i8, i8); 8] = [
    (-2, 1),
    (-1, 2),
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
];

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
    occupied: u64,
    pub turn: Color,
    pub zobrist: u64,
}

impl Board {
    pub fn empty() -> Self {
        Board {
            squares: [None; 64],
            occupied: 0,
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
        let bit = 1u64 << square;
        if piece.is_some() {
            self.occupied |= bit;
        } else {
            self.occupied &= !bit;
        }
        self.squares[square] = piece;
    }

    pub fn get(&self, square: usize) -> Option<(Color, PieceKind)> {
        self.squares[square]
    }

    /// Bitmask of occupied squares, bit index = square index.
    pub fn occupied(&self) -> u64 {
        self.occupied
    }

    fn for_each_piece(&self, color: Color, mut f: impl FnMut(usize, PieceKind)) {
        let mut occupied = self.occupied;
        while occupied != 0 {
            let square = occupied.trailing_zeros() as usize;
            occupied &= occupied - 1;
            if let Some((pc, kind)) = self.squares[square]
                && pc == color
            {
                f(square, kind);
            }
        }
    }

    /// Get a reference to the squares array.
    pub fn squares(&self) -> &[Option<(Color, PieceKind)>; 64] {
        &self.squares
    }

    /// Make a move, returning a new board. Zobrist is updated incrementally.
    pub fn make_move(&self, mv: &Move) -> Self {
        let mut new = self.clone();
        let from = mv.from as usize;
        let to = mv.to as usize;
        let piece = new.squares[from].take();
        new.occupied &= !(1u64 << from);

        // Remove piece from 'from' square in zobrist
        if let Some((color, kind)) = piece {
            new.zobrist ^= zobrist_piece_square(color, kind, from);
        }

        // Remove captured piece from zobrist
        if let Some((cap_color, cap_kind)) = new.squares[to] {
            new.zobrist ^= zobrist_piece_square(cap_color, cap_kind, to);
        }
        new.occupied &= !(1u64 << to);

        new.squares[to] = piece;
        if piece.is_some() {
            new.occupied |= 1u64 << to;
        }

        // Add piece to 'to' square in zobrist
        if let Some((color, kind)) = piece {
            new.zobrist ^= zobrist_piece_square(color, kind, to);
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
        let mut moves = Vec::new();
        self.generate_moves_for_into(square, captures_only, &mut moves);
        moves
    }

    fn generate_moves_for_into(&self, square: usize, captures_only: bool, moves: &mut Vec<Move>) {
        if let Some((color, kind)) = self.squares[square] {
            match kind {
                PieceKind::Emperor => {
                    pieces::generate_emperor_moves(self, square, color, captures_only, moves)
                }
                PieceKind::Empress => {
                    pieces::generate_empress_moves(self, square, color, captures_only, moves)
                }
                PieceKind::Priest => {
                    pieces::generate_priest_moves(self, square, color, captures_only, moves)
                }
                PieceKind::Paladin => {
                    pieces::generate_paladin_moves(self, square, color, captures_only, moves)
                }
                PieceKind::Dragon => {
                    pieces::generate_dragon_moves(self, square, color, captures_only, moves)
                }
                PieceKind::Knight => {
                    pieces::generate_knight_moves(self, square, color, captures_only, moves)
                }
            }
        }
    }

    /// Generate all legal moves for a color.
    pub fn generate_all_moves(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::with_capacity(128);
        self.for_each_piece(color, |sq, _| {
            self.generate_moves_for_into(sq, false, &mut moves);
        });
        moves
    }

    /// Generate only capture moves for a color. Faster than generating all+filtering.
    pub fn generate_captures(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::with_capacity(32);
        self.for_each_piece(color, |sq, _| {
            self.generate_moves_for_into(sq, true, &mut moves);
        });
        moves
    }

    // ---- Legal‑move filtering (king safety) ----

    fn find_king(&self, color: Color) -> Option<usize> {
        let mut occupied = self.occupied;
        while occupied != 0 {
            let square = occupied.trailing_zeros() as usize;
            occupied &= occupied - 1;
            if self.squares[square] == Some((color, PieceKind::Emperor)) {
                return Some(square);
            }
        }
        None
    }

    /// Returns `true` if `color`'s emperor is attacked by any opponent piece.
    ///
    /// Scans outward from the emperor instead of walking every enemy piece.
    pub fn is_in_check(&self, color: Color) -> bool {
        let Some(king_sq) = self.find_king(color) else {
            return false;
        };
        let opponent = color.opposite();
        let (kf, kr) = index_to_coord(king_sq);

        // Knights (pawns) capture one rank forward, diagonally.
        let knight_rank = kr + if color == Color::White { 1 } else { -1 };
        for df in [-1i8, 1] {
            if let Some(sq) = coord_to_index(kf + df, knight_rank)
                && self.squares[sq] == Some((opponent, PieceKind::Knight))
            {
                return true;
            }
        }

        // Priests jump like knights.
        for &(df, dr) in &KNIGHT_OFFSETS {
            if let Some(sq) = coord_to_index(kf + df, kr + dr)
                && self.squares[sq] == Some((opponent, PieceKind::Priest))
            {
                return true;
            }
        }

        // Along every ray: adjacent emperors/paladins, sliders, and the
        // paladin's two-square jump. A dragon jumps its own pieces, so
        // orthogonal rays continue past them; other sliders stop at a blocker.
        for (direction, &(df, dr)) in rays::DIRECTIONS.iter().enumerate() {
            let orthogonal = df == 0 || dr == 0;
            let diagonal = df.abs() == dr.abs();
            let mut distance = 0;
            let mut blocked = false;
            for &to in rays::ray(king_sq, direction) {
                distance += 1;
                if let Some((pc, kind)) = self.squares[to as usize] {
                    if pc == color {
                        break;
                    }
                    let attacks = match kind {
                        PieceKind::Emperor => distance == 1,
                        PieceKind::Paladin => distance == 1 || (orthogonal && distance == 2),
                        PieceKind::Empress => !blocked,
                        PieceKind::Priest => diagonal && !blocked,
                        PieceKind::Dragon => orthogonal,
                        PieceKind::Knight => false,
                    };
                    if attacks {
                        return true;
                    }
                    if !orthogonal {
                        break;
                    }
                    blocked = true;
                }
            }
        }

        false
    }

    /// Squares holding a friendly piece that cannot leave the line between the
    /// emperor and an enemy slider, plus pieces blocking a Paladin's two-square
    /// jump. A move from one of these squares is legal only when the
    /// destination stays on that line, which [`same_ray`] decides without
    /// touching the board.
    fn pinned_mask(&self, king_sq: usize, color: Color) -> u64 {
        let (kf, kr) = index_to_coord(king_sq);
        let mut pinned = 0u64;

        for (direction, &(df, dr)) in rays::DIRECTIONS.iter().enumerate() {
            let orthogonal = df == 0 || dr == 0;
            let diagonal = df.abs() == dr.abs();
            let mut blocker = None;
            let mut seeking_dragon = false;

            for &idx in rays::ray(king_sq, direction) {
                let sq = idx as usize;
                let Some((pc, kind)) = self.squares[sq] else {
                    continue;
                };

                if pc == color {
                    if blocker.is_some() {
                        break;
                    }
                    blocker = Some(sq);
                    continue;
                }

                let Some(pinned_sq) = blocker else {
                    break;
                };

                if seeking_dragon {
                    // Every closer enemy piece blocks all sliders except the
                    // Dragon, which jumps its own pieces.
                    if kind == PieceKind::Dragon {
                        pinned |= 1 << pinned_sq;
                    }
                    break;
                }

                let attacks = match kind {
                    PieceKind::Empress => true,
                    PieceKind::Priest => diagonal,
                    PieceKind::Dragon => orthogonal,
                    _ => false,
                };
                if attacks {
                    pinned |= 1 << pinned_sq;
                    break;
                }
                if !orthogonal {
                    break;
                }
                seeking_dragon = true;
            }
        }

        // The Paladin jump is blocked by the square next to the emperor, so a
        // friendly piece there may only move onto the Paladin itself.
        for &(df, dr) in &[(0i8, 1i8), (0, -1), (1, 0), (-1, 0)] {
            let Some(blocker) = coord_to_index(kf + df, kr + dr) else {
                continue;
            };
            if !matches!(self.squares[blocker], Some((pc, _)) if pc == color) {
                continue;
            }
            let Some(jumper) = coord_to_index(kf + 2 * df, kr + 2 * dr) else {
                continue;
            };
            if self.squares[jumper] == Some((color.opposite(), PieceKind::Paladin)) {
                pinned |= 1 << blocker;
            }
        }

        pinned
    }

    fn move_is_legal(&self, mv: &Move, king_sq: usize, color: Color, pinned: u64) -> bool {
        if mv.from as usize == king_sq {
            !self.make_move(mv).is_in_check(color)
        } else if pinned & (1u64 << mv.from) != 0 {
            same_ray(king_sq, mv.from as usize, mv.to as usize)
        } else {
            true
        }
    }

    /// Fast targeted attack check — does a piece of `kind` at `from` attack `to`?
    #[cfg(test)]
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
            PieceKind::Empress => self.sliding_attack(from, to, |df, dr| {
                df == 0 || dr == 0 || df.abs() == dr.abs()
            }),
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

    #[cfg(test)]
    fn sliding_attack(&self, from: usize, to: usize, direction_match: fn(i8, i8) -> bool) -> bool {
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
        self.count_moves(color, false)
    }

    /// Count pseudo-legal captures without allocating.
    pub fn count_captures(&self, color: Color) -> u32 {
        self.count_moves(color, true)
    }

    fn count_moves(&self, color: Color, captures_only: bool) -> u32 {
        let mut n = 0u32;
        self.for_each_piece(color, |sq, kind| {
            n += self.count_piece_moves(sq, kind, color, captures_only);
        });
        n
    }

    fn count_piece_moves(
        &self,
        from: usize,
        kind: PieceKind,
        color: Color,
        captures_only: bool,
    ) -> u32 {
        match kind {
            PieceKind::Emperor => self.count_leaper(from, color, &EMPEROR_OFFSETS, captures_only),
            PieceKind::Empress => self.count_slider(from, color, &ALL_DIRS, captures_only),
            PieceKind::Priest => {
                self.count_slider(from, color, &DIAG_DIRS, captures_only)
                    + self.count_leaper(from, color, &KNIGHT_OFFSETS, captures_only)
            }
            PieceKind::Paladin => {
                self.count_leaper(from, color, &EMPEROR_OFFSETS, captures_only)
                    + self.count_paladin_jumps(from, color, captures_only)
            }
            PieceKind::Dragon => self.count_dragon(from, color, captures_only),
            PieceKind::Knight => self.count_knight(from, color, captures_only),
        }
    }

    fn count_leaper(
        &self,
        from: usize,
        color: Color,
        offsets: &[(i8, i8)],
        captures_only: bool,
    ) -> u32 {
        let (f, r) = index_to_coord(from);
        let mut n = 0;
        for &(df, dr) in offsets {
            let Some(to) = coord_to_index(f + df, r + dr) else {
                continue;
            };
            match self.squares[to] {
                None => {
                    if !captures_only {
                        n += 1;
                    }
                }
                Some((pc, _)) if pc != color => n += 1,
                _ => {}
            }
        }
        n
    }

    fn count_slider(
        &self,
        from: usize,
        color: Color,
        directions: &[(i8, i8)],
        captures_only: bool,
    ) -> u32 {
        let (f, r) = index_to_coord(from);
        let mut n = 0;
        for &(df, dr) in directions {
            let mut cf = f + df;
            let mut cr = r + dr;
            while let Some(to) = coord_to_index(cf, cr) {
                match self.squares[to] {
                    None => {
                        if !captures_only {
                            n += 1;
                        }
                    }
                    Some((pc, _)) => {
                        if pc != color {
                            n += 1;
                        }
                        break;
                    }
                }
                cf += df;
                cr += dr;
            }
        }
        n
    }

    fn count_paladin_jumps(&self, from: usize, color: Color, captures_only: bool) -> u32 {
        let (f, r) = index_to_coord(from);
        let mut n = 0;
        for &(df, dr) in &[(0i8, 2i8), (0, -2), (2, 0), (-2, 0)] {
            let Some(to) = coord_to_index(f + df, r + dr) else {
                continue;
            };
            match self.squares[to] {
                Some((pc, _)) if pc == color => continue,
                None if captures_only => continue,
                _ => {}
            }
            let intermediate = coord_to_index(f + df.signum(), r + dr.signum());
            if matches!(intermediate, Some(idx) if self.squares[idx].is_none_or(|(pc, _)| pc == color))
            {
                n += 1;
            }
        }
        n
    }

    fn count_dragon(&self, from: usize, color: Color, captures_only: bool) -> u32 {
        let (f, r) = index_to_coord(from);
        let mut n = 0;
        for &(df, dr) in &[(0i8, 1i8), (0, -1), (1, 0), (-1, 0)] {
            let mut cf = f + df;
            let mut cr = r + dr;
            while let Some(to) = coord_to_index(cf, cr) {
                match self.squares[to] {
                    None => {
                        if !captures_only {
                            n += 1;
                        }
                    }
                    Some((pc, _)) => {
                        if pc != color {
                            n += 1;
                            break;
                        }
                        // Friendly piece: jump over it and keep sliding.
                    }
                }
                cf += df;
                cr += dr;
            }
        }
        n
    }

    fn count_knight(&self, from: usize, color: Color, captures_only: bool) -> u32 {
        let (f, r) = index_to_coord(from);
        let forward = if color == Color::White { 1 } else { -1 };
        let start_rank = if color == Color::White { 1 } else { 6 };
        let mut n = 0;

        if !captures_only
            && let Some(one) = coord_to_index(f, r + forward)
            && self.squares[one].is_none()
        {
            n += 1;
            if r == start_rank
                && let Some(two) = coord_to_index(f, r + 2 * forward)
                && self.squares[two].is_none()
            {
                n += 1;
            }
        }

        for df in [-1, 1] {
            if let Some(to) = coord_to_index(f + df, r + forward)
                && let Some((pc, _)) = self.squares[to]
                && pc != color
            {
                n += 1;
            }
        }

        n
    }

    /// Pseudolegal moves that do **not** leave the moving side's own emperor in check.
    ///
    /// Optimised with pin detection: when no opponent slider can discover a check,
    /// only the emperor's own moves need the full legality check.
    pub fn generate_legal_moves(&self, color: Color) -> Vec<Move> {
        let in_check = self.is_in_check(color);
        self.generate_legal_moves_with_check(color, in_check)
    }

    /// Same as [`Self::generate_legal_moves`], for callers that already computed
    /// [`Self::is_in_check`] for `color`.
    pub fn generate_legal_moves_with_check(&self, color: Color, in_check: bool) -> Vec<Move> {
        let mut pseudo = self.generate_all_moves(color);
        let king_sq = match self.find_king(color) {
            Some(sq) => sq,
            None => return pseudo,
        };

        if in_check {
            pseudo.retain(|mv| !self.make_move(mv).is_in_check(color));
            return pseudo;
        }

        let pinned = self.pinned_mask(king_sq, color);
        pseudo.retain(|mv| self.move_is_legal(mv, king_sq, color, pinned));
        pseudo
    }

    /// Bitmask of `color` pieces that can capture on `square`.
    pub fn attackers_to(&self, square: usize, color: Color) -> u64 {
        let (f, r) = index_to_coord(square);
        let mut attackers = 0u64;

        for &(df, dr) in &EMPEROR_OFFSETS {
            if let Some(idx) = coord_to_index(f + df, r + dr)
                && let Some((pc, kind)) = self.squares[idx]
                && pc == color
                && matches!(kind, PieceKind::Emperor | PieceKind::Paladin)
            {
                attackers |= 1u64 << idx;
            }
        }

        for &(df, dr) in &KNIGHT_OFFSETS {
            if let Some(idx) = coord_to_index(f + df, r + dr)
                && self.squares[idx] == Some((color, PieceKind::Priest))
            {
                attackers |= 1u64 << idx;
            }
        }

        for &(df, dr) in &[(0i8, 2i8), (0, -2), (2, 0), (-2, 0)] {
            let Some(idx) = coord_to_index(f + df, r + dr) else {
                continue;
            };
            if self.squares[idx] != Some((color, PieceKind::Paladin)) {
                continue;
            }
            let intermediate = coord_to_index(f + df / 2, r + dr / 2);
            if matches!(intermediate, Some(i) if self.squares[i].is_none_or(|(pc, _)| pc == color))
            {
                attackers |= 1u64 << idx;
            }
        }

        // Knights capture one rank forward along the diagonals.
        let forward = if color == Color::White { 1 } else { -1 };
        for df in [-1i8, 1] {
            if let Some(idx) = coord_to_index(f + df, r - forward)
                && self.squares[idx] == Some((color, PieceKind::Knight))
            {
                attackers |= 1u64 << idx;
            }
        }

        for (direction, &(df, dr)) in rays::DIRECTIONS.iter().enumerate() {
            let orthogonal = df == 0 || dr == 0;
            let diagonal = df.abs() == dr.abs();
            let mut passed_friendly = false;

            for &to in rays::ray(square, direction) {
                let idx = to as usize;
                let Some((pc, kind)) = self.squares[idx] else {
                    continue;
                };
                if pc != color {
                    break;
                }

                if passed_friendly {
                    // Only a Dragon reaches past a friendly blocker.
                    if kind == PieceKind::Dragon {
                        attackers |= 1u64 << idx;
                    }
                    break;
                }

                let slides = match kind {
                    PieceKind::Empress => true,
                    PieceKind::Priest => diagonal,
                    PieceKind::Dragon => orthogonal,
                    _ => false,
                };
                if slides {
                    attackers |= 1u64 << idx;
                }
                if !orthogonal {
                    break;
                }
                passed_friendly = true;
            }
        }

        attackers
    }

    /// Legal capture‑only moves (for quiescence search).
    pub fn generate_legal_captures(&self, color: Color) -> Vec<Move> {
        self.generate_legal_captures_with_check(color, self.is_in_check(color))
    }

    /// Same as [`Self::generate_legal_captures`], for callers that already
    /// computed [`Self::is_in_check`] for `color`.
    pub fn generate_legal_captures_with_check(&self, color: Color, in_check: bool) -> Vec<Move> {
        let mut pseudo = self.generate_captures(color);
        let Some(king_sq) = self.find_king(color) else {
            return pseudo;
        };

        if in_check {
            pseudo.retain(|mv| !self.make_move(mv).is_in_check(color));
            return pseudo;
        }

        let pinned = self.pinned_mask(king_sq, color);
        if pinned != 0 || pseudo.iter().any(|mv| mv.from as usize == king_sq) {
            pseudo.retain(|mv| self.move_is_legal(mv, king_sq, color, pinned));
        }
        pseudo
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

fn same_ray(a: usize, b: usize, c: usize) -> bool {
    let (af, ar) = index_to_coord(a);
    let (bf, br) = index_to_coord(b);
    let (cf, cr) = index_to_coord(c);
    let ab = (i32::from(bf - af), i32::from(br - ar));
    let ac = (i32::from(cf - af), i32::from(cr - ar));
    ab.0 * ac.1 == ab.1 * ac.0 && ab.0 * ac.0 + ab.1 * ac.1 > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn reference_in_check(board: &Board, color: Color) -> bool {
        let Some(king) = (0..64).find(|&sq| board.get(sq) == Some((color, PieceKind::Emperor)))
        else {
            return false;
        };
        for sq in 0..64 {
            if let Some((pc, kind)) = board.get(sq)
                && pc != color
                && board.attacks(sq, kind, pc, king)
            {
                return true;
            }
        }
        false
    }

    fn random_kind(rng: &mut Rng) -> PieceKind {
        match rng.next() % 5 {
            0 => PieceKind::Empress,
            1 => PieceKind::Priest,
            2 => PieceKind::Paladin,
            3 => PieceKind::Dragon,
            _ => PieceKind::Knight,
        }
    }

    fn place_random(rng: &mut Rng, board: &mut Board, color: Color, kind: PieceKind) {
        for _ in 0..64 {
            let square = (rng.next() % 64) as usize;
            if board.get(square).is_none() {
                board.set(square, Some((color, kind)));
                return;
            }
        }
    }

    fn assert_occupied_matches_squares(board: &Board) {
        let mut expected = 0u64;
        for sq in 0..64 {
            if board.get(sq).is_some() {
                expected |= 1u64 << sq;
            }
        }
        assert_eq!(board.occupied(), expected);
    }

    fn describe(board: &Board) -> String {
        let mut out = String::new();
        for sq in 0..64 {
            if let Some((c, k)) = board.get(sq) {
                out.push_str(&format!("{:?}{:?}@{},{} ", c, k, sq % 8, sq / 8));
            }
        }
        out
    }

    fn assert_generation_matches_full_verification(board: &Board) {
        let color = board.turn;
        let fast = board.generate_legal_moves(color);
        let slow: Vec<_> = board
            .generate_all_moves(color)
            .into_iter()
            .filter(|mv| !board.make_move(mv).is_in_check(color))
            .collect();
        for mv in &fast {
            assert!(
                slow.contains(mv),
                "move {mv:?} illegally allowed: turn {color:?} {}",
                describe(board)
            );
        }
        for mv in &slow {
            assert!(
                fast.contains(mv),
                "legal move {mv:?} missing: turn {color:?} {}",
                describe(board)
            );
        }

        let fast = board.generate_legal_captures(color);
        let slow: Vec<_> = board
            .generate_captures(color)
            .into_iter()
            .filter(|mv| !board.make_move(mv).is_in_check(color))
            .collect();
        for mv in &fast {
            assert!(
                slow.contains(mv),
                "capture {mv:?} illegally allowed: {}",
                describe(board)
            );
        }
        for mv in &slow {
            assert!(
                fast.contains(mv),
                "legal capture {mv:?} missing: {}",
                describe(board)
            );
        }
    }

    fn board_with(pieces: &[(usize, Color, PieceKind)], turn: Color) -> Board {
        let mut board = Board::empty();
        for &(sq, color, kind) in pieces {
            board.set(sq, Some((color, kind)));
        }
        board.turn = turn;
        board
    }

    #[test]
    fn pinned_pieces_are_restricted_to_the_pin_ray() {
        let empress_pin = board_with(
            &[
                (4, Color::White, PieceKind::Emperor),  // e1
                (12, Color::White, PieceKind::Knight),  // e2
                (60, Color::Black, PieceKind::Empress), // e8
                (56, Color::Black, PieceKind::Emperor), // a8
            ],
            Color::White,
        );
        let moves = empress_pin.generate_legal_moves(Color::White);
        assert!(moves.contains(&Move::new(12, 20, false)), "e2-e3");
        assert!(moves.contains(&Move::new(12, 28, false)), "e2-e4");
        assert!(!moves.contains(&Move::new(12, 17, false)), "e2-c3");
        assert_generation_matches_full_verification(&empress_pin);

        let dragon_pin = board_with(
            &[
                (49, Color::Black, PieceKind::Emperor), // b7
                (33, Color::Black, PieceKind::Dragon),  // b5
                (25, Color::White, PieceKind::Dragon),  // b4
                (0, Color::White, PieceKind::Emperor),  // a1
            ],
            Color::Black,
        );
        let moves = dragon_pin.generate_legal_moves(Color::Black);
        assert!(moves.contains(&Move::new(33, 41, false)), "b5-b6");
        assert!(
            !moves.contains(&Move::new(33, 57, false)),
            "b5-b8 must not jump its own emperor on the pin ray"
        );
        assert_generation_matches_full_verification(&dragon_pin);

        let paladin_blocker = board_with(
            &[
                (4, Color::White, PieceKind::Emperor),  // e1
                (12, Color::White, PieceKind::Knight),  // e2
                (20, Color::Black, PieceKind::Paladin), // e3
                (56, Color::Black, PieceKind::Emperor), // a8
            ],
            Color::White,
        );
        assert_generation_matches_full_verification(&paladin_blocker);

        let priest_diagonal = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),  // a1
                (9, Color::White, PieceKind::Knight),   // b2
                (18, Color::Black, PieceKind::Priest),  // c3
                (63, Color::Black, PieceKind::Emperor), // h8
            ],
            Color::White,
        );
        assert!(
            !priest_diagonal
                .generate_legal_moves(Color::White)
                .iter()
                .any(|mv| mv.from == 9 && mv.to != 18)
        );
        assert_generation_matches_full_verification(&priest_diagonal);

        let priest_orthogonal = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),  // a1
                (8, Color::White, PieceKind::Knight),   // a2
                (24, Color::Black, PieceKind::Priest),  // a4
                (63, Color::Black, PieceKind::Emperor), // h8
            ],
            Color::White,
        );
        assert_generation_matches_full_verification(&priest_orthogonal);

        let double_blocker = board_with(
            &[
                (0, Color::White, PieceKind::Emperor),  // a1
                (8, Color::White, PieceKind::Knight),   // a2
                (16, Color::White, PieceKind::Knight),  // a3
                (56, Color::Black, PieceKind::Empress), // a8
                (63, Color::Black, PieceKind::Emperor), // h8
            ],
            Color::White,
        );
        let moves = double_blocker.generate_legal_moves(Color::White);
        assert!(
            !moves.iter().any(|mv| mv.from == 8),
            "a2 knight is boxed in by its own knight"
        );
        assert!(moves.contains(&Move::new(16, 24, false)), "a3-a4");
        assert_generation_matches_full_verification(&double_blocker);
    }

    #[test]
    fn legal_move_generation_matches_full_verification() {
        let mut rng = Rng(0x5EED_1234_9876_ABCD);
        let mut pinned_positions = 0;

        for _ in 0..500 {
            let mut board = Board::empty();
            place_random(&mut rng, &mut board, Color::White, PieceKind::Emperor);
            place_random(&mut rng, &mut board, Color::Black, PieceKind::Emperor);
            for _ in 0..(rng.next() % 12) {
                let color = if rng.next() & 1 == 0 {
                    Color::White
                } else {
                    Color::Black
                };
                let kind = random_kind(&mut rng);
                place_random(&mut rng, &mut board, color, kind);
            }
            board.turn = if rng.next() & 1 == 0 {
                Color::White
            } else {
                Color::Black
            };
            if !board.is_in_check(board.turn)
                && let Some(king) = board.find_king(board.turn)
                && board.pinned_mask(king, board.turn) != 0
            {
                pinned_positions += 1;
            }
            assert_generation_matches_full_verification(&board);
        }
        assert!(pinned_positions > 0, "no pin cases generated");

        let mut board = Board::initial();
        for _ in 0..300 {
            assert_generation_matches_full_verification(&board);
            let moves = board.generate_legal_moves(board.turn);
            if moves.is_empty() {
                break;
            }
            board = board.make_move(&moves[(rng.next() % moves.len() as u64) as usize]);
        }
    }

    #[test]
    fn is_in_check_matches_full_piece_scan() {
        let mut rng = Rng(0x1234_5678_9ABC_DEF0);

        for _ in 0..400 {
            let mut board = Board::empty();
            place_random(&mut rng, &mut board, Color::White, PieceKind::Emperor);
            place_random(&mut rng, &mut board, Color::Black, PieceKind::Emperor);
            for _ in 0..(rng.next() % 8) {
                let color = if rng.next() & 1 == 0 {
                    Color::White
                } else {
                    Color::Black
                };
                let kind = random_kind(&mut rng);
                place_random(&mut rng, &mut board, color, kind);
            }
            assert_occupied_matches_squares(&board);
            for color in [Color::White, Color::Black] {
                assert_eq!(
                    board.is_in_check(color),
                    reference_in_check(&board, color),
                    "{color:?} on {:?}",
                    board.squares()
                );
            }
        }

        let mut board = Board::initial();
        for ply in 0..300 {
            assert_occupied_matches_squares(&board);
            for color in [Color::White, Color::Black] {
                assert_eq!(
                    board.is_in_check(color),
                    reference_in_check(&board, color),
                    "{color:?} at ply {ply}"
                );
            }
            let moves = board.generate_legal_moves(board.turn);
            if moves.is_empty() {
                break;
            }
            let mv = moves[(rng.next() % moves.len() as u64) as usize];
            board = board.make_move(&mv);
        }
    }
}
