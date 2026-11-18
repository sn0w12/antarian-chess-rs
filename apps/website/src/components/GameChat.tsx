import { useState } from "react";
import { MessageSquareIcon } from "lucide-react";
import { ChatConversation } from "@/components/chat/chat-conversation";
import { useChatScroll } from "@/components/chat/hooks/use-chat-scroll";
import { ChatMessage } from "@/components/chat/chat-message";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ChatLine } from "@/hooks/useOnlineGame";
import { cn } from "@/lib/utils";

const MAX_MESSAGE_LENGTH = 280;

export function GameChat({
  messages,
  onSend,
  active = true,
  showLabel = false,
  className,
}: {
  messages: ChatLine[];
  onSend: (text: string) => void;
  active?: boolean;
  showLabel?: boolean;
  className?: string;
}) {
  const [draft, setDraft] = useState("");
  const scroll = useChatScroll({ active, itemCount: messages.length });

  return (
    <section className={cn("flex min-h-0 flex-col", className)}>
      {showLabel ? (
        <span className="px-0 text-sm text-muted-foreground">Match Chat</span>
      ) : null}

      <ChatConversation
        scroll={scroll}
        label="Match chat"
        jumpLabel="Jump to latest"
        className="min-h-0 flex-1"
        contentClassName="px-0 pb-2"
      >
        {messages.length === 0 ? (
          <div className="flex h-full min-h-32 flex-col items-center justify-center gap-2 px-6 text-center text-muted-foreground">
            <MessageSquareIcon aria-hidden="true" className="size-5" />
            <p className="text-sm">No messages yet. Say hello.</p>
          </div>
        ) : (
          <div className="flex flex-col gap-3 pt-2">
            {messages.map((line, index) => {
              if (line.system) {
                return (
                  <p
                    key={index}
                    className="text-center text-muted-foreground text-xs"
                  >
                    {line.text}
                  </p>
                );
              }
              const own = line.own === true;
              const previous = messages[index - 1];
              const grouped = Boolean(
                previous && !previous.system && previous.from === line.from,
              );
              return (
                <ChatMessage
                  key={index}
                  side={own ? "outgoing" : "incoming"}
                  author={line.from}
                  grouped={grouped}
                >
                  {line.text}
                </ChatMessage>
              );
            })}
          </div>
        )}
      </ChatConversation>

      <form
        className="flex shrink-0 items-center gap-2 px-0 pt-2"
        onSubmit={(event) => {
          event.preventDefault();
          const text = draft.trim().slice(0, MAX_MESSAGE_LENGTH);
          if (!text) {
            return;
          }
          scroll.atBottomRef.current = true;
          onSend(text);
          setDraft("");
        }}
      >
        <Input
          aria-label="Message opponent"
          className="flex-1"
          maxLength={MAX_MESSAGE_LENGTH}
          placeholder="Message opponent"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
        />
        <Button type="submit" disabled={draft.trim().length === 0}>
          Send
        </Button>
      </form>
    </section>
  );
}
