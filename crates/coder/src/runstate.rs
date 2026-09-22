//! The run state store: what a crashed program run recovers from.
//!
//! A program run crosses process boundaries — steps dispatch, tasks
//! delegate into worktrees of their own, and the host can die anywhere
//! in between. ATIF stays the evidence log; this store is the recovery
//! state. It holds one append-only `<run>.jsonl` file per run in the
//! directory the host gives it, and its records stay small and digested:
//! the run pins the base commit and the program, question, and source
//! digests it ran against, and a result is a reference to where the
//! evidence lives, never the evidence itself.
//!
//! # The lifecycle
//!
//! `pending` → `dispatched` → `answered` | `refused` | `unverifiable` |
//! `cancelled` → `settled`, with `unknown` standing apart as recovery's
//! mark rather than a transition:
//!
//! - **pending** — the claim is on disk. [`Store::claim`] writes it
//!   under `create_new`, so a run id is claimed once and a second claim
//!   refuses rather than replacing.
//! - **dispatched** — the work was handed out: the run to its first
//!   step, a step to its work, a task to an attempt.
//! - **answered | refused | unverifiable** — the work produced an
//!   answer, declined, or came back with nothing anyone could check.
//! - **cancelled** — the caller chose to end the record: a budget
//!   spent, a run stopped deliberately. It is the end someone asked
//!   for, written down — not the work's own decline, and not what a
//!   crash left behind.
//! - **settled** — terminal. [`Store::settle`] records what the run came
//!   to and the reference its result lives under, and nothing appends
//!   after it.
//! - **unknown** — [`Store::recover`] found a record with no terminal
//!   state. The mark is written, not assumed: a record the host cannot
//!   account for is marked, never freed.
//!
//! Every record shares the shape — the run's own, each step's, and each
//! task attempt's — and step and task records retain the worktree they
//! ran in, so a reconciler finds the checkout where the crash left it.
//!
//! # Recovery
//!
//! [`Store::recover`] returns every run whose record is incomplete —
//! claimed and never dispatched, dispatched and never answered, answered
//! and never settled, and the claim whose first line never landed —
//! marked `unknown` for the caller to reconcile. A claim that names a
//! live process as its owner is not a crash to recover: two
//! coordinators can hold one store, and marking a live run `unknown`
//! would re-label another's in-flight work. Nothing here replays:
//! what the run did is ATIF's evidence, and what this store answers is
//! only whether it finished.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The schema every record in the store carries.
pub const SCHEMA: &str = "openagents.runstate.v1";

/// The extension one run's record file carries.
const RECORD_EXT: &str = "jsonl";

/// The environment variable that switches runstate recording off:
/// `0`, `off`, `no`, and `false` all mean off.
pub const SWITCH_ENV: &str = "CODER_RUNSTATE";

/// The environment variable naming the directory runstate records go
/// to when recording is on.
pub const DIR_ENV: &str = "CODER_RUNSTATE_DIR";

/// Where this machine records run state, or `None` when recording is
/// off: `CODER_RUNSTATE` switches it off, `CODER_RUNSTATE_DIR` names
/// somewhere else, and the default is `~/.openagents/runstate`.
#[must_use]
pub fn directory() -> Option<PathBuf> {
    resolve(
        std::env::var(SWITCH_ENV).ok().as_deref(),
        std::env::var_os(DIR_ENV),
    )
}

/// The directory those two settings name. Split out from [`directory`]
/// so it can be tested without a test writing to the process
/// environment.
fn resolve(switch: Option<&str>, dir: Option<std::ffi::OsString>) -> Option<PathBuf> {
    if switch.is_some_and(|switch| {
        matches!(
            switch.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "no" | "false"
        )
    }) {
        return None;
    }
    match dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => default_dir(),
    }
}

/// The directory a run records to when nothing says otherwise:
/// `~/.openagents/runstate`. `None` when the home directory is unknown.
fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".openagents").join("runstate"))
}

/// Where one record stands.
///
/// `Unknown` is not a transition — no mark writes it but recovery's,
/// and no record moves out of `Settled`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Claimed, not yet dispatched.
    Pending,
    /// Handed out and not yet back.
    Dispatched,
    /// The work produced an answer.
    Answered,
    /// The work declined, or the host declined it.
    Refused,
    /// The work came back with nothing anyone could check.
    Unverifiable,
    /// The caller chose to end the record — a deliberate stop, recorded
    /// as one. `Refused` is the work's or the host's decline, `Unknown`
    /// is what a crash left, and `Cancelled` is neither: it is what
    /// someone asked for.
    Cancelled,
    /// Terminal: what the record came to is written.
    Settled,
    /// Recovery's mark on an unfinished record.
    Unknown,
}

