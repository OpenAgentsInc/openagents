//! `control.best_of.select: "suite"`: rank candidates by what runs, not
//! by what they report.
//!
//! The combined verdict reads a candidate's final report, and a GPT-6
//! Luna report is short, so on Terminal-Bench the verdict said `unknown`
//! for every candidate and cost broke the tie
//! (`docs/terminal-bench/2026-09-24-best-of-n-luna.md`). The determinism
//! thesis (`docs/coder/design/thesis.md`) says the selection key should be
//! an observed program state instead. This key is one: an executable
//! suite, run on each candidate in the real workspace, and the candidate
//! with the most passing runs ranks first.
//!
//! The suite is a set of commands and a set of test files:
//!
//! - **Commands** are the test and check commands the task names, admitted
//!   by the same rule as `generic.public-command`
//!   ([`generic::refusal`]). With none, and a `tests` directory of
//!   `test_*.py` files the task provided, the suite runs
//!   `python3 -m pytest -q tests`. The commands a candidate's own session
//!   ran are left out: a command that runs a file only one candidate wrote
//!   would favor its author in every column.
//! - **Test files** are the files each candidate added or changed under a
//!   test path, found by comparing its copy with the workspace as the
//!   candidates started. Every candidate's code is run under every other
//!   candidate's tests: column `j` of the matrix overlays candidate `j`'s
//!   test files onto each candidate's copy and runs the commands. Column
//!   `i` for candidate `i` is its own work. A candidate that wrote no test
//!   files contributes a column equal to each candidate's own.
//!
//! A candidate's score is the count of `(column, command)` runs that exit
//! 0, over the columns every candidate completed within the suite's time
//! bound, so a bound that runs out never ranks one candidate on more runs
//! than another. The accept.define suite of #9588 slots in as another
//! source of commands when it lands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::accept;
use crate::checks::{Subject, generic, place::Place};

/// How `control.best_of` ranks its candidates first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Select {
    /// The calibrated verdict on each report, then the checks, then cost.
    #[default]
    Verdict,
    /// The executable suite, then the verdict, then the checks, then cost.
    Suite,
}

impl Select {
    /// Whether this is the default, for serialization.
    #[must_use]
    pub fn is_verdict(&self) -> bool {
        *self == Select::Verdict
    }
}

/// The default time bound for the whole suite, in seconds.
#[must_use]
pub fn suite_sec() -> u64 {
    900
}

/// Whether `sec` is the default bound, for serialization.
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn is_default_sec(sec: &u64) -> bool {
    *sec == suite_sec()
}

/// The most commands one suite runs.
pub const MAX_COMMANDS: usize = 4;

/// The most test files one candidate's column overlays.
pub const MAX_TEST_FILES: usize = 40;

/// The most characters of a run's output the record keeps.
const OUTPUT_CHARS: usize = 600;

/// One command the suite runs, and where it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteCommand {
    pub command: String,
    /// `instruction` or `workspace`.
    pub source: String,
}

/// The suite's commands: the test commands the task names, or a pytest
/// run of a provided `tests` directory when it names none.
#[must_use]
pub fn commands(subject: &Subject, instruction: &str, workdir: &Path) -> Vec<SuiteCommand> {
    let mut named: Vec<String> = Vec::new();
    if let Some(map) = &subject.requirements {
        for requirement in &map.requirements {
            named.extend(requirement.extracted.commands.iter().cloned());
        }
    }
    named.extend(crate::requirements::extract(instruction).commands);
    let mut out: Vec<SuiteCommand> = Vec::new();
    for command in named {
        let text = generic::core(&command);
        if generic::refusal(&command).is_some() || out.iter().any(|c| c.command == text) {
            continue;
        }
        out.push(SuiteCommand {
            command: text,
            source: "instruction".to_string(),
        });
        if out.len() == MAX_COMMANDS {
            break;
        }
    }
    if out.is_empty() && provided_pytest(workdir) {
        out.push(SuiteCommand {
            command: "python3 -m pytest -q tests".to_string(),
            source: "workspace".to_string(),
        });
    }
    out
}

/// Whether `workdir/tests` holds a `test_*.py` file.
fn provided_pytest(workdir: &Path) -> bool {
    std::fs::read_dir(workdir.join("tests")).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("test_") && name.ends_with(".py")
        })
    })
}

