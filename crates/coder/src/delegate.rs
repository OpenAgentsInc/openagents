//! Delegation: one bounded task handed to one executor, run beside others.
//!
//! A delegation is the step the `devin-fan-out-six` golden spends most of
//! its time in. Six read-only questions went to the Devin CLI on the
//! operator's computer and came back in 46.6 seconds of wall clock against
//! 80.7 seconds of summed agent time, which is the whole argument for the
//! shape: [`Delegator::fan_out`] runs the tasks concurrently under a stated
//! bound, and the bound is recorded next to the answers.
//!
//! # What this is not
//!
//! **Delegation is not offered to the model as a tool it may elect.** The
//! reference implementation measured that: a model-called capability got
//! zero calls across eighteen attempts on six task shapes, while its
//! declaration cost 2,307 bytes on every request of every turn. The call
//! site is the operator's sentence, reached through a program runtime, so
//! nothing here appears in a tool list or in an instruction block. Read
//! [`docs/programs.md`](../../../docs/programs.md).
//!
//! # The two things a first attempt gets wrong
//!
//! **`PATH` is not enough.** `devin` is on the operator's interactive
//! `PATH` and not on the one a spawned subshell inherits, so the first
//! recorded attempt failed six times out of six with `command not found`
//! on a machine that had the binary. [`resolve`] finds an absolute path
//! and [`Executor`] keeps it, which is also what
//! [NIP-CAP](../../../nips/openagents/NIP-CAP.md)'s `detect` field is for.
//!
//! **A present executor can still refuse a directory.** Six of six
//! delegations from a git worktree under `/private/tmp` came back with
//! `Refusing to run in an untrusted workspace`. The capability was
//! installed, detected, and unavailable for that directory. A refusal is
//! therefore its own [`Status`], carrying the code the executor's manifest
//! declares, rather than a failure like any other.
//!
//! # Refused, timed out, and failed are three different things
//!
//! [`gym::eval::classify`] draws this line for a decision door and the same
//! rule holds here: a typed refusal is the executor's own answer and
//! belongs in the record, and a failure carrying no code is the harness.
//! So [`Status`] separates [`Status::Refused`], which the executor said,
//! from [`Status::TimedOut`] and [`Status::Failed`], which it did, from
//! [`Status::Harness`], which nobody can read an answer out of. The trace
//! keeps all four apart.
//!
//! # Isolation
//!
//! A delegate that writes needs its own checkout, or six of them collide.
//! Worktree isolation is not built yet, so a task that asks for it — or a
//! task that says it writes — is refused before anything spawns rather
//! than run unisolated and recorded as though it were safe. Read-only
//! delegations need no isolation and are the path that works today.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::timeout;

/// The most delegations one fan-out runs at once, unless a caller says
/// otherwise. Six is the width the first recorded episode ran at.
pub const CONCURRENT_MAX: usize = 6;

/// The most output one delegation keeps, per stream, in bytes.
pub const OUTPUT_MAX: usize = 64 * 1024;

/// The capability slug the Devin CLI on this computer answers to, as
/// [NIP-CAP](../../../nips/openagents/NIP-CAP.md) names capabilities.
pub const DEVIN_LOCAL: &str = "devin-local";

/// The variable that points at a Devin binary, for an operator whose copy
/// is somewhere [`resolve`] does not look.
pub const DEVIN_ENV: &str = "CODER_DEVIN";

/// Directories searched after `PATH`, because a login shell's `PATH` is not
/// the one a spawned process inherits.
const EXTRA_BIN_DIRS: &[&str] = &[
    ".local/bin",
    ".bun/bin",
    ".cargo/bin",
    "bin",
    "/opt/homebrew/bin",
    "/usr/local/bin",
];

/// One refusal an executor declares: the code a host records, and the
/// phrase the executor prints when it refuses that way.
///
/// The phrase is matched rather than an exit code because the executor
/// publishes no typed code — it exits 1 and says why on stderr. Matching a
/// **declared** phrase from a manifest is not intent routing: the route is
/// already chosen, the executor is already named, and the phrase is a
/// bounded field of its description.
#[derive(Clone, Debug)]
pub struct Refusal {
    /// What the trace calls this refusal.
    pub code: String,
    /// The text the executor prints when it refuses this way.
    pub phrase: String,
}

