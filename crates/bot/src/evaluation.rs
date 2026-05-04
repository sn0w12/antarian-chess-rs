//! Phased (tapered) evaluation for Antarian Chess.
//!
//! Scores are always from White's perspective (positive = White better).

use chess_engine::{Board, Color, PieceKind};

// ---------------------------------------------------------------------------
// Material — opening      midgame (MG)   /   endgame (EG)
// ---------------------------------------------------------------------------

const MG: [i32; 6] = [
    10_000, // Emperor
    975,    // Empress
    680,    // Priest
    600,    // Paladin
    525,    // Dragon
    100,    // Knight
];

const EG: [i32; 6] = [
    10_000, // Emperor
    975,    // Empress
    630,    // Priest  (slightly less dominant without many diagonals)
    560,    // Paladin
    490,    // Dragon
    120,    // Knight
];

// ---------------------------------------------------------------------------
// Mid-game PSQT  (see the player's perspective, indexed by PieceKind)
// ---------------------------------------------------------------------------

const PST_MG: [[i32; 64]; 6] = [
    /* Emperor — safe and central */
    [
        -30,-25,-20,-15,-15,-20,-25,-30,
        -25,-10, -5,  0,  0, -5,-10,-25,
        -20, -5,  5, 10, 10,  5, -5,-20,
        -15,  0, 10, 20, 20, 10,  0,-15,
        -15,  0, 10, 20, 20, 10,  0,-15,
        -20, -5,  5, 10, 10,  5, -5,-20,
        -25,-10, -5,  0,  0, -5,-10,-25,
        -30,-25,-20,-15,-15,-20,-25,-30,
    ],
    /* Empress — centralised */
    [
        -20,-10,-10, -5, -5,-10,-10,-20,
        -10,  0,  5,  5,  5,  5,  0,-10,
        -10,  5, 10, 15, 15, 10,  5,-10,
         -5,  5, 15, 20, 20, 15,  5, -5,
         -5,  5, 15, 20, 20, 15,  5, -5,
        -10,  5, 10, 15, 15, 10,  5,-10,
        -10,  0,  5,  5,  5,  5,  0,-10,
        -20,-10,-10, -5, -5,-10,-10,-20,
    ],
    /* Priest — diagonals */
    [
        -20,-10,-10,-10,-10,-10,-10,-20,
        -10,  0,  5,  5,  5,  5,  0,-10,
        -10,  5, 15, 20, 20, 15,  5,-10,
        -10,  5, 20, 25, 25, 20,  5,-10,
        -10,  5, 20, 25, 25, 20,  5,-10,
        -10,  5, 15, 20, 20, 15,  5,-10,
        -10,  0,  5,  5,  5,  5,  0,-10,
        -20,-10,-10,-10,-10,-10,-10,-20,
    ],
    /* Paladin — centre */
    [
        -15,-10, -5,  0,  0, -5,-10,-15,
        -10,  0,  5, 10, 10,  5,  0,-10,
         -5,  5, 10, 15, 15, 10,  5, -5,
          0, 10, 15, 20, 20, 15, 10,  0,
          0, 10, 15, 20, 20, 15, 10,  0,
         -5,  5, 10, 15, 15, 10,  5, -5,
        -10,  0,  5, 10, 10,  5,  0,-10,
        -15,-10, -5,  0,  0, -5,-10,-15,
    ],
    /* Dragon — openness, 7th rank */
    [
          0,  0,  0,  5,  5,  0,  0,  0,
          5, 10, 10, 10, 10, 10, 10,  5,
         -5,  0,  0,  5,  5,  0,  0, -5,
         -5,  0,  0,  5,  5,  0,  0, -5,
         -5,  0,  0,  5,  5,  0,  0, -5,
         -5,  0,  0,  5,  5,  0,  0, -5,
          5, 10, 10, 10, 10, 10, 10,  5,
          0,  0,  0,  5,  5,  0,  0,  0,
    ],
    /* Knight — advance and central files */
    [
          0,  0,  0,  0,  0,  0,  0,  0,
         50, 50, 50, 55, 55, 50, 50, 50,
         10, 10, 20, 35, 35, 20, 10, 10,
          5,  5, 15, 30, 30, 15,  5,  5,
          0,  0, 10, 25, 25, 10,  0,  0,
          5, -5,-10,  0,  0,-10, -5,  5,
          5, 10, 10,-20,-20, 10, 10,  5,
          0,  0,  0,  0,  0,  0,  0,  0,
    ],
];

// ---------------------------------------------------------------------------
// End-game PSQT  (emperor activates, knights stay active)
// ---------------------------------------------------------------------------

