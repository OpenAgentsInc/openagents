//! The gate's test commands, run inside a boundary.
//!
//! The pre-pull-request gate runs `cargo test` on the packages a change
//! touches, and the tests it runs may be ones the model wrote. So each
//! test command runs:
//!
//! - inside `coder-boundary`, with writes allowed only to the checkout,
//!   the gate's target directory, and a scratch directory the boundary
//!   owns for `TMPDIR`;
//! - through `supervise`, with a [`DEADLINE`] and [`OUTPUT_KEPT`] bytes
//!   kept of each output stream;
//! - with every credential withheld: the variables Microluna withholds
//!   from a model's commands ([`crate::seal::is_withheld`]), every
//!   `GH_*` and `GITHUB_*` variable, the operator's `gh` login, and Git's
//!   credential helpers ([`crate::seal::Seal`]);
//! - with the network off in an evaluation run, whose seal is offline,
//!   and in a normal run when [`NETWORK_ENV`] is `off`. A normal run keeps
//!   the network on otherwise.
//!
//! When a host can't build the boundary, an evaluation run refuses to run
//! the tests and records the gate as incomplete. A normal run warns and
//! runs them without the write boundary, credentials still withheld.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::say::say;

/// How long one package's tests may run, building included, unless
/// [`DEADLINE_VAR`] says otherwise.
pub const DEADLINE: Duration = Duration::from_secs(1200);

/// Seconds that replace [`DEADLINE`] on a host whose cold builds of large
/// packages take longer (a cloud environment's first build of `coder`
/// takes about 12 minutes and its tests about 9, S1 2026-10-10). Every
/// check still runs; only the time allowed changes.
pub const DEADLINE_VAR: &str = "OPENAGENTS_GATE_DEADLINE_SECS";

/// The deadline in force: [`DEADLINE_VAR`] when it holds a whole number of
/// seconds above zero, else [`DEADLINE`].
pub fn deadline() -> Duration {
    deadline_from(std::env::var(DEADLINE_VAR).ok().as_deref())
}

fn deadline_from(value: Option<&str>) -> Duration {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map_or(DEADLINE, Duration::from_secs)
}

/// How long the host's `cargo fetch` before a normal run's tests may run.
const FETCH_DEADLINE: Duration = Duration::from_secs(600);

/// The bytes kept of each output stream of one test command.
pub const OUTPUT_KEPT: usize = 1024 * 1024;

/// The variable that, set to `off`, runs a normal issue-flow run's gate
/// tests with the network off. An evaluation run's seal decides instead.
pub const NETWORK_ENV: &str = "CODER_ONE_GATE_NETWORK";

/// The most output kept in one failing package's problem.
const TEST_OUTPUT_KEPT: usize = 3_000;

/// Builds the boundary from its spec: [`coder_boundary::Spec::build`],
/// or a stand-in in a test.
pub type Build =
    fn(coder_boundary::Spec) -> Result<coder_boundary::Boundary, coder_boundary::Error>;

/// How the gate ran its tests, as a run's manifest and a pull request
/// record it.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Confinement {
    /// `confined`, `unconfined` (a normal run on a host with no
    /// boundary), `refused` (an evaluation run on a host with no boundary:
    /// the gate is incomplete), or `none` (no Rust package changed).
    pub mode: &'static str,
    /// The sandbox program, when the tests ran in a boundary.
    pub backend: Option<String>,
    /// `on` or `off`.
    pub network: &'static str,
    /// Whether credentials, including GitHub's, were withheld.
    pub credentials_withheld: bool,
    /// The paths the tests could write, when they ran in a boundary.
    pub writable: Vec<String>,
    pub reads_confined: bool,
    pub readable: Vec<String>,
    pub deadline_seconds: u64,
    pub output_kept_bytes: usize,
    /// What the host's `cargo fetch` did before the tests: `fetched`,
    /// `skipped`, or why it failed.
    pub prefetch: String,
    /// The packages whose tests the gate ran or would have run.
    pub packages: Vec<String>,
    /// Why the tests ran without a boundary, or didn't run.
    pub reason: Option<String>,
    /// Nonblocking flaky or pre-existing test failures.
    pub notes: Vec<String>,
    #[serde(skip)]
    pub failed_tests: Vec<(String, String)>,
}

