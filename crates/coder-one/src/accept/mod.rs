//! `accept.define`: an executable acceptance suite, written and proven
//! before any fix, then frozen and run until green (issue #9588).
//!
//! The Luna pivot (`docs/coder/design/thesis.md`) replaces "the model says
//! it's done" with an observed program state. This module builds that
//! state in five steps:
//!
//! 1. **Write.** A Microluna session ([`writer::MicrolunaWriter`]) writes
//!    one shell test per file under `tests/` in a suite directory outside
//!    the solution workspace. Each test names the requirement IDs it
//!    decides in a header. The session's only writable directory is the
//!    suite directory, so it can't touch the solution.
//! 2. **Verify.** Code runs every test on the untouched workspace, and a
//!    test that passes there is rejected unless Jev reads its requirement
//!    as one that keeps something already true. Jev asks, per test,
//!    whether it asserts only what the task states, whether it hardcodes
//!    an answer the task doesn't give, and whether it could pass without
//!    its requirement met; and, per requirement, whether its tests decide
//!    it ([`verify`]). Problems go back to a new writing session, within
//!    [`Options::max_rounds`] and [`Options::spend_usd`]. What's still
//!    wrong after the last round is rejected, and a requirement left
//!    without a deciding test is a named gap that marks the suite partial.
//! 3. **Freeze.** The suite's files are digested ([`AcceptanceSuite::digest`]),
//!    and [`AcceptanceSuite::integrity`] reports any edit, addition, or
//!    removal since.
//! 4. **Run.** [`run`] refuses a tampered suite and otherwise returns red
//!    and green per test and per requirement ([`RunResult`]), which is what
//!    the loop's "done" and next-step decisions read.
//! 5. **Record.** `accept.define` and each `accept.run` are component
//!    invocations in the episode's recorder, and each leaves an ATIF step
//!    carrying the suite ([`SUITE_EXTENSION`]) or the run
//!    ([`RUN_EXTENSION`]), so the Gym sees the suite and its red and green
//!    history.
//!
//! A test is a POSIX shell script. It runs with the workspace root as its
//! working directory, `$ACCEPT_DIR` set to the suite directory,
//! `$WORKSPACE` to the workspace root, and `$ACCEPT_TMP` to an empty
//! scratch directory. Exit 0 is green; anything else is red. Where the
//! tests run is a [`runner::Runner`]: this host inside a `coder-boundary`
//! boundary, a task container directly, or a Docker image for the offline
//! validity measurement.

pub mod cli;
pub mod minitask;
pub mod offline;
pub mod runner;
pub mod verify;
pub mod writer;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::component::jev::JevMode;
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::requirements::{Kind, RequirementMap};

pub use runner::{Confine, Docker, Local, Rebased, Runner};
pub use writer::{MicrolunaWriter, Writer, Written};

/// The schema of a frozen suite's record.
pub const SCHEMA: &str = "openagents.coder-one.acceptance-suite.v1";

/// The schema of one run of a frozen suite.
pub const RUN_SCHEMA: &str = "openagents.coder-one.acceptance-run.v1";

/// The component that writes, verifies, and freezes a suite.
pub const DEFINE_COMPONENT: &str = "accept.define";

/// The component that runs a frozen suite.
pub const RUN_COMPONENT: &str = "accept.run";

/// The ATIF step extension that carries a frozen suite.
pub const SUITE_EXTENSION: &str = "accept.suite.v1";

/// The ATIF step extension that carries one run's red and green state.
pub const RUN_EXTENSION: &str = "accept.run.v1";

/// Where the tests live, relative to the suite directory.
pub const TESTS_DIR: &str = "tests";

/// Where rejected tests are moved, relative to the suite directory. They
/// stay in the digest but never run.
pub const REJECTED_DIR: &str = "rejected";

/// The host's helper scripts for the writing session: `env.sh` runs a
/// command in the workspace, and `run.sh` runs the suite as the host does.
/// They're replaced at the freeze by a `run.sh` for the edit sessions.
pub const HARNESS: [&str; 2] = ["run.sh", "env.sh"];

/// Where the writer lists the decisive facts, relative to the suite
/// directory: one per line, as `R3: the fact`.
pub const FACTS: &str = "facts.md";

/// The most output kept from one test's run.
pub const OUTPUT_CHARS: usize = 1_500;

/// The task, in its own words.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub title: String,
    pub instruction: String,
}

/// What `accept.define` is given.
pub struct Inputs<'a> {
    pub task: &'a Task,
    pub requirements: &'a RequirementMap,
    /// Evidence code already gathered, such as probe outputs.
    pub evidence: &'a [microluna::Evidence],
    /// The untouched solution workspace, as the host sees it.
    pub workspace: &'a Path,
    /// Where the suite is written: a directory outside the workspace. It
    /// is created, and anything in it is replaced.
    pub suite_dir: &'a Path,
    /// How the writing session reaches the workspace, in a sentence, such
    /// as "the directory /app, readable with `sh env.sh`". Empty for the
    /// default wording.
    pub workspace_note: String,
    /// The workspace the frozen suite runs on, when it isn't `workspace`.
    /// Set, `workspace` is a snapshot of it taken before anything edited
    /// it: the red-first proof runs on the snapshot, with every mention of
    /// this path in the suite read as the snapshot's ([`Rebased`]), while
    /// another session edits this one; the freeze then points the suite
    /// here.
    pub target: Option<&'a Path>,
}

/// The bounds and thresholds of `accept.define`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// The most write-and-verify rounds.
    pub max_rounds: u32,
    /// The spend bound in dollars, Microluna's list price plus Jev's. A
    /// round starts only while spend is under it.
    pub spend_usd: f64,
    /// The most tests one suite keeps; the rest are rejected.
    pub max_tests: usize,
    /// One test's wall-time bound, in seconds.
    pub test_sec: u64,
    /// A test is unfaithful below this probability that it asserts only
    /// what the task states.
    pub faithful_min: f64,
    /// A test hardcodes an answer at or above this probability.
    pub hardcoded_max: f64,
    /// A test is trivial at or above this probability.
    pub trivial_max: f64,
    /// A green-at-start test is kept as a guard at or above this
    /// probability that its requirement keeps something already true.
    pub keeps_min: f64,
    /// A requirement is decided at or above this probability that its
    /// tests would fail without it.
    pub decides_min: f64,
    /// A requirement's tests check its rule exactly at or above this
    /// probability.
    pub exact_min: f64,
    /// Jev requests sent at once.
    pub jev_parallel: usize,
    /// Writing sessions in the first round, each on its own share of the
    /// requirements and at the same time. More than one splits the
    /// decidable requirements into that many consecutive groups, merges
    /// the suites they write into one (tests renumbered, facts kept), and
    /// verifies the merged suite once. Later rounds fix it with one
    /// session, as before.
    #[serde(default = "one_writer")]
    pub writers: usize,
    /// What sends a suite back to a writer after a round.
    #[serde(default)]
    pub rewrite: Rewrite,
    /// With `rewrite: "hard"`, a later round's model requests: a short,
    /// targeted session on the flagged tests. `None` for the writer's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_turns: Option<usize>,
    /// Tell the writer to write every test in one pass and then run the
    /// suite once ([`ONE_PASS`]), instead of patching test by test with
    /// `run.sh` in between.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub one_pass: bool,
    /// Tell the writer to find the deciding facts by reading the code's
    /// documentation and running property probes on the untouched code
    /// ([`DISCOVER`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub discover: bool,
}

