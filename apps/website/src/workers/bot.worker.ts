import { loadEngine, type BotResult } from "antarian-chess";

interface BotRequest {
  id: number;
  board: string;
  thinkMs: number;
}

const ctx = self as unknown as {
  postMessage(value: unknown): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<BotRequest>) => void,
  ): void;
};

let enginePromise: ReturnType<typeof loadEngine> | undefined;

ctx.addEventListener("message", async (event) => {
  const { id, board, thinkMs } = event.data;

  let engine: Awaited<ReturnType<typeof loadEngine>>;
  try {
    enginePromise ??= loadEngine({ wasmUrl: "/chess_wasm.wasm" });
    engine = await enginePromise;
  } catch (error) {
    ctx.postMessage({ id, error: String(error) });
    return;
  }

  const start = performance.now();
  let best: BotResult | null = null;
  let depth = 4;

  while (depth <= 24) {
    const before = performance.now();
    const result = engine.bestMoveFor(board, depth);
    if (!result) {
      break;
    }
    best = result;

    const elapsed = performance.now() - before;
    const used = performance.now() - start;
    if (used >= thinkMs) {
      break;
    }
    if (used + Math.max(elapsed * 2, 12) > thinkMs) {
      break;
    }
    depth += 1;
  }

  ctx.postMessage({ id, result: best });
});
