mod config;
mod geometry;
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
            let cfg = config::Config::load()?;
            let s = rift::snapshot()?;
            // The computed rects travel with the raw state so a fixture records
            // both the input and what the geometry made of it.
            let rects: Vec<_> = geometry::container_rects(&s.layout, &s.windows, s.gaps)
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "depth": r.depth,
                        "selected": r.selected,
                        "color": format!("#{:08x}", cfg.color_for_depth(r.depth)),
                        "x": r.rect.origin.x,
                        "y": r.rect.origin.y,
                        "w": r.rect.size.width,
                        "h": r.rect.size.height,
                    })
                })
                .collect();
            let out = serde_json::json!({
                "layout": s.layout,
                "windows": s.windows,
                "gaps": { "inner_h": s.gaps.inner_h, "inner_v": s.gaps.inner_v },
                "container_rects": rects,
                "config": {
                    "theme": cfg.theme,
                    "flash_ms": cfg.flash_ms,
                    "stroke_width": cfg.stroke_width,
                    "corner_radius": cfg.corner_radius,
                    "level_inset": cfg.level_inset,
                    "dim_factor": cfg.dim_factor,
                    "gaps_override": cfg.gaps.map(|g| serde_json::json!({
                        "inner_h": g.inner_h, "inner_v": g.inner_v,
                    })),
                    "palette": cfg.colors().iter().map(|c| format!("#{c:08x}")).collect::<Vec<_>>(),
                },
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}
