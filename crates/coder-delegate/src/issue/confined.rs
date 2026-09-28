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

/// How long one package's tests may run.
pub const DEADLINE: Duration = Duration::from_secs(1200);

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
        let home = crate::credentials::openagents_dir().map(|dir| dir.join("coder-one"));
        let target = if seal.is_some_and(|seal| seal.read_scope().is_some()) {
            // A shared build directory can contain other attempts' source
            // and compiled answers. Sealed evaluations use their own.
            workdir.join("target")
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
    if let Some(scratch) = scratch {
        command.env("TMPDIR", scratch);
    }
    command.current_dir(&setup.workdir);
}

/// Runs each package's tests as the module docs say, and returns the
/// failing ones with the end of their output, and how they ran.
pub async fn run(setup: &Setup, packages: &[String]) -> (Vec<String>, Confinement) {
    let network_off = setup.seal.offline();
    let mut record = Confinement {
        mode: "none",
        backend: None,
        network: if network_off { "off" } else { "on" },
        credentials_withheld: true,
        writable: Vec::new(),
        reads_confined: setup.seal.read_scope().is_some(),
        readable: Vec::new(),
        deadline_seconds: DEADLINE.as_secs(),
        output_kept_bytes: OUTPUT_KEPT,
        prefetch: "skipped".to_string(),
        packages: packages.to_vec(),
        reason: None,
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
        say!("issue ▸ running the {package} tests");
        let args = ["test", "-q", "-p", package.as_str(), "--all-features"];
        let command = match &confined {
            Some(boundary) => match boundary.command(&cargo, args) {
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
                command.args(args);
                environment(&mut command, setup, None);
                command
            }
        };
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(DEADLINE).keeping(OUTPUT_KEPT))
            .run()
            .await;
        if let Some(failure) = failure(package, &ended) {
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

/// The problem one package's test command leaves, or `None` when its
/// tests pass.
pub fn failure(package: &str, ended: &supervise::Ended) -> Option<String> {
    match &ended.ending {
        ending if ending.success() => None,
        supervise::Ending::TimedOut => Some(format!(
            "the {package} tests did not finish within {} seconds",
            DEADLINE.as_secs()
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
