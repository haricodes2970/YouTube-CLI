use anyhow::Result;
use clap::Parser;
use youtube_cli::{app::App, detect_capabilities, renderer::RenderMode, run};

/// YouTube CLI — Linux-first terminal YouTube client (Milestone 1A: real video frame decoding).
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Play a local video file.
    #[command(subcommand)]
    command: Option<Command>,

    /// Force a render mode: 1 (ANSI truecolor), 2 (color ASCII), 3 (monochrome ASCII).
    #[arg(short = 'm', long = "mode", value_parser = parse_mode)]
    mode: Option<RenderMode>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Play a local video file in the terminal.
    Play {
        /// Path to the video file.
        #[arg(value_name = "FILE")]
        path: String,
    },
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

    // If a play command was given, open the video file.
    if let Some(Command::Play { path }) = cli.command {
        app.play(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    run(&mut app, youtube_cli::app::InstantClock::new())?;

    Ok(())
}
