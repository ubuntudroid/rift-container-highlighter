mod rift;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rift-container-highlighter", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the layout snapshot as JSON (debugging, fixture capture)
    Dump,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump => {
            let s = rift::snapshot()?;
            let out = serde_json::json!({
                "layout": s.layout,
                "windows": s.windows,
                "gaps": { "inner_h": s.gaps.inner_h, "inner_v": s.gaps.inner_v },
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}
