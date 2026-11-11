use crate::renderer::{RenderMode, VideoRenderer};
use crate::terminal::TerminalCapabilities;
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

/// Top-level application state.
pub struct App {
    pub playback: Playback,
    pub renderer: Box<dyn VideoRenderer>,
    pub mode: RenderMode,
    pub should_quit: bool,
    pub frame_count: u64,
}

impl App {
    /// Build a new App with a renderer selected for the given terminal capabilities.
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

    pub fn set_mode(&mut self, mode: RenderMode) {
        self.mode = mode;
        // Re-select renderer to match the requested mode when possible.
        let caps = TerminalCapabilities {
            color: match mode {
                RenderMode::AnsiFilled => crate::terminal::ColorDepth::Truecolor,
                RenderMode::ColorAscii => crate::terminal::ColorDepth::Ansi256,
                RenderMode::MonoAscii => crate::terminal::ColorDepth::None,
            },
            columns: 80,
            rows: 24,
            kitty: false,
            unicode: true,
        };
        self.renderer = crate::terminal::select_renderer(&caps);
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

    /// Advance playback by one tick (~60 FPS). No-op when stopped.
    pub fn tick(&mut self) {
        if self.playback.state == PlaybackState::Playing {
            self.playback.position += Duration::from_millis(16);
            if let Some(duration) = self.playback.duration {
                if self.playback.position >= duration {
                    self.playback.state = PlaybackState::Stopped;
                    self.playback.position = duration;
                }
            }
            self.frame_count += 1;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let caps = TerminalCapabilities {
            color: crate::terminal::ColorDepth::Truecolor,
            columns: 80,
            rows: 24,
            kitty: false,
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
    fn tick_advances_position_when_playing() {
        let mut a = app();
        a.toggle_playback();
        let before = a.playback.position;
        a.tick();
        assert!(a.playback.position > before);
    }

    #[test]
    fn tick_does_not_advance_when_stopped() {
        let mut a = app();
        let before = a.playback.position;
        a.tick();
        assert_eq!(a.playback.position, before);
    }

    #[test]
    fn playback_stops_at_duration() {
        let mut a = app();
        a.toggle_playback(); // playing
        let dur = a.playback.duration.unwrap();
        for _ in 0..100_000 {
            if a.playback.state == PlaybackState::Stopped {
                break;
            }
            a.tick();
        }
        assert_eq!(a.playback.state, PlaybackState::Stopped);
        assert_eq!(a.playback.position, dur);
    }
}
