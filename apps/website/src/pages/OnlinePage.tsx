import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import type { Color } from "antarian-chess";
import { Check, ClipboardCopy } from "lucide-react";
import { BoardArea } from "@/components/BoardArea";
import { GameResultDialog } from "@/components/GameResultDialog";
import { GameShell } from "@/components/GameShell";
import { GameSidebar } from "@/components/GameSidebar";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardHeader,
  CardPanel,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { useHistoryKeys } from "@/hooks/useHistoryKeys";
import { useOnlineGame } from "@/hooks/useOnlineGame";
import { capturedPieces, formatClock } from "@/lib/engine";
import { onlineGameResult } from "@/lib/gameResult";
import { loadName, saveName } from "@/lib/name";

type Intent = "random" | "browse" | "public" | "private" | "join";

function parseIntent(value: string | null): Intent {
  if (value === "random" || value === "public" || value === "private") {
    return value;
  }
  return "browse";
}

const TITLES: Record<Intent, string> = {
  random: "Random Matchmaking",
  browse: "Public Lobby Browser",
  public: "Public Lobby",
  private: "Private Lobby",
  join: "Join Lobby",
};

export function OnlinePage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [name, setName] = useState(loadName);
  const [intent, setIntent] = useState<Intent>(() =>
    parseIntent(params.get("intent")),
  );
  const [joiningCode, setJoiningCode] = useState<string | null>(null);
  const [lobbyMessage, setLobbyMessage] = useState("");
  const game = useOnlineGame();

  const connectStarted = useRef(false);
  const intentSent = useRef(false);

  useEffect(() => {
    if (!name || game.connected || game.connecting || connectStarted.current) {
      return;
    }
    connectStarted.current = true;
    void game.connect(name);
  }, [game, name]);

  useEffect(() => {
    if (!game.connected || intentSent.current) {
      return;
    }
    intentSent.current = true;
    if (intent === "random") {
      game.startMatchmaking();
    } else if (intent === "browse") {
      game.refreshLobbies();
    } else if (intent === "public") {
      game.createLobby(false);
    } else if (intent === "private") {
      game.createLobby(true);
    }
  }, [
    game.connected,
    game.createLobby,
    game.refreshLobbies,
    game.startMatchmaking,
    intent,
  ]);

  const backToMenu = () => {
    game.leave();
    navigate("/");
  };

  const join = (code: string) => {
    const cleaned = code.trim().toUpperCase();
    if (!cleaned) {
      setLobbyMessage("Lobby code is required.");
      return;
    }
    setLobbyMessage("");
    setJoiningCode(cleaned);
    setIntent("join");
    game.joinLobby(cleaned);
  };

  if (!name) {
    return (
      <NameScreen
        onContinue={(value) => {
          saveName(value);
          setName(value);
        }}
      />
    );
  }

  if (!game.connected) {
    return (
      <CenterCard
        title={game.error ? "Connection failed" : TITLES[intent]}
        detail={game.error ?? "Connecting..."}
        onCancel={backToMenu}
        cancelLabel="Back"
        loading={!game.error}
      />
    );
  }

  if (game.inGame) {
    return <OnlineGame game={game} onLeave={backToMenu} />;
  }

  if (intent === "browse") {
    return (
      <LobbyBrowser
        lobbies={game.lobbies}
        message={
          lobbyMessage || (game.error ? `Server error: ${game.error}` : "")
        }
        onJoin={join}
        onBack={backToMenu}
      />
    );
  }

  const detail = game.error
    ? `Server error: ${game.error}`
    : intent === "random"
      ? game.status === "matchmaking"
        ? "Looking for a random opponent..."
        : "Connecting to matchmaking..."
      : intent === "public"
        ? game.lobbyCode
          ? "Public lobby created. Waiting for another player."
          : "Creating public lobby..."
        : intent === "private"
          ? game.lobbyCode
            ? "Private lobby created. Share the code below."
            : "Creating private lobby..."
          : joiningCode
            ? `Joining lobby ${joiningCode}...`
            : "Joining lobby...";

  return (
    <CenterCard
      title={TITLES[intent]}
      detail={detail}
      code={game.lobbyCode}
      onCancel={backToMenu}
      cancelLabel="Cancel"
      loading
    />
  );
}

