import { useRef, useState } from "react";
import { squareName, type Color, type GameState } from "antarian-chess";
import { pieceImage } from "@/lib/pieces";
import { cn } from "@/lib/utils";
const LIGHT = "bg-[#e8e0d2]";
const DARK = "bg-[#a08064]";

interface BoardProps {
  state: GameState;
  orientation: Color;
  selected: number | null;
  targets: number[];
  interactive: boolean;
  onSelect: (square: number) => void;
  onMove: (from: number, to: number) => boolean;
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
  onSelect,
  onMove,
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
        className="grid h-full w-full touch-none grid-cols-8 grid-rows-8 overflow-hidden rounded-xl border border-border/80 shadow-2xl shadow-black/40"
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
                if (event.detail === 0) {
                  onSelect(square);
                }
              }}
              onPointerDown={(event) => {
                if (event.pointerType === "mouse" && event.button !== 0) {
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