impl Refusal {
    /// A declared refusal.
    #[must_use]
    pub fn new(code: &str, phrase: &str) -> Self {
        Refusal {
            code: code.to_string(),
            phrase: phrase.to_string(),
        }
    }
}

/// How to drive one executor: the local half of a capability manifest.
///
/// The fields a manifest publishes — what it enforces, what it cannot
/// enforce, whether it sees the repository — belong to the probe and the
/// admission check. What a delegation needs is narrower: which binary, at
/// which absolute path, with which arguments, and what it declares it
/// refuses.
#[derive(Clone, Debug)]
pub struct Executor {
    /// The capability slug, the name a trace records.
    pub capability: String,
    /// The absolute path to the binary. Never a bare name: see the module
    /// documentation.
    pub binary: PathBuf,
    /// The arguments that go before the prompt, as an argv. A manifest is
    /// untrusted input, so nothing here is ever a shell string.
    pub arguments: Vec<String>,
    /// What this executor declares it refuses, beyond being absent.
    pub refuses: Vec<Refusal>,
}

impl Executor {
    /// The Devin CLI on this computer, with its binary resolved.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the binary could not be resolved. An
    /// executor that is not installed is not a failure; it is a capability
    /// that is not an option, and the caller decides what that means.
    pub fn devin_local() -> Result<Self, String> {
        let binary = match std::env::var_os(DEVIN_ENV).filter(|path| !path.is_empty()) {
            Some(path) => PathBuf::from(path),
            None => resolve("devin")?,
        };
        Ok(Self::devin_at(binary))
    }

    /// The same adapter against a named binary, which is how a test drives
    /// it without the real CLI.
    #[must_use]
    pub fn devin_at(binary: impl Into<PathBuf>) -> Self {
        Executor {
            capability: DEVIN_LOCAL.to_string(),
            binary: binary.into(),
            // `devin -p -- <prompt>` is the non-interactive form: print the
            // answer and exit. The `--` keeps a prompt that starts with a
            // dash from being read as a flag.
            arguments: vec!["-p".to_string(), "--".to_string()],
            refuses: vec![Refusal::new(
                "untrusted_workspace",
                "Refusing to run in an untrusted workspace",
            )],
        }
    }

    /// The refusal a piece of output declares, when it declares one.
    #[must_use]
    pub fn refusal(&self, text: &str) -> Option<String> {
        self.refuses
            .iter()
            .find(|refusal| text.contains(&refusal.phrase))
            .map(|refusal| refusal.code.clone())
    }
}

/// Finds a binary's absolute path: `PATH` first, then the directories a
/// spawned process usually does not inherit.
///
/// # Errors
///
/// Returns a sentence naming the binary that was not found.
pub fn resolve(binary: &str) -> Result<PathBuf, String> {
    resolve_in(
        binary,
        &std::env::var_os("PATH").unwrap_or_default(),
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

/// [`resolve`] over a named search path, so a test can search a directory
/// it made rather than the machine's.
fn resolve_in(binary: &str, path: &OsStr, home: Option<&Path>) -> Result<PathBuf, String> {
    let extra = EXTRA_BIN_DIRS
        .iter()
        .filter_map(|dir| match Path::new(dir).is_absolute() {
            true => Some(PathBuf::from(dir)),
            false => home.map(|home| home.join(dir)),
        });
    for dir in std::env::split_paths(path).chain(extra) {
        let candidate = dir.join(binary);
        if executable(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "{binary} is not on PATH or in the usual bin directories"
    ))
}

/// Whether a path is a file this process may run.
#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|file| file.is_file() && file.permissions().mode() & 0o111 != 0)
}

/// Whether a path is a file this process may run.
#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|file| file.is_file())
}

