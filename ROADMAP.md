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

## Milestone 1 (next): FFmpeg frame decode + render
- Invoke `ffmpeg` / `ffprobe` as subprocess (no native dev libs)
- Decode a video file to rawvideo RGB frames via `std::process::Command`
- `VideoFrame` fed into `renderer.render` (decoder independent of rendering)
- Playback clock synced to frame timestamps
- Seek support backed by real frames
- Demonstrate actual video playback before claiming it works

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
- Audio (video-first; audio sync considered in Milestone 1)
- GUI, web, mobile

## Success gate for Milestone 1
A video file plays in the terminal at a visible frame rate, renderer-switchable at runtime,
with `cargo test` still green. Until that is demonstrated, playback is not claimed to work.
