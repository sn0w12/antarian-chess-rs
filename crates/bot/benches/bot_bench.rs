use chess_bot::evaluation::evaluate;
use chess_engine::*;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_evaluate(c: &mut Criterion) {
    let board = Board::initial();
    c.bench_function("evaluate_initial", |b| {
        b.iter(|| evaluate(black_box(&board)))
    });
}

fn bench_evaluate_midgame(c: &mut Criterion) {
    let mut board = Board::initial();
    board = board.make_move(&Move::new(12, 28, false)); // e2-e4
    board = board.make_move(&Move::new(52, 36, false)); // e7-e5
    board = board.make_move(&Move::new(5, 21, false)); // f1-c4
    board = board.make_move(&Move::new(61, 45, false)); // f8-c5
    board = board.make_move(&Move::new(1, 18, false)); // b1-c3
    board = board.make_move(&Move::new(57, 42, false)); // b8-c6
    c.bench_function("evaluate_midgame", |b| {
        b.iter(|| evaluate(black_box(&board)))
    });
}

criterion_group!(benches, bench_evaluate, bench_evaluate_midgame,);
criterion_main!(benches);
