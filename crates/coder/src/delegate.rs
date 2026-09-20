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
//! on a machine that had the binary. So an [`Executor`] carries an
//! absolute path and never a name, and the path comes from the capability
//! probe rather than from a search written here: the probe resolves it,
//! [`crate::survey::executor`] hands it over, and this module runs it.
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
//! # Isolation is provided or refused, never pretended
//!
//! A delegate that writes needs a checkout of its own, or six of them
//! collide. A [`Delegator`] told which checkout it is working in makes one
//! worktree per delegation and removes it afterwards; one that was not
//! refuses a task asking for a worktree, and refuses a task that says it
//! writes without one. Both refusals land before anything spawns, so
//! nothing records `isolation: worktree` and runs in the shared
//! directory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream;
use serde_json::{Value, json};
use supervise::{Ending, Job, Limits};

pub use crate::worktree::Worktree;

/// The most delegations one fan-out runs at once, unless a caller says
/// otherwise. Six is the width the first recorded episode ran at.
pub const CONCURRENT_MAX: usize = 6;

/// The most output one delegation keeps, per stream, in bytes.
///
/// The cap is applied as the bytes arrive rather than to the string at the
/// end, so it bounds what this process holds and not only what the trace
/// records. Bytes past it are counted and dropped, and
/// [`Delegation::bytes`] says how many there were.
pub const OUTPUT_MAX: usize = 64 * 1024;

/// The capability slug the Devin CLI on this computer answers to, as
/// [NIP-CAP](../../../nips/openagents/NIP-CAP.md) names capabilities.
pub const DEVIN_LOCAL: &str = "devin-local";

/// The directory, under the repository, that worktrees are made in.
///
/// Under the repository rather than under the system temporary directory,
/// because a checkout somewhere else is a directory nobody has trusted:
/// `devin` declines `/private/tmp/…` and accepts a worktree inside a
/// checkout it already trusts.
pub const WORKTREE_DIR: &str = ".coder/worktrees";

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
///
/// **Nothing here names an executor.** An `Executor` is built by
/// [`crate::survey::executor`] from a probed manifest, so the binary is
/// the absolute path the probe resolved and the arguments are the
/// manifest's `invoke`. A constructor that wrote a binary name and an argv
/// into this file would be a second source of truth for how to drive an
/// executor, next to the manifest that exists to be the first.
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
    /// An executor over a resolved binary and the arguments that go before
    /// the prompt.
    #[must_use]
    pub fn new(capability: &str, binary: impl Into<PathBuf>, arguments: Vec<String>) -> Self {
        Executor {
            capability: capability.to_string(),
            binary: binary.into(),
            arguments,
            refuses: Vec::new(),
        }
    }

    /// Adds one refusal this executor declares.
    #[must_use]
    pub fn refusing(mut self, code: &str, phrase: &str) -> Self {
        self.refuses.push(Refusal::new(code, phrase));
        self
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
    /// with its five siblings. A delegator that knows which checkout it
    /// works in makes one and removes it afterwards; one that does not
    /// refuses the task rather than running it in the shared directory.
    Worktree,
}

