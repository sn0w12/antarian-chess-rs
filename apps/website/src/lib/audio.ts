type Sound = "move" | "capture" | "check" | "game-start" | "game-end";

const SOURCES: Record<Sound, string> = {
  move: "/audio/move.mp3",
  capture: "/audio/capture.mp3",
  check: "/audio/check.mp3",
  "game-start": "/audio/game-start.mp3",
  "game-end": "/audio/game-end.mp3",
};

type AudioContextConstructor = typeof AudioContext;

type AudioSessionNavigator = Navigator & {
  audioSession?: {
    type:
      | "auto"
      | "playback"
      | "transient"
      | "transient-solo"
      | "ambient"
      | "play-and-record";
  };
};

if (typeof navigator !== "undefined") {
  const audioSession = (navigator as AudioSessionNavigator).audioSession;
  if (audioSession) {
    audioSession.type = "playback";
  }
}

let context: AudioContext | null = null;
const buffers = new Map<Sound, AudioBuffer>();
const pending = new Map<Sound, Promise<AudioBuffer | null>>();

function getContext(): AudioContext | null {
  if (context) {
    return context;
  }
  if (typeof window === "undefined") {
    return null;
  }
  const Constructor =
    window.AudioContext ??
    (window as Window & { webkitAudioContext?: AudioContextConstructor })
      .webkitAudioContext;
  if (!Constructor) {
    return null;
  }
  context = new Constructor();
  return context;
}

function load(sound: Sound): Promise<AudioBuffer | null> {
  const inFlight = pending.get(sound);
  if (inFlight) {
    return inFlight;
  }
  const ctx = getContext();
  if (!ctx) {
    return Promise.resolve(null);
  }
  const request = fetch(SOURCES[sound])
    .then((response) => response.arrayBuffer())
    .then((data) => ctx.decodeAudioData(data))
    .then((buffer) => {
      buffers.set(sound, buffer);
      return buffer;
    })
    .catch(() => null)
    .finally(() => pending.delete(sound));
  pending.set(sound, request);
  return request;
}

function start(ctx: AudioContext, buffer: AudioBuffer): void {
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}

function unlock(): void {
  const ctx = getContext();
  if (!ctx) {
    return;
  }
  void ctx.resume();
  const silence = ctx.createBuffer(1, 1, ctx.sampleRate);
  const source = ctx.createBufferSource();
  source.buffer = silence;
  source.connect(ctx.destination);
  source.start();
}

function play(sound: Sound): void {
  const ctx = getContext();
  if (!ctx) {
    return;
  }
  if (ctx.state !== "running") {
    void ctx.resume();
  }
  const buffer = buffers.get(sound);
  if (buffer) {
    start(ctx, buffer);
    return;
  }
  void load(sound).then((loaded) => {
    if (loaded) {
      start(ctx, loaded);
    }
  });
}

if (typeof window !== "undefined") {
  for (const sound of Object.keys(SOURCES) as Sound[]) {
    void load(sound);
  }
  window.addEventListener("pointerdown", unlock, { once: true });
  window.addEventListener("keydown", unlock, { once: true });
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