/// What a delegation may spend.
///
/// The wall bound is what the host enforces; `declared` is what the trace
/// records, so a bound stated in minutes reads back as minutes rather than
/// as however many seconds that came to.
#[derive(Clone, Debug)]
pub struct Bounds {
    wall: Duration,
    declared: Value,
}

impl Bounds {
    /// A bound in whole minutes, which is the form a manifest's `enforces`
    /// list names and the form the golden records.
    #[must_use]
    pub fn minutes(minutes: u64) -> Self {
        Bounds {
            wall: Duration::from_secs(minutes * 60),
            declared: json!({ "minutes": minutes }),
        }
    }

    /// A bound finer than a minute, for a caller that has one.
    #[must_use]
    pub fn within(wall: Duration) -> Self {
        Bounds {
            declared: json!({ "seconds": wall.as_secs_f64() }),
            wall,
        }
    }

    /// How long the delegation may run before the host kills it.
    #[must_use]
    pub fn wall(&self) -> Duration {
        self.wall
    }

    /// The bound as the trace records it.
    #[must_use]
    pub fn declared(&self) -> &Value {
        &self.declared
    }
}

/// Which checkout shape a delegation runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    /// The caller's working directory, shared with everything else in the
    /// fan-out. Correct for a delegate that only reads.
    Directory,
    /// A checkout of its own, so a delegate that writes cannot collide
    /// with its five siblings. Not built yet; a task that asks for it is
    /// refused rather than run in the shared directory.
    Worktree,
}

impl Isolation {
    /// The word the trace spells this with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Isolation::Directory => "directory",
            Isolation::Worktree => "worktree",
        }
    }
}

/// One bounded task for one executor.
#[derive(Clone, Debug)]
pub struct Task {
    /// What the executor is asked, verbatim.
    pub prompt: String,
    /// What the task is for, in the words the trace shows beside it.
    pub purpose: String,
    /// The file the task reads, when it reads one.
    pub reads: Option<String>,
    /// The answer the caller expects, when the caller knows it. This is
    /// what makes a delegation gradeable.
    pub expected: Option<String>,
    /// What the delegation may spend.
    pub bounds: Bounds,
    /// Which checkout shape it runs in.
    pub isolation: Isolation,
    /// Whether the task is allowed to write. A task that writes needs its
    /// own checkout.
    pub writes: bool,
}

impl Task {
    /// A read-only question about one file, which is the shape the first
    /// task fans out six of.
    #[must_use]
    pub fn reading(prompt: &str, reads: &str) -> Self {
        Task {
            prompt: prompt.to_string(),
            purpose: format!("Read {reads} and answer one question."),
            reads: Some(reads.to_string()),
            expected: None,
            bounds: Bounds::minutes(5),
            isolation: Isolation::Directory,
            writes: false,
        }
    }

    /// The answer this task is graded against.
    #[must_use]
    pub fn expecting(mut self, expected: &str) -> Self {
        self.expected = Some(expected.to_string());
        self
    }

    /// The bound this task runs under.
    #[must_use]
    pub fn bounded(mut self, bounds: Bounds) -> Self {
        self.bounds = bounds;
        self
    }

    /// Why a task cannot be run as it stands, when it cannot.
    ///
    /// Both answers are about isolation, and both are refusals rather than
    /// failures: nothing was attempted, and the reason is a property of
    /// the request.
    #[must_use]
    pub fn unisolated(&self) -> Option<&'static str> {
        match (self.isolation, self.writes) {
            (Isolation::Worktree, _) => Some("isolation_unavailable"),
            (Isolation::Directory, true) => Some("isolation_required"),
            (Isolation::Directory, false) => None,
        }
    }

    /// The first characters of the prompt, for the line a reader sees.
    #[must_use]
    pub fn head(&self, max: usize) -> &str {
        head(&self.prompt, max)
    }
}