impl Isolation {
    /// The shape a word names, or `None` for a word this host has no
    /// shape for. A program bound naming one of those is a bound the host
    /// cannot enforce, and the step it bounds does not run.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "directory" => Some(Isolation::Directory),
            "worktree" => Some(Isolation::Worktree),
            _ => None,
        }
    }

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
    /// One bounded piece of work, stated in words, saying nothing about
    /// which file it reads.
    ///
    /// The shape an operator's own list gives: a line of the request is a
    /// task, and the line does not have to name a path. A task that names
    /// the file it reads says so through [`Task::reading`].
    #[must_use]
    pub fn asking(prompt: &str) -> Self {
        Task {
            prompt: prompt.to_string(),
            purpose: "Do one item of the work the request lists.".to_string(),
            reads: None,
            expected: None,
            bounds: Bounds::minutes(5),
            isolation: Isolation::Directory,
            writes: false,
        }
    }

    /// A read-only question about one file, which is the shape the first
    /// task fans out six of.
    #[must_use]
    pub fn reading(prompt: &str, reads: &str) -> Self {
        Task {
            purpose: format!("Read {reads} and answer one question."),
            reads: Some(reads.to_string()),
            ..Task::asking(prompt)
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

    /// Why a task contradicts itself, when it does.
    ///
    /// A task that writes into the directory it shares with five siblings
    /// has asked for something nobody can grant, whatever the host can
    /// provide. That is a refusal rather than a failure: nothing was
    /// attempted, and the reason is a property of the request.
    ///
    /// Whether the isolation a task *does* ask for is available is the
    /// host's answer, not the task's, and [`Delegator::unisolated`] gives
    /// it.
    #[must_use]
    pub fn contradictory(&self) -> Option<&'static str> {
        match (self.isolation, self.writes) {
            (Isolation::Directory, true) => Some("isolation_required"),
            _ => None,
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
    /// What the executor printed on stdout, capped at [`OUTPUT_MAX`] and
    /// marked when the cap cut it.
    pub output: String,
    /// What it printed on stderr, capped the same way. A refusal arrives
    /// here.
    pub detail: String,
    /// How many bytes the executor printed across both streams, before
    /// the caps.
    pub bytes: u64,
    /// Wall time the delegation took, cleanup included.
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
    /// The checkout a worktree branches from, when this host knows one.
    /// `None` is a host that cannot isolate, and it refuses rather than
    /// sharing a directory it was asked not to.
    repository: Option<PathBuf>,
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
            repository: None,
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

    /// Runs the delegations in a checkout this host may branch worktrees
    /// from, which is what makes `isolation: worktree` something the host
    /// provides rather than something it refuses.
    ///
    /// The directory is both the shared working directory and the
    /// repository a worktree comes from, because they are the same
    /// checkout: the caller is saying which repository the fan-out is
    /// working on.
    #[must_use]
    pub fn in_repository(mut self, root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        self.workdir = root.clone();
        self.repository = Some(root);
        self
    }

    /// Whether this host can give a delegation the checkout shape it
    /// names.
    #[must_use]
    pub fn provides(&self, isolation: Isolation) -> bool {
        match isolation {
            Isolation::Directory => true,
            Isolation::Worktree => self.repository.is_some(),
        }
    }

    /// Why this host cannot run a task as it stands, when it cannot.
    ///
    /// Two answers, both refusals and both before anything spawns: the
    /// task contradicts itself, or it asks for a checkout shape this host
    /// has no way to make.
    #[must_use]
    pub fn unisolated(&self, task: &Task) -> Option<&'static str> {
        task.contradictory()
            .or(match self.provides(task.isolation) {
                true => None,
                false => Some("isolation_unavailable"),
            })
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
    ///
    /// A task asking for a checkout of its own gets one, made before the
    /// executor spawns and removed when the delegation ends, whatever it
    /// ended as.
    pub async fn run(&self, task: Task) -> Delegation {
        if let Some(code) = self.unisolated(&task) {
            return self.ended(
                task,
                Reported::of(Status::Refused(code.to_string())),
                Duration::ZERO,
                self.workdir.clone(),
            );
        }
        let started = Instant::now();
        // A worktree that will not be made is the harness rather than the
        // executor: nobody can say what the delegate would have answered,
        // and a host that ran the task in the shared directory instead
        // would be recording an isolation it did not provide.
        let checkout = match self.checkout(&task).await {
            Ok(checkout) => checkout,
            Err(why) => {
                return self.ended(
                    task,
                    Reported::of(Status::Harness(why.clone())).detailing(why),
                    started.elapsed(),
                    self.workdir.clone(),
                );
            }
        };
        let workdir = checkout.path(&self.workdir).to_path_buf();
        // A bound the host cannot enforce is a timer. [`supervise`] makes
        // this one a bound: the executor runs in a process group of its
        // own, an expired bound or a cancelled caller terminates that
        // group rather than abandoning the wait, and the direct child is
        // reaped before this returns — which is also why the checkout
        // below is removed after the executor is gone rather than while it
        // is still writing to it.
        let ended = Job::new(&self.executor.binary)
            .args(self.executor.arguments.iter().map(String::as_str))
            .arg(&task.prompt)
            .in_directory(&workdir)
            .bounded(Limits::within(task.bounds.wall()).keeping(OUTPUT_MAX))
            .run_holding(checkout.hold())
            .await;
        let said = Reported {
            status: Status::Answered,
            output: ended.stdout.marked(),
            detail: ended.stderr.marked(),
            bytes: ended.bytes(),
        };
        let reported = match ended.ending {
            Ending::Exited(code) => {
                let status = match self
                    .executor
                    .refusal(&format!("{}{}", said.detail, said.output))
                {
                    Some(code) => Status::Refused(code),
                    None if code == Some(0) => Status::Answered,
                    None => Status::Failed(code.unwrap_or(-1)),
                };
                Reported { status, ..said }
            }
            // The bound expired, and what the executor printed before it
            // did is kept: a timed-out delegation's partial output is
            // often the only account of what it was doing.
            Ending::TimedOut => Reported {
                status: Status::TimedOut,
                ..said
            },
            // The process never became a result. That is the harness, not
            // the executor, and nothing about it is the executor's answer.
            Ending::Failed(why) => Reported::of(Status::Harness(why.clone())).detailing(why),
        };
        let reported = match checkout.close().await {
            Ok(()) => reported,
            Err(error) => Reported {
                status: Status::Harness(error.clone()),
                detail: format!("{}\n{error}", reported.detail),
                ..reported
            },
        };
        self.ended(task, reported, started.elapsed(), workdir)
    }

    /// The checkout one task runs in.
    async fn checkout(&self, task: &Task) -> Result<Checkout, String> {
        match (task.isolation, &self.repository) {
            (Isolation::Worktree, Some(repository)) => {
                Worktree::add(repository)
                    .await
                    .map(|worktree| Checkout::Own(Arc::new(worktree)))
            }
            _ => Ok(Checkout::Shared),
        }
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
        reported: Reported,
        elapsed: Duration,
        workdir: PathBuf,
    ) -> Delegation {
        Delegation {
            task,
            capability: self.executor.capability.clone(),
            binary: self.executor.binary.clone(),
            workdir,
            concurrent_max: self.concurrent_max,
            status: reported.status,
            output: reported.output,
            detail: reported.detail,
            bytes: reported.bytes,
            elapsed,
        }
    }
}

/// What one delegation came back as, before it is paired with the task
/// and the host that ran it.
#[derive(Debug)]
struct Reported {
    status: Status,
    output: String,
    detail: String,
    bytes: u64,
}

impl Reported {
    /// A delegation that ended before the executor printed anything, or
    /// before it ran at all.
    fn of(status: Status) -> Self {
        Reported {
            status,
            output: String::new(),
            detail: String::new(),
            bytes: 0,
        }
    }

    /// The same, with the reason on the stream a reason arrives on.
    fn detailing(mut self, detail: String) -> Self {
        self.detail = detail;
        self
    }
}

/// Where one delegation runs.
#[derive(Debug)]
enum Checkout {
    /// The delegator's own directory, shared with the fan-out's siblings.
    /// Correct for a delegate that only reads.
    Shared,
    /// A checkout of this delegation's own, removed when it ends.
    Own(Arc<Worktree>),
}

impl Checkout {
    fn hold(&self) -> Option<Arc<Worktree>> {
        match self {
            Checkout::Shared => None,
            Checkout::Own(worktree) => Some(Arc::clone(worktree)),
        }
    }

    async fn close(self) -> Result<(), String> {
        match self {
            Checkout::Shared => Ok(()),
            Checkout::Own(worktree) => Arc::try_unwrap(worktree)
                .map_err(|_| "the executor still holds its worktree".to_string())?
                .close()
                .await,
        }
    }

    /// The directory the executor runs in.
    fn path<'a>(&'a self, shared: &'a Path) -> &'a Path {
        match self {
            Checkout::Shared => shared,
            Checkout::Own(worktree) => worktree.path(),
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

    /// A checkout with one commit, for the cases about isolation. `None`
    /// on a machine with no working version control, where the case is
    /// about something this test cannot set up rather than about the code.
    fn scratch_repository() -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().ok()?;
        std::fs::write(dir.path().join("a.rs"), "// a file\n").ok()?;
        let run = |arguments: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(arguments)
                .output()
                .ok()
                .filter(|done| done.status.success())
        };
        run(&["init", "--quiet"])?;
        run(&["config", "user.email", "test@example.invalid"])?;
        run(&["config", "user.name", "A Test"])?;
        run(&["add", "a.rs"])?;
        run(&["commit", "--quiet", "-m", "one file"])?;
        Some(dir)
    }

    /// The bound a case that is not about timing out runs under. Generous,
    /// because a test machine running its other tests at the same time
    /// spawns a process slowly enough to matter.
    const BOUND: Duration = Duration::from_secs(20);

    /// An executor over a stub, the way `survey::executor` builds one over
    /// a probed manifest: a resolved path, the arguments that manifest
    /// names, and the refusal it declares. The stub takes no arguments
    /// before the prompt, and nothing here names a real executor.
    fn executor(binary: impl Into<PathBuf>) -> Executor {
        Executor::new("stub-local", binary, Vec::new()).refusing(
            "untrusted_workspace",
            "Refusing to run in an untrusted workspace",
        )
    }

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
        let delegator = Delegator::new(executor(&binary)).in_directory(dir.path());

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
        let refused = Delegator::new(executor(&refusing)).run(task()).await;
        assert_eq!(
            refused.status,
            Status::Refused("untrusted_workspace".into())
        );
        // A refusal is the executor's own answer, so ATIF records the call
        // as one that never ran rather than as one that failed.
        assert_eq!(refused.outcome(), atif::Outcome::Cancelled);
        assert!(refused.recorded_output().contains("untrusted workspace"));

        let slow = stub(dir.path(), "slow", "sleep 30");
        let timed_out = Delegator::new(executor(&slow))
            .run(
                Task::reading("anything", "a.rs")
                    .bounded(Bounds::within(Duration::from_millis(250))),
            )
            .await;
        assert_eq!(timed_out.status, Status::TimedOut);
        assert_eq!(timed_out.outcome(), atif::Outcome::Failed);

        let broken = stub(dir.path(), "broken", "echo 'boom' >&2\nexit 3");
        let failed = Delegator::new(executor(&broken)).run(task()).await;
        assert_eq!(failed.status, Status::Failed(3));
        assert_eq!(failed.outcome(), atif::Outcome::Failed);

        // No binary at all is the harness, not the executor.
        let missing = Delegator::new(executor(dir.path().join("absent")))
            .run(task())
            .await;
        assert!(matches!(missing.status, Status::Harness(_)));
        assert_eq!(missing.correct(), None);
    }

    /// The audit's delegation probe: an executor that starts a background
    /// child and is killed on its bound. Killing the direct delegate is
    /// not enough — the child has its own process, and it writes its
    /// marker a second later.
    #[tokio::test]
    async fn a_timed_out_delegation_ends_its_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("descendant-marker");
        let binary = stub(
            dir.path(),
            "backgrounding",
            &format!(
                "(sleep 3; printf harmless > '{}') & printf 'partway through'; wait",
                marker.display()
            ),
        );
        let delegation = Delegator::new(executor(&binary))
            .in_directory(dir.path())
            .run(Task::reading("anything", "a.rs").bounded(Bounds::within(Duration::from_secs(1))))
            .await;

        assert_eq!(delegation.status, Status::TimedOut);
        // What the executor printed before the bound expired is the only
        // account of what it was doing, so the record keeps it.
        assert_eq!(delegation.output, "partway through");
        tokio::time::sleep(Duration::from_secs(4)).await;
        assert!(
            !marker.exists(),
            "a delegate's background child outlived the bound the delegation ran under"
        );
    }

    /// The cap holds what the host keeps, and the count says what the
    /// executor wrote.
    #[tokio::test]
    async fn a_noisy_delegate_is_bounded_while_it_prints() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(
            dir.path(),
            "noisy",
            "yes 0123456789abcde | head -n 8192\nyes 0123456789abcde | head -n 8192 >&2",
        );
        let delegation = Delegator::new(executor(&binary))
            .in_directory(dir.path())
            .run(Task::reading("anything", "a.rs").bounded(Bounds::within(BOUND)))
            .await;

        assert_eq!(delegation.status, Status::Answered);
        assert_eq!(delegation.bytes, 2 * 8192 * 16);
        for kept in [&delegation.output, &delegation.detail] {
            assert!(
                kept.len() <= OUTPUT_MAX + 64,
                "a stream ran past its cap: {} bytes",
                kept.len()
            );
            assert!(kept.ends_with("131072 bytes in all"));
        }
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
        let delegation = Delegator::new(executor(&binary))
            .run(Task::reading("how many", "a.rs").expecting("5"))
            .await;
        assert_eq!(delegation.correct(), Some(false));
    }

    /// Six delegations run at once, and six at a width of one do not.
    #[tokio::test]
    async fn the_fan_out_is_concurrent_under_its_bound() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "devin", "sleep 0.4\nprintf 'done\\n'");
        let executor = executor(&binary);

        let started = Instant::now();
        let wide = Delegator::new(executor.clone())
            .bounded_to(6)
            .fan_out(six_tasks())
            .await;
        let parallel = started.elapsed();
        let summed: Duration = wide.iter().map(|delegation| delegation.elapsed).sum();
        assert_eq!(wide.len(), 6);
        assert!(wide.iter().all(Delegation::answered));
        // Wall clock against summed delegation time, which is the
        // measurement the recorded episode reports and the one a busy
        // machine does not move: contention inflates both together, while
        // a fixed ceiling on the wall clock alone turns a loaded test
        // machine into a failure about concurrency.
        assert!(
            parallel * 2 < summed,
            "a width of six is not concurrent: {parallel:?} of wall clock against {summed:?} summed"
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
        let delegations = Delegator::new(executor(&binary))
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

    /// A delegate that would write into the shared directory, or that asks
    /// a host for a checkout it cannot make, is refused before anything
    /// spawns rather than run in the shared directory.
    #[tokio::test]
    async fn an_unisolated_write_is_refused_before_it_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "stub", "printf 'wrote it\\n'");
        let delegator = Delegator::new(executor(&binary));

        let mut writing = Task::reading("change a file", "a.rs");
        writing.writes = true;
        let refused = delegator.run(writing).await;
        assert_eq!(refused.status, Status::Refused("isolation_required".into()));
        assert!(refused.output.is_empty(), "nothing ran");

        let mut isolated = Task::reading("change a file", "a.rs");
        isolated.isolation = Isolation::Worktree;
        assert!(!delegator.provides(Isolation::Worktree));
        let unavailable = delegator.run(isolated).await;
        assert_eq!(
            unavailable.status,
            Status::Refused("isolation_unavailable".into()),
            "a host with no checkout to branch from refuses rather than sharing one"
        );
    }

    /// A delegation that asked for a checkout of its own gets one, runs in
    /// it, and leaves none behind.
    #[tokio::test]
    async fn a_worktree_delegation_runs_in_its_own_checkout() {
        let Some(repository) = scratch_repository() else {
            return;
        };
        let binary = stub(repository.path(), "stub", "pwd");
        let delegator = Delegator::new(executor(&binary)).in_repository(repository.path());
        assert!(delegator.provides(Isolation::Worktree));

        let mut task = Task::reading("where am I", "a.rs");
        task.isolation = Isolation::Worktree;
        let delegation = delegator.run(task).await;

        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert_ne!(
            delegation.workdir,
            repository.path(),
            "the delegation ran somewhere of its own"
        );
        assert!(
            delegation
                .workdir
                .starts_with(repository.path().canonicalize().unwrap().join(WORKTREE_DIR)),
            "the checkout sits under the repository, which is the directory an \
             executor has been told to trust: {}",
            delegation.workdir.display()
        );
        assert!(
            !delegation.workdir.exists(),
            "the checkout is removed when the delegation ends"
        );
    }

    /// Six delegations that each asked for a checkout of their own get six
    /// different ones, at once.
    #[tokio::test]
    async fn six_worktrees_do_not_collide() {
        let Some(repository) = scratch_repository() else {
            return;
        };
        let binary = stub(repository.path(), "stub", "pwd");
        let tasks: Vec<Task> = six_tasks()
            .into_iter()
            .map(|mut task| {
                task.isolation = Isolation::Worktree;
                task
            })
            .collect();

        let delegations = Delegator::new(executor(&binary))
            .in_repository(repository.path())
            .bounded_to(6)
            .fan_out(tasks)
            .await;

        assert!(
            delegations.iter().all(Delegation::answered),
            "{:?}",
            delegations.iter().map(Delegation::line).collect::<Vec<_>>()
        );
        let mut checkouts: Vec<&Path> = delegations
            .iter()
            .map(|delegation| delegation.workdir.as_path())
            .collect();
        checkouts.sort_unstable();
        checkouts.dedup();
        assert_eq!(checkouts.len(), 6, "six delegations, six checkouts");
        assert!(
            !repository
                .path()
                .join(WORKTREE_DIR)
                .join("..")
                .join(WORKTREE_DIR)
                .exists()
                || std::fs::read_dir(repository.path().join(WORKTREE_DIR))
                    .map(|entries| entries.count())
                    .unwrap_or(0)
                    == 0,
            "no checkout outlives its delegation"
        );
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
