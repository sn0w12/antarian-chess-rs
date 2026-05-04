use crate::board::{Board, coord_to_index, index_to_coord};
use crate::movegen;
use crate::types::{Color, Move};

// ---------- Emperor ----------
pub fn generate_emperor_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    const OFFSETS: [(i8, i8); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    movegen::generate_leaper_moves(board, from, color, &OFFSETS, true, captures_only)
}

// ---------- Empress ----------
pub fn generate_empress_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    const DIRECTIONS: [(i8, i8); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    movegen::generate_sliding_moves(board, from, color, &DIRECTIONS, false, captures_only)
}

// ---------- Priest ----------
pub fn generate_priest_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    // Diagonal sliding (bishop)
    const DIAG_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
    let mut moves =
        movegen::generate_sliding_moves(board, from, color, &DIAG_DIRS, false, captures_only);

    // Knight jumps
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
    moves.extend(movegen::generate_leaper_moves(
        board,
        from,
        color,
        &KNIGHT_OFFSETS,
        true,
        captures_only,
    ));
    moves
}

// ---------- Paladin ----------
pub fn generate_paladin_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    // 1) Adjacent squares (like Emperor)
    const ADJACENT: [(i8, i8); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    let mut moves =
        movegen::generate_leaper_moves(board, from, color, &ADJACENT, true, captures_only);

    // 2) 2‑square orthogonal jumps, can jump over friendly (or empty) intermediate
    {
        let (f, r) = index_to_coord(from);
        for &(df, dr) in &[(-2, 0), (2, 0), (0, -2), (0, 2)] {
            let to_file = f + df;
            let to_rank = r + dr;
            if let Some(to_idx) = coord_to_index(to_file, to_rank) {
                if let Some((pc, _)) = board.get(to_idx) {
                    if pc == color {
                        continue;
                    }
                } else if captures_only {
                    continue; // skip quiet moves in captures-only mode
                }
                // Intermediate square: one step in the direction
                let intermediate_file = f + df.signum();
                let intermediate_rank = r + dr.signum();
                let intermediate_idx = coord_to_index(intermediate_file, intermediate_rank)
                    .expect("intermediate should be valid");
                let intermediate_ok = match board.get(intermediate_idx) {
                    None => true,
                    Some((pc, _)) => pc == color, // own piece – allowed to jump over
                };
                if intermediate_ok {
                    moves.push(Move {
                        from: from as u8,
                        to: to_idx as u8,
                        capture: board.get(to_idx).is_some_and(|(pc, _)| pc != color),
                    });
                }
            }
        }
    }
    moves
}

// ---------- Dragon ----------
pub fn generate_dragon_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    let mut moves = Vec::new();
    let (f, r) = index_to_coord(from);
    let directions: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    for &(df, dr) in &directions {
        let mut file = f + df;
        let mut rank = r + dr;
        while let Some(to_idx) = coord_to_index(file, rank) {
            match board.get(to_idx) {
                None => {
                    if !captures_only {
                        moves.push(Move {
                            from: from as u8,
                            to: to_idx as u8,
                            capture: false,
                        });
                    }
                }
                Some((pc, _)) => {
                    if pc == color {
                        // Friendly piece – jump over it, continue sliding
                        file += df;
                        rank += dr;
                        continue;
                    } else {
                        // Enemy piece – capture and stop
                        moves.push(Move {
                            from: from as u8,
                            to: to_idx as u8,
                            capture: true,
                        });
                    }
                    break;
                }
            }
            file += df;
            rank += dr;
        }
    }
    moves
}

// ---------- Knight (pawn‑like) ----------
pub fn generate_knight_moves(
    board: &Board,
    from: usize,
    color: Color,
    captures_only: bool,
) -> Vec<Move> {
    let mut moves = Vec::new();
    let (f, r) = index_to_coord(from);
    let forward = if color == Color::White { 1 } else { -1 };
    let start_rank = if color == Color::White { 1 } else { 6 };

    // one step forward (quiet)
    if !captures_only
        && let Some(idx) = coord_to_index(f, r + forward)
        && board.get(idx).is_none()
    {
        moves.push(Move {
            from: from as u8,
            to: idx as u8,
            capture: false,
        });
        // double step from start rank
        if r == start_rank
            && let Some(idx2) = coord_to_index(f, r + 2 * forward)
            && board.get(idx2).is_none()
        {
            moves.push(Move {
                from: from as u8,
                to: idx2 as u8,
                capture: false,
            });
        }
    }

    // diagonal captures
    for df in [-1, 1] {
        let to_file = f + df;
        let to_rank = r + forward;
        if let Some(idx) = coord_to_index(to_file, to_rank)
            && let Some((pc, _)) = board.get(idx)
            && pc != color
        {
            moves.push(Move {
                from: from as u8,
                to: idx as u8,
                capture: true,
            });
        }
    }
    moves
}
