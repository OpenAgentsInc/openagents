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
//! worktree per delegation; one that was not refuses a task asking for a
//! worktree, and refuses a task that says it writes without one. Both
//! refusals land before anything spawns, so nothing records
//! `isolation: worktree` and runs in the shared directory.
//!
//! A read-only delegation's checkout is removed when it ends. A writing
//! delegation's stays where the executor left it — its edits are owed to
//! a reviewer, not silently merged or discarded — and the record names
//! the retained path.
//!
//! # The filesystem boundary is enforced, not declared
//!
//! `Task::writes` is a statement; `coder_boundary` is the wall. Every
//! delegated command runs under an enforced write profile: a read-only
//! task may write only its private scratch and the adapter state the
//! executor's [`Policy`] grants, and a writing task adds the checkout it
//! owns while the main checkout stays protected and the common Git
//! directory — like the manifest, the adapter, and the approval store —
//! stays sealed against every exception, so a delegate cannot rewrite the
//! approval it runs under. A platform with no enforced backend, a grant
//! that overlaps a protected path, or a sealed path a checkout sits
//! beneath is a refusal before spawn — there is no unrestricted fallback.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_boundary::Boundary;
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
    /// The arguments a writing task runs with instead, when the manifest
    /// declares `invoke_writing`. An executor that stops to confirm each
    /// edit answers a fan-out with nothing done, so the manifest names the
    /// argv that lets it work unattended and the boundary is what holds
    /// it. `None` runs `arguments` for every task.
    pub writing_arguments: Option<Vec<String>>,
    /// What this executor declares it refuses, beyond being absent.
    pub refuses: Vec<Refusal>,
    /// The host's filesystem policy for this executor's delegations.
    policy: Policy,
}

impl Executor {
    /// An executor over a resolved binary and the arguments that go before
    /// the prompt.
    ///
    /// The policy starts empty: no adapter state is granted, because an
    /// executor a caller built by hand carries no approval's word.
    /// [`Executor::under`] sets it.
    #[must_use]
    pub fn new(capability: &str, binary: impl Into<PathBuf>, arguments: Vec<String>) -> Self {
        Executor {
            capability: capability.to_string(),
            binary: binary.into(),
            arguments,
            writing_arguments: None,
            refuses: Vec::new(),
            policy: Policy::empty(),
        }
    }

    /// Sets the arguments a writing task runs with, from the manifest's
    /// `invoke_writing`.
    #[must_use]
    pub fn writing_with(mut self, arguments: Vec<String>) -> Self {
        self.writing_arguments = Some(arguments);
        self
    }

