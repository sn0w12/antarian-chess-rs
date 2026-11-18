import type { Color, GameState } from "antarian-chess";

export type GameResultTone = "win" | "loss" | "draw" | "neutral";

export interface GameResult {
  id: string;
  title: string;
  description: string;
  tone: GameResultTone;
}

export function botGameResult(
  flagged: Color | null,
  live: GameState,
): GameResult | null {
  const player: Color = "white";

  if (flagged) {
    const lost = flagged === player;
    return {
      id: `flagged-${flagged}`,
      tone: lost ? "loss" : "win",
      title: lost ? "You lost on time" : "You won on time",
      description: lost
        ? "Your clock ran out."
        : "The bot ran out of time.",
    };
  }

  if (live.status.kind === "checkmate") {
    const won = live.status.winner === player;
    return {
      id: "checkmate",
      tone: won ? "win" : "loss",
      title: won ? "You won" : "You lost",
      description: won
        ? "Checkmate. The bot has no legal moves."
        : "Checkmate. You have no legal moves.",
    };
  }

  if (live.status.kind === "stalemate") {
    return {
      id: "stalemate",
      tone: "draw",
      title: "Draw",
      description: "Stalemate. No legal moves remain.",
    };
  }

  return null;
}

export function onlineGameResult({
  live,
  color,
  over,
  result,
  reason,
}: {
  live: GameState;
  color: Color;
  over: boolean;
  result: string | null;
  reason: string | null;
}): GameResult | null {
  if (live.status.kind === "checkmate") {
    const won = live.status.winner === color;
    return {
      id: "checkmate",
      tone: won ? "win" : "loss",
      title: won ? "You won" : "You lost",
      description: won
        ? "Checkmate. Your opponent has no legal moves."
        : "Checkmate. You have no legal moves.",
    };
  }

  if (live.status.kind === "stalemate") {
    return {
      id: "stalemate",
      tone: "draw",
      title: "Draw",
      description: "Stalemate. No legal moves remain.",
    };
  }

  if (!over) {
    return null;
  }

  if (reason === "opponent disconnected") {
    return {
      id: "opponent-left",
      tone: "neutral",
      title: "Opponent left",
      description: "The match ended because your opponent disconnected.",
    };
  }

  if (result === "win") {
    const onTime = reason === "timeout" || reason === "time_out";
    return {
      id: `win-${reason ?? "game-over"}`,
      tone: "win",
      title: onTime ? "You won on time" : "You won",
      description: endDescription("win", reason),
    };
  }

  if (result === "loss") {
    const onTime = reason === "timeout" || reason === "time_out";
    return {
      id: `loss-${reason ?? "game-over"}`,
      tone: "loss",
      title: onTime ? "You lost on time" : "You lost",
      description: endDescription("loss", reason),
    };
  }

  if (result === "draw") {
    return {
      id: `draw-${reason ?? "game-over"}`,
      tone: "draw",
      title: "Draw",
      description: endDescription("draw", reason),
    };
  }

  return {
    id: `over-${reason ?? "unknown"}`,
    tone: "neutral",
    title: "Game over",
    description: "The game has ended.",
  };
}

function endDescription(
  result: "win" | "loss" | "draw",
  reason: string | null,
): string {
  if (reason === "resign") {
    return result === "win" ? "Your opponent resigned." : "You resigned.";
  }
  if (reason === "timeout" || reason === "time_out") {
    return result === "win"
      ? "Your opponent ran out of time."
      : "Your clock ran out.";
  }
  if (reason === "disconnect") {
    return result === "win"
      ? "Your opponent disconnected."
      : "You disconnected.";
  }
  if (result === "draw") {
    return "The game ended in a draw.";
  }
  return "The game is over.";
}
