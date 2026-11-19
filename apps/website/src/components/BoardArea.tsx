import type { Color, GameState, PieceKind } from "antarian-chess";
import { Board } from "./Board";
import { EvalBar } from "./EvalBar";

interface BoardAreaProps {
  state: GameState;
  orientation: Color;
  selected: number | null;
  targets: number[];
  interactive: boolean;
  promotion: { from: number; to: number } | null;
  onSelect: (square: number) => void;
  onMove: (from: number, to: number) => boolean;
  onPromote: (kind: PieceKind) => void;
  onCancelPromotion: () => void;
  centipawns: number;
}

export function BoardArea({
  state,
  orientation,
  selected,
  targets,
  interactive,
  promotion,
  onSelect,
  onMove,
  onPromote,
  onCancelPromotion,
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
            promotion={promotion}
            onSelect={onSelect}
            onMove={onMove}
            onPromote={onPromote}
            onCancelPromotion={onCancelPromotion}
          />
        </div>
      </div>
    </div>
  );
}
