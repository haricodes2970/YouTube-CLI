use crate::app::{App, PlaybackState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Draw one frame of the TUI using the app's authoritative active renderer.
///
/// The video area renders the latest decoded frame pulled from the frame
/// queue. If no frame is available yet (decoder starting, EOF, or stopped),
/// a placeholder message is shown.
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
    let source = app
        .metadata
        .as_ref()
        .map(|m| format!("{} ({}x{})", m.codec, m.width, m.height))
        .unwrap_or_else(|| "no video loaded".to_string());
    let title = format!("YouTube CLI — Mode: {}  |  {}", app.renderer.name(), source);
    let paragraph = Paragraph::new(Text::from(Span::raw(title)))
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
    let state = match app.playback.state {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
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

fn draw_placeholder(f: &mut Frame, area: Rect, app: &App) {
    let text = if app.metadata.is_some() && app.playback.state == PlaybackState::Stopped {
        "decoder ready — press space to play"
    } else if app.metadata.is_some() {
        "decoding…"
    } else {
        "no video loaded — use: youtube-cli play <file>"
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