fn one_writer() -> usize {
    1
}

/// What sends a suite back to a writer after a round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rewrite {
    /// Every problem: code's and Jev's. Jev's faithfulness, hardcoding,
    /// triviality, and coverage judgments reject tests and name gaps, and
    /// each rejection goes back to a fresh round.
    #[default]
    Any,
    /// Only hard failures code checks: a test that doesn't fail on the
    /// untouched workspace, one that doesn't run or fails in the harness,
    /// one that can't fail, and a suite with no tests. Jev's judgments
    /// become notes on the tests, which the edit sessions read, and a
    /// later round is a short session on the flagged tests that keeps the
    /// first round's prefix and findings.
    Hard,
}

/// What a writer is told with [`Options::discover`]: find the deciding
/// facts by reading the code's own documentation and by running small
/// probes on the untouched code, instead of guessing them.
pub const DISCOVER: &str = "Find the deciding facts by reading and running, not by guessing.
- Read every module the task touches in full: its docstrings, comments, parameter names, and \
defaults. Each property they state, such as a function being symmetric, ignoring scale, keeping \
order, mapping a zero input to zeros, or using an inclusive threshold, is a candidate fact. Where \
the task and a docstring disagree, the task wins; where the documentation names a formula or \
estimator, check it against the task's words.
- Before you write a test, run small property probes on the untouched code with env.sh: a sample \
against itself, two samples drawn from one distribution, scaled inputs, zero vectors, empty \
inputs, and extreme values. Note what the code does now, then decide what the task and the \
documentation say it should do. Encode the property they imply, never the current behavior just \
because the code does it.
- For every statistic, estimator, distance, or formula the task names, write a null test: on \
inputs whose true answer is known, such as two samples from one distribution, check the property \
a correct implementation must have.
List each fact in facts.md with where you found it: the task, a docstring, or a probe.";

/// What a writer is told with [`Options::one_pass`].
pub const ONE_PASS: &str = "Work in one pass: read what you need, write every test file, then \
run the whole suite once with `sh run.sh` and fix only what that run shows is broken. Don't run \
the suite after each test you write.";

impl Default for Options {
    fn default() -> Self {
        Options {
            max_rounds: 3,
            spend_usd: 1.0,
            max_tests: 40,
            test_sec: 120,
            // None of these is calibrated yet, so a Jev judgment rejects a
            // test or names a gap only when it's clear.
            faithful_min: 0.25,
            hardcoded_max: 0.7,
            trivial_max: 0.7,
            keeps_min: 0.5,
            decides_min: 0.3,
            exact_min: 0.3,
            jev_parallel: 6,
            writers: 1,
            rewrite: Rewrite::Any,
            repair_turns: None,
            one_pass: false,
            discover: false,
        }
    }
}

/// One acceptance test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Test {
    /// The file's stem, such as `T3`.
    pub id: String,
    /// The requirement IDs it decides, from its `# requirement:` header.
    pub requirements: Vec<String>,
    /// `example`, `edge`, `format`, `location`, `error`, or what the
    /// writer said.
    pub kind: String,
    /// What it asserts, from its `# what:` header.
    pub what: String,
    /// Its path, relative to the suite directory.
    pub path: String,
    /// Its source, for Jev; not recorded, since the digest covers it.
    #[serde(skip)]
    pub source: String,
    /// Jev's doubts about it that didn't reject it, with
    /// [`Rewrite::Hard`]: an edit session reads them beside the test.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// One test's run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestRun {
    pub id: String,
    pub requirements: Vec<String>,
    /// Exit 0.
    pub green: bool,
    /// The exit code, or `None` when a signal or the deadline ended it.
    pub exit: Option<i32>,
    /// The runner killed it at its deadline.
    pub killed: bool,
    pub milliseconds: u64,
    /// The tail of its standard output and error.
    pub output: String,
    /// It was red, then green when [`run`] ran it again: its green is
    /// the rerun's, and it may depend on timing or load.
    #[serde(default)]
    pub flaky: bool,
}

/// A requirement's state in one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequirementRun {
    pub id: String,
    pub green: usize,
    pub red: usize,
    /// `green` when every test of it passed, `red` when any failed, and
    /// `untested` when it has none.
    pub state: String,
}

/// One run of a suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunResult {
    pub schema: String,
    /// What was run, such as `start` or `after session 3`.
    pub label: String,
    /// The suite digest the run used.
    pub digest: String,
    /// Every test passed, and there was at least one.
    pub green: bool,
    /// Green, and the suite has no gaps: every requirement it must
    /// decide has a test. This, not `green`, is "done".
    #[serde(default)]
    pub complete: bool,
    /// The suite's gaps, the requirements no test decides.
    #[serde(default)]
    pub gaps: Vec<String>,
    pub passed: usize,
    pub total: usize,
    pub tests: Vec<TestRun>,
    pub requirements: Vec<RequirementRun>,
    pub milliseconds: u64,
}

