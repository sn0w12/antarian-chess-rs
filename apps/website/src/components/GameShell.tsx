import type { ReactNode } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";

export function GameShell({
  board,
  sidebar,
}: {
  board: ReactNode;
  sidebar: ReactNode;
}) {
  return (
    <TooltipProvider>
      <div className="flex min-h-screen flex-col gap-2 p-2 lg:h-screen lg:flex-row lg:justify-center">
        <div className="flex min-h-0 w-full justify-center lg:w-[min(calc(100vh+0.25rem),calc(100vw-22rem))] lg:shrink-0">
          {board}
        </div>
        <aside className="flex min-h-0 w-full flex-col lg:w-[320px] lg:shrink-0">
          {sidebar}
        </aside>
      </div>
    </TooltipProvider>
  );
}
