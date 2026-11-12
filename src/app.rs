use crate::decoder::Decoder;
use crate::metadata::VideoMetadata;
use crate::queue::{DecoderThread, FrameQueue, QueuedFrame};
use crate::renderer::{RenderMode, VideoRenderer};
use crate::terminal::{ColorDepth, TerminalCapabilities};
use std::time::Duration;

/// Playback lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Playback model. Phase 0/1: synthetic or decoded state — no FFmpeg decode
/// feeds it directly; the decoder supplies frames via a queue.
#[derive(Debug, Clone)]
pub struct Playback {
    pub state: PlaybackState,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: u8,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position: Duration::ZERO,
            duration: None,
            volume: 100,
        }
    }
}

/// Clock abstraction for playback timing. Implementations provide the elapsed
/// time since the previous frame; the event loop is responsible for measuring
/// it. This makes playback deterministic and testable.
pub trait Clock: Send + Sync {
    /// Elapsed time since the last call to `elapsed`.
    fn elapsed(&mut self) -> Duration;
}

/// Wall-clock implementation used by the terminal event loop.
pub struct InstantClock {
    last: std::time::Instant,
}

impl InstantClock {
    pub fn new() -> Self {
        Self {
            last: std::time::Instant::now(),
        }
    }
}

impl Default for InstantClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for InstantClock {
    fn elapsed(&mut self) -> Duration {
        let now = std::time::Instant::now();
        let elapsed = now - self.last;
        self.last = now;
        elapsed
    }
}

/// Deterministic clock for tests: returns a fixed duration on every call.
pub struct FixedClock {
    delta: Duration,
}

impl FixedClock {
    pub fn new(delta: Duration) -> Self {
        Self { delta }
    }
}

impl Clock for FixedClock {
    fn elapsed(&mut self) -> Duration {
        self.delta
    }
}

/// Top-level application state. Holds the single authoritative active renderer;
/// the UI draws through `app.renderer`. Switching modes replaces the renderer
/// in place without touching playback state.
pub struct App {
    pub playback: Playback,
    pub renderer: Box<dyn VideoRenderer>,
    pub mode: RenderMode,
    pub should_quit: bool,
    pub frame_count: u64,

    /// Probed video metadata (width, height, codec, duration, frame rate).
    pub metadata: Option<VideoMetadata>,
    /// FFmpeg subprocess decoder.
    pub decoder: Option<Decoder>,
    /// Bounded frame queue between decoder thread and UI.
    pub queue: Option<FrameQueue>,
    /// Background decoder thread.
    pub decoder_thread: Option<DecoderThread>,
    /// A frame popped from the queue ahead of the current position, held for
    /// the next update cycle.
    pub pending_frame: Option<QueuedFrame>,
    /// The latest frame pulled from the queue whose pts <= current position.
    /// This is what the UI renders.
    pub current_frame: Option<crate::renderer::VideoFrame>,
}

impl App {
    /// Build a new App with a renderer selected for the given terminal
    /// capabilities.
    pub fn new(caps: &TerminalCapabilities) -> Self {
        let renderer = crate::terminal::select_renderer(caps);
        let mode = renderer.mode();
        Self {
            playback: Playback::default(),
            renderer,
            mode,
            should_quit: false,
            frame_count: 0,
            metadata: None,
            decoder: None,
            queue: None,
            decoder_thread: None,
            pending_frame: None,
            current_frame: None,
        }
    }

    /// Force a render mode. Replaces the active renderer immediately so the UI
    /// uses it on the next draw. Playback state (position, state, duration)
    /// and the decoder are untouched — mode switching does not restart
    /// decoding.
    pub fn set_mode(&mut self, mode: RenderMode) {
        self.mode = mode;
        let caps = TerminalCapabilities {
            color: match mode {
                RenderMode::AnsiFilled => ColorDepth::Truecolor,
                RenderMode::ColorAscii => ColorDepth::Ansi256,
                RenderMode::MonoAscii => ColorDepth::None,
            },
            columns: 80,
            rows: 24,
            unicode: true,
        };
        self.renderer = crate::terminal::select_renderer(&caps);
    }

    /// Force a specific renderer directly, bypassing capability selection.
    pub fn set_renderer(&mut self, renderer: Box<dyn VideoRenderer>, mode: RenderMode) {
        self.mode = mode;
        self.renderer = renderer;
    }

