use anyhow::{Context, Result};
use rift_client::RiftMachClient;
use rift_protocol::{LayoutStateData, WindowData};

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
}

pub fn snapshot() -> Result<Snapshot> {
    let client = RiftMachClient::connect().context("rift is not running")?;
    let layout = client.get_layout_state(None).context("get_layout_state failed")?;
    let windows = client.get_windows(None).context("get_windows failed")?;
    // A missing or reshaped gaps block is not worth failing a flash over; zero
    // gaps just means container rects hug their windows' edges.
    let gaps = read_gaps(&client).unwrap_or_default();
    Ok(Snapshot { layout, windows, gaps })
}

fn read_gaps(client: &RiftMachClient) -> Result<Gaps> {
    let cfg = client.get_config()?;
    let inner = &cfg["settings"]["layout"]["gaps"]["inner"];
    Ok(Gaps {
        inner_h: inner["horizontal"].as_f64().unwrap_or(0.0),
        inner_v: inner["vertical"].as_f64().unwrap_or(0.0),
    })
}
