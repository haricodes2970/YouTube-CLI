# YouTube CLI — Architecture

## Status
Milestone 1A complete: real video frame decoding via FFmpeg/FFprobe subprocesses.
Compiles cleanly, clippy-clean, fmt-clean, 45 tests passing (39 unit + 6 integration).
No YouTube integration. No audio yet. Real playback not yet demonstrated in a
terminal emulator (requires TTY).

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
- FFmpeg 8.1.3 / FFprobe 8.1.3 — invoked as subprocess, not native dev libs
- clap, serde, serde_json, anyhow, thiserror

## Module boundaries
```
src/
  lib.rs        crate root + public re-exports
  main.rs       clap CLI, entry point (play subcommand)
  error.rs      AppError
  app.rs        App, Playback, PlaybackState, Clock (+ InstantClock, FixedClock)
  renderer.rs   VideoRenderer trait, RenderMode, VideoFrame (+ validate), luminance/ascii helpers
  terminal.rs   TerminalIdentity, TerminalCapabilities, ColorDepth,
                detect_capabilities, detect_identity, select_renderer,
                graphics_protocol_available, TerminalSession (RAII), run loop
  ui.rs         Ratatui draw loop (header, video area, status bar)
  metadata.rs   FFprobe subprocess: width, height, codec, duration, frame rate, pixel format
  decoder.rs    FFmpeg subprocess: rawvideo RGB24 decode, frame reading, lifecycle
  queue.rs      Bounded frame queue + decoder thread (backpressure, EOF)
  renderers/
    mod.rs          renderer module + re-exports
    ansi_filled.rs  Mode 1: ANSI truecolor half-blocks (▀)
    color_ascii.rs  Mode 2: color ASCII
    mono_ascii.rs   Mode 3: monochrome ASCII

tests/
  integration_test.rs  FFprobe metadata, RGB24 frame reading, EOF, validation, shutdown
```

## Core abstractions

### VideoRenderer (trait)
`src/renderer.rs`. One implementor per render mode. The UI draws through
`render(frame, area, buf)`; nothing else knows which mode is active. New mode
= one new file, no changes elsewhere.

### VideoFrame
`src/renderer.rs`. Contains width, height, RGB888 pixel data, timestamp_ms.
`validate()` checks that `pixels.len() == width * height * 3` before rendering.

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

Identity classification (`classify_identity`, pure and testable) uses
independent evidence in priority order — `TERM` alone is intentionally weak:

1. `KITTY_WINDOW_ID` set → Kitty
2. `TERMINAL_EMULATOR=kitty` → Kitty
3. `TERM` contains "kitty" → Kitty
4. `VTE_VERSION` set → GNOME Terminal
5. `TERMINAL_EMULATOR` contains "gnome" → GNOME Terminal
6. `TERM` contains "gnome" → GNOME Terminal
7. Otherwise → Unknown

`xterm-256color` is shared by many emulators and is **not** treated as
GNOME Terminal without independent evidence.

Color depth detection is conservative: truecolor requires
`COLORTERM=truecolor` or `24bit`; `xterm-kitty`/`xterm-256color` alone do
not imply truecolor. Unicode is detected from `LC_ALL`/`LC_CTYPE`/`LANG`
UTF-8 locale, never assumed.

The graphics-protocol probe (`graphics_protocol_available`) returns `false`
— we never claim Kitty graphics support merely because TERM contains "kitty".

### Renderer ownership — single authoritative renderer
`App` owns the single active `Box<dyn VideoRenderer>`. `run(&mut app, clock)`
draws from `app.renderer`. `App::set_mode` replaces the renderer in place;
playback state (position, state, duration, volume) and the decoder are
untouched — mode switching does not restart decoding.

### FFprobe metadata
`src/metadata.rs`. Spawns `ffprobe -v error -select_streams v:0 -show_entries
stream=... -show_entries format=duration -of json <path>` and parses the JSON
output. Returns `VideoMetadata { width, height, codec, duration_secs,
frame_rate, pixel_format }`. Handles missing/invalid metadata with descriptive
errors.

### FFmpeg decoder
`src/decoder.rs`. Spawns `ffmpeg -hide_banner -loglevel error -i <path>
-f rawvideo -pix_fmt rgb24 -s <W>x<H> -` and reads raw RGB24 frames
incrementally from stdout. Frames are read one at a time — the entire video
is never loaded into memory. `Decoder` owns the child process and cleans up
on `Drop`.

### Frame queue
`src/queue.rs`. `FrameQueue` is a bounded mpsc channel between the decoder
thread and the UI loop. `DecoderThread` runs the decode loop in a background
thread, pushing `QueuedFrame { frame, pts_secs }` into the queue. The queue
applies backpressure when full. `try_pop()` is non-blocking for the UI.

### Playback / App state
`src/app.rs`. Playback state (position, duration, volume, play/pause/stopped)
is driven by elapsed wall-clock time via `App::update(elapsed)`. The decoder
thread pushes frames to the queue; `update()` pulls frames whose presentation
time is <= the current position and sets `current_frame` for rendering.

```rust
pub enum PlaybackState { Stopped, Playing, Paused }
pub struct Playback { state, position, duration, volume }
pub struct App {
    playback, renderer, mode, should_quit, frame_count,
    metadata, decoder, queue, decoder_thread, pending_frame, current_frame
}
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
       [--mode N → app.set_mode(mode)]
       [play <file> → app.play(path): probe + spawn decoder + start thread]
run(&mut app, InstantClock::new()):
  TerminalSession::enter()   // RAII: raw mode + alternate screen
  loop:
    elapsed = clock.elapsed()
    terminal.draw(|f| ui::draw(f, app))   // app.renderer.render(current_frame)
    app.update(elapsed)                    // elapsed-time playback + queue drain
    crossterm input → App::handle_key      // q/1/2/3/space/arrows
  TerminalSession drop restores cooked mode, alternate screen, cursor
```

Decoder thread:
```
decoder.next_frame() → validate size → QueuedFrame { frame, pts_secs }
    → queue.push() (backpressure when full)
```

Terminal lifecycle uses `TerminalSession`, an RAII guard that enters raw mode
+ alternate screen on construction and restores on `Drop`. Initialization
failures unwind cleanly at every step (alternate screen, raw mode, Ratatui
terminal) without leaving the terminal broken.

Real decoded frames flow through `renderer.render` — the synthetic test
pattern has been removed from the playback path.

## Error handling
`AppError` in `src/error.rs`: Terminal, Renderer, Io, UnsupportedTerminal. No
`unwrap()` on user input / terminal ops in the hot path; `main` returns
`anyhow::Result`.

## Constraints honored
- No system packages installed.
- No YouTube integration yet.
- No audio yet.
- Not generated in one pass; modular, incremental.
- No native FFmpeg bindings (subprocess only).
- Complete videos never loaded into RAM.
- Playback not claimed working until demonstrated in a terminal emulator.
