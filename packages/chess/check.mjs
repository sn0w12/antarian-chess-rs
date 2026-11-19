import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
    STARTING_BOARD,
    formatScore,
    loadEngine,
    parseSquare,
    squareName,
} from "./dist/index.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmBytes = fs.readFileSync(path.join(here, "dist/chess_wasm.wasm"));

const engine = await loadEngine({ wasmBytes });
const game = engine.createGame();

const start = game.state();
if (start.turn !== "white" || start.squares.filter(Boolean).length !== 32) {
    throw new Error("bad starting position");
}

const knightMoves = game.legalMoves("b1");
if (knightMoves.length !== 2) {
    throw new Error(`expected 2 knight moves, got ${knightMoves.length}`);
}

const first = game.move("b1", "c3");
if (!first || game.state().turn !== "black") {
    throw new Error("b1-c3 did not apply");
}

const reply = game.playBest(8);
if (!reply || game.state().turn !== "white") {
    throw new Error("bot reply did not apply");
}

if (!game.undo() || game.state().turn !== "black") {
    throw new Error("undo failed");
}

const serialised = game.boardString();
if (serialised.length !== 65) {
    throw new Error("board string has the wrong length");
}

game.load(STARTING_BOARD);
if (game.state().squares.filter(Boolean).length !== 32) {
    throw new Error("loading the starting board failed");
}

const analysis = engine.bestMoveFor(STARTING_BOARD, 8);
if (!analysis) {
    throw new Error("stateless analysis returned nothing");
}

if (squareName(parseSquare("e4")) !== "e4") {
    throw new Error("square helpers disagree");
}

if (game.move("a1", "h8") !== null) {
    throw new Error("illegal move was accepted");
}

const promotionSquares = Array(64).fill(".");
promotionSquares[4] = "E";
promotionSquares[48] = "N";
promotionSquares[63] = "e";
const promotionBoard = `${promotionSquares.join("")}w`;
game.load(promotionBoard);
const promotions = game
    .legalMoves("a7")
    .filter((move) => move.to === parseSquare("a8"));
if (promotions.length !== 4) {
    throw new Error(`expected 4 promotions, got ${promotions.length}`);
}
for (const move of promotions) {
    game.load(promotionBoard);
    const applied = game.move("a7", "a8", move.promotion);
    if (!applied || game.pieceAt("a8")?.kind !== move.promotion) {
        throw new Error(`promotion to ${move.promotion} did not apply`);
    }
}

game.destroy();
console.log("engine ok", {
    reply: reply.move,
    analysis: analysis.move,
    eval: formatScore(analysis.score),
});
