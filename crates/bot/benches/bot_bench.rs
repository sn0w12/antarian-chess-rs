use chess_bot::Searcher;
use chess_bot::evaluation::evaluate;
use chess_engine::*;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const SEARCH_DEPTH: u32 = 8;

fn midgame_board() -> Board {
    let mut board = Board::initial();
    board = board.make_move(&Move::new(12, 28, false)); // e2-e4
    board = board.make_move(&Move::new(52, 36, false)); // e7-e5
    board = board.make_move(&Move::new(5, 21, false)); // f1-c4
    board = board.make_move(&Move::new(61, 45, false)); // f8-c5
    board = board.make_move(&Move::new(1, 18, false)); // b1-c3
    board = board.make_move(&Move::new(57, 42, false)); // b8-c6
    board
}

fn promotion_board() -> Board {
    let mut board = Board::empty();
    board.set(4, Some((Color::White, PieceKind::Emperor))); // e1
    board.set(48, Some((Color::White, PieceKind::Knight))); // a7
    board.set(63, Some((Color::Black, PieceKind::Emperor))); // h8
    board.turn = Color::White;
    board
}

// A fresh Searcher gives each search a cold transposition table. Criterion
// excludes the setup from the measurement, so only search time is reported.
fn bench_search(c: &mut Criterion, name: &str, board: &Board) {
    c.bench_function(name, |b| {
        b.iter_batched(
            Searcher::new,
            |searcher| {
                black_box(searcher.find_best_move_depth(black_box(board), SEARCH_DEPTH))
            },
            BatchSize::PerIteration,
        )
    });
}

fn bench_evaluate(c: &mut Criterion) {
    let board = Board::initial();
    c.bench_function("evaluate_initial", |b| {
        b.iter(|| evaluate(black_box(&board)))
    });
}

fn bench_evaluate_midgame(c: &mut Criterion) {
    let board = midgame_board();
    c.bench_function("evaluate_midgame", |b| {
        b.iter(|| evaluate(black_box(&board)))
    });
}

fn bench_search_initial(c: &mut Criterion) {
    bench_search(c, "search_initial_depth_8", &Board::initial());
}

fn bench_search_midgame(c: &mut Criterion) {
    bench_search(c, "search_midgame_depth_8", &midgame_board());
}

fn bench_search_promotion(c: &mut Criterion) {
    bench_search(c, "search_promotion_depth_8", &promotion_board());
}

criterion_group!(
    benches,
    bench_evaluate,
    bench_evaluate_midgame,
    bench_search_initial,
    bench_search_midgame,
    bench_search_promotion,
);
criterion_main!(benches);