    /// The arguments one task runs with: the writing argv for a task that
    /// writes and has one, `arguments` otherwise.
    #[must_use]
    pub fn arguments_for(&self, task: &Task) -> &[String] {
        match (&self.writing_arguments, task.writes) {
            (Some(writing), true) => writing,
            _ => &self.arguments,
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

    /// The filesystem policy this executor's delegations run under.
    ///
    /// [`crate::survey::executor`] sets it from the proof the probe ran
    /// under — the adapter state the approval granted, and the approval's
    /// own material sealed. A caller that built the executor directly
    /// states its own host's word with [`Policy::granting`] rather than
    /// constructing a record no operator made.
    #[must_use]
    pub fn under(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    /// The policy this executor's delegations run under.
    #[must_use]
    pub fn policy(&self) -> &Policy {
        &self.policy
    }
}

/// The host's word about what an executor's delegations may write, and
/// what they may never touch.
///
/// Grants are the approval's, not the manifest's: an operator adds
/// adapter state with `capability-trust approve <slug> --writable <dir>`,
/// and the approval pins the manifest, the adapter, and every argv file
/// it names. [`Policy::of`] carries the approval's *whereabouts* — which
/// manifest file, which store — so [`Policy::resolve`] can decide again
/// at dispatch: a survey's proof is a cache, and a manifest or adapter
/// changed since approval, a retargeted script, or a revoked record
/// approves nothing at launch. A manifest's own `enforces` list never
/// reaches here: it is a claim about bounds, and a claim is not a
/// boundary.
///
/// A caller-built executor — a test's stub, a host that drives one
/// directly — carries [`Policy::empty`] and states grants itself with
/// [`Policy::granting`], which is the host's explicit word rather than a
/// record no operator wrote.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    /// Adapter state the host grants a delegate to write — the caller's
    /// own word, or the record's, read at dispatch.
    writable: Vec<PathBuf>,
    /// Paths sealed against every writable exception, a writing
    /// boundary's checkout included: the manifest that drives the
    /// executor, the resolved adapter, the argv files the approval pins,
    /// and the directory the approval store lives in, plus whatever else
    /// the host names. A delegate that could rewrite its own approval
    /// material would approve itself next.
    sealed: Vec<PathBuf>,
    /// The approval a dispatch must re-decide: where the manifest sits,
    /// which kind of directory it came from, and which store the record
    /// lives in. `None` for proofs that carry no store and for
    /// caller-built policies, whose grants are the caller's own word and
    /// need no operator's record behind them.
    approval: Option<Approved>,
    /// The lexical paths a resolved record pins — the argv words as they
    /// are spelled, resolved in the directory the delegation runs in.
    /// Sealing the canonical file a word names does not protect the word
    /// itself: a grant over a directory the word sits in lets the
    /// delegate retarget it after verification. [`Delegator::boundary`]
    /// refuses a grant that overlaps a pinned word's ancestry.
    aliases: Vec<PathBuf>,
}

/// Where an approval lives and what it approved, carried from the survey
/// to the dispatch so the decision can be made again against the store as
/// it is now — and so a manifest changed since the survey, reapproved or
/// not, refuses the executor the survey built.
#[derive(Clone, Debug)]
struct Approved {
    /// The manifest file the probe read.
    manifest: PathBuf,
    /// Which kind of directory the manifest came from.
    source: crate::capability::Source,
    /// The store the approving record lives in.
    store: PathBuf,
    /// The digest of the manifest file the survey read. A dispatch that
    /// re-reads a different digest — the manifest was rewritten and
    /// reapproved — refuses this executor, whose argv the old file built;
    /// the new approval drives a fresh survey's executor.
    digest: String,
    /// The canonical adapter the survey resolved — what the executor
    /// runs. The record the dispatch verifies must pin the same one.
    adapter: PathBuf,
    /// The `invoke` argv the surveyed manifest declared — the digest
    /// check implies it, and the comparison keeps the binding explicit.
    invoke: Vec<String>,
    /// The `invoke_writing` argv the surveyed manifest declared, bound
    /// the same way.
    invoke_writing: Vec<String>,
}

impl Policy {
    /// No grants and nothing extra sealed: what a caller-built executor
    /// carries. A delegate under it can write only its private scratch —
    /// and its checkout, for a writing task — because nothing else was
    /// ever granted.
    #[must_use]
    pub fn empty() -> Self {
        Policy::default()
    }

    /// The policy a probed capability implies.
    ///
    /// `found` is the probe's answer — its manifest file, provenance, and
    /// the proof it ran under — and `adapter` is the binary it resolved.
    /// A `Proof::Approved` leaves the decision where it belongs: the
    /// manifest and the store are carried so [`Policy::resolve`] can ask
    /// again at dispatch, and the granted adapter state is read from the
    /// record then rather than cached here. Any other proof grants
    /// nothing, because approval is permission to probe and a manifest
    /// cannot grant itself a write. Either way the manifest, the adapter,
    /// and the approval store's directory are sealed now: the material a
    /// delegate could otherwise use to widen its own grant.
    #[must_use]
    pub fn of(found: &crate::capability::Found, adapter: &Path) -> Self {
        let mut policy = Policy::empty().sealing(adapter);
        policy.aliases.push(adapter.to_path_buf());
        if !found.path.as_os_str().is_empty() {
            policy = policy.sealing(&found.path);
            policy.aliases.push(found.path.clone());
        }
        // The store is sealed by sealing the directory that holds it:
        // sealing the file alone would leave the directory writable, and
        // a delegate could swap the file in it. Only a directory that
        // exists can be sealed — a missing one holds no store to protect.
        let store = match &found.proof {
            crate::capability::Proof::Approved { store, .. } => store.clone(),
            _ => crate::capability::store_path(),
        };
        if let Some(parent) = store
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty() && parent.exists())
        {
            policy = policy.sealing(parent);
        }
        if let crate::capability::Proof::Approved { .. } = &found.proof {
            policy.aliases.push(store.clone());
            policy.approval = Some(Approved {
                manifest: found.path.clone(),
                source: found.source,
                store,
                digest: found.digest.clone(),
                // The record pins the adapter canonically; the probe's
                // path is absolute but not canonicalized, so bind to the
                // resolved form — an adapter that cannot be resolved
                // simply never matches.
                adapter: adapter
                    .canonicalize()
                    .unwrap_or_else(|_| adapter.to_path_buf()),
                invoke: found.manifest.invoke.clone(),
                invoke_writing: found.manifest.invoke_writing.clone(),
            });
        }
        policy
    }

    /// The policy as it stands at dispatch, decided again rather than
    /// taken from the survey's cache.
    ///
    /// `workdir` is the directory the argv will actually run in, because
    /// that is where a relative argv word resolves. `Entry::load`
    /// re-reads the manifest — and it must be *the* manifest the survey
    /// read: a file rewritten since, reapproved or not, names a different
    /// executor than the one this policy was built for, so the cached
    /// executor refuses and a fresh survey builds the one the new
    /// approval names. `Trust::decide_verified` re-reads the store —
    /// a deleted or revoked record approves nothing — re-verifies the
    /// adapter's path and bytes and every pinned argv word in `workdir`,
    /// and hands back the very record it checked: the grants, seals, and
    /// lexical aliases this returns are the ones that decision saw, not
    /// a later load's.
    ///
    /// A policy with no approval behind it — a caller's own word, or a
    /// proof that carries no store — answers itself unchanged.
    fn resolve(&self, workdir: &Path) -> Result<Policy, String> {
        let Some(approval) = &self.approval else {
            return Ok(self.clone());
        };
        let entry = crate::capability::Entry::load(&approval.manifest, approval.source)
            .map_err(|why| format!("the approved manifest could not be re-read: {why}"))?;
        // The executor is bound to what the survey saw: the manifest's
        // bytes, its invoke argv, and the adapter the probe resolved. A
        // changed manifest is a different executor — even when the
        // operator has approved the change, this executor's argv came
        // from the old file, and the new approval belongs to a fresh
        // survey. The invoke comparison is the digest check's explicit
        // half: equal bytes imply it, and it is stated so the binding
        // does not rest on that implication.
        if entry.digest != approval.digest
            || entry.manifest.invoke != approval.invoke
            || entry.manifest.invoke_writing != approval.invoke_writing
        {
            return Err(format!(
                "the manifest at {} changed since this executor was surveyed — \
                 survey again so the executor the approval names is the one that runs",
                approval.manifest.display()
            ));
        }
        let trust = crate::capability::Trust::load(&approval.store).map_err(|why| {
            format!(
                "the trust store at {} could not be read: {why}",
                approval.store.display()
            )
        })?;
        let record = match trust.decide_verified(&entry, workdir) {
            crate::capability::Verified::Approved(record) => record,
            // An unconditional trust holds no record — there is nothing
            // to bind grants or seals to beyond the policy's own.
            crate::capability::Verified::Unconditional => return Ok(self.clone()),
            crate::capability::Verified::Unapproved(why) => return Err(why),
        };
        // The verified record must pin the adapter this executor runs —
        // an approval for the same manifest resolved somewhere else does
        // not cover the binary the survey chose.
        if record.adapter != approval.adapter {
            return Err(format!(
                "the approval's adapter {} is not the surveyed {} — survey again",
                record.adapter.display(),
                approval.adapter.display()
            ));
        }
        let mut resolved = self.clone();
        resolved.writable.extend(record.writable.iter().cloned());
        resolved.sealed.push(record.adapter.clone());
        for pinned in &record.pinned {
            resolved.sealed.push(pinned.path.clone());
            // The word the argv runs, resolved lexically where it will
            // run: the path a writable grant could let the delegate
            // retarget after this verification. `boundary` refuses a
            // grant that covers its ancestry.
            let word = Path::new(&pinned.word);
            resolved.aliases.push(match word.is_absolute() {
                true => word.to_path_buf(),
                false => workdir.join(word),
            });
        }
        Ok(resolved)
    }

    /// Grants one adapter-state path: the host's own word, for a caller
    /// that built the executor itself rather than carrying an approval.
    ///
    /// A grant that overlaps a protected or sealed path refuses the whole
    /// delegation when the boundary is built rather than bounding less
    /// than it claims.
    #[must_use]
    pub fn granting(mut self, path: impl Into<PathBuf>) -> Self {
        self.writable.push(path.into());
        self
    }

    /// Seals one path against every writable exception — a writing
    /// boundary's checkout included.
    #[must_use]
    pub fn sealing(mut self, path: impl Into<PathBuf>) -> Self {
        self.sealed.push(path.into());
        self
    }

    /// The adapter-state paths the host granted.
    #[must_use]
    pub fn writable(&self) -> &[PathBuf] {
        &self.writable
    }

    /// The paths sealed against every exception.
    #[must_use]
    pub fn sealed(&self) -> &[PathBuf] {
        &self.sealed
    }

    /// The lexical invocation paths a resolved record pinned — the argv
    /// words as spelled, whose ancestry must stay unwritable or the
    /// delegate could retarget a word the verification already checked.
    #[must_use]
    pub fn aliases(&self) -> &[PathBuf] {
        &self.aliases
    }
}

/// What a delegation may spend.
///
/// The wall bound is what the host enforces; `declared` is what the trace
/// records, so a bound stated in minutes reads back as minutes rather than
/// as however many seconds that came to.
///
/// A bound that arithmetic cannot express is not a shorter bound: zero is
/// no time at all, and an overflowed minute count is not whatever the wrap
/// left behind. Both are remembered here as untenable, and the delegation
/// answers with the refusal code rather than running unbounded.
#[derive(Clone, Debug)]
pub struct Bounds {
    wall: Duration,
    declared: Value,
    /// Why the bound cannot be held, when it cannot — the refusal code a
    /// delegation records instead of spawning.
    untenable: Option<&'static str>,
}

impl Bounds {
    /// A bound in whole minutes, which is the form a manifest's `enforces`
    /// list names and the form the golden records.
    #[must_use]
    pub fn minutes(minutes: u64) -> Self {
        Bounds::checked(
            minutes.checked_mul(60).map(Duration::from_secs),
            json!({ "minutes": minutes }),
        )
    }

    /// A bound finer than a minute, for a caller that has one.
    #[must_use]
    pub fn within(wall: Duration) -> Self {
        Bounds::checked(Some(wall), json!({ "seconds": wall.as_secs_f64() }))
    }

