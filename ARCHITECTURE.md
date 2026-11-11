# YouTube CLI — Architecture

## Status
Phase 0: terminal shell + renderer selection + application state. Compiles and passes tests.
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
  main.rs       clap CLI, entry point, terminal setup
  error.rs      AppError
  app.rs        App, Playback, PlaybackState
  renderer.rs   VideoRenderer trait, RenderMode, VideoFrame, luminance/ascii helpers
  terminal.rs   TerminalCapabilities, ColorDepth, detect_capabilities, select_renderer, run loop
  ui.rs         Ratatui draw loop (header, video area, status bar)
  renderers/
    mod.rs          renderer module + re-exports
    ansi_filled.rs  Mode 1: ANSI truecolor half-blocks (▀)
    color_ascii.rs  Mode 2: color ASCII
    mono_ascii.rs   Mode 3: monochrome ASCII
```

## Core abstractions

### VideoRenderer (trait)
`src/renderer.rs`. One implementor per render mode. The UI draws through `render(frame, area, buf)`;
nothing else knows which mode is active. New mode = one new file, no changes elsewhere.

```rust
pub trait VideoRenderer: Send + Sync {
    fn name(&self) -> &str;
    fn mode(&self) -> RenderMode;
    fn supports(&self, caps: &TerminalCapabilities) -> bool;
    fn render(&self, frame: &VideoFrame, area: Rect, buf: &mut Buffer) -> Result<(), AppError>;
    fn default_size(&self, caps: &TerminalCapabilities) -> (u32, u32);
}
```

### TerminalCapabilities
`src/terminal.rs`. Captures color depth, size, Kitty flag, unicode support. Drives renderer selection.

```rust
pub enum ColorDepth { Truecolor, Ansi256, Ansi16, None }
pub struct TerminalCapabilities {
    pub color: ColorDepth,
    pub columns: u16,
    pub rows: u16,
    pub kitty: bool,
    pub unicode: bool,
}
```

### Playback / App state
`src/app.rs`. Phase 0 is synthetic state only (position, duration, volume, play/pause/stopped).
No FFmpeg decode feeds it yet.

```rust
pub enum PlaybackState { Stopped, Playing, Paused }
pub struct Playback { state, position, duration, volume }
pub struct App { playback, renderer, mode, should_quit, frame_count }
```

## Render modes
1. **ANSI filled (half-blocks ▀)** — truecolor foreground/background per cell, 2 logical rows per cell.
   Requires truecolor + unicode. Default when terminal reports truecolor.
2. **Color ASCII** — one ASCII glyph per cell, truecolor foreground. Requires any ANSI color.
3. **Monochrome ASCII** — one ASCII glyph per cell, white foreground. Works on any terminal.

Glyph choice by luminance bucket; `@ # S % ? * + ; : . `.

## Data flow
```
main → App::new(caps) → select_renderer(caps) → run(app, renderer)
run loop:
  terminal.draw(|f| ui::draw(f, app, renderer))   // renderer.render(test_pattern)
  crossterm input event → App::handle_key          // q/1/2/3/space/arrows
  App::tick()                                       // advance synthetic playback
  restore terminal on exit (raw mode + alternate screen)
```

Phase 0 renders a synthetic color-bar test pattern so terminal output is visible immediately.
Real decoded frames flow through the same `renderer.render` path in Phase 1.

## Error handling
`AppError` in `src/error.rs`: Terminal, Renderer, Io, UnsupportedTerminal. No `unwrap()` on
user input / terminal ops in the hot path; `main` returns `anyhow::Result`.

## Terminal restoration
`run()` always restores cooked mode, leaves the alternate screen, and shows the cursor — even on
panic via `let _ = ...` cleanup. `q` / Esc sets `should_quit` to exit the loop cleanly.

## Constraints honored
- No system packages installed.
- No YouTube integration yet.
- Not generated in one pass; modular, incremental.
- Playback not claimed working until demonstrated.
