import { useEffect, useRef, useState } from "react";
import type { Color, PieceKind } from "antarian-chess";
import { ChevronLeft, ChevronRight, LogOut, MessageSquare } from "lucide-react";
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
import {
  Drawer,
  DrawerHeader,
  DrawerPopup,
  DrawerTitle,
} from "@/components/ui/drawer";
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
  history?: {
    canBack: boolean;
    canForward: boolean;
    onBack: () => void;
    onForward: () => void;
  };
  onLeave: () => void;
}

export function GameSidebar(props: GameSidebarProps) {
  const [chatOpen, setChatOpen] = useState(false);

  return (
    <>
      <MobileBar
        {...props}
        onOpenChat={props.chat ? () => setChatOpen(true) : undefined}
      />

      <Card className="hidden h-full min-h-0 py-0 lg:flex">
        <CardPanel className="flex h-full min-h-0 flex-col gap-2 overflow-y-auto p-2">
          <Details {...props} />
        </CardPanel>
      </Card>

      {props.chat ? (
        <Drawer open={chatOpen} onOpenChange={setChatOpen} position="bottom">
          <DrawerPopup
            className="h-[min(70dvh,34rem)]"
            showBar
            showCloseButton
          >
            <DrawerHeader className="px-4 pb-2">
              <DrawerTitle>Match chat</DrawerTitle>
            </DrawerHeader>
            <div className="flex min-h-0 flex-1 flex-col px-4 pb-4">
              <ChatPanel chat={props.chat} showLabel={false} />
            </div>
          </DrawerPopup>
        </Drawer>
      ) : null}
    </>
  );
}

function Details({
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
    <>
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
    </>
  );
}

function MobileBar({
  clocks,
  rematch,
  captures,
  history,
  onLeave,
  onOpenChat,
}: GameSidebarProps & { onOpenChat?: () => void }) {
  return (
    <div className="flex shrink-0 flex-col gap-1.5 rounded-xl border border-border/70 bg-card p-1.5 lg:hidden">
      {rematch ? <RematchPanel controls={rematch} /> : null}

      <div className="flex min-w-0 items-center gap-1">
        {history ? (
          <>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label="Previous move"
              disabled={!history.canBack}
              onClick={history.onBack}
            >
              <ChevronLeft aria-hidden="true" />
            </Button>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label="Next move"
              disabled={!history.canForward}
              onClick={history.onForward}
            >
              <ChevronRight aria-hidden="true" />
            </Button>
          </>
        ) : null}

        {clocks ? (
          <div className="flex shrink-0 items-center gap-1">
            <ClockChip
              label="W"
              time={clocks.white}
              active={clocks.active === "white"}
            />
            <ClockChip
              label="B"
              time={clocks.black}
              active={clocks.active === "black"}
            />
          </div>
        ) : null}

        <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden px-1">
          <CapturedInline
            pieces={captures.yours}
            color={captures.yourColor === "white" ? "black" : "white"}
          />
          {captures.yours.length > 0 && captures.theirs.length > 0 ? (
            <span className="h-3 w-px shrink-0 bg-border" />
          ) : null}
          <CapturedInline
            pieces={captures.theirs}
            color={captures.yourColor}
          />
        </div>

        {onOpenChat ? (
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Open match chat"
            onClick={onOpenChat}
          >
            <MessageSquare />
          </Button>
        ) : null}
        <LeaveButton onLeave={onLeave} compact />
      </div>
    </div>
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

function ClockChip({
  label,
  time,
  active,
}: {
  label: string;
  time: string;
  active: boolean;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-baseline gap-1 rounded-md px-1.5 py-1",
        active
          ? "bg-primary/20 text-foreground"
          : "bg-muted/40 text-muted-foreground",
      )}
    >
      <span className="text-[0.625rem] font-medium uppercase">{label}</span>
      <span className="font-mono text-xs tabular-nums">{time}</span>
    </span>
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

function CapturedInline({
  pieces,
  color,
}: {
  pieces: PieceKind[];
  color: Color;
}) {
  if (pieces.length === 0) {
    return null;
  }

  return (
    <span className="flex min-w-0 items-center gap-0.5 overflow-hidden">
      {pieces.map((kind, index) => (
        <img
          key={`${kind}-${index}`}
          src={pieceImage({ color, kind })}
          alt=""
          className="size-4 shrink-0 object-contain"
        />
      ))}
    </span>
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

function ChatPanel({
  chat,
  showLabel = true,
}: {
  chat: ChatControls;
  showLabel?: boolean;
}) {
  const [text, setText] = useState("");
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [chat.messages.length]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {showLabel ? (
        <span className="text-sm text-muted-foreground">Match Chat</span>
      ) : null}
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

function LeaveButton({
  onLeave,
  compact = false,
}: {
  onLeave: () => void;
  compact?: boolean;
}) {
  return (
    <AlertDialog>
      <AlertDialogTrigger
        render={
          compact ? (
            <Button size="icon-sm" variant="ghost" aria-label="Leave game" />
          ) : (
            <Button variant="outline" className="w-full" />
          )
        }
      >
        {compact ? <LogOut aria-hidden="true" /> : "Leave"}
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