impl Confinement {
    /// Whether the gate is incomplete: the tests had to run and didn't.
    #[must_use]
    pub fn incomplete(&self) -> bool {
        self.mode == "refused"
    }

    /// The record in a sentence, for a pull request body.
    #[must_use]
    pub fn describe(&self) -> String {
        let packages = self
            .packages
            .iter()
            .map(|package| format!("`{package}`"))
            .collect::<Vec<_>>()
            .join(", ");
        match self.mode {
            "confined" => format!(
                "The host ran the tests of {packages} inside a write boundary ({}): writes only \
                 to the checkout and the gate's target directory, credentials and GitHub \
                 withheld, the network {}, and a {}-second deadline.",
                self.backend.as_deref().unwrap_or("sandbox"),
                self.network,
                self.deadline_seconds
            ),
            "unconfined" => format!(
                "The host ran the tests of {packages} without a write boundary, because {}. \
                 Credentials and GitHub were still withheld.",
                self.reason.as_deref().unwrap_or("none could be built")
            ),
            "refused" => format!(
                "The host did not run the tests of {packages}, because {}, and an evaluation \
                 run never runs a model's tests without one.",
                self.reason
                    .as_deref()
                    .unwrap_or("no boundary could be built")
            ),
            _ => "No Rust package changed, so the host ran no tests.".to_string(),
        }
    }
}

/// Where and how the gate's tests run.
pub struct Setup {
    pub workdir: PathBuf,
    /// The Cargo target directory, shared across issue runs.
    pub target: PathBuf,
    /// What cuts the tests off from GitHub and, when offline, the network.
    pub seal: crate::seal::Seal,
    /// Whether this is an evaluation run, which never runs tests
    /// unconfined and fetches crates once, before the flow.
    pub evaluation: bool,
    /// The environment the tests start from, before anything is withheld.
    pub env: Vec<(OsString, OsString)>,
    pub build: Build,
}

impl Setup {
    /// The setup for an issue-flow run in `workdir`. `seal` is an
    /// evaluation run's; a normal run lays out its own, online unless
    /// [`NETWORK_ENV`] is `off`.
    ///
    /// # Errors
    ///
    /// Returns a message when the target directory or the seal can't be
    /// laid out.
    pub fn for_run(workdir: &Path, seal: Option<&crate::seal::Seal>) -> Result<Setup, String> {
        Setup::for_run_in(workdir, seal, None)
    }

    /// [`Setup::for_run`], building in `slot` when the caller leased one
    /// of the host's build slots for the run (#10293), so the gate's
    /// builds stay in the slot budget. A sealed evaluation still builds in
    /// its own directory.
    ///
    /// # Errors
    ///
    /// Returns a message when the target directory or the seal can't be
    /// laid out.
    pub fn for_run_in(
        workdir: &Path,
        seal: Option<&crate::seal::Seal>,
        slot: Option<&Path>,
    ) -> Result<Setup, String> {
        let home = crate::credentials::openagents_dir().map(|dir| dir.join("coder-one"));
        let target = if seal.is_some_and(|seal| seal.read_scope().is_some()) {
            // A shared build directory can contain other attempts' source
            // and compiled answers. Sealed evaluations use their own.
            workdir.join("target")
        } else if let Some(slot) = slot {
            slot.to_path_buf()
        } else if let Some(named) =
            std::env::var_os("CARGO_TARGET_DIR").filter(|dir| seal.is_none() && !dir.is_empty())
        {
            // A normal run on a host that names its own build directory
            // builds there, as the host's other cargo commands do.
            PathBuf::from(named)
        } else {
            home.as_ref()
                .map_or_else(|| workdir.join("target"), |dir| dir.join("target"))
        };
        std::fs::create_dir_all(&target)
            .map_err(|error| format!("cannot create {}: {error}", target.display()))?;
        let evaluation = seal.is_some();
        let seal = match seal {
            Some(seal) => seal.clone(),
            None => {
                let dir = home
                    .unwrap_or_else(|| std::env::temp_dir().join("coder-one"))
                    .join("gate-seal");
                let offline = std::env::var(NETWORK_ENV).is_ok_and(|value| value.trim() == "off");
                crate::seal::Seal::create(&dir, offline)
                    .map_err(|error| format!("cannot lay out {}: {error}", dir.display()))?
            }
        };
        Ok(Setup {
            workdir: workdir.to_path_buf(),
            target,
            seal,
            evaluation,
            env: std::env::vars_os().collect(),
            build: coder_boundary::Spec::build,
        })
    }
}