/// What a settled run came to — the terminal half of its state.
///
/// These are the states a run settles from, kept as their own type
/// because they are also the outcome a settled record remembers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The run produced its answer.
    Answered,
    /// The run was refused — a step declined, or the host did.
    Refused,
    /// The run finished without an answer anyone could check.
    Unverifiable,
    /// The run was cancelled — the caller's own bound ended it. An end
    /// someone chose and recorded, never a refusal the work gave and
    /// never the `unknown` a crash leaves: the distinction is the whole
    /// point of durable state.
    Cancelled,
}

/// What a claim pins — the run's identity and the digests it ran under.
///
/// The digests are what make two runs of "the same" program comparable:
/// the program, the question sets its `decide` steps read, and the
/// sources its `query` steps read, each by digest and none by content.
#[derive(Clone, Debug)]
pub struct Claim<'a> {
    /// The run's identifier. It is also the record file's name, so its
    /// charset is the file system's: letters, digits, `-`, `_`, `.`.
    pub run: &'a str,
    /// The commit the run branched from.
    pub base: &'a str,
    /// The digest of the program the run executes.
    pub program: &'a str,
    /// The digests of the question sets the run's `decide` steps read.
    pub questions: &'a [String],
    /// The digests of the sources the run's `query` steps read.
    pub sources: &'a [String],
    /// The process that claimed the run, `0` when the claimer does not
    /// say. Recovery asks the operating system whether the owner is
    /// still alive before it marks the run's records `unknown`, so a
    /// second coordinator cannot re-label a live run's work.
    pub owner: u32,
}

/// Which record a mark moves: the run's own, one step's, or one task
/// attempt's.
#[derive(Clone, Copy, Debug)]
pub enum Subject<'a> {
    /// The run's own record.
    Run,
    /// One step's record, by the name the program gives it.
    Step(&'a str),
    /// One attempt of one task, by task name and attempt number — a
    /// retry is a new record, not a rewrite.
    Task(&'a str, u32),
}

/// A state change on one record.
#[derive(Clone, Debug)]
pub struct Mark<'a> {
    /// Which record moves.
    pub subject: Subject<'a>,
    /// Where it moves to.
    pub state: State,
    /// The worktree the record keeps, when it ran in one. A worktree a
    /// record retains outlives the process, so a reconciler finds the
    /// checkout where the crash left it.
    pub worktree: Option<PathBuf>,
    /// The reference the record's result lives under — an ATIF session,
    /// a commit — never the result itself.
    pub result: Option<String>,
}

impl<'a> Mark<'a> {
    /// A move of the run's own record.
    #[must_use]
    pub fn run(state: State) -> Self {
        Mark {
            subject: Subject::Run,
            state,
            worktree: None,
            result: None,
        }
    }

    /// A move of one step's record.
    #[must_use]
    pub fn step(step: &'a str, state: State) -> Self {
        Mark {
            subject: Subject::Step(step),
            state,
            worktree: None,
            result: None,
        }
    }

    /// A move of one attempt of one task's record.
    #[must_use]
    pub fn task(task: &'a str, attempt: u32, state: State) -> Self {
        Mark {
            subject: Subject::Task(task, attempt),
            state,
            worktree: None,
            result: None,
        }
    }

    /// The worktree this record keeps.
    #[must_use]
    pub fn retaining(mut self, worktree: impl Into<PathBuf>) -> Self {
        self.worktree = Some(worktree.into());
        self
    }

    /// The reference the record's result lives under.
    #[must_use]
    pub fn result(mut self, result: impl Into<String>) -> Self {
        self.result = Some(result.into());
        self
    }
}

/// One run's folded state: its own record plus its steps and tasks.
#[derive(Clone, Debug)]
pub struct Run {
    /// The schema the record was written under.
    pub schema: String,
    /// The run's identifier.
    pub run: String,
    /// The commit the run branched from, as the claim pinned it.
    pub base: String,
    /// The digest of the program the run executes.
    pub program: String,
    /// The digests of the question sets the run reads.
    pub questions: Vec<String>,
    /// The digests of the sources the run reads.
    pub sources: Vec<String>,
    /// Where the run's own record stands.
    pub state: State,
    /// What the run came to, when it settled.
    pub outcome: Option<Outcome>,
    /// The reference the result lives under, when one was recorded.
    pub result: Option<String>,
    /// The worktree the run's record retains, when it keeps one.
    pub worktree: Option<PathBuf>,
    /// The process that claimed the run, when the claim recorded one.
    /// A resumer probes it: alive, the run is not a crash to recover.
    pub owner: Option<u32>,
    /// Unix seconds of the run's last record.
    pub unix: u64,
    /// The run's step records, by step name.
    pub steps: Vec<Step>,
    /// The run's task attempts, by task name and attempt.
    pub tasks: Vec<Task>,
}

