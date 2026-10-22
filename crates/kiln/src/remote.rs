//! Remote `cargo` runs: the job a page sends to a Kiln runner, the checks
//! the runner makes before it runs anything, and the summary it reads from
//! Cargo's output.
//!
//! A job travels as the `task` text of a NIP-CJ conversation request
//! (`25900`) sealed to the runner's key, and the result comes back as the
//! `text` of a `26900` result. The runner is a worker on the conversation
//! lane whose "model" runs Cargo; it publishes no beacon, so no Pylon buyer
//! routes chat to it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The job schema.
pub const JOB_V: &str = "openagents.kiln.cargo.v1";
/// The result schema.
pub const RESULT_V: &str = "openagents.kiln.cargo-result.v1";
/// The most file bytes one job may carry.
pub const MAX_FILE_BYTES: usize = 64 * 1024;
/// The most log bytes a result carries.
pub const MAX_LOG_BYTES: usize = 24 * 1024;

/// One remote Cargo run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub v: String,
    /// The commit the files apply to: 40 hex characters.
    pub base: String,
    /// The Cargo subcommand and its arguments, such as
    /// `["test", "-p", "memory-stream"]`.
    pub args: Vec<String>,
    /// Changed or added files, by repository path.
    pub files: BTreeMap<String, String>,
    /// Removed files, by repository path.
    #[serde(default)]
    pub deleted: Vec<String>,
    /// A signed NIP-34 patch (`1617`) to apply instead of `files`: the
    /// runner applies its text with `git apply`, runs the command, and
    /// publishes a NIP-22 check comment on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<nostr::domain::Event>,
    /// Kiln's self-edit: `publish` (test, build, smoke, and publish Kiln's
    /// own page module from these files), `revert` (point back at the last
    /// good build), or `reset` (drop the published build so the host serves
    /// its own). Only keys on the runner's admin list may ask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The changes as a unified diff (`git format-patch` text) instead of
    /// whole files: what a page sends, since a relay frame holds about
    /// 128 KiB and one of Kiln's own files is near 80 KiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// The crates Kiln's self-edit may change and publish.
pub const SELF_CRATES: &[&str] = &["kiln", "kiln-tools"];

/// The actions only admins may ask for.
pub const ACTIONS: &[&str] = &["publish", "revert", "reset"];

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub v: String,
    /// Cargo's exit code; `None` when the run was stopped or refused.
    pub code: Option<i32>,
    /// One line: the test counts, or why the run stopped.
    pub summary: String,
    /// The end of the log, at most [`MAX_LOG_BYTES`].
    pub log: String,
    /// Wall time of the Cargo run.
    pub ms: u64,
    /// The check comment the runner published, for a patch job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
    /// For a self-edit action: the manifest it published and each phase's
    /// milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<serde_json::Value>,
}

/// The repository paths a format-patch text touches (`diff --git a/X b/Y`).
#[must_use]
pub fn patch_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/")
            && let Some((a, b)) = rest.split_once(" b/")
        {
            for p in [a, b] {
                if !paths.iter().any(|q| q == p) {
                    paths.push(p.to_string());
                }
            }
        }
    }
    paths
}

/// One crate a runner tests, with the limits an operator gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateLimit {
    /// The package name, which is also its directory under `crates/`.
    pub name: String,
    /// The wall-clock cap for a run that names this crate, in seconds.
    pub timeout_secs: Option<u64>,
    /// The memory cap for the run's whole process tree, in MiB.
    pub memory_mb: Option<u64>,
}

/// Parse a runner's crate list (`--crates-file`): one crate a line, with
/// optional `timeout=SECS` and `memory=MB`; `#` starts a comment.
///
/// ```text
/// memory-stream
/// atif timeout=300
/// nostr timeout=900 memory=6000
/// ```
///
/// # Errors
///
/// The first line with a bad name or limit.
pub fn parse_crates(text: &str) -> Result<Vec<CrateLimit>, String> {
    let mut out: Vec<CrateLimit> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let name = words.next().unwrap_or_default();
        let plain = !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !plain {
            return Err(format!("line {}: {name} is not a crate name", number + 1));
        }
        let mut limit = CrateLimit {
            name: name.to_string(),
            timeout_secs: None,
            memory_mb: None,
        };
        for word in words {
            let parsed = match word.split_once('=') {
                Some(("timeout", v)) => v.parse().map(|v| limit.timeout_secs = Some(v)),
                Some(("memory", v)) => v.parse().map(|v| limit.memory_mb = Some(v)),
                _ => {
                    return Err(format!(
                        "line {}: {word} is not a limit; use timeout=SECS or memory=MB",
                        number + 1
                    ));
                }
            };
            parsed.map_err(|_| format!("line {}: {word} is not a number", number + 1))?;
        }
        if out.iter().any(|c| c.name == limit.name) {
            return Err(format!("line {}: {name} is listed twice", number + 1));
        }
        out.push(limit);
    }
    Ok(out)
}

