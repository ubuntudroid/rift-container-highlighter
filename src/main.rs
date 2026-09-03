mod cmd;
mod config;
mod geometry;
mod render;
mod rift;
mod vendor;

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
    /// Flash container outlines for the active workspace
    Peek {
        /// Override the configured flash duration, in milliseconds
        #[arg(long)]
        ms: Option<u64>,
    },
    /// Run a rift layout command, then flash. Bind this instead of the
    /// built-in action.
    Wrap {
        /// ascend, descend, move-node, join-window, consume-or-expel-window,
        /// toggle-stack, toggle-orientation, unjoin
        layout_command: String,
        /// left, right, up or down, for the commands that take one
        direction: Option<String>,
        /// Override the configured flash duration, in milliseconds
        #[arg(long)]
        ms: Option<u64>,
    },
    /// Clear any flash left on screen by killing other instances
    Reset,
    /// List the theme names with a built-in palette, one per line
    Themes,
    /// Draw one solid rectangle for a few seconds (checks the overlay plumbing)
    TestOverlay {
        /// How long to keep it on screen, in seconds
        #[arg(long, default_value_t = 3)]
        secs: u64,
    },
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
                    "band_width": cfg.band_width,
                    "outset": cfg.outset,
                    "band_decay": cfg.band_decay,
                    "band_width_by_depth": (0..4).map(|d| cfg.band_width_for_depth(d)).collect::<Vec<_>>(),
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
        Command::Peek { ms } => flash(ms)?,
        Command::Wrap { layout_command, direction, ms } => {
            let dir = direction.as_deref().map(cmd::parse_direction).transpose()?;
            let layout = cmd::to_layout_command(&layout_command, dir)?;
            // The rift command runs first: keypress latency must not depend on
            // anything the flash does.
            rift::execute_layout(layout)?;
            flash(ms)?;
        }
        Command::Themes => {
            for t in config::known_themes() {
                println!("{t}");
            }
        }
        Command::Reset => {
            let n = cmd::reset()?;
            println!("cleared {n} flash process(es)");
        }
        Command::TestOverlay { secs } => test_overlay(secs)?,
    }
    Ok(())
}

fn flash(ms: Option<u64>) -> Result<()> {
    let mut cfg = config::Config::load()?;
    if let Some(ms) = ms {
        cfg.flash_ms = ms;
    }
    let s = rift::snapshot()?;
    let rects = geometry::container_rects(&s.layout, &s.windows, s.gaps);
    render::flash(&rects, &cfg, s.screen)
}

/// Proves the vendored overlay plumbing works before any real drawing depends
/// on it: creates one CGS window, fills it, and lets it die with the process.
fn test_overlay(secs: u64) -> Result<()> {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::CGColor;
    use objc2_quartz_core::CALayer;

    use vendor::cgs_window::CgsWindow;
    use vendor::render_layer::{render_layer_to_cgs_window, with_disabled_actions};

    // Corner probe. A wide, short rectangle at frame origin (0, 0): if the
    // window server shares rift's top-left origin it lands against the top edge
    // of the screen, and if it flips Y it lands against the bottom edge. That
    // decides whether the renderer has to flip rects it gets from rift.
    //
    // level 0 + order_above is the combination verified to display; order_below
    // hides behind app windows and level 20 does not show at all.
    //   name, x, rgb, level, order_above
    let cases: [(&str, f64, (f64, f64, f64), i32, bool); 1] =
        [("red", 0.0, (1.0, 0.0, 0.0), 0, true)];

    // Windows must outlive the sleep: dropping a CgsWindow releases it.
    let mut windows = Vec::new();

    for (name, x, (r, g, b), level, above) in cases {
        let frame = CGRect::new(CGPoint::new(x, 0.0), CGSize::new(400.0, 80.0));

        // Setup copied from rift's stack_line.rs, which is known to work. Every
        // layer mutation goes inside with_disabled_actions as upstream does: it
        // suppresses the implicit animation the setters would start and commits
        // the change into the layer tree before anything renders it.
        let layer = CALayer::layer();
        with_disabled_actions(|| {
            layer.setFrame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
            layer.setBackgroundColor(Some(&CGColor::new_srgb(r, g, b, 1.0)));
        });

        let win = CgsWindow::new(frame)?;
        win.set_opacity(false)?;
        win.set_alpha(1.0)?;
        win.set_level(level)?;
        win.set_tags(1 << 3)?; // no system drop shadow
        if above {
            win.order_above(None)?;
        } else {
            win.order_below(None)?;
        }

        render_layer_to_cgs_window(win.id(), frame.size, &layer);

        println!(
            "{name} wid={:<6} x={x:<7} level={level:<3} order={}",
            win.id(),
            if above { "above" } else { "below" }
        );
        windows.push(win);
    }

    println!("{secs}s");
    // Run the CFRunLoop rather than sleeping. A blocking sleep never services
    // the run loop, and the window server does not composite our windows until
    // it is turned — which is why both working references (rift's daemon and
    // JankyBorders) sit in a run loop.
    objc2_core_foundation::CFRunLoop::run_in_mode(
        unsafe { objc2_core_foundation::kCFRunLoopDefaultMode },
        secs as f64,
        false,
    );
    Ok(())
}
