//! The engine update handoff (#11946, `docs/desktop/move-running-agents.md`).
//!
//! When a newer binary is installed under a running engine service, the
//! engine restarts into it without waiting for every run to end. Runs whose
//! agents are under keepers carry on: the engine refuses new work for a
//! moment, leaves the keepers running as it exits, and the new engine picks
//! them up, as do terminals under keepers (#11951). Runs and terminals
//! without a keeper, and voice, still hold the restart until they end.
//!
//! Two services restart this way:
//!
//! - The `openagents daemon` service, which `Updater::restart_when_superseded`
//!   restarts through launchd or systemd. Its quiet check is
//!   [`quiescent_check`]. On Linux, its unit leaves keepers running once it
//!   is installed with `KillMode=process`; before that, it waits for every
//!   run to end.
//! - The desktop app's background engine (#11931), one service per data
//!   folder. [`superseded`] notices the newer binary, and the engine exits
//!   with [`RESTART_EXIT_CODE`] so the service manager starts the new one.
//!
//! The hooks into the upstream `lib.rs` are listed in
//! `apps/openagents/docs/downstream-patches.md`.

use std::time::Duration;

use crate::sessions::{HANDOFF_LEASE, SessionsEngine};
use crate::terminals::Terminals;

/// The background engine exits with this code to restart into a newer
/// binary. Its service restarts it on any unsuccessful exit.
pub const RESTART_EXIT_CODE: i32 = 75;

/// How often the background engine looks for a newer installed binary.
const POLL: Duration = Duration::from_secs(60);

/// How long the background engine waits for starting dispatches to finish.
const DRAIN_WITHIN: Duration = Duration::from_secs(10);

/// The updater's "nothing would be interrupted by a restart" check. Where
/// the service keeps keepers alive when it restarts, kept runs and kept
/// terminals don't count, and a true answer holds new work back until the
/// restart.
pub(crate) fn quiescent_check(
    sessions: SessionsEngine,
    terminals: Terminals,
) -> openagents_update::QuiescentCheck {
    let survive = keepers_survive_restart();
    std::sync::Arc::new(move || {
        let open = if survive {
            terminals.any_open()
        } else {
            terminals.any_live()
        };
        if open {
            return false;
        }
        if survive {
            sessions.hold_for_handoff(HANDOFF_LEASE)
        } else {
            !sessions.any_active()
        }
    })
}

/// Resolves once this background engine should restart into a newer
/// installed binary: one is installed, and every live run can be handed
/// over. New work is refused from then on. Never resolves for any other
/// engine.
pub(crate) async fn superseded(sessions: SessionsEngine, terminals: Terminals) {
    if !running_as_background_service() {
        return std::future::pending().await;
    }
    let mut logged = false;
    loop {
        tokio::time::sleep(POLL).await;
        let installed = openagents_update::installed_version(&openagents_update::detect_install());
        let Some(installed) = installed else { continue };
        if !openagents_update::version_newer(&installed, openagents_update::current_version()) {
            continue;
        }
        if !terminals.any_open() && sessions.handoff_ready() && sessions.drain(DRAIN_WITHIN).await {
            if terminals.any_open() {
                sessions.end_drain();
                continue;
            }
            tracing::info!(%installed, "newer version installed; handing runs to it");
            return;
        }
        if !logged {
            logged = true;
            tracing::info!(
                %installed,
                "newer version installed; restart waits for runs and terminals that would stop"
            );
        }
    }
}

/// Whether this process is a desktop app's background engine service.
pub fn running_as_background_service() -> bool {
    if cfg!(target_os = "macos") {
        std::env::var("XPC_SERVICE_NAME").is_ok_and(|label| background_label(&label))
    } else if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/self/cgroup")
            .is_ok_and(|cgroups| background_cgroup(&cgroups))
    } else {
        false
    }
}

/// Whether restarting this engine's service leaves its keepers running.
/// Keepers start sessions of their own, which launchd leaves alone. systemd
/// stops every process in the unit unless the unit says otherwise: the
/// background engine's unit does, and the `openagents daemon` unit does once
/// it is installed with `KillMode=process` (#11951).
fn keepers_survive_restart() -> bool {
    if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/self/cgroup")
            .map(|cgroups| keepers_survive_in(&cgroups, daemon_unit_kill_mode))
            .unwrap_or(true)
    } else {
        true
    }
}

/// The installed `openagents daemon` unit's `KillMode`, as systemd loaded it.
fn daemon_unit_kill_mode() -> Option<String> {
    let output = std::process::Command::new("systemctl")
        .args([
            "--user",
            "show",
            "--property=KillMode",
            "--value",
            openagents_brand::SYSTEMD_UNIT,
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Whether a unit with this `KillMode` leaves its other processes running
/// when it stops.
fn kill_mode_keeps_others(mode: &str) -> bool {
    matches!(mode.trim(), "process" | "none")
}

fn background_label(label: &str) -> bool {
    label
        .strip_prefix(openagents_brand::BACKGROUND_ENGINE_LABEL)
        .is_some_and(|rest| rest.starts_with('.'))
}

fn background_cgroup(cgroups: &str) -> bool {
    cgroup_parts(cgroups).any(|part| {
        part.strip_prefix(openagents_brand::BACKGROUND_ENGINE_UNIT)
            .is_some_and(|rest| rest.starts_with('-') && rest.ends_with(".service"))
    })
}

fn keepers_survive_in(cgroups: &str, daemon_kill_mode: impl FnOnce() -> Option<String>) -> bool {
    if background_cgroup(cgroups) {
        return true;
    }
    if !cgroup_parts(cgroups).any(|part| part == openagents_brand::SYSTEMD_UNIT) {
        return true;
    }
    daemon_kill_mode().is_some_and(|mode| kill_mode_keeps_others(&mode))
}

fn cgroup_parts(cgroups: &str) -> impl Iterator<Item = &str> {
    cgroups
        .lines()
        .filter_map(|line| line.rsplit(':').next())
        .flat_map(|path| path.split('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_services_are_recognized_by_label_and_unit() {
        let label = format!("{}.abc123", openagents_brand::BACKGROUND_ENGINE_LABEL);
        assert!(background_label(&label));
        assert!(!background_label(openagents_brand::LAUNCHD_LABEL));
        assert!(!background_label(&format!(
            "{}x",
            openagents_brand::BACKGROUND_ENGINE_LABEL
        )));

        let background = format!(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/{}-abc123.service\n",
            openagents_brand::BACKGROUND_ENGINE_UNIT
        );
        let daemon = format!(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/{}\n",
            openagents_brand::SYSTEMD_UNIT
        );
        let shell = "0::/user.slice/user-1000.slice/session-2.scope\n";
        assert!(background_cgroup(&background));
        assert!(!background_cgroup(&daemon));
        assert!(!background_cgroup(shell));

        // The background unit and other places never stop keepers.
        let unasked = || -> Option<String> { panic!("only the daemon unit is asked") };
        assert!(keepers_survive_in(&background, unasked));
        assert!(keepers_survive_in(shell, unasked));
        // The daemon unit stops every process in it when it restarts, unless
        // it is installed with KillMode=process.
        assert!(!keepers_survive_in(&daemon, || Some("control-group".into())));
        assert!(!keepers_survive_in(&daemon, || None));
        assert!(keepers_survive_in(&daemon, || Some("process\n".into())));
        assert!(keepers_survive_in(&daemon, || Some("none".into())));
        assert!(!kill_mode_keeps_others("mixed"));
    }
}
