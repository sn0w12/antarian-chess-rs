import { useNavigate } from "react-router-dom";
import { BoardArea } from "@/components/BoardArea";
import { GameResultDialog } from "@/components/GameResultDialog";
import { GameShell } from "@/components/GameShell";
import { GameSidebar } from "@/components/GameSidebar";
import { GameLoading } from "@/components/gameBits";
import { useBotGame } from "@/hooks/useBotGame";
import { useHistoryKeys } from "@/hooks/useHistoryKeys";
import { capturedPieces, formatClock, formatEval } from "@/lib/engine";
import { botGameResult } from "@/lib/gameResult";

export function BotPage() {
  const navigate = useNavigate();
  const game = useBotGame({ playerColor: "white" });

  useHistoryKeys({
    length: game.history.length,
    viewIndex: game.viewIndex,
    onChange: game.setView,
  });

  if (!game.ready) {
    return <GameLoading ready={game.ready} error={game.error} />;
  }

  const { live, viewed } = game;
  if (!live || !viewed) {
    return <GameLoading ready={game.ready} error={game.error} />;
  }

  const check = live.status.inCheck ? " — check!" : "";
  const totalPlies = game.history.length - 1;
  const reviewing = game.isReviewing;

  let status: string;
  if (game.flagged === "white") {
    status = "Time forfeit — you lose";
  } else if (game.flagged === "black") {
    status = "Time forfeit — you win!";
  } else if (live.status.kind === "checkmate") {
    status =
      live.status.winner === "white"
        ? "Checkmate — you win!"
        : "Checkmate — you lose";
  } else if (live.status.kind === "stalemate") {
    status = "Stalemate — draw";
  } else if (reviewing) {
    status = `Viewing move ${game.viewIndex} of ${totalPlies}`;
  } else if (live.turn === "white") {
    status = `Your turn (White)${check}`;
  } else {
    status = `Black to move${check}`;
  }

  const yours = capturedPieces(viewed.squares, "black");
  const theirs = capturedPieces(viewed.squares, "white");
  const result = botGameResult(game.flagged, live);

  return (
    <>
      <GameShell
        board={
        <BoardArea
          state={viewed}
          orientation="white"
          selected={game.selected}
          targets={game.targets}
          interactive={game.interactive}
          promotion={game.pendingPromotion}
          onSelect={game.select}
          onMove={game.play}
          onPromote={game.choosePromotion}
          onCancelPromotion={game.cancelPromotion}
          centipawns={
            reviewing || game.botScore === null
              ? game.material
              : game.botScore
          }
        />
      }
      sidebar={
        <GameSidebar
          mode="vs Bot"
          you="White"
          opponent="Bot"
          moves={reviewing ? `${game.viewIndex} / ${totalPlies}` : `${totalPlies}`}
          history={{
            canBack: game.viewIndex > 0,
            canForward: game.viewIndex < game.history.length - 1,
            onBack: () => game.setView(game.viewIndex - 1),
            onForward: () => game.setView(game.viewIndex + 1),
          }}
          status={status}
          yourTurn={game.interactive}
          bot={{
            score: game.botScore === null ? "" : formatEval(game.botScore),
            depth: game.botDepth === null ? "" : `${game.botDepth}`,
          }}
          clocks={{
            white: formatClock(game.clocks.white),
            black: formatClock(game.clocks.black),
            active: live.turn,
          }}
          captures={{ yours, theirs, yourColor: "white" }}
          onLeave={() => navigate("/")}
        />
      }
      />
      <GameResultDialog result={result} onLeave={() => navigate("/")} />
    </>
  );
}
