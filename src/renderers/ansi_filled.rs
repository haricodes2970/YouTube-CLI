use crate::renderer::{RenderMode, VideoFrame, VideoRenderer};
use crate::terminal::{ColorDepth, TerminalCapabilities};
use crate::AppError;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Mode 1: ANSI truecolor filled using Unicode half-blocks (▀).
///
/// Pairs two rows of pixels into one half-block cell: top pixel -> foreground
/// color, bottom pixel -> background color. Doubles vertical resolution.
pub struct AnsiFilledRenderer;

impl AnsiFilledRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl VideoRenderer for AnsiFilledRenderer {
    fn name(&self) -> &str {
        "ANSI truecolor half-blocks"
    }

    fn mode(&self) -> RenderMode {
        RenderMode::AnsiFilled
    }

    fn supports(&self, caps: &TerminalCapabilities) -> bool {
        matches!(caps.color, ColorDepth::Truecolor) && caps.unicode
    }

    fn render(&self, frame: &VideoFrame, area: Rect, buf: &mut Buffer) -> Result<(), AppError> {
        let area_w = area.width.max(1) as usize;
        let area_h = area.height.max(1) as usize;

        // Downscale frame to fit area (half-block: 2 logical rows per cell).
        let cell_w = area_w;
        let cell_h = area_h.max(1);
        let src_w = frame.width.max(1) as usize;
        let src_h = frame.height.max(1) as usize;

        for cy in 0..cell_h {
            for cx in 0..cell_w {
                let sx = (cx * src_w) / cell_w;
                let top_y = (cy * 2 * src_h) / cell_h;
                let bot_y = ((cy * 2 + 1) * src_h) / cell_h;

                let top = pixel(frame, sx, top_y, src_w, src_h);
                let bot = pixel(frame, sx, bot_y, src_w, src_h);

                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if x >= area.width || y >= area.height {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                cell.set_symbol("▀");
                cell.set_style(
                    ratatui::style::Style::default()
                        .fg(Color::Rgb(top.0, top.1, top.2))
                        .bg(Color::Rgb(bot.0, bot.1, bot.2)),
                );
            }
        }
        Ok(())
    }

    fn default_size(&self, caps: &TerminalCapabilities) -> (u32, u32) {
        (caps.columns as u32, caps.rows.saturating_mul(2) as u32)
    }
}

/// Sample one pixel with nearest-neighbor, clamped to frame bounds.
fn pixel(frame: &VideoFrame, x: usize, y: usize, src_w: usize, src_h: usize) -> (u8, u8, u8) {
    let x = x.min(src_w.saturating_sub(1));
    let y = y.min(src_h.saturating_sub(1));
    let idx = (y * src_w + x) * 3;
    (
        frame.pixels[idx],
        frame.pixels[idx + 1],
        frame.pixels[idx + 2],
    )
}

impl Default for AnsiFilledRenderer {
    fn default() -> Self {
        Self::new()
    }
}
