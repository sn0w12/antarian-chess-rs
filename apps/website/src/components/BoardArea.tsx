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
    <div className="flex flex-col md:flex-row w-full min-w-0 items-stretch gap-2">
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
  );
}
