import { useCallback, useEffect, useRef, useState } from "react";
import {
  connectOnline,
  type GameState,
  type OnlineClient,
  type OnlineStatus,
} from "antarian-chess";
import { materialBalance } from "@/lib/engine";
import { playGameStart, playMove, playMoveResult } from "@/lib/audio";

export interface ChatLine {
  from: string;
  text: string;
  system?: boolean;
}

export function useOnlineGame() {
  const clientRef = useRef<OnlineClient | null>(null);
  const historyRef = useRef<GameState[]>([]);
  const viewRef = useRef(0);
  const selfNameRef = useRef("");

  const [client, setClient] = useState<OnlineClient | null>(null);
  const [version, setVersion] = useState(0);
  const [live, setLive] = useState<GameState | null>(null);
  const [history, setHistory] = useState<GameState[]>([]);
  const [viewIndex, setViewIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [targets, setTargets] = useState<number[]>([]);
  const [chat, setChat] = useState<ChatLine[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [rematchSent, setRematchSent] = useState(false);
  const [rematchDeclined, setRematchDeclined] = useState(false);
  const [inGame, setInGame] = useState(false);

  const activeClient = useCallback((): OnlineClient | null => {
    const current = clientRef.current;
    return current && !current.isClosed ? current : null;
  }, []);

  const sync = useCallback(() => {
    const current = clientRef.current;
    if (!current) {
      return;
    }
    const next = current.game.state();
    const previous = historyRef.current;

    if (next.moveNumber === 0 || previous.length === 0) {
      historyRef.current = [next];
      viewRef.current = 0;
    } else {
      const last = previous[previous.length - 1];
      if (next.moveNumber > last.moveNumber) {
        const updated = [...previous, next];
        historyRef.current = updated;
        viewRef.current = updated.length - 1;
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

  const connect = useCallback(
    async (name: string, url?: string) => {
      if (clientRef.current) {
        return;
      }
      setConnecting(true);
      setError(null);
      try {
        const created = await connectOnline({
          name,
          url: url?.trim() || undefined,
          wasmUrl: "/chess_wasm.wasm",
          // The live server rejects Ping until it runs the current protocol.
          heartbeat: false,
        });
        selfNameRef.current = name;
        clientRef.current = created;
        created.on("change", () => {
          if (created.isClosed && clientRef.current === created) {
            clientRef.current = null;
            setClient(null);
            setError("Connection closed.");
            return;
          }
          setVersion((value) => value + 1);
        });
        created.on("error", (message) => {
          setError(message);
          setChat((previous) => [
            ...previous,
            { from: "", text: `Server error: ${message}`, system: true },
          ]);
        });
        created.on("message", (message) => {
          if (message.type === "MatchFound" || message.type === "RematchAccepted") {
            playGameStart();
          }
          if (
            message.type === "ChatMessage" &&
            message.data.sender_name !== selfNameRef.current &&
            message.data.game_id === clientRef.current?.gameId
          ) {
            setChat((previous) => [
              ...previous,
              { from: message.data.sender_name, text: message.data.message },
            ]);
          } else if (message.type === "RematchRequested") {
            setRematchDeclined(false);
            setChat((previous) => [
              ...previous,
              { from: "", text: "Opponent requested a rematch.", system: true },
            ]);
          } else if (message.type === "RematchAccepted") {
            setChat((previous) => [
              ...previous,
              { from: "", text: "Rematch started.", system: true },
            ]);
          } else if (message.type === "RematchDeclined") {
            setRematchDeclined(true);
            setRematchSent(false);
            setChat((previous) => [
              ...previous,
              { from: "", text: "Rematch declined.", system: true },
            ]);
          }
        });
        historyRef.current = [];
        viewRef.current = 0;
        setChat([]);
        setInGame(false);
        setRematchSent(false);
        setRematchDeclined(false);
        sync();
        setClient(created);
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setConnecting(false);
      }
    },
    [sync],
  );

  useEffect(() => {
    if (version === 0) {
      return;
    }
    sync();
    const current = clientRef.current;
    if (current?.status === "playing") {
      setRematchSent(false);
      setRematchDeclined(false);
      setInGame(true);
    }
  }, [sync, version]);

  useEffect(() => {
    return () => {
      const current = clientRef.current;
      clientRef.current = null;
      current?.close();
    };
  }, []);

  const setView = useCallback((index: number) => {
    const clamped = Math.max(0, Math.min(index, historyRef.current.length - 1));
    if (clamped !== viewRef.current) {
      playMove();
    }
    viewRef.current = clamped;
    setViewIndex(clamped);
    setSelected(null);
    setTargets([]);
  }, []);

  const interactive =
    client !== null &&
    !client.isClosed &&
    client.status === "playing" &&
    live !== null &&
    live.status.kind === "ongoing" &&
    viewRef.current === historyRef.current.length - 1 &&
    (client.color === null || live.turn === client.color);

  const play = useCallback(
    (from: number, to: number) => {
      const current = activeClient();
      if (!current || !interactive) {
        return false;
      }
      const move = current.play(from, to);
      if (!move) {
        return false;
      }
      setSelected(null);
      setTargets([]);
      return true;
    },
    [activeClient, interactive],
  );

  const select = useCallback(
    (square: number) => {
      const current = activeClient();
      if (!current || !interactive) {
        return;
      }
      const state = current.game.state();
      if (selected !== null && targets.includes(square)) {
        play(selected, square);
        return;
      }
      if (selected === square) {
        setSelected(null);
        setTargets([]);
        return;
      }
      const piece = state.squares[square];
      if (
        piece &&
        piece.color === state.turn &&
        (current.color === null || piece.color === current.color)
      ) {
        setSelected(square);
        setTargets(current.legalMoves(square).map((move) => move.to));
      } else {
        setSelected(null);
        setTargets([]);
      }
    },
    [activeClient, interactive, play, selected, targets],
  );

  const sendChat = useCallback(
    (text: string) => {
      const current = activeClient();
      const body = text.trim();
      if (!current || !body) {
        return;
      }
      current.sendChat(body);
      setChat((previous) => [...previous, { from: "You", text: body }]);
    },
    [activeClient],
  );

  const startMatchmaking = useCallback(() => {
    activeClient()?.startMatchmaking();
  }, [activeClient]);

  const refreshLobbies = useCallback(() => {
    activeClient()?.requestLobbyList();
  }, [activeClient]);

  const createLobby = useCallback(
    (isPrivate: boolean) => {
      activeClient()?.createLobby(isPrivate);
    },
    [activeClient],
  );

  const joinLobby = useCallback(
    (code: string) => {
      const clean = code.trim().toUpperCase();
      if (clean) {
        activeClient()?.joinLobby(clean);
      }
    },
    [activeClient],
  );

  const leave = useCallback(() => {
    const current = clientRef.current;
    clientRef.current = null;
    current?.close("left the website");
    historyRef.current = [];
    viewRef.current = 0;
    setClient(null);
    setLive(null);
    setHistory([]);
    setViewIndex(0);
    setChat([]);
    setSelected(null);
    setTargets([]);
    setRematchSent(false);
    setRematchDeclined(false);
    setInGame(false);
  }, []);

  const requestRematch = useCallback(() => {
    activeClient()?.requestRematch();
    setRematchSent(true);
    setRematchDeclined(false);
  }, [activeClient]);

  const acceptRematch = useCallback(() => {
    activeClient()?.acceptRematch();
    setRematchSent(false);
    setRematchDeclined(false);
  }, [activeClient]);

  const declineRematch = useCallback(() => {
    activeClient()?.declineRematch();
    setRematchSent(false);
  }, [activeClient]);

  const status: OnlineStatus | "disconnected" = client?.status ?? "disconnected";
  const viewed = history[viewIndex] ?? live;

  return {
    connected: client !== null,
    connecting,
    error,
    status,
    client,
    connect,
    inGame,
    live,
    viewed,
    history,
    viewIndex,
    setView,
    selected,
    targets,
    select,
    play,
    interactive,
    chat,
    sendChat,
    startMatchmaking,
    refreshLobbies,
    createLobby,
    joinLobby,
    leave,
    requestRematch,
    acceptRematch,
    declineRematch,
    rematchSent,
    rematchDeclined,
    rematchRequested: client?.rematchRequested ?? false,
    lobbyCode: client?.lobbyCode ?? null,
    lobbies: client?.lobbies ?? [],
    opponentName: client?.opponentName ?? null,
    color: client?.color ?? null,
    gameId: client?.gameId ?? null,
    playerId: client?.playerId ?? null,
    lastResult: client?.lastResult ?? null,
    lastReason: client?.lastReason ?? null,
    material: viewed ? materialBalance(viewed.squares) : 0,
    goStart: () => setView(0),
    goBack: () => setView(viewIndex - 1),
    goForward: () => setView(viewIndex + 1),
    goEnd: () => setView(history.length - 1),
    isReviewing: viewIndex < history.length - 1,
  };
}