/// The two admitted workspace layouts; this is not a caller-supplied path.
pub fn crate_directory(name: &str) -> String {
    if name.starts_with("psionic-") {
        format!("crates/psionic/crates/{name}")
    } else {
        format!("crates/{name}")
    }
}

pub fn cargo_manifest(args: &[String]) -> Result<Option<&'static str>, String> {
    let names = named_crates(args);
    let nested = names.iter().filter(|n| n.starts_with("psionic-")).count();
    if nested > 0 && nested != names.len() {
        return Err("one runner job must stay in one Cargo workspace".into());
    }
    Ok((nested > 0).then_some("crates/psionic/Cargo.toml"))
}

/// The packages changed by paths in the root or nested Psionic workspace.
#[must_use]
pub fn touched_crates<'a>(paths: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for path in paths {
        if let Some(rest) = path
            .strip_prefix("crates/psionic/crates/")
            .or_else(|| path.strip_prefix("crates/"))
            && let Some((name, _)) = rest.split_once('/')
            && !out.iter().any(|c| c == name)
        {
            out.push(name.to_string());
        }
    }
    out.sort();
    out
}

/// The repository paths a reading tool names (`ls`, `cat`, `view`,
/// `grep`), which Kiln fetches the first time when they aren't in the
/// workspace (#12488): normalized, and only plain relative paths.
#[must_use]
pub fn read_paths(argv: &[String]) -> Vec<String> {
    let Some(tool) = argv.first() else {
        return Vec::new();
    };
    let words: Vec<&str> = argv[1..]
        .iter()
        .map(String::as_str)
        .filter(|w| !w.starts_with('-'))
        .collect();
    let named: Vec<&str> = match tool.as_str() {
        "ls" | "cat" => words,
        "view" => words.into_iter().take(1).collect(),
        // The first word is the text to find.
        "grep" => words.into_iter().skip(1).collect(),
        _ => Vec::new(),
    };
    named
        .into_iter()
        .map(|p| p.trim_start_matches("./").trim_end_matches('/'))
        .filter(|p| {
            !p.is_empty()
                && !p.starts_with('/')
                && !p.split('/').any(|part| part == ".." || part.is_empty())
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-/".contains(&b))
        })
        .map(str::to_owned)
        .collect()
}

/// The crates a Cargo command names with `-p`.
#[must_use]
pub fn named_crates(args: &[String]) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == "-p")
        .map(|w| w[1].clone())
        .collect()
}

/// Parse a `cargo` command line from the agent into its arguments.
///
/// Allowed: `cargo test -p CRATE [-- FILTER...]` and
/// `cargo check -p CRATE`, for a crate in `crates`.
///
/// # Errors
///
/// A plain reason for any other command.
pub fn parse_command(argv: &[String], crates: &[String]) -> Result<Vec<String>, String> {
    let args: Vec<String> = argv.iter().skip(1).cloned().collect();
    let sub = args.first().map(String::as_str);
    if !matches!(sub, Some("test" | "check")) {
        return Err("only `cargo test -p CRATE` and `cargo check -p CRATE` run here".into());
    }
    let mut i = 1;
    let mut named = 0;
    while args.get(i).map(String::as_str) == Some("-p") {
        let krate = args.get(i + 1).ok_or("name the crate after -p")?;
        if !crates.contains(krate) {
            return Err(format!(
                "this workspace has only {}; run `cargo test -p {}`",
                crates.join(", "),
                crates.first().map_or("CRATE", String::as_str)
            ));
        }
        named += 1;
        i += 2;
    }
    if named == 0 {
        return Err("name the crate: `cargo test -p CRATE`".into());
    }
    cargo_manifest(&args)?;
    let rest = &args[i..];
    match rest.first().map(String::as_str) {
        None => {}
        Some("--") if sub == Some("test") => {
            if rest[1..]
                .iter()
                .any(|a| a.starts_with('-') && a != "--nocapture")
            {
                return Err("after `--`, give only test name filters or --nocapture".into());
            }
        }
        Some(_) => return Err("no other Cargo flags run here".into()),
    }
    Ok(args)
}

