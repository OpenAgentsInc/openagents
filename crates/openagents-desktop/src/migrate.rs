//! Adopting a Mac set up the old way, from the desktop app.
//!
//! `coder-service`'s adoption module does the work (#9973): it reads an old-style
//! setup without changing it, and adopts it by moving the host and owner
//! keys into the keychain and stopping the old agent. The grants, projects,
//! auto-start policy, and tasks stay where they are, so a phone paired
//! before keeps working.
//!
//! The app never runs it in-process. It runs the bundled `coder` as a
//! child, `coder host adopt detect` and then `coder host adopt` (#9969),
//! which prints one line of JSON without a secret ([`Report`]) and exits.
//! Two reasons: the window process never reads a key file, and the
//! keychain items are written by `coder` itself, the program that reads
//! them later as the login agent, so the keychain never asks the person to
//! let `coder` read items OpenAgents.app wrote. Once `coder` reports the
//! setup adopted, the window registers its own login agent.
//!
//! Adoption is offered only when the bundled `coder` reads its keys from
//! the keychain ([`host_reads_keychain`]); an older build would lose the
//! host key once its file moved, and every phone with it. Such a `coder`
//! is never asked to detect or adopt anything.

use crate::model::OldSetup;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// What `coder host adopt [detect]` prints: one line of JSON.
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

/// Runs `coder host adopt`, or `coder host adopt detect` when
/// `detect_only`, for the user whose home is `home`, and reads its report.
pub fn run(coder: &Path, home: &Path, detect_only: bool) -> Report {
    let mut command = Command::new(coder);
    command.args(["host", "adopt"]);
    if detect_only {
        command.arg("detect");
    }
    let output = command
        .env("HOME", home)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Report::Failed {
            message: "coder did not start".into(),
        };
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| serde_json::from_str(line.trim()).ok())
        .unwrap_or(Report::Failed {
            message: "coder gave no answer".into(),
        })
}

