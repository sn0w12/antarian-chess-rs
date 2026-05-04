use crate::board::{Board, coord_to_index, index_to_coord};
use crate::types::{Color, Move};

/// Sliding moves in the given directions.
/// If `jump` is true, the piece can pass through occupied squares
/// (capturing opponent pieces, skipping own pieces).
/// If `captures_only` is true, only capture moves are generated.
pub fn generate_sliding_moves(
    board: &Board,
    from: usize,
    color: Color,
    directions: &[(i8, i8)],
    jump: bool,
    captures_only: bool,
) -> Vec<Move> {
    let mut moves = Vec::new();
    let (f, r) = index_to_coord(from);
    for &(df, dr) in directions {
        let mut file = f + df;
        let mut rank = r + dr;
        while let Some(idx) = coord_to_index(file, rank) {
            match board.get(idx) {
                None => {
                    if !captures_only {
                        moves.push(Move {
                            from: from as u8,
                            to: idx as u8,
                            capture: false,
                        });
                    }
                }
                Some((pc, _)) => {
                    if pc != color {
                        moves.push(Move {
                            from: from as u8,
                            to: idx as u8,
                            capture: true,
                        });
                    }
                    if !jump {
                        break;
                    }
                }
            }
            file += df;
            rank += dr;
        }
    }
    moves
}

/// Leaper moves defined by `offsets`.
/// If `jump` is false, the intermediate square (for offsets with one leg longer)
/// must be empty; otherwise the move is blocked.
/// If `captures_only` is true, only capture moves are generated.
pub fn generate_leaper_moves(
    board: &Board,
    from: usize,
    color: Color,
    offsets: &[(i8, i8)],
    jump: bool,
    captures_only: bool,
) -> Vec<Move> {
    let (f, r) = index_to_coord(from);
    offsets
        .iter()
        .filter_map(|&(df, dr)| {
            let to_file = f + df;
            let to_rank = r + dr;
            coord_to_index(to_file, to_rank).and_then(|to| {
                // Check blocking intermediate square if not jumping
                if !jump {
                    let adf = df.abs();
                    let adr = dr.abs();
                    if adf > adr {
                        let intermediate = coord_to_index(f + df.signum(), r);
                        if intermediate.is_some_and(|i| board.get(i).is_some()) {
                            return None;
                        }
                    } else if adr > adf {
                        let intermediate = coord_to_index(f, r + dr.signum());
                        if intermediate.is_some_and(|i| board.get(i).is_some()) {
                            return None;
                        }
                    }
                }

                match board.get(to) {
                    None => {
                        if captures_only {
                            None
                        } else {
                            Some(Move {
                                from: from as u8,
                                to: to as u8,
                                capture: false,
                            })
                        }
                    }
                    Some((pc, _)) if pc != color => Some(Move {
                        from: from as u8,
                        to: to as u8,
                        capture: true,
                    }),
                    _ => None,
                }
            })
        })
        .collect()
}
