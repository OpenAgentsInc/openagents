//! Starting Coder on launch, upgrading a computer set up the old way first.
//!
//! There is one flow and nothing to answer (#9965): on launch the app runs
//! [`start`] off the UI thread. When this computer already runs Coder from
//! an earlier setup (a launchd agent or systemd unit that runs
//! `coder host serve`, key files, grants), it upgrades it silently with
//! `coder-service`'s adoption (#9973): the host and owner keys move into
//! the keychain (the macOS keychain, the Secret Service, or Credential
//! Manager), each checked by reading it back before its file goes; the old
//! agent and any other agent that serves a host stop and are removed; and
//! then the app registers its own agent on the same `~/.openagents` state.
//! The host key, owner, grants, and epochs never change, so every phone
//! paired before keeps working without pairing again.
//!
//! The app never runs adoption in-process. It runs the bundled `coder` as
//! a child, `coder host adopt detect` and then `coder host adopt` (#9969),
//! which prints one line of JSON without a secret ([`Report`]) and exits.
//! The window process never reads a key file, and the keychain items are
//! written by `coder` itself, the program that reads them later as the
//! login agent, so the keychain never asks the person to let `coder` read
//! items OpenAgents wrote.
//!
//! When a safety check refuses (a read-back that differs, a keychain item
//! that holds a different key, an old agent that won't stop, a host still
//! holding the store) or the bundled `coder` can't read the keychain,
//! nothing moves: the earlier setup keeps running as it was, the reason is
//! logged, and the window shows its normal screens with one quiet line
//! ([`KEPT_RUNNING`]). There is no question and no extra screen.

use crate::model::{Agent, Started};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The one line the window shows when an earlier setup had to stay.
pub const KEPT_RUNNING: &str = "Coder keeps running here as it did before this app.";

/// Where the desktop app's host keeps its keys, as `coder host adopt
/// detect` reports it: the keychain, or, on a Linux desktop where no
/// Secret Service answers, private files as a command-line install keeps
/// them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPlace {
    #[default]
    Keychain,
    Files,
}

/// Where the host's keys go, for `coder host serve` and `coder host adopt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keys {
    /// `--keychain`.
    Keychain,
    /// `--keys DIR`.
    Files(PathBuf),
}

impl Keys {
    /// The key directory under `home` when the keys are files.
    pub fn at(place: KeyPlace, home: &Path) -> Keys {
        match place {
            KeyPlace::Keychain => Keys::Keychain,
            KeyPlace::Files => Keys::Files(home.join(".openagents/host-keys")),
        }
    }

    /// The `coder host serve` options that name this key source.
    pub fn serve_args(&self) -> Vec<String> {
        match self {
            Keys::Keychain => vec!["--keychain".into()],
            Keys::Files(dir) => vec!["--keys".into(), dir.display().to_string()],
        }
    }
}

/// What `coder host adopt [detect]` prints: one line of JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Report {
    /// No old-style setup here, or one already moved.
    None {
        #[serde(default)]
        keys: KeyPlace,
    },
    /// An old-style setup; `problems` lists why adoption would refuse.
    Found {
        phones: usize,
        problems: Vec<String>,
        #[serde(default)]
        keys: KeyPlace,
    },
    /// Adopted: the same host, now under this app.
    Adopted { phones: usize },
    /// Something failed; nothing secret is in `message`.
    Failed { message: String },
}

