//! Phased (tapered) evaluation for Antarian Chess.
//!
//! Scores are always from White's perspective (positive = White better).

use chess_engine::{Board, Color, PieceKind};

// ---------------------------------------------------------------------------
// Material — opening      midgame (MG)   /   endgame (EG)
// ---------------------------------------------------------------------------

const MG: [i32; 6] = [
    10_000, // Emperor
    900,    // Empress
    800,    // Priest  (bishop+knight hybrid — extremely versatile)
    500,    // Paladin (king+dabbaba — short-range, situational)
    600,    // Dragon  (rook that jumps over friendlies — never blocked)
    100,    // Knight  (pawn)
];

const EG: [i32; 6] = [
    10_000, // Emperor
    950,    // Empress  (relatively stronger in open endgames)
    700,    // Priest
    500,    // Paladin
    500,    // Dragon
    150,    // Knight  (pawns become more valuable as promotion nears)
];

// ---------------------------------------------------------------------------
// Mid-game PSQT  (see the player's perspective, indexed by PieceKind)
// ---------------------------------------------------------------------------

const PST_MG: [[i32; 64]; 6] = [
    /* Emperor — keep the back rank; advancing exposes the whole game */
    [
        0, 0, 0, 0, 0, 0, 0, 0, // rank 0 (home)
        -10, -10, -10, -10, -10, -10, -10, -10, // rank 1
        -20, -20, -20, -20, -20, -20, -20, -20, // rank 2
        -30, -30, -30, -30, -30, -30, -30, -30, // rank 3
        -40, -40, -40, -40, -40, -40, -40, -40, // rank 4
        -50, -50, -50, -50, -50, -50, -50, -50, // rank 5
        -60, -60, -60, -60, -60, -60, -60, -60, // rank 6
        -70, -70, -70, -70, -70, -70, -70, -70, // rank 7
    ],
    /* Empress — centralised */
    [
        -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 5, 5, 5, 5, 0, -10, -10, 5, 10, 15, 15, 10,
        5, -10, -5, 5, 15, 20, 20, 15, 5, -5, -5, 5, 15, 20, 20, 15, 5, -5, -10, 5, 10, 15, 15, 10,
        5, -10, -10, 0, 5, 5, 5, 5, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
    ],
    /* Priest — diagonals */
    [
        -20, -10, -10, -10, -10, -10, -10, -20, -10, 0, 5, 5, 5, 5, 0, -10, -10, 5, 15, 20, 20, 15,
        5, -10, -10, 5, 20, 25, 25, 20, 5, -10, -10, 5, 20, 25, 25, 20, 5, -10, -10, 5, 15, 20, 20,
        15, 5, -10, -10, 0, 5, 5, 5, 5, 0, -10, -20, -10, -10, -10, -10, -10, -10, -20,
    ],
    /* Paladin — centre */
    [
        -15, -10, -5, 0, 0, -5, -10, -15, -10, 0, 5, 10, 10, 5, 0, -10, -5, 5, 10, 15, 15, 10, 5,
        -5, 0, 10, 15, 20, 20, 15, 10, 0, 0, 10, 15, 20, 20, 15, 10, 0, -5, 5, 10, 15, 15, 10, 5,
        -5, -10, 0, 5, 10, 10, 5, 0, -10, -15, -10, -5, 0, 0, -5, -10, -15,
    ],
    /* Dragon — openness, 7th rank */
    [
        0, 0, 0, 5, 5, 0, 0, 0, 5, 10, 10, 10, 10, 10, 10, 5, -5, 0, 0, 5, 5, 0, 0, -5, -5, 0, 0,
        5, 5, 0, 0, -5, -5, 0, 0, 5, 5, 0, 0, -5, -5, 0, 0, 5, 5, 0, 0, -5, 5, 10, 10, 10, 10, 10,
        10, 5, 0, 0, 0, 5, 5, 0, 0, 0,
    ],
    /* Knight — rewarded for advancing and central control.
    No promotions exist in Antarian Chess, so values simply increase
    with advancement and favour the centre files. */
    [
        0, 0, 0, 0, 0, 0, 0, 0, // rank 0  (back rank — never here)
        5, 10, 15, 20, 20, 15, 10, 5, // rank 1  (starting rank — push to advance)
        15, 20, 25, 35, 35, 25, 20, 15, // rank 2
        25, 35, 40, 50, 50, 40, 35, 25, // rank 3
        35, 45, 55, 65, 65, 55, 45, 35, // rank 4  (centre control)
        40, 50, 60, 70, 70, 60, 50, 40, // rank 5
        45, 55, 65, 75, 75, 65, 55, 45, // rank 6
        35, 45, 55, 65, 65, 55, 45, 35, // rank 7  (last rank — still valuable)
    ],
];