    /// Open and start decoding a local video file.
    ///
    /// Probes metadata via FFprobe, spawns the FFmpeg decoder subprocess, and
    /// launches the decoder thread. Playback is left stopped; call
    /// `toggle_playback()` to begin.
    pub fn play(&mut self, path: &str) -> Result<(), crate::error::AppError> {
        // Stop any existing playback first.
        self.stop();

        // Step 1: probe metadata.
        let metadata = crate::metadata::probe(path)
            .map_err(|e| crate::error::AppError::Renderer(format!("ffprobe failed: {e}")))?;

        // Step 2: spawn decoder at the video's native resolution.
        let decoder = Decoder::spawn(path, metadata.width, metadata.height)
            .map_err(|e| crate::error::AppError::Renderer(format!("ffmpeg decode failed: {e}")))?;

        // Step 3: create bounded frame queue.
        let queue = FrameQueue::new(8);

        // Step 4: start decoder thread.
        let decoder_thread = DecoderThread::start(decoder, queue.sender(), metadata.frame_rate);

        self.metadata = Some(metadata);
        self.decoder = None; // moved into decoder thread
        self.queue = Some(queue);
        self.decoder_thread = Some(decoder_thread);
        self.playback.duration = self
            .metadata
            .as_ref()
            .map(|m| Duration::from_secs_f64(m.duration_secs));
        self.playback.position = Duration::ZERO;
        self.current_frame = None;
        self.pending_frame = None;

        Ok(())
    }

    /// Stop playback and clean up the decoder thread and subprocess.
    pub fn stop(&mut self) {
        self.playback.state = PlaybackState::Stopped;
        self.playback.position = Duration::ZERO;
        if let Some(thread) = self.decoder_thread.take() {
            let _ = thread.join();
        }
        if let Some(mut decoder) = self.decoder.take() {
            let _ = decoder.terminate();
        }
        self.queue = None;
        self.decoder = None;
        self.current_frame = None;
        self.pending_frame = None;
    }

    pub fn toggle_playback(&mut self) {
        self.playback.state = match self.playback.state {
            PlaybackState::Stopped | PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Playing => PlaybackState::Paused,
        };
    }

    pub fn seek_backward(&mut self) {
        let step = Duration::from_secs(10);
        self.seek_to(self.playback.position.saturating_sub(step));
    }

    pub fn seek_forward(&mut self) {
        let step = Duration::from_secs(10);
        let target = if let Some(duration) = self.playback.duration {
            (self.playback.position + step).min(duration)
        } else {
            self.playback.position + step
        };
        self.seek_to(target);
    }

    /// Seek to an absolute position. For this version, seeking clamps the
    /// playback position; the decoder continues producing frames and the queue
    /// naturally catches up. Full decoder restart at the requested timestamp
    /// requires retaining the file path on `App` (planned for a later version).
    pub fn seek_to(&mut self, target: Duration) {
        self.playback.position = target;
        self.current_frame = None;
        self.pending_frame = None;
    }