/// Whether a relative path reads as a test file: under a `test`, `tests`,
/// `spec`, or `__tests__` directory, or named `test_*`, `*_test.*`,
/// `*.test.*`, or `*_spec.*`.
#[must_use]
pub fn is_test_path(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    let Some(name) = parts.pop() else {
        return false;
    };
    let name = name.to_lowercase();
    let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
    parts
        .iter()
        .any(|p| matches!(*p, "test" | "tests" | "spec" | "__tests__"))
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with(".test")
        || stem.ends_with("_spec")
}

/// A digest of each file under `dir` a search reads, by relative path.
#[must_use]
pub fn digests(dir: &Path) -> BTreeMap<String, String> {
    crate::judge::walk(dir)
        .into_iter()
        .filter_map(|relative| {
            let bytes = std::fs::read(dir.join(&relative)).ok()?;
            let digest = Sha256::digest(&bytes);
            Some((
                relative,
                digest.iter().map(|b| format!("{b:02x}")).collect(),
            ))
        })
        .collect()
}

/// The test files in `copy` that are new or changed against `base`.
#[must_use]
pub fn changed_tests(base: &BTreeMap<String, String>, copy: &Path) -> Vec<String> {
    digests(copy)
        .into_iter()
        .filter(|(path, digest)| is_test_path(path) && base.get(path) != Some(digest))
        .map(|(path, _)| path)
        .take(MAX_TEST_FILES)
        .collect()
}

/// One command's run in one cell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub killed: bool,
    pub milliseconds: u64,
    pub output_tail: String,
}

/// The suite's result.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Matrix {
    pub commands: Vec<SuiteCommand>,
    /// Each candidate's test files: column `j` overlays `tests[j]`.
    pub tests: Vec<Vec<String>>,
    /// `cells[i][j]`: candidate `i`'s code under candidate `j`'s tests, one
    /// run per command, or `None` when it never ran.
    pub cells: Vec<Vec<Option<Vec<Run>>>>,
    /// Why a cell didn't run, when the bound ran out or a copy failed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stopped: Vec<String>,
    pub milliseconds: u64,
}

impl Matrix {
    /// The columns every candidate completed.
    #[must_use]
    pub fn complete(&self) -> Vec<usize> {
        let n = self.cells.len();
        (0..n)
            .filter(|&j| {
                self.cells
                    .iter()
                    .all(|row| row.get(j).is_some_and(Option::is_some))
            })
            .collect()
    }

    /// Candidate `i`'s passing runs and all its runs, over the complete
    /// columns; `None` without commands or complete columns.
    #[must_use]
    pub fn score(&self, i: usize) -> Option<(usize, usize)> {
        let columns = self.complete();
        if self.commands.is_empty() || columns.is_empty() {
            return None;
        }
        let row = self.cells.get(i)?;
        let mut passed = 0;
        let mut total = 0;
        for j in columns {
            for run in row[j].as_ref()? {
                total += 1;
                passed += usize::from(run.passed);
            }
        }
        Some((passed, total))
    }

    /// The record: commands, test files, and each cell's pass count.
    #[must_use]
    pub fn record(&self) -> Value {
        let cells: Vec<Vec<Value>> = self
            .cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        None => Value::Null,
                        Some(runs) => json!(runs),
                    })
                    .collect()
            })
            .collect();
        json!({
            "commands": self.commands,
            "tests": self.tests,
            "complete_columns": self.complete().iter().map(|j| j + 1).collect::<Vec<_>>(),
            "scores": (0..self.cells.len()).map(|i| self.score(i).map(|(p, t)| json!({ "passed": p, "total": t }))).collect::<Vec<_>>(),
            "cells": cells,
            "stopped": self.stopped,
            "milliseconds": self.milliseconds,
        })
    }
}

/// Runs `command` in `workdir`, bounded, without the episode's
/// credentials.
async fn run_command(command: &str, workdir: &Path, scratch: &Path, bound: Duration) -> Run {
    let shell = Place::live().shell(command, &workdir.to_string_lossy(), &[], scratch);
    let Ok(shell) = shell else {
        return Run {
            passed: false,
            exit_code: None,
            killed: false,
            milliseconds: 0,
            output_tail: "no shell on this host".to_string(),
        };
    };
    let ran = crate::minitask::process::run(shell, bound).await;
    let output = crate::support::scrub(&format!("{}{}", ran.stdout, ran.stderr));
    Run {
        passed: ran.code == Some(0) && !ran.killed,
        exit_code: ran.code,
        killed: ran.killed,
        milliseconds: ran.milliseconds,
        output_tail: crate::judge::clip_tail(output.trim(), OUTPUT_CHARS),
    }
}