// ---------------------------------------------------------------------------
// End-game PSQT  (emperor activates, knights stay active)
// ---------------------------------------------------------------------------

const PST_EG: [[i32; 64]; 6] = [
    /* Emperor — run to the centre */
    [
        -50, -40, -30, -20, -20, -30, -40, -50, -30, -20, -10, 0, 0, -10, -20, -30, -20, -10, 20,
        30, 30, 20, -10, -20, -10, 0, 30, 40, 40, 30, 0, -10, -10, 0, 30, 40, 40, 30, 0, -10, -20,
        -10, 20, 30, 30, 20, -10, -20, -30, -20, -10, 0, 0, -10, -20, -30, -50, -40, -30, -20, -20,
        -30, -40, -50,
    ],
    /* Empress — active anywhere */
    [
        -10, -5, -5, 0, 0, -5, -5, -10, -5, 0, 5, 5, 5, 5, 0, -5, -5, 5, 10, 10, 10, 10, 5, -5, 0,
        5, 10, 15, 15, 10, 5, 0, 0, 5, 10, 15, 15, 10, 5, 0, -5, 5, 10, 10, 10, 10, 5, -5, -5, 0,
        5, 5, 5, 5, 0, -5, -10, -5, -5, 0, 0, -5, -5, -10,
    ],
    /* Priest — centralised */
    [
        -10, -5, -5, 0, 0, -5, -5, -10, -5, 0, 5, 5, 5, 5, 0, -5, -5, 5, 10, 15, 15, 10, 5, -5, 0,
        5, 15, 20, 20, 15, 5, 0, 0, 5, 15, 20, 20, 15, 5, 0, -5, 5, 10, 15, 15, 10, 5, -5, -5, 0,
        5, 5, 5, 5, 0, -5, -10, -5, -5, 0, 0, -5, -5, -10,
    ],
    /* Paladin — active */
    [
        -5, 0, 0, 5, 5, 0, 0, -5, 0, 5, 5, 10, 10, 5, 5, 0, 0, 5, 10, 15, 15, 10, 5, 0, 5, 10, 15,
        20, 20, 15, 10, 5, 5, 10, 15, 20, 20, 15, 10, 5, 0, 5, 10, 15, 15, 10, 5, 0, 0, 5, 5, 10,
        10, 5, 5, 0, -5, 0, 0, 5, 5, 0, 0, -5,
    ],
    /* Dragon — active */
    [
        0, 5, 5, 5, 5, 5, 5, 0, 5, 10, 10, 10, 10, 10, 10, 5, 5, 10, 10, 10, 10, 10, 10, 5, 5, 10,
        10, 10, 10, 10, 10, 5, 5, 10, 10, 10, 10, 10, 10, 5, 5, 10, 10, 10, 10, 10, 10, 5, 5, 10,
        10, 10, 10, 10, 10, 5, 0, 5, 5, 5, 5, 5, 5, 0,
    ],
    /* Knight — even more valuable in endgames */
    [
        0, 0, 0, 0, 0, 0, 0, 0, // rank 0
        5, 10, 15, 20, 20, 15, 10, 5, // rank 1
        20, 25, 30, 40, 40, 30, 25, 20, // rank 2
        35, 40, 50, 60, 60, 50, 40, 35, // rank 3
        50, 55, 65, 75, 75, 65, 55, 50, // rank 4
        60, 65, 75, 85, 85, 75, 65, 60, // rank 5
        70, 75, 85, 95, 95, 85, 75, 70, // rank 6
        60, 65, 75, 85, 85, 75, 65, 60, // rank 7
    ],
];