/// Whether a variable is kept from the tests: a credential Microluna
/// withholds, or a GitHub variable.
pub fn withheld(name: &OsString) -> bool {
    name.to_str()
        .is_some_and(|name| crate::seal::is_withheld(name) || crate::seal::is_github(name))
}

/// The absolute path of `program` on the `PATH` in `env`: a boundary
/// never searches for the program it runs.
pub fn on_path(env: &[(OsString, OsString)], program: &str) -> Option<PathBuf> {
    let path = env.iter().find(|(name, _)| name == "PATH")?.1.clone();
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_absolute() && candidate.is_file())
}

/// Sets a test command's environment: `env` without what's withheld,
/// then the seal, the target directory, and the scratch directory.
pub fn environment(command: &mut Command, setup: &Setup, scratch: Option<&Path>) {
    command.env_clear();
    for (name, value) in &setup.env {
        if !withheld(name) {
            command.env(name, value);
        }
    }
    setup.seal.apply(command);
    command.env("CARGO_TARGET_DIR", &setup.target);
    // Checks test the code's defaults, not this host's adopted calibration.
    command.env("OPENAGENTS_CALIBRATION", "off");
    if let Some(scratch) = scratch {
        command.env("TMPDIR", scratch);
    }
    command.current_dir(&setup.workdir);
}

/// Which cargo command the gate runs on each package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    /// `cargo test -q -p PACKAGE --all-features`.
    Tests,
    /// `cargo clippy -q -p PACKAGE --all-targets -- -D warnings`, which
    /// builds the package's build scripts and macros, so it runs inside
    /// the same boundary as the tests.
    Clippy,
}

impl Suite {
    /// The cargo arguments for `package`.
    #[must_use]
    pub fn args(self, package: &str) -> Vec<String> {
        let words: &[&str] = match self {
            Suite::Tests => &["test", "-q", "-p", package, "--all-features"],
            Suite::Clippy => &[
                "clippy",
                "-q",
                "-p",
                package,
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        };
        words.iter().map(|word| (*word).to_string()).collect()
    }

    /// What the command checks, in words.
    #[must_use]
    pub fn noun(self) -> &'static str {
        match self {
            Suite::Tests => "tests",
            Suite::Clippy => "Clippy lints",
        }
    }
}

/// Runs each package's tests as the module docs say, and returns the
/// failing ones with the end of their output, and how they ran.
pub async fn run(setup: &Setup, packages: &[String]) -> (Vec<String>, Confinement) {
    let base = super::command(&setup.workdir, "git", &["rev-parse", "origin/main"]).ok();
    run_with_base(setup, packages, base.as_deref()).await
}

/// Runs `suite` on each package as the module docs say, and returns the
/// failing ones with the end of their output, and how they ran.
pub async fn run_suite(
    setup: &Setup,
    packages: &[String],
    suite: Suite,
) -> (Vec<String>, Confinement) {
    run_commands(setup, packages, suite, None).await
}