/// One step's folded record — the run's shape, minus the pins.
#[derive(Clone, Debug)]
pub struct Step {
    /// The schema the record was written under.
    pub schema: String,
    /// The step's name in the program.
    pub step: String,
    /// Where the record stands.
    pub state: State,
    /// The worktree the step ran in and retains.
    pub worktree: Option<PathBuf>,
    /// The reference the step's result lives under.
    pub result: Option<String>,
    /// Unix seconds of the step's last record.
    pub unix: u64,
}

/// One attempt of one task — the same shape again.
#[derive(Clone, Debug)]
pub struct Task {
    /// The schema the record was written under.
    pub schema: String,
    /// The task's name.
    pub task: String,
    /// The attempt number. A retry is a new record.
    pub attempt: u32,
    /// Where the record stands.
    pub state: State,
    /// The worktree the attempt ran in and retains.
    pub worktree: Option<PathBuf>,
    /// The reference the attempt's result lives under.
    pub result: Option<String>,
    /// Unix seconds of the attempt's last record.
    pub unix: u64,
}

/// Why the store refused.
#[derive(Debug)]
pub enum Refusal {
    /// The run id cannot name a record file.
    BadName { run: String },
    /// A record file already exists for this run id. A claim is once:
    /// it never replaces the record that is there.
    Claimed { run: String },
    /// No run record exists under this run id.
    Unclaimed { run: String },
    /// The run is settled, and terminal records take no more marks.
    Resolved { run: String },
    /// The mark is not a move the record may take.
    BadMark { run: String, reason: &'static str },
    /// The store itself failed — not a run-state answer.
    Store(Trouble),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadName { run } => write!(
                f,
                "run id {run:?} cannot name a record file — letters, digits, \
                 `-`, `_`, and `.`, and not a leading `.`"
            ),
            Self::Claimed { run } => write!(
                f,
                "run `{run}` is already claimed — a claim never replaces the \
                 record that is there"
            ),
            Self::Unclaimed { run } => write!(f, "no run record exists for `{run}`"),
            Self::Resolved { run } => write!(
                f,
                "run `{run}` is settled, and terminal records take no more marks"
            ),
            Self::BadMark { run, reason } => write!(f, "run `{run}`: {reason}"),
            Self::Store(trouble) => write!(f, "{trouble}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What went wrong with the store itself.
#[derive(Debug)]
pub enum Trouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// A record line did not parse where the record is not the torn
    /// tail of a crashed append — the store is corrupt.
    Corrupt(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Corrupt(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Trouble {}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Trouble> for Refusal {
    fn from(trouble: Trouble) -> Self {
        Self::Store(trouble)
    }
}

/// Which record a line carries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    Run,
    Step,
    Task,
}

/// One line of one run's record file: a record's complete state at the
/// time it was written. Folding keeps the last line per record, so a
/// mark line carries only what moved — the pins stay on the claim.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Record {
    schema: String,
    record: Kind,
    run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    program: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    questions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<String>,
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outcome: Option<Outcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worktree: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<u32>,
    unix: u64,
}

/// One file's lines folded: the run's records in order, each step's and
/// task attempt's latest, and where a torn tail begins when one is there.
#[derive(Default)]
struct Folded {
    runs: Vec<Record>,
    steps: BTreeMap<String, Record>,
    tasks: BTreeMap<(String, u32), Record>,
    /// The byte offset a torn tail starts at — the last line, when it
    /// does not parse. The next write truncates from here rather than
    /// leaving the torn bytes to corrupt the middle of the file.
    torn: Option<u64>,
}

/// The store: one directory, one append-only record file per run.
///
/// `open` creates the directory. `claim` creates the file — `create_new`
/// makes the claim atomic and exclusive, so the second claim of one run
/// id refuses rather than replacing. Every later write appends one line
/// and syncs before it reports, and `recover` folds what is there.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Open the store at `dir`, creating it.
    pub fn open(dir: &Path) -> Result<Self, Trouble> {
        std::fs::create_dir_all(dir)?;
        Ok(Store {
            dir: dir.to_path_buf(),
        })
    }

    /// The file one run's records live in.
    fn path(&self, run: &str) -> PathBuf {
        self.dir.join(format!("{run}.{RECORD_EXT}"))
    }

    /// Claim a run id.
    ///
    /// Writes the run's first record — `pending`, with the pins the
    /// claim carries — under `create_new`: the claim lands on disk
    /// before anything dispatches, and a second claim of the same run
    /// id refuses rather than replacing what is there. A claim that
    /// crashes between the create and the write still holds the id —
    /// the file is the claim — and recovery surfaces it.
    pub fn claim(&mut self, claim: &Claim<'_>) -> Result<Run, Refusal> {
        if !valid_run_id(claim.run) {
            return Err(Refusal::BadName {
                run: claim.run.to_string(),
            });
        }
        let record = Record {
            schema: SCHEMA.to_string(),
            record: Kind::Run,
            run: claim.run.to_string(),
            step: None,
            task: None,
            attempt: None,
            base: Some(claim.base.to_string()),
            program: Some(claim.program.to_string()),
            questions: claim.questions.to_vec(),
            sources: claim.sources.to_vec(),
            state: State::Pending,
            outcome: None,
            result: None,
            worktree: None,
            owner: (claim.owner != 0).then_some(claim.owner),
            unix: unix_now(),
        };
        let line =
            serde_json::to_string(&record).map_err(|error| Trouble::Corrupt(error.to_string()))?;
        let path = self.path(claim.run);
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{line}").map_err(Trouble::Io)?;
                file.sync_all().map_err(Trouble::Io)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Refusal::Claimed {
                    run: claim.run.to_string(),
                });
            }
            Err(error) => return Err(Trouble::Io(error).into()),
        }
        // The file's own sync orders its content; the directory's orders
        // the claim's existence.
        if let Ok(dir) = std::fs::File::open(&self.dir) {
            dir.sync_all().ok();
        }
        self.view(claim.run)
    }

    /// Move one record — the run's own, a step's, or a task attempt's —
    /// to a new state, appended and synced before it reports.
    ///
    /// What a mark may not be is `pending` — a record does not move back
    /// to its start — or `unknown`, which is recovery's mark and not a
    /// transition. And the run's own record does not settle here:
    /// [`Store::settle`] carries what the run came to. A step or task
    /// attempt may settle through a mark — its state is its outcome —
    /// and `cancelled` is a mark like any other move: the caller's own
    /// end, written where the record stands rather than left for
    /// recovery to guess at.
    pub fn advance(&mut self, run: &str, mark: Mark<'_>) -> Result<Run, Refusal> {
        if !valid_run_id(run) {
            return Err(Refusal::BadName {
                run: run.to_string(),
            });
        }
        let path = self.path(run);
        let folded = fold(&path)?;
        let Some(last) = folded.runs.last() else {
            return Err(Refusal::Unclaimed {
                run: run.to_string(),
            });
        };
        if last.state == State::Settled {
            return Err(Refusal::Resolved {
                run: run.to_string(),
            });
        }
        let reason = match (mark.subject, mark.state) {
            (_, State::Pending) => {
                Some("pending is where a claim starts, not a state a record moves to")
            }
            (_, State::Unknown) => {
                Some("unknown is recovery's mark on an unfinished record, not a transition")
            }
            (Subject::Run, State::Settled) => {
                Some("a run settles through settle(), which records what it came to")
            }
            _ => None,
        };
        if let Some(reason) = reason {
            return Err(Refusal::BadMark {
                run: run.to_string(),
                reason,
            });
        }
        let (record, step, task, attempt) = match mark.subject {
            Subject::Run => (Kind::Run, None, None, None),
            Subject::Step(step) => (Kind::Step, Some(step.to_string()), None, None),
            Subject::Task(task, attempt) => {
                (Kind::Task, None, Some(task.to_string()), Some(attempt))
            }
        };
        let record = Record {
            schema: SCHEMA.to_string(),
            record,
            run: run.to_string(),
            step,
            task,
            attempt,
            base: None,
            program: None,
            questions: Vec::new(),
            sources: Vec::new(),
            state: mark.state,
            outcome: None,
            result: mark.result,
            worktree: mark.worktree,
            owner: None,
            unix: unix_now(),
        };
        append(&path, &record, folded.torn)?;
        self.view(run)
    }

    /// Settle a run: record what it came to and the reference its result
    /// lives under. Terminal — nothing appends to the run after this.
    ///
    /// Settling twice returns the settled record: the answer recorded
    /// twice is not the answer twice. Settling a run id nobody claimed
    /// is refused — the store does not invent runs. Any incomplete state
    /// may settle, `unknown` included: the outcome the caller records is
    /// the reconciliation, and the reference says where its evidence is.
    pub fn settle(&mut self, run: &str, outcome: Outcome, result: &str) -> Result<Run, Refusal> {
        if !valid_run_id(run) {
            return Err(Refusal::BadName {
                run: run.to_string(),
            });
        }
        let path = self.path(run);
        if !path.exists() {
            return Err(Refusal::Unclaimed {
                run: run.to_string(),
            });
        }
        let folded = fold(&path)?;
        if folded
            .runs
            .last()
            .is_some_and(|record| record.state == State::Settled)
        {
            return self.view(run);
        }
        let record = Record {
            schema: SCHEMA.to_string(),
            record: Kind::Run,
            run: run.to_string(),
            step: None,
            task: None,
            attempt: None,
            base: None,
            program: None,
            questions: Vec::new(),
            sources: Vec::new(),
            state: State::Settled,
            outcome: Some(outcome),
            result: Some(result.to_string()),
            worktree: None,
            owner: None,
            unix: unix_now(),
        };
        append(&path, &record, folded.torn)?;
        self.view(run)
    }

    /// One run's folded state, when a record file exists for it.
    pub fn get(&self, run: &str) -> Result<Option<Run>, Trouble> {
        if !valid_run_id(run) {
            return Ok(None);
        }
        let path = self.path(run);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(view(run, fold(&path)?)))
    }

    /// One run's folded state, or the trouble that says it is not there —
    /// the claim, the advance, and the settle all end on this read.
    fn view(&self, run: &str) -> Result<Run, Refusal> {
        self.get(run)?.ok_or_else(|| {
            Refusal::Store(Trouble::Corrupt(format!(
                "run `{run}` has a claim and no record"
            )))
        })
    }

    /// Every run whose record is incomplete, marked `unknown`.
    ///
    /// Incomplete is anything that is not `settled`: claimed and never
    /// dispatched, dispatched and never back, back and never settled —
    /// and a claim file whose first line never landed, which is the
    /// claim that crashed mid-write and still holds its id. Each mark is
    /// appended before the run reports: an unfinished record is marked,
    /// not freed, and the caller reconciles what the mark names rather
    /// than replaying it. What the run did is ATIF's evidence; this
    /// store answers only whether it finished.
    ///
    /// A record already ended keeps its mark: `settled`, and `cancelled`
    /// — which is an end someone chose and wrote, not something a crash
    /// may be allowed to relabel — take no `unknown` from recovery.
    pub fn recover(&mut self) -> Result<Vec<Run>, Trouble> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == RECORD_EXT))
            .collect();
        paths.sort();
        let mut runs = Vec::new();
        for path in paths {
            let run = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_default();
            let folded = fold(&path)?;
            if folded
                .runs
                .last()
                .is_some_and(|record| record.state == State::Settled)
            {
                continue;
            }
            // A claim that names a live owner is not a crash to
            // recover — marking its records `unknown` would re-label
            // another coordinator's in-flight work.
            let owner = folded.runs.first().and_then(|record| record.owner);
            if owner.is_some_and(supervise::process_running) {
                continue;
            }
            let mut torn = folded.torn;
            if !folded
                .runs
                .last()
                .is_some_and(|record| record.state == State::Unknown)
            {
                append(&path, &unknown_run(&run), torn.take())?;
            }
            for record in folded.steps.values().chain(folded.tasks.values()) {
                // A settled or cancelled record is an end already
                // written, and an unknown one is already marked:
                // recovery adds to none of them.
                if matches!(
                    record.state,
                    State::Settled | State::Unknown | State::Cancelled
                ) {
                    continue;
                }
                append(&path, &unknown_mark(record), torn.take())?;
            }
            runs.push(view(&run, fold(&path)?));
        }
        Ok(runs)
    }
}

