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
      <div className="flex h-dvh flex-col gap-1.5 overflow-hidden pt-[max(0.375rem,env(safe-area-inset-top))] pr-[max(0.375rem,env(safe-area-inset-right))] pb-[max(0.375rem,env(safe-area-inset-bottom))] pl-[max(0.375rem,env(safe-area-inset-left))] lg:h-screen lg:flex-row lg:justify-center lg:gap-2 lg:p-2">
        <div className="flex min-h-0 w-full flex-1 items-center justify-center lg:w-[min(calc(100vh+0.25rem),calc(100vw-22rem))] lg:flex-none lg:shrink-0">
          {board}
        </div>
        <aside className="flex min-h-0 w-full shrink-0 flex-col lg:w-[320px]">
          {sidebar}
        </aside>
      </div>
    </TooltipProvider>
  );
}