const PST_EG: [[i32; 64]; 6] = [
    /* Emperor — run to the centre */
    [
        -50,-40,-30,-20,-20,-30,-40,-50,
        -30,-20,-10,  0,  0,-10,-20,-30,
        -20,-10, 20, 30, 30, 20,-10,-20,
        -10,  0, 30, 40, 40, 30,  0,-10,
        -10,  0, 30, 40, 40, 30,  0,-10,
        -20,-10, 20, 30, 30, 20,-10,-20,
        -30,-20,-10,  0,  0,-10,-20,-30,
        -50,-40,-30,-20,-20,-30,-40,-50,
    ],
    /* Empress — active anywhere */
    [
        -10, -5, -5,  0,  0, -5, -5,-10,
         -5,  0,  5,  5,  5,  5,  0, -5,
         -5,  5, 10, 10, 10, 10,  5, -5,
          0,  5, 10, 15, 15, 10,  5,  0,
          0,  5, 10, 15, 15, 10,  5,  0,
         -5,  5, 10, 10, 10, 10,  5, -5,
         -5,  0,  5,  5,  5,  5,  0, -5,
        -10, -5, -5,  0,  0, -5, -5,-10,
    ],
    /* Priest — centralised */
    [
        -10, -5, -5,  0,  0, -5, -5,-10,
         -5,  0,  5,  5,  5,  5,  0, -5,
         -5,  5, 10, 15, 15, 10,  5, -5,
          0,  5, 15, 20, 20, 15,  5,  0,
          0,  5, 15, 20, 20, 15,  5,  0,
         -5,  5, 10, 15, 15, 10,  5, -5,
         -5,  0,  5,  5,  5,  5,  0, -5,
        -10, -5, -5,  0,  0, -5, -5,-10,
    ],
    /* Paladin — active */
    [
         -5,  0,  0,  5,  5,  0,  0, -5,
          0,  5,  5, 10, 10,  5,  5,  0,
          0,  5, 10, 15, 15, 10,  5,  0,
          5, 10, 15, 20, 20, 15, 10,  5,
          5, 10, 15, 20, 20, 15, 10,  5,
          0,  5, 10, 15, 15, 10,  5,  0,
          0,  5,  5, 10, 10,  5,  5,  0,
         -5,  0,  0,  5,  5,  0,  0, -5,
    ],
    /* Dragon — active */
    [
          0,  5,  5,  5,  5,  5,  5,  0,
          5, 10, 10, 10, 10, 10, 10,  5,
          5, 10, 10, 10, 10, 10, 10,  5,
          5, 10, 10, 10, 10, 10, 10,  5,
          5, 10, 10, 10, 10, 10, 10,  5,
          5, 10, 10, 10, 10, 10, 10,  5,
          5, 10, 10, 10, 10, 10, 10,  5,
          0,  5,  5,  5,  5,  5,  5,  0,
    ],
    /* Knight */
    [
          0,  0,  0,  0,  0,  0,  0,  0,
         45, 45, 45, 45, 45, 45, 45, 45,
         25, 25, 25, 25, 25, 25, 25, 25,
          5,  5,  5,  5,  5,  5,  5,  5,
         -5, -5, -5, -5, -5, -5, -5, -5,
         -5, -5, -5, -5, -5, -5, -5, -5,
         -5, -5, -5, -5, -5, -5, -5, -5,
          0,  0,  0,  0,  0,  0,  0,  0,
    ],
];

// ---------------------------------------------------------------------------
// Phase calculation — fraction 0 = midgame, 1 = endgame
// ---------------------------------------------------------------------------

