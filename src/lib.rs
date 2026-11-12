//! YouTube CLI — Linux-first terminal YouTube client.
//! Milestone 1A: real video frame decoding via FFmpeg/FFprobe subprocesses.
//! No YouTube integration. No audio yet.

pub mod app;
pub mod decoder;
pub mod error;
pub mod metadata;
pub mod queue;
pub mod renderer;
pub mod renderers;
pub mod terminal;
pub mod ui;

pub use app::{App, Clock, FixedClock, InstantClock, Playback, PlaybackState};
pub use decoder::Decoder;
pub use error::AppError;
pub use metadata::VideoMetadata;
pub use queue::{DecoderThread, FrameQueue, QueuedFrame};
pub use renderer::{RenderMode, VideoFrame, VideoRenderer};
pub use terminal::{
    detect_capabilities, detect_identity, run, ColorDepth, TerminalCapabilities, TerminalIdentity,
};
