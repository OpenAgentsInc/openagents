//! Grades a checkout against an entry's checks.
//!
//! A check never sees the episode, and the episode never sees the checks:
//! the grader runs after the issue flow ends, on the checkout the flow
//! left. Test code a check places or splices in is removed again before
//! the next check runs, so each check sees the candidate as it was left.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use regex::Regex;
use serde::Serialize;

use super::{Check, Entry, Inject, Kind, Set, run_in};

/// One check's result.
#[derive(Clone, Debug, Serialize)]
pub struct Checked {
    pub id: String,
    pub group: String,
    pub kind: String,
    pub passed: bool,
    pub detail: String,
    pub milliseconds: u64,
}

/// The grader's verdict on a checkout.
#[derive(Clone, Debug, Serialize)]
pub struct Grade {
    /// `passed` when every check passed, and `failed` otherwise.
    pub verdict: String,
    pub detail: String,
    pub passed: usize,
    pub total: usize,
    /// The paths the candidate changed against the base.
    pub changed: Vec<String>,
    pub checks: Vec<Checked>,
}

impl Grade {
    /// 1 when the grade passed, and 0 otherwise.
    #[must_use]
    pub fn reward(&self) -> f64 {
        if self.verdict == "passed" { 1.0 } else { 0.0 }
    }

    /// Passed and total checks in `group`.
    #[must_use]
    pub fn group(&self, group: &str) -> (usize, usize) {
        let checks: Vec<&Checked> = self.checks.iter().filter(|c| c.group == group).collect();
        (checks.iter().filter(|c| c.passed).count(), checks.len())
    }
}

/// The paths `checkout` changes against `base`: tracked edits, staged or
/// not, and untracked files git doesn't ignore.
///
/// # Errors
///
/// Returns a message when `git` fails.
pub fn changed(checkout: &Path, base: &str) -> Result<Vec<String>, String> {
    let mut paths: Vec<String> = run_in(checkout, "git", &["diff", "--name-only", base])?
        .lines()
        .map(str::to_string)
        .collect();
    for path in run_in(
        checkout,
        "git",
        &["ls-files", "--others", "--exclude-standard"],
    )?
    .lines()
    {
        if !paths.iter().any(|p| p == path) {
            paths.push(path.to_string());
        }
    }
    paths.sort();
    Ok(paths)
}

/// Grades `checkout` against `entry`'s checks. Cargo builds go to
/// `target_dir`.
#[must_use]
pub fn grade(set: &Set, entry: &Entry, checkout: &Path, target_dir: &Path) -> Grade {
    let changed = changed(checkout, &entry.base).unwrap_or_default();
    let checks: Vec<Checked> = entry
        .checks
        .iter()
        .map(|check| {
            let started = Instant::now();
            let (passed, detail) = one(set, check, checkout, &changed, target_dir);
            Checked {
                id: check.id.clone(),
                group: check.group.clone(),
                kind: kind_word(&check.kind).to_string(),
                passed,
                detail,
                milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }
        })
        .collect();
    let passed = checks.iter().filter(|c| c.passed).count();
    let failed: Vec<&str> = checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.id.as_str())
        .collect();
    Grade {
        verdict: if failed.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        detail: if failed.is_empty() {
            format!("all {} checks passed", checks.len())
        } else {
            format!(
                "{passed} of {} checks passed; failed: {}",
                checks.len(),
                failed.join(", ")
            )
        },
        passed,
        total: checks.len(),
        changed,
        checks,
    }
}

fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::OnlyChanged { .. } => "only_changed",
        Kind::Changed { .. } => "changed",
        Kind::Contains { .. } => "contains",
        Kind::Lacks { .. } => "lacks",
        Kind::ChangedContains { .. } => "changed_contains",
        Kind::Links { .. } => "links",
        Kind::Command { .. } => "command",
    }
}

fn regex(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|error| format!("bad pattern {pattern}: {error}"))
}

