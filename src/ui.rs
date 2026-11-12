use crate::app::App;
use crate::renderer::VideoFrame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Draw one frame of the TUI using the app's authoritative active renderer.
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // video area
            Constraint::Length(3), // status bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_video(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = format!("YouTube CLI — Phase 0 (mode: {})", app.renderer.name());
    let paragraph = Paragraph::new(Text::from(Span::raw(title)))
        .block(Block::default().borders(Borders::ALL).title("header"));
    f.render_widget(paragraph, area);
}

fn draw_video(f: &mut Frame, app: &App, area: Rect) {
    // Phase 0: render a synthetic color-bar test pattern so output is visible
    // without any FFmpeg decode. Real frames come through this same path later.
    let frame = test_pattern(area.width, area.height);
    let _ = app.renderer.render(&frame, area, f.buffer_mut());

    // Overlay playback state as a small label in the top-left of the video area.
    let state = match app.playback.state {
        crate::app::PlaybackState::Playing => "playing",
        crate::app::PlaybackState::Paused => "paused",
        crate::app::PlaybackState::Stopped => "stopped",
    };
    let label = format!(
        "state: {state}  pos: {:.1}s",
        app.playback.position.as_secs_f32()
    );
    let label_width = label.len().min(area.width.saturating_sub(2) as usize);
    let paragraph = Paragraph::new(Text::from(Span::raw(label)))
        .style(Style::default().bg(Color::Black).fg(Color::Green));
    let label_area = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: label_width as u16,
        height: 1,
    };
    f.render_widget(paragraph, label_area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let help = "q: quit  1/2/3: renderer  space: play/pause  ←/→: seek";
    let position = app.playback.position.as_secs_f32();
    let duration = app
        .playback
        .duration
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let volume = app.playback.volume;
    let status = format!(
        "pos {position:.1}s / {duration:.1}s  vol {volume}%  frames {}{}",
        app.frame_count,
        if app.should_quit { "  quitting" } else { "" }
    );
    let text = format!("{help}  |  {status}");
    let paragraph =
        Paragraph::new(Text::from(Span::raw(text))).block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

/// Synthetic color-bar test pattern (Phase 0 placeholder for decoded frames).
fn test_pattern(width: u16, height: u16) -> VideoFrame {
    let w = width.max(1) as u32;
    let h = height.max(1) as u32;
    let mut pixels = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            // Vertical color bars + a luminance ramp so all three modes show something.
            let r = (x * 255 / w.max(1)) as u8;
            let g = ((w.max(1) - x) * 255 / w.max(1)) as u8;
            let b = (y * 255 / h.max(1)) as u8;
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }
    VideoFrame {
        width: w,
        height: h,
        pixels,
        timestamp_ms: 0,
    }
}
