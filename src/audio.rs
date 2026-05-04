//! Audio playback for game sounds with global volume control.

use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};

// Volume 0–100 mapped to 0.0–1.0
static VOLUME: AtomicU32 = AtomicU32::new(100);

pub fn set_volume(vol: f32) {
    VOLUME.store((vol.clamp(0.0, 1.0) * 100.0) as u32, Ordering::Relaxed);
}

pub fn get_volume() -> f32 {
    (VOLUME.load(Ordering::Relaxed) as f32).clamp(0.0, 100.0) / 100.0
}

// ---------------------------------------------------------------------------
// Audio output — thread-local because OutputStream is !Send on Windows
// ---------------------------------------------------------------------------

struct AudioDevice {
    _stream: OutputStream,
    sink: Sink,
}

impl AudioDevice {
    fn new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let sink = Sink::try_new(&handle).ok()?;
        Some(Self {
            _stream: stream,
            sink,
        })
    }
}

thread_local! {
    static DEVICE: std::mem::ManuallyDrop<AudioDevice> =
        std::mem::ManuallyDrop::new(
            AudioDevice::new().expect("failed to open audio output"),
        );
}

fn play_sound(data: &'static [u8]) {
    let vol = get_volume();
    if vol <= 0.0 {
        return;
    }
    DEVICE.with(|device| {
        if let Ok(source) = Decoder::new(Cursor::new(data)) {
            device.sink.append(source.amplify(vol));
        }
    });
}

// ---------------------------------------------------------------------------
// Game-sound data (embedded at compile time)
// ---------------------------------------------------------------------------

static SND_MOVE: &[u8] = include_bytes!("assets/audio/move.mp3");
static SND_CAPTURE: &[u8] = include_bytes!("assets/audio/capture.mp3");
static SND_CHECK: &[u8] = include_bytes!("assets/audio/check.mp3");
static SND_GAME_START: &[u8] = include_bytes!("assets/audio/game-start.mp3");
static SND_GAME_END: &[u8] = include_bytes!("assets/audio/game-end.mp3");

pub fn play_move() {
    play_sound(SND_MOVE);
}
pub fn play_capture() {
    play_sound(SND_CAPTURE);
}
pub fn play_check() {
    play_sound(SND_CHECK);
}
pub fn play_game_start() {
    play_sound(SND_GAME_START);
}
pub fn play_game_end() {
    play_sound(SND_GAME_END);
}