impl Job {
    /// Check a job before running it: the schema, the commit, the
    /// command, and that every file is a plain relative path inside one of
    /// `crates` (by directory `crates/NAME/`), within the size bound.
    ///
    /// # Errors
    ///
    /// A plain reason.
    pub fn check(&self, crates: &[String]) -> Result<(), String> {
        if self.v != JOB_V {
            return Err(format!("unknown job schema {}", self.v));
        }
        if self.base.len() != 40 || !self.base.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("base is not a 40-character commit".into());
        }
        let self_crates: Vec<String> = SELF_CRATES.iter().map(|c| (*c).to_string()).collect();
        let crates = match self.action.as_deref() {
            None => crates,
            Some(action) if ACTIONS.contains(&action) => &self_crates,
            Some(action) => return Err(format!("unknown action {action}")),
        };
        if self.action.is_none() || !self.args.is_empty() {
            let mut argv = vec!["cargo".to_string()];
            argv.extend(self.args.iter().cloned());
            parse_command(&argv, crates)?;
        }
        let bytes: usize = self.files.values().map(String::len).sum::<usize>()
            + self.diff.as_ref().map_or(0, String::len);
        if bytes > MAX_FILE_BYTES {
            return Err(format!("the files are over {} KiB", MAX_FILE_BYTES / 1024));
        }
        let mut touched: Vec<String> = self.files.keys().chain(&self.deleted).cloned().collect();
        if let Some(diff) = &self.diff {
            touched.extend(patch_paths(diff));
        }
        if let Some(patch) = &self.patch {
            if !self.files.is_empty() || !self.deleted.is_empty() || self.diff.is_some() {
                return Err("a patch job carries no other files".into());
            }
            if patch.kind != crate::patch::PATCH_KIND || patch.validate_crypto().is_err() {
                return Err("the patch is not a signed kind 1617 event".into());
            }
            if patch.content.len() > crate::patch::MAX_PATCH_BYTES {
                return Err("the patch is over 60 KB".into());
            }
            touched = patch_paths(&patch.content);
            if touched.is_empty() {
                return Err("the patch changes no files".into());
            }
            // The check comment vouches for the whole patch, so its run
            // tests every crate the patch changes.
            let named = named_crates(&self.args);
            let missing: Vec<String> = touched_crates(&touched)
                .into_iter()
                .filter(|c| !named.contains(c))
                .collect();
            if !missing.is_empty() {
                let mut all = named;
                all.extend(missing.iter().cloned());
                return Err(format!(
                    "the patch also changes {}; test it too: cargo test -p {}",
                    missing.join(", "),
                    all.join(" -p ")
                ));
            }
        }
        for path in &touched {
            let inside = crates
                .iter()
                .any(|c| path.starts_with(&format!("{}/", crate_directory(c))));
            let plain = path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
                && !path.starts_with('/');
            if !inside || !plain {
                return Err(format!("{path} is outside the crates this runner tests"));
            }
            if path.ends_with("/build.rs") || path.ends_with("Cargo.toml") {
                return Err(format!("{path} can't be changed here"));
            }
        }
        Ok(())
    }
}

/// Read the test counts and the failures from Cargo's output.
#[must_use]
pub fn summarize(code: Option<i32>, log: &str) -> String {
    let (mut passed, mut failed, mut results) = (0u64, 0u64, 0u32);
    for line in log.lines() {
        let Some(rest) = line.trim().strip_prefix("test result: ") else {
            continue;
        };
        results += 1;
        for part in rest.split(';') {
            let words: Vec<&str> = part.split_whitespace().collect();
            let count = |label: &str| {
                words
                    .windows(2)
                    .find(|w| w[1] == label)
                    .and_then(|w| w[0].parse::<u64>().ok())
            };
            passed += count("passed").unwrap_or(0);
            failed += count("failed").unwrap_or(0);
        }
    }
    let errors = log
        .lines()
        .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
        .count();
    match (code, results) {
        (Some(0), 0) => "finished without errors".into(),
        (Some(0), _) => format!("tests passed: {passed} passed, 0 failed"),
        (Some(_), 0) if errors > 0 => format!("did not compile: {errors} errors"),
        (Some(_), 0) => "failed before any test ran".into(),
        (Some(_), _) => format!("tests failed: {passed} passed, {failed} failed"),
        (None, _) => "stopped".into(),
    }
}