function NameScreen({ onContinue }: { onContinue: (name: string) => void }) {
  const [name, setName] = useState(loadName);
  const trimmed = name.trim();

  return (
    <div className="flex h-dvh items-center justify-center overflow-hidden p-4">
      <Card className="max-h-full w-full max-w-[360px]">
        <CardHeader>
          <CardTitle>Player Name</CardTitle>
        </CardHeader>
        <CardPanel className="min-h-0 overflow-y-auto">
          <form
            className="flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              if (trimmed) {
                onContinue(trimmed);
              }
            }}
          >
            <Field>
              <FieldLabel className="sr-only">Player Name</FieldLabel>
              <Input
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={24}
                autoFocus
              />
            </Field>
            <Button type="submit" disabled={trimmed.length === 0}>
              Connect
            </Button>
          </form>
        </CardPanel>
      </Card>
    </div>
  );
}

function CenterCard({
  title,
  detail,
  code,
  onCancel,
  cancelLabel,
  loading,
}: {
  title: string;
  detail: string;
  code?: string | null;
  onCancel: () => void;
  cancelLabel: string;
  loading?: boolean;
}) {
  return (
    <div className="flex h-dvh items-center justify-center overflow-hidden p-4">
      <div className="flex w-full max-w-[320px] flex-col items-center gap-2 text-center">
        <span className="text-sm font-medium">{title}</span>
        <span className="text-sm text-muted-foreground">{detail}</span>
        {code ? <LobbyCode code={code} /> : null}
        <div className="flex items-center gap-2">
          {loading ? <Spinner className="size-4" aria-label="Waiting" /> : null}
          <Button variant="outline" onClick={onCancel}>
            {cancelLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}

function LobbyBrowser({
  lobbies,
  message,
  onJoin,
  onBack,
}: {
  lobbies: { lobby_code: string; host_name: string }[];
  message: string;
  onJoin: (code: string) => void;
  onBack: () => void;
}) {
  const [code, setCode] = useState("");
  const empty = code.trim().length === 0;

  return (
    <div className="flex h-dvh items-center justify-center overflow-hidden p-4">
      <Card className="max-h-full w-full max-w-[460px]">
        <CardPanel className="flex h-[min(520px,calc(100dvh-2.25rem))] min-h-0 flex-col gap-3 p-4">
          <span className="text-sm font-medium">Public Lobbies</span>
          <span className="text-sm text-muted-foreground">
            {message || (lobbies.length === 0 ? "No public lobbies yet." : "Select a public lobby to join.")}
          </span>

          <ScrollArea className="min-h-0 flex-1 rounded-md bg-muted/10">
            {lobbies.length === 0 ? (
              <div className="flex h-full items-center justify-center p-2 text-sm text-muted-foreground">
                No public lobbies available right now.
              </div>
            ) : (
              <ul className="flex flex-col gap-1 p-1">
                {lobbies.map((lobby) => (
                  <li
                    key={lobby.lobby_code}
                    className="flex items-center justify-between gap-2 rounded-md p-2 hover:bg-muted/18"
                  >
                    <div className="flex min-w-0 flex-col">
                      <span className="truncate text-sm">{lobby.host_name}</span>
                      <span className="text-sm text-muted-foreground">
                        {lobby.lobby_code}
                      </span>
                    </div>
                    <Button onClick={() => onJoin(lobby.lobby_code)}>Join</Button>
                  </li>
                ))}
              </ul>
            )}
          </ScrollArea>

          <Separator />
          <span className="text-sm text-muted-foreground">
            Private Lobby Code
          </span>
          <form
            className="flex items-center gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              onJoin(code);
            }}
          >
            <Input
              value={code}
              onChange={(event) => setCode(event.target.value.toUpperCase())}
              placeholder="Lobby code"
              maxLength={12}
              className="flex-1 font-mono"
            />
            <Button type="submit" disabled={empty}>
              Join Private
            </Button>
          </form>
          <Button variant="outline" className="w-full" onClick={onBack}>
            Back
          </Button>
        </CardPanel>
      </Card>
    </div>
  );
}

