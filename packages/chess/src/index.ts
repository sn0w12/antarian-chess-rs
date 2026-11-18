import { Game } from "./engine.js";
import { loadEngine } from "./loader.js";
import type { EngineOptions } from "./types.js";

export {
    DEFAULT_DEPTH,
    STARTING_BOARD,
    fileOf,
    formatScore,
    fromBoardString,
    parseSquare,
    rankOf,
    squareName,
    toBoardString,
} from "./board.js";
export { Engine, Game } from "./engine.js";
export { loadEngine } from "./loader.js";
export {
    DEFAULT_SERVER_URL,
    OnlineClient,
    PROTOCOL_VERSION,
    connectOnline,
} from "./online.js";
export type {
    OnlineOptions,
    OnlineStatus,
    WebSocketConstructor,
    WebSocketLike,
} from "./online.js";
export type * from "./protocol.js";
export type {
    BotResult,
    Color,
    EngineOptions,
    GameState,
    Move,
    Piece,
    PieceKind,
    PlayedBotMove,
    Square,
    SquareInput,
    Status,
    StatusKind,
} from "./types.js";

/**
 * Load the engine if needed and create a game in the starting position.
 *
 * ```ts
 * const game = await createGame({ wasmUrl: "/chess_wasm.wasm" });
 * game.move("b1", "c3");
 * const reply = game.playBest(11);
 * ```
 */
export async function createGame(options: EngineOptions = {}): Promise<Game> {
    const engine = await loadEngine(options);
    return engine.createGame();
}
