use anyhow::Result;

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
}
