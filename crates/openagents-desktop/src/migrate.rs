//! Adopting a Mac set up the old way, from the desktop app.
//!
//! `coder_service::adopt` does the work (#9973): it reads an old-style
//! setup without changing it, and adopts it by moving the host and owner
//! keys into the keychain, stopping the old agent, and handing over to the
//! caller to register its own. The grants, projects, auto-start policy,
//! and tasks stay where they are, so a phone paired before keeps working.
//!
//! Both steps read a key file, so the window never runs them itself: it
//! starts its own executable as a helper, `openagents-desktop migrate
//! detect` or `migrate adopt`, which prints one line of JSON without a
//! secret ([`Report`]) and exits. The window reads that line.
//!
//! Adoption is offered only when the bundled `coder` reads its keys from
//! the keychain ([`host_reads_keychain`]); an older build would lose the
//! host key once its file moved, and every phone with it.

use crate::keychain::{KeychainKeySource, OsKeychain};
use coder_service::adopt::{self, Paths};
use coder_service::service::SystemRunner;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// What the helper prints: one line of JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Report {
    /// No old-style setup here.
    None,
    /// An old-style setup; `problems` lists why adoption would refuse.
    Found {
        phones: usize,
        problems: Vec<String>,
    },
    /// Adopted: the same host, now under this app.
    Adopted { phones: usize },
    /// Something failed; nothing secret is in `message`.
    Failed { message: String },
}

/// Reads an old-style setup under `home` without changing anything.
pub fn detect(home: &Path, now: u64) -> Report {
    match adopt::detect(&Paths::under(home), now) {
        Ok(None) => Report::None,
        Ok(Some(found)) => Report::Found {
            phones: found.kept.active_grants,
            problems: found.problems,
        },
        Err(error) => Report::Failed {
            message: error.to_string(),
        },
    }
}

/// Adopts the old-style setup under `home` into the OS keychain, stopping
/// its agent, then runs `register` to start this app's own.
pub fn adopt_here(
    home: &Path,
    now: u64,
    register: &mut dyn FnMut() -> Result<(), String>,
) -> Report {
    let mut keychain = KeychainKeySource::new(OsKeychain);
    let result = adopt::adopt(
        &Paths::under(home),
        now,
        &mut keychain,
        &mut SystemRunner,
        &mut |_| register().map_err(coder_service::Error::Refused),
    );
    match result {
        Ok(adopted) => Report::Adopted {
            phones: adopted.kept.active_grants,
        },
        Err(error) => Report::Failed {
            message: error.to_string(),
        },
    }
}

/// Whether the `coder` at `path` serves with its keys from the keychain.
/// It asks the binary's own usage text, which names the keychain once the
/// host supports it (#9969); anything else, including a missing binary,
/// is `false`, so adoption is not offered.
pub fn host_reads_keychain(coder: &Path) -> bool {
    Command::new(coder)
        .args(["host", "help"])
        .output()
        .ok()
        .is_some_and(|output| {
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            text.contains("keychain")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_without_a_setup_reports_none() {
        let home = tempfile::tempdir().expect("a home");
        assert_eq!(detect(home.path(), 1_790_000_000), Report::None);
    }

    #[test]
    fn reports_carry_no_secret_and_round_trip() {
        let report = Report::Found {
            phones: 6,
            problems: vec![],
        };
        let line = serde_json::to_string(&report).unwrap();
        assert_eq!(line, r#"{"kind":"found","phones":6,"problems":[]}"#);
        assert_eq!(serde_json::from_str::<Report>(&line).unwrap(), report);
    }

    #[test]
    fn a_missing_coder_does_not_read_the_keychain() {
        assert!(!host_reads_keychain(Path::new("/nonexistent/coder")));
    }
}
