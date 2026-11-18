import type { Engine } from "./engine.js";
import type { Game } from "./engine.js";
import { loadEngine } from "./loader.js";
import type {
    ClientInfo,
    ClientMessage,
    LobbySummary,
    ServerMessage,
} from "./protocol.js";
import type { Color, EngineOptions, Move, SquareInput } from "./types.js";

export const PROTOCOL_VERSION = 1;
export const DEFAULT_SERVER_URL = "wss://chess.arathia.net/";

const PING_INTERVAL_MS = 20_000;

export type OnlineStatus =
    | "idle"
    | "matchmaking"
    | "lobby"
    | "playing"
    | "over";

export interface WebSocketLike {
    readyState: number;
    readonly OPEN: number;
    send(data: string): void;
    close(code?: number, reason?: string): void;
    onopen: ((event: unknown) => void) | null;
    onmessage: ((event: { data: string }) => void) | null;
    onclose: ((event: unknown) => void) | null;
    onerror: ((event: unknown) => void) | null;
}

export type WebSocketConstructor = new (url: string) => WebSocketLike;

export interface OnlineOptions extends EngineOptions {
    /** Display name shown to the opponent. */
    name: string;
    /** Server websocket URL. Defaults to the public server. */
    url?: string;
    /** Reuse an already loaded engine instead of loading one. */
    engine?: Engine;
    clientVersion?: string;
    machineId?: string;
    os?: string;
    arch?: string;
    debug?: boolean;
    appName?: string;
    /** Injected for tests and non-browser runtimes. */
    WebSocket?: WebSocketConstructor;
}

interface OnlineEvents {
    change: undefined;
    /** Every server message, before it is processed. */
    message: ServerMessage;
    error: string;
}

type Listener<T> = (payload: T) => void;

type JoinedMessage = Extract<ServerMessage, { type: "Joined" }>;

