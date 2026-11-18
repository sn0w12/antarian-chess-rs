import type { Piece, PieceKind } from "antarian-chess";

export const PIECE_IMAGE: Record<string, string> = {
  "white.emperor": "/pieces/emperor_white.png",
  "white.empress": "/pieces/empress_white.png",
  "white.priest": "/pieces/priest_white.png",
  "white.paladin": "/pieces/paladin_white.png",
  "white.dragon": "/pieces/dragon_white.png",
  "white.knight": "/pieces/knight_white.png",
  "black.emperor": "/pieces/emperor_black.png",
  "black.empress": "/pieces/empress_black.png",
  "black.priest": "/pieces/priest_black.png",
  "black.paladin": "/pieces/paladin_black.png",
  "black.dragon": "/pieces/dragon_black.png",
  "black.knight": "/pieces/knight_black.png",
};

export function pieceImage(piece: Piece): string {
  return PIECE_IMAGE[`${piece.color}.${piece.kind}`];
}

export function pieceLabel(kind: PieceKind): string {
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}