function LobbyCode({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="flex items-center justify-center gap-2">
      <span className="text-sm">{code}</span>
      <Button
        size="icon-sm"
        variant="ghost"
        aria-label="Copy lobby code"
        onClick={copy}
      >
        {copied ? (
          <Check aria-hidden="true" />
        ) : (
          <ClipboardCopy aria-hidden="true" />
        )}
      </Button>
    </div>
  );
}

function OnlineGame({
  game,
  onLeave,
}: {
  game: ReturnType<typeof useOnlineGame>;
  onLeave: () => void;
}) {
  useHistoryKeys({
    length: game.history.length,
    viewIndex: game.viewIndex,
    onChange: game.setView,
  });

  const { live, viewed } = game;
  if (!live || !viewed) {
    return (
      <div className="flex h-dvh items-center justify-center overflow-hidden">
        <Spinner aria-label="Starting game" />
      </div>
    );
  }

  const color: Color = game.color ?? "white";
  const opponentColor: Color = color === "white" ? "black" : "white";
  const turnLabel = live.turn === "white" ? "White" : "Black";
  const youLabel = color === "white" ? "White" : "Black";
  const check = live.status.inCheck ? " — check!" : "";
  const totalPlies = game.history.length - 1;
  const reviewing = game.isReviewing;
  const over = game.status === "over";

  let status: string;
  if (live.status.kind === "checkmate") {
    status =
      live.status.winner === color
        ? "Checkmate — you win!"
        : "Checkmate — you lose";
  } else if (live.status.kind === "stalemate") {
    status = "Stalemate — draw";
  } else if (over) {
    status =
      game.lastReason === "opponent disconnected"
        ? "Opponent disconnected"
        : game.lastResult === "win"
          ? "You win!"
          : game.lastResult === "loss"
            ? "You lose"
            : game.lastResult === "draw"
              ? "Draw"
              : `Game over: ${game.lastReason ?? "unknown"}`;
  } else if (reviewing) {
    status = `Viewing move ${game.viewIndex} of ${totalPlies}`;
  } else if (live.turn === color) {
    status = `Your turn (${turnLabel})${check}`;
  } else {
    status = `${turnLabel} to move${check}`;
  }

  const result = onlineGameResult({
    live,
    color,
    over,
    result: game.lastResult,
    reason: game.lastReason,
    flagged: game.flagged,
  });

  return (
    <>
      <GameShell
        board={
        <BoardArea
          state={viewed}
          orientation={color}
          selected={game.selected}
          targets={game.targets}
          interactive={game.interactive}
          onSelect={game.select}
          onMove={game.play}
          centipawns={game.material}
        />
      }
      sidebar={
        <GameSidebar
          mode="Online"
          you={youLabel}
          opponent={game.opponentName ?? "Opponent"}
          moves={reviewing ? `${game.viewIndex} / ${totalPlies}` : `${totalPlies}`}
          history={{
            canBack: game.viewIndex > 0,
            canForward: game.viewIndex < game.history.length - 1,
            onBack: () => game.setView(game.viewIndex - 1),
            onForward: () => game.setView(game.viewIndex + 1),
          }}
          status={status}
          yourTurn={game.interactive}
          clocks={{
            white: formatClock(game.clocks.white),
            black: formatClock(game.clocks.black),
            active: live.turn,
          }}
          chat={{
            messages: game.chat,
            onSend: game.sendChat,
          }}
          rematch={
            over && game.lastReason !== "opponent disconnected"
              ? {
                  requestedByOpponent: game.rematchRequested,
                  requested: game.rematchSent,
                  declined: game.rematchDeclined,
                  onRequest: game.requestRematch,
                  onAccept: game.acceptRematch,
                  onDecline: game.declineRematch,
                }
              : undefined
          }
          captures={{
            yours: capturedPieces(viewed.squares, opponentColor),
            theirs: capturedPieces(viewed.squares, color),
            yourColor: color,
          }}
          onLeave={onLeave}
        />
      }
      />
      <GameResultDialog result={result} onLeave={onLeave} />
    </>
  );
}