/// Whether a run id can name a record file — the file system's share of
/// the claim's honesty. Letters, digits, `-`, `_`, `.` — no separators,
/// no leading `.`, and never `.` or `..`.
fn valid_run_id(run: &str) -> bool {
    !run.is_empty()
        && !run.starts_with('.')
        && run
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Fold one record file into its current state.
///
/// A line that does not parse is the torn tail of a crashed append when
/// it is the last line — an earlier one is corruption. The fold reports
/// where the tail starts, and the next write truncates from there.
fn fold(path: &Path) -> Result<Folded, Trouble> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Folded::default());
        }
        Err(error) => return Err(Trouble::Io(error)),
    };
    let mut offset = 0u64;
    let mut lines = Vec::new();
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len() as u64;
        let line = raw.trim();
        if !line.is_empty() {
            lines.push((start, line));
        }
    }
    let mut folded = Folded::default();
    for (n, (start, line)) in lines.iter().enumerate() {
        let record: Record = match serde_json::from_str(line) {
            Ok(record) => record,
            Err(_) if n + 1 == lines.len() => {
                folded.torn = Some(*start);
                break;
            }
            Err(error) => {
                return Err(Trouble::Corrupt(format!(
                    "{}: line {}: {error}",
                    path.display(),
                    n + 1
                )));
            }
        };
        if record.schema != SCHEMA {
            return Err(Trouble::Corrupt(format!(
                "{}: line {}: schema {:?} is not {SCHEMA:?}",
                path.display(),
                n + 1,
                record.schema
            )));
        }
        match record.record {
            Kind::Run => folded.runs.push(record),
            Kind::Step => {
                let Some(step) = record.step.clone() else {
                    return Err(Trouble::Corrupt(format!(
                        "{}: line {}: a step record names no step",
                        path.display(),
                        n + 1
                    )));
                };
                folded.steps.insert(step, record);
            }
            Kind::Task => {
                let (Some(task), Some(attempt)) = (record.task.clone(), record.attempt) else {
                    return Err(Trouble::Corrupt(format!(
                        "{}: line {}: a task record names no task or attempt",
                        path.display(),
                        n + 1
                    )));
                };
                folded.tasks.insert((task, attempt), record);
            }
        }
    }
    Ok(folded)
}

