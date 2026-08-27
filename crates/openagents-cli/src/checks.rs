//! Named check scopes for a repository, and a baseline of known failures.
//!
//! The distinction that was missing: "is my diff green" and "does the whole
//! repository pass" are different jobs with different costs, and a session
//! that runs the second whenever it means the first pays minutes for seconds.
//! `.openagents/checks.json` names the scopes; the model asks for one by
//! name. The baseline artifact records what a clean tree already fails, so a
//! mid-loop failure can be attributed — mine, or inherited — without a
//! second full sweep.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The reserved scope the harness runs when nothing else is asked for.
pub const DEFAULT_SCOPE: &str = "diff";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckScope {
    /// What the scope is for, shown when the model lists what it can ask for.
    #[serde(default)]
    pub description: Option<String>,
    /// The command lines to run, in order, through the same `/bin/sh` runner
    /// as every other tool call — same refusal gate, same output ceiling,
    /// same persistence for long runs.
    pub run: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChecksConfig {
    /// Scope name -> what to run. `diff`, `package`, and `full` are
    /// conventions this file is free to rearrange; `diff` is the default the
    /// harness falls back to.
    pub scopes: BTreeMap<String, CheckScope>,
}

impl ChecksConfig {
    /// The repository's declared scopes, or `None` when it declares none.
    ///
    /// Read from `.openagents/checks.json`. A missing file is a repository
    /// that has not opted in — no scopes, no baseline, no behaviour change.
    /// A malformed file is reported, not guessed around.
    pub fn load(repo_root: &Path) -> Result<Option<Self>, String> {
        let path = repo_root.join(".openagents").join("checks.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("could not read {}: {error}", path.display())),
        };
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        if config.scopes.is_empty() {
            return Err(format!("{} declares no scopes", path.display()));
        }
        Ok(Some(config))
    }

    /// The commands a named scope runs, or why the name is no good.
    pub fn scope(&self, name: &str) -> Result<&CheckScope, String> {
        self.scopes.get(name).ok_or_else(|| {
            let names: Vec<&String> = self.scopes.keys().collect();
            format!(
                "no check scope named `{name}`. Declared scopes: {}",
                names
                    .into_iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }
}

/// One entry of the baseline: a check that failed on a clean tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownFailure {
    /// The scope that produced it, so `full`'s failures do not alarm `diff`.
    pub scope: String,
    /// The command that failed.
    pub command: String,
    /// When the baseline was taken, epoch milliseconds.
    pub recorded_at_ms: u64,
    /// First line of the failure output, for a human scanning the file.
    pub summary: String,
}

/// The baseline artifact, beside the session records.
///
/// Written by `record_failures` after an explicit baseline run and read
/// before attributing any later failure. It is evidence, not authority: a
/// failure listed here is *probably* inherited, and the record says when and
/// from what command, never that the code was right.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FailureBaseline {
    /// Tree the baseline was taken against, for a cheap staleness check.
    pub commit: Option<String>,
    pub failures: Vec<KnownFailure>,
}

impl FailureBaseline {
    pub fn path(session_dir: &Path) -> PathBuf {
        session_dir.join("known-failures.json")
    }

    pub fn load(session_dir: &Path) -> Self {
        std::fs::read(FailureBaseline::path(session_dir))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Append failures and write atomically, keeping a refusal rather than
    /// failing the turn over it — the caller decides what a lost baseline
    /// costs, and it is never the turn.
    pub fn store(&self, session_dir: &Path) {
        let path = FailureBaseline::path(session_dir);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        match serde_json::to_vec_pretty(self) {
            Ok(bytes) => {
                if std::fs::write(&temp, bytes).is_ok() {
                    let _ = std::fs::rename(&temp, &path);
                }
            }
            Err(_) => {
                let _ = std::fs::remove_file(&temp);
            }
        }
    }

    /// Whether a failed command was already failing on the clean tree.
    pub fn is_known(&self, scope: &str, command: &str) -> bool {
        self.failures
            .iter()
            .any(|failure| failure.scope == scope && failure.command == command)
    }

    /// Fold one run's failures in, replacing earlier entries for the same
    /// scope+command so the baseline stays one fact per check.
    pub fn record_failures(&mut self, commit: Option<&str>, failures: Vec<KnownFailure>) {
        if let Some(commit) = commit {
            self.commit = Some(commit.to_string());
        }
        for failure in failures {
            self.failures.retain(|existing| {
                existing.scope != failure.scope || existing.command != failure.command
            });
            self.failures.push(failure);
        }
    }
}

/// First meaningful line of a failure, bounded for the artifact.
pub fn summarize(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("(no output)")
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repository_without_checks_declares_no_scopes() {
        let root = tempfile::tempdir().unwrap();
        assert!(ChecksConfig::load(root.path()).unwrap().is_none());
    }

    #[test]
    fn scopes_are_loaded_and_named() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(".openagents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("checks.json"),
            r#"{
                "scopes": {
                    "diff": { "run": ["vp test --run $(git diff --name-only HEAD | grep 'test\\.ts$')"] },
                    "full": { "run": ["pnpm run check", "cargo test --workspace"] }
                }
            }"#,
        )
        .unwrap();
        let config = ChecksConfig::load(root.path()).unwrap().unwrap();
        assert_eq!(config.scope("diff").unwrap().run.len(), 1);
        assert_eq!(config.scope("full").unwrap().run.len(), 2);
        assert!(config.scope("package").is_err());
    }

    #[test]
    fn a_malformed_config_is_an_error_not_an_empty_set() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(".openagents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("checks.json"), "{ not json").unwrap();
        assert!(ChecksConfig::load(root.path()).is_err());
    }

    #[test]
    fn the_baseline_records_replaces_and_classifies() {
        let session = tempfile::tempdir().unwrap();
        let mut baseline = FailureBaseline::default();
        baseline.record_failures(
            Some("abc123"),
            vec![KnownFailure {
                scope: "full".to_string(),
                command: "pnpm run test".to_string(),
                recorded_at_ms: 1,
                summary: summarize("first line\n8 failed"),
            }],
        );
        baseline.record_failures(
            None,
            vec![KnownFailure {
                scope: "full".to_string(),
                command: "pnpm run test".to_string(),
                recorded_at_ms: 2,
                summary: "newer".to_string(),
            }],
        );
        assert_eq!(baseline.failures.len(), 1, "same scope+command replaces");
        assert_eq!(baseline.failures[0].summary, "newer");
        assert!(baseline.is_known("full", "pnpm run test"));
        assert!(!baseline.is_known("diff", "pnpm run test"));

        baseline.store(session.path());
        let loaded = FailureBaseline::load(session.path());
        assert_eq!(loaded, baseline);
    }
}