    /// Advance playback by `elapsed` wall-clock time. Pulls decoded frames
    /// from the queue and sets the current render frame based on
    /// presentation timestamp. Paused and stopped playback do not advance.
    pub fn update(&mut self, elapsed: Duration) {
        if self.playback.state != PlaybackState::Playing {
            return;
        }

        self.playback.position += elapsed;
        if let Some(duration) = self.playback.duration {
            if self.playback.position >= duration {
                self.playback.state = PlaybackState::Stopped;
                self.playback.position = duration;
                return;
            }
        }

        // Pull frames from the queue. A frame becomes the current render
        // frame when its presentation time is <= the playback position.
        let mut latest_valid: Option<QueuedFrame> = None;

        // Check pending frame first (held from a previous cycle).
        if let Some(pending) = self.pending_frame.take() {
            if pending.frame.timestamp_ms as f64 / 1000.0 <= self.playback.position.as_secs_f64() {
                latest_valid = Some(pending);
            } else {
                self.pending_frame = Some(pending);
            }
        }

        // Drain the queue, keeping the latest frame with pts <= position.
        while let Some(queued) = self.queue.as_ref().and_then(|q| q.try_pop()) {
            let pts_secs = queued.pts_secs;
            if pts_secs <= self.playback.position.as_secs_f64() {
                latest_valid = Some(queued);
            } else {
                // Frame is ahead of position; hold it for the next cycle.
                self.pending_frame = Some(queued);
                break;
            }
        }

        if let Some(queued) = latest_valid {
            self.current_frame = Some(queued.frame);
            self.frame_count += 1;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let caps = TerminalCapabilities {
            color: ColorDepth::Truecolor,
            columns: 80,
            rows: 24,
            unicode: true,
        };
        Self::new(&caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::default()
    }

    #[test]
    fn toggle_playback_starts_and_pauses() {
        let mut a = app();
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        a.toggle_playback();
        assert_eq!(a.playback.state, PlaybackState::Playing);
        a.toggle_playback();
        assert_eq!(a.playback.state, PlaybackState::Paused);
        a.toggle_playback();
        assert_eq!(a.playback.state, PlaybackState::Playing);
    }

    #[test]
    fn seek_backward_clamps_to_zero() {
        let mut a = app();
        a.seek_backward();
        a.seek_backward();
        assert_eq!(a.playback.position, Duration::ZERO);
    }

    #[test]
    fn seek_forward_clamps_to_duration() {
        let mut a = app();
        a.playback.duration = Some(Duration::from_secs(120));
        for _ in 0..100 {
            a.seek_forward();
        }
        assert!(a.playback.position <= a.playback.duration.unwrap());
    }

    #[test]
    fn update_advances_when_playing() {
        let mut a = app();
        a.toggle_playback();
        let before = a.playback.position;
        a.update(Duration::from_secs(1));
        assert!(a.playback.position > before);
    }

    #[test]
    fn update_does_not_advance_when_paused() {
        let mut a = app();
        a.toggle_playback(); // playing
        a.toggle_playback(); // paused
        let before = a.playback.position;
        a.update(Duration::from_secs(10));
        assert_eq!(a.playback.position, before);
        assert_eq!(a.playback.state, PlaybackState::Paused);
    }

    #[test]
    fn update_does_not_advance_when_stopped() {
        let mut a = app();
        let before = a.playback.position;
        a.update(Duration::from_secs(10));
        assert_eq!(a.playback.position, before);
        assert_eq!(a.playback.state, PlaybackState::Stopped);
    }

    #[test]
    fn update_stops_at_duration() {
        let mut a = app();
        a.playback.duration = Some(Duration::from_secs(5));
        a.toggle_playback(); // playing
        for _ in 0..1000 {
            if a.playback.state == PlaybackState::Stopped {
                break;
            }
            a.update(Duration::from_secs(60));
        }
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        assert_eq!(a.playback.position, Duration::from_secs(5));
    }

    #[test]
    fn fixed_clock_returns_deterministic_delta() {
        let mut clock = FixedClock::new(Duration::from_millis(33));
        assert_eq!(clock.elapsed(), Duration::from_millis(33));
        assert_eq!(clock.elapsed(), Duration::from_millis(33));
    }

    #[test]
    fn set_mode_switches_renderer_without_resetting_playback() {
        let mut a = app();
        a.toggle_playback(); // playing
        let position = a.playback.position;
        let state = a.playback.state;

        a.set_mode(RenderMode::MonoAscii);
        assert_eq!(a.renderer.mode(), RenderMode::MonoAscii);
        assert_eq!(a.renderer.name(), "monochrome ASCII");
        assert_eq!(a.playback.position, position);
        assert_eq!(a.playback.state, state);
    }

    #[test]
    fn set_mode_all_three_modes() {
        let mut a = app();
        for (mode, expected_name) in [
            (RenderMode::AnsiFilled, "ANSI truecolor half-blocks"),
            (RenderMode::ColorAscii, "color ASCII"),
            (RenderMode::MonoAscii, "monochrome ASCII"),
        ] {
            a.set_mode(mode);
            assert_eq!(a.renderer.mode(), mode);
            assert_eq!(a.renderer.name(), expected_name);
        }
    }

    #[test]
    fn play_nonexistent_file_returns_error() {
        let mut a = app();
        let result = a.play("/tmp/does-not-exist-youtube-cli-test.mp4");
        assert!(result.is_err());
        // App state should be unchanged on failure.
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        assert!(a.metadata.is_none());
    }

    #[test]
    fn stop_clears_decoder_state() {
        let mut a = app();
        a.playback.position = Duration::from_secs(10);
        a.current_frame = Some(crate::renderer::VideoFrame {
            width: 2,
            height: 2,
            pixels: vec![0; 12],
            timestamp_ms: 0,
        });
        a.stop();
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        assert_eq!(a.playback.position, Duration::ZERO);
        assert!(a.current_frame.is_none());
        assert!(a.metadata.is_none());
    }

    #[test]
    fn set_renderer_preserves_playback() {
        let mut a = app();
        a.toggle_playback();
        let position = a.playback.position;

        let mono: Box<dyn VideoRenderer> = Box::new(crate::renderers::MonoAsciiRenderer::new());
        a.set_renderer(mono, RenderMode::MonoAscii);
        assert_eq!(a.renderer.mode(), RenderMode::MonoAscii);
        assert_eq!(a.playback.position, position);
    }

    #[test]
    fn video_frame_validation() {
        let frame = crate::renderer::VideoFrame {
            width: 2,
            height: 2,
            pixels: vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
            timestamp_ms: 0,
        };
        assert!(frame.validate().is_ok());

        let bad = crate::renderer::VideoFrame {
            width: 2,
            height: 2,
            pixels: vec![0, 0, 0], // too small
            timestamp_ms: 0,
        };
        assert!(bad.validate().is_err());
    }
}