    /// Assembles a bound, marking the ones nothing can hold.
    ///
    /// Three ways a stated bound is not a bound: zero is no time at all,
    /// a minute count that overflows is not whatever the wrap left
    /// behind, and a duration no `Instant` can reach is a deadline that
    /// would panic rather than fire. All three are remembered here, and
    /// the delegation answers with the refusal code instead of running
    /// unbounded.
    fn checked(wall: Option<Duration>, declared: Value) -> Self {
        let untenable = match wall {
            None => Some("bound_overflow"),
            Some(wall) if wall.is_zero() => Some("bound_zero"),
            Some(wall) if Instant::now().checked_add(wall).is_none() => Some("bound_overflow"),
            Some(_) => None,
        };
        Bounds {
            wall: wall.unwrap_or_default(),
            declared,
            untenable,
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

    /// Why this bound cannot be held, when it cannot — the refusal code a
    /// delegation answers with instead of running unbounded.
    #[must_use]
    pub fn untenable(&self) -> Option<&'static str> {
        self.untenable
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
    /// works in makes one — kept for the reviewer when the task writes,
    /// removed when it does not — and one that does not refuses the task
    /// rather than running it in the shared directory.
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

/// The filesystem boundary one delegation ran under, as the record needs
/// it: the resolved paths, not the profile text.
///
/// `None` on a [`Delegation`] means nothing ran — a refusal before spawn
/// has no boundary to report, and no command ever runs without one.
#[derive(Clone, Debug)]
pub struct EnforcedBoundary {
    /// The backend the command ran under — `sandbox-exec` on macOS,
    /// `bwrap` on Linux.
    pub backend: PathBuf,
    /// The isolated checkout a writing delegation could write, when it
    /// had one — the profile's one exception to the protected set.
    pub checkout: Option<PathBuf>,
    /// Every writable path the profile permitted, resolved: the private
    /// scratch and the adapter state the policy granted.
    pub writable: Vec<PathBuf>,
    /// The paths the profile denied, with the checkout as the only
    /// permitted exception — the main checkout, first of all.
    pub protected: Vec<PathBuf>,
    /// The paths the profile denied with no exception at all: the common
    /// Git directory, the manifest, the adapter, and the approval store's
    /// directory — the material a delegate could otherwise use to widen
    /// its own grant.
    pub sealed: Vec<PathBuf>,
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
    /// The filesystem boundary the executor ran under — what enforced the
    /// task's `writes` statement. `None` only on a refusal before spawn.
    pub boundary: Option<EnforcedBoundary>,
    /// The checkout kept for review, when a delegation that may write ran
    /// at all. A writing task's worktree stays where the executor left it
    /// whatever the status — partial work is still owed to a reviewer —
    /// and this names it. `None` when nothing is owed: a read-only task,
    /// or a refusal before anything ran.
    pub retained: Option<PathBuf>,
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
    /// Where each delegation's private scratch directory is made — the one
    /// writable path a read-only task's boundary permits. The system
    /// temporary directory, unless a caller says otherwise.
    scratch_under: PathBuf,
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
            scratch_under: std::env::temp_dir(),
        }
    }

    /// Runs the delegations somewhere other than this process's working
    /// directory.
    #[must_use]
    pub fn in_directory(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.workdir = workdir.into();
        self
    }

    /// Makes each delegation's private scratch directory under `parent`
    /// rather than the system temporary directory. The scratch is the
    /// boundary's own — created with it, writable to the delegate, removed
    /// when the held boundary drops after the child is reaped.
    #[must_use]
    pub fn scratch_under(mut self, parent: impl Into<PathBuf>) -> Self {
        self.scratch_under = parent.into();
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

    /// Bounds how many delegations run at once.
    ///
    /// Zero is not a width and is not read as one: a delegator bounded to
    /// zero refuses every task with `concurrency_zero`, and the record
    /// says `0`, rather than silently running at a width nobody asked for.
    #[must_use]
    pub fn bounded_to(mut self, concurrent_max: usize) -> Self {
        self.concurrent_max = concurrent_max;
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

    /// Why this host will not run this task at all, when it will not —
    /// the code the delegation records as refused.
    ///
    /// Three answers, all before anything spawns: the bound as stated is
    /// one nothing can hold, the width is zero, or the checkout shape the
    /// task asks for is one this host has no way to make.
    fn unrunnable(&self, task: &Task) -> Option<&'static str> {
        task.bounds
            .untenable()
            // The construction-time check asks `now`; ask again at
            // dispatch, because a bound built long ago can outlive the
            // clock it was checked against.
            .or_else(|| {
                Instant::now()
                    .checked_add(task.bounds.wall())
                    .is_none()
                    .then_some("bound_overflow")
            })
            .or_else(|| (self.concurrent_max == 0).then_some("concurrency_zero"))
            .or_else(|| self.unisolated(task))
    }

    /// Hands one task to the executor and waits for it.
    ///
    /// Every run is bounded twice before anything spawns: `supervise`
    /// holds the wall clock, the output caps, and the process group, and
    /// a `coder_boundary` profile holds the filesystem — a read-only task
    /// writes only its private scratch and the adapter state the policy
    /// grants; a writing task adds the checkout it owns and nothing else.
    /// A host that cannot build the boundary refuses rather than running
    /// the command without it.
    ///
    /// A task asking for a checkout of its own gets one, made before the
    /// executor spawns. A read-only delegation's checkout is removed when
    /// it ends; a writing delegation's stays where the executor left it,
    /// whatever it ended as — its edits are owed to a reviewer, and
    /// [`Delegation::retained`] names the path.
    pub async fn run(&self, task: Task) -> Delegation {
        if let Some(code) = self.unrunnable(&task) {
            return self.ended(
                task,
                Reported::of(Status::Refused(code.to_string())),
                Duration::ZERO,
                self.workdir.clone(),
                None,
                None,
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
                    None,
                    None,
                );
            }
        };
        let workdir = checkout.path(&self.workdir).to_path_buf();

        // A binary that is not there is the harness, not a refusal and
        // not the executor: the question was never asked. Checked here,
        // before the boundary is built, because the wrapper exists — it
        // is `sandbox-exec`, not the target — and would happily spawn.
        if !self.executor.binary.is_file() {
            let why = format!("no executor at {}", self.executor.binary.display());
            let mut reported = Reported::of(Status::Harness(why.clone())).detailing(why);
            if let Err(error) = checkout.close().await {
                reported.detail = format!("{}\n{error}", reported.detail);
            }
            return self.ended(task, reported, started.elapsed(), workdir, None, None);
        }

        // The survey's approval is a cache. The decision is made again
        // against the store as it is now, in the directory the argv will
        // run in — a manifest or adapter changed since approval, a
        // retargeted script, or a revoked record answers `unapproved`,
        // and nothing runs on a stale word.
        let policy = match self.executor.policy().resolve(&workdir) {
            Ok(policy) => policy,
            Err(why) => {
                let mut reported =
                    Reported::of(Status::Refused("unapproved".to_string())).detailing(why);
                if let Err(error) = checkout.close().await {
                    reported.detail = format!("{}\n{error}", reported.detail);
                }
                return self.ended(task, reported, started.elapsed(), workdir, None, None);
            }
        };

        // The boundary is built before the command is wrapped in it. A
        // spec the profile cannot express — a grant overlapping a
        // protected path, a sealed path the checkout sits beneath, a
        // platform with no backend — is a refusal here, with the checkout
        // handed back, never a quieter bound.
        let (boundary, command) = match self.prepare(&task, &checkout, &workdir, &policy).await {
            Ok(prepared) => prepared,
            Err(why) => {
                let mut reported =
                    Reported::of(Status::Refused("boundary_unavailable".to_string()))
                        .detailing(why);
                if let Err(error) = checkout.close().await {
                    reported.detail = format!("{}\n{error}", reported.detail);
                }
                return self.ended(task, reported, started.elapsed(), workdir, None, None);
            }
        };

        // A delegation that may write keeps its checkout, and that is
        // true from before the executor spawns: whatever happens next —
        // answered, failed, timed out, or a caller that walks away — the
        // worktree stays where the executor left it, registered in
        // `git worktree list` even when no Delegation comes back to name
        // it. A read-only checkout still goes back when the run ends.
        let retained = match task.writes {
            true => checkout.retain(),
            false => None,
        };
        let enforced = EnforcedBoundary {
            backend: boundary.backend().to_path_buf(),
            checkout: boundary.checkout().map(Path::to_path_buf),
            writable: boundary.writable().to_vec(),
            protected: boundary.protected().to_vec(),
            sealed: boundary.sealed().to_vec(),
        };

        // A bound the host cannot enforce is a timer. [`supervise`] makes
        // this one a bound: the executor runs in a process group of its
        // own, an expired bound or a cancelled caller terminates that
        // group rather than abandoning the wait, and the direct child is
        // reaped before this returns. Both holds live until the child is
        // reaped and its output drained — a scratch removed while the
        // group still runs is the same lie as a checkout removed while
        // the executor is still writing to it.
        let ended = Job::from_command(command)
            .bounded(Limits::within(task.bounds.wall()).keeping(OUTPUT_MAX))
            .run_holding((boundary.hold(), checkout.hold()))
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
        // A delegation that may have written keeps its checkout:
        // `retained` named it before the spawn, and what the executor
        // changed is a reviewer's to read, not this call's to discard.
        // A read-only delegation's checkout goes back — the boundary
        // says it holds nothing new.
        let reported = match task.writes {
            true => reported,
            false => match checkout.close().await {
                Ok(()) => reported,
                Err(error) => Reported {
                    status: Status::Harness(error.clone()),
                    detail: format!("{}\n{error}", reported.detail),
                    ..reported
                },
            },
        };
        self.ended(
            task,
            reported,
            started.elapsed(),
            workdir,
            Some(enforced),
            retained,
        )
    }

    /// The filesystem boundary this delegation runs under.
    ///
    /// A read-only task's profile denies every write but the private
    /// scratch and the adapter state the resolved policy grants. A
    /// writing task's adds the worktree it owns — the one exception to
    /// the main checkout's deny — while the common Git directory and
    /// every path the policy seals stay denied with no exception at all.
    ///
    /// Every path is resolved at build, and a spec that cannot be
    /// enforced — a grant overlapping a denied path, a checkout beneath
    /// a sealed one, a platform with no backend, a repository whose
    /// common Git directory cannot be found to seal — is an error here
    /// and a refusal in the record. There is no construction of the
    /// command without it.
    async fn boundary(
        &self,
        task: &Task,
        checkout: &Checkout,
        policy: &Policy,
    ) -> Result<Boundary, String> {
        let mut spec = match (task.writes, checkout) {
            (false, _) => Boundary::readonly(),
            (true, Checkout::Own(worktree)) => Boundary::writing(worktree.path()),
            // `unrunnable` refuses this pairing before the checkout is made.
            (true, Checkout::Shared) => {
                return Err("a writing task has no checkout of its own".to_string());
            }
        };
        // The main checkout is protected: a writing boundary's worktree
        // sits beneath it as the profile's one exception. The common Git
        // directory is sealed — even a writing delegation's checkout
        // grants it no exception — and so is every path the resolved
        // policy names: the manifest that drives the executor, the
        // adapter it runs, the argv files its approval pins, and the
        // directory that holds the store that approved it. The adapter
        // itself is sealed whatever the policy says — a delegate that
        // could rewrite the program it runs under approves itself next.
        let main = self.repository.as_deref().unwrap_or(&self.workdir);
        spec = spec.protecting(main);
        spec = spec.sealed(&self.executor.binary);
        for path in policy.sealed() {
            spec = spec.sealed(path);
        }
        match self.repository {
            // A repository was named: its common Git directory must be
            // sealed, or the boundary is not what it claims — refuse
            // rather than omit the seal.
            Some(_) => {
                let common = crate::worktree::common_directory(main)
                    .await
                    .map_err(|why| format!("cannot seal the common Git directory: {why}"))?;
                spec = spec.sealed(common);
            }
            // A standalone directory may still sit inside a checkout;
            // seal the common Git directory when there is one to find,
            // and a directory outside version control has none.
            None => {
                if let Ok(common) = crate::worktree::common_directory(main).await {
                    spec = spec.sealed(common);
                }
            }
        }
        for path in policy.writable() {
            spec = spec.writable(path);
        }
        // A pinned argv word's lexical path is not protected by sealing
        // the canonical file it resolved to: a grant over any directory
        // the word sits in — or over a symlink ancestor's parent — lets
        // the delegate retarget the word after verification, and the pin
        // then names bytes nothing runs. A grant that covers a pinned
        // word's ancestry is a spec this boundary cannot honor: refuse
        // it rather than narrow it. The writing boundary's checkout is a
        // writable root the same way, so it answers the same question.
        let mut grants: Vec<PathBuf> = policy
            .writable()
            .iter()
            .filter_map(|grant| grant.canonicalize().ok())
            .collect();
        // The writing checkout is a writable root the same way — a pinned
        // word beneath it could be swapped by the delegate too — and it is
        // canonicalized like the grants, because a lexical `/var` never
        // starts with a canonical `/private/var`.
        if let Checkout::Own(worktree) = checkout {
            grants.push(worktree.path().canonicalize().map_err(|error| {
                format!(
                    "cannot resolve the delegation's checkout {}: {error}",
                    worktree.path().display()
                )
            })?);
        }
        for alias in policy.aliases() {
            let mut ancestor = alias.parent();
            while let Some(dir) = ancestor {
                let canonical = dir.canonicalize().map_err(|error| {
                    format!(
                        "cannot check the pinned path's ancestry at {}: {error}",
                        dir.display()
                    )
                })?;
                if let Some(grant) = grants.iter().find(|grant| canonical.starts_with(grant)) {
                    return Err(format!(
                        "the protected invocation or approval path {} sits under the writable {} — \
                         the delegate could retarget it after verification",
                        alias.display(),
                        grant.display()
                    ));
                }
                ancestor = dir.parent();
            }
        }
        spec.owned_scratch_under(&self.scratch_under)
            .build()
            .map_err(|error| error.to_string())
    }

    /// The boundary and the command wrapped in it, ready to supervise.
    ///
    /// The command is the backend, its arguments, then `<binary> <argv>`
    /// — `sandbox-exec -f <profile>` on macOS, `bwrap <binds> --` on
    /// Linux — run in the delegation's directory — the declared argv and the caller's
    /// environment, preserved. The boundary's owned scratch is exported
    /// as `TMPDIR` only: where an adapter keeps its state is the
    /// approval's word, granted as writable adapter state by the policy,
    /// never a repointed `XDG` variable or a writable `HOME`.
    async fn prepare(
        &self,
        task: &Task,
        checkout: &Checkout,
        workdir: &Path,
        policy: &Policy,
    ) -> Result<(Boundary, std::process::Command), String> {
        let boundary = self.boundary(task, checkout, policy).await?;
        let argv: Vec<&OsStr> = self
            .executor
            .arguments_for(task)
            .iter()
            .map(OsStr::new)
            .chain(std::iter::once(OsStr::new(&task.prompt)))
            .collect();
        let mut command = boundary
            .command(&self.executor.binary, argv)
            .map_err(|error| error.to_string())?;
        command.current_dir(workdir);
        if let Some(scratch) = boundary.scratch() {
            command.env("TMPDIR", scratch);
        }
        Ok((boundary, command))
    }

    /// The checkout one task runs in.
    async fn checkout(&self, task: &Task) -> Result<Checkout, String> {
        match (task.isolation, &self.repository) {
            (Isolation::Worktree, Some(repository)) => Worktree::add(repository)
                .await
                .map(|worktree| Checkout::Own(Arc::new(worktree))),
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
    ///
    /// A width of zero is no width: every task is refused `concurrency_zero`
    /// rather than run at a width nobody asked for.
    pub async fn fan_out(&self, tasks: Vec<Task>) -> Vec<Delegation> {
        // The poll width is a buffer size, which cannot be zero; the
        // refusal lives in `unrunnable`, and refused tasks need no
        // parallelism.
        stream::iter(tasks.into_iter().map(|task| self.run(task)))
            .buffered(self.concurrent_max.max(1))
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
        boundary: Option<EnforcedBoundary>,
        retained: Option<PathBuf>,
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
            boundary,
            retained,
        }
    }
}

/// Whether this host can put a delegated command inside an enforced
/// filesystem boundary.
///
/// `false` where the platform has no backend — which a delegation answers
/// with `boundary_unavailable` rather than with a command that runs
/// unbounded. Callers deciding what a host may offer read this before
/// promising isolation.
#[must_use]
pub fn boundary_supported() -> bool {
    Boundary::readonly().build().is_ok()
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
    /// A checkout of this delegation's own. A read-only task's is removed
    /// when it ends; a writing task's is retained for the reviewer —
    /// [`Checkout::retain`] marks it before the executor spawns.
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
            Checkout::Own(worktree) => {
                Arc::try_unwrap(worktree)
                    .map_err(|_| "the executor still holds its worktree".to_string())?
                    .close()
                    .await
            }
        }
    }

