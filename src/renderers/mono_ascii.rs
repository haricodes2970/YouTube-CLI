use crate::renderer::{
    luminance, mono_ascii_char, RenderMode, VideoFrame, VideoRenderer, CHAR_ASPECT_RATIO,
};
use crate::terminal::TerminalCapabilities;
use crate::AppError;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// Mode 3: monochrome ASCII — one ASCII char per cell, single foreground color.
///
/// Uses a denser luminance ramp and aspect-ratio correction (terminal
/// characters are ~2x taller than wide) so the image is not stretched.
pub struct MonoAsciiRenderer;

impl MonoAsciiRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl VideoRenderer for MonoAsciiRenderer {
    fn name(&self) -> &str {
        "monochrome ASCII"
    }

    fn mode(&self) -> RenderMode {
        RenderMode::MonoAscii
    }

    fn supports(&self, _caps: &TerminalCapabilities) -> bool {
        true // works on any terminal
    }

    fn render(&self, frame: &VideoFrame, area: Rect, buf: &mut Buffer) -> Result<(), AppError> {
        let area_w = area.width.max(1) as usize;
        let area_h = area.height.max(1) as usize;
        let src_w = frame.width.max(1) as usize;
        let src_h = frame.height.max(1) as usize;

        // Account for terminal character aspect ratio (~0.5 width/height)
        // so the image is not stretched vertically.
        let eff_src_h = (src_h as f64 * CHAR_ASPECT_RATIO) as usize;

        for cy in 0..area_h {
            for cx in 0..area_w {
                let sx = (cx * src_w) / area_w;
                // Sample fewer vertical source rows per cell to compensate
                // for tall terminal characters.
                let sy = (cy * eff_src_h) / area_h;
                let (r, g, b) = pixel(frame, sx, sy, src_w, src_h);
                let lum = luminance(r, g, b);
                let ch = mono_ascii_char(lum);

                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if x >= area.width || y >= area.height {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                cell.set_symbol(&ch.to_string());
                cell.set_style(Style::default().fg(Color::White));
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

impl Default for MonoAsciiRenderer {
    fn default() -> Self {
        Self::new()
    }
}
