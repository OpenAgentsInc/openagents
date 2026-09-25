//! Sealing an evaluation run, and reading its sessions afterward for
//! commands that tried to reach the answer.
//!
//! Every entry is a closed issue whose fix is merged, so a session that
//! reads the issue on GitHub, or fetches the repository, can copy the
//! answer. A run is sealed two ways (see [`microluna::seal`]):
//!
//! - **GitHub withheld.** Every session's commands run with no `GH_*` or
//!   `GITHUB_*` variable, an empty `GH_CONFIG_DIR`, no Git credential
//!   helper, and a stub `gh` that refuses.
//! - **Network off.** Every session's commands run in a network namespace
//!   that holds only loopback, with `CARGO_NET_OFFLINE=true`. Before the
//!   flow starts, the host runs `cargo fetch --locked` in the clone, so
//!   Cargo builds and tests from crates already on disk.
//!
//! After the flow, [`scan`] reads every session trace the run left for a
//! command that looks like it reached for GitHub or the network. A refused
//! attempt is recorded and doesn't count against the run; an attempt the
//! seal didn't cover marks the run contaminated.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};

pub(crate) mod toolchain;

/// How a run was sealed, as its manifest records it.
#[derive(Clone, Debug, Serialize)]
pub struct Sealing {
    /// Whether the sessions' commands were cut off from GitHub.
    pub github_withheld: bool,
    /// Whether the sessions' commands ran with no network beyond loopback.
    pub network_off: bool,
    /// What preparing Cargo for an offline run did: `fetched`, `no
    /// Cargo.lock`, `skipped`, or why the fetch failed.
    pub prefetch: String,
    /// The read boundary used by the sessions and the test gate.
    pub read_isolation: &'static str,
    pub readable: Vec<std::path::PathBuf>,
    pub writable_tool_state: Vec<std::path::PathBuf>,
}

impl Sealing {
    /// The sealing in a line: "GitHub withheld, network off".
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "GitHub {}, network {}",
            if self.github_withheld {
                "withheld"
            } else {
                "reachable"
            },
            if self.network_off { "off" } else { "on" }
        )
    }
}

/// Lays the seal out under `dir` and prepares the clone at `repo` for it.
/// With `network_off`, the host fetches the clone's crates first, when
/// `prefetch` is set and the clone has a `Cargo.lock`, and checks that
/// this host can take the network away from a command.
///
/// # Errors
///
/// Returns a message when the seal can't be written, or when the network
/// can't be turned off here.
pub fn prepare(
    dir: &Path,
    repo: &Path,
    network_off: bool,
    prefetch: bool,
) -> Result<(microluna::Seal, Sealing), String> {
    let seal = microluna::Seal::create(dir, network_off)
        .map_err(|error| format!("cannot lay out the seal in {}: {error}", dir.display()))?;
    if network_off {
        offline_works()?;
    }
    let prefetch = if !network_off || !prefetch {
        "skipped".to_string()
    } else if !repo.join("Cargo.lock").is_file() {
        "no Cargo.lock".to_string()
    } else {
        match super::run_in(repo, "cargo", &["fetch", "--locked"]) {
            Ok(_) => "fetched".to_string(),
            Err(why) => format!("failed: {why}"),
        }
    };
    let scope = toolchain::scope(dir)?;
    let readable = scope.readable.clone();
    let writable_tool_state = scope.writable.clone();
    let seal = seal.with_read_scope(scope);
    // Refuse before inference if this host cannot construct the full scope.
    seal.constrain_reads(coder_boundary::Boundary::writing(repo))
        .build()
        .map_err(|error| format!("cannot enforce evaluation reads: {error}"))?;
    Ok((
        seal,
        Sealing {
            github_withheld: true,
            network_off,
            prefetch,
            read_isolation: "candidate-and-toolchain-v1",
            readable,
            writable_tool_state,
        },
    ))
}

