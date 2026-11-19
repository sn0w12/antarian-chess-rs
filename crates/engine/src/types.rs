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

    /// Rank on which this colour's knights promote.
    pub fn promotion_rank(self) -> i8 {
        match self {
            Color::White => 7,
            Color::Black => 0,
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
    /// Pieces a knight may promote to on the last rank.
    pub const PROMOTION_CHOICES: [PieceKind; 4] = [
        PieceKind::Empress,
        PieceKind::Priest,
        PieceKind::Paladin,
        PieceKind::Dragon,
    ];

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

    pub fn from_index(index: usize) -> Option<PieceKind> {
        match index {
            0 => Some(PieceKind::Emperor),
            1 => Some(PieceKind::Empress),
            2 => Some(PieceKind::Priest),
            3 => Some(PieceKind::Paladin),
            4 => Some(PieceKind::Dragon),
            5 => Some(PieceKind::Knight),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PieceKind::Emperor => "emperor",
            PieceKind::Empress => "empress",
            PieceKind::Priest => "priest",
            PieceKind::Paladin => "paladin",
            PieceKind::Dragon => "dragon",
            PieceKind::Knight => "knight",
        }
    }

    pub fn from_name(name: &str) -> Option<PieceKind> {
        match name {
            "emperor" => Some(PieceKind::Emperor),
            "empress" => Some(PieceKind::Empress),
            "priest" => Some(PieceKind::Priest),
            "paladin" => Some(PieceKind::Paladin),
            "dragon" => Some(PieceKind::Dragon),
            "knight" => Some(PieceKind::Knight),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub capture: bool,
    pub promotion: Option<PieceKind>,
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
        promotion: None,
    };

    pub fn new(from: u8, to: u8, capture: bool) -> Self {
        Move {
            from,
            to,
            capture,
            promotion: None,
        }
    }

    pub fn new_promotion(from: u8, to: u8, capture: bool, promotion: PieceKind) -> Self {
        Move {
            from,
            to,
            capture,
            promotion: Some(promotion),
        }
    }
}