async fn run_commands(
    setup: &Setup,
    packages: &[String],
    suite: Suite,
    selected: Option<&str>,
) -> (Vec<String>, Confinement) {
    let network_off = setup.seal.offline();
    let mut record = Confinement {
        mode: "none",
        backend: None,
        network: if network_off { "off" } else { "on" },
        credentials_withheld: true,
        writable: Vec::new(),
        reads_confined: setup.seal.read_scope().is_some(),
        readable: Vec::new(),
        deadline_seconds: deadline().as_secs(),
        output_kept_bytes: OUTPUT_KEPT,
        prefetch: "skipped".to_string(),
        packages: packages.to_vec(),
        reason: None,
        notes: Vec::new(),
        failed_tests: Vec::new(),
    };
    if packages.is_empty() {
        return (Vec::new(), record);
    }
    let Some(cargo) = on_path(&setup.env, "cargo") else {
        record.mode = if setup.evaluation {
            "refused"
        } else {
            "unconfined"
        };
        record.reason = Some("cargo is not on PATH".to_string());
        return (
            vec!["the tests could not run: cargo is not on PATH".to_string()],
            record,
        );
    };
    // An evaluation run fetched its crates before the flow; fetching here
    // would let a changed manifest pull code from the network.
    if !setup.evaluation {
        record.prefetch = prefetch(setup, &cargo).await;
    }
    let spec = coder_boundary::Boundary::writing(&setup.workdir);
    let spec = if setup.target.starts_with(&setup.workdir) {
        spec
    } else {
        spec.writable(&setup.target)
    };
    let spec = spec.owned_scratch_under(std::env::temp_dir());
    let spec = if network_off { spec.offline() } else { spec };
    let spec = setup.seal.constrain_reads(spec);
    let confined = match (setup.build)(spec) {
        Ok(boundary) => {
            record.mode = "confined";
            record.backend = Some(boundary.backend().display().to_string());
            record.readable = boundary
                .readable()
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            record.writable = std::iter::once(setup.workdir.display().to_string())
                .chain(
                    boundary
                        .writable()
                        .iter()
                        .map(|path| path.display().to_string()),
                )
                .collect();
            say!(
                "issue ▸ the tests run in a write boundary, credentials withheld, network {}",
                record.network
            );
            Some(boundary)
        }
        Err(error) if setup.evaluation => {
            record.mode = "refused";
            record.reason = Some(format!("this host can't build a write boundary ({error})"));
            say!(
                "issue ▸ the tests did not run: this host can't build a write boundary ({error}), \
                 and an evaluation run never runs them without one"
            );
            return (
                vec![format!(
                    "the tests did not run: this host can't build a write boundary ({error}), so \
                     the gate is incomplete"
                )],
                record,
            );
        }
        Err(error) => {
            record.mode = "unconfined";
            record.reason = Some(format!("this host can't build a write boundary ({error})"));
            say!(
                "issue ▸ warning: this host can't build a write boundary ({error}), so the tests \
                 run without one, credentials still withheld"
            );
            None
        }
    };
    // One boundary covers every package. It stays held, with its scratch
    // directory, until the last test command is reaped.
    let mut failures = Vec::new();
    for package in packages {
        say!("issue ▸ running the {package} {}", suite.noun());
        let mut args = suite.args(package);
        if let Some(test) = selected {
            args.extend([test.to_string(), "--".to_string(), "--exact".to_string()]);
        }
        let command = match &confined {
            Some(boundary) => match boundary.command(&cargo, &args) {
                Ok(mut command) => {
                    environment(&mut command, setup, boundary.scratch());
                    if boundary.confines_reads() {
                        let path = command
                            .get_envs()
                            .find(|(name, _)| *name == "PATH")
                            .and_then(|(_, value)| value.map(OsString::from))
                            .unwrap_or_default();
                        command.env("PATH", boundary.search_path(&path));
                        if let Some(scratch) = boundary.scratch() {
                            command.env("HOME", scratch);
                        }
                    }
                    command
                }
                Err(error) => {
                    failures.push(format!("the {package} tests could not run: {error}"));
                    continue;
                }
            },
            None => {
                let mut command = Command::new(&cargo);
                command.args(&args);
                environment(&mut command, setup, None);
                command
            }
        };
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(deadline()).keeping(OUTPUT_KEPT))
            .run()
            .await;
        if suite == Suite::Tests {
            record.failed_tests.extend(
                failing_names(&ended)
                    .into_iter()
                    .map(|test| (package.clone(), test)),
            );
        }
        let failed = match suite {
            Suite::Tests => failure(package, &ended),
            Suite::Clippy => lint_failure(package, &ended),
        };
        // A missing or ignored test is not evidence that it passes.
        let failed = if selected.is_some() && failed.is_none() && !ran_test(&ended) {
            Some(format!("the {package} selected test did not run"))
        } else {
            failed
        };
        if let Some(failure) = failed {
            failures.push(failure);
        }
    }
    drop(confined);
    (failures, record)
}

/// A normal run's `cargo fetch` on the host before the tests, so a test
/// command that can't write Cargo's cache still finds every crate.
pub async fn prefetch(setup: &Setup, cargo: &Path) -> String {
    if !setup.workdir.join("Cargo.toml").is_file() {
        return "skipped".to_string();
    }
    let ended = supervise::Job::new(cargo.as_os_str())
        .args(["fetch", "-q"])
        .in_directory(&setup.workdir)
        .bounded(supervise::Limits::within(FETCH_DEADLINE))
        .run()
        .await;
    if ended.ending.success() {
        "fetched".to_string()
    } else {
        format!(
            "failed ({}): {}",
            ended.ending,
            crate::judge::clip(ended.stderr.text.trim(), 300)
        )
    }
}