/// How a delegation ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The executor ran the task and answered.
    Answered,
    /// The executor declined, with a refusal it declares. This is its own
    /// answer, not a failure of the machinery around it.
    Refused(String),
    /// The bound expired and the host killed it.
    TimedOut,
    /// The executor ran and exited non-zero.
    Failed(i32),
    /// The host never got an answer: the binary would not spawn, or there
    /// was no binary to spawn. Nobody can say what the executor would have
    /// answered.
    Harness(String),
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Answered => write!(f, "answered"),
            Status::Refused(code) => write!(f, "refused: {code}"),
            Status::TimedOut => write!(f, "timed out"),
            Status::Failed(code) => write!(f, "failed: exit {code}"),
            Status::Harness(why) => write!(f, "harness: {why}"),
        }
    }
}

/// One delegated session and everything that came back from it.
#[derive(Clone, Debug)]
pub struct Delegation {
    /// The task that was handed over.
    pub task: Task,
    /// The capability that took it.
    pub capability: String,
    /// The binary that ran, by absolute path.
    pub binary: PathBuf,
    /// The directory it ran in.
    pub workdir: PathBuf,
    /// The fan-out width this ran under, which is the bound on concurrency.
    pub concurrent_max: usize,
    /// How it ended.
    pub status: Status,
    /// What the executor printed on stdout, capped at [`OUTPUT_MAX`].
    pub output: String,
    /// What it printed on stderr, capped at [`OUTPUT_MAX`]. A refusal
    /// arrives here.
    pub detail: String,
    /// Wall time the delegation took.
    pub elapsed: Duration,
}

impl Delegation {
    /// Whether the executor answered.
    #[must_use]
    pub fn answered(&self) -> bool {
        self.status == Status::Answered
    }

    /// Whether the delegate answered correctly, when the task said what to
    /// expect. A delegation that did not answer is not correct, and one
    /// with nothing to compare against is neither.
    #[must_use]
    pub fn correct(&self) -> Option<bool> {
        let expected = self.task.expected.as_deref()?;
        Some(self.answered() && normalize(&self.output) == normalize(expected))
    }

    /// What the trace records as the call's output: the answer when there
    /// is one, and the reason when there is not.
    #[must_use]
    pub fn recorded_output(&self) -> String {
        if self.answered() {
            return self.output.trim().to_string();
        }
        match self.detail.trim() {
            "" => self.status.to_string(),
            detail => detail.to_string(),
        }
    }

    /// How the document records this outcome.
    ///
    /// A refusal is `Cancelled`, the outcome ATIF keeps for a call that
    /// never ran because something refused it first. A timeout and a
    /// non-zero exit are `Failed`, and the `status` field in the call's
    /// `extra` says which.
    #[must_use]
    pub fn outcome(&self) -> atif::Outcome {
        match self.status {
            Status::Answered => atif::Outcome::Completed,
            Status::Refused(_) => atif::Outcome::Cancelled,
            _ => atif::Outcome::Failed,
        }
    }

    /// The display line for the scrollback: `answered · 6.3s`.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} · {:.1}s", self.status, self.elapsed.as_secs_f64())
    }
}

/// One executor, one working directory, and the width a fan-out may reach.
#[derive(Clone, Debug)]
pub struct Delegator {
    executor: Executor,
    workdir: PathBuf,
    concurrent_max: usize,
}

impl Delegator {
    /// A delegator for one executor, running in this process's working
    /// directory at the default width.
    #[must_use]
    pub fn new(executor: Executor) -> Self {
        Delegator {
            executor,
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            concurrent_max: CONCURRENT_MAX,
        }
    }

    /// Runs the delegations somewhere other than this process's working
    /// directory.
    #[must_use]
    pub fn in_directory(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.workdir = workdir.into();
        self
    }

    /// Bounds how many delegations run at once. Zero is read as one.
    #[must_use]
    pub fn bounded_to(mut self, concurrent_max: usize) -> Self {
        self.concurrent_max = concurrent_max.max(1);
        self
    }

    /// The executor these delegations reach.
    #[must_use]
    pub fn executor(&self) -> &Executor {
        &self.executor
    }

    /// The directory the delegations run in.
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// The most delegations this fans out at once.
    #[must_use]
    pub fn concurrent_max(&self) -> usize {
        self.concurrent_max
    }

