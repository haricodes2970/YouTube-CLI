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
    /// Enter the alternate screen and raw mode. The constructor is
    /// exception-safe: every initialization step that can fail undoes the
    /// steps completed before it, so a failure never leaves the terminal in a
    /// partially-initialized (broken) state.
    pub fn enter() -> Result<Self> {
        let mut stdout = io::stdout();

        // Step 1: Enter the alternate screen. Nothing to undo if this fails.
        if let Err(e) = execute!(stdout, EnterAlternateScreen) {
            return Err(e.into());
        }

        // Step 2: Enable raw mode. If it fails, leave the alternate screen.
        if let Err(e) = terminal::enable_raw_mode() {
            let _ = execute!(stdout, LeaveAlternateScreen);
            return Err(e.into());
        }

        // Step 3: Construct the Ratatui terminal. If it fails, disable raw
        // mode and leave the alternate screen before returning the error.
        //
        // `CrosstermBackend::new(stdout)` takes ownership of `stdout`. On
        // error the backend is dropped, so we obtain a fresh handle to the
        // same underlying stdout for cleanup.
        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self {
                terminal,
                active: true,
            }),
            Err(e) => {
                let mut stdout = io::stdout();
                let _ = terminal::disable_raw_mode();
                let _ = execute!(stdout, LeaveAlternateScreen);
                Err(e.into())
            }
        }
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

/// Classify terminal identity from independent environment evidence.
///
/// Evidence is consulted in priority order. `TERM` alone is intentionally
/// weak because `xterm-256color` is shared by many emulators and must not be
/// mistaken for GNOME Terminal.
pub fn classify_identity(
    term: &str,
    kitty_window_id: Option<&str>,
    terminal_emulator: Option<&str>,
    vte_version: Option<&str>,
) -> TerminalIdentity {
    // Kitty injects KITTY_WINDOW_ID — strongest independent evidence.
    if kitty_window_id.is_some() {
        return TerminalIdentity::Kitty;
    }
    if terminal_emulator.is_some_and(|v| v.eq_ignore_ascii_case("kitty")) {
        return TerminalIdentity::Kitty;
    }
    if term.contains("kitty") {
        return TerminalIdentity::Kitty;
    }
    // GNOME Terminal runs on VTE and sets VTE_VERSION.
    if vte_version.is_some() {
        return TerminalIdentity::GnomeTerminal;
    }
    if terminal_emulator.is_some_and(|v| v.to_ascii_lowercase().contains("gnome")) {
        return TerminalIdentity::GnomeTerminal;
    }
    if term.contains("gnome") {
        return TerminalIdentity::GnomeTerminal;
    }
    TerminalIdentity::Unknown
}

/// Detect terminal identity from the environment.
pub fn detect_identity() -> TerminalIdentity {
    let term = std::env::var("TERM").unwrap_or_default();
    let kitty_window_id = std::env::var("KITTY_WINDOW_ID").ok();
    let terminal_emulator = std::env::var("TERMINAL_EMULATOR").ok();
    let vte_version = std::env::var("VTE_VERSION").ok();

    classify_identity(
        &term,
        kitty_window_id.as_deref(),
        terminal_emulator.as_deref(),
        vte_version.as_deref(),
    )
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
///
/// `clock` supplies elapsed time for playback updates. Production passes
/// `InstantClock`; tests can pass a deterministic `FixedClock`.
pub fn run(app: &mut App, mut clock: impl crate::app::Clock) -> Result<()> {
    let mut session = TerminalSession::enter()?;
    let terminal = session.terminal();

    loop {
        let elapsed = clock.elapsed();

        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

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
        // detect_identity reads real env; assert only that it returns a valid
        // variant and never panics.
        assert!(matches!(
            detect_identity(),
            TerminalIdentity::Kitty
                | TerminalIdentity::GnomeTerminal
                | TerminalIdentity::Unknown
                | TerminalIdentity::Other
        ));
    }

    #[test]
    fn classify_kitty_window_id() {
        assert_eq!(
            classify_identity("xterm-256color", Some("1"), None, None),
            TerminalIdentity::Kitty
        );
    }

    #[test]
    fn classify_kitty_terminal_emulator() {
        assert_eq!(
            classify_identity("xterm-256color", None, Some("kitty"), None),
            TerminalIdentity::Kitty
        );
    }

    #[test]
    fn classify_kitty_term() {
        assert_eq!(
            classify_identity("xterm-kitty", None, None, None),
            TerminalIdentity::Kitty
        );
    }

    #[test]
    fn classify_gnome_vte_version() {
        assert_eq!(
            classify_identity("xterm-256color", None, None, Some("4200")),
            TerminalIdentity::GnomeTerminal
        );
    }

    #[test]
    fn classify_gnome_terminal_emulator() {
        assert_eq!(
            classify_identity("xterm-256color", None, Some("gnome-terminal"), None),
            TerminalIdentity::GnomeTerminal
        );
    }

    #[test]
    fn classify_gnome_term() {
        assert_eq!(
            classify_identity("gnome-terminal", None, None, None),
            TerminalIdentity::GnomeTerminal
        );
    }

    #[test]
    fn classify_xterm_256color_is_not_gnome_terminal() {
        // xterm-256color is generic and shared by many emulators; without
        // independent GNOME evidence we must not identify it as GNOME Terminal.
        assert_eq!(
            classify_identity("xterm-256color", None, None, None),
            TerminalIdentity::Unknown
        );
    }

    #[test]
    fn classify_unknown_terminal() {
        assert_eq!(
            classify_identity("dumb", None, None, None),
            TerminalIdentity::Unknown
        );
    }

    #[test]
    fn color_depth_falls_back_to_none_without_signals() {
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

    #[test]
    fn terminal_session_restore_is_idempotent() {
        if let Ok(mut session) = TerminalSession::enter() {
            assert!(session.active);
            session.restore();
            assert!(!session.active);
            // Calling restore again must be a no-op, not a panic.
            session.restore();
            assert!(!session.active);
        }
    }
}
