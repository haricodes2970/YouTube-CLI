# YouTube CLI — Roadmap

## Milestone 0 (done): Terminal shell
- Rust project scaffolded (Cargo, ratatui 0.29, crossterm 0.29)
- `VideoRenderer` trait + `TerminalCapabilities` + `ColorDepth`
- Three render modes: ANSI truecolor half-blocks, color ASCII, monochrome ASCII
- Auto renderer selection from terminal capabilities
- `App` + `Playback` + `PlaybackState` (synthetic state; no decode yet)
- Minimal Ratatui app with keyboard navigation (q/1/2/3/space/arrows)
- Tests: renderer selection + playback state (12 passing)
- Docs: ARCHITECTURE.md, ROADMAP.md
- Builds cleanly with `cargo build`; `cargo test` green

## Milestone 0.1 (done): Architecture corrections
- Single authoritative renderer owned by `App`; `run(&mut app)` draws from it
- `--mode 2` (ColorAsciiRenderer) forced correctly through the full lifecycle
- Terminal detection: `TerminalIdentity` separate from `TerminalCapabilities`;
  recognizes `xterm-kitty`; conservative truecolor (`COLORTERM=truecolor`);
  Unicode from UTF-8 locale; `graphics_protocol_available()` returns false
  (no Kitty graphics claim from TERM alone)
- Playback timing: elapsed-time `App::update(elapsed)` via `Clock` trait
  (`InstantClock` production, `FixedClock` tests); paused/stopped don't advance
- Terminal lifecycle: RAII `TerminalSession` guard restores raw mode, alternate
  screen, cursor on drop; init failures unwind cleanly
- Regression tests: mode switching all three modes preserves playback position;
  forced renderer; deterministic clock
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`,
  `cargo test`, `cargo build` all clean
- Docs: ARCHITECTURE.md, ROADMAP.md updated

## Milestone 0.2 (done): Final foundation hardening
- `Clock` trait integrated into the terminal event loop: `run(app, clock)` uses
  `InstantClock` in production; deterministic `FixedClock` retained for tests
- `TerminalSession::enter()` exception-safe for every init step, including
  `Terminal::new()` failure after raw mode is enabled — cleans up alternate
  screen and raw mode before returning the error
- `xterm-256color` no longer misidentified as GNOME Terminal: identity uses
  independent evidence (`KITTY_WINDOW_ID`, `VTE_VERSION`, `TERMINAL_EMULATOR`);
  falls back to `Unknown`
- Unit tests for terminal identity classification via pure `classify_identity`
  with injected values (kitty, gnome, xterm-256color→Unknown, dumb→Unknown)
- Lifecycle cleanup tests: session restores on drop; `restore()` is idempotent
- Test count: 28 passing
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`,
  `cargo test`, `cargo build` all clean
- Docs: ARCHITECTURE.md, ROADMAP.md updated

## Milestone 1A (done): Real video frame decoding
- `src/metadata.rs`: FFprobe subprocess probes width, height, codec, duration,
  frame rate, pixel format; JSON parsing; handles missing/invalid metadata
- `src/decoder.rs`: FFmpeg subprocess decodes to rawvideo RGB24 via stdout;
  incremental frame reading (entire video never in RAM); `Drop` cleanup
- `src/queue.rs`: Bounded `FrameQueue` (mpsc channel) + `DecoderThread`
  background decode loop with backpressure and EOF handling
- `src/renderer.rs`: `VideoFrame::validate()` checks buffer size before render
- `src/app.rs`: `App::play(path)` orchestrates probe + decode + queue; play/
  pause/stop/seek; `update(elapsed)` pulls frames by presentation timestamp
- `src/ui.rs`: renders decoded `current_frame` via `app.renderer`; synthetic
  test pattern removed from playback path
- `src/main.rs`: `youtube-cli play <file>` subcommand; keeps `--mode` option
- Meaningful errors for missing files, invalid formats, missing ffmpeg
- Tests: 39 unit + 6 integration (FFprobe metadata, RGB24 frame reading, EOF,
  validation, decoder shutdown, invalid paths)
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`,
  `cargo test`, `cargo build` all clean (45 tests total)
- Docs: ARCHITECTURE.md, ROADMAP.md updated

## Milestone 1B (next): Terminal playback demonstration
- Run `youtube-cli play <file>` in a real terminal emulator (Kitty, GNOME Terminal)
- Verify frames render at correct frame rate in all three modes
- Verify renderer switching mid-playback doesn't reset position
- Measure actual playback performance (FPS, CPU)
- Claim playback works only after demonstration

## Milestone 2: Terminal capability hardening
- Resize handling: re-detect capabilities, reflow layout
- Kitty keyboard protocol + GNOME Terminal compatibility verification
- Graceful degradation when truecolor/unicode unavailable

## Milestone 3: YouTube integration
- YouTube URL input + video ID extraction
- yt-dlp or equivalent to resolve stream URL (decision pending)
- Stream video to the FFmpeg decode pipeline
- Playlist / history (later)

## Out of scope for now
- Native FFmpeg development libraries (subprocess only)
- Audio (video-first; audio sync considered in Milestone 2)
- GUI, web, mobile

## Success gate for Milestone 1B
A video file plays in the terminal at a visible frame rate, renderer-switchable at runtime,
with `cargo test` still green. Until that is demonstrated in Kitty and GNOME Terminal,
playback is not claimed to work.