function randomId(): string {
    const crypto = globalThis.crypto;
    if (crypto && "randomUUID" in crypto) {
        return crypto.randomUUID();
    }
    return `web-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

export class OnlineClient {
    /** Assigned by the server after the handshake. */
    playerId = "";

    readonly game: Game;

    gameId: string | null = null;
    opponentName: string | null = null;
    color: Color | null = null;
    lobbyCode: string | null = null;
    lobbies: LobbySummary[] = [];
    status: OnlineStatus = "idle";
    rematchRequested = false;
    lastResult: string | null = null;
    lastReason: string | null = null;
    lastError: string | null = null;

    private readonly listeners = new Map<
        keyof OnlineEvents,
        Set<Listener<never>>
    >();
    private pingTimer: ReturnType<typeof setInterval> | undefined;
    private closed = false;

    private constructor(
        private readonly socket: WebSocketLike,
        engine: Engine
    ) {
        this.game = engine.createGame();
        socket.onmessage = (event) => this.handleRaw(event.data);
        socket.onclose = () => this.handleClose();
        socket.onerror = () => this.emit("error", "websocket error");
    }

    /** Connect, send `Join`, and resolve once the server confirms the player. */
    static async connect(options: OnlineOptions): Promise<OnlineClient> {
        const engine =
            options.engine ??
            (await loadEngine({
                wasmUrl: options.wasmUrl,
                wasmBytes: options.wasmBytes,
            }));
        const Constructor =
            options.WebSocket ??
            (globalThis.WebSocket as unknown as
                | WebSocketConstructor
                | undefined);
        if (!Constructor) {
            throw new Error("WebSocket is unavailable; pass options.WebSocket");
        }

        const socket = new Constructor(options.url ?? DEFAULT_SERVER_URL);
        await new Promise<void>((resolve, reject) => {
            socket.onopen = () => resolve();
            socket.onerror = () => reject(new Error("cannot reach the server"));
        });

        const client = new OnlineClient(socket, engine);
        const player: ClientInfo = {
            client_version: options.clientVersion ?? "web",
            machine_id: options.machineId ?? randomId(),
            os: options.os ?? "web",
            arch: options.arch ?? "wasm32",
            debug: options.debug ?? false,
            app_name: options.appName ?? "Antarian Chess Web",
            protocol_version: PROTOCOL_VERSION,
        };

        const joined = new Promise<JoinedMessage>((resolve, reject) => {
            const timer = setTimeout(() => {
                cleanup();
                reject(new Error("the server did not confirm the join"));
            }, 10_000);
            const cleanup = () => {
                clearTimeout(timer);
                offMessage();
                offError();
            };
            const offMessage = client.on("message", (message) => {
                if (message.type === "Joined") {
                    cleanup();
                    resolve(message);
                }
            });
            const offError = client.on("error", (message) => {
                cleanup();
                reject(new Error(message));
            });
        });
        client.send({
            type: "Join",
            data: { name: options.name, client: player },
        });
        const message = await joined;
        client.playerId = message.data.player_id;
        client.startHeartbeat();
        return client;
    }

    /** Subscribe to state changes, raw messages, or errors. */
    on<K extends keyof OnlineEvents>(
        event: K,
        listener: Listener<OnlineEvents[K]>
    ): () => void {
        let set = this.listeners.get(event);
        if (!set) {
            set = new Set();
            this.listeners.set(event, set);
        }
        set.add(listener as Listener<never>);
        return () => set.delete(listener as Listener<never>);
    }

    /** Send a protocol message. The type is checked against the generated union. */
    send(message: ClientMessage): void {
        if (this.closed) {
            throw new Error("connection is closed");
        }
        this.socket.send(JSON.stringify(message));
    }

    startMatchmaking(): void {
        this.send({ type: "StartMatchmaking" });
    }

    leaveMatchmaking(): void {
        this.send({ type: "LeaveMatchmaking" });
    }

    requestLobbyList(): void {
        this.send({ type: "RequestLobbyList" });
    }

    createLobby(privateLobby = false): void {
        this.send({
            type: "CreateLobby",
            data: { private_lobby: privateLobby },
        });
    }

    leaveLobby(): void {
        this.send({ type: "LeaveLobby" });
    }

    joinLobby(code: string): void {
        this.send({ type: "JoinLobby", data: { lobby_code: code } });
    }

    /** Legal moves in the current online game, optionally from one square. */
    legalMoves(from?: SquareInput): Move[] {
        return this.game.legalMoves(from);
    }

    /**
     * Play a move in the current online game. Applies it to the local game and
     * sends it to the server. Returns the applied move, or `null` when it is not
     * your turn or the move is illegal.
     */
    play(from: SquareInput, to: SquareInput): Move | null {
        if (this.status !== "playing" || !this.gameId) {
            return null;
        }
        if (this.color && this.game.state().turn !== this.color) {
            return null;
        }
        const move = this.game.move(from, to);
        if (!move) {
            return null;
        }
        this.send({
            type: "MakeMove",
            data: { game_id: this.gameId, from: move.from, to: move.to },
        });
        this.emit("change", undefined);
        return move;
    }

    resign(): void {
        if (this.gameId) {
            this.send({ type: "Resign", data: { game_id: this.gameId } });
        }
    }

    requestRematch(): void {
        if (this.gameId) {
            this.send({
                type: "RequestRematch",
                data: { game_id: this.gameId },
            });
        }
    }

    acceptRematch(): void {
        if (this.gameId) {
            this.send({
                type: "AcceptRematch",
                data: { game_id: this.gameId },
            });
        }
    }

    declineRematch(): void {
        if (this.gameId) {
            this.send({
                type: "DeclineRematch",
                data: { game_id: this.gameId },
            });
        }
    }

    sendChat(message: string): void {
        if (this.gameId) {
            this.send({
                type: "SendChat",
                data: { game_id: this.gameId, message },
            });
        }
    }

    close(reason?: string): void {
        if (this.closed) {
            return;
        }
        try {
            this.send({ type: "Disconnect", data: { reason: reason ?? null } });
        } catch {
            // The socket may already be gone.
        }
        this.closed = true;
        this.stopHeartbeat();
        this.socket.close();
    }

    private handleRaw(data: string): void {
        let message: ServerMessage;
        try {
            message = JSON.parse(data) as ServerMessage;
        } catch {
            return;
        }
        this.handleMessage(message);
    }

    private handleMessage(message: ServerMessage): void {
        switch (message.type) {
            case "Joined":
                this.playerId = message.data.player_id;
                break;
            case "MatchmakingStarted":
                this.status = "matchmaking";
                break;
            case "MatchmakingLeft":
                this.status = "idle";
                break;
            case "LobbyList":
            case "LobbyListUpdated":
                this.lobbies = message.data.lobbies;
                break;
            case "LobbyCreated":
                this.lobbyCode = message.data.lobby_code;
                this.status = "lobby";
                break;
            case "LobbyLeft":
                this.lobbyCode = null;
                this.status = "idle";
                break;
            case "MatchFound":
                this.startGame(
                    message.data.game_id,
                    message.data.opponent_name,
                    message.data.your_color
                );
                break;
            case "OpponentMove":
                if (message.data.game_id === this.gameId) {
                    this.game.move(message.data.from, message.data.to);
                }
                break;
            case "GameOver":
                if (message.data.game_id === this.gameId) {
                    this.status = "over";
                    this.lastResult = message.data.result ?? null;
                    this.lastReason = message.data.reason;
                }
                break;
            case "RematchRequested":
                if (message.data.game_id === this.gameId) {
                    this.rematchRequested = true;
                }
                break;
            case "RematchAccepted":
                this.startGame(
                    message.data.new_game_id,
                    this.opponentName ?? "",
                    message.data.your_color
                );
                break;
            case "RematchDeclined":
                if (message.data.game_id === this.gameId) {
                    this.rematchRequested = false;
                }
                break;
            case "OpponentDisconnected":
                if (message.data.game_id === this.gameId) {
                    this.status = "over";
                    this.lastReason = "opponent disconnected";
                }
                break;
            case "ChatMessage":
                break;
            case "Pong":
                break;
            case "Error":
                this.lastError = message.data.message;
                this.emit("error", message.data.message);
                break;
        }
        this.emit("message", message);
        this.emit("change", undefined);
    }

    private startGame(
        gameId: string,
        opponentName: string,
        color: string
    ): void {
        this.game.reset();
        this.gameId = gameId;
        this.opponentName = opponentName;
        this.color = color === "white" ? "white" : "black";
        this.status = "playing";
        this.rematchRequested = false;
        this.lastResult = null;
        this.lastReason = null;
        this.lobbyCode = null;
    }

    private handleClose(): void {
        this.closed = true;
        this.stopHeartbeat();
        this.emit("change", undefined);
    }

    private startHeartbeat(): void {
        this.stopHeartbeat();
        this.pingTimer = setInterval(() => {
            try {
                this.send({ type: "Ping" });
            } catch {
                this.stopHeartbeat();
            }
        }, PING_INTERVAL_MS);
    }

    private stopHeartbeat(): void {
        if (this.pingTimer !== undefined) {
            clearInterval(this.pingTimer);
            this.pingTimer = undefined;
        }
    }

    private emit<K extends keyof OnlineEvents>(
        event: K,
        payload: OnlineEvents[K]
    ): void {
        const set = this.listeners.get(event);
        if (!set) {
            return;
        }
        for (const listener of set) {
            (listener as Listener<OnlineEvents[K]>)(payload);
        }
    }
}

/** Connect to the server and resolve after the handshake completes. */
export function connectOnline(options: OnlineOptions): Promise<OnlineClient> {
    return OnlineClient.connect(options);
}