/// The problem one package's Clippy command leaves, or `None` when it
/// finds nothing.
pub fn lint_failure(package: &str, ended: &supervise::Ended) -> Option<String> {
    match &ended.ending {
        ending if ending.success() => None,
        supervise::Ending::TimedOut => Some(format!(
            "Clippy on {package} did not finish within {} seconds",
            deadline().as_secs()
        )),
        supervise::Ending::Failed(why) => Some(format!("Clippy on {package} could not run: {why}")),
        supervise::Ending::Exited(_) => {
            let text = format!("{}{}", ended.stdout.text, ended.stderr.text);
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| l.starts_with("error") || l.starts_with("warning") || l.contains("-->"))
                .collect();
            Some(format!(
                "Clippy finds problems in {package} (`cargo clippy -p {package} --all-targets -- \
                 -D warnings`): {}",
                crate::judge::clip(&lines.join("\n"), TEST_OUTPUT_KEPT)
            ))
        }
    }
}

/// The problem one package's test command leaves, or `None` when its
/// tests pass.
pub fn failure(package: &str, ended: &supervise::Ended) -> Option<String> {
    match &ended.ending {
        ending if ending.success() => None,
        supervise::Ending::TimedOut => Some(format!(
            "the {package} tests did not finish within {} seconds",
            deadline().as_secs()
        )),
        supervise::Ending::Failed(why) => Some(format!("the {package} tests could not run: {why}")),
        supervise::Ending::Exited(_) => {
            let text = format!("{}{}", ended.stdout.text, ended.stderr.text);
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| {
                    l.contains("FAILED")
                        || l.contains("panicked")
                        || l.starts_with("error")
                        || l.contains("assertion")
                        || l.trim_start().starts_with("left")
                        || l.trim_start().starts_with("right")
                })
                .collect();
            Some(format!(
                "the {package} tests fail (`cargo test -p {package} --all-features`): {}",
                crate::judge::clip(&lines.join("\n"), TEST_OUTPUT_KEPT)
            ))
        }
    }
}

/// Names reported by libtest, before the diagnostic output is clipped.
fn failing_names(ended: &supervise::Ended) -> Vec<String> {
    if !matches!(ended.ending, supervise::Ending::Exited(_)) || ended.ending.success() {
        return Vec::new();
    }
    let mut names = Vec::new();
    for line in ended.stdout.text.lines().chain(ended.stderr.text.lines()) {
        if let Some(name) = line
            .trim()
            .strip_prefix("test ")
            .and_then(|line| line.strip_suffix(" ... FAILED"))
            .or_else(|| line.trim().strip_suffix(" --- FAILED"))
            && !names.iter().any(|existing| existing == name)
        {
            names.push(name.to_string());
        }
    }
    names
}