/// Copies `files` from `from` into `to`, making parent directories.
fn overlay(from: &Path, files: &[String], to: &Path) -> Result<(), String> {
    for file in files {
        let target = to.join(file);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::copy(from.join(file), &target).map_err(|e| format!("cannot copy {file}: {e}"))?;
    }
    Ok(())
}

/// Where the suite runs and how long it may take.
pub struct Bench<'a> {
    pub workdir: &'a Path,
    /// A directory commands may use as scratch.
    pub scratch: &'a Path,
    /// Each command's bound.
    pub command: Duration,
    /// The whole suite's bound.
    pub total: Duration,
}

/// Runs the suite: each candidate's copy (`copies[i]`) is put in the
/// workspace, with each column's test files on top, and every command runs.
/// Puts nothing back: the caller places the candidate it keeps.
pub async fn run(
    bench: &Bench<'_>,
    commands: Vec<SuiteCommand>,
    copies: &[PathBuf],
    tests: Vec<Vec<String>>,
) -> Matrix {
    let started = Instant::now();
    let n = copies.len();
    let mut matrix = Matrix {
        commands,
        tests,
        cells: vec![vec![None; n]; n],
        ..Matrix::default()
    };
    if matrix.commands.is_empty() {
        return matrix;
    }
    // Each candidate's own work first, then each column with test files of
    // its own, so a bound that runs out still leaves the diagonal.
    let mut order: Vec<(usize, usize)> = (0..n).map(|i| (i, i)).collect();
    for j in 0..n {
        if matrix.tests[j].is_empty() {
            continue;
        }
        order.extend((0..n).filter(|&i| i != j).map(|i| (i, j)));
    }
    for (i, j) in order {
        let left = bench.total.saturating_sub(started.elapsed());
        if left.is_zero() {
            matrix.stopped.push(format!(
                "candidate {} under candidate {}'s tests: the suite's {}-second bound ran out",
                i + 1,
                j + 1,
                bench.total.as_secs()
            ));
            continue;
        }
        let placed = super::super::replace_contents(bench.workdir, &copies[i])
            .and_then(|()| overlay(&copies[j], &matrix.tests[j], bench.workdir));
        if let Err(why) = placed {
            matrix.stopped.push(format!(
                "candidate {} under candidate {}'s tests: {why}",
                i + 1,
                j + 1
            ));
            continue;
        }
        let mut runs = Vec::with_capacity(matrix.commands.len());
        for command in &matrix.commands {
            let left = bench.total.saturating_sub(started.elapsed());
            let bound = bench.command.min(left).max(Duration::from_secs(1));
            runs.push(run_command(&command.command, bench.workdir, bench.scratch, bound).await);
        }
        matrix.cells[i][j] = Some(runs);
    }
    // A column with no test files of its own equals each candidate's own
    // work.
    for j in 0..n {
        if matrix.tests[j].is_empty() {
            for i in 0..n {
                if i != j {
                    matrix.cells[i][j] = matrix.cells[i][i].clone();
                }
            }
        }
    }
    matrix.milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    matrix
}

/// An `accept.define` run to make beside the candidates.
pub struct AcceptJob {
    pub map: crate::requirements::RequirementMap,
    pub jev: crate::component::jev::JevMode,
    /// Whether the episode runs in a disposable task container, which is
    /// then the tests' boundary.
    pub container: bool,
    pub model: String,
    pub instruction: String,
    /// Where the suite and the writer's traces go, outside the workspace.
    pub dir: PathBuf,
}

impl AcceptJob {
    /// The bounds: two rounds and $0.20 of writing and verifying, so a
    /// best-of trial stays within cents.
    #[must_use]
    pub fn options() -> accept::Options {
        accept::Options {
            max_rounds: 2,
            spend_usd: 0.2,
            ..accept::Options::default()
        }
    }

    /// Where the tests run.
    #[must_use]
    pub fn runner(&self) -> accept::Local {
        accept::Local {
            confine: if self.container {
                accept::Confine::TaskContainer
            } else {
                accept::Confine::Writing
            },
            test_sec: Self::options().test_sec,
        }
    }
}