impl RunResult {
    /// A run of `tests` over the requirement IDs in `ids`.
    #[must_use]
    pub fn of(label: &str, digest: &str, ids: &[String], tests: Vec<TestRun>, ms: u64) -> Self {
        let passed = tests.iter().filter(|t| t.green).count();
        let requirements = ids
            .iter()
            .map(|id| {
                let mine: Vec<&TestRun> = tests
                    .iter()
                    .filter(|t| t.requirements.contains(id))
                    .collect();
                let green = mine.iter().filter(|t| t.green).count();
                let red = mine.len() - green;
                let state = if mine.is_empty() {
                    "untested"
                } else if red == 0 {
                    "green"
                } else {
                    "red"
                };
                RequirementRun {
                    id: id.clone(),
                    green,
                    red,
                    state: state.to_string(),
                }
            })
            .collect();
        RunResult {
            schema: RUN_SCHEMA.to_string(),
            label: label.to_string(),
            digest: digest.to_string(),
            green: !tests.is_empty() && passed == tests.len(),
            complete: false,
            gaps: Vec::new(),
            passed,
            total: tests.len(),
            tests,
            requirements,
            milliseconds: ms,
        }
    }

    /// The requirement IDs with a red test, in order.
    #[must_use]
    pub fn red_requirements(&self) -> Vec<String> {
        self.requirements
            .iter()
            .filter(|r| r.state == "red")
            .map(|r| r.id.clone())
            .collect()
    }

    /// The red tests as lines an edit session reads in its state: the
    /// test, its requirements, and the tail of what it printed.
    #[must_use]
    pub fn red_lines(&self, suite: &AcceptanceSuite, max_output: usize) -> Vec<String> {
        let mut lines = vec![format!(
            "The frozen acceptance suite: {} of {} tests green.",
            self.passed, self.total
        )];
        for run in self.tests.iter().filter(|t| !t.green) {
            let what = suite
                .tests
                .iter()
                .find(|t| t.id == run.id)
                .map(|t| t.what.clone())
                .unwrap_or_default();
            let exit = if run.killed {
                "timed out".to_string()
            } else {
                run.exit
                    .map_or("ended by a signal".to_string(), |c| format!("exit {c}"))
            };
            lines.push(format!(
                "{} ({}) is red, {exit}: {what}\n{}",
                run.id,
                run.requirements.join(", "),
                crate::judge::clip_tail(run.output.trim(), max_output)
            ));
        }
        lines
    }
}

/// Why a test was rejected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub id: String,
    pub requirements: Vec<String>,
    pub reasons: Vec<String>,
}

/// A requirement's coverage in the frozen suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    pub id: String,
    pub kind: String,
    /// The accepted tests that name it.
    pub tests: Vec<String>,
    /// Jev's probability that the tests would fail without it.
    pub decides: Option<f64>,
    /// Jev's probability that the tests check its rule exactly, not a
    /// simplification.
    #[serde(default)]
    pub exact: Option<f64>,
    pub covered: bool,
}

/// A requirement with no accepted test that decides it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub requirement: String,
    pub why: String,
}

/// One write-and-verify round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Round {
    pub number: u32,
    pub writer: Written,
    pub tests: usize,
    /// Tests green on the untouched workspace.
    pub green_at_start: usize,
    /// The problems sent to the next round, or left at the end.
    pub problems: Vec<String>,
    pub gaps: Vec<Gap>,
    pub jev_requests: usize,
    pub jev_usd: f64,
    /// The writing sessions that ran at the same time in this round, when
    /// there were several; `writer` then sums them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
}

/// One of several writing sessions in a round, and how its tests joined
/// the suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub writer: Written,
    /// The requirements it wrote tests for.
    pub requirements: Vec<String>,
    /// Its tests' IDs and the IDs they have in the merged suite.
    pub renumbered: Vec<(String, String)>,
    /// Its helper files renamed so they don't overwrite another part's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renamed: Vec<(String, String)>,
}

/// A suite's state: `accepted` when every requirement is decided, and
/// `partial` when some are named gaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Accepted,
    Partial,
}

/// The frozen acceptance suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceSuite {
    pub schema: String,
    /// The suite directory.
    pub dir: PathBuf,
    pub status: Status,
    /// SHA-256 of the task's instruction.
    pub instruction_sha256: String,
    /// The accepted tests, in order.
    pub tests: Vec<Test>,
    pub rejected: Vec<Rejected>,
    pub coverage: Vec<Coverage>,
    pub gaps: Vec<Gap>,
    /// The run on the untouched workspace, over the accepted tests.
    pub start: Option<RunResult>,
    /// Every file in the suite directory and its SHA-256, at the freeze.
    pub files: BTreeMap<String, String>,
    /// The digest of `files`.
    pub digest: String,
    pub rounds: Vec<Round>,
    /// Microluna's list-price spend on writing.
    pub writer_usd: f64,
    /// Jev's spend on verifying.
    pub jev_usd: f64,
    pub milliseconds: u64,
    /// Everything else a reader of the record may want: the runner, the
    /// options, and the Jev answers per test.
    pub detail: Value,
}

/// What changed in a suite directory since its freeze.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Integrity {
    pub intact: bool,
    pub digest: String,
    pub changed: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// A run refused because the suite was edited after its freeze.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tampered(pub Integrity);

impl std::fmt::Display for Tampered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Integrity {
            changed,
            added,
            removed,
            ..
        } = &self.0;
        write!(
            f,
            "the acceptance suite was edited after its freeze (changed: {}; added: {}; removed: {}); \
             a change needs a new accept.define",
            list_or_none(changed),
            list_or_none(added),
            list_or_none(removed)
        )
    }
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

impl AcceptanceSuite {
    /// Whether the suite directory still holds exactly the frozen files.
    #[must_use]
    pub fn integrity(&self) -> Integrity {
        let now = digest_files(&self.dir);
        let mut out = Integrity {
            digest: digest_of(&now),
            ..Integrity::default()
        };
        for (path, sha) in &self.files {
            match now.get(path) {
                None => out.removed.push(path.clone()),
                Some(other) if other != sha => out.changed.push(path.clone()),
                Some(_) => {}
            }
        }
        for path in now.keys() {
            if !self.files.contains_key(path) {
                out.added.push(path.clone());
            }
        }
        out.intact = out.changed.is_empty() && out.added.is_empty() && out.removed.is_empty();
        out
    }

    /// The requirement IDs the suite reports on: every requirement in its
    /// coverage, in map order.
    #[must_use]
    pub fn requirement_ids(&self) -> Vec<String> {
        self.coverage.iter().map(|c| c.id.clone()).collect()
    }

