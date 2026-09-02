use anyhow::{Context, Result};
use rift_client::RiftMachClient;
use rift_protocol::{LayoutStateData, Point, Rect, Size, WindowData};

/// Inner gaps only. The outer gap sits between the outermost windows and the
/// screen edge, which is outside every container rect we draw, so it plays no
/// part in the geometry.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Gaps {
    pub inner_h: f64,
    pub inner_v: f64,
}

pub struct Snapshot {
    pub layout: LayoutStateData,
    pub windows: Vec<WindowData>,
    pub gaps: Gaps,
    /// Frame of the display showing the queried workspace. The overlay covers
    /// this, and rect coordinates are relative to its origin.
    pub screen: Rect,
}

pub fn snapshot() -> Result<Snapshot> {
    let client = RiftMachClient::connect().context("rift is not running")?;
    let layout = client.get_layout_state(None).context("get_layout_state failed")?;
    let windows = client.get_windows(None).context("get_windows failed")?;
    // A missing or reshaped gaps block is not worth failing a flash over; zero
    // gaps just means container rects hug their windows' edges.
    let gaps = read_gaps(&client).unwrap_or_default();
    let screen = screen_for_space(&client, layout.space_id)?;
    Ok(Snapshot { layout, windows, gaps, screen })
}

/// The display whose active space is the one we queried. Falls back to the
/// active context, then to the first display, so a multi-display setup where
/// the space ids do not line up still draws somewhere sensible.
fn screen_for_space(client: &RiftMachClient, space_id: u64) -> Result<Rect> {
    let displays = client.get_displays().context("get_displays failed")?;
    let chosen = displays
        .iter()
        .find(|d| d.active_space_ids.contains(&space_id))
        .or_else(|| displays.iter().find(|d| d.is_active_context))
        .or_else(|| displays.first());
    match chosen {
        Some(d) => Ok(d.frame),
        None => Ok(Rect {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size { width: 0.0, height: 0.0 },
        }),
    }
}

fn read_gaps(client: &RiftMachClient) -> Result<Gaps> {
    let cfg = client.get_config()?;
    let inner = &cfg["settings"]["layout"]["gaps"]["inner"];
    Ok(Gaps {
        inner_h: inner["horizontal"].as_f64().unwrap_or(0.0),
        inner_v: inner["vertical"].as_f64().unwrap_or(0.0),
    })
}
