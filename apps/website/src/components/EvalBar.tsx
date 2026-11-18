import { evalFraction, type Color } from "@/lib/engine";
import { cn } from "@/lib/utils";

export function EvalBar({
    centipawns,
    orientation,
}: {
    centipawns: number;
    orientation: Color;
}) {
    const share = Math.round(evalFraction(centipawns) * 1000) / 10;

    return (
        <div
            className="relative h-2 md:w-3 md:h-full shrink-0 self-stretch overflow-hidden rounded-full border border-border bg-[#141414]"
            role="img"
            aria-label={`Evaluation ${centipawns >= 0 ? "plus" : "minus"} ${Math.abs(centipawns) / 100} pawns`}
        >
            <div
                className={cn(
                    "absolute inset-x-0 bg-[#f3efe4] transition-[height,width] duration-500 w-(--share) h-full md:h-(--share) md:w-full",
                    orientation === "white"
                        ? "left-0 md:bottom-0"
                        : "right-0 md:top-0"
                )}
                style={{ "--share": `${share}%` } as React.CSSProperties}
            />
        </div>
    );
}
