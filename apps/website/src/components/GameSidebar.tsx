import { useEffect, useRef, useState } from "react";
import type { Color, PieceKind } from "antarian-chess";
import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import type { ChatLine } from "@/hooks/useOnlineGame";
import { pieceImage } from "@/lib/pieces";
import { cn } from "@/lib/utils";

export interface RematchControls {
  requestedByOpponent: boolean;
  requested: boolean;
  declined: boolean;
  onRequest: () => void;
  onAccept: () => void;
  onDecline: () => void;
}

export interface ChatControls {
  messages: ChatLine[];
  onSend: (text: string) => void;
}

interface GameSidebarProps {
  mode: string;
  you: string;
  opponent: string;
  moves: string;
  status: string;
  yourTurn: boolean;
  bot?: { score: string; depth: string };
  clocks?: { white: string; black: string; active: Color };
  chat?: ChatControls;
  rematch?: RematchControls;
  captures: { yours: PieceKind[]; theirs: PieceKind[]; yourColor: Color };
  onLeave: () => void;
}

export function GameSidebar({
  mode,
  you,
  opponent,
  moves,
  status,
  yourTurn,
  bot,
  clocks,
  chat,
  rematch,
  captures,
  onLeave,
}: GameSidebarProps) {
  return (
    <Card className="h-full py-0">
      <CardPanel className="flex h-full flex-col gap-2 overflow-y-auto p-2">
        <Detail label="Mode" value={mode} />
        <Detail label="You" value={you} />
        <Detail label="Opponent" value={opponent} />
        <Detail label="Moves" value={moves} />

        <Separator />

        <div
          className={cn(
            "rounded-md p-2 text-sm",
            yourTurn ? "bg-primary/25" : "bg-muted/40",
          )}
        >
          {status}
        </div>

        {rematch && <RematchPanel controls={rematch} />}

        {bot && (
          <>
            <Separator />
            <Detail label="Eval (White)" value={bot.score} />
            <Detail label="Search Depth" value={bot.depth} />
          </>
        )}

        {clocks && (
          <>
            <Separator />
            <ClockRow
              label="White"
              time={clocks.white}
              active={clocks.active === "white"}
            />
            <ClockRow
              label="Black"
              time={clocks.black}
              active={clocks.active === "black"}
            />
          </>
        )}

        {chat && (
          <>
            <Separator />
            <ChatPanel chat={chat} />
          </>
        )}

        <Separator />
        <LeaveButton onLeave={onLeave} />
        <Separator />

        <span className="text-sm text-muted-foreground">Yours:</span>
        <Captured
          pieces={captures.yours}
          color={captures.yourColor === "white" ? "black" : "white"}
          tone="own"
        />
        <span className="text-sm text-muted-foreground">Theirs:</span>
        <Captured
          pieces={captures.theirs}
          color={captures.yourColor}
          tone="opponent"
        />
      </CardPanel>
    </Card>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex w-full items-center justify-between gap-2 px-2">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-sm">{value}</span>
    </div>
  );
}

function ClockRow({
  label,
  time,
  active,
}: {
  label: string;
  time: string;
  active: boolean;
}) {
  return (
    <div className="flex w-full items-center justify-between rounded-md px-2 py-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span
        className={cn(
          "font-mono text-sm tabular-nums",
          active ? "text-primary" : "text-foreground",
        )}
      >
        {time}
      </span>
    </div>
  );
}

function Captured({
  pieces,
  color,
  tone,
}: {
  pieces: PieceKind[];
  color: Color;
  tone: "own" | "opponent";
}) {
  return (
    <div className="flex flex-wrap gap-2 px-2">
      {pieces.map((kind, index) => (
        <span
          key={`${kind}-${index}`}
          className={cn(
            "flex size-7 items-center justify-center rounded-sm",
            tone === "own" ? "bg-primary/15" : "bg-destructive/15",
          )}
        >
          <img
            src={pieceImage({ color, kind })}
            alt=""
            className="size-6 object-contain"
          />
        </span>
      ))}
    </div>
  );
}

function RematchPanel({ controls }: { controls: RematchControls }) {
  return (
    <div className="flex flex-col gap-2 rounded-md border border-border/60 p-2">
      <span className="text-sm">Rematch</span>
      {controls.requestedByOpponent ? (
        <>
          <span className="text-sm text-muted-foreground">
            Your opponent wants a rematch.
          </span>
          <div className="flex gap-2">
            <Button className="flex-1" onClick={controls.onAccept}>
              Accept
            </Button>
            <Button
              className="flex-1"
              variant="outline"
              onClick={controls.onDecline}
            >
              Decline
            </Button>
          </div>
        </>
      ) : controls.requested ? (
        <span className="text-sm text-muted-foreground">
          Rematch request sent. Waiting for your opponent.
        </span>
      ) : controls.declined ? (
        <span className="text-sm text-muted-foreground">Rematch declined.</span>
      ) : (
        <>
          <span className="text-sm text-muted-foreground">
            Want another game with the same opponent?
          </span>
          <Button onClick={controls.onRequest}>Request Rematch</Button>
        </>
      )}
    </div>
  );
}

function ChatPanel({ chat }: { chat: ChatControls }) {
  const [text, setText] = useState("");
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [chat.messages.length]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <span className="text-sm text-muted-foreground">Match Chat</span>
      <ScrollArea className="min-h-24 flex-1 rounded-md bg-muted/15">
        <div className="flex flex-col gap-1.5 p-2 text-sm">
          {chat.messages.length === 0 ? (
            <span className="text-muted-foreground">No messages yet.</span>
          ) : (
            chat.messages.map((line, index) => (
              <p key={index} className="break-words">
                {line.system ? (
                  <span className="text-muted-foreground">{line.text}</span>
                ) : (
                  <>
                    <span className="text-primary">{line.from}:</span>{" "}
                    <span>{line.text}</span>
                  </>
                )}
              </p>
            ))
          )}
          <div ref={bottomRef} />
        </div>
      </ScrollArea>
      <form
        className="flex items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          const body = text.trim();
          if (!body) {
            return;
          }
          chat.onSend(body);
          setText("");
        }}
      >
        <Input
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder="Message opponent"
          maxLength={280}
          className="flex-1"
        />
        <Button type="submit" disabled={text.trim().length === 0}>
          Send
        </Button>
      </form>
    </div>
  );
}

function LeaveButton({ onLeave }: { onLeave: () => void }) {
  return (
    <AlertDialog>
      <AlertDialogTrigger render={<Button variant="outline" className="w-full" />}>
        Leave
      </AlertDialogTrigger>
      <AlertDialogPopup>
        <AlertDialogHeader>
          <AlertDialogTitle>Leave game?</AlertDialogTitle>
          <AlertDialogDescription>
            This will leave the current match and disconnect you from online
            play.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogClose render={<Button variant="ghost" />}>
            Stay
          </AlertDialogClose>
          <AlertDialogClose
            render={<Button variant="destructive" onClick={onLeave} />}
          >
            Leave
          </AlertDialogClose>
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
