import type { Color, Piece, Square, SquareInput } from "./types.js";

export const DEFAULT_DEPTH = 10;

export const STARTING_BOARD =
    "RBPEQPBR" + "NNNNNNNN" + ".".repeat(32) + "nnnnnnnn" + "rbpeqpbr" + "w";

const FILES = "abcdefgh";
const PIECE_CODES: Record<string, Piece> = {
    E: { color: "white", kind: "emperor" },
    Q: { color: "white", kind: "empress" },
    B: { color: "white", kind: "priest" },
    P: { color: "white", kind: "paladin" },
    R: { color: "white", kind: "dragon" },
    N: { color: "white", kind: "knight" },
    e: { color: "black", kind: "emperor" },
    q: { color: "black", kind: "empress" },
    b: { color: "black", kind: "priest" },
    p: { color: "black", kind: "paladin" },
    r: { color: "black", kind: "dragon" },
    n: { color: "black", kind: "knight" },
};

const KIND_CODES: Record<Piece["kind"], string> = {
    emperor: "E",
    empress: "Q",
    priest: "B",
    paladin: "P",
    dragon: "R",
    knight: "N",
};

export function squareName(square: Square): string {
    if (!Number.isInteger(square) || square < 0 || square > 63) {
        throw new RangeError(`square out of range: ${square}`);
    }
    return `${FILES[square % 8]}${Math.floor(square / 8) + 1}`;
}

export function parseSquare(value: SquareInput): Square {
    if (typeof value === "number") {
        if (!Number.isInteger(value) || value < 0 || value > 63) {
            throw new RangeError(`square out of range: ${value}`);
        }
        return value;
    }

    const match = /^([a-h])([1-8])$/i.exec(value.trim());
    if (!match) {
        throw new RangeError(`invalid square: ${value}`);
    }
    const [, file, rank] = match;
    if (!file || !rank) {
        throw new RangeError(`invalid square: ${value}`);
    }
    return (Number(rank) - 1) * 8 + (file.toLowerCase().charCodeAt(0) - 97);
}

export function fileOf(square: Square): number {
    return square % 8;
}

export function rankOf(square: Square): number {
    return Math.floor(square / 8);
}

/** Serialise a position to the 65-byte board string the engine uses. */
export function toBoardString(
    squares: readonly (Piece | null)[],
    turn: Color
): string {
    if (squares.length !== 64) {
        throw new RangeError(`expected 64 squares, got ${squares.length}`);
    }

    let out = "";
    for (const piece of squares) {
        if (!piece) {
            out += ".";
            continue;
        }
        const code = KIND_CODES[piece.kind];
        out += piece.color === "white" ? code : code.toLowerCase();
    }
    return `${out}${turn === "white" ? "w" : "b"}`;
}

/** Parse a 65-byte board string into a square array and side to move. */
export function fromBoardString(board: string): {
    squares: (Piece | null)[];
    turn: Color;
} {
    if (board.length !== 65) {
        throw new RangeError(`expected 65 characters, got ${board.length}`);
    }
    if (board[64] !== "w" && board[64] !== "b") {
        throw new RangeError("last character must be w or b");
    }

    const squares: (Piece | null)[] = [];
    for (let index = 0; index < 64; index += 1) {
        const character = board[index];
        if (character === undefined) {
            throw new RangeError(`board ends at square ${index}`);
        }
        if (character === ".") {
            squares.push(null);
        } else {
            const piece = PIECE_CODES[character];
            if (!piece) {
                throw new RangeError(`unknown piece code: ${character}`);
            }
            squares.push(piece);
        }
    }

    return { squares, turn: board[64] === "w" ? "white" : "black" };
}

const MATE_SCORE = 90_000;

/** Human-readable centipawn score, e.g. `+1.24` or `M5`. */
export function formatScore(score: number): string {
    if (Math.abs(score) >= MATE_SCORE - 128) {
        const moves = Math.ceil((MATE_SCORE - Math.abs(score)) / 2);
        return score > 0 ? `M${moves}` : `-M${moves}`;
    }
    const pawns = score / 100;
    return `${pawns >= 0 ? "+" : ""}${pawns.toFixed(2)}`;
}
