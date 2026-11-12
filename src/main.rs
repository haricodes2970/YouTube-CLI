use anyhow::Result;
use clap::Parser;
use youtube_cli::{app::App, detect_capabilities, renderer::RenderMode, run};

/// YouTube CLI — Linux-first terminal YouTube client (Phase 0 shell).
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Force a render mode: 1 (ANSI truecolor), 2 (color ASCII), 3 (monochrome ASCII).
    #[arg(short = 'm', long = "mode", value_parser = parse_mode)]
    mode: Option<RenderMode>,
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

    let caps = detect_capabilities();
    let mut app = App::new(&caps);

    // Force the selected renderer into the App's single authoritative renderer.
    // run() draws from App.renderer, so the forced mode takes effect for the
    // whole lifecycle.
    if let Some(mode) = cli.mode {
        app.set_mode(mode);
    }

    run(&mut app, youtube_cli::app::InstantClock::new())?;

    Ok(())
}