    /// Marks the checkout retained: a writing delegation's edits are owed
    /// to a reviewer, and that is true from before the executor spawns —
    /// a caller that walks away mid-run leaves the worktree on disk and
    /// in `git worktree list`, where a reviewer finds it even when no
    /// Delegation came back to name it. `None` for the shared directory,
    /// which is nobody's to keep.
    fn retain(&self) -> Option<PathBuf> {
        match self {
            Checkout::Shared => None,
            Checkout::Own(worktree) => Some(worktree.retain()),
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

    /// A real approval and the executor a survey would build from it: a
    /// manifest under `<repo>/capabilities`, a record in a store outside
    /// the checkout, and a `Found` carrying `Proof::Approved` — so the
    /// delegation re-decides a real record at dispatch rather than a
    /// staged one. Returns the executor, the store, and the manifest.
    ///
    /// `binary` is the manifest's `detect.binary` — an absolute path for
    /// a stub adapter, or `sh` when the adapter is an interpreter and an
    /// argv word names the script it runs. `invoke` is the manifest's
    /// `invoke` argv; the prompt is appended at dispatch as always.
    fn approved(
        repo: &Path,
        store_dir: &Path,
        binary: &str,
        invoke: &[String],
        writable: &[PathBuf],
    ) -> (Executor, PathBuf, PathBuf) {
        let capabilities = repo.join("capabilities");
        std::fs::create_dir_all(&capabilities).unwrap();
        let manifest = capabilities.join("approved-local.json");
        std::fs::write(
            &manifest,
            json!({
                "v": 1,
                "slug": "approved-local",
                "transport": "subprocess",
                "detect": {"binary": binary, "version": [binary, "--version"]},
                "invoke": invoke,
            })
            .to_string(),
        )
        .unwrap();
        let store = store_dir.join("capability-trust.json");
        let mut trust = crate::capability::Trust::load(&store).unwrap();
        trust
            .approve(Some(repo), "approved-local", writable)
            .unwrap();
        let entry =
            crate::capability::Entry::load(&manifest, crate::capability::Source::Repository)
                .unwrap();
        let proof = match trust.decide(&entry, repo) {
            crate::capability::Decision::Approved(proof) => proof,
            crate::capability::Decision::Unapproved(why) => {
                panic!("the approval just written does not decide: {why}")
            }
        };
        // The adapter the record pinned is the path the delegation runs.
        let adapter = trust
            .records()
            .iter()
            .find(|record| record.slug == "approved-local")
            .unwrap()
            .adapter
            .clone();
        let found = crate::capability::Found {
            manifest: entry.manifest.clone(),
            presence: crate::capability::Presence::Present {
                version: "1.0.0".to_string(),
                report: "stub 1.0.0".to_string(),
                path: adapter,
            },
            workspace: repo.to_path_buf(),
            milliseconds: 0,
            proof,
            source: crate::capability::Source::Repository,
            digest: entry.digest.clone(),
            path: manifest.clone(),
        };
        let executor = crate::survey::executor(&found).expect("a present probe yields an executor");
        (executor, store, manifest)
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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

        // No binary at all is the harness, not the executor — the wrapper
        // exists even when the target does not, so absence is checked
        // before the command is ever built.
        let missing = Delegator::new(executor(dir.path().join("absent")))
            .run(task())
            .await;
        assert!(
            matches!(missing.status, Status::Harness(_)),
            "a missing binary is the harness, not a refusal: {missing:?}"
        );
        assert!(
            missing.detail.contains("no executor at"),
            "the detail names what was missing: {}",
            missing.detail
        );
        assert_eq!(missing.correct(), None);
    }

    /// The audit's delegation probe: an executor that starts a background
    /// child and is killed on its bound. Killing the direct delegate is
    /// not enough — the child has its own process, and it writes its
    /// marker a second later. The marker lands where the boundary grants
    /// a write, so its absence is the kill's proof rather than the
    /// deny's.
    #[tokio::test]
    async fn a_timed_out_delegation_ends_its_descendants() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let state = host.path().join("adapter-state");
        std::fs::create_dir(&state).unwrap();
        let marker = state.join("descendant-marker");
        let binary = stub(
            dir.path(),
            "backgrounding",
            &format!(
                "(sleep 3; printf harmless > '{}') & printf 'partway through'; wait",
                marker.display()
            ),
        );
        let executor = executor(&binary).under(Policy::empty().granting(&state));
        let delegation = Delegator::new(executor)
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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
        if !boundary_supported() {
            return;
        }
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

    /// A task's `writes: false` is a statement; the boundary is the wall.
    /// A delegate that tries to write anyway is denied by the kernel — in
    /// the directory it shares, everywhere it was never granted, and on
    /// every path the host sealed — and it writes only its private
    /// scratch and the adapter state the host granted.
    #[tokio::test]
    async fn a_read_only_delegate_writes_nowhere_but_scratch_and_grants() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // The grant and the seal live outside the shared directory: a
        // grant inside the protected checkout is refused, not bounded,
        // which is its own test below.
        let host = tempfile::tempdir().unwrap();
        let state = host.path().join("adapter-state");
        let sealed = host.path().join("sealed");
        std::fs::create_dir(&state).unwrap();
        std::fs::create_dir(&sealed).unwrap();
        let binary = stub(
            dir.path(),
            "writer",
            &format!(
                "printf x > '{0}/forbidden'; printf x > '{1}/nowhere'; \
                 printf x > '{2}/sealed-file'; printf x > '{3}/kept'; \
                 printf x > \"$TMPDIR/scratch-file\" && printf 'scratch\\n'; \
                 printf 'done\\n'",
                dir.path().display(),
                host.path().display(),
                sealed.display(),
                state.display(),
            ),
        );
        let executor = executor(&binary).under(Policy::empty().granting(&state).sealing(&sealed));
        let delegation = Delegator::new(executor)
            .in_directory(dir.path())
            .run(Task::reading("write something", "a.rs"))
            .await;

        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert!(delegation.output.contains("done"), "{}", delegation.output);
        assert!(
            delegation.output.contains("scratch"),
            "the private scratch was writable: {}",
            delegation.output
        );
        assert!(
            !dir.path().join("forbidden").exists(),
            "the shared directory stayed unwritten"
        );
        assert!(
            !host.path().join("nowhere").exists(),
            "the deny is the default, not the exception"
        );
        assert!(
            !sealed.join("sealed-file").exists(),
            "a sealed path accepts no exception"
        );
        assert!(
            state.join("kept").exists(),
            "the granted adapter state is the writable path"
        );

        let boundary = delegation.boundary.expect("the run records its boundary");
        let main = dir.path().canonicalize().unwrap();
        assert!(
            boundary.protected.contains(&main),
            "the shared checkout is protected: {:?}",
            boundary.protected
        );
        assert!(
            boundary.sealed.contains(&sealed.canonicalize().unwrap()),
            "the seal is in the record: {:?}",
            boundary.sealed
        );
        assert!(
            boundary.writable.contains(&state.canonicalize().unwrap()),
            "the grant is in the record: {:?}",
            boundary.writable
        );
    }

    /// A writing delegation gets a checkout of its own and writes nowhere
    /// else: the kernel denies the main checkout and the common Git
    /// directory, and the checkout stays for the reviewer when it ends.
    #[tokio::test]
    async fn a_writing_delegate_writes_only_its_checkout_and_keeps_it() {
        if !boundary_supported() {
            return;
        }
        let Some(repository) = scratch_repository() else {
            return;
        };
        let root = repository.path().canonicalize().unwrap();
        let binary = stub(
            repository.path(),
            "writer",
            &format!(
                "printf work > wrote.txt; printf x > '{0}/pwned'; \
                 printf x > '{0}/.git/pwned'; printf 'done\\n'",
                root.display()
            ),
        );
        let mut task = Task::reading("change a file", "a.rs");
        task.isolation = Isolation::Worktree;
        task.writes = true;

        let delegation = Delegator::new(executor(&binary))
            .in_repository(repository.path())
            .run(task)
            .await;

        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert!(
            !root.join("pwned").exists(),
            "the main checkout is protected"
        );
        assert!(
            !root.join(".git").join("pwned").exists(),
            "the common Git directory is sealed"
        );

        let boundary = delegation.boundary.expect("the run records its boundary");
        assert_eq!(
            boundary.checkout.as_deref(),
            Some(delegation.workdir.as_path()),
            "the checkout is the profile's one exception"
        );
        assert!(
            boundary.protected.contains(&root),
            "the main checkout is protected: {:?}",
            boundary.protected
        );
        assert!(
            boundary
                .sealed
                .contains(&root.join(".git").canonicalize().unwrap()),
            "the common Git directory is sealed: {:?}",
            boundary.sealed
        );

        // The writing delegation keeps its checkout: the edits are the
        // reviewer's to read, not this call's to discard.
        let kept = delegation
            .retained
            .as_ref()
            .expect("a writing delegation keeps its checkout");
        assert_eq!(kept, &delegation.workdir);
        assert_eq!(
            std::fs::read_to_string(kept.join("wrote.txt")).unwrap(),
            "work",
            "the edit is still there"
        );
    }

    /// A writing delegation that fails keeps its checkout too — partial
    /// work is still owed to a reviewer.
    #[tokio::test]
    async fn a_failed_writing_delegate_keeps_its_checkout() {
        if !boundary_supported() {
            return;
        }
        let Some(repository) = scratch_repository() else {
            return;
        };
        let binary = stub(
            repository.path(),
            "writer",
            "printf partial > wrote.txt\nexit 3",
        );
        let mut task = Task::reading("change a file", "a.rs");
        task.isolation = Isolation::Worktree;
        task.writes = true;

        let delegation = Delegator::new(executor(&binary))
            .in_repository(repository.path())
            .run(task)
            .await;

        assert_eq!(delegation.status, Status::Failed(3), "{delegation:?}");
        let kept = delegation
            .retained
            .expect("a writing delegation keeps its checkout, failed or not");
        assert_eq!(kept, delegation.workdir);
        assert!(
            kept.join("wrote.txt").exists(),
            "the partial edit is still there for the reviewer"
        );
    }

    /// What an approval grants, a delegate may write; the store that
    /// approved it, the manifest, and the adapter stay sealed all the
    /// same. Reads pass through the boundary — only writes are denied —
    /// so the adapter's world is unchanged except for its pen.
    #[tokio::test]
    async fn an_approved_delegate_writes_its_state_but_never_its_approval() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap(); // the shared checkout
        let host = tempfile::tempdir().unwrap(); // the operator's side
        let state = tempfile::tempdir().unwrap(); // the granted adapter state
        std::fs::write(
            host.path().join("credentials.toml"),
            "[fake]\ntoken = \"not-a-real-secret\"\n",
        )
        .unwrap();
        let binary = stub(
            host.path(),
            "adapter",
            &format!(
                "cat '{0}/credentials.toml'; printf x > '{1}/kept'; \
                 printf x > '{0}/forged.json'; printf x > '{2}/forbidden'; \
                 printf 'done\\n'",
                host.path().display(),
                state.path().display(),
                repo.path().display()
            ),
        );
        let (executor, _store, manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[state.path().to_path_buf()],
        );
        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("keep state", "a.rs"))
            .await;

        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert!(
            delegation.output.contains("not-a-real-secret"),
            "reads pass through the boundary: {}",
            delegation.output
        );
        assert!(
            state.path().join("kept").exists(),
            "the granted adapter state is writable"
        );
        assert!(
            !host.path().join("forged.json").exists(),
            "the store's directory is sealed"
        );
        assert!(
            !repo.path().join("forbidden").exists(),
            "the shared checkout is protected"
        );
        let boundary = delegation.boundary.expect("the run records its boundary");
        assert!(
            boundary
                .sealed
                .contains(&host.path().canonicalize().unwrap()),
            "the store's directory is sealed: {:?}",
            boundary.sealed
        );
        assert!(
            boundary.sealed.contains(&manifest.canonicalize().unwrap()),
            "the manifest is sealed: {:?}",
            boundary.sealed
        );
        assert!(
            boundary.sealed.contains(&binary.canonicalize().unwrap()),
            "the adapter is sealed: {:?}",
            boundary.sealed
        );
        assert!(
            boundary
                .writable
                .contains(&state.path().canonicalize().unwrap()),
            "the granted state is the writable path: {:?}",
            boundary.writable
        );
    }