/// Runs one check: whether it passed, and why.
fn one(
    set: &Set,
    check: &Check,
    checkout: &Path,
    changed: &[String],
    target_dir: &Path,
) -> (bool, String) {
    let result = match &check.kind {
        Kind::OnlyChanged { patterns } => only_changed(patterns, changed),
        Kind::Changed { pattern } => {
            regex(pattern).map(|re| match changed.iter().find(|path| re.is_match(path)) {
                Some(path) => (true, format!("{path} changed")),
                None => (false, format!("no changed path matches {pattern}")),
            })
        }
        Kind::Contains { path, pattern } => file_match(checkout, path, pattern, true),
        Kind::Lacks { path, pattern } => file_match(checkout, path, pattern, false),
        Kind::ChangedContains { paths, pattern } => {
            changed_contains(checkout, changed, paths, pattern)
        }
        Kind::Links { path } => Ok(links(checkout, path)),
        Kind::Command {
            run,
            timeout_secs,
            tests,
            expect,
            files,
            inject,
        } => Ok(command(
            set,
            checkout,
            target_dir,
            &CommandCheck {
                run,
                timeout_secs: *timeout_secs,
                tests: *tests,
                expect,
                files,
                inject: inject.as_ref(),
            },
        )),
    };
    result.unwrap_or_else(|why| (false, why))
}