fn ran_test(ended: &supervise::Ended) -> bool {
    ended.stdout.text.lines().any(|line| {
        line.strip_prefix("test result: ok. ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|count| count.parse::<usize>().ok())
            .is_some_and(|count| count > 0)
    })
}

/// Retries named failures once, then compares persistent failures with the
/// immutable main commit captured before the worker starts.
pub async fn run_with_base(
    setup: &Setup,
    packages: &[String],
    base: Option<&str>,
) -> (Vec<String>, Confinement) {
    let (initial, mut record) = run_suite(setup, packages, Suite::Tests).await;
    if initial.is_empty() || record.failed_tests.is_empty() {
        return (initial, record);
    }
    let mut problems = Vec::new();
    // Keep build errors, timeouts, and failures whose names could not be read.
    for problem in initial {
        if !record
            .failed_tests
            .iter()
            .any(|(package, _)| problem.starts_with(&format!("the {package} tests fail (")))
        {
            problems.push(problem);
        }
    }
    let mut persistent = Vec::new();
    for (package, test) in &record.failed_tests {
        let (retry, _) = run_commands(
            setup,
            std::slice::from_ref(package),
            Suite::Tests,
            Some(test),
        )
        .await;
        if retry.is_empty() {
            record
                .notes
                .push(format!("flaky: {test} (passed on retry)"));
        } else {
            persistent.push((package.clone(), test.clone(), retry));
        }
    }
    if persistent.is_empty() {
        return (problems, record);
    }
    let worktree = base.and_then(|base| BaseWorktree::create(&setup.workdir, base).ok());
    for (package, test, retry) in persistent {
        let mut already_failing = false;
        if let Some(tree) = &worktree {
            let baseline = Setup {
                workdir: tree.path.clone(),
                target: setup.target.clone(),
                seal: setup.seal.clone(),
                evaluation: setup.evaluation,
                env: setup.env.clone(),
                build: setup.build,
            };
            let (failures, ran) = run_commands(
                &baseline,
                std::slice::from_ref(&package),
                Suite::Tests,
                Some(&test),
            )
            .await;
            // Only an actual named failure, not a build or infrastructure error,
            // establishes that the test was already failing.
            already_failing =
                !failures.is_empty() && ran.failed_tests.contains(&(package.clone(), test.clone()));
        }
        if already_failing {
            record
                .notes
                .push(format!("already failing on main: {test}"));
        } else {
            problems.extend(retry);
        }
    }
    (problems, record)
}

struct BaseWorktree {
    source: PathBuf,
    path: PathBuf,
    _scratch: tempfile::TempDir,
}

impl BaseWorktree {
    fn create(source: &Path, base: &str) -> Result<Self, String> {
        let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = scratch.path().join("base");
        super::command(
            source,
            "git",
            &[
                "worktree",
                "add",
                "--detach",
                &path.to_string_lossy(),
                base.trim(),
            ],
        )?;
        Ok(Self {
            source: source.to_path_buf(),
            path,
            _scratch: scratch,
        })
    }
}

impl Drop for BaseWorktree {
    fn drop(&mut self) {
        let _ = super::command(
            &self.source,
            "git",
            &[
                "worktree",
                "remove",
                "--force",
                &self.path.to_string_lossy(),
            ],
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn unavailable(
        _: coder_boundary::Spec,
    ) -> Result<coder_boundary::Boundary, coder_boundary::Error> {
        Err(coder_boundary::Error::Unsupported("test stand-in"))
    }

    async fn scenario(kind: &str) -> (Vec<String>, Confinement, String) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("change");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
            vec!["commit", "--allow-empty", "-qm", "base"],
        ] {
            super::super::command(&repo, "git", &args).unwrap();
        }
        let base = super::super::command(&repo, "git", &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(repo.join("changed"), "change").unwrap();
        let log = dir.path().join("runs");
        let cargo = bin.join("cargo");
        std::fs::write(&cargo, r#"#!/bin/sh
printf '%s|%s|%s\n' "$PWD" "$*" "$CARGO_TARGET_DIR" >> "$RUN_LOG"
fail() { echo 'tests::timing --- FAILED'; echo 'test result: FAILED. 0 passed; 1 failed;'; exit 101; }
pass() { echo 'test result: ok. 1 passed; 0 failed;'; exit 0; }
case "$*" in
  *tests::regression*)
    case "$PWD" in */base) pass ;; *) echo "test tests::regression ... FAILED"; exit 101 ;; esac ;;
  *tests::timing*)
    case "$PWD" in
      */base)
        case "$SCENARIO" in
          existing) fail ;;
          infra) echo 'error: build failed' >&2; exit 101 ;;
          missing) echo 'test result: ok. 0 passed; 0 failed;'; exit 0 ;;
          *) pass ;;
        esac ;;
      *) { [ "$SCENARIO" = flaky ] || [ "$SCENARIO" = mixed ]; } && pass; fail ;;
    esac ;;
  *) [ "$SCENARIO" = mixed ] && echo "test tests::regression ... FAILED"; fail ;;