/// Whether the `coder` at `path` serves with its keys from the keychain.
/// It asks the binary's own usage text, which names the keychain once the
/// host supports it (#9969); anything else, including a missing binary,
/// is `false`, so adoption is not offered.
pub fn host_reads_keychain(coder: &Path) -> bool {
    Command::new(coder)
        .args(["host", "help"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .is_some_and(|output| {
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            text.contains("keychain")
        })
}

/// The old-style setup under `home`, as the adoption question shows it.
///
/// With a `coder` that reads the keychain, `coder host adopt detect` says
/// whether there is one, how many phones reach it, and whether adoption
/// can proceed. Without one, or when it gives no answer, the access store's
/// presence alone decides, and adoption is not offered: an earlier setup
/// is never mistaken for none, since registering this app's agent beside
/// it would start a second host on the same state.
pub fn old_setup(coder: Option<&Path>, home: &Path) -> Option<OldSetup> {
    if let Some(coder) = coder.filter(|coder| host_reads_keychain(coder)) {
        match run(coder, home, true) {
            Report::None => return None,
            Report::Found { phones, problems } => {
                return Some(OldSetup {
                    phones: Some(phones),
                    ready: problems.is_empty(),
                });
            }
            Report::Adopted { .. } | Report::Failed { .. } => {}
        }
    }
    home.join(".openagents/coder-access/access.json")
        .exists()
        .then_some(OldSetup {
            phones: None,
            ready: false,
        })
}

/// Adopts the old-style setup under `home` with `coder host adopt`, then
/// runs `register` to start this app's own login agent. It detects first
/// and adopts only a setup that is there with nothing in the way. The
/// message on failure is for the person.
pub fn adopt(
    coder: &Path,
    home: &Path,
    register: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    const NOT_MOVED: &str =
        "Couldn't move your setup. Coder keeps running as it was. Try again later.";
    if !host_reads_keychain(coder) {
        return Err(NOT_MOVED.into());
    }
    match run(coder, home, true) {
        Report::Found { problems, .. } if problems.is_empty() => {}
        _ => return Err(NOT_MOVED.into()),
    }
    match run(coder, home, false) {
        Report::Adopted { .. } => register().map_err(|_| {
            "Your setup moved, but Coder didn't start. Quit OpenAgents and open it again.".into()
        }),
        Report::None | Report::Found { .. } | Report::Failed { .. } => Err(NOT_MOVED.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A fake `coder` in `dir`: `host help` names the keychain when
    /// `keychain`; `host adopt detect` prints `detect`; `host adopt`
    /// prints `adopt` and, as the real one writes the keychain, appends
    /// `adopted` to `keychain.txt` beside it. Every call is logged to
    /// `calls.txt`. Nothing here touches a real keychain, home, or agent.
    fn fake_coder(dir: &Path, keychain: bool, detect: &str, adopt: &str) -> PathBuf {
        let path = dir.join("coder");
        let help = if keychain {
            "serve [--keychain | --keys DIR]"
        } else {
            "serve [--keys DIR]"
        };
        let script = format!(
            r#"#!/bin/sh
echo "$* HOME=$HOME" >> '{dir}/calls.txt'
case "$*" in
  "host help") echo '{help}' ;;
  "host adopt detect") echo '{detect}' ;;
  "host adopt") echo 'adopted' >> '{dir}/keychain.txt'; echo '{adopt}' ;;
  *) exit 2 ;;
esac
"#,
            dir = dir.display(),
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn calls(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("calls.txt")).unwrap_or_default()
    }

    const FOUND: &str = r#"{"kind":"found","phones":6,"problems":[]}"#;
    const ADOPTED: &str = r#"{"kind":"adopted","phones":6}"#;

    #[test]
    fn reports_carry_no_secret_and_round_trip() {
        let report = Report::Found {
            phones: 6,
            problems: vec![],
        };
        let line = serde_json::to_string(&report).unwrap();
        assert_eq!(line, FOUND);
        assert_eq!(serde_json::from_str::<Report>(&line).unwrap(), report);
        assert_eq!(
            serde_json::from_str::<Report>(r#"{"kind":"failed","message":"no"}"#).unwrap(),
            Report::Failed {
                message: "no".into()
            }
        );
    }

    #[test]
    fn a_missing_coder_does_not_read_the_keychain() {
        assert!(!host_reads_keychain(Path::new("/nonexistent/coder")));
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            run(Path::new("/nonexistent/coder"), home.path(), true),
            Report::Failed {
                message: "coder did not start".into()
            }
        );
    }

    #[test]
    fn detect_runs_coder_under_the_given_home_and_changes_nothing() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(bin.path(), true, FOUND, ADOPTED);
        assert_eq!(
            old_setup(Some(&coder), home.path()),
            Some(OldSetup {
                phones: Some(6),
                ready: true
            })
        );
        let log = calls(bin.path());
        assert!(log.contains(&format!("host adopt detect HOME={}", home.path().display())));
        assert!(!bin.path().join("keychain.txt").exists());
    }

    #[test]
    fn a_problem_or_nothing_found_is_reported_as_such() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let problem =
            r#"{"kind":"found","phones":2,"problems":["the service serves another host"]}"#;
        let coder = fake_coder(bin.path(), true, problem, ADOPTED);
        assert_eq!(
            old_setup(Some(&coder), home.path()),
            Some(OldSetup {
                phones: Some(2),
                ready: false
            })
        );
        let coder = fake_coder(bin.path(), true, r#"{"kind":"none"}"#, ADOPTED);
        assert_eq!(old_setup(Some(&coder), home.path()), None);
    }

    /// A `coder` that doesn't read the keychain is never asked to detect,
    /// and an earlier setup is still seen, without the offer.
    #[test]
    fn an_older_coder_is_never_asked_and_the_setup_is_still_seen() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(bin.path(), false, FOUND, ADOPTED);
        assert_eq!(old_setup(Some(&coder), home.path()), None);
        let access = home.path().join(".openagents/coder-access");
        std::fs::create_dir_all(&access).unwrap();
        std::fs::write(access.join("access.json"), "{}").unwrap();
        let unready = Some(OldSetup {
            phones: None,
            ready: false,
        });
        assert_eq!(old_setup(Some(&coder), home.path()), unready);
        assert_eq!(old_setup(None, home.path()), unready);
        assert!(!calls(bin.path()).contains("adopt"));
        let mut registered = false;
        assert!(
            adopt(&coder, home.path(), &mut || {
                registered = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!registered);
        assert!(!calls(bin.path()).contains("adopt"));

        // A keychain `coder` that gives no answer: still seen, not offered.
        let coder = fake_coder(bin.path(), true, "not json", ADOPTED);
        assert_eq!(old_setup(Some(&coder), home.path()), unready);
    }

    #[test]
    fn adoption_detects_first_then_adopts_as_coder_then_registers() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(bin.path(), true, FOUND, ADOPTED);
        let mut registered = 0;
        adopt(&coder, home.path(), &mut || {
            registered += 1;
            Ok(())
        })
        .expect("adopted");
        assert_eq!(registered, 1);
        let log = calls(bin.path());
        let detect = log.find("host adopt detect").expect("detected");
        let adopted = log
            .find(&format!("host adopt HOME={}", home.path().display()))
            .expect("adopted");
        assert!(detect < adopted, "{log}");
        assert_eq!(
            std::fs::read_to_string(bin.path().join("keychain.txt")).unwrap(),
            "adopted\n"
        );
    }

    #[test]
    fn a_refused_detection_or_adoption_never_registers() {
        let home = tempfile::tempdir().unwrap();
        let problem = r#"{"kind":"found","phones":6,"problems":["in the way"]}"#;
        let failed = r#"{"kind":"failed","message":"the old agent did not stop"}"#;
        for (detect, adopt_line, adopts) in [
            (problem, ADOPTED, false),
            (r#"{"kind":"none"}"#, ADOPTED, false),
            (FOUND, failed, true),
            (FOUND, "", true),
        ] {
            let bin = tempfile::tempdir().unwrap();
            let coder = fake_coder(bin.path(), true, detect, adopt_line);
            let mut registered = false;
            let error = adopt(&coder, home.path(), &mut || {
                registered = true;
                Ok(())
            })
            .expect_err("refused");
            assert!(error.starts_with("Couldn't move your setup."), "{error}");
            assert!(!registered);
            assert_eq!(bin.path().join("keychain.txt").exists(), adopts);
        }
    }

    #[test]
    fn a_failed_registration_after_adoption_says_the_setup_moved() {
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(bin.path(), true, FOUND, ADOPTED);
        let error = adopt(&coder, home.path(), &mut || Err("no bundle".into()))
            .expect_err("not registered");
        assert!(error.starts_with("Your setup moved"), "{error}");
        assert!(!error.contains("no bundle"));
    }
}
