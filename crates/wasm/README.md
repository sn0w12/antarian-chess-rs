# chess_wasm

The engine and bot compiled to WebAssembly.

For application code use the npm package in [`packages/chess`](../../packages/chess)
instead of calling the raw exports. It wraps this module with typed game,
move-generation, bot, and online APIs.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build -p chess_wasm --target wasm32-unknown-unknown --release
```

The artifact is `target/wasm32-unknown-unknown/release/chess_wasm.wasm`
(about 105 KB). A raw smoke test runs it under Node:

```bash
node crates/wasm/check.mjs
```

## Exports

Strings cross the boundary as a pointer and a byte length into the module's
linear memory. Results are UTF-8 JSON in a shared buffer.

```js
const { instance } = await WebAssembly.instantiate(bytes, {});
const wasm = instance.exports;
```

| Export | Purpose |
|---|---|
| `alloc(len)` / `dealloc(ptr, len)` | Pass a board string in |
| `result_ptr()` | Pointer to the JSON written by the last call |
| `game_create()` | New game, returns an integer handle |
| `game_destroy(id)` / `game_reset(id)` | Drop or reset a game |
| `game_load(id, ptr, len)` | Load a 65-byte board string |
| `game_state(id)` | Squares, turn, status, last move |
| `game_legal_moves(id, from)` | Legal moves, `from = 255` for all |
| `game_play(id, from, to)` | Play a move if legal |
| `game_undo(id)` | Take back the last move |
| `game_best_move(id, depth)` | Search without playing |
| `game_play_best(id, depth)` | Search and play |
| `bot_move(ptr, len, depth)` | Stateless search of a board string |

Board strings are 65 ASCII bytes: 64 squares from `a1` to `h8`, then the side
to move (`w` or `b`). Pieces are `E Q B P R N` for white, lowercase for black,
`.` for empty.

## Notes

- The search is single-threaded. WebAssembly threads need `SharedArrayBuffer`
  and cross-origin isolation headers, so this build avoids them. Depth 10 to
  12 takes tens to hundreds of milliseconds.
- One instance keeps one 4 MB transposition table for the lifetime of the
  page. Create one instance per page.
- Run searches in a Web Worker to keep the page responsive.
