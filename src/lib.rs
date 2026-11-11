//! YouTube CLI — Linux-first terminal YouTube client.
//! Phase 0: terminal shell + renderer selection + application state.
//! No YouTube integration, no FFmpeg decode yet.

pub mod app;
pub mod error;
pub mod renderer;
pub mod renderers;
pub mod terminal;
pub mod ui;

pub use app::App;
pub use error::AppError;
pub use renderer::{RenderMode, VideoRenderer};
pub use terminal::{detect_capabilities, select_renderer, run, ColorDepth, TerminalCapabilities};
