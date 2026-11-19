// Smoke test for the raw wasm module.
//
//   cargo build -p chess_wasm --target wasm32-unknown-unknown --release
//   node crates/wasm/check.mjs

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.resolve(
  here,
  "../../target/wasm32-unknown-unknown/release/chess_wasm.wasm",
);

const { instance } = await WebAssembly.instantiate(fs.readFileSync(wasmPath), {});
const wasm = instance.exports;
const decoder = new TextDecoder();

function readResult(length) {
  const pointer = wasm.result_ptr();
  const bytes = new Uint8Array(wasm.memory.buffer, pointer, length);
  return JSON.parse(decoder.decode(bytes));
}

function gameState(id) {
  return readResult(wasm.game_state(id));
}

function legalMoves(id, from = 255) {
  return readResult(wasm.game_legal_moves(id, from));
}

function play(id, from, to, promotion = 0) {
  return readResult(wasm.game_play(id, from, to, promotion));
}

const id = wasm.game_create();
const start = gameState(id);
console.log("turn:", start.turn, "pieces:", start.squares.filter(Boolean).length);

const knightMoves = legalMoves(id, 1);
console.log("b1 moves:", knightMoves);
if (knightMoves.length !== 2) process.exit(1);

const applied = play(id, 1, 18);
console.log("b1-c3:", applied.ok, applied.state.turn, applied.state.moveNumber);
if (!applied.ok || applied.state.turn !== "black") process.exit(1);

console.log("reply:", readResult(wasm.game_best_move(id, 10)));

console.log("undo:", readResult(wasm.game_undo(id)).state.turn);
console.log("illegal:", play(id, 0, 63));

wasm.game_destroy(id);
console.log("ok");
