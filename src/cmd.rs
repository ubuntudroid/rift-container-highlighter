use anyhow::{Result, bail};
use rift_protocol::{Direction, LayoutCommand};

/// Map a CLI command name (plus an optional direction) to a rift layout
/// command. Only the structural commands are accepted — the ones whose effect
/// on the container tree is invisible without an outline. Wrapping
/// `move_focus` or a workspace switch would just add a flash to something you
/// can already see.
pub fn to_layout_command(name: &str, dir: Option<Direction>) -> Result<LayoutCommand> {
    let needs_dir = |d: Option<Direction>| -> Result<Direction> {
        match d {
            Some(d) => Ok(d),
            None => bail!("{name} needs a direction: left, right, up or down"),
        }
    };
    Ok(match name {
        "ascend" => LayoutCommand::Ascend,
        "descend" => LayoutCommand::Descend,
        "toggle-stack" => LayoutCommand::ToggleStack,
        "toggle-orientation" => LayoutCommand::ToggleOrientation,
        "unjoin" | "unjoin-windows" => LayoutCommand::UnjoinWindows,
        "move-node" => LayoutCommand::MoveNode(needs_dir(dir)?),
        "join-window" => LayoutCommand::JoinWindow(needs_dir(dir)?),
        "consume-or-expel-window" => LayoutCommand::ConsumeOrExpelWindow(needs_dir(dir)?),
        other => bail!(
            "unknown layout command {other:?}; expected one of: ascend, descend, \
             toggle-stack, toggle-orientation, unjoin, move-node, join-window, \
             consume-or-expel-window"
        ),
    })
}

pub fn parse_direction(s: &str) -> Result<Direction> {
    Ok(match s {
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        other => bail!("unknown direction {other:?}; expected left, right, up or down"),
    })
}

/// Kill every other instance so their overlays go away. Each dying process
/// releases its own window, so this needs no bookkeeping.
///
/// Deliberately not `pkill -x rift-container-highlighter`: that matches the
/// process running it and kills itself instead of its siblings.
pub fn reset() -> Result<usize> {
    let out = std::process::Command::new("pgrep")
        .args(["-x", "rift-container-highlighter"])
        .output()?;
    let me = std::process::id();
    let victims: Vec<u32> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .filter(|pid| *pid != me)
        .collect();
    for pid in &victims {
        // Best effort: a process that exited between pgrep and now is fine.
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
    }
    Ok(victims.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_we_bind_maps() {
        for c in [
            "ascend",
            "descend",
            "toggle-stack",
            "toggle-orientation",
            "unjoin",
            "consume-or-expel-window",
        ] {
            let dir = Some(Direction::Left);
            assert!(to_layout_command(c, dir).is_ok(), "{c} must map");
        }
        assert!(to_layout_command("move-node", Some(Direction::Left)).is_ok());
        assert!(to_layout_command("join-window", Some(Direction::Right)).is_ok());
    }

    #[test]
    fn directionless_commands_ignore_a_direction() {
        assert_eq!(
            to_layout_command("ascend", Some(Direction::Left)).unwrap(),
            LayoutCommand::Ascend
        );
        assert_eq!(to_layout_command("ascend", None).unwrap(), LayoutCommand::Ascend);
    }

    #[test]
    fn a_directional_command_without_a_direction_is_an_error() {
        // Silently defaulting to a direction would move a window somewhere the
        // user did not ask for.
        assert!(to_layout_command("move-node", None).is_err());
        assert!(to_layout_command("join-window", None).is_err());
    }

    #[test]
    fn an_unknown_command_is_an_error() {
        assert!(to_layout_command("not-a-command", None).is_err());
        // Non-structural commands are rejected on purpose.
        assert!(to_layout_command("move-focus", Some(Direction::Left)).is_err());
        assert!(to_layout_command("switch-to-workspace", None).is_err());
    }

    #[test]
    fn directions_parse_and_reject() {
        assert_eq!(parse_direction("left").unwrap(), Direction::Left);
        assert_eq!(parse_direction("down").unwrap(), Direction::Down);
        assert!(parse_direction("sideways").is_err());
    }
}
