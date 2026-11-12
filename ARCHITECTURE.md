# YouTube CLI — Architecture

## Status
Phase 0.2 complete: terminal shell hardened. Compiles cleanly, clippy-clean, fmt-clean, 28 tests passing.
No YouTube integration. No FFmpeg decode yet. Video playback does not work and is not claimed.

## Goals
- Linux-first terminal-native video player.
- Kitty (0.47.1) and GNOME Terminal compatible.
- Video decoding independent of rendering.
- Modular renderer interface; auto terminal-capability detection.
- Modular but no unnecessary abstractions.

## Tech stack
- Rust 1.99 (edition 2021, rust-version 1.99)
- Ratatui 0.29 — terminal UI
- Crossterm 0.29 — keyboard input + raw mode + alternate screen
- FFmpeg 8.1.3 / FFprobe 8.1.3 — invoked as subprocess (Phase 1+), not native dev libs
- clap, serde, anyhow, thiserror

## Module boundaries
```
src/
  lib.rs        crate root + public re-exports
  main.rs       clap CLI, entry point
  error.rs      AppError
  app.rs        App, Playback, PlaybackState, Clock (+ InstantClock, FixedClock)
  renderer.rs   VideoRenderer trait, RenderMode, VideoFrame, luminance/ascii helpers
  terminal.rs   TerminalIdentity, TerminalCapabilities, ColorDepth,
                detect_capabilities, detect_identity, select_renderer,
                graphics_protocol_available, TerminalSession (RAII), run loop
  ui.rs         Ratatui draw loop (header, video area, status bar)
  renderers/
    mod.rs          renderer module + re-exports
    ansi_filled.rs  Mode 1: ANSI truecolor half-blocks (▀)
    color_ascii.rs  Mode 2: color ASCII
    mono_ascii.rs   Mode 3: monochrome ASCII
```

## Core abstractions

### VideoRenderer (trait)
`src/renderer.rs`. One implementor per render mode. The UI draws through
`render(frame, area, buf)`; nothing else knows which mode is active. New mode
= one new file, no changes elsewhere.

### TerminalIdentity vs TerminalCapabilities
`src/terminal.rs`. Identity (who is running us) is kept separate from
capabilities (what we can render) so renderer selection never depends on the
emulator name alone.

```rust
pub enum TerminalIdentity { Unknown, Kitty, GnomeTerminal, Other }
pub enum ColorDepth { Truecolor, Ansi256, Ansi16, None }

pub struct TerminalCapabilities {
    pub color: ColorDepth,
    pub columns: u16,
    pub rows: u16,
    pub unicode: bool,
}
```

Detection:
- Identity (`classify_identity`, pure and testable) uses independent evidence
  in priority order: `KITTY_WINDOW_ID` → Kitty; `TERMINAL_EMULATOR=kitty`
  → Kitty; `TERM` contains "kitty" → Kitty; `VTE_VERSION` → GNOME Terminal;
  `TERMINAL_EMULATOR` contains "gnome" → GNOME Terminal; `TERM` contains
  "gnome" → GNOME Terminal; otherwise Unknown. `xterm-256color` is shared by
  many emulators and is **not** treated as GNOME Terminal without independent
  evidence.
- Truecolor: conservative — requires `COLORTERM=truecolor` or `24bit`.
  `xterm-kitty`/`xterm-256color` alone does NOT imply truecolor.
- Unicode: detected from `LC_ALL`/`LC_CTYPE`/`LANG` UTF-8 locale, never assumed.
- Kitty graphics protocol: `graphics_protocol_available()` returns `false` —
  we never claim graphics support merely because TERM contains "kitty".

### Renderer ownership — single authoritative renderer
`App` owns the single active `Box<dyn VideoRenderer>`. `run(&mut app)` draws
from `app.renderer`. `App::set_mode` replaces the renderer in place; playback
state (position, state, duration, volume) is untouched. `App::set_renderer`
allows direct injection (used by tests and forced CLI mode).

### Playback / App state
`src/app.rs`. Phase 0 is synthetic state only (position, duration, volume,
play/pause/stopped). No FFmpeg decode feeds it yet.

```rust
pub enum PlaybackState { Stopped, Playing, Paused }
pub struct Playback { state, position, duration, volume }
pub struct App { playback, renderer, mode, should_quit, frame_count }
```

### Clock abstraction
Playback advances by elapsed wall-clock time via `App::update(elapsed)`, not a
fixed tick. The terminal event loop (`run`) integrates the `Clock` trait,
passing `InstantClock` in production. `FixedClock` is available for
deterministic tests. Paused/stopped playback does not advance.

### Render modes
1. **ANSI filled (half-blocks ▀)** — truecolor foreground/background per cell,
   2 logical rows per cell. Requires truecolor + unicode.
2. **Color ASCII** — one ASCII glyph per cell, truecolor foreground.
   Requires any ANSI color.
3. **Monochrome ASCII** — one ASCII glyph per cell, white foreground. Works on
   any terminal.

Glyph choice by luminance bucket; `@ # S % ? * + ; : . `.

## Data flow
```
main → detect_capabilities() → App::new(&caps)
       [--mode N → app.set_mode(mode)]   // forces renderer into App
run(&mut app):
  TerminalSession::enter()   // RAII: raw mode + alternate screen
  loop:
    elapsed = clock.elapsed()
    terminal.draw(|f| ui::draw(f, app))   // app.renderer.render(test_pattern)
    app.update(elapsed)                    // elapsed-time playback
    crossterm input → App::handle_key      // q/1/2/3/space/arrows
  TerminalSession drop restores cooked mode, alternate screen, cursor
```

Terminal lifecycle uses `TerminalSession`, an RAII guard that enters raw mode
+ alternate screen on construction and restores on `Drop`. Initialization
failures unwind cleanly (alternate screen left, raw mode disabled) without
leaving the terminal broken.

Phase 0.2 renders a synthetic color-bar test pattern so terminal output is
visible immediately. Real decoded frames flow through the same
`renderer.render` path in Phase 1.

## Error handling
`AppError` in `src/error.rs`: Terminal, Renderer, Io, UnsupportedTerminal. No
`unwrap()` on user input / terminal ops in the hot path; `main` returns
`anyhow::Result`.

## Constraints honored
- No system packages installed.
- No YouTube integration yet.
- Not generated in one pass; modular, incremental.
- Playback not claimed working until demonstrated.
