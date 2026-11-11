//! Renderer implementations for the three initial modes.
//! Mode 1: ANSI truecolor half-blocks. Mode 2: color ASCII. Mode 3: monochrome ASCII.

pub mod ansi_filled;
pub mod color_ascii;
pub mod mono_ascii;

pub use ansi_filled::AnsiFilledRenderer;
pub use color_ascii::ColorAsciiRenderer;
pub use mono_ascii::MonoAsciiRenderer;
