# antarian-chess

Antarian chess for the browser: move generation, bot search, and an online
client. The engine and bot are the same Rust code the desktop app uses,
compiled to WebAssembly. The protocol types are generated from the server's
Rust definitions.

## Build

From this directory:

```bash
npm install
npm run build
```

That regenerates `src/protocol.ts` from `crates/server`, builds
`crates/wasm` to `dist/chess_wasm.wasm`, and compiles the TypeScript into
`dist/`.

```bash
npm test        # engine and online smoke tests
npm run check   # type-check only
```

## Local games

```ts
import { createGame, formatScore, squareName } from "antarian-chess";

const game = await createGame({ wasmUrl: "/chess_wasm.wasm" });

game.state(); // squares, turn, status, moveNumber, lastMove
game.legalMoves("b1"); // [{ from: 1, to: 16, capture: false, promotion: null }, ...]
game.legalMoves(); // every legal move

const move = game.move("b1", "c3");
const reply = game.playBest(11); // searches and plays
console.log(reply?.move, formatScore(reply?.score ?? 0));
```

`createGame` loads the wasm module on first use and reuses it afterwards, so
several games on one page share a transposition table.

Squares are indices `0` (a1) to `63` (h8). `squareName` and `parseSquare`
convert to and from names. Pieces are `{ color: "white" | "black", kind }`
where `kind` is `emperor`, `empress`, `priest`, `paladin`, `dragon`, or
`knight`.

Knights act as pawns and promote when they reach the last rank. Pass
`"empress"`, `"priest"`, `"paladin"`, or `"dragon"` as the third argument to
`move` (or `play`) to choose the promoted piece. Without a choice the engine
promotes to an empress.

### `Game`

| Member              | Description                                                   |
| ------------------- | ------------------------------------------------------------- |
| `state()`           | `{ squares, turn, status, moveNumber, lastMove }`             |
| `legalMoves(from?)` | Legal moves, optionally only from one square                  |
| `move(from, to, promotion?)` | Play a move if legal; returns it or `null`           |
| `undo()`            | Take back the last move; returns the restored state or `null` |
| `reset()`           | Return to the starting position                               |
| `load(boardString)` | Load a 65-byte position string                                |
| `boardString()`     | Serialise the current position                                |
| `bestMove(depth?)`  | Bot move without playing it                                   |
| `playBest(depth?)`  | Bot move, played; includes score, depth, nodes                |
| `pieceAt(square)`   | Piece on a square                                             |
| `destroy()`         | Release the wasm-side game state                              |

`status` is `{ kind: "ongoing" | "checkmate" | "stalemate", winner, inCheck }`.

Bot scores are centipawns from the side to move, and mate scores are near
`±90000`; `formatScore` turns both into display strings.

The search is single-threaded in wasm. Depth 10 takes tens of milliseconds,
depth 12 a few hundred, depth 14 about a second. Above depth 10, run the game
inside a Web Worker so the page stays responsive.

## Online games

```ts
import { connectOnline } from "antarian-chess";

const client = await connectOnline({
    name: "you",
    wasmUrl: "/chess_wasm.wasm",
});

client.on("change", () => render(client));
client.on("message", (message) => console.log(message));

client.startMatchmaking();
// or: client.createLobby(true); client.joinLobby("ABC123");

// Once MatchFound arrives the client owns a Game in the starting position
// and sets gameId, opponentName, and color.
client.play("b1", "c3"); // validates, plays locally, sends MakeMove
client.legalMoves();
client.sendChat("good luck");
client.resign();
```

`heartbeat` defaults to true and sends `Ping` every 20 seconds. Pass
`heartbeat: false` when the server does not implement `Ping`.

The client keeps these fields in sync with the server:

- `status`: `idle`, `matchmaking`, `lobby`, `playing`, or `over`
- `game`: the current `Game`, reset on `MatchFound` and `RematchAccepted`
- `gameId`, `opponentName`, `color`
- `lobbies`, `lobbyCode`
- `rematchRequested`, `lastResult`, `lastReason`, `lastError`

Actions: `startMatchmaking`, `leaveMatchmaking`, `requestLobbyList`,
`createLobby(private?)`, `joinLobby(code)`, `leaveLobby`, `play`, `resign`,
`requestRematch`, `acceptRematch`, `declineRematch`, `sendChat`, `close`.
`send(message)` takes any generated `ClientMessage`.

`play` only applies a move when it is your turn and the move is legal, then
sends `MakeMove`. Opponent moves and rematches are applied to the local game
automatically.

The client sends a protocol-level `Ping` every 20 seconds so proxy read
timeouts do not close an idle connection. Browsers cannot send WebSocket ping
frames, which is why the protocol has `Ping`/`Pong`.

## Protocol generation

`src/protocol.ts` is generated from `crates/server/src/protocol.rs` with
`ts-rs`. Do not edit it by hand; run `npm run generate:protocol` (or
`npm run build`). Adding or changing a message in Rust is enough to update the
TypeScript types.

## Package layout

- `src/protocol.ts`: generated `ClientMessage` and `ServerMessage` unions
- `src/engine.ts`: `Engine` and `Game`, wrapping the wasm module
- `src/online.ts`: websocket client
- `dist/chess_wasm.wasm`: compiled bot
