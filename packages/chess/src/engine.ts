import { DEFAULT_DEPTH, parseSquare, toBoardString } from "./board.js";
import type {
    BotResult,
    EngineOptions,
    GameState,
    Move,
    PieceKind,
    PlayedBotMove,
    Square,
    SquareInput,
    Status,
} from "./types.js";
import { instantiateWasm, type WasmExports } from "./wasm.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface ErrorResult {
    error: string;
}

type PlayResponse =
    | { ok: true; move: Move; state: GameState }
    | { ok: false; error: string };

type PlayBestResponse =
    | {
          ok: true;
          move: Move;
          score: number;
          depth: number;
          nodes: number;
          state: GameState;
      }
    | { ok: false; error: string };

type UndoResponse =
    | { ok: true; state: GameState }
    | { ok: false; error: string };

type BestMoveResponse =
    | { move: Move; score: number; depth: number; nodes: number }
    | { move: null; error: string };

const PROMOTION_CODES: Record<PieceKind, number> = {
    empress: 1,
    priest: 2,
    paladin: 3,
    dragon: 4,
    emperor: 0,
    knight: 0,
};

function normalizeDepth(depth: number): number {
    if (!Number.isFinite(depth)) {
        return DEFAULT_DEPTH;
    }
    return Math.min(30, Math.max(1, Math.round(depth)));
}

function isError(value: object): value is ErrorResult {
    return "error" in value;
}

export class Engine {
    /** @internal Raw wasm exports. */
    readonly wasm: WasmExports;

    private constructor(wasm: WasmExports) {
        this.wasm = wasm;
    }

    static async load(options: EngineOptions = {}): Promise<Engine> {
        return new Engine(await instantiateWasm(options));
    }

    /** Create a new game in the starting position. */
    createGame(): Game {
        return new Game(this, this.wasm.game_create());
    }

    /**
     * Analyse an arbitrary 65-byte board string without creating a game.
     * Returns `null` when the position has no legal moves.
     */
    bestMoveFor(board: string, depth = DEFAULT_DEPTH): BotResult | null {
        const response = this.withString<BestMoveResponse>(
            board,
            (pointer, length) =>
                this.wasm.bot_move(pointer, length, normalizeDepth(depth))
        );
        if (!response.move) {
            return null;
        }
        return response as BotResult;
    }

    /** @internal */
    decode<T>(length: number): T {
        const pointer = this.wasm.result_ptr();
        const bytes = new Uint8Array(this.wasm.memory.buffer, pointer, length);
        return JSON.parse(decoder.decode(bytes)) as T;
    }

    /** @internal */
    withString<T>(
        value: string,
        call: (pointer: number, length: number) => number
    ): T {
        const bytes = encoder.encode(value);
        const pointer = this.wasm.alloc(bytes.length);
        try {
            new Uint8Array(this.wasm.memory.buffer, pointer, bytes.length).set(
                bytes
            );
            return this.decode<T>(call(pointer, bytes.length));
        } finally {
            this.wasm.dealloc(pointer, bytes.length);
        }
    }
}

export class Game {
    private destroyed = false;

    /** @internal Create games through {@link Engine.createGame}. */
    constructor(
        private readonly engine: Engine,
        readonly id: number
    ) {}

    /** Current position, side to move, status, and last move. */
    state(): GameState {
        const state = this.engine.decode<GameState | ErrorResult>(
            this.engine.wasm.game_state(this.id)
        );
        if (isError(state)) {
            throw new Error(state.error);
        }
        return state;
    }

    status(): Status {
        return this.state().status;
    }

    /** Reset to the starting position. */
    reset(): GameState {
        return this.engine.decode<GameState>(
            this.engine.wasm.game_reset(this.id)
        );
    }

    /** Load a 65-byte board string, replacing the position and history. */
    load(board: string): GameState {
        const state = this.engine.withString<GameState | ErrorResult>(
            board,
            (pointer, length) =>
                this.engine.wasm.game_load(this.id, pointer, length)
        );
        if (isError(state)) {
            throw new Error(state.error);
        }
        return state;
    }

    /** Serialise the current position to a board string. */
    boardString(): string {
        const state = this.state();
        return toBoardString(state.squares, state.turn);
    }

    /**
     * Legal moves, optionally only the ones starting on `from`.
     * Squares can be indices or names such as `"e2"`.
     */
    legalMoves(from?: SquareInput): Move[] {
        const filter = from === undefined ? 255 : parseSquare(from);
        return this.engine.decode<Move[]>(
            this.engine.wasm.game_legal_moves(this.id, filter)
        );
    }

    /**
     * Play a move if it is legal. Returns the applied move, or `null`.
     * Pass `promotion` to choose the piece a knight promotes to; without it the
     * engine picks the first legal promotion.
     */
    move(
        from: SquareInput,
        to: SquareInput,
        promotion?: PieceKind
    ): Move | null {
        const response = this.engine.decode<PlayResponse>(
            this.engine.wasm.game_play(
                this.id,
                parseSquare(from),
                parseSquare(to),
                promotion ? (PROMOTION_CODES[promotion] ?? 0) : 0
            )
        );
        return response.ok ? response.move : null;
    }

    /** Take back the last move. Returns the restored state, or `null`. */
    undo(): GameState | null {
        const response = this.engine.decode<UndoResponse>(
            this.engine.wasm.game_undo(this.id)
        );
        return response.ok ? response.state : null;
    }

    /** Search the current position without playing the move. */
    bestMove(depth = DEFAULT_DEPTH): BotResult | null {
        const response = this.engine.decode<BestMoveResponse>(
            this.engine.wasm.game_best_move(this.id, normalizeDepth(depth))
        );
        if (!response.move) {
            return null;
        }
        return response as BotResult;
    }

    /** Search and play the resulting move. */
    playBest(depth = DEFAULT_DEPTH): PlayedBotMove | null {
        const response = this.engine.decode<PlayBestResponse>(
            this.engine.wasm.game_play_best(this.id, normalizeDepth(depth))
        );
        if (!response.ok) {
            return null;
        }
        return {
            move: response.move,
            score: response.score,
            depth: response.depth,
            nodes: response.nodes,
            state: response.state,
        };
    }

    /** Piece on a square, or `null` when empty. */
    pieceAt(square: SquareInput): GameState["squares"][number] {
        const index = parseSquare(square) as Square;
        return this.state().squares[index] ?? null;
    }

    /** Release the wasm-side state. The instance stays usable for other games. */
    destroy(): void {
        if (this.destroyed) {
            return;
        }
        this.destroyed = true;
        this.engine.wasm.game_destroy(this.id);
    }
}