/// Total material on the board (excluding Emperors).  We use this to
/// interpolate opening → endgame evaluation.
fn game_phase(board: &Board) -> f64 {
    let mut total = 0i32;
    for sq in 0..64 {
        if let Some((_, kind)) = board.get(sq)
            && kind != PieceKind::Emperor {
                total += match kind {
                    PieceKind::Empress => 2,
                    PieceKind::Priest => 2,
                    PieceKind::Paladin => 2,
                    PieceKind::Dragon => 1,
                    PieceKind::Knight => 0,
                    _ => 0,
                };
            }
    }
    // Max = 2·2 ·2(Empress) ·2(Priest) ·2(Paladin) ·1·2(Dragon) = 14
    // Phase goes from 0 (opening) to 1 (endgame)
    (1.0 - (total as f64 / 14.0)).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// King-safety penalty
// ---------------------------------------------------------------------------

fn king_safety(board: &Board, color: Color) -> i32 {
    let emperor_sq = match (0..64).find(|&sq| board.get(sq) == Some((color, PieceKind::Emperor))) {
        Some(sq) => sq as i32,
        None => return 0,
    };

    let file = emperor_sq % 8;
    let rank = emperor_sq / 8;

    // Front shield: are there friendly knights on the three squares in front?
    // We check the two ranks ahead (one or two ranks, depending on side).
    let forward = if color == Color::White { 1 } else { -1 };
    let mut shield = 0;

    // One rank ahead: the three squares directly in front.
    for df in -1..=1 {
        let f = file + df;
        let r = rank + forward;
        if (0..8).contains(&f) && (0..8).contains(&r) {
            let idx = (r * 8 + f) as usize;
            if board.get(idx) == Some((color, PieceKind::Knight)) {
                shield += 30;
            }
        }
    }

    // Two ranks ahead (if on starting back rank).
    let home_rank = if color == Color::White { 0 } else { 7 };
    if rank == home_rank {
        for df in -1..=1 {
            let f = file + df;
            let r = rank + 2 * forward;
            if (0..8).contains(&f) && (0..8).contains(&r) {
                let idx = (r * 8 + f) as usize;
                if board.get(idx) == Some((color, PieceKind::Knight)) {
                    shield += 20;
                }
            }
        }
    }

    // Penalty: missing shield = danger.  Max penalty ≈ 120
    (120 - shield).max(0)
}

// ---------------------------------------------------------------------------
// Mobility
// ---------------------------------------------------------------------------

/// Small bonus for having more legal moves.
fn mobility(board: &Board, color: Color) -> i32 {
    let n = board.generate_legal_moves(color).len() as i32;
    // 3 centipawns per move, capped at ~100
    (n * 3).min(90)
}

// ---------------------------------------------------------------------------
// Main tapered evaluation
// ---------------------------------------------------------------------------

pub fn evaluate(board: &Board) -> i32 {
    let phase = game_phase(board);
    let mut score_mg = 0i32;
    let mut score_eg = 0i32;

    // ----- material + PSQT -----
    for sq in 0..64 {
        if let Some((color, kind)) = board.get(sq) {
            let idx = kind_index(kind);
            let mg_mat = MG[idx];
            let eg_mat = EG[idx];
            let mg_pst = if color == Color::White {
                PST_MG[idx][sq]
            } else {
                PST_MG[idx][sq ^ 56]
            };
            let eg_pst = if color == Color::White {
                PST_EG[idx][sq]
            } else {
                PST_EG[idx][sq ^ 56]
            };

            let mg = mg_mat + mg_pst;
            let eg = eg_mat + eg_pst;

            if color == Color::White {
                score_mg += mg;
                score_eg += eg;
            } else {
                score_mg -= mg;
                score_eg -= eg;
            }
        }
    }

    // ----- knight structure (midgame quality) -----
    score_mg += knight_structure(board, Color::White);
    score_mg -= knight_structure(board, Color::Black);

    // ----- development (midgame only — penalise pieces still on the back rank) -----
    score_mg += development(board, Color::White);
    score_mg -= development(board, Color::Black);

    // ----- centre control (midgame only) -----
    score_mg += centre_control(board, Color::White);
    score_mg -= centre_control(board, Color::Black);

    // ----- king safety (midgame only) -----
    let ks_w = king_safety(board, Color::White);
    let ks_b = king_safety(board, Color::Black);
    score_mg -= ks_w * 2; // penalty = opponent's shield is bad for us
    score_mg += ks_b * 2;
    // king safety fades in endgame (interpolation handles this)

    // ----- mobility (both phases) -----
    let mob_w = mobility(board, Color::White);
    let mob_b = mobility(board, Color::Black);
    score_mg += mob_w - mob_b;
    score_eg  += (mob_w - mob_b) / 2; // less important in endgame

    // ----- tapered blend -----
    let mg_contrib = (1.0 - phase) * score_mg as f64;
    let eg_contrib = phase * score_eg as f64;
    (mg_contrib + eg_contrib) as i32
}

fn kind_index(kind: PieceKind) -> usize {
    kind.index()
}

fn knight_structure(board: &Board, color: Color) -> i32 {
    let mut bonus = 0i32;
    for sq in 0..64 {
        if board.get(sq) == Some((color, PieceKind::Knight)) {
            let file = (sq % 8) as i32;
            if file > 0 && board.get(sq - 1) == Some((color, PieceKind::Knight)) {
                bonus += 12;
            }
            if file < 7 && board.get(sq + 1) == Some((color, PieceKind::Knight)) {
                bonus += 12;
            }
        }
    }
    bonus
}

/// Penalise pieces that are still on their starting back rank (not developed).
fn development(board: &Board, color: Color) -> i32 {
    let back_rank = if color == Color::White { 0 } else { 7 };
    let mut score = 0i32;
    for file in 0..8 {
        let sq = back_rank * 8 + file;
        if let Some((pc, kind)) = board.get(sq)
            && pc == color
            && kind != PieceKind::Knight
            && kind != PieceKind::Emperor
        {
            score -= match kind {
                PieceKind::Empress => 14,
                PieceKind::Priest => 10,
                PieceKind::Paladin => 10,
                PieceKind::Dragon => 7,
                _ => 0,
            };
        }
    }
    score
}

/// Bonus for controlling the four central squares.
fn centre_control(board: &Board, color: Color) -> i32 {
    const CENTRE: [usize; 4] = [27, 28, 35, 36]; // d4, e4, d5, e5
    let mut score = 0i32;
    for &sq in &CENTRE {
        if let Some((pc, _)) = board.get(sq)
            && pc == color
        {
            score += 12;
        }
    }
    score
}
