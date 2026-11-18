type Sound = "move" | "capture" | "check" | "game-start" | "game-end";

const SOURCES: Record<Sound, string> = {
  move: "/audio/move.mp3",
  capture: "/audio/capture.mp3",
  check: "/audio/check.mp3",
  "game-start": "/audio/game-start.mp3",
  "game-end": "/audio/game-end.mp3",
};

const cache = new Map<Sound, HTMLAudioElement>();

function element(sound: Sound): HTMLAudioElement {
  let audio = cache.get(sound);
  if (!audio) {
    audio = new Audio(SOURCES[sound]);
    audio.preload = "auto";
    cache.set(sound, audio);
  }
  return audio;
}

function play(sound: Sound): void {
  const audio = element(sound).cloneNode(true) as HTMLAudioElement;
  void audio.play().catch(() => {
    // Browsers block audio until the user interacts with the page.
  });
}

export function playMove(): void {
  play("move");
}

export function playCapture(): void {
  play("capture");
}

export function playCheck(): void {
  play("check");
}

export function playGameStart(): void {
  play("game-start");
}

export function playGameEnd(): void {
  play("game-end");
}

export function playMoveResult(state: {
  status: { kind: string; inCheck: boolean };
  lastMove: { capture: boolean } | null;
}): void {
  if (state.status.kind !== "ongoing") {
    playGameEnd();
  } else if (state.status.inCheck) {
    playCheck();
  } else if (state.lastMove?.capture) {
    playCapture();
  } else {
    playMove();
  }
}
