use crate::terminal::TerminalCapabilities;
use crate::AppError;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::result::Result;

/// A decoded video frame, ready for rendering but not yet on screen.
///
/// Kept decoder-independent: the renderer only sees pixels + metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    /// RGB888 pixels, row-major.
    pub pixels: Vec<u8>,
    pub timestamp_ms: u64,
}

impl VideoFrame {
    /// Expected pixel buffer size for this frame (width * height * 3).
    pub fn expected_size(&self) -> usize {
        self.width as usize * self.height as usize * 3
    }

    /// Validate that the pixel buffer matches the frame dimensions.
    ///
    /// Returns an error if the buffer is too small or too large.
    pub fn validate(&self) -> Result<(), AppError> {
        let expected = self.expected_size();
        if self.pixels.len() != expected {
            return Err(AppError::Renderer(format!(
                "frame buffer size {} does not match expected {} ({}x{}x3)",
                self.pixels.len(),
                expected,
                self.width,
                self.height
            )));
        }
        Ok(())
    }
}

/// Renderer interface. New modes add one implementor; nothing else changes.
pub trait VideoRenderer: Send + Sync {
    fn name(&self) -> &str;
    fn mode(&self) -> RenderMode;
    /// Whether this renderer can drive the given terminal.
    fn supports(&self, caps: &TerminalCapabilities) -> bool;
    /// Render one frame into the Ratatui buffer at `area`.
    fn render(&self, frame: &VideoFrame, area: Rect, buf: &mut Buffer) -> Result<(), AppError>;
    /// Preferred frame size (downscale target) for this terminal.
    fn default_size(&self, caps: &TerminalCapabilities) -> (u32, u32);
}

/// Render mode, selected from terminal capabilities or by the user (1/2/3 keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Mode 1: ANSI truecolor filled Unicode half-blocks.
    AnsiFilled,
    /// Mode 2: color ASCII.
    ColorAscii,
    /// Mode 3: monochrome ASCII.
    MonoAscii,
}

impl RenderMode {
    pub fn label(self) -> &'static str {
        match self {
            RenderMode::AnsiFilled => "1: ANSI truecolor half-blocks",
            RenderMode::ColorAscii => "2: color ASCII",
            RenderMode::MonoAscii => "3: monochrome ASCII",
        }
    }
}

/// Convert an RGB888 triple to an ANSI truecolor escape sequence.
pub fn ansi_truecolor(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Reset ANSI colors.
pub const ANSI_RESET: &str = "\x1b[0m";

/// Brightness of an RGB pixel (perceived luminance).
pub fn luminance(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

/// ASCII character by luminance bucket (0 = darkest, 10 = brightest).
pub fn ascii_char(lum: u8) -> char {
    match lum {
        0..=25 => '@',
        26..=50 => '#',
        51..=75 => 'S',
        76..=100 => '%',
        101..=125 => '?',
        126..=150 => '*',
        151..=175 => '+',
        176..=200 => ';',
        201..=225 => ':',
        226..=250 => '.',
        _ => ' ',
    }
}
