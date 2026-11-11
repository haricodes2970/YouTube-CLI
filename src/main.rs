use clap::Parser;
use youtube_cli::{app::App, renderer::RenderMode, run, select_renderer, detect_capabilities, ColorDepth};
use anyhow::Result;

/// YouTube CLI — Linux-first terminal YouTube client (Phase 0 shell).
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Force a render mode: 1 (ANSI truecolor), 2 (color ASCII), 3 (monochrome ASCII).
    #[arg(short = 'm', long = "mode", value_parser = parse_mode)]
    mode: Option<RenderMode>,
    /// Force color depth for renderer selection: truecolor, 256, 16, none.
    #[arg(long = "color")]
    color: Option<String>,
}

fn parse_mode(s: &str) -> Result<RenderMode, String> {
    match s {
        "1" => Ok(RenderMode::AnsiFilled),
        "2" => Ok(RenderMode::ColorAscii),
        "3" => Ok(RenderMode::MonoAscii),
        _ => Err("mode must be 1, 2, or 3".into()),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut caps = detect_capabilities();

    // Allow CLI override of color depth.
    if let Some(ref c) = cli.color {
        caps.color = match c.as_str() {
            "truecolor" => ColorDepth::Truecolor,
            "256" => ColorDepth::Ansi256,
            "16" => ColorDepth::Ansi16,
            "none" => ColorDepth::None,
            _ => anyhow::bail!("color must be truecolor, 256, 16, or none"),
        };
    }

    let mut app = App::new(&caps);

    // If the user forced a mode, apply it (overrides auto-selection).
    if let Some(mode) = cli.mode {
        app.set_mode(mode);
    }

    let renderer = select_renderer(&caps);
    run(&mut app, renderer)?;

    Ok(())
}
