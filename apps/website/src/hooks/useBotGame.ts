import { useCallback, useEffect, useRef, useState } from "react";
import {
  toBoardString,
  type BotResult,
  type Color,
  type Game,
  type GameState,
} from "antarian-chess";
import { playGameStart, playMoveResult } from "@/lib/audio";
import { botThinkTime, createGame, materialBalance } from "@/lib/engine";

const INITIAL_CLOCK = 600;

export interface BotGameOptions {
  playerColor: Color;
}

interface BotResponse {
  id: number;
  result?: BotResult | null;
  error?: string;
}

export function useBotGame({ playerColor }: BotGameOptions) {
  const gameRef = useRef<Game | null>(null);
  const historyRef = useRef<GameState[]>([]);
  const viewRef = useRef(0);
  const workerRef = useRef<Worker | null>(null);
  const requestRef = useRef(0);
  const clocksRef = useRef({ white: INITIAL_CLOCK, black: INITIAL_CLOCK });

  const [live, setLive] = useState<GameState | null>(null);
  const [history, setHistory] = useState<GameState[]>([]);
  const [viewIndex, setViewIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [targets, setTargets] = useState<number[]>([]);
  const [thinking, setThinking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [clocks, setClocks] = useState({
    white: INITIAL_CLOCK,
    black: INITIAL_CLOCK,
  });
  const [flagged, setFlagged] = useState<Color | null>(null);
  const [botScore, setBotScore] = useState<number | null>(null);
  const [botDepth, setBotDepth] = useState<number | null>(null);

  const record = useCallback((next: GameState) => {
    const previous = historyRef.current;
    if (next.moveNumber === 0 || previous.length === 0) {
      historyRef.current = [next];
      viewRef.current = 0;
      if (next.moveNumber === 0) {
        playGameStart();
      }
    } else {
      const last = previous[previous.length - 1];
      if (next.moveNumber > last.moveNumber) {
        const following = viewRef.current === previous.length - 1;
        const updated = [...previous, next];
        historyRef.current = updated;
        if (following) {
          viewRef.current = updated.length - 1;
        }
        playMoveResult(next);
      } else {
        const updated = previous.slice();
        updated[updated.length - 1] = next;
        historyRef.current = updated;
      }
    }

    setHistory(historyRef.current);
    setViewIndex(viewRef.current);
    setLive(next);
  }, []);

  useEffect(() => {
    let cancelled = false;
    createGame()
      .then((game) => {
        if (cancelled) {
          game.destroy();
          return;
        }
        gameRef.current = game;
        record(game.state());
      })
      .catch((cause: unknown) => setError(String(cause)));

    return () => {
      cancelled = true;
      gameRef.current?.destroy();
      gameRef.current = null;
    };
  }, [record]);

  useEffect(() => {
    const worker = new Worker(
      new URL("../workers/bot.worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    return () => {
      worker.terminate();
      workerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const worker = workerRef.current;
    if (!worker) {
      return;
    }

    const onMessage = (event: MessageEvent<BotResponse>) => {
      const { id, result, error: message } = event.data;
      if (id !== requestRef.current) {
        return;
      }
      setThinking(false);
      if (message) {
        setError(message);
        return;
      }
      const game = gameRef.current;
      if (!result || !game) {
        return;
      }
      const applied = game.move(result.move.from, result.move.to);
      if (!applied) {
        setError("Bot returned an illegal move");
        return;
      }
      setBotScore(result.score * (playerColor === "white" ? -1 : 1));
      setBotDepth(result.depth);
      record(game.state());
    };

    worker.addEventListener("message", onMessage);
    return () => worker.removeEventListener("message", onMessage);
  }, [playerColor, record]);

  const setView = useCallback((index: number) => {
    const clamped = Math.max(0, Math.min(index, historyRef.current.length - 1));
    viewRef.current = clamped;
    setViewIndex(clamped);
    setSelected(null);
    setTargets([]);
  }, []);

  const interactive =
    live !== null &&
    viewRef.current === historyRef.current.length - 1 &&
    live.status.kind === "ongoing" &&
    flagged === null &&
    !thinking &&
    live.turn === playerColor;

  const play = useCallback(
    (from: number, to: number) => {
      const game = gameRef.current;
      if (!game || !interactive) {
        return false;
      }
      const move = game.move(from, to);
      if (!move) {
        return false;
      }
      setSelected(null);
      setTargets([]);
      record(game.state());
      return true;
    },
    [interactive, record],
  );

  const select = useCallback(
    (square: number) => {
      const game = gameRef.current;
      if (!game || !interactive) {
        return;
      }
      const current = game.state();
      if (selected !== null && targets.includes(square)) {
        play(selected, square);
        return;
      }
      if (selected === square) {
        setSelected(null);
        setTargets([]);
        return;
      }
      const piece = current.squares[square];
      if (piece && piece.color === current.turn && piece.color === playerColor) {
        setSelected(square);
        setTargets(game.legalMoves(square).map((move) => move.to));
      } else {
        setSelected(null);
        setTargets([]);
      }
    },
    [interactive, play, playerColor, selected, targets],
  );

  useEffect(() => {
    if (!live || live.status.kind !== "ongoing" || flagged) {
      return;
    }
    if (live.turn === playerColor) {
      return;
    }
    const game = gameRef.current;
    const worker = workerRef.current;
    if (!game || !worker) {
      return;
    }

    const id = requestRef.current + 1;
    requestRef.current = id;
    setThinking(true);

    const remaining = clocksRef.current[live.turn];
    const thinkMs = Math.max(
      50,
      botThinkTime(remaining, live.moveNumber, game.legalMoves().length) * 1000,
    );
    worker.postMessage({
      id,
      board: toBoardString(live.squares, live.turn),
      thinkMs,
    });

    return () => {
      if (requestRef.current === id) {
        requestRef.current += 1;
      }
      setThinking(false);
    };
  }, [flagged, live, playerColor]);

  useEffect(() => {
    if (!live || live.status.kind !== "ongoing" || flagged) {
      return;
    }
    const turn = live.turn;
    const timer = window.setInterval(() => {
      setClocks((previous) => {
        if (previous[turn] <= 0) {
          return previous;
        }
        const next = { ...previous, [turn]: Math.max(0, previous[turn] - 0.1) };
        clocksRef.current = next;
        return next;
      });
    }, 100);
    return () => window.clearInterval(timer);
  }, [flagged, live, live?.status.kind, live?.turn]);

  useEffect(() => {
    if (!live || live.status.kind !== "ongoing" || flagged) {
      return;
    }
    if (clocks.white <= 0) {
      setFlagged("white");
    } else if (clocks.black <= 0) {
      setFlagged("black");
    }
  }, [clocks, flagged, live, live?.status.kind]);

  const viewed = history[viewIndex] ?? live;

  return {
    ready: live !== null,
    error,
    live,
    viewed,
    history,
    viewIndex,
    setView,
    selected,
    targets,
    select,
    play,
    thinking,
    clocks,
    flagged,
    botScore,
    botDepth,
    interactive,
    material: viewed ? materialBalance(viewed.squares) : 0,
    isReviewing: viewIndex < history.length - 1,
  };
}
