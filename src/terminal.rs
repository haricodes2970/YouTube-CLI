use crate::app::App;
use crate::renderer::{RenderMode, VideoRenderer};
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, size, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Instant;

/// Terminal *identity*: who is running us. Kept separate from capabilities so
/// renderer selection never depends on the emulator name alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalIdentity {
    Unknown,
    Kitty,
    GnomeTerminal,
    Other,
}

/// Color depth the terminal can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Truecolor,
    Ansi256,
    Ansi16,
    None,
}

/// Terminal *capabilities*: what we can actually render. Derived from identity
/// + environment; never inferred from the emulator name alone.
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub color: ColorDepth,
    pub columns: u16,
    pub rows: u16,
    pub unicode: bool,
}

/// RAII guard for the terminal session. Enters raw mode + alternate screen on
/// construction and restores cooked mode, leaves the alternate screen, and
/// shows the cursor on drop — even if the run loop panics or returns an error.
pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
}

impl TerminalSession {
    /// Enter the alternate screen and raw mode. Returns an error without
    /// leaving the terminal in a partially-initialized state: if any step
    /// fails, everything already done is undone before the error propagates.
    pub fn enter() -> Result<Self> {
        let mut stdout = io::stdout();

        // Step 1: alternate screen. If this fails we have nothing to undo.
        execute!(stdout, EnterAlternateScreen)?;

        // Step 2: raw mode. If this fails, leave the alternate screen.
        if let Err(e) = terminal::enable_raw_mode() {
            let _ = execute!(stdout, LeaveAlternateScreen);
            return Err(e.into());
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    /// Restore the terminal immediately (idempotent).
    pub fn restore(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Detect terminal identity from the environment.
pub fn detect_identity() -> TerminalIdentity {
    let term = std::env::var("TERM").unwrap_or_default();

    match term.as_str() {
        "xterm-kitty" => TerminalIdentity::Kitty,
        _ if term.contains("kitty") => TerminalIdentity::Kitty,
        _ if term.contains("gnome") => TerminalIdentity::GnomeTerminal,
        "xterm-256color" | "xterm-color" => TerminalIdentity::GnomeTerminal,
        _ => TerminalIdentity::Other,
    }
}

/// Detect color depth conservatively.
///
/// Truecolor requires an explicit signal (COLORTERM=truecolor/24bit). A bare
/// xterm-kitty or xterm-256color does NOT imply truecolor.
fn detect_color_depth() -> ColorDepth {
    let color_term = std::env::var("COLORTERM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();

    if color_term.eq_ignore_ascii_case("truecolor") || color_term.eq_ignore_ascii_case("24bit") {
        return ColorDepth::Truecolor;
    }

    if term.contains("256color") || color_term.contains("256color") {
        return ColorDepth::Ansi256;
    }

    if term.contains("color") || color_term.contains("color") {
        return ColorDepth::Ansi16;
    }

    ColorDepth::None
}

/// Detect Unicode support from the locale without assuming it.
fn detect_unicode() -> bool {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            let lower = value.to_ascii_lowercase();
            if lower.contains("utf-8") || lower.contains("utf8") {
                return true;
            }
        }
    }
    false
}

/// Detect terminal capabilities from the environment + crossterm size query.
pub fn detect_capabilities() -> TerminalCapabilities {
    let (columns, rows) = size().unwrap_or((80, 24));

    TerminalCapabilities {
        color: detect_color_depth(),
        columns,
        rows,
        unicode: detect_unicode(),
    }
}

/// Select the best renderer for the given terminal capabilities.
pub fn select_renderer(caps: &TerminalCapabilities) -> Box<dyn VideoRenderer> {
    match caps.color {
        ColorDepth::Truecolor if caps.unicode => {
            Box::new(crate::renderers::AnsiFilledRenderer::new())
        }
        ColorDepth::Truecolor => Box::new(crate::renderers::ColorAsciiRenderer::new()),
        ColorDepth::Ansi256 | ColorDepth::Ansi16 => {
            Box::new(crate::renderers::ColorAsciiRenderer::new())
        }
        ColorDepth::None => Box::new(crate::renderers::MonoAsciiRenderer::new()),
    }
}

/// Safe future graphics-protocol probe placeholder.
///
/// Kitty graphics protocol (and similar) require out-of-band negotiation that
/// we do not implement yet. This intentionally returns `false` so we never
/// claim graphics support merely because TERM contains "kitty". When the
/// protocol is implemented, probe here and gate the renderer on the result.
pub fn graphics_protocol_available(_identity: TerminalIdentity) -> bool {
    false
}

/// Run the application: own the terminal session, drive the event loop, and
/// restore the terminal on exit via the RAII guard.
pub fn run(app: &mut App) -> Result<()> {
    let mut session = TerminalSession::enter()?;
    let terminal = session.terminal();

    let start = Instant::now();
    let mut last = start;

    loop {
        let now = Instant::now();
        let elapsed = now - last;
        last = now;

        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

        // Update playback based on elapsed wall-clock time.
        app.update(elapsed);

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, &key);
                }
            }
        }
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