/// Writes, verifies, and freezes an acceptance suite for `workdir`, which
/// no candidate touches while they run.
///
/// # Errors
///
/// Why the writer couldn't start, such as a missing Codex login.
pub async fn define_accept(
    job: &AcceptJob,
    workdir: &Path,
    recorder: &crate::record::Recorder,
) -> Result<accept::AcceptanceSuite, String> {
    let wire = crate::micro::codex_wire(&format!("accept-best-of-{}", atif::now_ms()))?;
    let task = accept::Task {
        title: "best-of acceptance suite".to_string(),
        instruction: job.instruction.clone(),
    };
    let writer = accept::MicrolunaWriter {
        transport: &wire,
        config: microluna::Config {
            model: job.model.clone(),
            deadline: Some(Duration::from_secs(600)),
            ..microluna::Config::luna(&format!(
                "accept-writer-{}",
                &accept::sha256(job.instruction.as_bytes())[..16]
            ))
        },
        isolation: if job.container {
            microluna::Isolation::TaskContainer
        } else {
            microluna::Isolation::Boundary
        },
        traces: Some(job.dir.clone()),
        echo: false,
    };
    let suite_dir = job.dir.join("suite");
    let inputs = accept::Inputs {
        task: &task,
        requirements: &job.map,
        evidence: &[],
        workspace: workdir,
        suite_dir: &suite_dir,
        workspace_note: String::new(),
    };
    let suite = accept::define(
        &inputs,
        &writer,
        &job.runner(),
        &job.jev,
        recorder,
        &AcceptJob::options(),
    )
    .await;
    let _ = suite.save(&accept::AcceptanceSuite::record_path(&suite_dir));
    Ok(suite)
}

