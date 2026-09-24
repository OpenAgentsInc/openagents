//! Where acceptance tests run: this host, a task container, or a Docker
//! image.
//!
//! A [`Runner`] runs a list of tests against one workspace and returns a
//! [`TestRun`] per test, in order. It also writes the writing session's
//! harness: `env.sh`, which runs a command in the workspace, and `run.sh`,
//! which runs the suite the way the runner does.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{OUTPUT_CHARS, TESTS_DIR, Test, TestRun};

/// Where tests run.
pub trait Runner {
    /// The runner, for the record.
    fn describe(&self) -> Value;

    /// The writing session's harness files, by name, for a suite in
    /// `suite_dir` and the workspace at `workspace`.
    fn harness(&self, suite_dir: &Path, workspace: &Path) -> Vec<(&'static str, String)>;

    /// Runs `tests` from `suite_dir` against `workspace`, in order.
    fn run_all(
        &self,
        tests: &[Test],
        suite_dir: &Path,
        workspace: &Path,
    ) -> impl Future<Output = Vec<TestRun>>;
}

/// How a local test is confined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confine {
    /// Inside a `coder-boundary` boundary whose only writable checkout is
    /// the workspace: a test may build there, but can't change the suite
    /// or anything else. A host that can't enforce one refuses the test.
    Writing,
    /// Inside a read-only boundary: the test writes only its scratch.
    ReadOnly,
    /// Directly, because the process already runs in a disposable task
    /// container that is the boundary. The caller asserts this.
    TaskContainer,
}

impl Confine {
    fn word(self) -> &'static str {
        match self {
            Confine::Writing => "writing",
            Confine::ReadOnly => "read-only",
            Confine::TaskContainer => "task-container",
        }
    }
}

/// Runs tests on this host.
#[derive(Clone, Debug)]
pub struct Local {
    pub confine: Confine,
    pub test_sec: u64,
}

impl Local {
    /// Tests confined to writing only the workspace, bounded by `test_sec`.
    #[must_use]
    pub fn writing(test_sec: u64) -> Self {
        Local {
            confine: Confine::Writing,
            test_sec,
        }
    }
}

/// `text` quoted for a POSIX shell.
#[must_use]
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn tail(stdout: &str, stderr: &str) -> String {
    let mut text = stdout.trim_end().to_string();
    if !stderr.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr.trim_end());
    }
    crate::judge::clip_tail(&text, OUTPUT_CHARS)
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The `run.sh` that runs a suite on this host from the workspace root:
/// every test, or the IDs given, with a line per test and the tail of a
/// red test's output.
#[must_use]
pub fn local_run_sh(workspace: &Path, test_sec: u64) -> String {
    format!(
        r#"#!/bin/sh
# Runs the acceptance tests from the workspace root, as the host does.
# Usage: sh run.sh [TEST_ID ...]
ACCEPT_DIR=$(cd "$(dirname "$0")" && pwd)
WORKSPACE=${{WORKSPACE:-{workspace}}}
export ACCEPT_DIR WORKSPACE
{tests}"#,
        workspace = sh_quote(&workspace.display().to_string()),
        tests = tests_sh(test_sec),
    )
}