    /// A script an approved argv names is pinned like the adapter: read
    /// again at dispatch, sealed against every writable exception, and
    /// hashed against the record — `sh` runs it, the boundary protects
    /// it.
    #[tokio::test]
    async fn a_pinned_script_is_sealed_for_the_delegate() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let script = stub(repo.path(), "program.sh", "printf 'ran\\n'");
        let (executor, _store, _manifest) = approved(
            repo.path(),
            host.path(),
            "sh",
            &["sh".to_string(), script.display().to_string()],
            &[],
        );
        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("run the program", "a.rs"))
            .await;

        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert!(delegation.output.contains("ran"), "{}", delegation.output);
        let boundary = delegation.boundary.expect("the run records its boundary");
        assert!(
            boundary.sealed.contains(&script.canonicalize().unwrap()),
            "the pinned script is sealed: {:?}",
            boundary.sealed
        );
    }

    /// A survey's approval is a cache, not a license: what the record
    /// approved at probe time is re-decided at dispatch, and a manifest
    /// changed since then runs nothing.
    #[tokio::test]
    async fn a_manifest_changed_since_approval_runs_nothing() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let marker = host.path().join("ran");
        let binary = stub(
            host.path(),
            "adapter",
            &format!("touch '{}'; printf 'ran\\n'", marker.display()),
        );
        let (executor, _store, manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[],
        );
        // Rewrite the manifest between the survey and the dispatch —
        // a different digest names no record.
        let changed = format!("{}\n", std::fs::read_to_string(&manifest).unwrap());
        std::fs::write(&manifest, changed).unwrap();

        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("trust me", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("unapproved".into()),
            "{delegation:?}"
        );
        assert!(!marker.exists(), "nothing spawned");
        assert!(delegation.boundary.is_none());
    }

    /// The adapter itself is pinned the same way: a binary replaced
    /// between the survey and the dispatch is a different binary than
    /// the operator approved, and runs nothing.
    #[tokio::test]
    async fn an_adapter_changed_since_approval_runs_nothing() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let marker = host.path().join("ran");
        let binary = stub(
            host.path(),
            "adapter",
            &format!("touch '{}'; printf 'ran\\n'", marker.display()),
        );
        let (executor, _store, _manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[],
        );
        let _ = stub(host.path(), "adapter", "printf 'evil\\n'");

        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("trust me", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("unapproved".into()),
            "{delegation:?}"
        );
        assert!(!marker.exists(), "nothing spawned");
        assert!(delegation.boundary.is_none());
    }

    /// A revoked record is the same refusal: the store is re-read at
    /// dispatch, so an operator pulling the plug between survey and run
    /// stops the delegation.
    #[tokio::test]
    async fn a_revoked_approval_runs_nothing() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let marker = host.path().join("ran");
        let binary = stub(
            host.path(),
            "adapter",
            &format!("touch '{}'; printf 'ran\\n'", marker.display()),
        );
        let (executor, store, _manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[],
        );
        crate::capability::Trust::load(&store)
            .unwrap()
            .revoke("approved-local")
            .unwrap();

        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("trust me", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("unapproved".into()),
            "{delegation:?}"
        );
        assert!(!marker.exists(), "nothing spawned");
        assert!(delegation.boundary.is_none());
    }

    /// A manifest rewritten and reapproved still refuses the executor the
    /// survey built: the fresh approval names the fresh manifest, and the
    /// cached executor's argv came from the old file. The operator's new
    /// word drives a fresh survey's executor — not this one.
    #[tokio::test]
    async fn a_reapproved_manifest_still_refuses_the_surveyed_executor() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let marker = host.path().join("ran");
        let binary = stub(
            host.path(),
            "adapter",
            &format!("touch '{}'; printf 'ran\\n'", marker.display()),
        );
        let (executor, store, manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[],
        );
        // The manifest gains an argument, and the operator approves the
        // change — the store now names the new bytes.
        let mut changed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
        changed["invoke"] = json!([binary.display().to_string(), "--eager"]);
        std::fs::write(&manifest, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
        crate::capability::Trust::load(&store)
            .unwrap()
            .approve(Some(repo.path()), "approved-local", &[])
            .unwrap();

        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("trust me", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("unapproved".into()),
            "{delegation:?}"
        );
        assert!(!marker.exists(), "nothing spawned");
        assert!(delegation.boundary.is_none());
    }

    /// A pinned argv word's *target* is sealed, but the word itself is
    /// what the argv spells: a word inside a granted directory can be
    /// retargeted by the delegate after verification — the pinned script
    /// swapped for one the approval never read. A grant covering a pinned
    /// word's ancestry refuses the delegation rather than carry a pin a
    /// child can swap.
    #[tokio::test]
    async fn a_pinned_alias_inside_a_grant_refuses_the_delegation() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let granted = tempfile::tempdir().unwrap();
        let real = stub(host.path(), "real.sh", "printf 'ran\\n'");
        // The argv names the script through an alias inside the granted
        // directory — the approval pins what the alias pointed at.
        let alias = granted.path().join("alias.sh");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let (executor, _store, _manifest) = approved(
            repo.path(),
            host.path(),
            "sh",
            &["sh".to_string(), alias.display().to_string()],
            &[granted.path().to_path_buf()],
        );
        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("run the program", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("boundary_unavailable".into()),
            "{delegation:?}"
        );
        assert!(delegation.output.is_empty(), "nothing ran");
        assert!(
            delegation.detail.contains("retarget"),
            "the refusal names the alias risk: {}",
            delegation.detail
        );
        assert!(delegation.boundary.is_none());
    }

    #[tokio::test]
    async fn a_trust_store_alias_under_a_writable_grant_refuses() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let granted = tempfile::tempdir().unwrap();
        let store_alias = granted.path().join("approval-directory");
        std::os::unix::fs::symlink(host.path(), &store_alias).unwrap();
        let (executor, _, _) = approved(
            repo.path(),
            &store_alias,
            "sh",
            &["sh".into(), "-c".into(), "printf ran".into()],
            &[granted.path().to_path_buf()],
        );
        let result = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("read only", "a.rs"))
            .await;
        assert_eq!(
            result.status,
            Status::Refused("boundary_unavailable".into()),
            "{result:?}"
        );
        assert!(result.output.is_empty());
        assert!(result.detail.contains("retarget"), "{}", result.detail);
    }

    /// The same refusal through a symlinked directory: the word's parent
    /// is a link inside the grant, and the delegate could swing the link
    /// to a directory of its own. The ancestry check follows the link —
    /// and the directory that holds the link is still the grant's.
    #[tokio::test]
    async fn a_pinned_alias_behind_a_writable_symlink_ancestor_refuses() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let granted = tempfile::tempdir().unwrap();
        // `target` holds the approved script; the argv word reaches it
        // through a symlink inside the granted directory.
        let target = tempfile::tempdir().unwrap();
        let _real = stub(target.path(), "real.sh", "printf 'ran\\n'");
        let linked = granted.path().join("linked-dir");
        std::os::unix::fs::symlink(target.path(), &linked).unwrap();
        let alias = linked.join("real.sh");
        let (executor, _store, _manifest) = approved(
            repo.path(),
            host.path(),
            "sh",
            &["sh".to_string(), alias.display().to_string()],
            &[granted.path().to_path_buf()],
        );
        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("run the program", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("boundary_unavailable".into()),
            "{delegation:?}"
        );
        assert!(delegation.output.is_empty(), "nothing ran");
        assert!(
            delegation.detail.contains("retarget"),
            "the refusal names the alias risk: {}",
            delegation.detail
        );
    }

    /// A grant that covers the approval store is refused rather than
    /// narrowed — a delegate that could rewrite the record it runs under
    /// would approve itself next.
    #[tokio::test]
    async fn a_grant_covering_the_store_refuses_rather_than_narrowing() {
        if !boundary_supported() {
            return;
        }
        let repo = tempfile::tempdir().unwrap();
        let host = tempfile::tempdir().unwrap();
        let binary = stub(host.path(), "adapter", "printf 'ran\\n'");
        // The operator's own grant names the store's directory.
        let (executor, _store, _manifest) = approved(
            repo.path(),
            host.path(),
            &binary.display().to_string(),
            &[binary.display().to_string()],
            &[host.path().to_path_buf()],
        );
        let delegation = Delegator::new(executor)
            .in_directory(repo.path())
            .run(Task::reading("approve yourself", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("boundary_unavailable".into()),
            "{delegation:?}"
        );
        assert!(delegation.output.is_empty(), "nothing ran");
        assert!(
            delegation.boundary.is_none(),
            "a refused delegation ran under no boundary"
        );
    }

    /// A grant that overlaps what the boundary protects is not a smaller
    /// grant: it is a refusal, because the profile cannot express
    /// "writable inside the protected checkout".
    #[tokio::test]
    async fn an_overbroad_grant_refuses_the_delegation_rather_than_narrowing_it() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "writer", "printf 'ran\\n'");
        // Adapter state inside the checkout the boundary protects would
        // let a delegate write into it.
        let grant = dir.path().join("adapter-state");
        std::fs::create_dir(&grant).unwrap();
        let executor = executor(&binary).under(Policy::empty().granting(&grant));
        let delegation = Delegator::new(executor)
            .in_directory(dir.path())
            .run(Task::reading("write somewhere", "a.rs"))
            .await;

        assert_eq!(
            delegation.status,
            Status::Refused("boundary_unavailable".into()),
            "{delegation:?}"
        );
        assert!(delegation.output.is_empty(), "nothing ran");
        assert!(delegation.boundary.is_none());
    }

    /// A bound that says nothing can be held refuses before the executor
    /// is asked: zero is no time, an overflowed count is not a smaller
    /// number, and a width of zero is not a width.
    #[tokio::test]
    async fn an_untenable_bound_or_width_refuses_before_anything_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let binary = stub(
            dir.path(),
            "spy",
            &format!("touch '{}'; printf 'ran\\n'", marker.display()),
        );
        let delegator = Delegator::new(executor(&binary)).in_directory(dir.path());

        let zero = delegator
            .run(Task::reading("anything", "a.rs").bounded(Bounds::within(Duration::ZERO)))
            .await;
        assert_eq!(zero.status, Status::Refused("bound_zero".into()));

        let overflow = delegator
            .run(Task::reading("anything", "a.rs").bounded(Bounds::minutes(u64::MAX)))
            .await;
        assert_eq!(overflow.status, Status::Refused("bound_overflow".into()));

        let narrow = Delegator::new(executor(&binary))
            .in_directory(dir.path())
            .bounded_to(0);
        let none = narrow.run(Task::reading("anything", "a.rs")).await;
        assert_eq!(none.status, Status::Refused("concurrency_zero".into()));
        assert_eq!(
            narrow.concurrent_max(),
            0,
            "the record says the width that was asked for"
        );

        assert!(!marker.exists(), "nothing spawned for any of them");
    }

    /// A writing delegation's checkout is retained before the executor
    /// ever spawns, so a caller that walks away mid-run leaves it — and
    /// the edits already in it — for the reviewer. The supervisor still
    /// terminates the group: `finished` is what the stub reaches only if
    /// its sleep ran out, and it never does.
    #[tokio::test]
    async fn a_cancelled_writing_delegation_keeps_its_checkout_and_its_edits() {
        if !boundary_supported() {
            return;
        }
        let Some(repository) = scratch_repository() else {
            return;
        };
        let binary = stub(
            repository.path(),
            "slow",
            "printf work > wrote.txt\nsleep 5\nprintf done > finished",
        );
        let delegator = Delegator::new(executor(&binary)).in_repository(repository.path());
        let mut task = Task::reading("work", "a.rs");
        task.isolation = Isolation::Worktree;
        task.writes = true;

        let running = tokio::spawn(async move { delegator.run(task).await });
        // Wait until the delegate's edit exists inside its checkout — the
        // delegation is in flight and has already written.
        let worktrees = repository.path().join(WORKTREE_DIR);
        let mut checkout = None;
        for _ in 0..160 {
            if let Some(path) = std::fs::read_dir(&worktrees)
                .ok()
                .and_then(|mut entries| entries.next())
                .and_then(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.join("wrote.txt").exists())
            {
                checkout = Some(path);
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let checkout = checkout.expect("the delegation wrote into its checkout");
        running.abort();
        assert!(
            running.await.is_err(),
            "the caller walked away mid-delegation"
        );

        // Past the point the sleep would have ended, `finished` is still
        // absent — the group was terminated — and the checkout keeps its
        // edit rather than following the cancelled task down.
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert!(
            !checkout.join("finished").exists(),
            "the delegate was terminated, not left running"
        );
        assert_eq!(
            std::fs::read_to_string(checkout.join("wrote.txt")).unwrap(),
            "work",
            "a cancelled writing delegation's edits stay for the reviewer"
        );
    }

    /// A read-only delegation's checkout is still released on
    /// cancellation: nothing was writable in it, nothing is owed to a
    /// reviewer, and the cleanup runs after the reaping — not while the
    /// executor could still be writing.
    #[tokio::test]
    async fn a_cancelled_read_only_delegation_releases_its_checkout() {
        if !boundary_supported() {
            return;
        }
        let Some(repository) = scratch_repository() else {
            return;
        };
        let binary = stub(repository.path(), "slow", "sleep 30");
        let delegator = Delegator::new(executor(&binary)).in_repository(repository.path());
        let mut task = Task::reading("work", "a.rs");
        task.isolation = Isolation::Worktree;

        let running = tokio::spawn(async move { delegator.run(task).await });
        let worktrees = repository.path().join(WORKTREE_DIR);
        let mut checkout = None;
        for _ in 0..200 {
            if let Some(path) = std::fs::read_dir(&worktrees)
                .ok()
                .and_then(|mut entries| entries.next())
                .and_then(|entry| entry.ok())
                .map(|entry| entry.path())
            {
                checkout = Some(path);
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let checkout = checkout.expect("the delegation made its checkout");
        running.abort();
        assert!(
            running.await.is_err(),
            "the caller walked away mid-delegation"
        );

        // The held guards drop inside the supervisor, after the group is
        // terminated and the child reaped — and then the worktree's own
        // cleanup removes it.
        for _ in 0..200 {
            if !checkout.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!(
            "a cancelled read-only delegation's checkout is still released: {}",
            checkout.display()
        );
    }

    /// The wrapped command repoints nothing but `TMPDIR`: the caller's
    /// environment passes through, so `HOME`, `XDG_DATA_HOME`,
    /// `XDG_CACHE_HOME`, and `XDG_RUNTIME_DIR` keep whatever the caller
    /// set. Where an adapter keeps its state is the approval's word —
    /// granted as writable paths — never an `XDG` variable the boundary
    /// redirected.
    #[tokio::test]
    async fn the_wrapped_command_repoints_only_tmpdir() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "writer", "true");
        let executor = executor(&binary);
        let delegator = Delegator::new(executor.clone()).in_directory(dir.path());
        let policy = executor.policy().resolve(dir.path()).unwrap();
        let (boundary, command) = delegator
            .prepare(
                &Task::reading("check", "a.rs"),
                &Checkout::Shared,
                dir.path(),
                &policy,
            )
            .await
            .unwrap();
        let envs: std::collections::BTreeMap<&OsStr, Option<&OsStr>> = command.get_envs().collect();
        assert_eq!(
            envs.get(OsStr::new("TMPDIR")).copied().flatten(),
            boundary.scratch().map(Path::as_os_str),
            "TMPDIR is the boundary's private scratch"
        );
        for name in ["HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"] {
            assert!(
                !envs.contains_key(OsStr::new(name)),
                "{name} passes through from the caller, never repointed"
            );
        }
    }

    /// And the passthrough is real, not just absent overrides: the
    /// delegate sees the caller's `HOME`, and `$TMPDIR` is a scratch the
    /// boundary owns — writable, private, and gone with the run.
    #[tokio::test]
    async fn the_caller_environment_passes_through_but_tmpdir_is_private() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let home = std::env::var("HOME").unwrap_or_default();
        let binary = stub(
            dir.path(),
            "env",
            "printf 'home=%s\\n' \"$HOME\"; printf x > \"$TMPDIR/private\"; printf 'tmp=ok\\n'",
        );
        let delegation = Delegator::new(executor(&binary))
            .in_directory(dir.path())
            .run(Task::reading("env", "a.rs"))
            .await;
        assert_eq!(delegation.status, Status::Answered, "{delegation:?}");
        assert!(
            delegation.output.contains(&format!("home={home}")),
            "the caller's environment passes through: {}",
            delegation.output
        );
        assert!(
            delegation.output.contains("tmp=ok"),
            "the private scratch is writable: {}",
            delegation.output
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
        assert_eq!(minutes.untenable(), None);
        let short = Bounds::within(Duration::from_millis(250));
        assert_eq!(short.wall(), Duration::from_millis(250));
        // And the bounds nothing can hold say so rather than wrapping or
        // clamping into a different bound.
        assert_eq!(
            Bounds::minutes(u64::MAX).untenable(),
            Some("bound_overflow")
        );
        assert_eq!(
            Bounds::within(Duration::ZERO).untenable(),
            Some("bound_zero")
        );
        assert_eq!(Bounds::minutes(0).untenable(), Some("bound_zero"));
        // A duration that multiplies but no Instant can reach is a
        // deadline that would panic rather than fire — refused, not run.
        assert_eq!(
            Bounds::within(Duration::MAX).untenable(),
            Some("bound_overflow")
        );
    }
}
