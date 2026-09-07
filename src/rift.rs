use anyhow::{Context, Result};
use rift_client::RiftMachClient;
use rift_protocol::{LayoutStateData, Point, Rect, Size};

pub struct Snapshot {
    pub layout: LayoutStateData,
    /// Frame of the display showing the queried workspace. The overlay covers
    /// this, and rect coordinates are relative to its origin.
    pub screen: Rect,
}

pub fn snapshot() -> Result<Snapshot> {
    let client = RiftMachClient::connect().context("rift is not running")?;
    let layout = client.get_layout_state(None).context("get_layout_state failed")?;
    let screen = screen_for_space(&client, layout.space_id)?;
    Ok(Snapshot { layout, screen })
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
