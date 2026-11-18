import { evalFraction, type Color } from "@/lib/engine";

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
      className="relative w-3 shrink-0 self-stretch overflow-hidden rounded-full border border-border bg-[#141414]"
      role="img"
      aria-label={`Evaluation ${centipawns >= 0 ? "plus" : "minus"} ${Math.abs(centipawns) / 100} pawns`}
    >
      <div
        className="absolute inset-x-0 bg-[#f3efe4] transition-[height] duration-500"
        style={
          orientation === "white"
            ? { bottom: 0, height: `${share}%` }
            : { top: 0, height: `${share}%` }
        }
      />
    </div>
  );
}
