"use client";

import type { ReactElement } from "react";
import { cn } from "@/lib/utils";
import { ChatBubble, type ChatBubbleProps } from "./chat-bubble";

export type ChatMessageBubbleProps = Omit<
  ChatBubbleProps,
  "align" | "variant" | "grouped" | "render"
> & {
  side: "incoming" | "outgoing";
  firstInGroup?: boolean;
  highlighted?: boolean;
};

export function ChatMessageBubble({
  side,
  firstInGroup = true,
  highlighted = false,
  children,
  className,
  ...props
}: ChatMessageBubbleProps): ReactElement {
  return (
    <ChatBubble
      {...props}
      align={side === "outgoing" ? "end" : "start"}
      variant={side === "outgoing" ? "default" : "muted"}
      grouped={!firstInGroup}
      data-slot="chat-message-bubble"
      className={cn(
        highlighted &&
          "ring-2 ring-foreground/25 ring-offset-2 ring-offset-background",
        className,
      )}
    >
      {children}
    </ChatBubble>
  );
}