/// Keep the part of a Cargo log that matters: compile errors and test
/// failures first, then the tail, within `max` bytes.
#[must_use]
pub fn trim_log(log: &str, max: usize) -> String {
    let noise = |l: &str| {
        let t = l.trim_start();
        t.starts_with("Compiling ")
            || t.starts_with("Checking ")
            || t.starts_with("Downloaded ")
            || t.starts_with("Blocking ")
            || t.starts_with("Running ")
            || t.starts_with("Doc-tests ")
            || t.starts_with("Fresh ")
            || t.starts_with("Locking ")
            || t.starts_with("Adding ")
            || (t.starts_with("test ") && t.ends_with(" ... ok"))
            || t.is_empty()
    };
    let kept: Vec<&str> = log.lines().filter(|l| !noise(l)).collect();
    crate::agent::clip(&kept.join("\n"), max)
}

#[cfg(test)]
mod tests {
    #[test]
    fn nested_psionic_jobs_use_their_workspace_and_stay_inside_it() {
        use super::*;
        let name = "psionic-backend-cpu".to_string();
        let path = "crates/psionic/crates/psionic-backend-cpu/src/lib.rs".to_string();
        let mut j = Job {
            v: JOB_V.into(),
            base: "22".repeat(20),
            args: vec!["test".into(), "-p".into(), name.clone()],
            files: [(path.clone(), "source".into())].into(),
            deleted: vec![],
            patch: None,
            action: None,
            diff: None,
        };
        assert_eq!(touched_crates([&path]), [name.clone()]);
        assert_eq!(
            cargo_manifest(&j.args).unwrap(),
            Some("crates/psionic/Cargo.toml")
        );
        j.check(std::slice::from_ref(&name)).unwrap();
        j.files
            .insert("crates/pylon/src/lib.rs".into(), "outside".into());
        assert!(j.check(std::slice::from_ref(&name)).is_err());
        assert!(
            cargo_manifest(&[
                "test".into(),
                "-p".into(),
                name,
                "-p".into(),
                "pylon".into()
            ])
            .is_err()
        );
    }
    use super::*;

    #[test]
    fn reading_tools_name_their_paths() {
        let argv = |line: &str| line.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert_eq!(read_paths(&argv("ls -R crates/atif/")), ["crates/atif"]);
        assert_eq!(
            read_paths(&argv("view crates/atif/src/lib.rs 1 40")),
            ["crates/atif/src/lib.rs"]
        );
        assert_eq!(
            read_paths(&argv("grep -n Step crates/atif ./docs")),
            ["crates/atif", "docs"]
        );
        assert!(read_paths(&argv("grep Step")).is_empty());
        assert!(read_paths(&argv("cat ../secret /etc/passwd a//b")).is_empty());
        assert!(read_paths(&argv("write crates/x.rs")).is_empty());
    }

    fn crates() -> Vec<String> {
        vec!["memory-stream".to_string()]
    }

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn only_test_and_check_on_listed_crates_run() {
        assert!(parse_command(&words("cargo test -p memory-stream"), &crates()).is_ok());
        assert!(parse_command(&words("cargo test -p memory-stream -- recency"), &crates()).is_ok());
        assert!(parse_command(&words("cargo check -p memory-stream"), &crates()).is_ok());
        let two = vec!["kiln".to_string(), "kiln-tools".to_string()];
        assert!(parse_command(&words("cargo test -p kiln -p kiln-tools"), &two).is_ok());
        assert!(parse_command(&words("cargo test -p kiln -p nostr"), &two).is_err());
        assert!(parse_command(&words("cargo run -p memory-stream"), &crates()).is_err());
        assert!(parse_command(&words("cargo test -p nostr"), &crates()).is_err());
        assert!(parse_command(&words("cargo test -p memory-stream --release"), &crates()).is_err());
        assert!(
            parse_command(
                &words("cargo test -p memory-stream -- --exact x"),
                &crates()
            )
            .is_err()
        );
    }

