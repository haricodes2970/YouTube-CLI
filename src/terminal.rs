use crate::app::App;
use crate::renderer::{RenderMode, VideoRenderer};
use crate::ui;
use crate::renderers::{AnsiFilledRenderer, ColorAsciiRenderer, MonoAsciiRenderer};
use anyhow::Result;
use crossterm::{execute};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, size, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;

/// Color depth the terminal can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Truecolor,
    Ansi256,
    Ansi16,
    None,
}

/// Terminal capabilities used for renderer selection and layout.
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub color: ColorDepth,
    pub columns: u16,
    pub rows: u16,
    pub kitty: bool,
    pub unicode: bool,
}

/// Detect terminal capabilities from the environment + crossterm size query.
pub fn detect_capabilities() -> TerminalCapabilities {
    let (columns, rows) = size().unwrap_or((80, 24));

    let color_term = std::env::var("COLORTERM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();

    let color = if color_term.eq_ignore_ascii_case("truecolor") {
        ColorDepth::Truecolor
    } else if term.contains("256color") || color_term.contains("256color") {
        ColorDepth::Ansi256
    } else if term.contains("color") || color_term.contains("color") {
        ColorDepth::Ansi16
    } else {
        ColorDepth::None
    };

    let kitty = std::env::var("TERMINAL_EMULATOR")
        .map(|v| v.eq_ignore_ascii_case("kitty"))
        .unwrap_or(false);

    TerminalCapabilities {
        color,
        columns,
        rows,
        kitty,
        unicode: true,
    }
}

/// Select the best renderer for the given terminal capabilities.
pub fn select_renderer(caps: &TerminalCapabilities) -> Box<dyn VideoRenderer> {
    match caps.color {
        ColorDepth::Truecolor => Box::new(AnsiFilledRenderer::new()),
        ColorDepth::Ansi256 | ColorDepth::Ansi16 => Box::new(ColorAsciiRenderer::new()),
        ColorDepth::None => Box::new(MonoAsciiRenderer::new()),
    }
}

/// Run the application: enter alternate screen, run loop, restore on exit.
pub fn run(app: &mut App, renderer: Box<dyn VideoRenderer>) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(app, renderer, &mut terminal);

    // Restore terminal regardless of outcome.
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

fn run_loop(
    app: &mut App,
    renderer: Box<dyn VideoRenderer>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app, &*renderer))?;

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, &key);
                }
            }
        }

        app.tick();
    }
    Ok(())
}

fn handle_key(app: &mut App, key: &crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('1') => app.set_mode(RenderMode::AnsiFilled),
        KeyCode::Char('2') => app.set_mode(RenderMode::ColorAscii),
        KeyCode::Char('3') => app.set_mode(RenderMode::MonoAscii),
        KeyCode::Char(' ') => app.toggle_playback(),
        KeyCode::Left => app.seek_backward(),
        KeyCode::Right => app.seek_forward(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(color: ColorDepth) -> TerminalCapabilities {
        TerminalCapabilities {
            color,
            columns: 80,
            rows: 24,
            kitty: false,
            unicode: true,
        }
    }

    #[test]
    fn select_renderer_truecolor_picks_ansi_filled() {
        let r = select_renderer(&caps(ColorDepth::Truecolor));
        assert_eq!(r.name(), "ANSI truecolor half-blocks");
        assert_eq!(r.mode(), RenderMode::AnsiFilled);
        assert!(r.supports(&caps(ColorDepth::Truecolor)));
    }

    #[test]
    fn select_renderer_256_picks_color_ascii() {
        let r = select_renderer(&caps(ColorDepth::Ansi256));
        assert_eq!(r.name(), "color ASCII");
        assert_eq!(r.mode(), RenderMode::ColorAscii);
    }

    #[test]
    fn select_renderer_16_picks_color_ascii() {
        let r = select_renderer(&caps(ColorDepth::Ansi16));
        assert_eq!(r.name(), "color ASCII");
    }

    #[test]
    fn select_renderer_none_picks_mono_ascii() {
        let r = select_renderer(&caps(ColorDepth::None));
        assert_eq!(r.name(), "monochrome ASCII");
        assert_eq!(r.mode(), RenderMode::MonoAscii);
        assert!(r.supports(&caps(ColorDepth::None)));
    }

    #[test]
    fn mono_ascii_supports_any_terminal() {
        let r = select_renderer(&caps(ColorDepth::None));
        assert!(r.supports(&TerminalCapabilities {
            color: ColorDepth::None,
            columns: 1,
            rows: 1,
            kitty: false,
            unicode: false,
        }));
    }

    #[test]
    fn detect_capabilities_returns_sane_defaults() {
        let c = detect_capabilities();
        assert!(c.columns > 0);
        assert!(c.rows > 0);
    }
}