/// Whether an offline boundary builds and runs a command on this host.
fn offline_works() -> Result<(), String> {
    let refuse = |why: String| {
        format!(
            "this host can't run the sessions with the network off ({why}); pass --network on \
             to run with GitHub withheld and the network open"
        )
    };
    let boundary = coder_boundary::Boundary::readonly()
        .offline()
        .build()
        .map_err(|error| refuse(error.to_string()))?;
    let status = boundary
        .command("/bin/sh", ["-c", ":"])
        .map_err(|error| refuse(error.to_string()))?
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| refuse(error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(refuse(format!("the sandbox exited with {status}")))
    }
}

/// One command a session ran that looked like it reached for GitHub or
/// the network.
#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    /// The session trace it's in, relative to the run's directory.
    pub trace: String,
    /// The command as the model wrote it.
    pub command: String,
    /// `github` for the GitHub CLI, `network` for anything else that
    /// reaches out: `curl`, `wget`, a Git fetch, pull, or clone of a URL,
    /// or a GitHub address.
    pub reach: &'static str,
    /// Whether the seal stopped it.
    pub blocked: bool,
    /// The start of what came back, so a reader sees the refusal or what
    /// got through.
    pub output: String,
}

/// What [`scan`] found.
#[derive(Clone, Debug, Serialize)]
pub struct Scan {
    pub attempts: Vec<Attempt>,
    /// True when an attempt got past the seal, or might have.
    pub contaminated: bool,
    /// Session traces read.
    pub traces: usize,
}

impl Scan {
    /// The scan as the manifest records it.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "contaminated": self.contaminated,
            "attempts": self.attempts,
            "blocked": self.attempts.iter().filter(|a| a.blocked).count(),
            "traces_read": self.traces,
        })
    }
}

fn patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            (r#"(?:^|[\s;&|()`'"/])gh(?:\s|$)"#, "github"),
            (r"\b(?:curl|wget)\b", "network"),
            (
                r"\bgit\b[^;&|\n]*\b(?:fetch|pull|clone|ls-remote|push)\b[^;&|\n]*(?:[a-z]+://|git@)",
                "network",
            ),
            (r"(?i)github\.com|githubusercontent\.com", "network"),
        ]
        .into_iter()
        .map(|(pattern, reach)| (Regex::new(pattern).expect("a valid pattern"), reach))
        .collect()
    })
}

/// What a command reached for, when it looked like it reached for
/// GitHub or the network: `github` or `network`.
#[must_use]
pub fn reach(command: &str) -> Option<&'static str> {
    patterns()
        .iter()
        .find(|(pattern, _)| pattern.is_match(command))
        .map(|(_, reach)| *reach)
}

/// Reads every session trace under the run's directory `dir`, except the
/// clone, for commands that reached for GitHub or the network, and judges
/// each against how the run was sealed. A GitHub attempt is blocked when
/// GitHub was withheld or the stub answered; a network attempt when the
/// network was off.
#[must_use]
pub fn scan(dir: &Path, sealing: Option<&Sealing>) -> Scan {
    let mut traces = Vec::new();
    collect(dir, &dir.join("repo"), &mut traces);
    traces.sort();
    let mut seen = BTreeSet::new();
    let mut attempts = Vec::new();
    for path in &traces {
        let Ok(recording) = atif::log::read(path) else {
            continue;
        };
        let trace = path.strip_prefix(dir).unwrap_or(path).display().to_string();
        for step in recording.steps {
            let Some(call) = step.call else { continue };
            if call.name != "run_command" {
                continue;
            }
            let Some(command) = call.arguments["command"].as_str() else {
                continue;
            };
            let Some(reach) = reach(command) else {
                continue;
            };
            // The episode log and a session's own trace can hold the same call.
            if !seen.insert((call.id.clone(), command.to_string(), call.output.clone())) {
                continue;
            }
            let blocked = match reach {
                "github" => {
                    sealing.is_some_and(|s| s.github_withheld)
                        || call.output.contains(microluna::seal::GH_REFUSAL)
                }
                _ => sealing.is_some_and(|s| s.network_off),
            };
            attempts.push(Attempt {
                trace: trace.clone(),
                command: command.to_string(),
                reach,
                blocked,
                output: crate::judge::clip(call.output.trim(), 300),
            });
        }
    }
    Scan {
        contaminated: attempts.iter().any(|attempt| !attempt.blocked),
        attempts,
        traces: traces.len(),
    }
}