// ---------------------------------------------------------------------------
// Phase calculation — 0 = midgame, 256 = endgame
// ---------------------------------------------------------------------------

/// Material weight used for the phase blend. Knights are pawns here and do not
/// shift the phase; emperors are excluded.
fn phase_weight(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Empress | PieceKind::Priest | PieceKind::Paladin => 2,
        PieceKind::Dragon => 1,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// King-safety penalty
// ---------------------------------------------------------------------------

fn king_safety(board: &Board, color: Color, emperor_sq: usize) -> i32 {
    let file = (emperor_sq % 8) as i32;
    let rank = (emperor_sq / 8) as i32;

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

/// Small bonus for having more moves (pseudo-legal is cheap enough).
fn mobility(board: &Board, color: Color) -> i32 {
    let n = board.count_all_moves(color) as i32;
    (n * 2).min(60)
}

// ---------------------------------------------------------------------------
// Main tapered evaluation
// ---------------------------------------------------------------------------

pub fn evaluate(board: &Board) -> i32 {
    let mut score_mg = 0i32;
    let mut score_eg = 0i32;
    let mut phase_total = 0i32;
    let mut emperors = [None; 2];
    let mut material = [0i32; 2];

    // ----- material + PSQT (one pass, collects phase and emperor squares) -----
    let mut occupied = board.occupied();
    while occupied != 0 {
        let sq = occupied.trailing_zeros() as usize;
        occupied &= occupied - 1;
        let Some((color, kind)) = board.get(sq) else {
            continue;
        };

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

        let ci = if color == Color::White { 0 } else { 1 };
        phase_total += phase_weight(kind);
        if kind == PieceKind::Emperor {
            emperors[ci] = Some(sq);
        } else {
            material[ci] += mg_mat;
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
    if let Some(sq) = emperors[0] {
        score_mg -= king_safety(board, Color::White, sq) * 2;
    }
    if let Some(sq) = emperors[1] {
        score_mg += king_safety(board, Color::Black, sq) * 2;
    }

    // ----- mobility (both phases) -----
    let mob_w = mobility(board, Color::White);
    let mob_b = mobility(board, Color::Black);
    score_mg += mob_w - mob_b;
    score_eg += (mob_w - mob_b) / 2; // less important in endgame

    // ----- endgame conversion: drive a bare emperor to the edge -----
    let advantage = material[0] - material[1];
    if advantage >= 400
        && !has_major_piece(board, Color::Black)
        && let (Some(strong), Some(weak)) = (emperors[0], emperors[1])
    {
        score_eg += mop_up(strong, weak);
    } else if advantage <= -400
        && !has_major_piece(board, Color::White)
        && let (Some(strong), Some(weak)) = (emperors[1], emperors[0])
    {
        score_eg -= mop_up(strong, weak);
    }

    // ----- tapered blend (0 = midgame, 256 = endgame) -----
    let phase = ((14 - phase_total).clamp(0, 14) * 256) / 14;
    (score_mg * (256 - phase) + score_eg * phase) / 256
}

pub(crate) fn has_major_piece(board: &Board, color: Color) -> bool {
    (0..64).any(|sq| {
        matches!(
            board.get(sq),
            Some((
                c,
                PieceKind::Empress | PieceKind::Priest | PieceKind::Paladin | PieceKind::Dragon
            )) if c == color
        )
    })
}

/// Convert a material advantage: push the weak emperor to the edge and bring
/// the strong one closer.
fn mop_up(strong: usize, weak: usize) -> i32 {
    let wf = (weak % 8) as i32;
    let wr = (weak / 8) as i32;
    let sf = (strong % 8) as i32;
    let sr = (strong / 8) as i32;

    let edge_distance = wf.min(7 - wf) + wr.min(7 - wr);
    let emperor_distance = (wf - sf).abs() + (wr - sr).abs();

    (6 - edge_distance) * 12 + (14 - emperor_distance) * 4
}

fn kind_index(kind: PieceKind) -> usize {
    kind.index()
}

fn knight_structure(board: &Board, color: Color) -> i32 {
    let mut bonus = 0i32;
    let home_rank = if color == Color::White { 1 } else { 6 };
    let mut occupied = board.occupied();
    while occupied != 0 {
        let sq = occupied.trailing_zeros() as usize;
        occupied &= occupied - 1;
        if board.get(sq) != Some((color, PieceKind::Knight)) {
            continue;
        }
        let rank = (sq / 8) as i32;
        let file = (sq % 8) as i32;
        // Connected knights
        if file > 0 && board.get(sq - 1) == Some((color, PieceKind::Knight)) {
            bonus += 15;
        }
        if file < 7 && board.get(sq + 1) == Some((color, PieceKind::Knight)) {
            bonus += 15;
        }
        // Advancement bonus on top of PST (toward the opponent's side).
        let advance = if color == Color::White {
            rank - home_rank
        } else {
            home_rank - rank
        };
        if advance > 0 {
            bonus += advance * 8;
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
            && kind != PieceKind::Emperor
        {
            score -= match kind {
                PieceKind::Empress => 14,
                PieceKind::Priest => 10,
                PieceKind::Paladin => 10,
                PieceKind::Dragon => 7,
                PieceKind::Knight => 6,
                _ => 0,
            };
        }
    }
    // Also penalise knights that haven't left their start rank.
    let pawn_rank = if color == Color::White { 1 } else { 6 };
    for file in 0..8 {
        let sq = pawn_rank * 8 + file;
        if board.get(sq) == Some((color, PieceKind::Knight)) {
            score -= 8;
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

#[cfg(test)]
mod tests {
    use super::*;
    use chess_engine::{Board, Color, PieceKind};

    #[test]
    fn middlegame_emperor_prefers_home_rank() {
        let home = Board::initial();
        let mut advanced = Board::initial();
        advanced.set(3, None);
        advanced.set(27, Some((Color::White, PieceKind::Emperor)));
        assert!(evaluate(&home) > evaluate(&advanced));
    }

    #[test]
    fn mop_up_rewards_cornered_emperor() {
        let mut cornered = Board::empty();
        cornered.set(42, Some((Color::White, PieceKind::Emperor))); // c6
        cornered.set(3, Some((Color::White, PieceKind::Empress))); // d1
        cornered.set(56, Some((Color::Black, PieceKind::Emperor))); // a8

        let mut central = Board::empty();
        central.set(42, Some((Color::White, PieceKind::Emperor)));
        central.set(3, Some((Color::White, PieceKind::Empress)));
        central.set(28, Some((Color::Black, PieceKind::Emperor))); // e4

        assert!(evaluate(&cornered) > evaluate(&central));
    }
}

#[cfg(test)]
mod symmetry_tests {
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

    fn mirrored(board: &Board) -> Board {
        let mut result = Board::empty();
        for sq in 0..64 {
            if let Some((color, kind)) = board.get(sq) {
                result.set(sq ^ 56, Some((color.opposite(), kind)));
            }
        }
        result.turn = board.turn.opposite();
        result
    }

    #[test]
    fn evaluation_is_color_symmetric() {
        let mut rng = Rng(0x0BAD_F00D_DEAD_BEEF);
        let mut board = Board::initial();

        for ply in 0..200 {
            assert_eq!(
                evaluate(&mirrored(&board)),
                -evaluate(&board),
                "asymmetric at ply {ply}: {:?}",
                board.squares()
            );

            let moves = board.generate_legal_moves(board.turn);
            if moves.is_empty() {
                break;
            }
            let mv = moves[(rng.next() % moves.len() as u64) as usize];
            board = board.make_move(&mv);
        }
    }
}