    /// A short account for a brief or a log line.
    #[must_use]
    pub fn headline(&self) -> String {
        format!(
            "{} acceptance suite: {} tests over {} requirements, {} rejected, {} gaps{}, digest {}",
            match self.status {
                Status::Accepted => "an accepted",
                Status::Partial => "a partial",
            },
            self.tests.len(),
            self.coverage.len(),
            self.rejected.len(),
            self.gaps.len(),
            if self.gaps.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    self.gaps
                        .iter()
                        .map(|g| g.requirement.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            &self.digest[..self.digest.len().min(12)]
        )
    }

    /// The suite as evidence for an edit session: each test's ID, the
    /// requirements it decides, what it asserts, and how to run it.
    #[must_use]
    pub fn evidence(&self) -> microluna::Evidence {
        let mut text = format!(
            "These tests are frozen: you can't change them, and the task is done when every one \
             passes. Run them all with `sh {dir}/run.sh`, or some with `sh {dir}/run.sh T1 T4`. \
             Each runs from the workspace root with $ACCEPT_DIR set to {dir}.\n",
            dir = self.dir.display()
        );
        for test in &self.tests {
            text.push_str(&format!(
                "\n- {} ({}; {}): {} [{}/{}]",
                test.id,
                test.requirements.join(", "),
                test.kind,
                test.what,
                self.dir.display(),
                test.path
            ));
            for note in &test.notes {
                text.push_str(&format!("\n  Note: {note}"));
            }
        }
        microluna::Evidence {
            label: "The acceptance tests".to_string(),
            text,
        }
    }

    /// Reads a suite record written by [`AcceptanceSuite::save`].
    ///
    /// # Errors
    ///
    /// A message when the file doesn't read or isn't a suite record.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let mut suite: AcceptanceSuite = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a suite record: {error}", path.display()))?;
        if suite.schema != SCHEMA {
            return Err(format!(
                "{} has schema {}, not {SCHEMA}",
                path.display(),
                suite.schema
            ));
        }
        for test in &mut suite.tests {
            test.source = std::fs::read_to_string(suite.dir.join(&test.path)).unwrap_or_default();
        }
        Ok(suite)
    }

    /// Writes the record to `path`.
    ///
    /// # Errors
    ///
    /// A message when the file can't be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        crate::record::write_atomic(path, format!("{text}\n").as_bytes())
    }

    /// Where [`define`] writes the record by default: beside the suite
    /// directory, as `<name>.accept.json`.
    #[must_use]
    pub fn record_path(dir: &Path) -> PathBuf {
        let name = dir
            .file_name()
            .map_or("suite".to_string(), |n| n.to_string_lossy().into_owned());
        dir.with_file_name(format!("{name}.accept.json"))
    }
}

/// SHA-256 of `bytes`, hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every regular file under `dir` and its SHA-256, by path relative to
/// `dir`. A symbolic link is digested by its target's path, not followed.
#[must_use]
pub fn digest_files(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let relative = path
                .strip_prefix(dir)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_symlink() {
                let target = std::fs::read_link(&path).unwrap_or_default();
                out.insert(
                    relative,
                    sha256(format!("link:{}", target.display()).as_bytes()),
                );
            } else if let Ok(bytes) = std::fs::read(&path) {
                out.insert(relative, sha256(&bytes));
            }
        }
    }
    out
}

/// The digest of a file map: SHA-256 over `path\tsha\n` lines in order.
#[must_use]
pub fn digest_of(files: &BTreeMap<String, String>) -> String {
    let mut text = String::new();
    for (path, sha) in files {
        text.push_str(path);
        text.push('\t');
        text.push_str(sha);
        text.push('\n');
    }
    sha256(text.as_bytes())
}

