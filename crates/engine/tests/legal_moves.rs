use chess_engine::*;

fn brute_force_legal(board: &Board, color: Color) -> Vec<(u8, u8)> {
    let mut moves: Vec<(u8, u8)> = board
        .generate_all_moves(color)
        .into_iter()
        .filter(|mv| !board.make_move(mv).is_in_check(color))
        .map(|mv| (mv.from, mv.to))
        .collect();
    moves.sort_unstable();
    moves
}

fn legal_move_pairs(board: &Board, color: Color) -> Vec<(u8, u8)> {
    let mut moves: Vec<(u8, u8)> = board
        .generate_legal_moves(color)
        .into_iter()
        .map(|mv| (mv.from, mv.to))
        .collect();
    moves.sort_unstable();
    moves
}

fn brute_force_captures(board: &Board, color: Color) -> Vec<(u8, u8)> {
    let mut moves: Vec<(u8, u8)> = board
        .generate_captures(color)
        .into_iter()
        .filter(|mv| !board.make_move(mv).is_in_check(color))
        .map(|mv| (mv.from, mv.to))
        .collect();
    moves.sort_unstable();
    moves
}

fn legal_capture_pairs(board: &Board, color: Color) -> Vec<(u8, u8)> {
    let mut moves: Vec<(u8, u8)> = board
        .generate_legal_captures(color)
        .into_iter()
        .map(|mv| (mv.from, mv.to))
        .collect();
    moves.sort_unstable();
    moves
}

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

#[test]
fn paladin_jump_discovery_is_not_legal() {
    let mut board = Board::empty();
    board.set(4, Some((Color::White, PieceKind::Emperor))); // e1
    board.set(3, Some((Color::White, PieceKind::Dragon))); // d1 blocks c1-e1 jump
    board.set(2, Some((Color::Black, PieceKind::Paladin))); // c1
    board.set(63, Some((Color::Black, PieceKind::Emperor)));

    assert!(!board.is_in_check(Color::White));
    let moves = legal_move_pairs(&board, Color::White);
    let dragon_moves: Vec<_> = moves.iter().filter(|&&(from, _)| from == 3).collect();
    assert_eq!(
        dragon_moves,
        vec![&(3, 2)],
        "only capturing the paladin may move the blocker: {moves:?}"
    );
    assert_eq!(moves, brute_force_legal(&board, Color::White));
}

#[test]
fn optimized_legal_generation_matches_brute_force() {
    for seed in [
        0x9E37_79B9_7F4A_7C15,
        0x0123_4567_89AB_CDEF,
        0xDEAD_BEEF_CAFE_BABE,
        0x1357_9BDF_2468_ACE0,
        0xA5A5_5A5A_1234_5678,
    ] {
        let mut rng = Rng(seed);
        let mut board = Board::initial();

        for ply in 0..300 {
            assert_eq!(
                legal_move_pairs(&board, board.turn),
                brute_force_legal(&board, board.turn),
                "legal moves diverged at seed {seed:#x} ply {ply}: {:?}",
                board.squares()
            );
            assert_eq!(
                legal_capture_pairs(&board, board.turn),
                brute_force_captures(&board, board.turn),
                "legal captures diverged at seed {seed:#x} ply {ply}: {:?}",
                board.squares()
            );

            for color in [Color::White, Color::Black] {
                assert_eq!(
                    board.count_all_moves(color),
                    board.generate_all_moves(color).len() as u32,
                    "move count diverged for {color:?} at seed {seed:#x} ply {ply}"
                );
                assert_eq!(
                    board.count_captures(color),
                    board.generate_captures(color).len() as u32,
                    "capture count diverged for {color:?} at seed {seed:#x} ply {ply}"
                );
            }

            let moves = brute_force_legal(&board, board.turn);
            if moves.is_empty() {
                break;
            }
            let pick = moves[(rng.next() % moves.len() as u64) as usize];
            let mv = Move::new(pick.0, pick.1, board.get(pick.1 as usize).is_some());
            board = board.make_move(&mv);
        }
    }
}
