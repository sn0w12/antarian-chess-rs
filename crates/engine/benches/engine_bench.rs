use chess_engine::*;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_initial(c: &mut Criterion) {
    c.bench_function("Board::initial", |b| b.iter(|| Board::initial()));
}

fn bench_make_move(c: &mut Criterion) {
    let board = Board::initial();
    let mv = Move::new(12, 28, false); // e2 → e4
    c.bench_function("make_move", |b| {
        b.iter(|| black_box(&board).make_move(black_box(&mv)))
    });
}

fn bench_generate_all_moves(c: &mut Criterion) {
    let board = Board::initial();
    c.bench_function("generate_all_moves_initial", |b| {
        b.iter(|| board.generate_all_moves(black_box(Color::White)))
    });
}

fn bench_generate_legal_moves(c: &mut Criterion) {
    let board = Board::initial();
    c.bench_function("generate_legal_moves_initial", |b| {
        b.iter(|| board.generate_legal_moves(black_box(Color::White)))
    });
}

fn bench_is_in_check(c: &mut Criterion) {
    let board = Board::initial();
    c.bench_function("is_in_check_initial", |b| {
        b.iter(|| black_box(&board).is_in_check(black_box(Color::White)))
    });
}

fn bench_is_in_check_check(c: &mut Criterion) {
    let mut board = Board::empty();
    board.set(4, Some((Color::White, PieceKind::Emperor))); // e1
    board.set(12, Some((Color::White, PieceKind::Knight))); // e2
    board.set(61, Some((Color::Black, PieceKind::Priest))); // h6 → gives check
    board.turn = Color::White;
    c.bench_function("is_in_check_active", |b| {
        b.iter(|| black_box(&board).is_in_check(black_box(Color::White)))
    });
}

fn bench_generate_legal_moves_midgame(c: &mut Criterion) {
    let mut board = Board::initial();
    board = board.make_move(&Move::new(12, 28, false)); // e2-e4
    board = board.make_move(&Move::new(52, 36, false)); // e7-e5
    board = board.make_move(&Move::new(5, 21, false)); // f1-c4
    board = board.make_move(&Move::new(61, 45, false)); // f8-c5
    board = board.make_move(&Move::new(1, 18, false)); // b1-c3
    board = board.make_move(&Move::new(57, 42, false)); // b8-c6
    c.bench_function("generate_legal_moves_midgame", |b| {
        b.iter(|| board.generate_legal_moves(black_box(Color::White)))
    });
}

criterion_group!(
    benches,
    bench_initial,
    bench_make_move,
    bench_generate_all_moves,
    bench_generate_legal_moves,
    bench_is_in_check,
    bench_is_in_check_check,
    bench_generate_legal_moves_midgame,
);
criterion_main!(benches);
