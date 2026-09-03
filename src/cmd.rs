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
    let victims = victims(&String::from_utf8_lossy(&out.stdout), std::process::id());
    for pid in &victims {
        // Best effort: a process that exited between pgrep and now is fine.
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
    }
    Ok(victims.len())
}

/// Pids from `pgrep` output, minus our own. Self-exclusion is the whole point:
/// including it would kill the process doing the killing.
fn victims(pgrep_stdout: &str, me: u32) -> Vec<u32> {
    pgrep_stdout
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .filter(|pid| *pid != me)
        .collect()
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
    fn reset_never_targets_itself() {
        // The obvious implementation, `pkill -x <name>`, matches the process
        // running it and kills itself instead of its siblings.
        assert_eq!(victims("100\n200\n300\n", 200), vec![100, 300]);
        assert!(victims("4242\n", 4242).is_empty());
    }

    #[test]
    fn reset_tolerates_empty_and_junk_pgrep_output() {
        assert!(victims("", 1).is_empty());
        assert!(victims("\n\n", 1).is_empty());
        assert_eq!(victims("  77  \nnot-a-pid\n78\n", 1), vec![77, 78]);
    }

    #[test]
    fn directions_parse_and_reject() {
        assert_eq!(parse_direction("left").unwrap(), Direction::Left);
        assert_eq!(parse_direction("down").unwrap(), Direction::Down);
        assert!(parse_direction("sideways").is_err());
    }
}