esac
"#).unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        let setup = Setup {
            workdir: repo.clone(),
            target: target.clone(),
            seal: crate::seal::Seal::create(&dir.path().join("seal"), true).unwrap(),
            // No fetch, and force the normal unconfined fallback with the stand-in.
            evaluation: false,
            env: vec![
                ("PATH".into(), bin.into_os_string()),
                ("SCENARIO".into(), kind.into()),
                ("RUN_LOG".into(), log.clone().into_os_string()),
            ],
            build: unavailable,
        };
        let (problems, record) = run_with_base(&setup, &["example".into()], Some(&base)).await;
        let runs = std::fs::read_to_string(log).unwrap();
        let lines: Vec<_> = runs.lines().collect();
        assert_eq!(
            lines.len(),
            if kind == "flaky" {
                2
            } else if kind == "mixed" {
                4
            } else {
                3
            }
        );
        assert!(lines[0].contains("test -q -p example --all-features|"));
        for line in &lines[1..] {
            assert!(line.contains("test -q -p example --all-features tests::"));
            assert!(line.contains(" -- --exact|"));
            assert!(line.ends_with(&target.display().to_string()));
        }
        if kind != "flaky" {
            assert!(lines.last().unwrap().contains("/base|"));
        }
        let worktrees =
            super::super::command(&repo, "git", &["worktree", "list", "--porcelain"]).unwrap();
        assert_eq!(
            worktrees.matches("worktree ").count(),
            1,
            "temporary worktree removed"
        );
        (problems, record, runs)
    }

    #[tokio::test]
    async fn flaky_failure_retries_only_the_named_test_and_does_not_block() {
        let (problems, record, _) = scenario("flaky").await;
        assert!(problems.is_empty());
        assert_eq!(record.notes, ["flaky: tests::timing (passed on retry)"]);
        assert!(super::super::checked(&problems, Some(&record)).contains(&record.notes[0]));
        let prompt = super::super::ignored_test_notes(&record.notes);
        assert!(prompt.contains("Do not edit"));
        assert!(prompt.contains(&record.notes[0]));
    }

    #[tokio::test]
    async fn pre_existing_failure_does_not_block_and_is_reported() {
        let (problems, record, _) = scenario("existing").await;
        assert!(problems.is_empty());
        assert_eq!(record.notes, ["already failing on main: tests::timing"]);
        assert!(super::super::checked(&problems, Some(&record)).contains(&record.notes[0]));
        assert!(super::super::ignored_test_notes(&record.notes).contains(&record.notes[0]));
    }

    #[tokio::test]
    async fn mixed_failures_ignore_flakes_but_keep_regressions() {
        let (problems, record, _) = scenario("mixed").await;
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("tests::regression"));
        assert_eq!(record.notes, ["flaky: tests::timing (passed on retry)"]);
        let prompt = format!(
            "{}{}",
            super::super::fix_request(Path::new("."), 10145, &problems),
            super::super::ignored_test_notes(&record.notes)
        );
        assert!(prompt.contains("tests::regression"));
        assert!(prompt.contains("Do not edit"));
        assert!(prompt.contains(&record.notes[0]));
    }

    #[tokio::test]
    async fn regression_passing_on_base_blocks() {
        let (problems, record, _) = scenario("regression").await;
        assert_eq!(problems.len(), 1);
        assert!(record.notes.is_empty());
    }

    #[tokio::test]
    async fn base_build_error_or_missing_test_is_not_pre_existing() {
        for kind in ["infra", "missing"] {
            let (problems, record, _) = scenario(kind).await;
            assert!(!problems.is_empty());
            assert!(record.notes.is_empty());
        }
    }

    #[test]
    fn a_leased_slot_is_where_a_normal_runs_checks_build() {
        let dir = tempfile::tempdir().unwrap();
        let slot = dir.path().join("targets").join("project-slot-0");
        let setup = Setup::for_run_in(dir.path(), None, Some(&slot)).unwrap();
        assert_eq!(setup.target, slot);
        assert!(slot.is_dir());
        assert!(!setup.evaluation);
    }

    #[test]
    fn an_offline_seal_without_a_read_scope_still_builds_in_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let seal = crate::seal::Seal::create(&dir.path().join("seal"), true).unwrap();
        assert!(seal.read_scope().is_none());
        let slot = dir.path().join("slot");
        let setup = Setup::for_run_in(dir.path(), Some(&seal), Some(&slot)).unwrap();
        assert_eq!(setup.target, slot);
        assert!(setup.evaluation);
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;

    #[test]
    fn the_deadline_override_takes_only_a_positive_whole_number() {
        assert_eq!(deadline_from(None), DEADLINE);
        assert_eq!(deadline_from(Some("3600")), Duration::from_secs(3600));
        assert_eq!(deadline_from(Some(" 2400 ")), Duration::from_secs(2400));
        assert_eq!(deadline_from(Some("0")), DEADLINE);
        assert_eq!(deadline_from(Some("soon")), DEADLINE);
    }
}
