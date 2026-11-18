use crate::rays;
use crate::types::{Color, Move};

/// Sliding moves in the given directions.
/// If `jump` is true, the piece can pass through occupied squares
/// (capturing opponent pieces, skipping own pieces).
/// If `captures_only` is true, only capture moves are generated.
pub fn generate_sliding_moves(
    board: &crate::board::Board,
    from: usize,
    color: Color,
    directions: &[(i8, i8)],
    jump: bool,
    captures_only: bool,
    moves: &mut Vec<Move>,
) {
    let sqs = board.squares();
    for &(df, dr) in directions {
        let Some(dir) = rays::direction_index(df, dr) else {
            continue;
        };
        rays::generate_ray_moves(from, dir, sqs, color, captures_only, jump, moves);
    }
}

/// Leaper moves defined by `offsets`.
pub fn generate_leaper_moves(
    board: &crate::board::Board,
    from: usize,
    color: Color,
    offsets: &'static [(i8, i8)],
    jump: bool,
    captures_only: bool,
    moves: &mut Vec<Move>,
) {
    let sqs = board.squares();
    let set = rays::LeaperSet::new(offsets);
    set.generate(from, sqs, color, captures_only, jump, moves);
}
