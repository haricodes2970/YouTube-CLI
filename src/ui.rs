use crate::app::{App, PlaybackState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Draw one frame of the TUI using the app's authoritative active renderer.
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // video area
            Constraint::Length(3), // status bar
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_video(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let state_str = match app.playback.state {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };

    let source = app.metadata.as_ref().map(|m| {
        let name = std::path::Path::new(&m.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video");
        format!(
            "{} ({}x{}, {} fps)",
            name, m.width, m.height, m.frame_rate as u32
        )
    });

    let title = if let Some(src) = &source {
        format!(
            "YouTube CLI — {}  |  {}  |  mode: {}",
            src,
            state_str,
            app.renderer.name()
        )
    } else {
        format!(
            "YouTube CLI — {}  |  mode: {}",
            state_str,
            app.renderer.name()
        )
    };

    let paragraph = Paragraph::new(Text::from(Line::from(Span::raw(title))))
        .block(Block::default().borders(Borders::ALL).title("header"));
    f.render_widget(paragraph, area);
}

fn draw_video(f: &mut Frame, app: &App, area: Rect) {
    if let Some(frame) = &app.current_frame {
        // Validate frame buffer size before rendering.
        if frame.validate().is_err() {
            draw_error(f, area, "invalid frame buffer");
            return;
        }
        let _ = app.renderer.render(frame, area, f.buffer_mut());
    } else {
        draw_placeholder(f, area, app);
    }

    // Overlay playback state as a small label in the top-left of the video area.
    let state_str = match app.playback.state {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };
    let label = format!(
        "state: {state_str}  pos: {:.1}s",
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

fn draw_placeholder(f: &mut Frame, area: Rect, app: &App) {
    let text = match app.playback.state {
        PlaybackState::Stopped if app.metadata.is_some() => "decoder ready — press space to play",
        PlaybackState::Playing | PlaybackState::Paused if app.metadata.is_some() => "decoding…",
        _ => "no video loaded — use: youtube-cli play <file>",
    };
    let paragraph =
        Paragraph::new(Text::from(Span::raw(text))).block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

fn draw_error(f: &mut Frame, area: Rect, msg: &str) {
    let paragraph = Paragraph::new(Text::from(Span::raw(msg)))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Red));
    f.render_widget(paragraph, area);
}

/// Draw a simple text progress bar inside `area`.
fn draw_progress_bar(f: &mut Frame, area: Rect, app: &App) {
    let duration = app.playback.duration.unwrap_or_default();
    if duration == Duration::ZERO {
        return;
    }
    let position = app.playback.position;
    let ratio = if position >= duration {
        1.0
    } else {
        position.as_secs_f64() / duration.as_secs_f64()
    };
    let width = area.width.max(1) as usize;
    let filled = (width as f64 * ratio) as usize;
    let bar = "█".repeat(filled).to_string() + &"░".repeat(width.saturating_sub(filled));
    let paragraph =
        Paragraph::new(Text::from(Span::raw(bar))).style(Style::default().fg(Color::Green));
    f.render_widget(paragraph, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let position = app.playback.position.as_secs_f32();
    let duration = app
        .playback
        .duration
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);

    // Progress bar line.
    let bar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    draw_progress_bar(f, bar_chunks[0], app);

    // Status text line.
    let state_str = match app.playback.state {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };
    let status = format!(
        "pos {position:.1}s / {duration:.1}s  |  state: {state_str}  |  frames: {}  |  q: quit  1/2/3: renderer  space: play/pause  ←/→: seek",
        app.frame_count
    );
    let paragraph =
        Paragraph::new(Text::from(Span::raw(status))).block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, bar_chunks[1]);
}

use std::time::Duration;
