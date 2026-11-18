import { useEffect, useState } from "react";
import {
  CircleCheckIcon,
  CircleXIcon,
  HandshakeIcon,
  WifiOffIcon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPopup,
  DialogTitle,
} from "@/components/ui/dialog";
import type { GameResult, GameResultTone } from "@/lib/gameResult";
import { cn } from "@/lib/utils";

const ICONS: Record<GameResultTone, LucideIcon> = {
  win: CircleCheckIcon,
  loss: CircleXIcon,
  draw: HandshakeIcon,
  neutral: WifiOffIcon,
};

const TONE_CLASSES: Record<GameResultTone, string> = {
  win: "bg-success/15 text-success-foreground",
  loss: "bg-destructive/15 text-destructive-foreground",
  draw: "bg-muted text-muted-foreground",
  neutral: "bg-info/15 text-info-foreground",
};

export function GameResultDialog({
  result,
  onLeave,
}: {
  result: GameResult | null;
  onLeave: () => void;
}) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    setOpen(result !== null);
  }, [result?.id]);

  if (!result) {
    return null;
  }

  const Icon = ICONS[result.tone];

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogPopup className="max-w-sm">
        <DialogHeader className="items-center pb-4 text-center">
          <div
            className={cn(
              "mb-1 flex size-12 items-center justify-center rounded-full",
              TONE_CLASSES[result.tone],
            )}
          >
            <Icon aria-hidden="true" className="size-6" />
          </div>
          <DialogTitle className="text-2xl">{result.title}</DialogTitle>
          <DialogDescription>{result.description}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2 px-6 pb-6">
          <Button onClick={() => setOpen(false)}>Review board</Button>
          <Button
            variant="outline"
            onClick={() => {
              setOpen(false);
              onLeave();
            }}
          >
            Back to menu
          </Button>
        </div>
      </DialogPopup>
    </Dialog>
  );
}
