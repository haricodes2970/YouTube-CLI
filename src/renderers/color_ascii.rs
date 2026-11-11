use crate::renderer::{ascii_char, luminance, RenderMode, VideoFrame, VideoRenderer};
use crate::terminal::{ColorDepth, TerminalCapabilities};
use crate::AppError;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
/// Mode 2: color ASCII — one ASCII char per pixel cell, colored truecolor.
pub struct ColorAsciiRenderer;

impl ColorAsciiRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl VideoRenderer for ColorAsciiRenderer {
    fn name(&self) -> &str {
        "color ASCII"
    }

    fn mode(&self) -> RenderMode {
        RenderMode::ColorAscii
    }

    fn supports(&self, caps: &TerminalCapabilities) -> bool {
        matches!(
            caps.color,
            ColorDepth::Truecolor | ColorDepth::Ansi256 | ColorDepth::Ansi16
        )
    }

    fn render(&self, frame: &VideoFrame, area: Rect, buf: &mut Buffer) -> Result<(), AppError> {
        let area_w = area.width.max(1) as usize;
        let area_h = area.height.max(1) as usize;
        let src_w = frame.width.max(1) as usize;
        let src_h = frame.height.max(1) as usize;

        for cy in 0..area_h {
            for cx in 0..area_w {
                let sx = (cx * src_w) / area_w;
                let sy = (cy * src_h) / area_h;
                let (r, g, b) = pixel(frame, sx, sy, src_w, src_h);
                let lum = luminance(r, g, b);
                let ch = ascii_char(lum);

                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if x >= area.width || y >= area.height {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                cell.set_symbol(&ch.to_string());
                cell.set_style(Style::default().fg(Color::Rgb(r, g, b)));
            }
        }
        Ok(())
    }

    fn default_size(&self, caps: &TerminalCapabilities) -> (u32, u32) {
        (caps.columns as u32, caps.rows as u32)
    }
}

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

impl Default for ColorAsciiRenderer {
    fn default() -> Self {
        Self::new()
    }
}