/// Runs `coder host ARGS` for the user whose home is `home`, and reads its
/// report.
fn run(coder: &Path, home: &Path, args: &[String]) -> Report {
    let output = Command::new(coder)
        .args(["host", "adopt"])
        .args(args)
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
/// is `false`, and an earlier setup is left running rather than moved to
/// a `coder` that would lose its host key.
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

/// Starts Coder under this app: upgrades an earlier setup under `home`
/// silently when there is one, then runs `register` with the key source to
/// register this app's own agent. When the earlier setup has to stay, it
/// is left running exactly as it was, `register` never runs (two hosts on
/// one state never both serve), and the result carries [`KEPT_RUNNING`].
pub fn start(
    coder: Option<&Path>,
    home: &Path,
    register: &mut dyn FnMut(&Keys) -> Agent,
) -> Started {
    // A test that reached the person's real home would move their setup.
    #[cfg(test)]
    assert!(
        home.canonicalize().ok()
            != std::env::var_os("HOME")
                .and_then(|real| std::path::Path::new(&real).canonicalize().ok()),
        "a test reached the real home; give it a temporary one"
    );
    let kept = |reason: &str| {
        eprintln!("openagents-desktop: the earlier Coder setup keeps running: {reason}");
        Started {
            agent: Agent::NotRegistered,
            note: Some(KEPT_RUNNING.into()),
        }
    };
    let earlier = home.join(".openagents/coder-access/access.json").exists();
    let Some(coder) = coder.filter(|coder| host_reads_keychain(coder)) else {
        if earlier {
            return kept("this app's coder does not read the keychain");
        }
        return Started {
            agent: register(&Keys::Keychain),
            note: None,
        };
    };
    match run(coder, home, &["detect".into()]) {
        Report::None { keys } => Started {
            agent: register(&Keys::at(keys, home)),
            note: None,
        },
        Report::Found { problems, keys, .. } if problems.is_empty() => {
            let keys = Keys::at(keys, home);
            let args = match &keys {
                Keys::Keychain => vec![],
                Keys::Files(_) => keys.serve_args(),
            };
            match run(coder, home, &args) {
                Report::Adopted { .. } => Started {
                    agent: register(&keys),
                    note: None,
                },
                Report::Failed { message } => kept(&message),
                other => kept(&format!("coder answered {other:?}")),
            }
        }
        Report::Found { problems, .. } => kept(&problems.join("; ")),
        Report::Failed { message } => kept(&message),
        Report::Adopted { .. } => kept("coder adopted during detection"),
    }
}

/// Starts Coder under a dev build ([`crate::RELEASE`] is false): as
/// [`start`], but it never runs `coder host adopt`, so a dev build never
/// moves an earlier setup's keys into the keychain or prompts for the
/// signed app's items (#10096). An earlier setup keeps running as it was.
pub fn start_dev(home: &Path, register: &mut dyn FnMut(&Keys) -> Agent) -> Started {
    if home.join(".openagents/coder-access/access.json").exists() {
        eprintln!(
            "openagents-desktop: a dev build never adopts; the earlier Coder setup keeps running"
        );
        return Started {
            agent: Agent::NotRegistered,
            note: Some(KEPT_RUNNING.into()),
        };
    }
    Started {
        agent: register(&Keys::Keychain),
        note: None,
    }
}

// The tests stand in for `coder` with shell scripts.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Tests that write a fake `coder` and run it take turns: on Linux a
    /// script another test's fork still holds open for writing fails to
    /// exec with "text file busy".
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// A fake `coder` in `dir`: `host help` names the keychain when
    /// `keychain`; `host adopt detect` prints `detect`; `host adopt ...`
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
  "host adopt"*) echo 'adopted' >> '{dir}/keychain.txt'; echo '{adopt}' ;;
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

    fn earlier_setup(home: &Path) {
        let access = home.join(".openagents/coder-access");
        std::fs::create_dir_all(&access).unwrap();
        std::fs::write(access.join("access.json"), "{}").unwrap();
    }

    const FOUND: &str = r#"{"kind":"found","phones":6,"problems":[],"keys":"keychain"}"#;
    const ADOPTED: &str = r#"{"kind":"adopted","phones":6}"#;

    /// Runs [`start`], recording each registration's key source.
    fn launch(coder: Option<&Path>, home: &Path) -> (Started, Vec<Keys>) {
        let mut registered = Vec::new();
        let started = start(coder, home, &mut |keys| {
            registered.push(keys.clone());
            Agent::Enabled
        });
        (started, registered)
    }

    #[test]
    fn reports_carry_no_secret_and_round_trip() {
        let report = Report::Found {
            phones: 6,
            problems: vec![],
            keys: KeyPlace::Keychain,
        };
        let line = serde_json::to_string(&report).unwrap();
        assert_eq!(line, FOUND);
        assert_eq!(serde_json::from_str::<Report>(&line).unwrap(), report);
        // An older coder says nothing about keys: the keychain.
        assert_eq!(
            serde_json::from_str::<Report>(r#"{"kind":"none"}"#).unwrap(),
            Report::None {
                keys: KeyPlace::Keychain
            }
        );
        assert_eq!(
            serde_json::from_str::<Report>(r#"{"kind":"failed","message":"no"}"#).unwrap(),
            Report::Failed {
                message: "no".into()
            }
        );
    }

    /// A dev build never adopts: an earlier setup keeps running and
    /// nothing registers; without one it registers as the release does.
    #[test]
    fn a_dev_build_never_adopts() {
        let home = tempfile::tempdir().unwrap();
        let mut registered = Vec::new();
        let started = start_dev(home.path(), &mut |keys| {
            registered.push(keys.clone());
            Agent::NotRegistered
        });
        assert_eq!(started.note, None);
        assert_eq!(registered, [Keys::Keychain]);
        earlier_setup(home.path());
        let started = start_dev(home.path(), &mut |_| panic!("a dev build registered"));
        assert_eq!(started.agent, Agent::NotRegistered);
        assert_eq!(started.note.as_deref(), Some(KEPT_RUNNING));
    }

    #[test]
    fn a_missing_coder_does_not_read_the_keychain() {
        assert!(!host_reads_keychain(Path::new("/nonexistent/coder")));
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            run(Path::new("/nonexistent/coder"), home.path(), &[]),
            Report::Failed {
                message: "coder did not start".into()
            }
        );
    }

    /// An earlier setup is upgraded with no question: detect, adopt as
    /// `coder` under the person's home, then register this app's agent.
    #[test]
    fn an_earlier_setup_upgrades_silently_then_registers() {
        let _serial = serial();
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        earlier_setup(home.path());
        let coder = fake_coder(bin.path(), true, FOUND, ADOPTED);
        let (started, registered) = launch(Some(&coder), home.path());
        assert_eq!(
            started,
            Started {
                agent: Agent::Enabled,
                note: None
            }
        );
        assert_eq!(registered, [Keys::Keychain]);
        let log = calls(bin.path());
        let detect = log
            .find(&format!("host adopt detect HOME={}", home.path().display()))
            .expect("detected");
        let adopted = log
            .find(&format!("host adopt HOME={}", home.path().display()))
            .expect("adopted");
        assert!(detect < adopted, "{log}");
        assert_eq!(
            std::fs::read_to_string(bin.path().join("keychain.txt")).unwrap(),
            "adopted\n"
        );
    }

    /// A Linux desktop with no Secret Service: the keys move into private
    /// files, and the agent serves from them.
    #[test]
    fn without_a_keychain_the_upgrade_keeps_the_keys_in_private_files() {
        let _serial = serial();
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        earlier_setup(home.path());
        let found = r#"{"kind":"found","phones":2,"problems":[],"keys":"files"}"#;
        let coder = fake_coder(bin.path(), true, found, ADOPTED);
        let (started, registered) = launch(Some(&coder), home.path());
        assert_eq!(started.agent, Agent::Enabled);
        let dir = home.path().join(".openagents/host-keys");
        assert_eq!(registered, [Keys::Files(dir.clone())]);
        assert!(calls(bin.path()).contains(&format!("host adopt --keys {}", dir.display())));
        assert_eq!(
            Keys::Files(dir.clone()).serve_args(),
            ["--keys".to_string(), dir.display().to_string()]
        );
    }

    #[test]
    fn nothing_to_upgrade_just_registers() {
        let _serial = serial();
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(
            bin.path(),
            true,
            r#"{"kind":"none","keys":"keychain"}"#,
            ADOPTED,
        );
        let (started, registered) = launch(Some(&coder), home.path());
        assert_eq!(started.note, None);
        assert_eq!(registered, [Keys::Keychain]);
        assert!(!calls(bin.path()).contains("host adopt HOME"));
        assert!(!bin.path().join("keychain.txt").exists());
    }

    /// Every refusal leaves the earlier setup running, registers nothing,
    /// and gives one quiet line; never a question.
    #[test]
    fn a_refused_check_keeps_the_earlier_setup_running_with_one_quiet_line() {
        let _serial = serial();
        let problem = r#"{"kind":"found","phones":6,"problems":["in the way"]}"#;
        let failed = r#"{"kind":"failed","message":"the keychain did not return the host key"}"#;
        for (detect, adopt_line, adopts) in [
            (problem, ADOPTED, false),
            (failed, ADOPTED, false),
            ("not json", ADOPTED, false),
            (FOUND, failed, true),
            (FOUND, "", true),
        ] {
            let bin = tempfile::tempdir().unwrap();
            let home = tempfile::tempdir().unwrap();
            earlier_setup(home.path());
            let coder = fake_coder(bin.path(), true, detect, adopt_line);
            let (started, registered) = launch(Some(&coder), home.path());
            assert_eq!(
                started,
                Started {
                    agent: Agent::NotRegistered,
                    note: Some(KEPT_RUNNING.into())
                },
                "{detect} / {adopt_line}"
            );
            assert!(registered.is_empty());
            assert_eq!(bin.path().join("keychain.txt").exists(), adopts);
        }
    }

    /// A `coder` that doesn't read the keychain is never asked to adopt; an
    /// earlier setup stays running, and a computer without one registers.
    #[test]
    fn an_older_coder_never_adopts() {
        let _serial = serial();
        let bin = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let coder = fake_coder(bin.path(), false, FOUND, ADOPTED);
        let (started, registered) = launch(Some(&coder), home.path());
        assert_eq!(started.note, None);
        assert_eq!(registered, [Keys::Keychain]);
        earlier_setup(home.path());
        for coder in [Some(coder.as_path()), None] {
            let (started, registered) = launch(coder, home.path());
            assert_eq!(started.note.as_deref(), Some(KEPT_RUNNING));
            assert!(registered.is_empty());
        }
        assert!(!calls(bin.path()).contains("adopt"));
    }

    #[test]
    #[should_panic(expected = "a test reached the real home")]
    fn a_test_that_reaches_the_real_home_fails() {
        let real = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        let _ = start(None, &real, &mut |_| Agent::NotRegistered);
    }
}