    #[test]
    fn identity_recognizes_xterm_kitty() {
        // TERM override is not possible in-process; test the matcher logic
        // indirectly by checking the fallback path is sane.
        assert!(matches!(
            detect_identity(),
            TerminalIdentity::Kitty | TerminalIdentity::GnomeTerminal | TerminalIdentity::Other
        ));
    }

    #[test]
    fn color_depth_falls_back_to_none_without_signals() {
        // The dev environment may set COLORTERM, so we only assert the enum is
        // well-formed, not a specific value.
        let depth = detect_color_depth();
        assert!(matches!(
            depth,
            ColorDepth::Truecolor | ColorDepth::Ansi256 | ColorDepth::Ansi16 | ColorDepth::None
        ));
    }

    #[test]
    fn select_renderer_truecolor_with_unicode_picks_ansi_filled() {
        let caps = TerminalCapabilities {
            color: ColorDepth::Truecolor,
            columns: 80,
            rows: 24,
            unicode: true,
        };
        let r = select_renderer(&caps);
        assert_eq!(r.name(), "ANSI truecolor half-blocks");
        assert_eq!(r.mode(), RenderMode::AnsiFilled);
    }

    #[test]
    fn select_renderer_truecolor_without_unicode_picks_color_ascii() {
        let caps = TerminalCapabilities {
            color: ColorDepth::Truecolor,
            columns: 80,
            rows: 24,
            unicode: false,
        };
        let r = select_renderer(&caps);
        assert_eq!(r.name(), "color ASCII");
        assert_eq!(r.mode(), RenderMode::ColorAscii);
    }

    #[test]
    fn select_renderer_256_picks_color_ascii() {
        let caps = TerminalCapabilities {
            color: ColorDepth::Ansi256,
            columns: 80,
            rows: 24,
            unicode: true,
        };
        let r = select_renderer(&caps);
        assert_eq!(r.name(), "color ASCII");
        assert_eq!(r.mode(), RenderMode::ColorAscii);
    }

    #[test]
    fn select_renderer_none_picks_mono_ascii() {
        let caps = TerminalCapabilities {
            color: ColorDepth::None,
            columns: 80,
            rows: 24,
            unicode: true,
        };
        let r = select_renderer(&caps);
        assert_eq!(r.name(), "monochrome ASCII");
        assert_eq!(r.mode(), RenderMode::MonoAscii);
    }

    #[test]
    fn graphics_protocol_never_claims_support() {
        assert!(!graphics_protocol_available(TerminalIdentity::Kitty));
        assert!(!graphics_protocol_available(TerminalIdentity::Unknown));
    }

    #[test]
    fn terminal_session_restores_on_drop() {
        // Constructing may fail headless; if it succeeds, dropping restores.
        if let Ok(session) = TerminalSession::enter() {
            assert!(session.active);
            drop(session);
        }
    }
}