    #[test]
    fn a_job_stays_inside_its_crate() {
        let mut job = Job {
            v: JOB_V.into(),
            base: "a".repeat(40),
            args: words("test -p memory-stream"),
            files: BTreeMap::from([("crates/memory-stream/src/lib.rs".into(), "x".into())]),
            deleted: vec![],
            patch: None,
            action: None,
            diff: None,
        };
        assert!(job.check(&crates()).is_ok());
        job.files.insert(
            "crates/memory-stream/../nostr/src/lib.rs".into(),
            "x".into(),
        );
        assert!(job.check(&crates()).is_err());
        job.files.clear();
        job.files
            .insert("crates/memory-stream/build.rs".into(), "x".into());
        assert!(job.check(&crates()).is_err());
        job.files.clear();
        job.action = Some("publish".into());
        job.args.clear();
        job.files
            .insert("crates/kiln/src/agent.rs".into(), "x".into());
        assert!(job.check(&crates()).is_ok(), "a publish touches only Kiln");
        job.files
            .insert("crates/memory-stream/src/lib.rs".into(), "x".into());
        assert!(job.check(&crates()).is_err());
        job.files.clear();
        job.action = Some("deploy".into());
        assert!(job.check(&crates()).is_err());
        job.action = None;
        job.args = words("test -p memory-stream");
        job.base = "main".into();
        assert!(job.check(&crates()).is_err());
    }

    #[test]
    fn a_crate_list_reads_names_and_limits() {
        let list = parse_crates(
            "# crates\nmemory-stream\natif timeout=300\nnostr timeout=900 memory=6000\n",
        )
        .unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].timeout_secs, Some(300));
        assert_eq!(list[2].memory_mb, Some(6000));
        assert!(parse_crates("../etc").is_err());
        assert!(parse_crates("atif speed=9").is_err());
        assert!(parse_crates("atif timeout=soon").is_err());
        assert!(parse_crates("atif\natif").is_err());
    }

    #[test]
    fn a_patch_job_tests_every_crate_it_changes() {
        let key = crate::cj::Key::from_bytes([3; 32]).unwrap();
        let text = "diff --git a/crates/atif/src/lib.rs b/crates/atif/src/lib.rs\n--- a\n+++ b\ndiff --git a/crates/xp-ledger/src/lib.rs b/crates/xp-ledger/src/lib.rs\n";
        let patch = key.sign(crate::patch::PATCH_KIND, vec![], text.into(), 1);
        let listed: Vec<String> = ["atif", "xp-ledger"].map(String::from).to_vec();
        let mut job = Job {
            v: JOB_V.into(),
            base: "a".repeat(40),
            args: words("test -p atif"),
            files: BTreeMap::new(),
            deleted: vec![],
            patch: Some(patch),
            action: None,
            diff: None,
        };
        let refused = job.check(&listed).unwrap_err();
        assert!(
            refused.contains("cargo test -p atif -p xp-ledger"),
            "{refused}"
        );
        job.args = words("test -p atif -p xp-ledger");
        assert!(job.check(&listed).is_ok());
        assert!(job.check(&listed[..1]).is_err(), "xp-ledger isn't listed");
        assert_eq!(
            touched_crates(&patch_paths(text)),
            vec!["atif".to_string(), "xp-ledger".to_string()]
        );
    }

    #[test]
    fn cargo_output_reads_as_a_summary() {
        let log = "running 3 tests\ntest a ... ok\ntest b ... FAILED\n\ntest result: FAILED. 2 passed; 1 failed; 0 ignored\n";
        assert_eq!(
            summarize(Some(101), log),
            "tests failed: 2 passed, 1 failed"
        );
        let ok = "test result: ok. 4 passed; 0 failed\ntest result: ok. 1 passed; 0 failed\n";
        assert_eq!(summarize(Some(0), ok), "tests passed: 5 passed, 0 failed");
        let broken = "error[E0425]: cannot find value `x`\nerror: could not compile\n";
        assert_eq!(summarize(Some(101), broken), "did not compile: 2 errors");
        let patch = "diff --git a/crates/x/src/lib.rs b/crates/x/src/lib.rs\n--- a\n";
        assert_eq!(patch_paths(patch), vec!["crates/x/src/lib.rs"]);
        let trimmed = trim_log("   Compiling a v1\ntest a ... ok\ntest b ... FAILED\n", 100);
        assert_eq!(trimmed, "test b ... FAILED");
    }
}
