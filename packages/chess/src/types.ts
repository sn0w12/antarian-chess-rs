export type Color = "white" | "black";

export type PieceKind =
    | "emperor"
    | "empress"
    | "priest"
    | "paladin"
    | "dragon"
    | "knight";

export interface Piece {
    color: Color;
    kind: PieceKind;
}

/** Square indices run from 0 (a1) to 63 (h8). */
export type Square = number;

export interface Move {
    from: Square;
    to: Square;
    capture: boolean;
    /** Piece chosen when a knight promotes, otherwise `null`. */
    promotion: PieceKind | null;
}

export type StatusKind = "ongoing" | "checkmate" | "stalemate";

export interface Status {
    kind: StatusKind;
    winner: Color | null;
    inCheck: boolean;
}

export interface GameState {
    squares: (Piece | null)[];
    turn: Color;
    status: Status;
    /** Number of moves played since the current position's start. */
    moveNumber: number;
    lastMove: Move | null;
}

export interface BotResult {
    move: Move;
    /** Centipawns from the side to move. Mate scores are near +/-90000. */
    score: number;
    depth: number;
    nodes: number;
}

export interface PlayedBotMove extends BotResult {
    state: GameState;
}

export interface EngineOptions {
    /** URL of `chess_wasm.wasm`. Defaults to the file next to the package. */
    wasmUrl?: string | URL;
    /** Raw wasm bytes, useful in Node and tests. */
    wasmBytes?: BufferSource;
}

export type SquareInput = Square | string;
