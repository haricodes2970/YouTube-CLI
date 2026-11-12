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

/// Playback model. Phase 0: synthetic state only — no FFmpeg decode yet.
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
            duration: Some(Duration::from_secs(120)),
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
        }
    }

    /// Force a render mode. Replaces the active renderer immediately so the UI
    /// uses it on the next draw. Playback state (position, state, duration)
    /// is untouched.
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

    pub fn toggle_playback(&mut self) {
        self.playback.state = match self.playback.state {
            PlaybackState::Stopped | PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Playing => PlaybackState::Paused,
        };
    }

    pub fn seek_backward(&mut self) {
        let step = Duration::from_secs(10);
        self.playback.position = self.playback.position.saturating_sub(step);
    }

    pub fn seek_forward(&mut self) {
        let step = Duration::from_secs(10);
        if let Some(duration) = self.playback.duration {
            self.playback.position = (self.playback.position + step).min(duration);
        } else {
            self.playback.position += step;
        }
    }

    /// Advance playback by `elapsed` wall-clock time. Paused and stopped
    /// playback do not advance. When playing past the duration, playback stops
    /// exactly at the duration boundary.
    pub fn update(&mut self, elapsed: Duration) {
        if self.playback.state != PlaybackState::Playing {
            return;
        }

        self.playback.position += elapsed;
        if let Some(duration) = self.playback.duration {
            if self.playback.position >= duration {
                self.playback.state = PlaybackState::Stopped;
                self.playback.position = duration;
            }
        }
        self.frame_count += 1;
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
    fn seek_forward_clamps_to_duration() {
        let mut a = app();
        a.toggle_playback(); // playing
        let dur = a.playback.duration.unwrap();
        for _ in 0..100 {
            a.seek_forward();
        }
        assert!(a.playback.position <= dur);
    }

    #[test]
    fn seek_backward_clamps_to_zero() {
        let mut a = app();
        a.seek_backward();
        a.seek_backward();
        assert_eq!(a.playback.position, Duration::ZERO);
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
        a.toggle_playback(); // playing
        let dur = a.playback.duration.unwrap();
        for _ in 0..100_000 {
            if a.playback.state == PlaybackState::Stopped {
                break;
            }
            a.update(Duration::from_secs(60));
        }
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        assert_eq!(a.playback.position, dur);
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
        a.update(Duration::from_secs(5));
        let position = a.playback.position;
        let state = a.playback.state;

        a.set_mode(RenderMode::MonoAscii);
        assert_eq!(a.renderer.mode(), RenderMode::MonoAscii);
        assert_eq!(a.renderer.name(), "monochrome ASCII");
        // Playback state untouched by the switch.
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
    fn set_renderer_preserves_playback() {
        let mut a = app();
        a.toggle_playback();
        a.update(Duration::from_secs(3));
        let position = a.playback.position;

        let mono: Box<dyn VideoRenderer> = Box::new(crate::renderers::MonoAsciiRenderer::new());
        a.set_renderer(mono, RenderMode::MonoAscii);
        assert_eq!(a.renderer.mode(), RenderMode::MonoAscii);
        assert_eq!(a.playback.position, position);
    }
}