/// Append one record line and sync before returning.
///
/// `torn` is where a torn tail starts, when the fold found one: the
/// write truncates from there first, because bytes that never reported
/// are not a record and leaving them would corrupt the middle of the
/// file once they stopped being last.
fn append(path: &Path, record: &Record, torn: Option<u64>) -> Result<(), Trouble> {
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    if let Some(torn) = torn {
        file.set_len(torn)?;
    }
    let line =
        serde_json::to_string(record).map_err(|error| Trouble::Corrupt(error.to_string()))?;
    writeln!(file, "{line}")?;
    file.sync_all()?;
    Ok(())
}

/// An `unknown` mark on the run's own record — what recovery appends to
/// a file whose run record is unfinished or was never written.
fn unknown_run(run: &str) -> Record {
    Record {
        schema: SCHEMA.to_string(),
        record: Kind::Run,
        run: run.to_string(),
        step: None,
        task: None,
        attempt: None,
        base: None,
        program: None,
        questions: Vec::new(),
        sources: Vec::new(),
        state: State::Unknown,
        outcome: None,
        result: None,
        worktree: None,
        owner: None,
        unix: unix_now(),
    }
}

/// An `unknown` mark on a step or task record. The mark keeps the
/// worktree and result the record carried — they are what a reconciler
/// looks at.
fn unknown_mark(record: &Record) -> Record {
    Record {
        schema: SCHEMA.to_string(),
        record: record.record,
        run: record.run.clone(),
        step: record.step.clone(),
        task: record.task.clone(),
        attempt: record.attempt,
        base: None,
        program: None,
        questions: Vec::new(),
        sources: Vec::new(),
        state: State::Unknown,
        outcome: None,
        result: record.result.clone(),
        worktree: record.worktree.clone(),
        owner: None,
        unix: unix_now(),
    }
}