    /// Hands one task to the executor and waits for it.
    pub async fn run(&self, task: Task) -> Delegation {
        if let Some(code) = task.unisolated() {
            return self.ended(
                task,
                Status::Refused(code.to_string()),
                String::new(),
                String::new(),
                Duration::ZERO,
            );
        }
        let started = Instant::now();
        let running = Command::new(&self.executor.binary)
            .args(&self.executor.arguments)
            .arg(&task.prompt)
            .current_dir(&self.workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // A bound that expires kills the delegate. Without this the
            // host stops waiting and the executor keeps running against
            // the repository, which makes the bound a timer rather than a
            // bound — and leaves a process nobody is reaping behind every
            // timed-out delegation.
            .kill_on_drop(true)
            .output();
        let finished = timeout(task.bounds.wall(), running).await;
        let elapsed = started.elapsed();
        let (status, output, detail) = match finished {
            Ok(Ok(done)) => {
                let output = cap(&String::from_utf8_lossy(&done.stdout));
                let detail = cap(&String::from_utf8_lossy(&done.stderr));
                let status = match self.executor.refusal(&format!("{detail}{output}")) {
                    Some(code) => Status::Refused(code),
                    None if done.status.success() => Status::Answered,
                    None => Status::Failed(done.status.code().unwrap_or(-1)),
                };
                (status, output, detail)
            }
            // The process never became a result. That is the harness, not
            // the executor, and nothing about it is the executor's answer.
            Ok(Err(error)) => (
                Status::Harness(error.to_string()),
                String::new(),
                error.to_string(),
            ),
            Err(_) => (Status::TimedOut, String::new(), String::new()),
        };
        self.ended(task, status, output, detail, elapsed)
    }

    /// Hands every task to the executor at once, no more than
    /// [`Delegator::concurrent_max`] of them running at a time.
    ///
    /// Concurrency is the point rather than a later optimization: six
    /// sequential delegations took 80.7 seconds and six parallel ones took
    /// 46.6. The results come back in the order the tasks were given,
    /// whatever order they finished in, so a caller can pair them with what
    /// it asked.
    pub async fn fan_out(&self, tasks: Vec<Task>) -> Vec<Delegation> {
        stream::iter(tasks.into_iter().map(|task| self.run(task)))
            .buffered(self.concurrent_max)
            .collect()
            .await
    }

    /// Assembles one finished delegation.
    fn ended(
        &self,
        task: Task,
        status: Status,
        output: String,
        detail: String,
        elapsed: Duration,
    ) -> Delegation {
        Delegation {
            task,
            capability: self.executor.capability.clone(),
            binary: self.executor.binary.clone(),
            workdir: self.workdir.clone(),
            concurrent_max: self.concurrent_max,
            status,
            output,
            detail,
            elapsed,
        }
    }
}

/// The first `max` characters of a string, cut on a character boundary.
fn head(text: &str, max: usize) -> &str {
    match text.len() <= max {
        true => text,
        false => {
            let mut end = max;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            &text[..end]
        }
    }
}

/// One stream of output, bounded. The bound is on what the process holds,
/// which is the only place a cap can be applied without losing the fact
/// that there was more.
fn cap(text: &str) -> String {
    if text.len() <= OUTPUT_MAX {
        return text.to_string();
    }
    let mut end = OUTPUT_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…truncated", &text[..end])
}

/// An answer as it is compared: trimmed, lowercased, and with runs of
/// whitespace flattened, so `L1, L2, L3` and `l1,  l2, l3` do not differ.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// Writes an executable stub that stands in for the executor.
    fn stub(dir: &Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh\n{script}").unwrap();
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    /// The bound a case that is not about timing out runs under. Generous,
    /// because a test machine running its other tests at the same time
    /// spawns a process slowly enough to matter.
    const BOUND: Duration = Duration::from_secs(20);

    fn six_tasks() -> Vec<Task> {
        (1..=6)
            .map(|n| Task::reading(&format!("question {n}"), &format!("file{n}.rs")))
            .collect()
    }

    /// The answer is stdout, and it is graded against what the task
    /// expected.
    #[tokio::test]
    async fn a_delegate_answers_and_is_graded() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "devin", "printf '5\\n'");
        let delegator = Delegator::new(Executor::devin_at(&binary)).in_directory(dir.path());

