//! OpenAgents #11971: agent chats build through a gate.
//!
//! The app turns the gate on once at start ([`enable`]). From then on every
//! harness child gets the `cargo` shim from [`coder_lease::agent_builds`]
//! first on its `PATH`, plus the shared target folder root and the
//! `openagents` command that takes the build lease. Heavy `cargo` commands
//! then build in one shared target folder per workspace, refuse below the
//! disk floor, and wait for one of the coordinator's build slots. Until
//! [`enable`] runs, children get their `PATH` unchanged, so tests never
//! write into the real home.

use std::ffi::OsString;

#[cfg(unix)]
static GATE: std::sync::RwLock<Option<coder_lease::agent_builds::Gate>> =
    std::sync::RwLock::new(None);

/// Writes the shim and gives every harness child started from now on the
/// gate. Does nothing when `OPENAGENTS_AGENT_BUILD_GATE=0`, without a home,
/// or off Unix.
///
/// # Errors
/// The shim can't be written; children then get no gate.
pub fn enable() -> std::io::Result<()> {
    #[cfg(unix)]
    if let Some(gate) = coder_lease::agent_builds::Gate::from_env() {
        gate.install()?;
        if let Ok(mut slot) = GATE.write() {
            *slot = Some(gate);
        }
    }
    Ok(())
}

/// The variables a child sets for `path`: `PATH` alone without the gate,
/// else the gate's variables with the shim folder first.
pub(crate) fn child_vars(path: OsString) -> Vec<(OsString, OsString)> {
    #[cfg(unix)]
    {
        let gate = GATE.read().ok().and_then(|gate| gate.clone());
        vars_with(gate.as_ref(), path)
    }
    #[cfg(not(unix))]
    {
        vec![(OsString::from("PATH"), path)]
    }
}

#[cfg(unix)]
fn vars_with(
    gate: Option<&coder_lease::agent_builds::Gate>,
    path: OsString,
) -> Vec<(OsString, OsString)> {
    match gate {
        Some(gate) => gate.vars(Some(&path)),
        None => vec![(OsString::from("PATH"), path)],
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn children_get_the_shim_first_only_with_the_gate() {
        let path = OsString::from("/usr/bin:/bin");
        assert_eq!(
            vars_with(None, path.clone()),
            vec![(OsString::from("PATH"), path.clone())]
        );
        let gate = coder_lease::agent_builds::Gate {
            shims: PathBuf::from("/g/shims"),
            targets: PathBuf::from("/g/targets"),
            bin: Some(PathBuf::from("/g/openagents")),
        };
        let vars = vars_with(Some(&gate), path);
        assert_eq!(
            vars,
            vec![
                (
                    OsString::from("PATH"),
                    OsString::from("/g/shims:/usr/bin:/bin")
                ),
                (
                    OsString::from("OPENAGENTS_AGENT_TARGETS"),
                    OsString::from("/g/targets")
                ),
                (
                    OsString::from("OPENAGENTS_LEASE_BIN"),
                    OsString::from("/g/openagents")
                ),
            ]
        );
    }
}