fn collect(dir: &Path, skip: &Path, into: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect(&path, skip, into);
        } else if kind.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".atif.jsonl"))
        {
            into.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(dir: &Path, name: &str, calls: &[(&str, &str)]) {
        let session = atif::Session::opening(name, "m", "d", "r", "v");
        let mut log = atif::Log::create_at(&dir.join(name), &session).unwrap();
        for (index, (command, output)) in calls.iter().enumerate() {
            log.append(&atif::document::Step::called(atif::Call {
                id: format!("c{index}"),
                name: "run_command".to_string(),
                arguments: json!({ "command": command }),
                output: (*output).to_string(),
                outcome: atif::Outcome::Failed,
                milliseconds: 1,
                purpose: None,
                extra: serde_json::Map::new(),
            }))
            .unwrap();
        }
    }

    /// A refused attempt is recorded and doesn't contaminate the run; one
    /// the seal didn't cover does. The clone is never read.
    #[test]
    fn a_scan_records_attempts_and_marks_what_got_past_the_seal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("artifacts/artifacts")).unwrap();
        std::fs::create_dir_all(dir.path().join("repo")).unwrap();
        trace(
            &dir.path().join("artifacts/artifacts"),
            "microluna-1.atif.jsonl",
            &[
                (
                    "gh issue view 9450",
                    "[exit 1]\n[stderr]\ngh: GitHub access is off during an evaluation run.",
                ),
                ("git status", "[exit 0]"),
                ("curl -s https://api.example.com/x", "[exit 7]"),
            ],
        );
        trace(
            &dir.path().join("repo"),
            "planted.atif.jsonl",
            &[("gh pr view 1", "")],
        );
        let sealed = Sealing {
            github_withheld: true,
            network_off: true,
            prefetch: "skipped".to_string(),
            read_isolation: "candidate-and-toolchain-v1",
            readable: Vec::new(),
            writable_tool_state: Vec::new(),
        };
        let scan = scan(dir.path(), Some(&sealed));
        assert_eq!(scan.traces, 1);
        assert_eq!(scan.attempts.len(), 2, "{:#?}", scan.attempts);
        assert!(scan.attempts.iter().all(|attempt| attempt.blocked));
        assert!(!scan.contaminated);
        assert!(
            scan.attempts[0]
                .output
                .contains(microluna::seal::GH_REFUSAL)
        );

        // The same commands with the network open: the curl got past.
        let open = Sealing {
            network_off: false,
            ..sealed
        };
        let scan = super::scan(dir.path(), Some(&open));
        assert!(scan.contaminated);
        assert_eq!(
            scan.attempts
                .iter()
                .filter(|attempt| !attempt.blocked)
                .map(|attempt| attempt.reach)
                .collect::<Vec<_>>(),
            ["network"]
        );
        // An unsealed run's gh counts as blocked only when the stub answered.
        let unsealed = super::scan(dir.path(), None);
        assert_eq!(unsealed.attempts.iter().filter(|a| a.blocked).count(), 1);
    }

    #[test]
    fn commands_that_reach_out_are_named() {
        for (command, expected) in [
            ("gh issue view 9450", Some("github")),
            ("cd x && gh pr list", Some("github")),
            ("/usr/bin/gh api repos", Some("github")),
            ("curl -s https://example.com", Some("network")),
            ("wget http://example.com/x", Some("network")),
            (
                "git clone https://example.com/OpenAgentsInc/openagents x",
                Some("network"),
            ),
            ("git fetch git@example.com:o/r.git", Some("network")),
            (
                "python3 -c 'import urllib; urllib.request.urlopen(\"https://api.github.com\")'",
                Some("network"),
            ),
            ("git status && git diff", None),
            ("git log --oneline", None),
            ("cargo test -p coder", None),
            ("grep -rn high docs/", None),
            ("ls ghost/", None),
        ] {
            assert_eq!(reach(command), expected, "{command}");
        }
    }
}
