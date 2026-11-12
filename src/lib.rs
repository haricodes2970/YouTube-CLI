//! YouTube CLI — Linux-first terminal YouTube client.
//! Phase 0: terminal shell + renderer selection + application state.
//! No YouTube integration, no FFmpeg decode yet.

pub mod app;
pub mod error;
pub mod renderer;
pub mod renderers;
pub mod terminal;
pub mod ui;

pub use app::{App, Clock, FixedClock, InstantClock, Playback, PlaybackState};
pub use error::AppError;
pub use renderer::{RenderMode, VideoFrame, VideoRenderer};
pub use terminal::{
    detect_capabilities, detect_identity, run, ColorDepth, TerminalCapabilities, TerminalIdentity,
};
