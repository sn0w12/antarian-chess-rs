use chess_bot::{find_best_move, mate_distance};
use chess_engine::*;
use std::time::Duration;

fn board_with(pieces: &[(usize, Color, PieceKind)], turn: Color) -> Board {
    let mut board = Board::empty();
    for &(square, color, kind) in pieces {
        board.set(square, Some((color, kind)));
    }
    board.turn = turn;
    board
}

#[test]
fn initial_position_is_balanced() {
    assert_eq!(chess_bot::evaluation::evaluate(&Board::initial()), 0);
}

#[test]
fn finds_mate_in_one() {
    // White: emperor f6, empress g6. Black: emperor h8.
    // Empress to g7 is mate.
    let board = board_with(
        &[
            (45, Color::White, PieceKind::Emperor),
            (46, Color::White, PieceKind::Empress),
            (63, Color::Black, PieceKind::Emperor),
        ],
        Color::White,
    );
    let mating_move = Move::new(46, 54, false);
    assert_eq!(
        board.make_move(&mating_move).game_result(),
        GameResult::Checkmate {
            winner: Color::White
        }
    );

    let result = find_best_move(&board, Duration::from_millis(500)).expect("legal moves exist");
    assert_eq!(result.best_move, mating_move);
    assert_eq!(mate_distance(result.score), Some(1));
}

#[test]
fn prefers_mate_over_stalemate() {
    let board = board_with(
        &[
            (45, Color::White, PieceKind::Emperor),
            (46, Color::White, PieceKind::Empress),
            (63, Color::Black, PieceKind::Emperor),
        ],
        Color::White,
    );
    let result = find_best_move(&board, Duration::from_millis(500)).expect("legal moves exist");
    let child = board.make_move(&result.best_move);
    assert_ne!(child.game_result(), GameResult::Stalemate);
}

#[test]
fn returns_none_without_legal_moves() {
    let checkmate = board_with(
        &[
            (45, Color::White, PieceKind::Emperor),
            (54, Color::White, PieceKind::Empress), // g7
            (63, Color::Black, PieceKind::Emperor), // h8
        ],
        Color::Black,
    );
    assert!(matches!(
        checkmate.game_result(),
        GameResult::Checkmate { .. }
    ));
    assert!(find_best_move(&checkmate, Duration::from_millis(100)).is_none());

    let stalemate = board_with(
        &[
            (42, Color::White, PieceKind::Emperor), // c6
            (41, Color::White, PieceKind::Empress), // b6
            (56, Color::Black, PieceKind::Emperor), // a8
        ],
        Color::Black,
    );
    assert_eq!(stalemate.game_result(), GameResult::Stalemate);
    assert!(find_best_move(&stalemate, Duration::from_millis(100)).is_none());
}

#[test]
fn takes_a_free_piece() {
    let board = board_with(
        &[
            (4, Color::White, PieceKind::Emperor),  // e1
            (3, Color::White, PieceKind::Empress),  // d1
            (51, Color::Black, PieceKind::Priest),  // d7, undefended
            (63, Color::Black, PieceKind::Emperor), // h8
        ],
        Color::White,
    );
    let result = find_best_move(&board, Duration::from_millis(500)).expect("legal moves exist");
    assert_eq!(result.best_move, Move::new(3, 51, true));
}

#[test]
fn self_play_only_returns_legal_moves() {
    let mut board = Board::initial();
    for ply in 0..16 {
        if board.game_result() != GameResult::Ongoing {
            break;
        }
        let legal = board.generate_legal_moves(board.turn);
        let result = find_best_move(&board, Duration::from_millis(40)).expect("legal moves exist");
        assert!(
            legal.contains(&result.best_move),
            "illegal move {:?} at ply {ply}",
            result.best_move
        );
        board = board.make_move(&result.best_move);
    }
}
