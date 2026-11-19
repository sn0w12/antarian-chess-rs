import { useRef, useState } from "react";
import {
  squareName,
  type Color,
  type GameState,
  type PieceKind,
} from "antarian-chess";
import { pieceImage, pieceLabel } from "@/lib/pieces";
import { cn } from "@/lib/utils";
const LIGHT = "bg-[#e8e0d2]";
const DARK = "bg-[#a08064]";
const PROMOTION_CHOICES: PieceKind[] = [
  "empress",
  "priest",
  "paladin",
  "dragon",
];

interface BoardProps {
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
}

interface DragState {
  from: number;
  originX: number;
  originY: number;
  x: number;
  y: number;
  size: number;
  dragging: boolean;
}

export function Board({
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
}: BoardProps) {
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [dropSquare, setDropSquare] = useState<number | null>(null);

  const checkSquare = state.status.inCheck
    ? state.squares.findIndex(
        (piece) => piece?.kind === "emperor" && piece.color === state.turn,
      )
    : -1;

  const draggedPiece = drag ? (state.squares[drag.from] ?? null) : null;

  const promotionCell = (() => {
    if (!promotion) {
      return null;
    }
    const file = promotion.to % 8;
    const rank = Math.floor(promotion.to / 8);
    return {
      file,
      rank,
      column: orientation === "white" ? file : 7 - file,
      row: orientation === "white" ? 7 - rank : rank,
    };
  })();
  const promotionColor = promotion
    ? (state.squares[promotion.from]?.color ?? state.turn)
    : state.turn;
  const promotionTop = promotionCell
    ? Math.min(promotionCell.row, 8 - PROMOTION_CHOICES.length)
    : 0;

  const squareAt = (x: number, y: number): number | null => {
    const node = document
      .elementFromPoint(x, y)
      ?.closest<HTMLElement>("[data-square]");
    if (!node) {
      return null;
    }
    const value = Number(node.dataset.square);
    return Number.isInteger(value) ? value : null;
  };

  return (
    <>
      <div
        ref={boardRef}
        className="relative grid h-full w-full touch-none grid-cols-8 grid-rows-8 overflow-hidden rounded-xl border border-border/80 shadow-2xl shadow-black/40"
      >
        {Array.from({ length: 64 }, (_, index) => {
          const square =
            orientation === "white"
              ? (7 - Math.floor(index / 8)) * 8 + (index % 8)
              : Math.floor(index / 8) * 8 + (7 - (index % 8));
          const file = square % 8;
          const rank = Math.floor(square / 8);
          const light = (file + rank) % 2 === 1;
          const piece = state.squares[square] ?? null;
          const name = squareName(square);
          const isSelected = selected === square;
          const isTarget = targets.includes(square);
          const isLast =
            state.lastMove?.from === square || state.lastMove?.to === square;
          const isCheck = checkSquare === square;
          const canDrag =
            interactive && piece !== null && piece.color === state.turn;
          const isDrop = dropSquare === square;
          const hidden = drag?.dragging === true && drag.from === square;

          return (
            <button
              key={square}
              type="button"
              data-square={square}
              aria-label={
                piece
                  ? `${name}, ${piece.color} ${piece.kind}`
                  : `${name}, empty`
              }
              className={cn(
                "relative select-none outline-none transition-colors",
                light ? LIGHT : DARK,
                interactive ? "cursor-pointer" : "cursor-default",
              )}
              onClick={(event) => {
                if (promotion) {
                  onCancelPromotion();
                  return;
                }
                if (event.detail === 0) {
                  onSelect(square);
                }
              }}
              onPointerDown={(event) => {
                if (event.pointerType === "mouse" && event.button !== 0) {
                  return;
                }
                if (promotion) {
                  onCancelPromotion();
                  return;
                }
                onSelect(square);
                if (!canDrag) {
                  return;
                }
                const width = boardRef.current?.clientWidth ?? 0;
                try {
                  event.currentTarget.setPointerCapture(event.pointerId);
                } catch {
                  // Synthetic pointer events have no active pointer to capture.
                }
                setDrag({
                  from: square,
                  originX: event.clientX,
                  originY: event.clientY,
                  x: event.clientX,
                  y: event.clientY,
                  size: width / 8,
                  dragging: false,
                });
                setDropSquare(null);
              }}
              onPointerMove={(event) => {
                if (!drag || drag.from !== square) {
                  return;
                }
                const dx = event.clientX - drag.originX;
                const dy = event.clientY - drag.originY;
                const dragging = drag.dragging || Math.hypot(dx, dy) > 4;
                setDrag({
                  ...drag,
                  x: event.clientX,
                  y: event.clientY,
                  dragging,
                });
                if (dragging) {
                  const target = squareAt(event.clientX, event.clientY);
                  setDropSquare(
                    target !== null && targets.includes(target) ? target : null,
                  );
                }
              }}
              onPointerUp={(event) => {
                if (!drag || drag.from !== square) {
                  return;
                }
                const wasDragging = drag.dragging;
                const target = wasDragging
                  ? squareAt(event.clientX, event.clientY)
                  : null;
                setDrag(null);
                setDropSquare(null);
                if (wasDragging && target !== null && target !== drag.from) {
                  onMove(drag.from, target);
                }
              }}
              onPointerCancel={() => {
                setDrag(null);
                setDropSquare(null);
              }}
            >
              {isLast && (
                <span className="pointer-events-none absolute inset-0 bg-[#f0c987]/45" />
              )}
              {isSelected && (
                <span className="pointer-events-none absolute inset-0 ring-2 ring-[#e9b949] ring-inset" />
              )}
              {isCheck && (
                <span className="pointer-events-none absolute inset-0 bg-red-500/35" />
              )}
              {isTarget && (
                <span className="pointer-events-none absolute inset-0 bg-emerald-500/25" />
              )}
              {isDrop && (
                <span className="pointer-events-none absolute inset-0 bg-emerald-400/35" />
              )}

              {piece && !hidden && (
                <img
                  src={pieceImage(piece)}
                  alt=""
                  draggable={false}
                  className="pointer-events-none absolute inset-0 h-full w-full object-contain p-[7%] drop-shadow-[0_2px_2px_rgba(0,0,0,0.35)]"
                />
              )}

              {isTarget && !piece && (
                <span className="pointer-events-none absolute left-1/2 top-1/2 size-[24%] -translate-x-1/2 -translate-y-1/2 rounded-full bg-emerald-400/80" />
              )}
              {isTarget && piece && (
                <span className="pointer-events-none absolute inset-0 ring-2 ring-emerald-400/70 ring-inset" />
              )}
            </button>
          );
        })}

        {promotionCell && (
          <div
            className="absolute z-20 flex flex-col overflow-hidden rounded-md border border-border bg-popover shadow-2xl shadow-black/50"
            style={{
              left: `${(promotionCell.column / 8) * 100}%`,
              top: `${(promotionTop / 8) * 100}%`,
              width: "12.5%",
              height: `${(PROMOTION_CHOICES.length / 8) * 100}%`,
            }}
          >
            {PROMOTION_CHOICES.map((kind, index) => {
              const fromTop = promotionCell.row > 3;
              const choice = fromTop
                ? PROMOTION_CHOICES[PROMOTION_CHOICES.length - 1 - index]
                : kind;
              if (!choice) {
                return null;
              }
              return (
                <button
                  key={choice}
                  type="button"
                  aria-label={`Promote to ${pieceLabel(choice)}`}
                  className="flex flex-1 items-center justify-center p-[6%] transition-colors hover:bg-accent/60 focus-visible:bg-accent/60 focus-visible:outline-none"
                  onClick={(event) => {
                    event.stopPropagation();
                    onPromote(choice);
                  }}
                >
                  <img
                    src={pieceImage({ color: promotionColor, kind: choice })}
                    alt=""
                    draggable={false}
                    className="size-full object-contain drop-shadow-[0_2px_2px_rgba(0,0,0,0.35)]"
                  />
                </button>
              );
            })}
          </div>
        )}
      </div>

      {drag?.dragging && draggedPiece && (
        <div
          className="pointer-events-none fixed z-50 box-border"
          style={{
            left: drag.x - drag.size / 2,
            top: drag.y - drag.size / 2,
            width: drag.size,
            height: drag.size,
          }}
        >
          <img
            src={pieceImage(draggedPiece)}
            alt=""
            className="h-full w-full object-contain p-[7%] drop-shadow-[0_6px_12px_rgba(0,0,0,0.45)]"
          />
        </div>
      )}
    </>
  );
}