/// Runs the fan's frozen acceptance suite on each candidate in the real
/// workspace: each candidate's green and total tests, or `None` where it
/// didn't run, and the record.
pub async fn run_accept(
    setup: &super::super::Setup<'_>,
    fan: &super::Fan,
) -> (Vec<Option<(usize, usize)>>, Value) {
    let (Some(defined), Some(job)) = (&fan.accept, &fan.accept_job) else {
        return (Vec::new(), Value::Null);
    };
    let suite = match defined {
        Ok(suite) => suite,
        Err(why) => return (Vec::new(), json!({ "error": why })),
    };
    let mut record = json!({
        "status": suite.status,
        "tests": suite.tests.len(),
        "rejected": suite.rejected.len(),
        "gaps": suite.gaps.len(),
        "digest": suite.digest,
        "start": suite.start.as_ref().map(|r| json!({ "passed": r.passed, "total": r.total })),
        "writer_usd": suite.writer_usd,
        "jev_usd": suite.jev_usd,
        "milliseconds": suite.milliseconds,
    });
    if suite.tests.is_empty() {
        record["note"] = json!("the suite has no accepted tests, so it ranks nothing");
        return (Vec::new(), record);
    }
    let runner = job.runner();
    let mut scores = Vec::new();
    let mut runs = Vec::new();
    for (i, candidate) in fan.candidates.iter().enumerate() {
        let label = format!("candidate-{}", i + 1);
        let placed = if candidate.refused.is_some() {
            Err("its copy was never made".to_string())
        } else {
            super::super::replace_contents(setup.workdir, &candidate.dir)
        };
        let score = match placed {
            Err(why) => {
                runs.push(json!({ "candidate": i + 1, "error": why }));
                None
            }
            Ok(()) => {
                match accept::run(suite, setup.workdir, &runner, Some(setup.recorder), &label).await
                {
                    Ok(result) => {
                        let red: Vec<String> = result
                            .tests
                            .iter()
                            .filter(|t| !t.green)
                            .map(|t| t.id.clone())
                            .collect();
                        runs.push(json!({
                            "candidate": i + 1,
                            "passed": result.passed,
                            "total": result.total,
                            "red": red,
                        }));
                        Some((result.passed, result.total))
                    }
                    Err(tampered) => {
                        runs.push(json!({ "candidate": i + 1, "error": tampered.to_string() }));
                        None
                    }
                }
            }
        };
        scores.push(score);
    }
    let shown: Vec<String> = scores
        .iter()
        .map(|s| s.map_or_else(|| "-".to_string(), |(g, t)| format!("{g}/{t}")))
        .collect();
    println!(
        "  best of {} ▸ acceptance suite of {} test(s): {}",
        scores.len(),
        suite.tests.len(),
        shown.join(", ")
    );
    record["runs"] = json!(runs);
    (scores, record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(passed: bool) -> Run {
        Run {
            passed,
            exit_code: Some(if passed { 0 } else { 1 }),
            killed: false,
            milliseconds: 1,
            output_tail: String::new(),
        }
    }

    fn matrix(cells: Vec<Vec<Option<Vec<bool>>>>) -> Matrix {
        let n = cells.len();
        Matrix {
            commands: vec![SuiteCommand {
                command: "make test".to_string(),
                source: "instruction".to_string(),
            }],
            tests: vec![Vec::new(); n],
            cells: cells
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| cell.map(|runs| runs.into_iter().map(run).collect()))
                        .collect()
                })
                .collect(),
            stopped: Vec::new(),
            milliseconds: 0,
        }
    }

    #[test]
    fn a_score_counts_passing_runs_over_the_complete_columns_only() {
        // The mvcc-lsm-compaction trial's matrix: candidate 1's code
        // passes its own and candidate 2's tests, and fails candidate 3's.
        let m = matrix(vec![
            vec![Some(vec![true]), Some(vec![true]), Some(vec![false])],
            vec![Some(vec![false]), Some(vec![true]), Some(vec![false])],
            vec![Some(vec![false]), Some(vec![true]), Some(vec![true])],
        ]);
        assert_eq!(m.score(0), Some((2, 3)));
        assert_eq!(m.score(1), Some((1, 3)));
        assert_eq!(m.score(2), Some((2, 3)));
        // A column one candidate never ran counts for nobody.
        let m = matrix(vec![
            vec![Some(vec![true]), Some(vec![true])],
            vec![None, Some(vec![false])],
        ]);
        assert_eq!(m.complete(), [1]);
        assert_eq!(m.score(0), Some((1, 1)));
        assert_eq!(m.score(1), Some((0, 1)));
        // No commands, no score.
        let mut empty = matrix(vec![vec![Some(vec![true])]]);
        empty.commands.clear();
        assert_eq!(empty.score(0), None);
    }

    #[test]
    fn test_paths_are_test_directories_and_test_names() {
        for path in [
            "tests/regression_test.cc",
            "test/x.py",
            "src/__tests__/a.js",
            "test_parser.py",
            "pkg/parser_test.go",
            "web/app.test.ts",
            "lib/thing_spec.rb",
        ] {
            assert!(is_test_path(path), "{path}");
        }
        for path in ["src/lsm_db.cc", "attest.py", "contest/main.c", "tests"] {
            assert!(!is_test_path(path), "{path}");
        }
    }

    #[tokio::test]
    async fn the_suite_runs_each_candidate_under_each_candidates_tests() {
        let root = std::env::temp_dir().join(format!(
            "coder-one-suite-test-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        let workdir = root.join("app");
        let base = root.join("base");
        std::fs::create_dir_all(workdir.join("tests")).unwrap();
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(workdir.join("value"), "0").unwrap();
        let before = digests(&workdir);
        // Candidate 1 writes 1 and a test that wants 1; candidate 2 writes
        // 2 and no test.
        let copies: Vec<PathBuf> = (1..=2).map(|i| root.join(format!("c{i}"))).collect();
        for (i, copy) in copies.iter().enumerate() {
            crate::repair::copy_tree(&workdir, copy).unwrap();
            std::fs::write(copy.join("value"), format!("{}", i + 1)).unwrap();
        }
        std::fs::write(
            copies[0].join("tests/test_value.sh"),
            "test \"$(cat value)\" = 1\n",
        )
        .unwrap();
        let tests: Vec<Vec<String>> = copies
            .iter()
            .map(|copy| changed_tests(&before, copy))
            .collect();
        assert_eq!(tests, [vec!["tests/test_value.sh".to_string()], vec![]]);
        let commands = vec![SuiteCommand {
            command: "for t in tests/*.sh; do [ -e \"$t\" ] || continue; sh \"$t\" || exit 1; done"
                .to_string(),
            source: "instruction".to_string(),
        }];
        let bench = Bench {
            workdir: &workdir,
            scratch: &root,
            command: Duration::from_secs(10),
            total: Duration::from_secs(60),
        };
        let m = super::run(&bench, commands, &copies, tests).await;
        // Candidate 1 passes its own test, and candidate 2 fails it.
        assert_eq!(m.score(0), Some((2, 2)), "{}", m.record());
        assert_eq!(m.score(1), Some((1, 2)), "{}", m.record());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
