#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub fn multiplier(self) -> i32 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Emperor,
    Empress,
    Priest,
    Paladin,
    Dragon,
    Knight,
}

impl PieceKind {
    pub fn value(self) -> i32 {
        match self {
            PieceKind::Emperor => 10_000,
            PieceKind::Empress => 900,
            PieceKind::Priest => 650,
            PieceKind::Paladin => 600,
            PieceKind::Dragon => 500,
            PieceKind::Knight => 100,
        }
    }

    pub fn index(self) -> usize {
        match self {
            PieceKind::Emperor => 0,
            PieceKind::Empress => 1,
            PieceKind::Priest => 2,
            PieceKind::Paladin => 3,
            PieceKind::Dragon => 4,
            PieceKind::Knight => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub capture: bool,
}

/// The result of a game at any given board state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    /// The game is still in progress.
    Ongoing,
    /// The side to move has no legal moves and is in check.
    Checkmate { winner: Color },
    /// The side to move has no legal moves and is not in check.
    Stalemate,
}

impl Move {
    pub const NULL: Move = Move {
        from: 0,
        to: 0,
        capture: false,
    };

    pub fn new(from: u8, to: u8, capture: bool) -> Self {
        Move { from, to, capture }
    }
}
