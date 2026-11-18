import {
  loadEngine as loadPackageEngine,
  type Color,
  type Engine,
  type Game,
  type GameState,
  type Move,
  type Piece,
  type PieceKind,
} from "antarian-chess";

export type { Color, Game, GameState, Move, Piece, PieceKind };

const ENGINE_URL = "/chess_wasm.wasm";

let enginePromise: Promise<Engine> | undefined;

export function loadEngine(): Promise<Engine> {
  enginePromise ??= loadPackageEngine({ wasmUrl: ENGINE_URL });
  return enginePromise;
}

export async function createGame(): Promise<Game> {
  const engine = await loadEngine();
  return engine.createGame();
}

export const STARTING_PIECES: Record<PieceKind, number> = {
  emperor: 1,
  empress: 1,
  priest: 2,
  paladin: 2,
  dragon: 2,
  knight: 8,
};

export const PIECE_VALUES: Record<PieceKind, number> = {
  emperor: 0,
  empress: 9,
  priest: 7,
  paladin: 6,
  dragon: 6,
  knight: 1,
};

export function materialBalance(squares: readonly (Piece | null)[]): number {
  let score = 0;
  for (const piece of squares) {
    if (!piece) continue;
    const value = PIECE_VALUES[piece.kind];
    score += piece.color === "white" ? value : -value;
  }
  return Math.round(score * 100);
}

export function capturedPieces(
  squares: readonly (Piece | null)[],
  color: Color,
): PieceKind[] {
  const counts = new Map<PieceKind, number>();
  for (const piece of squares) {
    if (piece?.color === color) {
      counts.set(piece.kind, (counts.get(piece.kind) ?? 0) + 1);
    }
  }

  const missing: PieceKind[] = [];
  for (const [kind, total] of Object.entries(STARTING_PIECES) as [
    PieceKind,
    number,
  ][]) {
    const lost = total - (counts.get(kind) ?? 0);
    for (let index = 0; index < lost; index += 1) {
      missing.push(kind);
    }
  }
  return missing;
}

export function evalFraction(centipawns: number): number {
  const probability = 1 / (1 + Math.exp(-centipawns / 400));
  return Math.min(0.97, Math.max(0.03, probability));
}

export function formatEval(centipawns: number): string {
  const pawns = centipawns / 100;
  return `${pawns >= 0 ? "+" : ""}${pawns.toFixed(2)}`;
}

export function formatClock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  return `${minutes.toString().padStart(2, "0")}:${(total % 60)
    .toString()
    .padStart(2, "0")}`;
}

export function botThinkTime(
  remaining: number,
  moveNumber: number,
  legalMoves: number,
): number {
  const expected = Math.max(20, 100 - moveNumber);
  const perMove = remaining / expected;
  const complexity = Math.min(2, Math.max(0.5, legalMoves / 30));
  const maxTime = remaining * 0.33;
  let base = Math.min(
    Math.max(perMove * complexity, 0.5),
    Math.max(maxTime, 0.5),
  );
  if (moveNumber < 20) {
    base = Math.min(base * 1.5, Math.max(maxTime, 0.5));
  }
  return base;
}