/// The part of a `run.sh` that runs `$ACCEPT_DIR/tests/*.sh` from
/// `$WORKSPACE`, with a line per test and the tail of a red test's output.
fn tests_sh(test_sec: u64) -> String {
    format!(
        r#"if command -v timeout >/dev/null 2>&1; then bound="timeout {test_sec}"; else bound=""; fi
green=0
red=0
for test in "$ACCEPT_DIR"/tests/*.sh; do
  [ -e "$test" ] || continue
  id=$(basename "$test" .sh)
  if [ $# -gt 0 ]; then
    case " $* " in *" $id "*) ;; *) continue ;; esac
  fi
  ACCEPT_TMP=$(mktemp -d)
  export ACCEPT_TMP
  out=$(cd "$WORKSPACE" && $bound sh "$test" 2>&1)
  code=$?
  rm -rf "$ACCEPT_TMP"
  if [ "$code" -eq 0 ]; then
    green=$((green + 1))
    echo "GREEN $id"
  else
    red=$((red + 1))
    echo "RED   $id (exit $code)"
    printf '%s\n' "$out" | tail -n 12 | sed 's/^/      /'
  fi
done
echo "$green green, $red red"
[ "$red" -eq 0 ]
"#
    )
}

/// `text` escaped for a basic regular expression between `|` delimiters
/// in `sed`, or for its replacement.
fn sed_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '.' | '[' | ']' | '*' | '^' | '$' | '\\' | '|' | '&') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The writing session's `run.sh` when the proof runs on a snapshot: it
/// copies the suite to a scratch directory, reads every mention of `real`
/// in the copy as `snapshot`, and runs the copy against the snapshot, the
/// way [`Rebased`] proves the tests.
#[must_use]
pub fn rebasing_run_sh(real: &Path, snapshot: &Path, test_sec: u64) -> String {
    format!(
        r#"#!/bin/sh
# Runs the acceptance tests against the snapshot of the workspace, the way
# the host proves them: every mention of {real} in the suite reads as
# {snapshot}. Usage: sh run.sh [TEST_ID ...]
here=$(cd "$(dirname "$0")" && pwd)
here_re=$(printf '%s' "$here" | sed 's/[].[*^$\|&]/\\&/g')
ACCEPT_DIR=$(mktemp -d)
cp -R "$here"/. "$ACCEPT_DIR"/
find "$ACCEPT_DIR" -type f ! -name run.sh | while IFS= read -r file; do
  sed -e "s|$here_re|$ACCEPT_DIR|g" -e {rule} "$file" > "$file.rebased" &&
    cat "$file.rebased" > "$file"
  rm -f "$file.rebased"
done
WORKSPACE={workspace}
export ACCEPT_DIR WORKSPACE
{tests}rm -rf "$ACCEPT_DIR"
"#,
        real = real.display(),
        snapshot = snapshot.display(),
        rule = sh_quote(&format!(
            "s|{}|{}|g",
            sed_escape(&real.display().to_string()),
            sed_escape(&snapshot.display().to_string())
        )),
        workspace = sh_quote(&snapshot.display().to_string()),
        tests = tests_sh(test_sec),
    )
}

/// Copies the directory `from` to `to`, replacing what `to` held, with
/// every whole-path mention of each pair's first path in a text file
/// rewritten to its second ([`crate::compose::best_of::rebase`]). A file
/// that isn't UTF-8 is copied as it is.
///
/// # Errors
///
/// A message when a file can't be read or written.
pub fn rebase_tree(from: &Path, to: &Path, pairs: &[(&Path, &Path)]) -> Result<(), String> {
    crate::handoff::copy_tree(from, to)?;
    let mut stack = vec![to.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at)
            .map_err(|error| format!("{}: {error}", at.display()))?
            .flatten()
        {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file()
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let mut rebased = text.clone();
                for (old, new) in pairs {
                    rebased = crate::compose::best_of::rebase(&rebased, old, new);
                }
                if rebased != text {
                    std::fs::write(&path, rebased)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                }
            }
        }
    }
    Ok(())
}

/// Runs tests written for the workspace `real` against `snapshot`, a copy
/// of it taken before anything edited it. Each run copies the suite to a
/// scratch directory with every mention of `real` (and of the suite
/// directory) rewritten, so a test that names the task's own paths reads
/// the snapshot, not the workspace another session is editing.
pub struct Rebased<'a, R: Runner> {
    pub inner: &'a R,
    pub real: PathBuf,
    pub snapshot: PathBuf,
    /// One test's wall-time bound, for the writing session's `run.sh`.
    pub test_sec: u64,
}

impl<R: Runner> Runner for Rebased<'_, R> {
    fn describe(&self) -> Value {
        let mut out = self.inner.describe();
        out["rebased"] = json!({ "real": self.real, "snapshot": self.snapshot });
        out
    }

    fn harness(&self, _suite_dir: &Path, _workspace: &Path) -> Vec<(&'static str, String)> {
        vec![
            (
                "run.sh",
                rebasing_run_sh(&self.real, &self.snapshot, self.test_sec),
            ),
            ("env.sh", local_env_sh(&self.snapshot)),
        ]
    }

    async fn run_all(&self, tests: &[Test], suite_dir: &Path, workspace: &Path) -> Vec<TestRun> {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "accept-rebased-{}-{}-{}",
            std::process::id(),
            atif::now_ms(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        if let Err(error) = rebase_tree(
            suite_dir,
            &scratch,
            &[(suite_dir, &scratch), (&self.real, &self.snapshot)],
        ) {
            let _ = std::fs::remove_dir_all(&scratch);
            return tests
                .iter()
                .map(|test| TestRun {
                    id: test.id.clone(),
                    requirements: test.requirements.clone(),
                    green: false,
                    exit: None,
                    killed: false,
                    milliseconds: 0,
                    output: format!("[runner] the suite could not be rebased: {error}"),
                    flaky: false,
                })
                .collect();
        }
        let mut runs = self.inner.run_all(tests, &scratch, workspace).await;
        for run in &mut runs {
            run.output = crate::compose::best_of::rebase(&run.output, &scratch, suite_dir);
        }
        let _ = std::fs::remove_dir_all(&scratch);
        runs
    }
}

/// The frozen suite's `env.sh`: runs one command in the workspace root.
#[must_use]
pub fn local_env_sh(workspace: &Path) -> String {
    format!(
        "#!/bin/sh\n# Runs one command in the workspace root: sh env.sh 'COMMAND'\ncd {} && exec sh -c \"$1\"\n",
        sh_quote(&workspace.display().to_string())
    )
}

impl Runner for Local {
    fn describe(&self) -> Value {
        json!({ "runner": "local", "confine": self.confine.word(), "test_sec": self.test_sec })
    }

    fn harness(&self, _suite_dir: &Path, workspace: &Path) -> Vec<(&'static str, String)> {
        vec![
            ("run.sh", local_run_sh(workspace, self.test_sec)),
            ("env.sh", local_env_sh(workspace)),
        ]
    }

    async fn run_all(&self, tests: &[Test], suite_dir: &Path, workspace: &Path) -> Vec<TestRun> {
        let mut out = Vec::new();
        for test in tests {
            out.push(self.one(test, suite_dir, workspace).await);
        }
        out
    }
}

impl Local {
    async fn one(&self, test: &Test, suite_dir: &Path, workspace: &Path) -> TestRun {
        let started = Instant::now();
        let script = suite_dir.join(TESTS_DIR).join(format!("{}.sh", test.id));
        let wall = Duration::from_secs(self.test_sec.max(1));
        let refused = |why: String| TestRun {
            id: test.id.clone(),
            flaky: false,
            requirements: test.requirements.clone(),
            green: false,
            exit: None,
            killed: false,
            milliseconds: 0,
            output: format!("[runner] the test did not run: {why}"),
        };
        let env = |command: &mut Command, scratch: &Path| {
            command
                .current_dir(workspace)
                .env("ACCEPT_DIR", suite_dir)
                .env("WORKSPACE", workspace)
                .env("ACCEPT_TMP", scratch)
                .env("TMPDIR", scratch);
        };
        let ended = match self.confine {
            Confine::Writing | Confine::ReadOnly => {
                let spec = if self.confine == Confine::Writing {
                    coder_boundary::Boundary::writing(workspace)
                } else {
                    coder_boundary::Boundary::readonly()
                };
                let boundary = match spec.owned_scratch_under(std::env::temp_dir()).build() {
                    Ok(boundary) => boundary,
                    Err(error) => return refused(format!("no enforced boundary: {error}")),
                };
                let Some(scratch) = boundary.scratch().map(Path::to_path_buf) else {
                    return refused("the boundary has no scratch directory".to_string());
                };
                let mut command = match boundary.command("/bin/sh", [script.as_os_str()]) {
                    Ok(command) => command,
                    Err(error) => return refused(error.to_string()),
                };
                env(&mut command, &scratch);
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                    .run_holding(boundary.hold())
                    .await
            }
            Confine::TaskContainer => {
                let scratch = std::env::temp_dir().join(format!(
                    "accept-{}-{}-{}",
                    std::process::id(),
                    test.id,
                    atif::now_ms()
                ));
                if let Err(error) = std::fs::create_dir_all(&scratch) {
                    return refused(format!("no scratch directory: {error}"));
                }
                let mut command = Command::new("/bin/sh");
                command.arg(&script);
                env(&mut command, &scratch);
                let ended = supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                    .run()
                    .await;
                let _ = std::fs::remove_dir_all(&scratch);
                ended
            }
        };
        let exit = match ended.ending {
            supervise::Ending::Exited(code) => code,
            _ => None,
        };
        let killed = matches!(ended.ending, supervise::Ending::TimedOut);
        let mut output = tail(&ended.stdout.marked(), &ended.stderr.marked());
        if let supervise::Ending::Failed(why) = &ended.ending {
            output = format!("[runner] the test did not run: {why}");
        }
        TestRun {
            id: test.id.clone(),
            flaky: false,
            requirements: test.requirements.clone(),
            green: exit == Some(0),
            exit,
            killed,
            milliseconds: millis(started),
            output,
        }
    }
}

/// Runs tests in a fresh container of a Docker image, with no network.
///
/// The workspace is the image's own `workdir`, or, with a candidate, the
/// candidate's files: `candidate` is a directory whose contents are
/// copied onto `/` after `workdir` is emptied, such as a restored
/// post-executor snapshot whose paths are relative to `/`. The suite is
/// copied to `/accept`.
#[derive(Clone, Debug)]
pub struct Docker {
    pub image: String,
    pub workdir: String,
    pub candidate: Option<PathBuf>,
    pub test_sec: u64,
    /// A running container the writing session's harness runs in, when
    /// there is one.
    pub dev: Option<String>,
    /// A command run in the workspace after the candidate is copied in and
    /// before any test, with network access, such as installing the
    /// packages a candidate's `requirements.txt` names, as a verifier that
    /// grades in a separate container does. The container then keeps its
    /// network for the tests.
    pub setup: Option<String>,
}

/// Runs `docker` with `args` and returns its standard output.
///
/// # Errors
///
/// Its standard error when it fails.
pub fn docker(args: &[&str]) -> Result<String, String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(|error| format!("cannot run docker: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "docker {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

impl Docker {
    /// Starts a container of the image that sleeps, with no network, and
    /// returns its ID.
    ///
    /// # Errors
    ///
    /// Docker's message when it can't.
    pub fn start(&self, name: Option<&str>) -> Result<String, String> {
        let network = if self.setup.is_some() {
            "bridge"
        } else {
            "none"
        };
        let mut args = vec!["create", "--network", network, "-w", &self.workdir];
        if let Some(name) = name {
            args.extend(["--name", name]);
        }
        args.extend(["--entrypoint", "sleep", &self.image, "infinity"]);
        let id = docker(&args)?;
        docker(&["start", &id])?;
        if let Some(candidate) = &self.candidate {
            // The image may run as another user: the host's own steps run
            // as root, and the candidate belongs to the user the image
            // runs as, who wrote it during the trial.
            let workdir = sh_quote(&self.workdir);
            let owner = docker(&["exec", &id, "sh", "-c", "echo \"$(id -u):$(id -g)\""])?;
            let clear = format!("find {workdir} -mindepth 1 -maxdepth 1 -exec rm -rf {{}} +");
            docker(&["exec", "-u", "0", &id, "sh", "-c", &clear])?;
            let from = format!("{}/.", candidate.display());
            docker(&["cp", &from, &format!("{id}:/")])?;
            let chown = format!("chown -R {} {workdir}", sh_quote(owner.trim()));
            docker(&["exec", "-u", "0", &id, "sh", "-c", &chown])?;
        }
        if let Some(setup) = &self.setup {
            // A failed setup is the candidate's problem, as it would be
            // the verifier's: the tests run and say so.
            let _ = docker(&[
                "exec",
                "-u",
                "0",
                "-w",
                &self.workdir,
                &id,
                "sh",
                "-c",
                setup,
            ]);
        }
        Ok(id)
    }

    fn setup(&self, suite_dir: &Path) -> Result<String, String> {
        let id = self.start(None)?;
        let copied = docker(&["exec", "-u", "0", &id, "mkdir", "-p", "/accept"])
            .and_then(|_| {
                docker(&[
                    "cp",
                    &format!("{}/.", suite_dir.display()),
                    &format!("{id}:/accept/"),
                ])
            })
            .and_then(|_| docker(&["exec", "-u", "0", &id, "chmod", "-R", "a+rX", "/accept"]));
        if let Err(error) = copied {
            let _ = docker(&["rm", "-f", &id]);
            return Err(error);
        }
        Ok(id)
    }

    fn exec_line(&self, container: &str, id: &str) -> Vec<String> {
        let inner = format!(
            "rm -rf \"$ACCEPT_TMP\"; mkdir -p \"$ACCEPT_TMP\"; \
             if command -v timeout >/dev/null 2>&1; then exec timeout {sec} sh \"/accept/{TESTS_DIR}/$0.sh\"; \
             else exec sh \"/accept/{TESTS_DIR}/$0.sh\"; fi",
            sec = self.test_sec.max(1)
        );
        vec![
            "exec".to_string(),
            "-w".to_string(),
            self.workdir.clone(),
            "-e".to_string(),
            "ACCEPT_DIR=/accept".to_string(),
            "-e".to_string(),
            format!("WORKSPACE={}", self.workdir),
            "-e".to_string(),
            format!("ACCEPT_TMP=/tmp/accept-{id}"),
            container.to_string(),
            "sh".to_string(),
            "-c".to_string(),
            inner,
            id.to_string(),
        ]
    }
}

impl Runner for Docker {
    fn describe(&self) -> Value {
        json!({
            "runner": "docker",
            "image": self.image,
            "workdir": self.workdir,
            "candidate": self.candidate,
            "test_sec": self.test_sec,
            "network": if self.setup.is_some() { "bridge" } else { "none" },
            "setup": self.setup,
        })
    }

    fn harness(&self, _suite_dir: &Path, _workspace: &Path) -> Vec<(&'static str, String)> {
        let Some(dev) = &self.dev else {
            return Vec::new();
        };
        let workdir = sh_quote(&self.workdir);
        let run = format!(
            r#"#!/bin/sh
# Runs the acceptance tests in the task's container, from the workspace
# root, as the host does. Usage: sh run.sh [TEST_ID ...]
here=$(cd "$(dirname "$0")" && pwd)
docker exec -u 0 {dev} sh -c 'rm -rf /accept && mkdir -p /accept' >/dev/null 2>&1 &&
  docker cp "$here/." {dev}:/accept/ >/dev/null 2>&1 &&
  docker exec -u 0 {dev} chmod -R a+rX /accept >/dev/null 2>&1 ||
  {{ echo "could not copy the suite into the task's container"; exit 2; }}
green=0
red=0
for test in "$here"/tests/*.sh; do
  [ -e "$test" ] || continue
  id=$(basename "$test" .sh)
  if [ $# -gt 0 ]; then
    case " $* " in *" $id "*) ;; *) continue ;; esac
  fi
  out=$(timeout {outer} docker exec -w {workdir} -e ACCEPT_DIR=/accept -e WORKSPACE={workdir} \
    -e ACCEPT_TMP=/tmp/accept-$id {dev} sh -c 'rm -rf "$ACCEPT_TMP"; mkdir -p "$ACCEPT_TMP"; \
    if command -v timeout >/dev/null 2>&1; then exec timeout {sec} sh "/accept/tests/$0.sh"; \
    else exec sh "/accept/tests/$0.sh"; fi' "$id" 2>&1)
  code=$?
  if [ "$code" -eq 0 ]; then
    green=$((green + 1))
    echo "GREEN $id"
  else
    red=$((red + 1))
    echo "RED   $id (exit $code)"
    printf '%s\n' "$out" | tail -n 12 | sed 's/^/      /'
  fi
done
echo "$green green, $red red"
"#,
            sec = self.test_sec.max(1),
            outer = self.test_sec.max(1) + 30,
        );
        let env = format!(
            "#!/bin/sh\n# Runs one command in the workspace root inside the task's container,\n# which has no network: sh env.sh 'COMMAND'\nexec docker exec -i -w {workdir} -e WORKSPACE={workdir} {dev} sh -c \"$1\"\n"
        );
        vec![("run.sh", run), ("env.sh", env)]
    }

    async fn run_all(&self, tests: &[Test], suite_dir: &Path, _workspace: &Path) -> Vec<TestRun> {
        let failed = |why: &str| {
            tests
                .iter()
                .map(|test| TestRun {
                    id: test.id.clone(),
                    flaky: false,
                    requirements: test.requirements.clone(),
                    green: false,
                    exit: None,
                    killed: false,
                    milliseconds: 0,
                    output: format!("[runner] {why}"),
                })
                .collect::<Vec<_>>()
        };
        if tests.is_empty() {
            return Vec::new();
        }
        let container = match self.setup(suite_dir) {
            Ok(id) => id,
            Err(error) => return failed(&error),
        };
        let mut out = Vec::new();
        for test in tests {
            let started = Instant::now();
            let mut command = Command::new("docker");
            command.args(self.exec_line(&container, &test.id));
            let ran = crate::minitask::process::run(
                command,
                Duration::from_secs(self.test_sec.max(1) + 30),
            )
            .await;
            out.push(TestRun {
                id: test.id.clone(),
                flaky: false,
                requirements: test.requirements.clone(),
                green: ran.code == Some(0),
                exit: ran.code,
                killed: ran.killed || ran.code == Some(124),
                milliseconds: millis(started),
                output: tail(&ran.stdout, &ran.stderr),
            });
        }
        let _ = docker(&["rm", "-f", &container]);
        out
    }
}