        let right = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs").expecting("5"))
            .await;
        assert_eq!(right.status, Status::Answered);
        assert_eq!(right.recorded_output(), "5");
        assert_eq!(right.correct(), Some(true));
        assert_eq!(right.outcome(), atif::Outcome::Completed);

        let wrong = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs").expecting("6"))
            .await;
        assert_eq!(wrong.correct(), Some(false));

        // A task that says nothing about the answer is not graded.
        let ungraded = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs"))
            .await;
        assert_eq!(ungraded.correct(), None);
    }

    /// A refusal, a timeout, and a non-zero exit are three different
    /// things, and the delegation says which.
    #[tokio::test]
    async fn refusal_timeout_and_failure_stay_apart() {
        let dir = tempfile::tempdir().unwrap();
        // A bound loose enough that a busy machine does not turn one of
        // these cases into the timeout case. The timeout case states its
        // own.
        let task = || Task::reading("anything", "a.rs").bounded(Bounds::within(BOUND));

        let refusing = stub(
            dir.path(),
            "refusing",
            "echo 'Error: Refusing to run in an untrusted workspace: /private/tmp' >&2\nexit 1",
        );
        let refused = Delegator::new(Executor::devin_at(&refusing))
            .run(task())
            .await;
        assert_eq!(
            refused.status,
            Status::Refused("untrusted_workspace".into())
        );
        // A refusal is the executor's own answer, so ATIF records the call
        // as one that never ran rather than as one that failed.
        assert_eq!(refused.outcome(), atif::Outcome::Cancelled);
        assert!(refused.recorded_output().contains("untrusted workspace"));

        let slow = stub(dir.path(), "slow", "sleep 30");
        let timed_out = Delegator::new(Executor::devin_at(&slow))
            .run(
                Task::reading("anything", "a.rs")
                    .bounded(Bounds::within(Duration::from_millis(250))),
            )
            .await;
        assert_eq!(timed_out.status, Status::TimedOut);
        assert_eq!(timed_out.outcome(), atif::Outcome::Failed);

        let broken = stub(dir.path(), "broken", "echo 'boom' >&2\nexit 3");
        let failed = Delegator::new(Executor::devin_at(&broken))
            .run(task())
            .await;
        assert_eq!(failed.status, Status::Failed(3));
        assert_eq!(failed.outcome(), atif::Outcome::Failed);

        // No binary at all is the harness, not the executor.
        let missing = Delegator::new(Executor::devin_at(dir.path().join("absent")))
            .run(task())
            .await;
        assert!(matches!(missing.status, Status::Harness(_)));
        assert_eq!(missing.correct(), None);
    }

    /// A refusal is not an answer, so a graded task that was refused is
    /// wrong rather than unjudged.
    #[tokio::test]
    async fn a_refused_delegate_did_not_answer_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(
            dir.path(),
            "devin",
            "echo 'Refusing to run in an untrusted workspace' >&2\nexit 1",
        );
        let delegation = Delegator::new(Executor::devin_at(&binary))
            .run(Task::reading("how many", "a.rs").expecting("5"))
            .await;
        assert_eq!(delegation.correct(), Some(false));
    }

    /// Six delegations run at once, and six at a width of one do not.
    #[tokio::test]
    async fn the_fan_out_is_concurrent_under_its_bound() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "devin", "sleep 0.4\nprintf 'done\\n'");
        let executor = Executor::devin_at(&binary);

        let started = Instant::now();
        let wide = Delegator::new(executor.clone())
            .bounded_to(6)
            .fan_out(six_tasks())
            .await;
        let parallel = started.elapsed();
        assert_eq!(wide.len(), 6);
        assert!(wide.iter().all(Delegation::answered));
        assert!(
            parallel < Duration::from_millis(1_500),
            "six 0.4s delegations at a width of six should not take {parallel:?}"
        );

        let started = Instant::now();
        let narrow = Delegator::new(executor)
            .bounded_to(1)
            .fan_out(six_tasks())
            .await;
        let sequential = started.elapsed();
        assert_eq!(narrow.len(), 6);
        assert!(
            sequential > parallel,
            "a width of one is the sequential case: {sequential:?} against {parallel:?}"
        );
    }

    /// Results come back in the order the tasks were given, whatever order
    /// they finished in.
    #[tokio::test]
    async fn results_keep_the_order_the_tasks_were_given() {
        let dir = tempfile::tempdir().unwrap();
        // The first task is the slowest, so a fan-out that reported in
        // completion order would put it last.
        // The prompt is the last argument, after the executor's own.
        let binary = stub(
            dir.path(),
            "devin",
            "for a in \"$@\"; do prompt=\"$a\"; done\ncase \"$prompt\" in *' 1') sleep 0.5 ;; esac\nprintf '%s\\n' \"$prompt\"",
        );
        let delegations = Delegator::new(Executor::devin_at(&binary))
            .bounded_to(6)
            .fan_out(six_tasks())
            .await;
        let answers: Vec<&str> = delegations.iter().map(|d| d.output.trim()).collect();
        assert_eq!(
            answers,
            vec![
                "question 1",
                "question 2",
                "question 3",
                "question 4",
                "question 5",
                "question 6"
            ]
        );
    }

    /// A delegate that would write, or that asks for a worktree, is
    /// refused before anything spawns rather than run in the shared
    /// directory.
    #[tokio::test]
    async fn an_unisolated_write_is_refused_before_it_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "devin", "printf 'wrote it\\n'");
        let delegator = Delegator::new(Executor::devin_at(&binary));

        let mut writing = Task::reading("change a file", "a.rs");
        writing.writes = true;
        let refused = delegator.run(writing).await;
        assert_eq!(refused.status, Status::Refused("isolation_required".into()));
        assert!(refused.output.is_empty(), "nothing ran");

        let mut isolated = Task::reading("change a file", "a.rs");
        isolated.isolation = Isolation::Worktree;
        let unavailable = delegator.run(isolated).await;
        assert_eq!(
            unavailable.status,
            Status::Refused("isolation_unavailable".into())
        );
    }

    /// The resolver returns an absolute path, and finds a binary in a
    /// directory that is not on the search path at all — which is the case
    /// that broke the first recorded attempt.
    #[test]
    fn the_resolver_returns_an_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join(".local/bin")).unwrap();
        stub(dir.path(), "on-the-path", "true");
        stub(&home.join(".local/bin"), "off-the-path", "true");
        let path = std::env::join_paths([dir.path()]).unwrap();

        let found = resolve_in("on-the-path", &path, Some(&home)).unwrap();
        assert!(found.is_absolute());
        assert!(executable(&found));

        let elsewhere = resolve_in("off-the-path", &path, Some(&home)).unwrap();
        assert_eq!(elsewhere, home.join(".local/bin/off-the-path"));

        assert!(resolve_in("absent", &path, Some(&home)).is_err());
        // A file nobody may run is not the binary.
        std::fs::write(dir.path().join("not-executable"), "").unwrap();
        assert!(resolve_in("not-executable", &path, Some(&home)).is_err());
    }

    #[test]
    fn answers_compare_without_their_whitespace() {
        assert_eq!(normalize(" L1,  L2, L3\n"), "l1, l2, l3");
        assert_eq!(normalize("5"), normalize(" 5 "));
    }

    #[test]
    fn a_bound_reads_back_the_way_it_was_stated() {
        let minutes = Bounds::minutes(5);
        assert_eq!(minutes.wall(), Duration::from_secs(300));
        assert_eq!(minutes.declared(), &json!({ "minutes": 5 }));
        let short = Bounds::within(Duration::from_millis(250));
        assert_eq!(short.wall(), Duration::from_millis(250));
    }
}