/// Reads the tests in `dir/tests`, in natural order of their IDs, and the
/// problems with their headers: no requirement named, or one the map
/// doesn't have.
#[must_use]
pub fn read_tests(dir: &Path, known: &[String]) -> (Vec<Test>, Vec<(String, String)>) {
    let mut tests = Vec::new();
    let mut problems = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir.join(TESTS_DIR)) else {
        return (tests, problems);
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sh") && p.is_file())
        .collect();
    paths.sort_by_key(|p| natural_key(&p.file_stem().unwrap_or_default().to_string_lossy()));
    for path in paths {
        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let header = |name: &str| {
            source.lines().take(30).find_map(|line| {
                let rest = line.trim_start().strip_prefix('#')?.trim_start();
                let (key, value) = rest.split_once(':')?;
                let key = key.trim().to_ascii_lowercase();
                (key == name || key == format!("{name}s")).then(|| value.trim().to_string())
            })
        };
        let requirements: Vec<String> = header("requirement")
            .unwrap_or_default()
            .split([',', ' '])
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        if requirements.is_empty() {
            problems.push((
                id.clone(),
                "it names no requirement in a `# requirement:` header".to_string(),
            ));
        }
        let unknown: Vec<&String> = requirements.iter().filter(|r| !known.contains(r)).collect();
        if !unknown.is_empty() {
            problems.push((
                id.clone(),
                format!(
                    "it names {} that the requirement list doesn't have",
                    unknown
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        tests.push(Test {
            notes: Vec::new(),
            id,
            requirements: requirements
                .into_iter()
                .filter(|r| known.contains(r))
                .collect(),
            kind: header("kind").unwrap_or_default(),
            what: header("what").unwrap_or_default(),
            path: format!(
                "{TESTS_DIR}/{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            source,
        });
    }
    (tests, problems)
}

/// `T10` after `T9`: the ID's letters, then its number.
fn natural_key(id: &str) -> (String, u64, String) {
    let letters: String = id.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let digits: String = id[letters.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (
        letters.clone(),
        digits.parse().unwrap_or(u64::MAX),
        id[letters.len() + digits.len()..].to_string(),
    )
}

/// The requirements a suite must decide: every one but context and
/// constraints, in map order. A constraint ("don't cheat", "you have 28800
/// seconds") holds throughout and has no failing state to prove red; the
/// edit sessions carry it as a fact instead.
#[must_use]
pub fn decidable(map: &RequirementMap) -> Vec<&crate::requirements::Requirement> {
    map.requirements
        .iter()
        .filter(|r| !matches!(r.kind, Kind::Context | Kind::Constraint))
        .collect()
}

/// The guidance every writing session reads: the format and the rules.
pub const GUIDANCE: &str = "You write the acceptance tests for the task before anyone solves it. \
Don't solve the task, and don't change the solution workspace: your tools can write only in this \
suite directory.

First find the decisive facts: every exact format, edge case, unit, threshold, ordering, and \
rule that the task or its data states and that a simpler reading would get wrong. Read the task \
and the workspace's data for them. Write them to facts.md, one per line, as `R3: the fact`. \
Then encode each fact as a test that fails for the simpler reading, and name the fact in the \
test's `# what:` line. Write at least one test per requirement.

Write one test per file under tests/, named tests/T1.sh, tests/T2.sh, and so on. A test is a POSIX \
shell script that exits 0 when its requirement is met and nonzero when it isn't. Start each test \
with three header lines:
# requirement: R3        (one or more requirement IDs from the list, comma-separated)
# kind: example          (example, edge, format, location, or error)
# what: one sentence that says what the test asserts
A test runs with the workspace root as its working directory. $ACCEPT_DIR is this suite \
directory, $WORKSPACE the workspace root, and $ACCEPT_TMP an empty scratch directory. Put helper \
programs and fixture files under lib/ and call them as \"$ACCEPT_DIR/lib/...\". When the task \
names an output file or location, a test checks that file at that location: run the program the \
way the task says, so it writes where the task says, then check what it wrote there. Put other \
scratch files under $ACCEPT_TMP. Refer to the task's own paths as the task states them.

Cover every requirement in the list: the task's stated examples with their exact expected values, \
the edge cases the task implies, the output's format and location, and the error behavior the \
task states. Assert only what the task states or what follows from it. Never hardcode an answer \
the task doesn't give: compute it from the task's own rules, or check a property that any correct \
answer has. Read the workspace's real inputs first. Prefer testing on them; when a test builds \
its own input, copy their exact format, since a correct solution parses that format and nothing \
else. Every test must be able to fail: no test that only checks that a file exists, and none \
that passes whatever the program does.

Run the suite with `sh run.sh`, or some tests with `sh run.sh T2 T5`. The work isn't done yet, so a \
test must fail now, on the untouched workspace, and fail because the behavior is missing, not \
because the test itself is broken. A test may pass now only when its requirement asks to keep \
something that is already true. Use only programs the workspace already has; check with env.sh \
first. The tests run without network access.

When the suite is written and `sh run.sh` shows each test failing for the right reason, call \
finish with a one-line summary.";

/// The brief for one writing round.
#[must_use]
pub fn brief(inputs: &Inputs<'_>, problems: &[String]) -> microluna::Brief {
    let note = if inputs.workspace_note.trim().is_empty() {
        format!(
            "The solution workspace is the directory {}. Read it with run_command, for example \
             `sh env.sh 'ls -la'` or `cat {}/FILE`; it is read-only to you.",
            inputs.workspace.display(),
            inputs.workspace.display()
        )
    } else {
        inputs.workspace_note.clone()
    };
    let mut evidence = vec![microluna::Evidence {
        label: "The requirements the suite must decide".to_string(),
        text: decidable(inputs.requirements)
            .iter()
            .map(|r| {
                format!(
                    "- {} ({}): {}",
                    r.id,
                    r.kind.word(),
                    r.text.split_whitespace().collect::<Vec<_>>().join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }];
    evidence.extend(inputs.evidence.iter().cloned());
    let mut state = Vec::new();
    if !problems.is_empty() {
        state.push(
            "The host ran and reviewed the suite you wrote. Fix these problems by editing, \
             rewriting, or deleting tests, then run `sh run.sh` again:"
                .to_string(),
        );
        state.extend(problems.iter().cloned());
    }
    microluna::Brief {
        task: format!(
            "Write an executable acceptance suite for this task. Don't solve it.\n\n{note}\n\n\
             ## The task: {}\n\n{}",
            inputs.task.title.trim(),
            inputs.task.instruction.trim()
        ),
        guidance: GUIDANCE.to_string(),
        evidence,
        state,
    }
}

/// Writes, verifies, and freezes an acceptance suite.
///
/// The result is always a suite: when writing fails or finds nothing, it
/// is a partial suite whose gaps name every requirement. The component
/// invocation, the Jev decisions, and a step carrying the suite are
/// recorded in `recorder`; the record is also written beside the suite
/// directory ([`AcceptanceSuite::record_path`]).
///
/// With [`Inputs::target`] set, the proof runs on the snapshot in
/// [`Inputs::workspace`] through [`Rebased`], and the frozen suite runs on
/// the target.
pub async fn define<W: Writer, R: Runner>(
    inputs: &Inputs<'_>,
    writer: &W,
    runner: &R,
    jev: &JevMode,
    recorder: &Recorder,
    options: &Options,
) -> AcceptanceSuite {
    match inputs.target.filter(|target| *target != inputs.workspace) {
        Some(target) => {
            let rebased = Rebased {
                inner: runner,
                real: target.to_path_buf(),
                snapshot: inputs.workspace.to_path_buf(),
                test_sec: options.test_sec,
            };
            define_on(inputs, writer, &rebased, jev, recorder, options).await
        }
        None => define_on(inputs, writer, runner, jev, recorder, options).await,
    }
}

/// The requirement IDs split into at most `parts` consecutive groups of
/// near-equal size.
#[must_use]
pub fn split(ids: &[String], parts: usize) -> Vec<Vec<String>> {
    if ids.is_empty() {
        return Vec::new();
    }
    let parts = parts.clamp(1, ids.len());
    let size = ids.len().div_ceil(parts);
    ids.chunks(size).map(<[String]>::to_vec).collect()
}

/// [`brief`] with the guidance `options` add: [`DISCOVER`] and
/// [`ONE_PASS`].
#[must_use]
pub fn briefed(inputs: &Inputs<'_>, problems: &[String], options: &Options) -> microluna::Brief {
    let mut out = brief(inputs, problems);
    if options.discover {
        out.guidance.push_str("\n\n");
        out.guidance.push_str(DISCOVER);
    }
    if options.one_pass {
        out.guidance.push_str("\n\n");
        out.guidance.push_str(ONE_PASS);
    }
    out
}

/// The brief for a targeted repair round with [`Rewrite::Hard`]: the
/// first round's task, guidance, and evidence unchanged, so the provider
/// serves that prefix from its cache, then what the first round found
/// (its facts and its tests) and only the flagged tests' problems, so the
/// session fixes those without rereading the workspace.
#[must_use]
pub fn repair_brief(
    inputs: &Inputs<'_>,
    problems: &[String],
    dir: &Path,
    known: &[String],
    options: &Options,
) -> microluna::Brief {
    let mut out = briefed(inputs, &[], options);
    let facts = std::fs::read_to_string(dir.join(FACTS)).unwrap_or_default();
    let (tests, _) = read_tests(dir, known);
    out.state = vec![
        "The host ran and reviewed the suite you wrote, and most of it stands. Fix only the \
         tests the problems below name, by editing, rewriting, or deleting them. Leave every \
         other test and facts.md as they are, and read the workspace only as far as these \
         tests need. Then run `sh run.sh` on the tests you changed, once, and call finish."
            .to_string(),
        format!(
            "Your facts so far, from facts.md:\n{}",
            crate::judge::clip(facts.trim(), 3_000)
        ),
        format!(
            "Your tests so far:\n{}",
            tests
                .iter()
                .map(|t| format!("{} ({}): {}", t.id, t.requirements.join(", "), t.what))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    ];
    out.state.extend(problems.iter().cloned());
    out
}

/// The brief for one of several writers in the first round: the usual
/// brief with only its share of the requirements, and a line that says
/// who writes the rest.
#[must_use]
pub fn part_brief(
    inputs: &Inputs<'_>,
    mine: &[String],
    others: &[String],
    parts: usize,
    options: &Options,
) -> microluna::Brief {
    let mut out = briefed(inputs, &[], options);
    if let Some(first) = out.evidence.first_mut() {
        first.label = "The requirements your part of the suite must decide".to_string();
        first.text = decidable(inputs.requirements)
            .iter()
            .filter(|r| mine.contains(&r.id))
            .map(|r| {
                format!(
                    "- {} ({}): {}",
                    r.id,
                    r.kind.word(),
                    r.text.split_whitespace().collect::<Vec<_>>().join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    out.state.push(format!(
        "You are one of {parts} writers working at the same time, each in a suite directory of \
         its own. You write the tests for {} only; the others write the tests for {}. Name your \
         tests tests/T1.sh, tests/T2.sh, and so on: the host renumbers them when it merges the \
         suites, and puts your facts.md lines with the others'.",
        mine.join(", "),
        if others.is_empty() {
            "nothing else".to_string()
        } else {
            others.join(", ")
        }
    ));
    out
}

/// A file's name with `prefix` before it, in the same directory:
/// `lib/check.py` becomes `lib/w2-check.py`.
fn prefixed(path: &str, prefix: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, name)) => format!("{dir}/{prefix}{name}"),
        None => format!("{prefix}{name}", name = path),
    }
}

/// Pairs of an old name and the new one.
pub type Renames = Vec<(String, String)>;

/// Merges the suites the parts wrote into `dir`, in part order: tests
/// renumbered `T1`, `T2`, and so on; helper files copied, renamed when
/// another part already wrote a different file at the same path, with
/// every mention in that part's files rewritten; and the facts joined.
/// Each part's directory becomes `dir` in what is copied.
///
/// # Errors
///
/// A message when a file can't be written.
pub fn merge_parts(
    dir: &Path,
    parts: &[PathBuf],
    known: &[String],
) -> Result<Vec<(Renames, Renames)>, String> {
    let write = |path: &Path, bytes: &[u8]| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
    };
    let mut next = 1usize;
    let mut facts: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let prefix = format!("w{}-", index + 1);
        // Helper files first, so the renames are known before the tests
        // are rewritten.
        let mut renamed: Vec<(String, String)> = Vec::new();
        let mut helpers: Vec<(String, String)> = Vec::new();
        for (path, sha) in digest_files(part) {
            if path.starts_with(TESTS_DIR)
                || path.starts_with(REJECTED_DIR)
                || HARNESS.contains(&path.as_str())
                || path == FACTS
            {
                continue;
            }
            let there = dir.join(&path);
            let target = if !there.exists() {
                path.clone()
            } else if std::fs::read(&there).map(|b| sha256(&b)).ok().as_deref() == Some(&sha) {
                continue;
            } else {
                let renamed_to = prefixed(&path, &prefix);
                renamed.push((path.clone(), renamed_to.clone()));
                renamed_to
            };
            helpers.push((path, target));
        }
        let rewrite = |text: &str| {
            let mut text = crate::compose::best_of::rebase(text, part, dir);
            for (from, to) in &renamed {
                text = text.replace(from.as_str(), to.as_str());
            }
            text
        };
        let copy = |from: &Path, to: &Path| -> Result<(), String> {
            let bytes =
                std::fs::read(from).map_err(|error| format!("{}: {error}", from.display()))?;
            match String::from_utf8(bytes) {
                Ok(text) => write(to, rewrite(&text).as_bytes()),
                Err(error) => write(to, error.as_bytes()),
            }
        };
        for (from, to) in &helpers {
            copy(&part.join(from), &dir.join(to))?;
        }
        let (tests, _) = read_tests(part, known);
        let mut renumbered = Vec::new();
        for test in tests {
            let id = format!("T{next}");
            next += 1;
            copy(
                &part.join(TESTS_DIR).join(format!("{}.sh", test.id)),
                &dir.join(TESTS_DIR).join(format!("{id}.sh")),
            )?;
            renumbered.push((test.id, id));
        }
        for line in std::fs::read_to_string(part.join(FACTS))
            .unwrap_or_default()
            .lines()
        {
            let line = line.trim_end();
            if !line.trim().is_empty() && !facts.iter().any(|held| held == line) {
                facts.push(line.to_string());
            }
        }
        out.push((renumbered, renamed));
    }
    if !facts.is_empty() {
        write(
            &dir.join(FACTS),
            format!("{}\n", facts.join("\n")).as_bytes(),
        )?;
    }
    Ok(out)
}

/// The first round with several writers at once: each writes its share of
/// the requirements in a directory of its own beside `dir`, then their
/// suites merge into `dir`. Returns the round's summed writer and each
/// part.
async fn write_parts<W: Writer, R: Runner>(
    inputs: &Inputs<'_>,
    writer: &W,
    runner: &R,
    dir: &Path,
    known: &[String],
    options: &Options,
) -> (Written, Vec<Part>) {
    let groups = split(known, options.writers);
    let name = dir
        .file_name()
        .map_or("suite".to_string(), |n| n.to_string_lossy().into_owned());
    let dirs: Vec<PathBuf> = (1..=groups.len())
        .map(|k| dir.with_file_name(format!("{name}-w{k}")))
        .collect();
    for part in &dirs {
        let _ = std::fs::remove_dir_all(part);
        let _ = std::fs::create_dir_all(part.join(TESTS_DIR));
        for (file, text) in runner.harness(part, inputs.workspace) {
            let _ = std::fs::write(part.join(file), text);
        }
    }
    let n = groups.len();
    let briefs: Vec<(microluna::Brief, String)> = groups
        .iter()
        .enumerate()
        .map(|(k, mine)| {
            let others: Vec<String> = known
                .iter()
                .filter(|id| !mine.contains(id))
                .cloned()
                .collect();
            let with: Vec<String> = (1..=n)
                .filter(|j| *j != k + 1)
                .map(|j| j.to_string())
                .collect();
            (
                part_brief(inputs, mine, &others, n, options),
                format!(
                    "writer {} of {n} writes the tests for {}, in parallel with writer{} {}",
                    k + 1,
                    mine.join(", "),
                    if with.len() == 1 { "" } else { "s" },
                    with.join(" and ")
                ),
            )
        })
        .collect();
    let names: Vec<String> = (1..=n).map(|k| format!("accept-writer-1-{k}")).collect();
    let written: Vec<Written> =
        futures_util::future::join_all(briefs.iter().zip(&dirs).zip(&names).map(
            |(((brief, directive), part), name)| writer.write_as(brief, part, 1, name, directive),
        ))
        .await;
    let merged = match merge_parts(dir, &dirs, known) {
        Ok(merged) => merged,
        Err(error) => {
            crate::say::line(&format!(
                "  accept ▸ merging the writers' suites failed: {error}"
            ));
            vec![(Vec::new(), Vec::new()); dirs.len()]
        }
    };
    let started = written.iter().filter_map(|w| w.started_at_ms).min();
    let ended = written
        .iter()
        .filter_map(|w| w.started_at_ms.map(|s| s + w.milliseconds))
        .max();
    let sum = Written {
        ending: if written.iter().all(|w| w.ending == "finished") {
            "finished".to_string()
        } else {
            written
                .iter()
                .find(|w| w.ending != "finished")
                .map_or("finished".to_string(), |w| w.ending.clone())
        },
        summary: written
            .iter()
            .enumerate()
            .map(|(k, w)| format!("Writer {}: {}", k + 1, w.summary))
            .collect::<Vec<_>>()
            .join("\n"),
        turns: written.iter().map(|w| w.turns).sum(),
        calls: written.iter().map(|w| w.calls).sum(),
        usd: written.iter().map(|w| w.usd).sum(),
        milliseconds: match (started, ended) {
            (Some(s), Some(e)) => e.saturating_sub(s),
            _ => written.iter().map(|w| w.milliseconds).max().unwrap_or(0),
        },
        trace: None,
        started_at_ms: started,
        name: Some("accept-writer-1".to_string()),
        input_tokens: written.iter().map(|w| w.input_tokens).sum(),
        cached_tokens: written.iter().map(|w| w.cached_tokens).sum(),
        output_tokens: written.iter().map(|w| w.output_tokens).sum(),
    };
    let parts = written
        .into_iter()
        .zip(groups)
        .zip(merged)
        .map(|((writer, requirements), (renumbered, renamed))| Part {
            writer,
            requirements,
            renumbered,
            renamed,
        })
        .collect();
    (sum, parts)
}

#[allow(clippy::too_many_lines)]
async fn define_on<W: Writer, R: Runner>(
    inputs: &Inputs<'_>,
    writer: &W,
    runner: &R,
    jev: &JevMode,
    recorder: &Recorder,
    options: &Options,
) -> AcceptanceSuite {
    let started = Instant::now();
    // Tests run from the workspace root, so every path to the suite is
    // absolute.
    let _ = std::fs::remove_dir_all(inputs.suite_dir);
    let _ = std::fs::create_dir_all(inputs.suite_dir.join(TESTS_DIR));
    let dir = inputs
        .suite_dir
        .canonicalize()
        .unwrap_or_else(|_| inputs.suite_dir.to_path_buf());
    let known: Vec<String> = decidable(inputs.requirements)
        .iter()
        .map(|r| r.id.clone())
        .collect();
    let invocation = recorder.enter(
        Start::new(
            DEFINE_COMPONENT,
            Implementation::new(
                DEFINE_COMPONENT,
                "microluna writer, code red-first, jev verify",
                &json!({
                    "options": options,
                    "runner": runner.describe(),
                    "writer": writer.describe(),
                    "questions": verify::question_digest(),
                }),
            ),
        )
        .named("accept.define")
        .reading(&json!({
            "instruction_sha256": sha256(inputs.task.instruction.as_bytes()),
            "requirements": known,
        }))
        .with_effects(),
    );
    for (name, text) in runner.harness(&dir, inputs.workspace) {
        let _ = std::fs::write(dir.join(name), text);
    }
    let mut cache = verify::Cache::default();
    let mut rounds: Vec<Round> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    let mut writer_usd = 0.0;
    let mut jev_usd = 0.0;
    let mut last: Option<verify::Verified> = None;
    for number in 1..=options.max_rounds.max(1) {
        if writer_usd + jev_usd >= options.spend_usd {
            problems.push(format!(
                "stopped before round {number}: the spend bound of ${:.2} was reached",
                options.spend_usd
            ));
            break;
        }
        let (written, parts) = if number == 1 && options.writers > 1 && known.len() > 1 {
            write_parts(inputs, writer, runner, &dir, &known, options).await
        } else if number > 1 && options.rewrite == Rewrite::Hard {
            (
                writer
                    .repair(
                        &repair_brief(inputs, &problems, &dir, &known, options),
                        &dir,
                        number,
                        options.repair_turns,
                    )
                    .await,
                Vec::new(),
            )
        } else {
            (
                writer
                    .write(&briefed(inputs, &problems, options), &dir, number)
                    .await,
                Vec::new(),
            )
        };
        writer_usd += written.usd;
        let verified = verify::verify(
            inputs, &dir, &known, runner, jev, recorder, options, &mut cache, number,
        )
        .await;
        jev_usd += verified.jev_usd;
        problems = verified.problems();
        rounds.push(Round {
            number,
            writer: written,
            tests: verified.tests.len(),
            green_at_start: verified.start.iter().filter(|t| t.green).count(),
            problems: problems.clone(),
            gaps: verified.gaps.clone(),
            jev_requests: verified.jev_requests,
            jev_usd: verified.jev_usd,
            parts,
        });
        let done = problems.is_empty();
        last = Some(verified);
        if done {
            break;
        }
    }
    let verified = last.unwrap_or_default();
    // Freeze: rejected tests leave tests/, the harness is replaced by the
    // edit sessions' run.sh and env.sh (the tests call env.sh, so a frozen
    // suite without it can't run a single assertion), and the directory is
    // digested.
    let _ = std::fs::create_dir_all(dir.join(REJECTED_DIR));
    for rejected in &verified.rejected {
        let from = dir.join(TESTS_DIR).join(format!("{}.sh", rejected.id));
        let _ = std::fs::rename(
            &from,
            dir.join(REJECTED_DIR).join(format!("{}.sh", rejected.id)),
        );
    }
    for name in HARNESS {
        let _ = std::fs::remove_file(dir.join(name));
    }
    // A suite proven on a snapshot runs on its target from now on: a test
    // that named the snapshot's path names the target's instead.
    let frozen_on = inputs.target.unwrap_or(inputs.workspace);
    if frozen_on != inputs.workspace {
        for path in digest_files(&dir).keys() {
            let at = dir.join(path);
            if let Ok(text) = std::fs::read_to_string(&at) {
                let rebased = crate::compose::best_of::rebase(&text, inputs.workspace, frozen_on);
                if rebased != text {
                    let _ = std::fs::write(&at, rebased);
                }
            }
        }
    }
    let _ = std::fs::write(
        dir.join("run.sh"),
        runner::local_run_sh(frozen_on, options.test_sec),
    );
    let _ = std::fs::write(dir.join("env.sh"), runner::local_env_sh(frozen_on));
    let kept: Vec<Test> = verified
        .tests
        .iter()
        .filter(|t| !verified.rejected.iter().any(|r| r.id == t.id))
        .map(|t| {
            let path = format!("{TESTS_DIR}/{}.sh", t.id);
            Test {
                source: std::fs::read_to_string(dir.join(&path))
                    .unwrap_or_else(|_| t.source.clone()),
                notes: verified.notes.get(&t.id).cloned().unwrap_or_default(),
                path,
                ..t.clone()
            }
        })
        .collect();
    let files = digest_files(&dir);
    let digest = digest_of(&files);
    let start_runs: Vec<TestRun> = verified
        .start
        .iter()
        .filter(|r| kept.iter().any(|t| t.id == r.id))
        .cloned()
        .collect();
    let start = RunResult::of("start", &digest, &known, start_runs, verified.start_ms);
    let status = if verified.gaps.is_empty() && !kept.is_empty() {
        Status::Accepted
    } else {
        Status::Partial
    };
    let suite = AcceptanceSuite {
        schema: SCHEMA.to_string(),
        dir: dir.clone(),
        status,
        instruction_sha256: sha256(inputs.task.instruction.as_bytes()),
        tests: kept,
        rejected: verified.rejected.clone(),
        coverage: verified.coverage.clone(),
        gaps: verified.gaps.clone(),
        start: Some(start),
        files,
        digest,
        rounds,
        writer_usd,
        jev_usd,
        milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        detail: json!({
            "runner": runner.describe(),
            "writer": writer.describe(),
            "options": options,
            "judged": verified.judged,
            "left": problems,
        }),
    };
    let _ = suite.save(&AcceptanceSuite::record_path(&dir));
    recorder.push(
        atif::Step::said(
            atif::Source::System,
            &format!("accept.define froze {}.", suite.headline()),
        )
        .noting(
            SUITE_EXTENSION,
            serde_json::to_value(&suite).unwrap_or(Value::Null),
        ),
    );
    recorder.end(
        &invocation,
        Finish::new(if suite.tests.is_empty() {
            Outcome::Failed
        } else {
            Outcome::Completed
        })
        .summary(json!({
            "status": suite.status,
            "digest": suite.digest,
            "tests": suite.tests.len(),
            "rejected": suite.rejected.len(),
            "gaps": suite.gaps,
            "rounds": suite.rounds.len(),
            "writer_usd": suite.writer_usd,
            "jev_usd": suite.jev_usd,
        })),
    );
    suite
}

/// Runs the frozen suite on `workspace` through `runner`.
///
/// # Errors
///
/// [`Tampered`] when the suite directory changed since the freeze; nothing
/// runs.
pub async fn run<R: Runner>(
    suite: &AcceptanceSuite,
    workspace: &Path,
    runner: &R,
    recorder: Option<&Recorder>,
    label: &str,
) -> Result<RunResult, Tampered> {
    let integrity = suite.integrity();
    if !integrity.intact {
        if let Some(recorder) = recorder {
            recorder.push(
                atif::Step::said(
                    atif::Source::System,
                    &format!("accept.run refused: {}", Tampered(integrity.clone())),
                )
                .noting(
                    RUN_EXTENSION,
                    json!({ "label": label, "refused": integrity }),
                ),
            );
        }
        return Err(Tampered(integrity));
    }
    let invocation = recorder.map(|recorder| {
        recorder.begin(
            Start::new(
                RUN_COMPONENT,
                Implementation::new(RUN_COMPONENT, "runner", &runner.describe()),
            )
            .named(label)
            .reading_digest(suite.digest.clone()),
        )
    });
    let started = Instant::now();
    let mut runs = runner.run_all(&suite.tests, &suite.dir, workspace).await;
    // A red test runs once more: one that passes then is green but marked
    // flaky, since timing-bound tests fail under load.
    let red: Vec<Test> = suite
        .tests
        .iter()
        .filter(|t| runs.iter().any(|r| r.id == t.id && !r.green))
        .cloned()
        .collect();
    if !red.is_empty() {
        for again in runner.run_all(&red, &suite.dir, workspace).await {
            if again.green
                && let Some(first) = runs.iter_mut().find(|r| r.id == again.id)
            {
                *first = TestRun {
                    flaky: true,
                    ..again
                };
            }
        }
    }
    let mut result = RunResult::of(
        label,
        &suite.digest,
        &suite.requirement_ids(),
        runs,
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    );
    result.gaps = suite.gaps.iter().map(|g| g.requirement.clone()).collect();
    result.complete = result.green && result.gaps.is_empty();
    if let (Some(recorder), Some(invocation)) = (recorder, invocation) {
        recorder.push(
            atif::Step::said(
                atif::Source::System,
                &format!(
                    "accept.run {label}: {} of {} tests green{}.",
                    result.passed,
                    result.total,
                    if result.green { ", all green" } else { "" }
                ),
            )
            .noting(
                RUN_EXTENSION,
                serde_json::to_value(&result).unwrap_or(Value::Null),
            ),
        );
        recorder.end(
            &invocation,
            Finish::new(Outcome::Completed).summary(json!({
                "green": result.green,
                "complete": result.complete,
                "flaky": result.tests.iter().filter(|t| t.flaky).map(|t| t.id.clone()).collect::<Vec<_>>(),
                "passed": result.passed,
                "total": result.total,
                "red_requirements": result.red_requirements(),
            })),
        );
    }
    Ok(result)
}
