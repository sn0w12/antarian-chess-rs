import type { Color, GameState } from "antarian-chess";
import { Board } from "./Board";
import { EvalBar } from "./EvalBar";

interface BoardAreaProps {
  state: GameState;
  orientation: Color;
  selected: number | null;
  targets: number[];
  interactive: boolean;
  onSelect: (square: number) => void;
  onMove: (from: number, to: number) => boolean;
  centipawns: number;
}

export function BoardArea({
  state,
  orientation,
  selected,
  targets,
  interactive,
  onSelect,
  onMove,
  centipawns,
}: BoardAreaProps) {
  return (
    <div className="flex h-full w-full min-h-0 items-center justify-center [container-type:size]">
      <div className="flex w-[min(100cqw,calc(100cqh_-_0.75rem))] flex-col items-stretch gap-1.5 lg:h-full lg:w-full lg:flex-row lg:gap-2">
        <EvalBar centipawns={centipawns} orientation={orientation} />
        <div className="aspect-square min-w-0 flex-1">
          <Board
            state={state}
            orientation={orientation}
            selected={selected}
            targets={targets}
            interactive={interactive}
            onSelect={onSelect}
            onMove={onMove}
          />
        </div>
      </div>
    </div>
  );
}