/// The folded state as the caller sees it: the pins from the claim, the
/// state from the last run record, and every step and task attempt's
/// latest.
///
/// A file with no run record — a claim that crashed mid-write — folds
/// to a run that is `unknown` with empty pins. The file is the claim;
/// the run id is all it carries.
fn view(run: &str, folded: Folded) -> Run {
    let pins = folded
        .runs
        .iter()
        .find(|record| record.base.is_some() || record.program.is_some());
    let last = folded.runs.last();
    Run {
        schema: pins
            .or(last)
            .map(|record| record.schema.clone())
            .unwrap_or_else(|| SCHEMA.to_string()),
        run: run.to_string(),
        base: pins
            .and_then(|record| record.base.clone())
            .unwrap_or_default(),
        program: pins
            .and_then(|record| record.program.clone())
            .unwrap_or_default(),
        questions: pins
            .map(|record| record.questions.clone())
            .unwrap_or_default(),
        sources: pins
            .map(|record| record.sources.clone())
            .unwrap_or_default(),
        state: last.map(|record| record.state).unwrap_or(State::Unknown),
        outcome: last.and_then(|record| record.outcome),
        result: last.and_then(|record| record.result.clone()),
        worktree: last.and_then(|record| record.worktree.clone()),
        owner: pins.and_then(|record| record.owner),
        unix: last.map(|record| record.unix).unwrap_or_default(),
        steps: folded.steps.values().map(Step::of).collect(),
        tasks: folded.tasks.values().map(Task::of).collect(),
    }
}

impl Step {
    /// The folded record as a step.
    fn of(record: &Record) -> Self {
        Step {
            schema: record.schema.clone(),
            step: record.step.clone().unwrap_or_default(),
            state: record.state,
            worktree: record.worktree.clone(),
            result: record.result.clone(),
            unix: record.unix,
        }
    }
}

impl Task {
    /// The folded record as a task attempt.
    fn of(record: &Record) -> Self {
        Task {
            schema: record.schema.clone(),
            task: record.task.clone().unwrap_or_default(),
            attempt: record.attempt.unwrap_or_default(),
            state: record.state,
            worktree: record.worktree.clone(),
            result: record.result.clone(),
            unix: record.unix,
        }
    }
}