fn only_changed(patterns: &[String], changed: &[String]) -> Result<(bool, String), String> {
    let patterns: Vec<Regex> = patterns
        .iter()
        .map(|p| regex(p))
        .collect::<Result<_, _>>()?;
    let outside: Vec<&String> = changed
        .iter()
        .filter(|path| !patterns.iter().any(|re| re.is_match(path)))
        .collect();
    Ok(if changed.is_empty() {
        (false, "nothing changed".to_string())
    } else if outside.is_empty() {
        (true, format!("changed only {}", changed.join(", ")))
    } else {
        (
            false,
            format!(
                "also changed {}",
                outside
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    })
}

fn file_match(
    checkout: &Path,
    path: &str,
    pattern: &str,
    want: bool,
) -> Result<(bool, String), String> {
    let re = regex(pattern)?;
    let text = std::fs::read_to_string(checkout.join(path))
        .map_err(|error| format!("cannot read {path}: {error}"))?;
    let found = re.find(&text).map(|m| {
        let line = text[..m.start()].lines().count().max(1);
        format!("{path}:{line} matches {pattern}")
    });
    Ok(match (found, want) {
        (Some(line), true) => (true, line),
        (Some(line), false) => (false, line),
        (None, true) => (false, format!("{path} doesn't match {pattern}")),
        (None, false) => (true, format!("{path} doesn't match {pattern}")),
    })
}

fn changed_contains(
    checkout: &Path,
    changed: &[String],
    paths: &str,
    pattern: &str,
) -> Result<(bool, String), String> {
    let (paths_re, re) = (regex(paths)?, regex(pattern)?);
    for path in changed.iter().filter(|p| paths_re.is_match(p)) {
        if std::fs::read_to_string(checkout.join(path)).is_ok_and(|text| re.is_match(&text)) {
            return Ok((true, format!("{path} matches {pattern}")));
        }
    }
    Ok((
        false,
        format!("no changed file matching {paths} matches {pattern}"),
    ))
}

/// A heading's anchor as GitHub writes it.
fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Whether every relative link in the Markdown file `path` resolves.
fn links(checkout: &Path, path: &str) -> (bool, String) {
    let Ok(text) = std::fs::read_to_string(checkout.join(path)) else {
        return (false, format!("cannot read {path}"));
    };
    let dir = checkout.join(path).parent().map(Path::to_path_buf);
    let mut broken = Vec::new();
    let mut fenced = false;
    let mut count = 0;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find("](") {
            let after = &rest[at + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = &after[..end];
            rest = &after[end..];
            if target.contains("://") || target.starts_with("mailto:") || target.is_empty() {
                continue;
            }
            count += 1;
            let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
            let resolved = if file.is_empty() {
                checkout.join(path)
            } else {
                dir.clone()
                    .unwrap_or_else(|| checkout.to_path_buf())
                    .join(file)
            };
            if !resolved.exists() {
                broken.push(format!("{target} (no such file)"));
                continue;
            }
            if !anchor.is_empty()
                && resolved.is_file()
                && let Ok(linked) = std::fs::read_to_string(&resolved)
                && !linked
                    .lines()
                    .filter(|l| l.starts_with('#'))
                    .any(|l| slug(l.trim_start_matches('#')) == anchor)
            {
                broken.push(format!("{target} (no such heading)"));
            }
        }
    }
    if broken.is_empty() {
        (true, format!("{count} relative links in {path} resolve"))
    } else {
        (false, format!("{path}: {}", broken.join(", ")))
    }
}

struct CommandCheck<'a> {
    run: &'a str,
    timeout_secs: u64,
    tests: bool,
    expect: &'a [String],
    files: &'a std::collections::BTreeMap<String, String>,
    inject: Option<&'a Inject>,
}

/// A file as it was before a check changed it, to put back afterwards.
struct Saved {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

impl Drop for Saved {
    fn drop(&mut self) {
        let _ = match &self.bytes {
            Some(bytes) => std::fs::write(&self.path, bytes),
            None => std::fs::remove_file(&self.path),
        };
    }
}

fn save(path: PathBuf) -> Saved {
    let bytes = std::fs::read(&path).ok();
    Saved { path, bytes }
}

/// The most command output kept in a failing check's detail.
const OUTPUT_KEPT: usize = 2_000;

fn command(set: &Set, checkout: &Path, target_dir: &Path, check: &CommandCheck) -> (bool, String) {
    let mut saved = Vec::new();
    for (to, from) in check.files {
        let path = checkout.join(to);
        saved.push(save(path.clone()));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(error) = std::fs::copy(set.hidden(from), &path) {
            return (false, format!("cannot place {to}: {error}"));
        }
    }
    if let Some(inject) = check.inject {
        let path = checkout.join(&inject.path);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (false, format!("{} is missing", inject.path));
        };
        let Ok(snippet) = std::fs::read_to_string(set.hidden(&inject.snippet)) else {
            return (false, format!("cannot read hidden/{}", inject.snippet));
        };
        saved.push(save(path.clone()));
        if let Err(error) = std::fs::write(&path, splice(&text, &snippet, &inject.after)) {
            return (
                false,
                format!("cannot splice into {}: {error}", inject.path),
            );
        }
    }
    let scratch = std::env::temp_dir().join(format!(
        "coder-one-issue-eval-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let _ = std::fs::create_dir_all(&scratch);
    let output = Command::new("timeout")
        .args([&check.timeout_secs.to_string(), "sh", "-c", check.run])
        .env("CARGO_TARGET_DIR", target_dir)
        .env("ISSUE_EVAL_TMP", &scratch)
        .current_dir(checkout)
        .output();
    let _ = std::fs::remove_dir_all(&scratch);
    drop(saved);
    let output = match output {
        Ok(output) => output,
        Err(error) => return (false, format!("cannot run `{}`: {error}", check.run)),
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let tail = || {
        let start = text.len().saturating_sub(OUTPUT_KEPT);
        let start = (start..text.len())
            .find(|i| text.is_char_boundary(*i))
            .unwrap_or(text.len());
        text[start..].trim().to_string()
    };
    if !output.status.success() {
        let why = if output.status.code() == Some(124) {
            format!("timed out after {} s", check.timeout_secs)
        } else {
            format!("exited {}", output.status)
        };
        return (false, format!("`{}` {why}: {}", check.run, tail()));
    }
    if check.tests {
        let (passed, failed) = test_counts(&text);
        if passed == 0 || failed > 0 {
            return (
                false,
                format!(
                    "`{}` ran {passed} passing and {failed} failing tests: {}",
                    check.run,
                    tail()
                ),
            );
        }
    }
    for pattern in check.expect {
        match regex(pattern) {
            Ok(re) if re.is_match(&text) => {}
            Ok(_) => {
                return (
                    false,
                    format!(
                        "the output of `{}` doesn't match {pattern}: {}",
                        check.run,
                        tail()
                    ),
                );
            }
            Err(why) => return (false, why),
        }
    }
    let (passed, _) = test_counts(&text);
    (
        true,
        if check.tests {
            format!("`{}` passed {passed} tests", check.run)
        } else {
            format!("`{}` succeeded", check.run)
        },
    )
}

/// Splices `snippet` in after the first line of `text` that reads `after`
/// once trimmed, or appends it in a test module of its own.
#[must_use]
pub fn splice(text: &str, snippet: &str, after: &str) -> String {
    let mut out = String::with_capacity(text.len() + snippet.len() + 64);
    let mut done = false;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        if !done && line.trim() == after {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(snippet);
            if !snippet.ends_with('\n') {
                out.push('\n');
            }
            done = true;
        }
    }
    if !done {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n#[cfg(test)]\nmod issue_eval_hidden {\n    use super::*;\n\n");
        out.push_str(snippet);
        out.push_str("}\n");
    }
    out
}

/// Passing and failing tests in `cargo test` output, summed over every
/// `test result:` line.
#[must_use]
pub fn test_counts(output: &str) -> (u64, u64) {
    let mut totals = (0, 0);
    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("test result: ") else {
            continue;
        };
        for part in rest.split(';') {
            let mut words = part.split_whitespace().rev();
            let (Some(label), Some(number)) = (words.next(), words.next()) else {
                continue;
            };
            let number: u64 = number.parse().unwrap_or(0);
            match label {
                "passed" => totals.0 += number,
                "failed" => totals.1 += number,
                _ => {}
            }
        }
    }
    totals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_eval::{ENTRY_SCHEMA, Fix, FrozenIssue};
    use serde_json::json;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=T", "-c", "user.email=t@example.invalid"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn entry(base: &str, checks: serde_json::Value) -> Entry {
        Entry {
            schema: ENTRY_SCHEMA.to_string(),
            id: "t".to_string(),
            category: "docs".to_string(),
            issue: FrozenIssue {
                repository: "o/r".to_string(),
                number: 1,
                title: "t".to_string(),
                body: "b".to_string(),
                created_at: String::new(),
                frozen: String::new(),
            },
            base: base.to_string(),
            fix: Fix {
                commits: vec![base.to_string()],
                head: base.to_string(),
            },
            checks: serde_json::from_value(checks).unwrap(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn test_counts_sum_every_result_line() {
        let output = "test result: ok. 3 passed; 0 failed; 1 ignored\n\
                      test result: FAILED. 2 passed; 1 failed; 0 ignored\n";
        assert_eq!(test_counts(output), (5, 1));
        assert_eq!(test_counts("running 0 tests\n"), (0, 0));
    }

    #[test]
    fn a_snippet_goes_inside_the_tests_module_or_in_one_of_its_own() {
        let text = "fn a() {}\n#[cfg(test)]\nmod tests {\n    use super::*;\n}\n";
        let spliced = splice(text, "    #[test]\n    fn x() {}\n", "mod tests {");
        assert!(spliced.contains("mod tests {\n    #[test]\n    fn x() {}\n    use super::*;"));
        let appended = splice("fn a() {}", "    #[test]\n    fn x() {}\n", "mod tests {");
        assert!(appended.ends_with(
            "mod issue_eval_hidden {\n    use super::*;\n\n    #[test]\n    fn x() {}\n}\n"
        ));
    }

    #[test]
    fn checks_read_the_candidate_against_the_base_and_undo_what_they_place() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/a.md"), "# A\n\nOld line.\n").unwrap();
        std::fs::write(repo.join("docs/b.md"), "# B\n\n## Section two\n").unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let base = run_in(&repo, "git", &["rev-parse", "HEAD"]).unwrap();
        let set_dir = dir.path().join("set");
        std::fs::create_dir_all(set_dir.join("hidden")).unwrap();
        std::fs::write(
            set_dir.join("hidden/check.sh"),
            "grep -q 'New line' docs/a.md\n",
        )
        .unwrap();
        let set = Set {
            dir: set_dir,
            digest: String::new(),
            entries: Vec::new(),
        };
        let entry = entry(
            base.trim(),
            json!([
                { "id": "only", "group": "deliverables", "kind": "only_changed", "patterns": ["^docs/a\\.md$"] },
                { "id": "new", "group": "deliverables", "kind": "contains", "path": "docs/a.md", "pattern": "New line" },
                { "id": "old", "group": "deliverables", "kind": "lacks", "path": "docs/a.md", "pattern": "Old line" },
                { "id": "links", "group": "deliverables", "kind": "links", "path": "docs/a.md" },
                { "id": "script", "group": "fix_tests", "kind": "command",
                  "run": "sh check.sh", "files": { "check.sh": "check.sh" } }
            ]),
        );
        let target = dir.path().join("target");
        let before = grade(&set, &entry, &repo, &target);
        assert_eq!(before.verdict, "failed");
        assert!(
            before.checks.iter().all(|c| c.id == "links" || !c.passed),
            "{before:#?}"
        );
        assert!(!repo.join("check.sh").exists(), "a placed file is removed");

        std::fs::write(
            repo.join("docs/a.md"),
            "# A\n\nNew line, see [B](b.md#section-two).\n",
        )
        .unwrap();
        let after = grade(&set, &entry, &repo, &target);
        assert_eq!(after.verdict, "passed", "{after:#?}");
        assert_eq!(after.changed, vec!["docs/a.md".to_string()]);
        assert_eq!(after.group("fix_tests"), (1, 1));

        std::fs::write(
            repo.join("docs/a.md"),
            "# A\n\nNew line, see [B](b.md#nowhere).\n",
        )
        .unwrap();
        std::fs::write(repo.join("stray.txt"), "x").unwrap();
        let broken = grade(&set, &entry, &repo, &target);
        let failed: Vec<&str> = broken
            .checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(failed, vec!["only", "links"], "{broken:#?}");
    }
}