/// Unix seconds now.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim<'a>(run: &'a str, questions: &'a [String], sources: &'a [String]) -> Claim<'a> {
        Claim {
            run,
            base: "commit-abc123",
            program: "sha256:program",
            questions,
            sources,
            owner: 0,
        }
    }

    fn digests(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|tag| format!("sha256:{tag}")).collect()
    }

    #[test]
    fn claim_then_settle_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().join("wt-1");
        let mut store = Store::open(dir.path()).unwrap();
        let questions = digests(&["q1"]);
        let sources = digests(&["s1"]);
        let run = store.claim(&claim("run-1", &questions, &sources)).unwrap();
        assert_eq!(run.state, State::Pending);

        let run = store
            .advance("run-1", Mark::run(State::Dispatched))
            .unwrap();
        assert_eq!(run.state, State::Dispatched);
        let run = store
            .advance(
                "run-1",
                Mark::step("fan_out", State::Dispatched).retaining(&worktree),
            )
            .unwrap();
        assert_eq!(run.steps.len(), 1);
        assert_eq!(run.steps[0].step, "fan_out");
        assert_eq!(run.steps[0].worktree.as_deref(), Some(worktree.as_path()));
        let run = store
            .advance(
                "run-1",
                Mark::task("task-1", 1, State::Answered).result("atif:task-1"),
            )
            .unwrap();
        assert_eq!(run.tasks.len(), 1);
        assert_eq!(run.tasks[0].task, "task-1");
        assert_eq!(run.tasks[0].attempt, 1);
        assert_eq!(run.tasks[0].state, State::Answered);

        store.advance("run-1", Mark::run(State::Answered)).unwrap();
        let run = store
            .settle("run-1", Outcome::Answered, "atif:run-1")
            .unwrap();
        assert_eq!(run.state, State::Settled);
        assert_eq!(run.outcome, Some(Outcome::Answered));
        assert_eq!(run.result.as_deref(), Some("atif:run-1"));

        // And it all comes back off disk.
        let read = store.get("run-1").unwrap().unwrap();
        assert_eq!(read.state, State::Settled);
        assert_eq!(read.base, "commit-abc123");
        assert_eq!(read.steps.len(), 1);
        assert_eq!(read.tasks.len(), 1);
        // A settled run is complete: recovery surfaces nothing.
        assert!(store.recover().unwrap().is_empty());
        // Settling twice is the same record.
        let again = store
            .settle("run-1", Outcome::Answered, "atif:run-1")
            .unwrap();
        assert_eq!(again.state, State::Settled);
        // And nothing appends after terminal.
        assert!(matches!(
            store.advance("run-1", Mark::step("fan_out", State::Answered)),
            Err(Refusal::Resolved { .. })
        ));
    }

    #[test]
    fn a_second_claim_for_the_same_run_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store.claim(&claim("run-1", &[], &[])).unwrap();
        assert!(matches!(
            store.claim(&claim("run-1", &[], &[])),
            Err(Refusal::Claimed { .. })
        ));
        // A different run id claims its own record.
        store.claim(&claim("run-2", &[], &[])).unwrap();
    }

    #[test]
    fn an_abandoned_dispatch_surfaces_as_unknown_under_recover() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().join("wt-1");
        {
            let mut store = Store::open(dir.path()).unwrap();
            store.claim(&claim("run-1", &[], &[])).unwrap();
            store
                .advance("run-1", Mark::run(State::Dispatched))
                .unwrap();
            store
                .advance(
                    "run-1",
                    Mark::task("task-1", 1, State::Dispatched).retaining(&worktree),
                )
                .unwrap();
            // The host dies here: no answer, no settle.
        }
        // Recovery reopens the directory: the unfinished run is marked
        // unknown, and the retained worktree is still named for whoever
        // reconciles it.
        let mut store = Store::open(dir.path()).unwrap();
        let runs = store.recover().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run, "run-1");
        assert_eq!(runs[0].state, State::Unknown);
        assert_eq!(runs[0].tasks.len(), 1);
        assert_eq!(runs[0].tasks[0].state, State::Unknown);
        assert_eq!(
            runs[0].tasks[0].worktree.as_deref(),
            Some(worktree.as_path())
        );
        // The mark is durable: a second recover surfaces the still
        // unsettled run without appending again.
        let lines = std::fs::read_to_string(dir.path().join("run-1.jsonl"))
            .unwrap()
            .lines()
            .count();
        let runs = store.recover().unwrap();
        assert_eq!(runs.len(), 1);
        let again = std::fs::read_to_string(dir.path().join("run-1.jsonl"))
            .unwrap()
            .lines()
            .count();
        assert_eq!(lines, again);
        // And reconciliation ends in a settle, not a replay.
        let settled = store
            .settle("run-1", Outcome::Refused, "atif:run-1")
            .unwrap();
        assert_eq!(settled.state, State::Settled);
        assert!(store.recover().unwrap().is_empty());
    }

    #[test]
    fn the_pinned_digests_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let questions = digests(&["q-alpha", "q-beta"]);
        let sources = digests(&["s-gamma"]);
        store.claim(&claim("run-1", &questions, &sources)).unwrap();
        // Through marks and a settle the pins do not move.
        store
            .advance("run-1", Mark::run(State::Dispatched))
            .unwrap();
        store
            .settle("run-1", Outcome::Unverifiable, "atif:run-1")
            .unwrap();
        let run = store.get("run-1").unwrap().unwrap();
        assert_eq!(run.schema, SCHEMA);
        assert_eq!(run.base, "commit-abc123");
        assert_eq!(run.program, "sha256:program");
        assert_eq!(run.questions, questions);
        assert_eq!(run.sources, sources);
        // And they are on disk as the claim wrote them, not re-derived.
        let text = std::fs::read_to_string(dir.path().join("run-1.jsonl")).unwrap();
        let first: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(first["schema"], serde_json::json!(SCHEMA));
        assert_eq!(first["record"], serde_json::json!("run"));
        assert_eq!(first["base"], serde_json::json!("commit-abc123"));
        assert_eq!(first["program"], serde_json::json!("sha256:program"));
        assert_eq!(first["questions"], serde_json::json!(questions));
        assert_eq!(first["sources"], serde_json::json!(sources));
    }

    #[test]
    fn a_claim_that_crashed_mid_write_still_surfaces() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        // The create landed and the first line did not: the file is the
        // claim, and a second claim still refuses the id.
        std::fs::write(dir.path().join("run-1.jsonl"), "").unwrap();
        assert!(matches!(
            store.claim(&claim("run-1", &[], &[])),
            Err(Refusal::Claimed { .. })
        ));
        // Recovery surfaces the run id — it is all the file carries —
        // marked unknown for the caller to reconcile.
        let runs = store.recover().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run, "run-1");
        assert_eq!(runs[0].state, State::Unknown);
    }

    #[test]
    fn a_cancelled_mark_is_not_what_a_crash_left() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut store = Store::open(dir.path()).unwrap();
            store.claim(&claim("run-1", &[], &[])).unwrap();
            store
                .advance("run-1", Mark::run(State::Dispatched))
                .unwrap();
            store
                .advance("run-1", Mark::step("done", State::Answered))
                .unwrap();
            store
                .advance("run-1", Mark::step("rest", State::Cancelled))
                .unwrap();
            // The host dies between the cancelled marks and the settle.
        }
        let mut store = Store::open(dir.path()).unwrap();
        let runs = store.recover().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].state, State::Unknown);
        // Recovery marks what the crash left, not what the caller
        // chose: the answered step is unknown, the cancelled one keeps
        // the deliberate mark.
        let done = runs[0]
            .steps
            .iter()
            .find(|step| step.step == "done")
            .unwrap();
        let rest = runs[0]
            .steps
            .iter()
            .find(|step| step.step == "rest")
            .unwrap();
        assert_eq!(done.state, State::Unknown);
        assert_eq!(rest.state, State::Cancelled);
        // And reconciliation still ends in a settle — cancelled this
        // time, because that is what the run came to.
        let settled = store
            .settle("run-1", Outcome::Cancelled, "atif:run-1")
            .unwrap();
        assert_eq!(settled.state, State::Settled);
        assert_eq!(settled.outcome, Some(Outcome::Cancelled));
        assert!(store.recover().unwrap().is_empty());
    }

    /// A process id that was alive and is not: spawned, reaped, gone.
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn recovery_leaves_a_live_owners_run_alone() {
        let dir = tempfile::tempdir().unwrap();
        let questions = digests(&[]);
        let sources = digests(&[]);
        {
            let mut store = Store::open(dir.path()).unwrap();
            let mut live = claim("run-live", &questions, &sources);
            live.owner = std::process::id();
            store.claim(&live).unwrap();
            store
                .advance("run-live", Mark::step("work", State::Dispatched))
                .unwrap();
            let mut dead = claim("run-dead", &questions, &sources);
            dead.owner = dead_pid();
            store.claim(&dead).unwrap();
        }
        let mut store = Store::open(dir.path()).unwrap();
        let runs = store.recover().unwrap();
        // The live owner's run is not a crash to recover — a second
        // coordinator does not re-label another's in-flight work. The
        // dead owner's is.
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run, "run-dead");
        assert_eq!(runs[0].state, State::Unknown);
        let live = store.get("run-live").unwrap().unwrap();
        assert_eq!(live.state, State::Pending);
        assert_eq!(live.owner, Some(std::process::id()));
        // And the record file shows no unknown mark on the live run.
        let text = std::fs::read_to_string(dir.path().join("run-live.jsonl")).unwrap();
        assert!(!text.contains(r#""state":"unknown""#));
    }

    #[test]
    fn the_directory_answers_the_two_environment_names() {
        for off in ["0", "off", "no", "false", " OFF "] {
            assert_eq!(resolve(Some(off), None), None, "{off}");
        }
        assert_eq!(
            resolve(None, Some(std::ffi::OsString::from("/tmp/elsewhere"))),
            Some(PathBuf::from("/tmp/elsewhere"))
        );
        assert_eq!(resolve(None, None), default_dir());
    }
}
