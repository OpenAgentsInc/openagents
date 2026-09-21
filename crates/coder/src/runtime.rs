//! The program runtime: a program's steps, run from the program.
//!
//! [`crate::program`] reads a program and refuses one it cannot read.
//! This module runs the ones it can. The distinction it exists to keep is
//! that the order, the bounds, and the questions come out of the
//! definition rather than out of a function that happens to do the same
//! things in the same order — so an operator who edits
//! `programs/delegate-fan-out.json` changes what runs, and a host that
//! meets a program it cannot run says so instead of running most of it.
//!
//! # Four rules, and they are the whole point
//!
//! All four are [NIP-PRG](../../../nips/openagents/NIP-PRG.md)'s, and
//! together they are what makes this a runtime rather than a loop over a
//! list.
//!
//! **A step whose bounds the host cannot enforce does not run.** Not a
//! warning and not a substitution. [`Runtime::admit`] checks every step's
//! bounds against [`Host`] before the first step runs, so a program that
//! asks for something this host cannot hold to fails before it has done
//! anything rather than half way through.
//!
//! **A step kind the host does not run refuses the whole program.** An
//! unrecognized kind is refused when the file is read, by
//! [`crate::program::Program::load`]. A kind this version recognizes and
//! does not run — `module`, which is WebAssembly — is refused here, at
//! admission. Neither is ever skipped: a program whose unknown steps are
//! skipped is a different program, and it is the one a host would run by
//! accident.
//!
//! A `program` step runs the child program its address resolves to,
//! nested inside the parent's run. The composition's shape — cycles,
//! depth, step and call totals, widening bounds, pins, bindings, and the
//! propagation table — is checked once at admission by
//! [`crate::child::Composition`]; nothing structural is discovered at
//! dispatch. The child's steps spend the same budget and deadline the
//! parent's do, record themselves `parent/child` in the run's state so a
//! receipt can name where inside the composition work happened, and end
//! the way the step's stated propagation table says: a child that
//! completed or was refused lands on the parent step exactly as the
//! document mapped it. A `module` step stays refused until a dedicated
//! Wasm implementation exists.
//!
//! **A `decide` step names a question, never its wording.** The wording
//! lives in [`crate::questions`], addressed by identifier and digested as
//! a whole, and the run records which set answered beside the answer.
//!
//! **A `query` step names a source, never a command.** The source lives in
//! [`crate::source`], addressed by slug, and a slug this host has no
//! definition for refuses at admission. A step carrying an argv would make
//! the program code, which is the one thing NIP-PRG says a program is not.
//!
//! **A refused step stops the program**, and the reason it stopped is what
//! the run reports.
//!
//! # What runs, and what does not
//!
//! Five step kinds: `query`, `decide`, `check`, `delegate`, and `program`,
//! which nests a resolved child under narrowed bounds. `module` and
//! fetching stay specified and unbuilt. A runtime that runs one program
//! correctly is worth more than one that describes five.
//!
//! A `check` step gated on `gate_not_met` runs the operator-installed
//! verification plan rather than anything the program carries. Its
//! `acceptance` bound names the evidence every check in the plan must
//! produce — a typed suite verdict, never a bare exit status where a
//! suite was asked for — and `max_tests` bounds how many checks the plan
//! may run. Admission holds the plan to both before any check executes.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atif::{Call, Decision, Outcome};
use jev::{Answer, SystemOneRequest};
use serde_json::{Map, Value, json};

use crate::capability::{self, Presence};
use crate::delegate::{
    Bounds, Delegation, Delegator, Isolation, Status, Task, Verdict, boundary_supported,
};
use crate::program::{Kind, Program, Step};
use crate::program_authority::{self, Effects, Grant};
use crate::questions::{self, Fill, Set};
use crate::relay::RelayDoor;
use crate::runstate::{self, Claim, Mark, State, Store};
use crate::source::{self, OnOverflow, Overflow, Selection, Source};
use crate::survey::Survey;
use crate::trace::{Recorder, answers_value};

/// The name a task lookup records itself under in a trace.
pub const SELECT_CALL: &str = "task_select";

/// The name an admission check records itself under in a trace.
pub const ADMISSION_CALL: &str = "admission_check";

/// The name the program-selection decision records itself under.
pub const PROGRAM_CALL: &str = "program";

/// The question set that picks a program.
pub const PROGRAM_QUESTION: &str = "openagents.program.v1";

/// The option that says the request asks for no program at all.
///
/// The wording behind it lives in the question set, beside the wording of
/// every other option. This is only the slug the host reads the answer
/// back by. A program resolved under this slug would be unreachable, which
/// is why [`crate::program::Program::load`] refuses one.
pub const NO_PROGRAM: &str = "none";

/// The schema a recorded runtime step carries in its `extra`.
pub const STEP_SCHEMA: &str = "openagents.program-step.v1";

/// Bounds enforced by the host's delegation implementation.
///
/// A manifest's declaration is not evidence that an executor enforces a
/// bound. The host owns concurrency, checkout isolation, and deadlines.
const HOST_BOUNDS: &[&str] = &["concurrent_max", "isolation", "minutes"];

/// The one condition a `check` step's `refuse_on` may name here.
const INTERSECTION: &str = "cannot_enforce_intersection";

/// The field of a `delegate` step that every delegate reads before its
/// item.
const BRIEFING: &str = "briefing";

/// What a check refuses on beyond the condition the bound names: a bound
/// nobody has claimed either way.
const UNKNOWN: &str = "enforcement_unknown";

/// The code a run stopped by its own budget records. The caller's bound
/// ended the run rather than the work, so the run settles `cancelled` —
/// never `refused`, which is a decline the run gave itself.
const BUDGET_EXCEEDED: &str = "budget_exceeded";

/// Who holds a delegation to one of its bounds.
///
/// Three states rather than two, and the third is the one a two-state
/// answer gets wrong. `Manifest::ignored_bounds` intersects the required
/// bounds with `cannot_enforce` and admits everything else, so a bound
/// named by **neither** list passes — enforced, apparently, by having gone
/// unmentioned. It is not, and a check that admitted it would be a
/// formality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enforcement {
    /// The host keeps it, and does not need the executor's agreement.
    Host,
    /// The executor has been independently verified to keep it.
    /// Manifest declarations alone never establish this state.
    Executor,
    /// The executor declares it will silently ignore it. This is the
    /// dangerous one, and it is the one the bound names.
    Ignored,
    /// Nobody has said. Refused for the same reason as the last one: the
    /// delegation would run as though the bound held.
    Unknown,
}

impl Enforcement {
    /// The word a trace records this state under.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Enforcement::Host => "host",
            Enforcement::Executor => "executor",
            Enforcement::Ignored => "ignored",
            Enforcement::Unknown => "unknown",
        }
    }

    /// Whether a delegation may run under a bound in this state.
    #[must_use]
    pub fn holds(self) -> bool {
        matches!(self, Enforcement::Host | Enforcement::Executor)
    }
}

/// Why a step, or a program, did not run.
///
/// A refusal names the step rather than only the reason, because the
/// reason alone does not say where a program stopped and a run's evidence
/// is the pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The step that refused, or the empty string when the program
    /// refused before any step was reached.
    pub step: String,
    /// What the host calls this refusal.
    pub code: String,
    /// Why, in a sentence.
    pub reason: String,
}

impl Refused {
    /// A refusal at one step.
    #[must_use]
    pub fn at(step: &str, code: &str, reason: impl Into<String>) -> Self {
        Refused {
            step: step.to_string(),
            code: code.to_string(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.step.is_empty() {
            true => write!(f, "{}", self.reason),
            false => write!(f, "step {}: {}", self.step, self.reason),
        }
    }
}

/// What this host can hold a program to.
///
/// A host is not a policy and not a permission: it is the list of things
/// this process knows how to enforce. A bound outside it refuses the step
/// that named it, which is the rule that keeps a program from running
/// unbounded on a host that shrugged.
#[derive(Clone, Debug)]
pub struct Host {
    /// The checkout shapes this host can give a delegation.
    pub isolation: Vec<Isolation>,
}

impl Host {
    /// The host a machine with a git checkout is: it can share its
    /// directory, and it can make a checkout per delegation.
    #[must_use]
    pub fn with_repository() -> Self {
        Host {
            isolation: vec![Isolation::Directory, Isolation::Worktree],
        }
    }

    /// The host a machine with no checkout is. It can still delegate; it
    /// cannot isolate, so a step asking it to does not run.
    #[must_use]
    pub fn without_repository() -> Self {
        Host {
            isolation: vec![Isolation::Directory],
        }
    }

    /// Whether this host can give a delegation the shape a word names.
    #[must_use]
    pub fn provides(&self, word: &str) -> bool {
        Isolation::named(word).is_some_and(|shape| self.isolation.contains(&shape))
    }
}

/// The bound keys one step kind may carry here.
///
/// The table is the host's half of the contract: a program may name any
/// bound it likes, and a host runs only the steps whose bounds it is on
/// this list to keep.
#[must_use]
pub fn enforced(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Query => &["max_results", "on_overflow"],
        Kind::Decide => &[
            "refuse_below",
            "requires_scorable_answer",
            "per_requirement",
            "per_finding",
        ],
        Kind::Check => &["refuse_on", "acceptance", "max_tests"],
        Kind::Delegate => &["concurrent_max", "isolation", "minutes"],
        // The bounds a `program` step may declare for its child, each a
        // narrowing of what the composition has left: `spend` is the
        // child's ceiling in USD micros and `minutes` its own deadline.
        Kind::Program => &["depth", "steps", "calls", "spend", "minutes"],
        // WebAssembly is specified and not built. A host that met one and
        // ran the rest would be running a different program.
        Kind::Module => &[],
    }
}

/// What the selection question answered.
///
/// Two answers rather than one and an error, because "this is not a
/// program request" is the common case and reading it as a failure would
/// put the ordinary turn on the error path.
///
/// Named apart from [`source::Selection`], which is what a `query` step's
/// lookup selected. This one is which program runs at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selected {
    /// The request asks for no program. The caller answers it the way it
    /// always has.
    None,
    /// The request asks for this program, by slug.
    Program(String),
}

impl Selected {
    /// The program this answer names, when it names one.
    #[must_use]
    pub fn program(&self) -> Option<&str> {
        match self {
            Selected::None => None,
            Selected::Program(slug) => Some(slug),
        }
    }
}

/// What a run was given: the operator's sentence, the work, and which
/// capability is to do it.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// The operator's sentence, verbatim. A program is selected from it
    /// and it is the state the selection question reads.
    pub request: String,
    /// The work, one task per delegation. This is what a `query` step
    /// looks up, bounded by that step's `max_results`.
    pub tasks: Vec<Task>,
    /// The capability slug a `delegate` step hands work to.
    pub executor: String,
}

impl Inputs {
    /// What a run gets from the operator's sentence.
    ///
    /// The work is the list the sentence carries: one task per bulleted or
    /// numbered line, in the order it was written. A sentence carrying no
    /// list supplies no work, and a `query` step reading the request then
    /// refuses rather than inventing any — a fan-out over work nobody
    /// named is the failure this whole path is bounded against.
    ///
    /// Reading a list is deterministic parsing of a bounded field, which
    /// `AGENTS.md` allows **after** the semantic route is chosen: the
    /// program was selected by a decision model before anything here runs,
    /// and what is read is the shape of a line rather than its meaning.
    ///
    /// This is what the `request` source answers with. A `query` step that
    /// names another source reads that instead, and the sentence supplies
    /// nothing — see [`crate::source`].
    #[must_use]
    pub fn read(request: &str, executor: &str) -> Self {
        Inputs {
            request: request.to_string(),
            tasks: listed(request),
            executor: executor.to_string(),
        }
    }
}

/// One step, as the run reports it.
#[derive(Clone, Debug)]
pub struct Ran {
    pub name: String,
    pub kind: Kind,
    /// What the step produced, in one line.
    pub output: String,
}

/// What a run of one program did.
#[derive(Clone, Debug, Default)]
pub struct Run {
    /// The program that ran, when one was selected.
    pub program: Option<String>,
    /// The steps that ran, in order.
    pub steps: Vec<Ran>,
    /// Where the program stopped, when it stopped early.
    pub stopped: Option<Refused>,
    /// What the `query` step looked up: the work, its order, what was
    /// dropped, and what collides.
    pub selection: Option<Selection>,
    /// Every delegation a `delegate` step started.
    pub delegations: Vec<Delegation>,
    /// The typed answers each `decide` step got, by step name.
    pub answers: BTreeMap<String, Value>,
    /// Independently executed host checks, separate from delegate answers.
    pub verification: Vec<crate::verification::Report>,
    /// The collected review evidence and per-finding dispositions, when a
    /// `per_finding` decide step ran. `None` is unreviewed — a run whose
    /// review never happened — which an empty findings list would hide.
    pub review: Option<crate::review::Reviewed>,
    /// What the run's priced lanes held when it settled, one book per
    /// lane — the spend record a bounded run leaves behind. Empty when
    /// the caller stated no spend bound.
    pub spend: Vec<crate::spend::Book>,
}

/// What a resumer rules over one recovered run.
///
/// The rulings pair the two questions recovery asks in the order a
/// resumer reads them: [`crate::reattach`] answers whether in-flight
/// work still runs under the reference a `dispatched` record holds,
/// and [`crate::reconcile`] answers what a record recovery marked
/// `unknown` may do next. A record that reattaches `Observe` needs no
/// replay decision; one that cannot reattach is what reconciliation
/// rules.
#[derive(Clone, Debug)]
pub struct Rulings {
    /// Per-subject reattachment rulings — `run`, `step:<name>`, or
    /// `task:<name>#<attempt>` — one per unfinished record holding a
    /// dispatch reference.
    pub reattach: Vec<(String, crate::reattach::Reattachment)>,
    /// Per-subject reconciliation rulings over every `unknown` record,
    /// each carrying the declared effects the ruling read and the
    /// evidence the record kept.
    pub reconcile: Vec<crate::reconcile::Ruling>,
}

impl Run {
    /// Whether every step ran.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.stopped.is_none()
    }

    /// The steps that ran, by name.
    #[must_use]
    pub fn step_names(&self) -> Vec<String> {
        self.steps.iter().map(|step| step.name.clone()).collect()
    }

    /// How many delegations answered.
    #[must_use]
    pub fn answered(&self) -> usize {
        self.delegations
            .iter()
            .filter(|delegation| delegation.answered())
            .count()
    }

    /// How many answered the way the task expected, out of the ones the
    /// task could say anything about.
    #[must_use]
    pub fn correct(&self) -> (usize, usize) {
        let graded: Vec<bool> = self
            .delegations
            .iter()
            .filter_map(Delegation::correct)
            .collect();
        (graded.iter().filter(|right| **right).count(), graded.len())
    }

    /// Each delegation's completion verdict, under the requirement name
    /// the `accept` step asks about it by.
    #[must_use]
    pub fn verdicts(&self) -> Vec<(String, Verdict)> {
        self.delegations
            .iter()
            .enumerate()
            .map(|(n, delegation)| (requirement_name(n), delegation.verdict()))
            .collect()
    }

    /// How many delegations passed, failed, and could not be verified.
    /// The three sum to the delegations; only the first is a pass.
    #[must_use]
    pub fn tally(&self) -> Tally {
        let mut tally = Tally::default();
        for delegation in &self.delegations {
            match delegation.verdict() {
                Verdict::Passed => tally.passed += 1,
                Verdict::Failed => tally.failed += 1,
                Verdict::Unverifiable => tally.unverifiable += 1,
            }
        }
        tally
    }

    /// What the run comes to, in the sentence a reader sees last.
    #[must_use]
    pub fn summary(&self) -> String {
        let program = self.program.as_deref().unwrap_or("no program");
        let Some(stopped) = &self.stopped else {
            if !self.verification.is_empty() {
                return format!(
                    "{program} ran {} steps and passed {} independent verification plans.",
                    self.steps.len(),
                    self.verification.len()
                );
            }
            let tally = self.tally();
            let wall: f64 = self
                .delegations
                .iter()
                .map(|delegation| delegation.elapsed.as_secs_f64())
                .fold(0.0, f64::max);
            let summed: f64 = self
                .delegations
                .iter()
                .map(|delegation| delegation.elapsed.as_secs_f64())
                .sum();
            return format!(
                "{program} ran its {} steps: {} delegations, {} answered, {tally}, \
                 in {wall:.1} seconds of wall clock against {summed:.1} seconds of summed agent time.",
                self.steps.len(),
                self.delegations.len(),
                self.answered(),
            );
        };
        format!("{program} stopped at {stopped}.")
    }
}

/// What a step list's run came to.
///
/// The distinction the runtime keeps between the three ends: `Finished`
/// is the document's own end, `Ended` is the work's or the host's answer
/// to a step, and `Cancelled` is the caller's bound — which is never a
/// child program's outcome and never something a propagation table maps.
enum StepsEnd {
    /// Every step in the list answered.
    Finished,
    /// A step refused; the list stops where it stopped.
    Ended(Refused),
    /// The caller's budget or deadline spent; carries the refusal the
    /// run settles `cancelled` under.
    Cancelled(Refused),
}

/// Whose bound spent at a step boundary — the distinction between the
/// run's own budget, which cancels the run, and a `minutes` a `program`
/// step declared for its child, which is the child's refusal and the
/// propagation table's to map.
enum Spent {
    /// The run's step budget or deadline — the caller's end.
    Run(Refused),
    /// A child's declared deadline — the child's refusal.
    Child(Refused),
}

/// The state one [`Runtime::run_steps`] borrows for the whole run, shared
/// by every nested list it expands.
///
/// A child program does not get a copy: its steps spend the same
/// `dispatched` count against the same budget, mark the same `record`,
/// and produce into the same map — a child's bound is what the parent
/// had left, which is how a child narrows and never widens.
struct StepRun<'a> {
    /// The operator's sentence, the work, and the executor — the child's
    /// own when a `program` step's binding projects them.
    inputs: &'a Inputs,
    /// The grant the whole run answers to; a child runs under it, never
    /// a wider one.
    grant: &'a Grant,
    /// The deadline this step list answers to: the run's own, narrowed
    /// by any `minutes` a `program` step declared for this child. An
    /// expiry against `run_until` is the caller's cancellation; an
    /// expiry against a tighter `until` is the child's own bound ending
    /// it, which the step's propagation table decides.
    until: Option<Instant>,
    /// The run's own deadline, constant across the composition — the
    /// mark an expiry compares `until` against to say whose bound spent.
    run_until: Option<Instant>,
    /// The work the `query` step selected, when one ran.
    selection: &'a mut Selection,
    /// The run being recorded.
    run: &'a mut Run,
    /// The recovery record, when the operator pointed the runtime at one.
    record: &'a mut Option<(Store, String)>,
    /// Steps dispatched across the whole composition, for the step
    /// budget.
    dispatched: &'a mut usize,
    /// Every step's output and declared produced fields, keyed
    /// `step` and `step.field` under the step's full `parent/child`
    /// name — what a `program` step's binding reads when it projects
    /// inputs for its child.
    produced: &'a mut BTreeMap<String, Value>,
    /// The spend scopes the composition is inside: the run's own book
    /// first, then one per child that declared a `spend` ceiling. A
    /// charge walks them all — the child's ceiling answers for the
    /// child and the run's for the run, so a child spends the parent's
    /// room and never its own copy of it.
    scopes: &'a mut Vec<Scope>,
    /// The session's trace, when one is being kept.
    trace: Option<&'a mut Recorder>,
}

/// One level's spend ledger — the run's own book or a child's declared
/// ceiling — and the lane books opened under it.
struct Scope {
    /// Whether this scope is the run's own: its bound spending cancels
    /// the run, where a child's bound spending is the child's refusal —
    /// an outcome the propagation table decides.
    run: bool,
    /// The bound the level runs under. A child's declared `spend` is a
    /// hard ceiling: a bound that does not stop is not a bound, and a
    /// lane that cannot price itself cannot hold one.
    bound: crate::spend::Bound,
    /// Lane books opened under the bound, as the work first charges
    /// them.
    books: BTreeMap<crate::spend::Lane, crate::spend::Book>,
}

impl Scope {
    /// The room this scope still promises one lane: its ceiling less
    /// what the lane's book already holds, when the scope has a ceiling
    /// and a book at all. `None` is "no ceiling stated", the room a
    /// child may always ask into.
    fn room(&self, lane: crate::spend::Lane) -> Option<u64> {
        let ceiling = self.bound.ceiling()?;
        Some(
            ceiling.saturating_sub(
                self.books
                    .get(&lane)
                    .map_or(0, crate::spend::Book::metered_micros),
            ),
        )
    }
}

/// A run's completion verdicts, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Delegations whose answer is the one the work item stated.
    pub passed: usize,
    /// Delegations whose work item stated an answer they did not give.
    pub failed: usize,
    /// Delegations whose work item stated no answer to check.
    pub unverifiable: usize,
}

impl std::fmt::Display for Tally {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} passed, {} failed, {} unverifiable",
            self.passed, self.failed, self.unverifiable
        )
    }
}

/// The caller's bound on a whole run.
///
/// A budget is the caller's, stated when the runtime is built — a
/// program cannot state its own honestly, and no step bound reaches
/// across the run. Either half may stand alone: a deadline with no step
/// count ends the run when the clock does, a step count with no
/// deadline ends it when the count is spent. A run that reaches a step
/// boundary past its budget cancels the step rather than dispatching
/// it, and a deadline that expires while a step dispatches ends the
/// step's work where it stands — the subprocess group goes down through
/// supervise's own cancel path. Either way the step marks `cancelled`
/// and the run settles `cancelled` — the end the caller chose, which is
/// the whole point of recording it apart from `refused` and `unknown`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Budget {
    /// How long the run may take, measured from when it starts. The
    /// bound reaches inside a step: the work a step spawns runs under
    /// the tighter of its own bound and what the deadline leaves, and an
    /// expiry while the step dispatched ends the step `cancelled`.
    pub deadline: Option<Duration>,
    /// How many steps the run may dispatch. The step past the count
    /// cancels rather than dispatching.
    pub max_steps: Option<usize>,
    /// What the run may spend, in the book's own terms: a hard ceiling
    /// holds only where the work prices itself, a soft one records and
    /// flags, and `None` keeps the ledger without promising anything.
    /// [`crate::spend`] carries the rules.
    pub spend: Option<crate::spend::Bound>,
}

/// One machine, running programs.
pub struct Runtime {
    survey: Survey,
    questions: questions::Registry,
    door: Option<jev::Client>,
    door_error: Option<String>,
    relay: Option<RelayDoor>,
    repository: Option<PathBuf>,
    host: Host,
    verification: Option<(PathBuf, crate::verification::Plan, capability::Trust)>,
    review: Option<crate::review::Context>,
    runstate: Option<PathBuf>,
    budget: Option<Budget>,
}

impl Runtime {
    /// Reads what this machine can reach, what it could run, and what it
    /// may ask, then holds them for the run.
    ///
    /// `repository` is the checkout the host is working in. A host with
    /// one can isolate a delegation; a host without one cannot, and says
    /// so through [`Host`] rather than by pretending.
    #[must_use]
    pub fn open(repository: Option<&Path>, workspace: &Path) -> Self {
        Self::using(Survey::read(repository, workspace), repository)
    }

    /// A runtime over a survey the caller already read.
    ///
    /// The probe spawns a process per declared capability, so a caller
    /// holding a survey hands it over rather than paying for a second one.
    /// A conversation reads its survey once and runs every turn's
    /// selection against it.
    #[must_use]
    pub fn using(survey: Survey, repository: Option<&Path>) -> Self {
        let (door, door_error) = match crate::decision::from_env() {
            Ok(door) => (door, None),
            Err(error) => (None, Some(error)),
        };
        Runtime {
            survey,
            questions: questions::Registry::open(&questions::search(repository)),
            door,
            door_error,
            relay: None,
            verification: None,
            review: None,
            runstate: None,
            budget: None,
            repository: repository.map(Path::to_path_buf),
            host: match repository {
                Some(_) => Host::with_repository(),
                None => Host::without_repository(),
            },
        }
    }

    /// A runtime over parts a caller assembled, for a test that drives it
    /// without the machine's own answers.
    #[must_use]
    pub fn over(survey: Survey, questions: questions::Registry, host: Host) -> Self {
        let (door, door_error) = match crate::decision::from_env() {
            Ok(door) => (door, None),
            Err(error) => (None, Some(error)),
        };
        Runtime {
            repository: Some(survey.workspace.clone()),
            survey,
            questions,
            door,
            door_error,
            relay: None,
            verification: None,
            review: None,
            runstate: None,
            budget: None,
            host,
        }
    }

    /// Install an operator-prepared verification plan. Program text cannot set it.
    #[must_use]
    pub fn with_verification(
        mut self,
        workspace: PathBuf,
        plan: crate::verification::Plan,
        trust: capability::Trust,
    ) -> Self {
        self.verification = Some((workspace, plan, trust));
        self
    }

    /// Install an operator-prepared review: the pinned reviewer, the
    /// captured diff scope, the disposition policy, and the trust that
    /// approves. Program text cannot set it, and a `per_finding` decide
    /// step refuses at admission without one.
    #[must_use]
    pub fn with_review(mut self, context: crate::review::Context) -> Self {
        self.review = Some(context);
        self
    }

    /// The review the operator installed, when one is.
    #[must_use]
    pub fn review(&self) -> Option<&crate::review::Context> {
        self.review.as_ref()
    }

    /// Records each program run in the runstate store at `dir`, so a run
    /// leaves the recovery state a crash reconciles from. Unset, no
    /// store is opened and nothing about a run changes. The store
    /// observes a run — it never gates one.
    #[must_use]
    pub fn with_runstate(mut self, dir: impl Into<PathBuf>) -> Self {
        self.runstate = Some(dir.into());
        self
    }

    /// Bounds each run: the caller's deadline and step count, checked at
    /// every step boundary before the step dispatches and again while it
    /// does. A step reached past the budget never runs, and a deadline
    /// that expires mid-step stops the step's work through supervise's
    /// cancel path — the bound each spawned process group already runs
    /// under — rather than waiting for the next boundary. Either way the
    /// step marks `cancelled`, and so does every step after it, and the
    /// run settles `cancelled`: the end the caller chose, never a
    /// refusal the work gave. Unset, the run checks nothing and behaves
    /// exactly as before.
    #[must_use]
    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Answers the decision questions through this door rather than the
    /// one the environment names.
    #[must_use]
    pub fn asking(mut self, door: Option<jev::Client>) -> Self {
        self.door = door;
        self.door_error = None;
        self
    }

    /// Holds programs to what a different host can enforce, for a caller
    /// showing what a machine without this one's checkout would do.
    #[must_use]
    pub fn on(mut self, host: Host) -> Self {
        self.host = host;
        self
    }

    /// What the machine can reach and what it could run.
    #[must_use]
    pub fn survey(&self) -> &Survey {
        &self.survey
    }

    /// Probes the relay capabilities the survey declared and keeps the
    /// door that answered, so a `delegate` step naming one can hand its
    /// tasks over it. See [`Survey::probe_relays`].
    pub async fn probe_relays(&mut self) {
        self.relay = self.survey.probe_relays().await;
    }

    /// What this host can hold a program to.
    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Whether this host would run every step of a program, and why not
    /// when it would not.
    ///
    /// Run before the first step, so a program this host cannot hold to
    /// fails before it has done anything.
    ///
    /// # Errors
    ///
    /// Returns the first step this host would refuse, and the reason.
    pub fn admit(&self, program: &Program) -> Result<(), Refused> {
        // A program that calls children answers for its whole composition
        // before the first step runs: every reference resolves pinned,
        // every propagation is stated, no chain returns to itself, and no
        // bound exceeds or widens. The first problem is the refusal.
        if program.steps.iter().any(|step| step.kind == Kind::Program)
            && let Some(problem) = crate::child::Composition::check(program, &self.survey.programs)
                .into_iter()
                .next()
        {
            return Err(Refused::at("", "composition_refused", problem.to_string()));
        }
        // A spend ceiling holds only where the work prices itself: a
        // hard bound over a lane this host cannot meter is refused here,
        // at the same door every other unenforceable bound answers at,
        // rather than discovered at the first charge.
        if let Some(bound) = self.budget.and_then(|budget| budget.spend) {
            let mut lanes = Vec::new();
            self.spend_lanes(program, "", &mut lanes);
            for (step, lane) in lanes {
                if crate::spend::Book::open(lane, bound, false).is_err() {
                    return Err(Refused::at(
                        &step,
                        "bound_unenforceable",
                        format!(
                            "the run's spend bound is a hard ceiling and this host cannot price {} work, so it cannot hold the ceiling",
                            lane.name()
                        ),
                    ));
                }
            }
        }
        self.admit_tree(program)
    }

    /// The priced lanes a program's steps will charge, with the step
    /// that charges each — recursively, so a child's `decide` is as
    /// visible to a spend bound as the parent's own.
    fn spend_lanes<'a>(
        &'a self,
        program: &'a Program,
        prefix: &str,
        lanes: &mut Vec<(String, crate::spend::Lane)>,
    ) {
        for step in &program.steps {
            match step.kind {
                Kind::Decide => lanes.push((
                    format!("{prefix}{}", step.name),
                    crate::spend::Lane::Decision,
                )),
                Kind::Delegate => lanes.push((
                    format!("{prefix}{}", step.name),
                    crate::spend::Lane::Delegate,
                )),
                Kind::Program => {
                    if let Some(child) = self.resolve_child(step) {
                        self.spend_lanes(child, &format!("{prefix}{}/", step.name), lanes);
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether this host would run every step of a program and of every
    /// child its `program` steps resolve to.
    ///
    /// The composition check already ran, so the recursion is acyclic
    /// and depth-bounded: a child a step names is admitted as its own
    /// program, and a step kind the host does not run anywhere in the
    /// graph refuses the whole thing before the first step rather than
    /// mid-run.
    fn admit_tree(&self, program: &Program) -> Result<(), Refused> {
        for step in &program.steps {
            self.admit_step(program, step)?;
        }
        for step in &program.steps {
            if step.kind == Kind::Program
                && let Some(child) = self.resolve_child(step)
            {
                self.admit_tree(child)?;
            }
        }
        Ok(())
    }

    /// Whether this host would run one step.
    fn admit_step(&self, program: &Program, step: &Step) -> Result<(), Refused> {
        let keeps = enforced(step.kind);
        if keeps.is_empty() {
            return Err(Refused::at(
                &step.name,
                "step_kind_unavailable",
                format!(
                    "this host does not run a {} step, and a program whose steps it skipped would be a different program",
                    step.kind.word()
                ),
            ));
        }
        for (bound, value) in &step.bounds {
            if !keeps.contains(&bound.as_str()) {
                return Err(Refused::at(
                    &step.name,
                    "bound_unenforceable",
                    format!(
                        "this host cannot enforce {bound} on a {} step, and it will not run one unbounded",
                        step.kind.word()
                    ),
                ));
            }
            self.admit_bound(step, bound, value)?;
        }
        match step.kind {
            Kind::Query => self.admit_source(step).map(|_| ()),
            Kind::Decide => self.admit_question(step),
            Kind::Check => self.admit_check(program, step),
            Kind::Delegate if !boundary_supported() => Err(Refused::at(
                &step.name,
                "boundary_unavailable",
                "this host has no available filesystem boundary for delegation",
            )),
            _ => Ok(()),
        }
    }

    /// The source a `query` step names, or why this host cannot read it.
    ///
    /// The same shape as a `decide` step's question: the program names a
    /// slug, the host resolves it against its own registry, and a slug it
    /// has no definition for refuses before the first step runs rather
    /// than falling back to whatever work happened to be handed in.
    fn admit_source(&self, step: &Step) -> Result<Source, Refused> {
        let slug = step.source.as_deref().unwrap_or(source::REQUEST);
        self.survey.sources.get(slug).ok_or_else(|| {
            Refused::at(
                &step.name,
                "source_unresolved",
                format!("this host has no source called {slug}, and a query step names one"),
            )
        })
    }

    /// Whether this host can hold to one bound as the program states it.
    ///
    /// A bound key it knows, carrying a value it does not, is a bound it
    /// cannot enforce: `isolation: "vm"` names a shape nobody here can
    /// make, and running the step in a directory instead would be the
    /// substitution the rule forbids.
    fn admit_bound(&self, step: &Step, bound: &str, value: &Value) -> Result<(), Refused> {
        let refuse = |reason: String| Err(Refused::at(&step.name, "bound_unenforceable", reason));
        match bound {
            "isolation" => match value.as_str() {
                Some(word) if self.host.provides(word) => Ok(()),
                Some(word) => refuse(format!(
                    "this host cannot give a delegation a {word} checkout, and it will not run one somewhere else instead"
                )),
                None => refuse(format!(
                    "isolation is named by a word, and this step names {value}"
                )),
            },
            "refuse_on" => match value.as_str() {
                Some(INTERSECTION | "gate_not_met") => Ok(()),
                other => refuse(format!(
                    "this host runs no check that refuses on {}",
                    other.unwrap_or("that")
                )),
            },
            "acceptance" => match value.as_str() {
                Some("exit-success" | "suite") => Ok(()),
                other => refuse(format!(
                    "acceptance names the evidence a gated check requires, and this step names {}",
                    other.unwrap_or("that")
                )),
            },
            "on_overflow" => match value.as_str().and_then(OnOverflow::named) {
                Some(_) => Ok(()),
                None => refuse(format!(
                    "a lookup that answers with more than its bound truncates or refuses, and this step names {value}"
                )),
            },
            "minutes" => match value.as_u64() {
                Some(count)
                    if Bounds::minutes(count).untenable().is_none()
                        && Instant::now()
                            .checked_add(Bounds::minutes(count).wall())
                            .is_some() =>
                {
                    Ok(())
                }
                _ => refuse(format!(
                    "minutes must name a positive deadline this host can represent, and this step names {value}"
                )),
            },
            "depth" | "steps" | "calls" => {
                let ceiling = match bound {
                    "depth" => crate::child::MAX_DEPTH,
                    "steps" => crate::child::MAX_STEPS,
                    _ => crate::child::MAX_CALLS,
                };
                match value.as_u64() {
                    Some(count) if count > 0 && count <= ceiling => Ok(()),
                    _ => refuse(format!(
                        "{bound} is a count between one and {ceiling}, and this step names {value}"
                    )),
                }
            }
            "spend" => match value.as_u64() {
                Some(micros) if micros > 0 => Ok(()),
                _ => refuse(format!(
                    "spend is a ceiling in USD micros above zero, and this step names {value}"
                )),
            },
            "max_results" | "concurrent_max" | "max_tests" => match value.as_u64() {
                Some(count) if count > 0 && usize::try_from(count).is_ok() => Ok(()),
                _ => refuse(format!(
                    "{bound} is a count above zero, and this step names {value}"
                )),
            },
            "refuse_below" => match value.as_f64() {
                Some(floor) if (0.0..=1.0).contains(&floor) => Ok(()),
                _ => refuse(format!(
                    "refuse_below is a probability, and this step names {value}"
                )),
            },
            "requires_scorable_answer" | "per_requirement" | "per_finding" => {
                match value.is_boolean() {
                    true => Ok(()),
                    false => refuse(format!(
                        "{bound} is true or false, and this step names {value}"
                    )),
                }
            }
            // Unreachable: the key was checked against the table above.
            _ => refuse(format!("{bound} is not a bound this host keeps")),
        }
    }

    /// Whether this host can ask what a `decide` step names.
    fn admit_question(&self, step: &Step) -> Result<(), Refused> {
        let refuse = |code: &str, reason: String| Err(Refused::at(&step.name, code, reason));
        let Some(id) = step.question.as_deref() else {
            return refuse(
                "question_missing",
                "a decide step names a question".to_string(),
            );
        };
        let Some(set) = self.questions.get(id) else {
            return refuse(
                "question_unresolved",
                format!("this host has no wording for {id}, and a decide step carries none"),
            );
        };
        let per_requirement = step
            .bounds
            .get("per_requirement")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let per_finding = step
            .bounds
            .get("per_finding")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let wanted = match (per_requirement, per_finding) {
            (true, true) => {
                return refuse(
                    "bound_unenforceable",
                    "a step is asked once per requirement or once per finding, and this step claims both".to_string(),
                );
            }
            (true, false) => Some(questions::Template::Requirement),
            (false, true) => Some(questions::Template::Finding),
            (false, false) => None,
        };
        if wanted != set.template() {
            return refuse(
                "bound_unenforceable",
                match (wanted, set.template()) {
                    (Some(questions::Template::Requirement), _) => format!(
                        "{id} is not asked once per requirement, so this step cannot ask it that way"
                    ),
                    (Some(questions::Template::Finding), _) => format!(
                        "{id} is not asked once per finding, so this step cannot ask it that way"
                    ),
                    (None, Some(questions::Template::Requirement)) => {
                        format!("{id} is asked once per requirement, and this step did not say to")
                    }
                    (None, Some(questions::Template::Finding)) => {
                        format!("{id} is asked once per finding, and this step did not say to")
                    }
                    (None, None) => unreachable!("a set's template is one of these"),
                },
            );
        }
        if per_finding && self.review.is_none() {
            return refuse(
                "review_unavailable",
                "no host-pinned review is installed, and a per-finding step runs against one"
                    .to_string(),
            );
        }
        if set.supplies_options() {
            return refuse(
                "question_unresolved",
                format!(
                    "{id} takes its options from the host, and a step inside a program supplies none"
                ),
            );
        }
        if step.bounds.contains_key("refuse_below") && set.gate.is_empty() {
            return refuse(
                "bound_unenforceable",
                format!("{id} names no question to gate on, so a refuse_below bound reads nothing"),
            );
        }
        if let Some(error) = &self.door_error {
            return refuse("door_configuration", error.clone());
        }
        match self.door.is_some() {
            true => Ok(()),
            false => refuse(
                "door_unavailable",
                format!("no decision door is configured, and {id} has to be asked of one"),
            ),
        }
    }

    /// Whether this host can run what a `check` step names.
    ///
    /// A step gated on `gate_not_met` runs the operator-installed
    /// verification plan rather than anything the program carries, so
    /// admission is where the plan is held to the step: missing evidence
    /// and bounds the plan cannot meet refuse here, before any check runs.
    fn admit_check(&self, program: &Program, step: &Step) -> Result<(), Refused> {
        if step.bounds.get("refuse_on").and_then(Value::as_str) == Some("gate_not_met") {
            let (_, plan, _) = self.verification.as_ref().ok_or_else(|| {
                Refused::at(
                    &step.name,
                    "check_unavailable",
                    "no host-prepared verification plan is installed",
                )
            })?;
            plan.validate()
                .map_err(|reason| Refused::at(&step.name, "bound_unenforceable", reason))?;
            self.admit_evidence(step, plan)?;
            if !boundary_supported() {
                return Err(Refused::at(
                    &step.name,
                    "boundary_unavailable",
                    "verification requires an enforcing filesystem boundary",
                ));
            }
            return Ok(());
        }
        if step.bounds.get("refuse_on").and_then(Value::as_str) != Some(INTERSECTION) {
            return Err(Refused::at(
                &step.name,
                "check_unavailable",
                "this host runs one check, and it is the one refuse_on names".to_string(),
            ));
        }
        if step.bounds.contains_key("acceptance") || step.bounds.contains_key("max_tests") {
            return Err(Refused::at(
                &step.name,
                "bound_unenforceable",
                "acceptance and max_tests bound a host-prepared check, and this check tests a delegation's bounds",
            ));
        }
        match self.next_delegate(program, &step.name).is_some() {
            true => Ok(()),
            false => Err(Refused::at(
                &step.name,
                "bound_unenforceable",
                format!(
                    "{INTERSECTION} tests a delegation against a capability, and no delegate step follows this one"
                ),
            )),
        }
    }

    /// Whether the installed plan can produce the evidence a gated check
    /// requires, before any of it runs.
    ///
    /// `acceptance` is the kind every check in the plan must answer with:
    /// a step that asks for a typed suite cannot be satisfied by an exit
    /// status, and a step that asks for an exit status is not satisfied by
    /// suite evidence it did not ask for. A suite requirement also narrows
    /// the plan to one suite identity, because the step runs *a* suite.
    /// `max_tests` is the most checks the plan may carry under the step.
    fn admit_evidence(&self, step: &Step, plan: &crate::verification::Plan) -> Result<(), Refused> {
        if let Some(required) = step.bounds.get("acceptance").and_then(Value::as_str) {
            let mut suites = BTreeSet::new();
            for check in &plan.checks {
                if check.acceptance.word() != required {
                    return Err(Refused::at(
                        &step.name,
                        "check_unavailable",
                        format!(
                            "this step requires {required} evidence and the installed plan's {} check does not answer with it",
                            check.id
                        ),
                    ));
                }
                if let crate::verification::Acceptance::Suite { suite_digest, .. } =
                    &check.acceptance
                {
                    suites.insert(suite_digest.as_str());
                }
            }
            if suites.len() > 1 {
                return Err(Refused::at(
                    &step.name,
                    "check_unavailable",
                    "the installed plan names more than one suite, and a suite step runs one pinned suite",
                ));
            }
        }
        if let Some(budget) = step.bounds.get("max_tests").and_then(Value::as_u64)
            && plan.checks.len() as u64 > budget
        {
            return Err(Refused::at(
                &step.name,
                "bound_unenforceable",
                format!(
                    "the installed plan runs {} checks, past the test budget of {budget} this step states",
                    plan.checks.len()
                ),
            ));
        }
        Ok(())
    }

    /// The `delegate` step a check admits: the next one after it.
    fn next_delegate<'a>(&self, program: &'a Program, after: &str) -> Option<&'a Step> {
        program
            .steps
            .iter()
            .skip_while(|step| step.name != after)
            .find(|step| step.kind == Kind::Delegate)
    }

    /// The programs this host would offer a request, each with the
    /// summary that describes it.
    ///
    /// The ones it resolved **and would admit**. A program this host
    /// refuses at admission is not a route: offering it puts an option on
    /// the question whose only possible outcome is a refusal, and a
    /// shorter option set is the same answer the capability probe gives
    /// for an executor that is not here. `run-suite` needs a host-prepared
    /// verification plan this runtime does not carry and `review-changes`
    /// requires a host-prepared reviewer and artifact scope. Neither is
    /// offered without its independent host configuration.
    #[must_use]
    pub fn selectable(&self) -> Vec<(String, String)> {
        self.survey
            .programs
            .programs()
            .iter()
            .filter(|program| self.admit(program).is_ok())
            .map(|program| (program.slug.clone(), program.summary.clone()))
            .collect()
    }

    /// Asks which program a request wants, from the ones this host
    /// would run, or none.
    ///
    /// The option set is built from the registry rather than written down,
    /// so an operator whose machine resolved three programs is offered
    /// three. A choice naming nothing the registry holds is refused rather
    /// than guessed at.
    ///
    /// [`Selected::None`] is an answer and not an error. Almost every
    /// request asks for no program, and the option that says so is on the
    /// question rather than in a floor a caller applies to the confidence
    /// afterwards: a model that can only name programs has to name one.
    ///
    /// # Errors
    ///
    /// Returns why the question could not be asked or its answer could not
    /// be read. A host with no programs, no wording, or no door refuses
    /// here, and a caller that meant to run a turn runs it unchanged.
    pub async fn select(
        &self,
        request: &str,
        trace: Option<&mut Recorder>,
    ) -> Result<Selected, Refused> {
        let options = self.selectable();
        if options.is_empty() {
            return Err(Refused::at(
                "",
                "no_programs",
                "this host would run none of the programs it resolved, so there is none to select",
            ));
        }
        let set = self.questions.get(PROGRAM_QUESTION).ok_or_else(|| {
            Refused::at(
                "",
                "question_unresolved",
                format!("this host has no wording for {PROGRAM_QUESTION}"),
            )
        })?;
        let answers = self
            .ask(
                set,
                PROGRAM_CALL,
                &json!({ "request": request }),
                &Fill::Options(options.clone()),
                trace,
                |read| format!("program {read}"),
            )
            .await?;
        let choice = answers
            .answers
            .get(&set.gate)
            .and_then(|answer| match answer {
                Answer::Choice(choice) => Some(choice.choice.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                Refused::at(
                    "",
                    "no_program_chosen",
                    "the door named no program, and this host will not pick one for it",
                )
            })?;
        if choice == NO_PROGRAM {
            return Ok(Selected::None);
        }
        // The answer is held to the option set the question offered: a
        // door naming a program this host resolved but would not run —
        // inadmissible, ungranted in capability, uninstalled — is the
        // same refusal as one naming a program it never heard of, not a
        // selection a later check has to catch.
        match options.iter().any(|(slug, _)| *slug == choice) {
            true => Ok(Selected::Program(choice)),
            false => Err(Refused::at(
                "",
                "no_program_chosen",
                format!("the door named {choice}, which this host did not offer it"),
            )),
        }
    }

    /// Whether the operator's grant covers this program, and why not when
    /// it does not.
    ///
    /// The grant is the operator's, built from the session's settings
    /// before the selection question was asked. A selection is a
    /// proposal — the answer a decision model gave — and this is where
    /// the proposal is held to the authority it was made under. Two
    /// checks, in order: the grant has to name the program, and every
    /// effect a step declares has to be one the grant allows.
    ///
    /// What is not checked here: which items the `query` step's source
    /// will answer with. A work item's `writes` is read at dispatch, off
    /// the item itself, because a source answers after admission ran —
    /// see [`Runtime::delegate`].
    pub fn authorize(
        &self,
        program: &Program,
        inputs: &Inputs,
        grant: &Grant,
    ) -> Result<(), Refused> {
        if !grant.authorizes(&program.slug) {
            return Err(Refused::at(
                "",
                program_authority::UNAUTHORIZED,
                format!(
                    "the operator's grant does not name {}, and a selection is a proposal rather than a grant — {} or --programs says which programs a session may run",
                    program.slug,
                    program_authority::PROGRAMS_ENV
                ),
            ));
        }
        for (step, declared) in self.declared_effects(program, inputs) {
            let missing = grant.missing(declared);
            if !missing.is_empty() {
                return Err(Refused::at(
                    &step.name,
                    program_authority::UNAUTHORIZED,
                    format!(
                        "{} declares {} and this session's grant does not allow it — {} bounds what an authorized program may do",
                        step.name,
                        missing.join(", "),
                        program_authority::EFFECTS_ENV
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Rule one recovered run's unfinished records for a resumer — the
    /// runtime consulting [`crate::reattach`] and [`crate::reconcile`]
    /// over the program the record pinned.
    ///
    /// The declared effects a reconciliation ruling reads come from the
    /// pinned program, resolved in this host's registry by digest or by
    /// the slug an older record kept; a pin that resolves to nothing
    /// declares nothing, and undeclared records rule `NeedsDecision`
    /// rather than guessing. `grant` is the resuming session's own
    /// ceiling — a replay it does not allow rules `OutsideAuthority`
    /// whatever the first session held. `observe` is the host's probe of
    /// each record's dispatch reference; a probe that answers
    /// [`crate::reattach::Observation::Unsupported`] for every reference
    /// rules `NewAttempt` honestly, because that is what an executor
    /// that cannot resume means.
    pub fn rulings(
        &self,
        run: &runstate::Run,
        grant: &Grant,
        observe: impl for<'a> Fn(&str, &'a str) -> crate::reattach::Observation<'a>,
    ) -> Rulings {
        Rulings {
            reattach: crate::reattach::reattachable(run, observe),
            reconcile: crate::reconcile::reconcile(
                run,
                &self.declared_subjects(run),
                grant.effects(),
            ),
        }
    }

    /// The effects each step of a program declares, derived from what the
    /// step does rather than stated in the program — a program's own
    /// words are a claim, and the grant is held against what the step
    /// will do.
    fn declared_effects<'a>(
        &self,
        program: &'a Program,
        inputs: &Inputs,
    ) -> Vec<(&'a Step, Effects)> {
        let mut visiting = vec![program.slug.clone()];
        program
            .steps
            .iter()
            .map(|step| (step, self.step_effects(step, inputs, &mut visiting)))
            .collect()
    }

    /// The effects one step declares, given the executor the run names.
    ///
    /// A `decide` step discloses its state to a decision door and can bill
    /// an account; this host cannot tell a local door from a billed one,
    /// so both axes are declared rather than assumed. A `delegate` step
    /// reads and discloses by construction — the task carries what to
    /// read and everything about it crosses to the executor — and the
    /// executor's manifest adds the rest: `subprocesses` for any
    /// transport that is not the relay, `spend` for any cost that is not
    /// `local`. An executor this host cannot resolve declares the wider
    /// set, because the refusal that names it reads the same survey.
    fn step_effects(&self, step: &Step, inputs: &Inputs, visiting: &mut Vec<String>) -> Effects {
        let mut effects = match step.kind {
            Kind::Query => Effects {
                reads: true,
                ..Effects::none()
            },
            Kind::Decide => Effects {
                network: true,
                spend: true,
                ..Effects::none()
            },
            Kind::Check
                if step.bounds.get("refuse_on").and_then(Value::as_str) == Some("gate_not_met") =>
            {
                Effects {
                    reads: true,
                    network: true,
                    subprocesses: true,
                    spend: true,
                    ..Effects::none()
                }
            }
            Kind::Check => Effects::none(),
            Kind::Delegate => Effects {
                delegation: true,
                reads: true,
                network: true,
                ..Effects::none()
            },
            // A `program` step declares what its resolved child declares:
            // naming a program in the grant consents to the composition it
            // pins, and the effect ceiling is what bounds it. A child that
            // does not resolve declares nothing — admission refused the
            // composition before a grant ever read it — and `visiting`
            // keeps a cycle that slipped admission from recursing.
            Kind::Program => self
                .child_of(step, visiting)
                .map(|child| {
                    visiting.push(child.slug.clone());
                    let union = child.steps.iter().fold(Effects::none(), |union, step| {
                        union.union(self.step_effects(step, inputs, visiting))
                    });
                    visiting.pop();
                    union
                })
                .unwrap_or_else(Effects::none),
            // Refused at admission and never reaches a grant.
            Kind::Module => Effects::none(),
        };
        if step.kind == Kind::Delegate {
            match self.survey.capability(&inputs.executor) {
                Some(found) => {
                    if found.manifest.transport != capability::RELAY {
                        effects.subprocesses = true;
                    }
                    if found.manifest.cost != "local" {
                        effects.spend = true;
                    }
                }
                None => {
                    effects.subprocesses = true;
                    effects.spend = true;
                }
            }
        }
        effects
    }

    /// The program a `program` step's reference resolves to, the same
    /// resolution composition checking and execution use — `visiting`
    /// holds the slugs on the path to the step, so a program that names
    /// an ancestor resolves to nothing rather than recursing.
    fn child_of<'a>(&'a self, step: &Step, visiting: &[String]) -> Option<&'a Program> {
        let reference = crate::child::ChildRef::parse(step.program.as_deref()?).ok()?;
        let resolution = reference.resolve(&self.survey.programs)?;
        if visiting.contains(&resolution.slug) {
            return None;
        }
        self.survey.programs.get(&resolution.slug)
    }

    /// The declared effects of every subject a recovered run can name —
    /// `run`, `step:<name>` namespaced by composition path the way the
    /// record namespaces them, and `task:<name>#<attempt>` — derived from
    /// the program the record pinned, which the claim wrote as a digest
    /// and older records kept as a slug. A task attempt's declaration is
    /// the union of every `delegate` step's effects: the record does not
    /// say which step dispatched it, so the honest bound is any of them.
    /// A pin that names no program this host holds declares nothing, and
    /// undeclared records rule `NeedsDecision` rather than guessing at
    /// what an unknown program did.
    fn declared_subjects(&self, run: &runstate::Run) -> BTreeMap<String, Effects> {
        let mut declared = BTreeMap::new();
        let Some(program) = self.survey.programs.programs().iter().find(|program| {
            crate::child::digest(program) == run.program || program.slug == run.program
        }) else {
            return declared;
        };
        let inputs = Inputs::read("", "");
        let mut delegation = Effects::none();
        let union = self.subject_effects(
            program,
            "",
            &inputs,
            &mut vec![program.slug.clone()],
            &mut declared,
            &mut delegation,
        );
        declared.insert("run".to_string(), union);
        for task in &run.tasks {
            declared
                .entry(format!("task:{}#{}", task.task, task.attempt))
                .or_insert(delegation);
        }
        declared
    }

    /// Walk one program's steps recording each one's declared effects
    /// under its composition path, recursing into `program` steps so a
    /// child's steps rule under names like `step:call/pick`. Returns the
    /// union — what the walked program itself declares.
    fn subject_effects(
        &self,
        program: &Program,
        prefix: &str,
        inputs: &Inputs,
        visiting: &mut Vec<String>,
        declared: &mut BTreeMap<String, Effects>,
        delegation: &mut Effects,
    ) -> Effects {
        let mut union = Effects::none();
        for step in &program.steps {
            let subject = if prefix.is_empty() {
                step.name.clone()
            } else {
                format!("{prefix}/{}", step.name)
            };
            let effects = if step.kind == Kind::Program {
                match self.child_of(step, visiting) {
                    Some(child) => {
                        visiting.push(child.slug.clone());
                        let union = self.subject_effects(
                            child, &subject, inputs, visiting, declared, delegation,
                        );
                        visiting.pop();
                        union
                    }
                    // An unresolvable child declares nothing: the step's
                    // subject stays out of the map and rules `NeedsDecision`.
                    None => continue,
                }
            } else {
                self.step_effects(step, inputs, visiting)
            };
            union = union.union(effects);
            declared.insert(format!("step:{subject}"), effects);
            if step.kind == Kind::Delegate {
                *delegation = delegation.union(effects);
            }
        }
        union
    }

    /// The grant decision, recorded the way every other check is: what
    /// was asked, what it answered, and what the grant and the program
    /// each declared.
    fn record_grant(
        &self,
        program: &Program,
        inputs: &Inputs,
        grant: &Grant,
        refused: Option<&Refused>,
        trace: Option<&mut Recorder>,
    ) {
        let declared: Map<String, Value> = self
            .declared_effects(program, inputs)
            .into_iter()
            .map(|(step, effects)| (step.name.clone(), json!(effects.words())))
            .collect();
        let mut extra = Map::new();
        extra.insert("grant".to_string(), grant.value());
        extra.insert("declared".to_string(), Value::Object(declared));
        if let Some(refused) = refused {
            extra.insert(
                "refused".to_string(),
                json!({ "step": refused.step, "code": refused.code }),
            );
        }
        if !grant.notes().is_empty() {
            extra.insert("notes".to_string(), json!(grant.notes()));
        }
        self.record(
            trace,
            &format!("Held {} to the operator's grant.", program.slug),
            Call {
                id: String::new(),
                name: program_authority::AUTHORITY_CALL.to_string(),
                arguments: json!({
                    "program": program.slug,
                    "executor": inputs.executor,
                }),
                output: match refused {
                    Some(refused) => refused.to_string(),
                    None => "authorized".to_string(),
                },
                outcome: match refused {
                    Some(_) => Outcome::Cancelled,
                    None => Outcome::Completed,
                },
                milliseconds: 0,
                purpose: Some(
                    "Run a selected program only under the authority the operator granted."
                        .to_string(),
                ),
                extra,
            },
        );
    }

    /// Runs one program, step by step, from its definition.
    ///
    /// Admission comes first, so a program this host cannot hold to fails
    /// before it has done anything — and the operator's grant is held
    /// against the program at the same point, so a program nobody
    /// authorized fails there too. After that each step runs in the order
    /// the program lists, and the first refusal stops the rest.
    ///
    /// When the runtime carries a runstate directory — see
    /// [`Runtime::with_runstate`] — the run also claims a recovery record
    /// once admission and the grant have passed, marks each step as it
    /// dispatches and resolves, and settles when the run ends. A run
    /// refused before that point holds no run id, and nothing the store
    /// says changes what the run does.
    ///
    /// When the runtime carries a budget — see [`Runtime::with_budget`]
    /// — each step boundary checks it before the step dispatches, and
    /// the deadline checks again while the step does: a step reached
    /// past the budget never runs, and a deadline expiring mid-step ends
    /// the step's subprocesses through supervise's cancel path. Either
    /// way the step and every step after it mark `cancelled` — the
    /// pending steps of a cancelled `program` step's child too — and the
    /// run settles `cancelled` rather than truncating silently as
    /// `refused`. Cancelled is the end the caller chose; `unknown` stays
    /// what a crash leaves, and a worktree the run claimed stays claimed
    /// under the mark rather than disappearing with it.
    pub async fn run(
        &self,
        program: &Program,
        inputs: &Inputs,
        grant: &Grant,
        mut trace: Option<&mut Recorder>,
    ) -> Run {
        let started = Instant::now();
        let mut run = Run {
            program: Some(program.slug.clone()),
            ..Run::default()
        };
        if let Err(refused) = self.admit(program) {
            run.stopped = Some(refused);
            self.report(&run, started, trace.as_deref_mut());
            return run;
        }
        match self.authorize(program, inputs, grant) {
            Ok(()) => self.record_grant(program, inputs, grant, None, trace.as_deref_mut()),
            Err(refused) => {
                self.record_grant(program, inputs, grant, Some(&refused), trace.as_deref_mut());
                run.stopped = Some(refused);
                self.report(&run, started, trace.as_deref_mut());
                return run;
            }
        }
        // Admission and the grant have passed: the run claims its
        // recovery record before the first step dispatches. A run
        // refused earlier holds no run id, and a store that cannot
        // answer observes the run as nothing — it never gates one.
        let mut record = self.claim_runstate(program, trace.as_deref_mut());
        self.advance_runstate(
            &mut record,
            Mark::run(State::Dispatched),
            trace.as_deref_mut(),
        );
        let mut selection = Selection::default();
        let mut dispatched = 0usize;
        let mut produced = BTreeMap::new();
        let until = self
            .budget
            .and_then(|budget| budget.deadline)
            .and_then(|deadline| started.checked_add(deadline));
        let mut scopes = Vec::new();
        if let Some(bound) = self.budget.and_then(|budget| budget.spend) {
            scopes.push(Scope {
                run: true,
                bound,
                books: BTreeMap::new(),
            });
        }
        let mut steps = StepRun {
            inputs,
            grant,
            until,
            run_until: until,
            selection: &mut selection,
            run: &mut run,
            record: &mut record,
            dispatched: &mut dispatched,
            produced: &mut produced,
            scopes: &mut scopes,
            trace: trace.as_deref_mut(),
        };
        match self.run_steps(&mut steps, program, "", 1).await {
            StepsEnd::Finished => {}
            StepsEnd::Ended(refused) => run.stopped = Some(refused),
            StepsEnd::Cancelled(refused) => {
                self.record_claims(&mut record, &run, trace.as_deref_mut());
                run.stopped = Some(refused);
            }
        }
        if let Some(scope) = scopes.first()
            && scope.run
        {
            run.spend = scope.books.values().cloned().collect();
        }
        self.settle_runstate(&mut record, &run, trace.as_deref_mut());
        self.report(&run, started, trace);
        run
    }

    /// Runs one step list — a program's own steps or a child's — in the
    /// order the program lists them.
    ///
    /// `prefix` namespaces the run's records: a child's steps mark
    /// themselves `parent/child` so a receipt names where inside the
    /// composition the work happened, and `dispatched` counts against the
    /// run's step budget whichever list spent it. The caller's budget
    /// stands at each boundary and the deadline reaches inside each
    /// dispatch: either spending ends the list `cancelled`, the end the
    /// caller chose, never a refusal the step did not give. A step that
    /// refuses ends the list `Ended` and its own mark says how; the steps
    /// after it hold no records because none ran.
    async fn run_steps<'a>(
        &'a self,
        ctx: &'a mut StepRun<'a>,
        program: &'a Program,
        prefix: &'a str,
        depth: u64,
    ) -> StepsEnd {
        for (position, step) in program.steps.iter().enumerate() {
            let name = format!("{prefix}{}", step.name);
            match self.budget_spent(&name, *ctx.dispatched, ctx) {
                Some(Spent::Run(refused)) => {
                    self.cancel_pending(
                        ctx.record,
                        &program.steps,
                        position,
                        prefix,
                        depth,
                        ctx.trace.as_deref_mut(),
                    );
                    return StepsEnd::Cancelled(refused);
                }
                Some(Spent::Child(refused)) => {
                    self.advance_runstate(
                        ctx.record,
                        Mark::step(&name, refusal_state(&refused)),
                        ctx.trace.as_deref_mut(),
                    );
                    self.cancel_pending(
                        ctx.record,
                        &program.steps,
                        position + 1,
                        prefix,
                        depth,
                        ctx.trace.as_deref_mut(),
                    );
                    return StepsEnd::Ended(refused);
                }
                None => {}
            }
            // The spend books answer before the step dispatches: the
            // run's bound spending is the run's cancellation, and a
            // child's declared ceiling failing is the child's refusal —
            // which ends this list so the propagation table decides.
            if let Some((run_scope, refused)) = self.charge(ctx, step, &name) {
                if run_scope {
                    self.cancel_pending(
                        ctx.record,
                        &program.steps,
                        position,
                        prefix,
                        depth,
                        ctx.trace.as_deref_mut(),
                    );
                    return StepsEnd::Cancelled(refused);
                }
                self.advance_runstate(
                    ctx.record,
                    Mark::step(&name, refusal_state(&refused)),
                    ctx.trace.as_deref_mut(),
                );
                return StepsEnd::Ended(refused);
            }
            self.advance_runstate(
                ctx.record,
                Mark::step(&name, State::Dispatched),
                ctx.trace.as_deref_mut(),
            );
            let remaining = Self::remaining(ctx.until);
            let outcome = match step.kind {
                Kind::Query => self
                    .look_up(step, ctx.inputs, ctx.trace.as_deref_mut())
                    .map(|found| {
                        let output = found.output();
                        *ctx.selection = found.clone();
                        ctx.run.selection = Some(found);
                        output
                    }),
                Kind::Decide => {
                    self.within(
                        &name,
                        remaining,
                        self.decide(
                            step,
                            program,
                            ctx.inputs,
                            ctx.selection,
                            ctx.run,
                            ctx.trace.as_deref_mut(),
                        ),
                    )
                    .await
                }
                Kind::Check
                    if step.bounds.get("refuse_on").and_then(Value::as_str)
                        == Some("gate_not_met") =>
                {
                    self.within(
                        &name,
                        remaining,
                        self.verify_step(step, ctx.run, ctx.trace.as_deref_mut()),
                    )
                    .await
                }
                Kind::Check => self.check(step, program, ctx.inputs, ctx.trace.as_deref_mut()),
                Kind::Delegate => {
                    self.delegate(
                        step,
                        ctx.inputs,
                        ctx.selection,
                        ctx.run,
                        ctx.grant,
                        ctx.until,
                        ctx.trace.as_deref_mut(),
                    )
                    .await
                }
                Kind::Program => self.nested(ctx, step, prefix, depth).await,
                // Admission refused these before the first step ran.
                Kind::Module => Err(Refused::at(
                    &name,
                    "step_kind_unavailable",
                    format!("this host does not run a {} step", step.kind.word()),
                )),
            };
            // The deadline reaches inside the step, not only to its
            // boundary: an expiry while the step dispatched ends it.
            // For delegate work the tightened bound already stopped
            // the subprocess group — supervise's own cancel path —
            // and a dispatch `within` timed out was dropped the same
            // way. Whose deadline spent decides the end: the run's own
            // is the caller's cancellation — the step marks `cancelled`,
            // never `refused` for an end it did not give and never
            // `unknown`, which is a crash's mark — and a child's declared
            // `minutes` is the child's refusal, which the propagation
            // table decides.
            if self.deadline_spent(ctx.until)
                || matches!(&outcome, Err(refused) if refused.code == BUDGET_EXCEEDED)
            {
                if ctx.until != ctx.run_until {
                    let refused = Refused::at(
                        &name,
                        "child_bound_spent",
                        "the deadline this step's parent declared for the child passed while the step was dispatched".to_string(),
                    );
                    self.advance_runstate(
                        ctx.record,
                        Mark::step(&name, refusal_state(&refused)),
                        ctx.trace.as_deref_mut(),
                    );
                    self.cancel_pending(
                        ctx.record,
                        &program.steps,
                        position + 1,
                        prefix,
                        depth,
                        ctx.trace.as_deref_mut(),
                    );
                    return StepsEnd::Ended(refused);
                }
                self.cancel_pending(
                    ctx.record,
                    &program.steps,
                    position,
                    prefix,
                    depth,
                    ctx.trace.as_deref_mut(),
                );
                return StepsEnd::Cancelled(Refused::at(
                    &name,
                    BUDGET_EXCEEDED,
                    "the run's deadline passed while the step was dispatched".to_string(),
                ));
            }
            match outcome {
                Ok(output) => {
                    if let Err(refused) = collect_produced(step, &name, &output, ctx.produced) {
                        self.advance_runstate(
                            ctx.record,
                            Mark::step(&name, refusal_state(&refused)),
                            ctx.trace.as_deref_mut(),
                        );
                        return StepsEnd::Ended(refused);
                    }
                    self.advance_runstate(
                        ctx.record,
                        Mark::step(&name, State::Answered),
                        ctx.trace.as_deref_mut(),
                    );
                    ctx.run.steps.push(Ran {
                        name,
                        kind: step.kind,
                        output,
                    });
                    *ctx.dispatched += 1;
                }
                Err(refused) => {
                    self.advance_runstate(
                        ctx.record,
                        Mark::step(&name, refusal_state(&refused)),
                        ctx.trace.as_deref_mut(),
                    );
                    return StepsEnd::Ended(refused);
                }
            }
        }
        StepsEnd::Finished
    }

    /// The child program a `program` step runs, nested inside the
    /// parent's run.
    ///
    /// The composition's shape was checked at admission; what happens
    /// here is only the run half. The child resolves against the same
    /// registry again, runs one level deeper under the run's own budget
    /// and deadline, and its steps mark `step/child` in the run's
    /// records. Its ending is what the step's stated propagation table
    /// maps: `completed`, `failed`, or `refused` lands on this step's
    /// success, failure, or refusal exactly as the document said —
    /// never a default the host chose. A spending of the caller's bound
    /// inside the child is the run's cancellation, not a child outcome
    /// the table maps.
    async fn nested(
        &self,
        ctx: &mut StepRun<'_>,
        step: &Step,
        prefix: &str,
        depth: u64,
    ) -> Result<String, Refused> {
        use crate::child::{Binding, Effect, Outcome, Propagation};
        let name = format!("{prefix}{}", step.name);
        if depth >= crate::child::MAX_DEPTH {
            return Err(Refused::at(
                &name,
                "composition_depth",
                format!(
                    "the composition's depth bound of {} is spent",
                    crate::child::MAX_DEPTH
                ),
            ));
        }
        let Some(child) = self.resolve_child(step) else {
            return Err(Refused::at(
                &name,
                "child_unresolved",
                "the child program this step names resolves to nothing in this host's registry",
            ));
        };
        let binding = Binding::of(step)
            .map_err(|reason| Refused::at(&name, "contract_invalid", reason))?
            .unwrap_or_default();
        let table = match Propagation::of(step) {
            Ok(Some(table)) => table,
            Ok(None) => {
                return Err(Refused::at(
                    &name,
                    "propagation_undeclared",
                    "a program step states its propagation table, and this one states none",
                ));
            }
            Err(reason) => return Err(Refused::at(&name, "contract_invalid", reason)),
        };
        let child_inputs = child_inputs(ctx.inputs, &binding, ctx.produced, prefix, &name)?;
        let child_prefix = format!("{name}/");
        // A `minutes` bound narrows the child's deadline to the tighter
        // of its declaration and what the run has left — never wider.
        let until = [
            ctx.until,
            step.bounds
                .get("minutes")
                .and_then(Value::as_u64)
                .and_then(|count| Instant::now().checked_add(Bounds::minutes(count).wall())),
        ]
        .into_iter()
        .flatten()
        .min();
        // A declared `spend` is the child's hard ceiling, opened against
        // the room the scopes above it still promise. Opening answers
        // before a child step marks: a bound wider than the room, or a
        // ceiling over lanes nobody can price, is the child's refusal —
        // an `Ended` the table below decides, never the run's
        // cancellation.
        let mut pushed = false;
        let mut early = None;
        if let Some(micros) = step.bounds.get("spend").and_then(Value::as_u64) {
            let mut scope = Scope {
                run: false,
                bound: crate::spend::Bound::Hard(micros),
                books: BTreeMap::new(),
            };
            let mut lanes = Vec::new();
            self.spend_lanes(child, &child_prefix, &mut lanes);
            for (step_name, lane) in lanes {
                let room = room_left(ctx.scopes, lane);
                match open_book(scope.bound, lane, room, &step_name) {
                    Ok(book) => {
                        scope.books.insert(lane, book);
                    }
                    Err(refused) => {
                        early = Some(StepsEnd::Ended(refused));
                        break;
                    }
                }
            }
            if early.is_none() {
                ctx.scopes.push(scope);
                pushed = true;
            }
        }
        let end = match early {
            Some(end) => end,
            None => {
                let mut child_ctx = StepRun {
                    inputs: &child_inputs,
                    grant: ctx.grant,
                    until,
                    run_until: ctx.run_until,
                    selection: &mut *ctx.selection,
                    run: &mut *ctx.run,
                    record: &mut *ctx.record,
                    dispatched: &mut *ctx.dispatched,
                    produced: &mut *ctx.produced,
                    scopes: &mut *ctx.scopes,
                    trace: ctx.trace.as_deref_mut(),
                };
                // `run_steps` and `nested` recurse through each other, so
                // this call is boxed to keep the future a size the
                // compiler can name.
                Box::pin(self.run_steps(&mut child_ctx, child, &child_prefix, depth + 1)).await
            }
        };
        if pushed {
            ctx.scopes.pop();
        }
        let (outcome, detail) = match end {
            StepsEnd::Finished => (Outcome::Completed, String::new()),
            // The caller's bound spent inside the child is the run's own
            // end — it propagates, whatever the table says.
            StepsEnd::Cancelled(refused) => return Err(refused),
            // A child whose step refused ends `refused`. `failed` stays
            // for a step kind that can report its work failed without
            // refusing; none does yet, and the table's row is honored
            // when one does.
            StepsEnd::Ended(refused) => (Outcome::Refused, refused.to_string()),
        };
        match outcome.propagation(&table) {
            Effect::Success => {
                let mut output = Map::new();
                output.insert("child".to_string(), Value::String(child.slug.clone()));
                output.insert(
                    "outcome".to_string(),
                    serde_json::to_value(outcome).unwrap_or_default(),
                );
                // The step's declared `produces` resolve from what the
                // child's steps produced — same field name, later steps
                // winning — from a child step's output object when it
                // carries the field, and from this step's own `child`
                // and `outcome`, which it produces by construction. A
                // field nothing produced is the contract the document
                // stated and the run could not meet, which refuses
                // rather than inventing a value.
                for field in binding.produces.keys() {
                    let mut found = None;
                    for child_step in &child.steps {
                        if let Some(value) = ctx
                            .produced
                            .get(&format!("{child_prefix}{}.{field}", child_step.name))
                        {
                            found = Some(value.clone());
                            continue;
                        }
                        if let Some(Value::Object(object)) = ctx
                            .produced
                            .get(&format!("{child_prefix}{}", child_step.name))
                            && let Some(value) = object.get(field)
                        {
                            found = Some(value.clone());
                        }
                    }
                    if found.is_none() {
                        found = match field.as_str() {
                            "child" => Some(Value::String(child.slug.clone())),
                            "outcome" => Some(serde_json::to_value(outcome).unwrap_or_default()),
                            _ => None,
                        };
                    }
                    match found {
                        Some(value) => {
                            ctx.produced
                                .insert(format!("{name}.{field}"), value.clone());
                            output.insert(field.clone(), value);
                        }
                        None => {
                            return Err(Refused::at(
                                &name,
                                "output_unproduced",
                                format!(
                                    "the child program {} produced no field named {field:?}, which this step declares it produces",
                                    child.slug
                                ),
                            ));
                        }
                    }
                }
                Ok(Value::Object(output).to_string())
            }
            Effect::Failure => Err(Refused::at(
                &name,
                "child_failed",
                format!(
                    "the child program {} did not complete: {detail}",
                    child.slug
                ),
            )),
            Effect::Refusal => Err(Refused::at(
                &name,
                "child_refused",
                format!(
                    "the child program {} did not complete: {detail}",
                    child.slug
                ),
            )),
        }
    }

    /// Marks `steps[from..]` and their pending children `cancelled`,
    /// under `prefix` — the marks a list writes when the run's own bound
    /// ends it. A step whose record already resolved keeps it: an
    /// answered child step is never relabelled `cancelled` because a
    /// later bound spent.
    fn cancel_pending(
        &self,
        record: &mut Option<(Store, String)>,
        steps: &[Step],
        from: usize,
        prefix: &str,
        depth: u64,
        mut trace: Option<&mut Recorder>,
    ) {
        for step in steps.iter().skip(from) {
            self.cancel_step(
                record,
                &format!("{prefix}{}", step.name),
                step,
                depth,
                trace.as_deref_mut(),
            );
        }
    }

    /// Whether the run's record already says how `name` ended — a mark
    /// a cancelled run must not overwrite. `dispatched` is not an end:
    /// a step in flight when the bound spent marks `cancelled`.
    fn step_resolved(&self, record: &Option<(Store, String)>, name: &str) -> bool {
        let Some((store, id)) = record else {
            return false;
        };
        store.get(id).is_ok_and(|view| {
            view.is_some_and(|view| {
                view.steps.iter().any(|step| {
                    step.step == name && !matches!(step.state, State::Pending | State::Dispatched)
                })
            })
        })
    }

    /// Claims the run's recovery record, when the operator pointed this
    /// runtime at a runstate directory.
    ///
    /// The claim lands after admission and the grant have passed and
    /// before the first step dispatches, so a run refused earlier holds
    /// no run id. The store observes the run: a store that cannot open,
    /// or a claim it refuses, is noted in the trace and the run goes on
    /// unrecorded rather than not at all.
    fn claim_runstate(
        &self,
        program: &Program,
        trace: Option<&mut Recorder>,
    ) -> Option<(Store, String)> {
        let dir = self.runstate.as_ref()?;
        let mut store = match Store::open(dir) {
            Ok(store) => store,
            Err(trouble) => {
                self.note(
                    trace,
                    &format!("the runstate store did not open: {trouble}"),
                );
                return None;
            }
        };
        let id = run_id(&program.slug);
        let base = self.base_commit();
        let pin = crate::child::digest(program);
        let (questions, sources) = self.claim_pins(program);
        match store.claim(&Claim {
            run: &id,
            base: &base,
            program: &pin,
            questions: &questions,
            sources: &sources,
        }) {
            Ok(_) => Some((store, id)),
            Err(refusal) => {
                self.note(
                    trace,
                    &format!("the runstate store refused the claim: {refusal}"),
                );
                None
            }
        }
    }

    /// Appends one mark to the run's record. A mark the store refuses or
    /// cannot write is noted in the trace — the store observes the run,
    /// and nothing it says changes what the run does.
    fn advance_runstate(
        &self,
        record: &mut Option<(Store, String)>,
        mark: Mark<'_>,
        trace: Option<&mut Recorder>,
    ) {
        let Some((store, id)) = record.as_mut() else {
            return;
        };
        if let Err(refusal) = store.advance(id, mark) {
            self.note(
                trace,
                &format!("the runstate store refused a mark: {refusal}"),
            );
        }
    }

    /// Settles the run's record with what the run came to. The result
    /// reference is where the run's evidence lives — the trace when one
    /// is being kept, the program's slug otherwise — never the evidence
    /// itself.
    fn settle_runstate(
        &self,
        record: &mut Option<(Store, String)>,
        run: &Run,
        trace: Option<&mut Recorder>,
    ) {
        let Some((store, id)) = record.as_mut() else {
            return;
        };
        let outcome = match &run.stopped {
            None => runstate::Outcome::Answered,
            Some(refused) if refused.code == BUDGET_EXCEEDED => runstate::Outcome::Cancelled,
            Some(refused) if unverifiable(&refused.code) => runstate::Outcome::Unverifiable,
            Some(_) => runstate::Outcome::Refused,
        };
        let result = trace
            .as_deref()
            .map(|recorder| recorder.path().to_string_lossy().into_owned())
            .or_else(|| run.program.clone())
            .unwrap_or_default();
        if let Err(refusal) = store.settle(id, outcome, &result) {
            self.note(
                trace,
                &format!("the runstate store refused the settle: {refusal}"),
            );
        }
    }

    /// Whether the caller's budget is spent at a step boundary, and the
    /// stop the run records when it is.
    ///
    /// The boundary is before the step dispatches: `dispatched` counts
    /// the steps already handed out, so a step count of one spends the
    /// budget at the second step, and the deadline compares against when
    /// the run began. A runtime carrying no budget answers `None` at
    /// every boundary — the check is the budget's, not the run's.
    fn budget_spent(&self, step: &str, dispatched: usize, ctx: &StepRun<'_>) -> Option<Spent> {
        if let Some(max) = self.budget.and_then(|budget| budget.max_steps)
            && dispatched >= max
        {
            return Some(Spent::Run(Refused::at(
                step,
                BUDGET_EXCEEDED,
                format!("the run's step budget of {max} is spent"),
            )));
        }
        if ctx.until.is_some_and(|until| Instant::now() >= until) {
            // Whose deadline spent decides the end: the run's own is
            // the caller's cancellation; a `minutes` a `program` step
            // declared is the child's refusal, which its table decides.
            if ctx.until == ctx.run_until {
                return Some(Spent::Run(Refused::at(
                    step,
                    BUDGET_EXCEEDED,
                    "the run's deadline has passed".to_string(),
                )));
            }
            return Some(Spent::Child(Refused::at(
                step,
                "child_bound_spent",
                "the deadline this step's parent declared for the child has passed".to_string(),
            )));
        }
        None
    }

    /// What the effective deadline still leaves, when there is one.
    ///
    /// A step bounds the work it spawns by the tighter of its own bound
    /// and this, so a step never outlives the deadline it runs under.
    /// The answer saturates at zero: a step dispatching past the
    /// deadline hands its work a bound of nothing, and the check after
    /// dispatch ends the step.
    fn remaining(until: Option<Instant>) -> Option<Duration> {
        until.map(|until| until.saturating_duration_since(Instant::now()))
    }

    /// Whether the effective deadline passed while a step dispatched —
    /// the mid-step half of [`Runtime::budget_spent`], which sees only
    /// the boundary.
    fn deadline_spent(&self, until: Option<Instant>) -> bool {
        until.is_some_and(|until| Instant::now() >= until)
    }

    /// Charges the step's priced lane in every spend scope the
    /// composition is inside, innermost first, so a child's ceiling
    /// answers before the run's. The charge books before the work
    /// runs — `Price::Unknown`, because no executor here prices a call
    /// yet, recorded as unknown rather than estimated.
    ///
    /// The answer says whose bound refused: `(true, _)` is the run's
    /// own, the caller's cancellation; `(false, _)` is a child's
    /// declared ceiling, the child's refusal. `None` is nothing owed.
    fn charge(&self, ctx: &mut StepRun<'_>, step: &Step, name: &str) -> Option<(bool, Refused)> {
        let lane = match step.kind {
            Kind::Decide => crate::spend::Lane::Decision,
            Kind::Delegate => crate::spend::Lane::Delegate,
            _ => return None,
        };
        for index in (0..ctx.scopes.len()).rev() {
            if !ctx.scopes[index].books.contains_key(&lane) {
                let room = room_left(&ctx.scopes[..index], lane);
                match open_book(ctx.scopes[index].bound, lane, room, name) {
                    Ok(book) => {
                        ctx.scopes[index].books.insert(lane, book);
                    }
                    Err(refused) => return Some((ctx.scopes[index].run, refused)),
                }
            }
            let Some(book) = ctx.scopes[index].books.get_mut(&lane) else {
                continue;
            };
            match book.charge(crate::spend::Price::Unknown) {
                crate::spend::Charge::Recorded { .. } | crate::spend::Charge::OverSoft { .. } => {}
                crate::spend::Charge::OverBound { bound_micros, .. } => {
                    return Some((
                        ctx.scopes[index].run,
                        Refused::at(
                            name,
                            BUDGET_EXCEEDED,
                            format!("the spend ceiling of {bound_micros} micros is spent"),
                        ),
                    ));
                }
                crate::spend::Charge::Unguaranteeable => {
                    return Some((
                        ctx.scopes[index].run,
                        Refused::at(
                            name,
                            BUDGET_EXCEEDED,
                            "a charge reported no price under a hard spend ceiling".to_string(),
                        ),
                    ));
                }
            }
        }
        None
    }

    /// Runs one step's dispatch under what the run's deadline leaves.
    ///
    /// An expiry while the step awaited its work drops the dispatch —
    /// supervise's own cancel path, since a dropped `Job` still
    /// terminates its process group — and the step comes back as the
    /// budget's end rather than the work's. Cancellation is the caller's
    /// bound reaching inside the step, not a refusal the step gave.
    async fn within(
        &self,
        step: &str,
        remaining: Option<Duration>,
        dispatch: impl Future<Output = Result<String, Refused>>,
    ) -> Result<String, Refused> {
        let Some(remaining) = remaining else {
            return dispatch.await;
        };
        match tokio::time::timeout(remaining, dispatch).await {
            Ok(outcome) => outcome,
            Err(_) => Err(Refused::at(
                step,
                BUDGET_EXCEEDED,
                "the run's deadline passed while the step was dispatched".to_string(),
            )),
        }
    }

    /// Marks a step and every step after it `cancelled` — the end the
    /// caller's bound chose — then records what the run still holds:
    /// each worktree a delegation retained and each delegation that came
    /// back as the harness's rather than the executor's. `run_steps`
    /// writes the same marks through [`Runtime::cancel_pending`]; this
    /// wrapper remains the tests' entry point.
    #[cfg(test)]
    fn cancel_from(
        &self,
        record: &mut Option<(Store, String)>,
        run: &Run,
        program: &Program,
        from: usize,
        mut trace: Option<&mut Recorder>,
    ) {
        self.cancel_pending(record, &program.steps, from, "", 1, trace.as_deref_mut());
        self.record_claims(record, run, trace);
    }

    /// Marks `step` `cancelled` under `name`, then every step of the
    /// child program it names — `name/step` — down to the composition's
    /// own depth bound. A cancelled parent's pending children never ran;
    /// the record says so step by step rather than leaving them for
    /// recovery to guess at. A `program` step whose reference resolves
    /// to nothing marks only itself: there is no child to name. A step
    /// whose record already resolved keeps its mark — an answered child
    /// step is never relabelled `cancelled` because a later bound spent.
    fn cancel_step(
        &self,
        record: &mut Option<(Store, String)>,
        name: &str,
        step: &Step,
        depth: u64,
        mut trace: Option<&mut Recorder>,
    ) {
        if !self.step_resolved(record, name) {
            self.advance_runstate(
                record,
                Mark::step(name, State::Cancelled),
                trace.as_deref_mut(),
            );
        }
        if step.kind != Kind::Program || depth >= crate::child::MAX_DEPTH {
            return;
        }
        let Some(child) = self.resolve_child(step) else {
            return;
        };
        for child_step in &child.steps {
            self.cancel_step(
                record,
                &format!("{name}/{}", child_step.name),
                child_step,
                depth + 1,
                trace.as_deref_mut(),
            );
        }
    }

    /// The child a `program` step's address resolves to in this host's
    /// registry — the binding [`crate::child`] checked before anything
    /// ran, read again so a cancelled parent's record can name the
    /// pending steps its child would have taken.
    fn resolve_child(&self, step: &Step) -> Option<&Program> {
        let reference = crate::child::ChildRef::parse(step.program.as_deref()?).ok()?;
        let resolution = reference.resolve(&self.survey.programs)?;
        self.survey.programs.get(&resolution.slug)
    }

    /// Writes what a cancelled run still owes the record: each worktree
    /// a delegation retained, kept under the cancelled mark so a later
    /// reconciler finds the checkout where the run left it — never
    /// silently removed and never replayed — and each delegation that
    /// came back as the harness's rather than the executor's. A cleanup
    /// failure is its own mark on the task's record: the step's
    /// `cancelled` stays what it is, and the failure is named apart
    /// rather than smudged into it.
    fn record_claims(
        &self,
        record: &mut Option<(Store, String)>,
        run: &Run,
        mut trace: Option<&mut Recorder>,
    ) {
        for (n, delegation) in run.delegations.iter().enumerate() {
            let harness = matches!(delegation.status, Status::Harness(_));
            if !harness && delegation.retained.is_none() {
                continue;
            }
            let state = if harness {
                State::Unverifiable
            } else if delegation.answered() {
                State::Answered
            } else {
                State::Cancelled
            };
            let task = requirement_name(n);
            let mut mark = Mark::task(&task, 1, state);
            if let Some(worktree) = &delegation.retained {
                mark = mark.retaining(worktree.clone());
            }
            self.advance_runstate(record, mark, trace.as_deref_mut());
        }
    }

    /// One delegation's wall bound: the tighter of the step's stated
    /// `minutes` — the task's own bound when the step states none — and
    /// what the run's deadline leaves.
    fn step_bound(
        &self,
        bound: Bounds,
        minutes: Option<u64>,
        remaining: Option<Duration>,
    ) -> Bounds {
        let stated = minutes.map(Bounds::minutes).unwrap_or(bound);
        match remaining {
            Some(remaining) if remaining < stated.wall() => Bounds::within(remaining),
            _ => stated,
        }
    }

    /// What the claim pins: each `decide` step's question set by digest —
    /// by its identifier when this host has no wording for it — and each
    /// `query` step's source by slug. The program and its sources carry
    /// no digests; the question sets do.
    fn claim_pins(&self, program: &Program) -> (Vec<String>, Vec<String>) {
        let mut questions: Vec<String> = Vec::new();
        let mut sources: Vec<String> = Vec::new();
        for step in &program.steps {
            match step.kind {
                Kind::Decide => {
                    let id = step.question.clone().unwrap_or_default();
                    let pin = match self.questions.get(&id) {
                        Some(set) => set.digest(),
                        None => id,
                    };
                    if !questions.contains(&pin) {
                        questions.push(pin);
                    }
                }
                Kind::Query => {
                    let slug = step
                        .source
                        .clone()
                        .unwrap_or_else(|| source::REQUEST.to_string());
                    if !sources.contains(&slug) {
                        sources.push(slug);
                    }
                }
                _ => {}
            }
        }
        (questions, sources)
    }

    /// The commit the run branched from, or empty when this host has no
    /// checkout or the checkout cannot name one.
    fn base_commit(&self) -> String {
        let Some(root) = &self.repository else {
            return String::new();
        };
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(root).args(["rev-parse", "HEAD"]);
        let Ok(output) =
            crate::capability::bounded::run(command, std::time::Duration::from_secs(2))
        else {
            return String::new();
        };
        match output.code == Some(0) && !output.truncated {
            true => output.out.trim().to_string(),
            false => String::new(),
        }
    }

    /// A runstate observation in the trace, when one is being kept.
    fn note(&self, trace: Option<&mut Recorder>, text: &str) {
        if let Some(trace) = trace {
            trace.note(text);
        }
    }

    /// Selects the program a request asks for and runs it, under the
    /// operator's grant.
    ///
    /// A request that asks for no program stops here, reported the way any
    /// other run that did nothing is. A caller that has an ordinary turn to
    /// fall back on wants [`Runtime::select`] instead, so it can tell
    /// `none` from a refusal.
    pub async fn apply(
        &self,
        inputs: &Inputs,
        grant: &Grant,
        mut trace: Option<&mut Recorder>,
    ) -> Run {
        let slug = match self.select(&inputs.request, trace.as_deref_mut()).await {
            Ok(Selected::Program(slug)) => slug,
            Ok(Selected::None) => {
                return Run {
                    stopped: Some(Refused::at(
                        "",
                        "no_program_asked",
                        "the request asks for no program",
                    )),
                    ..Run::default()
                };
            }
            Err(refused) => {
                return Run {
                    stopped: Some(refused),
                    ..Run::default()
                };
            }
        };
        let Some(program) = self.survey.programs.get(&slug).cloned() else {
            return Run {
                stopped: Some(Refused::at(
                    "",
                    "no_program_chosen",
                    format!("{slug} is gone"),
                )),
                ..Run::default()
            };
        };
        self.run(&program, inputs, grant, trace).await
    }

    /// A `query` step: the structured lookup that produces the work.
    ///
    /// The step names a source, the host resolves it, and the answer is
    /// ordered, held to `max_results`, and recorded. A query naming no
    /// source reads the work the request carried, which is a source like
    /// any other and reached through the same code — the first real
    /// burndown runs on work chosen by inspection, and that must exercise
    /// the path a queried list will later take.
    ///
    /// `max_results` is applied rather than described: a lookup that
    /// answered with more than the step allows is a step that ran
    /// unbounded. `on_overflow` says what applying it does, and the trace
    /// records which happened along with everything that was left out.
    fn look_up(
        &self,
        step: &Step,
        inputs: &Inputs,
        trace: Option<&mut Recorder>,
    ) -> Result<Selection, Refused> {
        let source = self.admit_source(step)?;
        let found = source
            .read(&self.survey.workspace, &inputs.tasks)
            .map_err(|reason| Refused::at(&step.name, "source_unreadable", reason))?;
        if found.is_empty() {
            return Err(Refused::at(
                &step.name,
                "no_tasks",
                format!(
                    "{} answered with no work, and a fan-out over nothing is not a fan-out",
                    source.slug
                ),
            ));
        }
        let max = step
            .bounds
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX) as usize;
        let on_overflow = step
            .bounds
            .get("on_overflow")
            .and_then(Value::as_str)
            .and_then(OnOverflow::named)
            .unwrap_or_default();
        let selection = Selection::of(&source, found, max, on_overflow);

        let mut extra = self.step_extra(step);
        extra.insert("source".to_string(), json!(selection.source));
        extra.insert("resolved_from".to_string(), json!(selection.resolved_from));
        extra.insert("order".to_string(), json!(selection.order.word()));
        extra.insert("ordered".to_string(), json!(selection.ordered));
        extra.insert("found".to_string(), json!(selection.found));
        extra.insert("selected".to_string(), json!(selection.selected()));
        extra.insert("overflow".to_string(), json!(selection.overflow.word()));
        extra.insert("dropped".to_string(), selection.dropped_value());
        if !selection.collisions.is_empty() {
            extra.insert("collisions".to_string(), selection.collisions_value());
        }
        extra.insert(
            "tasks".to_string(),
            json!(
                selection
                    .work
                    .iter()
                    .map(source::Work::value)
                    .collect::<Vec<_>>()
            ),
        );
        let refused = selection.overflow == Overflow::Refused;
        self.record(
            trace,
            &format!(
                "Looked {} up from {}: {}.",
                selection.source,
                selection.resolved_from,
                selection.output()
            ),
            Call {
                id: String::new(),
                name: SELECT_CALL.to_string(),
                arguments: json!({
                    "source": selection.source,
                    "from": selection.resolved_from,
                    "order": selection.order.word(),
                    "max_results": max,
                    "on_overflow": on_overflow.word(),
                }),
                output: selection.output(),
                outcome: match refused {
                    true => Outcome::Cancelled,
                    false => Outcome::Completed,
                },
                milliseconds: 0,
                purpose: Some(
                    "Find the work before asking whether it may run at once.".to_string(),
                ),
                extra,
            },
        );
        if refused {
            return Err(Refused::at(
                &step.name,
                "too_many_results",
                format!(
                    "{} answered with {} work items against a max_results of {max}, and this step refuses rather than choosing {max} of them",
                    selection.source,
                    selection.work.len()
                ),
            ));
        }
        if selection.is_empty() {
            return Err(Refused::at(
                &step.name,
                "no_tasks",
                format!(
                    "every one of {}'s {} work items comes after work in the same list, so none of them can run beside the others",
                    selection.source, selection.found
                ),
            ));
        }
        Ok(selection)
    }

    /// A `decide` step: one typed question set put to a decision door.
    async fn decide(
        &self,
        step: &Step,
        program: &Program,
        inputs: &Inputs,
        selection: &Selection,
        run: &mut Run,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        // Admission established both of these.
        let id = step.question.clone().unwrap_or_default();
        let set = self.questions.get(&id).expect("admission resolved the set");
        if step.bounds.get("per_finding").and_then(Value::as_bool) == Some(true) {
            return self.review_findings(step, set, run, trace).await;
        }
        let (state, fill) = match set.templated() {
            true => {
                let requirements = requirement_names(run);
                (
                    requirements_state(run, selection),
                    Fill::Requirements(requirements),
                )
            }
            false => (plan_state(program, inputs, selection), Fill::None),
        };
        let floor = step.bounds.get("refuse_below").and_then(Value::as_f64);
        let gate = set.gate.clone();
        let route = move |answer: String| match floor {
            Some(floor) => format!("{answer}, against a floor of {floor}"),
            None => answer,
        };
        let response = self
            .ask(set, &step.name, &state, &fill, trace, route)
            .await?;
        run.answers
            .insert(step.name.clone(), answers_value(&response.answers));

        // Scoreability means a probability from a named model, beside the
        // recorded question digest. It does not establish calibration or
        // authorize a model for this workload.
        if step
            .bounds
            .get("requires_scorable_answer")
            .and_then(Value::as_bool)
            == Some(true)
        {
            let scorable = !gate.is_empty()
                && response.answers.get(&gate).and_then(probability).is_some()
                && !response.model.is_empty();
            if !scorable {
                return Err(Refused::at(
                    &step.name,
                    "unscorable_answer",
                    format!(
                        "{id} must answer with a probability from a named model to be scored against an outcome, and this answer cannot be"
                    ),
                ));
            }
        }
        let Some(floor) = floor else {
            // A per-requirement step is the acceptance: what it comes to
            // is each work item's verdict against the answer it stated,
            // not the count of questions the door answered.
            if set.templated() {
                return Ok(run.tally().to_string());
            }
            return Ok(format!("{} answers", response.answers.len()));
        };
        let Some(read) = response.answers.get(&gate).and_then(probability) else {
            return Err(Refused::at(
                &step.name,
                "gate_unanswered",
                format!("{gate} went unanswered, and a floor of {floor} reads nothing"),
            ));
        };
        if read < floor {
            return Err(Refused::at(
                &step.name,
                "below_floor",
                format!("{gate} came back at {read:.2}, below the {floor} this step refuses under"),
            ));
        }
        Ok(format!("{gate} {read:.2} clears {floor}"))
    }

    /// A `per_finding` decide step: collect the pinned reviewer's evidence,
    /// then judge each anchored finding against it.
    ///
    /// The reviewer is a checker capability, run here rather than before
    /// the program so a mechanical gate that fails first never pays for a
    /// review the run cannot use. The evidence is recorded either way, and
    /// an outcome that is not `answered` refuses the step — refused,
    /// failed, and unverifiable are kept apart, and none of them reads as
    /// a review that found nothing.
    ///
    /// Findings the host anchored as `excluded` or `unanchored` are never
    /// asked about and never confirmed: their dispositions are mechanical
    /// facts about the captured diff, which a door cannot overturn. Each
    /// anchored finding's raw probability stays beside the disposition the
    /// operator's policy gave it; a probability between the thresholds is
    /// `unresolved`, not rounded.
    async fn review_findings(
        &self,
        step: &Step,
        set: &Set,
        run: &mut Run,
        mut trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        let context = self
            .review
            .as_ref()
            .expect("admission installed the review");
        let evidence = crate::review::collect(context)
            .await
            .map_err(|reason| Refused::at(&step.name, "review_unverifiable", reason))?;
        let outcome = evidence.outcome;
        let mut extra = self.step_extra(step);
        extra.insert("review".to_string(), json!(&evidence));
        self.record(
            trace.as_deref_mut(),
            "Collected the pinned reviewer's findings.",
            Call {
                id: String::new(),
                name: crate::review::REVIEW_CALL.into(),
                arguments: json!({
                    "base": context.scope.base,
                    "tip": context.scope.tip,
                    "input_digest": context.scope.input_digest,
                    "diff_digest": context.scope.diff_digest,
                }),
                output: format!("{}: {}", outcome.word(), evidence.reason),
                outcome: match outcome {
                    crate::review::Outcome::Answered => Outcome::Completed,
                    _ => Outcome::Cancelled,
                },
                milliseconds: 0,
                purpose: Some(
                    "Run the approved reviewer over the read-only candidate and anchor what it reports."
                        .to_string(),
                ),
                extra,
            },
        );
        if outcome != crate::review::Outcome::Answered {
            run.review = Some(crate::review::Reviewed::unreviewed(evidence));
            return Err(Refused::at(
                &step.name,
                outcome.refusal(),
                "the reviewer produced no usable evidence, and missing evidence never passes",
            ));
        }
        let mut judged: Vec<crate::review::Judged> = evidence
            .findings
            .iter()
            .map(|anchored| crate::review::Judged {
                id: anchored.id.clone(),
                anchor: anchored.anchor,
                disposition: match anchored.anchor {
                    crate::review::Anchor::Anchored => crate::review::Disposition::Unanswered,
                    crate::review::Anchor::Excluded => crate::review::Disposition::Excluded,
                    crate::review::Anchor::Unanchored => crate::review::Disposition::Unanchored,
                },
                probability: None,
                finding: anchored.finding.clone(),
            })
            .collect();
        let askable: Vec<crate::review::AnchoredFinding> = evidence
            .findings
            .iter()
            .filter(|finding| finding.anchor == crate::review::Anchor::Anchored)
            .cloned()
            .collect();
        if askable.is_empty() {
            let reviewed = crate::review::Reviewed {
                asked: 0,
                confirmed: 0,
                dismissed: 0,
                unresolved: 0,
                unanswered: 0,
                model: String::new(),
                findings: judged,
                evidence,
            };
            let output = format!(
                "0 of {} findings anchored for review",
                reviewed.findings.len()
            );
            run.review = Some(reviewed);
            return Ok(output);
        }
        let ids: Vec<String> = askable.iter().map(|finding| finding.id.clone()).collect();
        let state = context.state(&askable);
        let response = match self
            .ask(
                set,
                &step.name,
                &state,
                &Fill::Findings(ids),
                trace,
                |read| read,
            )
            .await
        {
            Ok(response) => response,
            Err(refused) => {
                run.review = Some(crate::review::Reviewed {
                    asked: askable.len(),
                    confirmed: 0,
                    dismissed: 0,
                    unresolved: 0,
                    unanswered: askable.len(),
                    model: String::new(),
                    findings: judged,
                    evidence,
                });
                return Err(refused);
            }
        };
        run.answers
            .insert(step.name.clone(), answers_value(&response.answers));
        for finding in judged
            .iter_mut()
            .filter(|finding| finding.anchor == crate::review::Anchor::Anchored)
        {
            match response.answers.get(&finding.id).and_then(probability) {
                Some(read) => {
                    finding.probability = Some(read);
                    finding.disposition = context.policy.judge(read);
                }
                None => finding.disposition = crate::review::Disposition::Unanswered,
            }
        }
        let reviewed = crate::review::Reviewed {
            asked: askable.len(),
            confirmed: judged
                .iter()
                .filter(|f| f.disposition == crate::review::Disposition::Confirmed)
                .count(),
            dismissed: judged
                .iter()
                .filter(|f| f.disposition == crate::review::Disposition::Dismissed)
                .count(),
            unresolved: judged
                .iter()
                .filter(|f| f.disposition == crate::review::Disposition::Unresolved)
                .count(),
            unanswered: judged
                .iter()
                .filter(|f| f.disposition == crate::review::Disposition::Unanswered)
                .count(),
            model: response.model.clone(),
            findings: judged,
            evidence,
        };
        let output = reviewed.output();
        run.review = Some(reviewed);
        Ok(output)
    }

    async fn verify_step(
        &self,
        step: &Step,
        run: &mut Run,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        let (workspace, plan, trust) = self
            .verification
            .as_ref()
            .expect("admission resolved verification");
        let report = crate::verification::run(workspace, plan, trust)
            .await
            .map_err(|reason| Refused::at(&step.name, "verification_unverifiable", reason))?;
        let passed = report.verdict == crate::verification::Verdict::Passed;
        let output = format!(
            "verification {:?}: {} bounded checks",
            report.verdict,
            report.checks.len()
        );
        let mut extra = self.step_extra(step);
        extra.insert("verification".into(), json!(report));
        self.record(
            trace,
            "Checked independent host evidence.",
            Call {
                id: String::new(),
                name: "program_verification".into(),
                arguments: json!({"plan_digest":plan.digest(),"input_digest":plan.input_digest}),
                output: output.clone(),
                outcome: if passed {
                    Outcome::Completed
                } else {
                    Outcome::Cancelled
                },
                milliseconds: 0,
                purpose: Some("Require pinned mechanical evidence before accepting work.".into()),
                extra,
            },
        );
        run.verification.push(report);
        if passed {
            Ok(output)
        } else {
            Err(Refused::at(&step.name, "gate_not_met", output))
        }
    }

    /// A `check` step: the deterministic admission test.
    ///
    /// The test is who holds the delegation to each bound it names: this
    /// host, the executor, or nobody. An executor that ignores a bound is
    /// more dangerous than one that refuses it, so a declared
    /// `cannot_enforce` refuses the pairing rather than issuing it and
    /// hoping — and so does a bound **neither** list mentions, because a
    /// requirement nobody has claimed is not met by having gone
    /// unmentioned.
    ///
    /// Refusing on the second case is stricter than the `refuse_on` bound
    /// names, which is always allowed: a host never has to run a step, and
    /// NIP-PRG's general rule is that a bound it cannot establish refuses.
    fn check(
        &self,
        step: &Step,
        program: &Program,
        inputs: &Inputs,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        let delegate = self
            .next_delegate(program, &step.name)
            .expect("admission found the delegate step");
        let Some(found) = self.survey.capability(&inputs.executor) else {
            return Err(Refused::at(
                &step.name,
                "capability_undeclared",
                format!("{} is not a capability this host declares", inputs.executor),
            ));
        };
        let manifest = &found.manifest;
        let mut held: BTreeMap<String, Enforcement> = BTreeMap::new();
        for bound in delegate.bounds.keys() {
            let by = match bound.as_str() {
                bound if HOST_BOUNDS.contains(&bound) => Enforcement::Host,
                bound if manifest.cannot_enforce.iter().any(|named| named == bound) => {
                    Enforcement::Ignored
                }
                _ => Enforcement::Unknown,
            };
            held.insert(bound.clone(), by);
        }
        let named = |state: Enforcement| -> Vec<String> {
            held.iter()
                .filter(|(_, by)| **by == state)
                .map(|(bound, _)| bound.clone())
                .collect()
        };
        let required: Vec<String> = delegate
            .bounds
            .keys()
            .filter(|bound| !HOST_BOUNDS.contains(&bound.as_str()))
            .cloned()
            .collect();
        let ignored = named(Enforcement::Ignored);
        let unknown = named(Enforcement::Unknown);
        let admitted = ignored.is_empty() && unknown.is_empty();
        let output = match (ignored.is_empty(), unknown.is_empty()) {
            (true, true) => format!(
                "admitted: {{{}}} kept by {}, {{{}}} kept by the host",
                named(Enforcement::Executor).join(", "),
                inputs.executor,
                named(Enforcement::Host).join(", "),
            ),
            (false, _) => format!(
                "refused: {} declares it will silently ignore {{{}}}",
                inputs.executor,
                ignored.join(", ")
            ),
            (true, false) => format!(
                "refused: enforcement of {{{}}} by {} is unverified",
                unknown.join(", "),
                inputs.executor
            ),
        };
        let mut extra = self.step_extra(step);
        extra.insert("admitted".to_string(), json!(admitted));
        extra.insert("required".to_string(), json!(required));
        extra.insert("cannot_enforce".to_string(), json!(manifest.cannot_enforce));
        extra.insert(
            "enforcement".to_string(),
            json!(
                held.iter()
                    .map(|(bound, by)| (bound.clone(), json!(by.word())))
                    .collect::<Map<String, Value>>()
            ),
        );
        if !ignored.is_empty() {
            extra.insert("ignored".to_string(), json!(ignored));
        }
        if !unknown.is_empty() {
            extra.insert("unknown".to_string(), json!(unknown));
        }
        self.record(
            trace,
            &format!(
                "Checked the delegation against what {} will hold to.",
                inputs.executor
            ),
            Call {
                id: String::new(),
                name: ADMISSION_CALL.to_string(),
                arguments: json!({
                    "capability": inputs.executor,
                    "required_bounds": required,
                }),
                output: output.clone(),
                outcome: match admitted {
                    true => Outcome::Completed,
                    false => Outcome::Cancelled,
                },
                milliseconds: 0,
                purpose: Some(
                    "Refuse a delegation under a bound nobody is holding it to.".to_string(),
                ),
                extra,
            },
        );
        if admitted {
            return Ok(output);
        }
        let code = match ignored.is_empty() {
            true => UNKNOWN,
            false => INTERSECTION,
        };
        Err(Refused::at(&step.name, code, output))
    }

    /// A `delegate` step: the work, handed over under the step's bounds.
    #[allow(clippy::too_many_arguments)]
    async fn delegate(
        &self,
        step: &Step,
        inputs: &Inputs,
        selection: &Selection,
        run: &mut Run,
        grant: &Grant,
        until: Option<Instant>,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        // A step that hands over nothing did not run: it reported "0 of 0
        // answered" and the program carried on. That is the shape a
        // wrongly selected program takes when the request listed no work,
        // and reporting it as a step that ran is how a turn answers with a
        // summary of nothing.
        if selection.is_empty() {
            return Err(Refused::at(
                &step.name,
                "no_tasks",
                "there is no work to hand over, and a delegation of nothing is not a delegation",
            ));
        }
        // A work item's `writes` is read off the item itself, here at
        // dispatch rather than at admission, because a `query` step's
        // source answers after admission ran. The work list is data and
        // data cannot widen the grant: an item that says it writes under
        // a grant that does not allow writes refuses the step rather than
        // handing over what it was never authorized to do.
        if !grant.effects().writes
            && let Some(work) = selection.work.iter().find(|work| work.task.writes)
        {
            return Err(Refused::at(
                &step.name,
                program_authority::UNAUTHORIZED,
                format!(
                    "work item {} declares it writes, and this session's grant does not allow writes",
                    work.id
                ),
            ));
        }
        let relayed = self
            .survey
            .capability(&inputs.executor)
            .filter(|found| found.manifest.transport == capability::RELAY)
            .filter(|found| matches!(found.presence, Presence::Present { .. }))
            .and(self.relay.as_ref());
        let executor = match (relayed, self.survey.executor(&inputs.executor)) {
            (Some(_), _) => None,
            (None, Some(executor)) => Some(executor),
            (None, None) => {
                let state = self
                    .survey
                    .capability(&inputs.executor)
                    .map_or("undeclared".to_string(), |found| {
                        found.presence.state().to_string()
                    });
                return Err(Refused::at(
                    &step.name,
                    "executor_unavailable",
                    format!(
                        "{} is {state} here, and a route that cannot be taken is not a route",
                        inputs.executor
                    ),
                ));
            }
        };
        // Every one of these was admitted. `isolation` is a shape this
        // host provides, `concurrent_max` is the width, and `minutes` is
        // the wall bound each delegation runs under.
        let isolation = step
            .bounds
            .get("isolation")
            .and_then(Value::as_str)
            .and_then(Isolation::named)
            .unwrap_or(Isolation::Directory);
        let width = step
            .bounds
            .get("concurrent_max")
            .and_then(Value::as_u64)
            .map(|count| usize::try_from(count).expect("admission checked the concurrency bound"))
            .unwrap_or(1);
        let minutes = step.bounds.get("minutes").and_then(Value::as_u64);
        // A step's `briefing` is what every delegate is told before its
        // item: the shape of the place it runs in, which the program knows
        // and the work list does not. The burn-down's says the common Git
        // directory is sealed and where a commit goes instead.
        let briefing = step
            .rest
            .get(BRIEFING)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|briefing| !briefing.is_empty());
        // Each delegation's wall is the tighter of the step's stated
        // bound and what the deadline it runs under leaves: supervise
        // holds the bound against the process group, so threading the
        // remaining budget into it is what stops the step's subprocesses
        // when the bound expires mid-step.
        let remaining = Self::remaining(until);
        let bounded: Vec<Task> = selection
            .tasks()
            .into_iter()
            .map(|task| {
                let mut task = task;
                task.isolation = isolation;
                task.bounds = self.step_bound(task.bounds, minutes, remaining);
                if let Some(briefing) = briefing {
                    task.prompt = format!("{briefing}\n\n{}", task.prompt);
                }
                task
            })
            .collect();
        // A relay capability runs nothing here: each task becomes one
        // NIP-CJ job to the worker, which applies its own approval.
        let delegations = match (relayed, executor) {
            (Some(door), _) => door.fan_out(&inputs.executor, bounded, width).await,
            (None, Some(executor)) => {
                Delegator::new(executor)
                    .in_repository(
                        self.repository
                            .clone()
                            .unwrap_or(self.survey.workspace.clone()),
                    )
                    .bounded_to(width)
                    .fan_out(bounded)
                    .await
            }
            (None, None) => unreachable!("one of the two routes was resolved above"),
        };
        if let Some(trace) = trace {
            for delegation in &delegations {
                trace.delegation(delegation);
            }
        }
        let started = delegations.len();
        let answered = delegations
            .iter()
            .filter(|delegation| delegation.answered())
            .count();
        run.delegations.extend(delegations);
        Ok(format!(
            "{answered} of {started} answered at a width of {width}"
        ))
    }

    /// Puts one question set to the door and records the call.
    async fn ask(
        &self,
        set: &Set,
        name: &str,
        state: &Value,
        fill: &Fill,
        trace: Option<&mut Recorder>,
        route: impl FnOnce(String) -> String,
    ) -> Result<jev::SystemOneResponse, Refused> {
        let door = self.door.as_ref().ok_or_else(|| {
            // A resolution that failed keeps its reason: the refusal a
            // caller surfaces is the one the resolver named, not a
            // quieter "nothing configured".
            Refused::at(
                name,
                "door_unavailable",
                self.door_error
                    .clone()
                    .unwrap_or_else(|| "no decision door is configured".to_string()),
            )
        })?;
        // The set's policy binds before anything goes out: a state
        // bigger than the function declares, or a requested model the
        // function does not admit, refuses rather than asking.
        if let Some(max) = set.policy.state_max_bytes {
            let size = state.to_string().len() as u64;
            if size > max {
                return Err(Refused::at(
                    name,
                    "state_over_bound",
                    format!(
                        "{} asks over {size} bytes of state and its policy admits {max}",
                        set.id
                    ),
                ));
            }
        }
        if !set.policy.models.is_empty()
            && !set
                .policy
                .models
                .iter()
                .any(|model| model == door.default_model())
        {
            return Err(Refused::at(
                name,
                "unbound_model",
                format!(
                    "{} admits answers from [{}] and this door requests {}",
                    set.id,
                    set.policy.models.join(", "),
                    door.default_model()
                ),
            ));
        }
        let questions = set
            .build(fill)
            .map_err(|message| Refused::at(name, "question_invalid", message))?;
        let request = SystemOneRequest::new(state.clone(), questions);
        // The body is what goes on the wire; reading it here is what a
        // recorded exchange means.
        let asked = request
            .body(door.default_model())
            .map_or(Value::Null, Value::Object);
        let started = Instant::now();
        let answered = door.system_one(request).await;
        let milliseconds = started.elapsed().as_millis() as u64;
        let mut decision = Decision {
            id: String::new(),
            name: name.to_string(),
            door: door.base_url().to_string(),
            model: door.default_model().to_string(),
            request: asked,
            answers: Value::Null,
            route: None,
            error: None,
            milliseconds,
        };
        match answered {
            Ok(response) => {
                decision.model = response.model.clone();
                decision.answers = answers_value(&response.answers);
                decision.route = Some(route(read_of(&response, &set.gate)));
                // The policy binds what came back, too: an answer
                // reporting a model the function does not admit, or a
                // gated confidence under its abstention floor, is the
                // typed outcome — not a read a caller treats as an
                // answer.
                let refused = if !set.policy.models.is_empty()
                    && !set.policy.models.contains(&response.model)
                {
                    Some(Refused::at(
                        name,
                        "unbound_model",
                        format!(
                            "{} admits answers from [{}] and this answer came from {}",
                            set.id,
                            set.policy.models.join(", "),
                            response.model
                        ),
                    ))
                } else {
                    set.policy.abstain_below.and_then(|floor| {
                        response
                            .answers
                            .get(&set.gate)
                            .and_then(probability)
                            .filter(|read| *read < floor)
                            .map(|read| {
                                Refused::at(
                                    name,
                                    "abstained",
                                    format!(
                                        "{} abstains: {} came back at {read:.2}, under the policy's {floor}",
                                        set.id, set.gate
                                    ),
                                )
                            })
                    })
                };
                if let Some(refused) = &refused {
                    decision.error = Some(refused.to_string());
                }
                self.record_decision(trace, set, decision);
                match refused {
                    Some(refused) => Err(refused),
                    None => Ok(response),
                }
            }
            Err(error) => {
                decision.error = Some(error.to_string());
                self.record_decision(trace, set, decision);
                Err(door_refused(name, &error))
            }
        }
    }

    /// Writes one decision to the trace, with the set it asked from.
    fn record_decision(&self, trace: Option<&mut Recorder>, set: &Set, decision: Decision) {
        let Some(trace) = trace else { return };
        let mut call = decision.call();
        if let Some(provenance) = set.provenance().as_object() {
            call.extra.extend(provenance.clone());
        }
        // A decision reaches the trace as a decision. `Recorder::decision`
        // takes the typed value, so the provenance is folded into the call
        // it builds rather than around it.
        trace.decision_call(call);
    }

    /// Writes one deterministic call to the trace.
    fn record(&self, trace: Option<&mut Recorder>, message: &str, call: Call) {
        if let Some(trace) = trace {
            trace.check(message, call);
        }
    }

    /// What every recorded step of a run carries: which program, which
    /// step, and what bounds it ran under.
    fn step_extra(&self, step: &Step) -> Map<String, Value> {
        let mut extra = Map::new();
        extra.insert("schema".to_string(), json!(STEP_SCHEMA));
        extra.insert("step".to_string(), json!(step.name));
        extra.insert("kind".to_string(), json!(step.kind.word()));
        extra.insert("bounds".to_string(), json!(step.bounds));
        extra
    }

    /// The sentence a run ends on.
    fn report(&self, run: &Run, started: Instant, trace: Option<&mut Recorder>) {
        let Some(trace) = trace else { return };
        trace.answer(
            &run.summary(),
            None,
            started.elapsed().as_millis() as u64,
            None,
        );
    }
}

/// The identifier one program run is claimed under: the program's slug,
/// the millisecond it started, and the process it runs in. The id is
/// also the record file's name, so anything outside its charset —
/// letters, digits, `-`, `_`, `.` — becomes a `-`.
fn run_id(slug: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    format!("run-{slug}-{millis}-{pid}")
        .chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                true => c,
                false => '-',
            },
        )
        .collect()
}

/// Whether a refusal code names work that produced nothing checkable —
/// the `*_unverifiable` codes — rather than work that declined.
fn unverifiable(code: &str) -> bool {
    code.ends_with("unverifiable")
}

/// The record state a stopped step's refusal maps to: `unverifiable`
/// when the refusal names work that produced nothing anyone could
/// check, `refused` otherwise.
fn refusal_state(refused: &Refused) -> State {
    match unverifiable(&refused.code) {
        true => State::Unverifiable,
        false => State::Refused,
    }
}

/// A decision door's failure as a step's refusal. Authorization and
/// quota name themselves rather than wearing `door_unavailable`: a key
/// the door stopped accepting (`unauthenticated`) and a door the key
/// no longer reaches (`unauthorized`) are the revocation and
/// changed-access signals #9502 keeps distinct from transport failure,
/// and a rate limit is quota exhaustion, not absence. Every other
/// failure — transport, timeout, an unreadable answer — is
/// `door_unavailable` as before.
fn door_refused(step: &str, error: &jev::Error) -> Refused {
    let code = match error {
        jev::Error::Api(api) => match api.kind {
            jev::ApiErrorKind::Authentication => "door_unauthenticated",
            jev::ApiErrorKind::PermissionDenied => "door_unauthorized",
            jev::ApiErrorKind::RateLimit { .. } => "door_rate_limited",
            _ => "door_unavailable",
        },
        _ => "door_unavailable",
    };
    Refused::at(step, code, error.to_string())
}

/// The tightest room the enclosing scopes still promise one lane — the
/// smallest ceiling-minus-held among them, `None` when none of them
/// states a ceiling.
fn room_left(scopes: &[Scope], lane: crate::spend::Lane) -> Option<u64> {
    scopes.iter().filter_map(|scope| scope.room(lane)).min()
}

/// Opens one lane's book under a scope's bound, checked against the
/// room the enclosing scopes have left.
///
/// Asking for more than the room is the widening the composition
/// forbids — refused, never silently clamped — and a hard ceiling over
/// a lane nobody can price is refused for what it is: a guarantee the
/// host cannot give. `step` is who the refusal names.
fn open_book(
    bound: crate::spend::Bound,
    lane: crate::spend::Lane,
    room: Option<u64>,
    step: &str,
) -> Result<crate::spend::Book, Refused> {
    if let (Some(room), Some(ask)) = (room, bound.ceiling())
        && ask > room
    {
        return Err(Refused::at(
            step,
            "bound_widens",
            format!(
                "a spend ceiling of {ask} micros is wider than the {room} the composition has left"
            ),
        ));
    }
    crate::spend::Book::open(lane, bound, false).map_err(|_| {
        Refused::at(
            step,
            "spend_unguaranteeable",
            format!(
                "a spend ceiling is a hard bound and this host cannot price {} work, so it cannot hold one",
                lane.name()
            ),
        )
    })
}

/// Records one answered step's output for the steps that read it later:
/// the whole output under the step's `parent/child` name, and each field
/// its binding declares under `name.field`.
///
/// A step that declares `produces` and answers with something that is
/// not an object carrying those fields is a contract the run could not
/// meet — it refuses rather than a later step reading a gap as an
/// answer. A `program` step's fields were resolved by its child before
/// its output was written; they are already in the map.
fn collect_produced(
    step: &Step,
    name: &str,
    output: &str,
    produced: &mut BTreeMap<String, Value>,
) -> Result<(), Refused> {
    produced.insert(
        name.to_string(),
        serde_json::from_str(output).unwrap_or_else(|_| Value::String(output.to_string())),
    );
    let binding = crate::child::Binding::of(step)
        .map_err(|reason| Refused::at(name, "contract_invalid", reason))?;
    let Some(binding) = binding else {
        return Ok(());
    };
    if step.kind == Kind::Program || binding.produces.is_empty() {
        return Ok(());
    }
    let object = serde_json::from_str::<Map<String, Value>>(output).map_err(|_| {
        Refused::at(
            name,
            "output_unproduced",
            format!(
                "step {name:?} declares produces, and its output is not an object that can carry them"
            ),
        )
    })?;
    for field in binding.produces.keys() {
        match object.get(field) {
            Some(value) => {
                produced.insert(format!("{name}.{field}"), value.clone());
            }
            None => {
                return Err(Refused::at(
                    name,
                    "output_unproduced",
                    format!(
                        "step {name:?} declares it produces {field:?}, and its output carries none"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The inputs a `program` step's binding hands the child: `request` and
/// `executor` project from the origins the binding states, and absent a
/// binding the child inherits the parent's.
///
/// `tasks` is the run's work list — not a field a binding projects —
/// and any other name is a declared input this host cannot hand a
/// child, which refuses rather than arriving empty.
fn child_inputs(
    inputs: &Inputs,
    binding: &crate::child::Binding,
    produced: &BTreeMap<String, Value>,
    prefix: &str,
    name: &str,
) -> Result<Inputs, Refused> {
    let mut child = inputs.clone();
    let mut projected = false;
    for (input, origin) in &binding.inputs {
        let value = resolve_input(origin, inputs, produced, prefix, name)?;
        match input.as_str() {
            "request" | "executor" => {
                let text = match &value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                match input.as_str() {
                    "request" => {
                        child.request = text;
                        projected = true;
                    }
                    _ => child.executor = text,
                }
            }
            _ => {
                return Err(Refused::at(
                    name,
                    "input_unsupported",
                    format!(
                        "input {input:?} names no field of a run's state this host can hand a child"
                    ),
                ));
            }
        }
    }
    // A projected request is the child's own: the work it lists reads
    // the way `Inputs::read` reads the operator's.
    if projected {
        child.tasks = listed(&child.request);
    }
    Ok(child)
}

/// What one declared input origin answers with: a field of the run's
/// state, an earlier step's produced output, or the literal the document
/// carried. An origin that answers nothing refuses — the composition
/// check proved the producer was declared, so an absent value means the
/// step produced no such field.
fn resolve_input(
    origin: &crate::child::Input,
    inputs: &Inputs,
    produced: &BTreeMap<String, Value>,
    prefix: &str,
    name: &str,
) -> Result<Value, Refused> {
    match origin {
        crate::child::Input::State { field } => match field.as_str() {
            "request" => Ok(Value::String(inputs.request.clone())),
            "executor" => Ok(Value::String(inputs.executor.clone())),
            _ => Err(Refused::at(
                name,
                "input_absent",
                format!("the run's state holds no field {field:?} this input can read"),
            )),
        },
        crate::child::Input::Step { step, field } => {
            let key = match field {
                Some(field) => format!("{prefix}{step}.{field}"),
                None => format!("{prefix}{step}"),
            };
            produced.get(&key).cloned().ok_or_else(|| {
                let what = field.as_ref().map_or_else(
                    || "its output".to_string(),
                    |field| format!("field {field:?}"),
                );
                Refused::at(
                    name,
                    "producer_absent",
                    format!("step {step:?} produced {what} this input reads nothing of"),
                )
            })
        }
        crate::child::Input::Literal { value } => Ok(value.clone()),
    }
}

/// The work a request lists, one task per list item.
///
/// A line counts when it opens with a list marker: `-`, `*`, `•`, `1.`, or
/// `1)`. Everything else is prose around the list — the sentence that asks
/// for the fan-out, a closing remark — and none of it becomes work.
fn listed(request: &str) -> Vec<Task> {
    request.lines().filter_map(item).map(Task::asking).collect()
}

/// The text of one list item, or `None` when the line is not one.
fn item(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = match line.strip_prefix(['-', '*', '•']) {
        Some(rest) => rest,
        None => {
            // A number, then the punctuation that ends it. Three digits is
            // room for more items than any bound here admits.
            let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            if digits == 0 || digits > 3 {
                return None;
            }
            line[digits..].strip_prefix(['.', ')'])?
        }
    };
    // The marker has to be a marker rather than the start of a word:
    // `*args` is prose and `- do the thing` is an item.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    match rest.trim() {
        "" => None,
        text => Some(text),
    }
}

/// The state a plan question reads: named fields rather than a sentence,
/// so a question can point at the tasks directly.
///
/// The collisions the lookup found are put in front of the decision when
/// there are any, and left out when there are none. A plan whose tasks
/// touch six different files therefore reads exactly as it did before the
/// lookup could see a collision at all, which keeps the answers #9414
/// measured comparable, and a plan whose tasks share a file says so in the
/// state rather than leaving the door to work it out.
fn plan_state(program: &Program, inputs: &Inputs, selection: &Selection) -> Value {
    let mut state = Map::new();
    state.insert(
        "plan".to_string(),
        json!({
            "program": program.slug,
            "request": inputs.request,
            "executor": inputs.executor,
            "tasks": selection.len(),
        }),
    );
    state.insert(
        "tasks".to_string(),
        json!(
            selection
                .work
                .iter()
                .map(source::Work::value)
                .collect::<Vec<_>>()
        ),
    );
    if !selection.collisions.is_empty() {
        state.insert("collisions".to_string(), selection.collisions_value());
    }
    Value::Object(state)
}

/// The state a per-requirement question reads: one entry per delegation,
/// under the name its question carries.
fn requirements_state(run: &Run, selection: &Selection) -> Value {
    let mut requirements = Map::new();
    for (n, delegation) in run.delegations.iter().enumerate() {
        requirements.insert(
            requirement_name(n),
            json!({
                "asked": delegation.task.prompt,
                "reads": delegation.task.reads,
                "status": delegation.status.to_string(),
                "answer": delegation.recorded_output(),
                "expects": delegation.task.expected,
                "verdict": delegation.verdict().to_string(),
            }),
        );
    }
    json!({ "requirements": requirements, "tasks": selection.len() })
}

/// The names a per-requirement question set is asked under.
fn requirement_names(run: &Run) -> Vec<String> {
    (0..run.delegations.len()).map(requirement_name).collect()
}

/// One requirement's name. `t1` is what the golden's delegation ids use.
///
/// The names are positional rather than the work's own identifiers, and
/// the delegations are in the order the lookup recorded, so the `ordered`
/// list in the `task_select` record is what maps `t1` back to the work it
/// came from.
fn requirement_name(n: usize) -> String {
    format!("t{}", n + 1)
}

/// The probability an answer carries, whatever its type.
///
/// Every System One answer is a probability or has one: a Noul is the
/// probability, and a Choice and a Score carry the confidence of what they
/// named. An answer with none is one a floor cannot read.
fn probability(answer: &Answer) -> Option<f64> {
    match answer {
        Answer::Noul(noul) => Some(noul.noul),
        Answer::Choice(choice) => Some(choice.confidence),
        Answer::Score(score) => Some(score.confidence),
    }
}

/// What the gated answer said, for the route a decision records.
fn read_of(response: &jev::SystemOneResponse, gate: &str) -> String {
    match response.answers.get(gate) {
        Some(Answer::Choice(choice)) => format!("{} at {:.2}", choice.choice, choice.confidence),
        Some(Answer::Noul(noul)) => format!("{gate} {:.2}", noul.noul),
        Some(Answer::Score(score)) => format!("{gate} {:.2}", score.score),
        None => format!("{} answers", response.answers.len()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn empty_runtime() -> Runtime {
        Runtime {
            survey: Survey {
                capabilities: Vec::new(),
                programs: crate::program::Registry::open(&[]),
                sources: source::Registry::open(&[]),
                workspace: std::env::temp_dir(),
            },
            questions: questions::Registry::open(&[]),
            door: None,
            door_error: None,
            relay: None,
            verification: None,
            review: None,
            runstate: None,
            budget: None,
            repository: None,
            host: Host::without_repository(),
        }
    }

    fn delegation(task: Task, output: &str) -> Delegation {
        Delegation {
            task,
            capability: "stub-local".to_string(),
            binary: PathBuf::from("/bin/stub"),
            workdir: PathBuf::from("/work"),
            concurrent_max: 6,
            status: crate::delegate::Status::Answered,
            output: output.to_string(),
            detail: String::new(),
            bytes: output.len() as u64,
            elapsed: Duration::from_secs(1),
            boundary: None,
            retained: None,
            relayed: None,
        }
    }

    /// Completion is a verdict per work item: the answer it stated is
    /// passed or failed, and an item that stated none is unverifiable.
    /// An unverifiable item is never counted as passed, so a list that
    /// states nothing reads `0 passed`, not `0 of 0 correct`.
    #[test]
    fn acceptance_grades_each_item_against_what_it_expects() {
        let run = Run {
            program: Some("burn-down".to_string()),
            steps: Vec::new(),
            stopped: None,
            selection: None,
            delegations: vec![
                delegation(Task::reading("how many", "a.rs").expecting("5"), "5\n"),
                delegation(Task::reading("how many", "b.rs").expecting("5"), "6"),
                delegation(Task::reading("describe it", "c.rs"), "a module that counts"),
            ],
            answers: BTreeMap::new(),
            verification: vec![],
            review: None,
            spend: Vec::new(),
        };

        assert_eq!(
            run.verdicts(),
            [
                ("t1".to_string(), Verdict::Passed),
                ("t2".to_string(), Verdict::Failed),
                ("t3".to_string(), Verdict::Unverifiable),
            ]
        );
        let tally = run.tally();
        assert_eq!(
            tally,
            Tally {
                passed: 1,
                failed: 1,
                unverifiable: 1
            }
        );
        assert_eq!(tally.to_string(), "1 passed, 1 failed, 1 unverifiable");
        assert!(run.summary().contains("1 passed, 1 failed, 1 unverifiable"));

        let selection = Selection::of(&Source::request(), Vec::new(), 6, OnOverflow::Refuse);
        let state = requirements_state(&run, &selection);
        assert_eq!(state["requirements"]["t1"]["expects"], json!("5"));
        assert_eq!(state["requirements"]["t1"]["verdict"], json!("passed"));
        assert_eq!(state["requirements"]["t2"]["verdict"], json!("failed"));
        assert_eq!(state["requirements"]["t3"]["expects"], Value::Null);
        assert_eq!(
            state["requirements"]["t3"]["verdict"],
            json!("unverifiable")
        );
    }

    #[test]
    fn an_unrepresentable_deadline_refuses_before_any_step_runs() {
        let runtime = empty_runtime();
        for minutes in [0, u64::MAX, u64::MAX / 60] {
            let program: Program = serde_json::from_value(json!({
                "v": 1, "slug": "deadline",
                "steps": [
                    {"name": "select", "kind": "query", "bounds": {}},
                    {"name": "work", "kind": "delegate", "bounds": {"minutes": minutes}}
                ]
            }))
            .unwrap();
            let refused = runtime
                .admit(&program)
                .expect_err("an invalid deadline cannot run");
            assert_eq!(refused.step, "work");
            assert_eq!(refused.code, "bound_unenforceable");
        }
    }

    #[test]
    fn calibration_is_not_silently_reinterpreted_as_scoreability() {
        let runtime = empty_runtime();
        for value in [true, false] {
            let program: Program = serde_json::from_value(json!({
                "v":1,"slug":"legacy-calibration",
                "steps":[{"name":"judge","kind":"decide","bounds":{"requires_calibration":value}}]
            }))
            .unwrap();
            let refused = runtime.admit(&program).unwrap_err();
            assert_eq!(refused.code, "bound_unenforceable");
            assert!(refused.reason.contains("requires_calibration"));
        }
        let program: Program = serde_json::from_value(json!({
            "v":1,"slug":"scoreability",
            "steps":[{"name":"judge","kind":"decide","bounds":{"requires_scorable_answer":true}}]
        }))
        .unwrap();
        assert!(
            runtime
                .admit_bound(&program.steps[0], "requires_scorable_answer", &json!(true))
                .is_ok()
        );
        assert!(
            runtime
                .admit_bound(&program.steps[0], "requires_scorable_answer", &json!("yes"))
                .is_err()
        );
    }

    #[test]
    fn executor_claims_cannot_admit_an_unsupported_bound() {
        let runtime = empty_runtime();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "unknown-bound",
            "steps": [{"name": "work", "kind": "delegate", "bounds": {"memory_mb": 128}}]
        }))
        .unwrap();
        let refused = runtime
            .admit(&program)
            .expect_err("no memory enforcement exists");
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains("memory_mb"));
        for bound in ["read_paths", "network_allowlist", "budget_cents"] {
            let mut program = program.clone();
            program.steps[0].bounds.clear();
            program.steps[0].bounds.insert(bound.into(), json!([]));
            assert_eq!(
                runtime.admit(&program).unwrap_err().code,
                "bound_unenforceable"
            );
        }
    }

    /// A question set the `decide` steps below name, in a registry of
    /// its own.
    fn asked() -> (tempfile::TempDir, Program) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("asked.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": "test.profile-door.v1",
                "name": "A profile door",
                "questions": {
                    "q": {"type": "noul", "instructions": "Whether the step ran."}
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "judge",
            "steps": [{"name": "judge", "kind": "decide",
                       "question": "test.profile-door.v1", "bounds": {}}]
        }))
        .unwrap();
        (dir, program)
    }

    /// A flag-sourced setting, for a profile a test constructs.
    fn flagged(value: &str) -> crate::profiles::Sourced<String> {
        crate::profiles::Sourced {
            value: value.to_string(),
            source: crate::profiles::Source::Flag,
        }
    }

    /// A door admission accepts and dispatch cannot reach — the
    /// loopback profile pointed at a closed port, for tests that need a
    /// configured door but never a live answer.
    fn dead_door() -> jev::Client {
        crate::profiles::Profile::DirectLocal {
            url: flagged("http://127.0.0.1:1"),
            model: flagged("kev-local"),
            picked: crate::profiles::Source::Flag,
        }
        .client()
        .unwrap()
    }

    /// Every profile kind that builds a System One door.
    fn http_profiles() -> [crate::profiles::Profile; 4] {
        use crate::profiles::{Profile, Source, Sourced};
        [
            Profile::HostedHttp {
                url: flagged("https://decisions.example.com"),
                model: flagged("jev"),
                key: Sourced {
                    value: jev::ApiKey::new("oak_test.secret"),
                    source: Source::Flag,
                },
                picked: Source::Flag,
            },
            Profile::DirectLocal {
                url: flagged("http://127.0.0.1:1"),
                model: flagged("kev-local"),
                picked: Source::Flag,
            },
            Profile::OwnProvider {
                url: flagged("https://doors.example.com"),
                model: flagged("kev-shared"),
                key: Some(Sourced {
                    value: jev::ApiKey::new("oak_test.secret"),
                    source: Source::Flag,
                }),
                picked: Source::Flag,
            },
            Profile::OwnProvider {
                url: flagged("http://[::1]:9"),
                model: flagged("kev-own"),
                key: None,
                picked: Source::Flag,
            },
        ]
    }

    /// The door a `decide` step asks is the one the resolved profile
    /// built: every profile kind's client admits the step, and none of
    /// them needed a construction of its own.
    #[test]
    fn a_decide_step_asks_whichever_profile_built_the_door() {
        let (dir, program) = asked();
        for profile in http_profiles() {
            let door = profile
                .client()
                .unwrap_or_else(|refusal| panic!("{} builds a door: {refusal}", profile.name()));
            let mut runtime = empty_runtime().asking(Some(door));
            runtime.questions = questions::Registry::open(&[dir.path().to_path_buf()]);
            runtime
                .admit(&program)
                .unwrap_or_else(|refused| panic!("{} admits the step: {refused}", profile.name()));
        }
    }

    /// A resolution that failed stops the path that needed the door:
    /// admission refuses the `decide` step before it is ever asked, and
    /// the reason it surfaces is the resolver's own.
    #[test]
    fn a_failed_door_resolution_refuses_the_step_that_needed_it() {
        let (dir, program) = asked();
        let mut runtime = empty_runtime();
        runtime.questions = questions::Registry::open(&[dir.path().to_path_buf()]);
        runtime.door_error = Some(
            "CODER_DECISION_PROFILE must be hosted_http, direct_local, own_provider, or relay"
                .to_string(),
        );
        let refused = runtime.admit(&program).unwrap_err();
        assert_eq!(refused.step, "judge");
        assert_eq!(refused.code, "door_configuration");
        assert!(refused.reason.contains("CODER_DECISION_PROFILE"));
    }

    /// The same failure reaches the turn's selection question: a
    /// resolution that erred refuses `select` with the resolver's
    /// reason, never a quieter nothing-configured.
    #[tokio::test]
    async fn a_failed_door_resolution_refuses_selection_with_its_reason() {
        let programs = tempfile::tempdir().unwrap();
        std::fs::write(
            programs.path().join("ask-only.json"),
            serde_json::to_string(&json!({
                "v": 1, "slug": "ask-only",
                "steps": [{"name": "look", "kind": "query", "bounds": {}}]
            }))
            .unwrap(),
        )
        .unwrap();
        let questions_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            questions_dir.path().join("program.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": PROGRAM_QUESTION,
                "name": "Which program applies",
                "gate": "program",
                "questions": {
                    "program": {
                        "type": "choice",
                        "instructions": "Which program does this request ask for?",
                        "options": "supplied",
                        "criteria": {"none": "This request asks for no program."}
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.door_error = Some("CODER_DECISION_URL is not an http or https URL".to_string());

        let refused = runtime.select("run it", None).await.unwrap_err();
        assert_eq!(refused.code, "door_unavailable");
        assert_eq!(
            refused.reason,
            "CODER_DECISION_URL is not an http or https URL"
        );
    }

    /// A door that reads each request and answers `status` with a small
    /// error body — enough for a caller's failure mapping to see the
    /// status the API returned.
    async fn serve_status(status: u16) -> u16 {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0u8; 1 << 16];
                let _ = socket.read(&mut buf).await;
                let body = br#"{"error":{"message":"denied"}}"#;
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body).await;
            }
        });
        port
    }

    /// A decision door's authorization and quota answers keep their own
    /// refusal codes: the status the door returned — not a flattened
    /// "unavailable" — is what a revocation, a narrowed permission, and
    /// an exhausted quota look like to a caller that acts on them.
    #[tokio::test]
    async fn a_door_refusing_authorization_names_itself() {
        let questions_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            questions_dir.path().join("program.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": PROGRAM_QUESTION,
                "name": "Which program applies",
                "gate": "program",
                "questions": {
                    "program": {
                        "type": "choice",
                        "instructions": "Which program does this request ask for?",
                        "options": "supplied",
                        "criteria": {"none": "This request asks for no program."}
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "ask-only",
            r#"[{"name": "look", "kind": "query", "bounds": {}}]"#,
        );
        for (status, code) in [
            (401u16, "door_unauthenticated"),
            (403, "door_unauthorized"),
            (429, "door_rate_limited"),
        ] {
            let port = serve_status(status).await;
            let mut runtime = empty_runtime();
            runtime.survey.programs =
                crate::program::Registry::open(&[programs.path().to_path_buf()]);
            runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
            runtime.door = Some(
                jev::Client::new(jev::Config::local(
                    format!("http://127.0.0.1:{port}"),
                    "stub",
                ))
                .unwrap(),
            );
            let refused = runtime.select("run it", None).await.unwrap_err();
            assert_eq!(refused.code, code, "status {status}");
        }
    }

    /// A door's choice is held to the option set the question offered:
    /// naming a program this host resolved but would not run is the
    /// same refusal as naming one it never heard of — executor
    /// selection considers only host-authorized candidates, and a
    /// selection cannot exceed what the host admitted.
    #[tokio::test]
    async fn a_selection_is_held_to_the_offered_programs() {
        let questions_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            questions_dir.path().join("program.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": PROGRAM_QUESTION,
                "name": "Which program applies",
                "gate": "program",
                "questions": {
                    "program": {
                        "type": "choice",
                        "instructions": "Which program does this request ask for?",
                        "options": "supplied",
                        "criteria": {"none": "This request asks for no program."}
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let programs = tempfile::tempdir().unwrap();
        // Admitted: a query step asks nothing this host lacks.
        stage_program(
            programs.path(),
            "ask-only",
            r#"[{"name": "look", "kind": "query", "bounds": {}}]"#,
        );
        // Resolved but inadmissible: a decide step on wording this host
        // never resolved is a program it would not run.
        stage_program(
            programs.path(),
            "needs-wording",
            r#"[{"name": "judge", "kind": "decide", "question": "openagents.missing.v1", "bounds": {}}]"#,
        );

        for (choice, expected) in [
            // The registry resolved it, but admission never offered it:
            // naming it is not a selection.
            ("needs-wording", None),
            // The one admitted program selects.
            ("ask-only", Some("ask-only")),
            // And none is an answer, not an error.
            ("none", None),
        ] {
            let port = serve_answer(json!({
                "model": "stub",
                "answers": {
                    "program": {
                        "type": "choice",
                        "choice": choice,
                        "confidence": 0.9,
                        "probabilities": {"ask-only": 0.5, "needs-wording": 0.3, "none": 0.2}
                    }
                }
            }))
            .await;
            let mut runtime = empty_runtime();
            runtime.survey.programs =
                crate::program::Registry::open(&[programs.path().to_path_buf()]);
            runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
            runtime.door = Some(
                jev::Client::new(jev::Config::local(
                    format!("http://127.0.0.1:{port}"),
                    "stub",
                ))
                .unwrap(),
            );

            match (runtime.select("run it", None).await, expected) {
                (Ok(Selected::Program(slug)), Some(wanted)) => assert_eq!(slug, wanted),
                (Ok(Selected::None), None) if choice == "none" => {}
                (Err(refused), None) => assert_eq!(refused.code, "no_program_chosen"),
                (answered, _) => panic!("{choice} answered {answered:?}"),
            }
        }
    }

    /// A door answering every request with one System One body — the
    /// answer a set's policy is held against.
    async fn serve_answer(body: Value) -> u16 {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0u8; 1 << 16];
                let _ = socket.read(&mut buf).await;
                let body = body.to_string();
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body.as_bytes()).await;
            }
        });
        port
    }

    /// A question set under `dir` carrying `policy` — the binding a run
    /// is held to.
    fn policy_questions(dir: &Path, policy: &str) {
        std::fs::write(
            dir.join("policy.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": "test.policy.v1",
                "name": "A bound function",
                "gate": "q",
                "questions": {"q": {"type": "noul", "instructions": "Whether the work may run."}},
                "policy": serde_json::from_str::<Value>(policy).unwrap()
            }))
            .unwrap(),
        )
        .unwrap();
    }

    /// A `decide` step asking the policy-bound set.
    fn policy_program() -> Program {
        serde_json::from_value(json!({
            "v": 1, "slug": "policy-run",
            "steps": [{"name": "judge", "kind": "decide", "question": "test.policy.v1", "bounds": {}}]
        }))
        .unwrap()
    }

    /// A question set's policy binds before dispatch: a state bigger
    /// than the function declares, and a requested model the function
    /// does not admit, each refuse the step before the door is asked —
    /// a binding is a claim the host enforces, not a note.
    #[tokio::test]
    async fn a_sets_policy_binds_before_the_door_is_asked() {
        for (policy, code) in [
            (r#"{"state_max_bytes": 8}"#, "state_over_bound"),
            (r#"{"models": ["other-model"]}"#, "unbound_model"),
        ] {
            let questions_dir = tempfile::tempdir().unwrap();
            policy_questions(questions_dir.path(), policy);
            let mut runtime = empty_runtime();
            runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
            runtime.door = Some(dead_door());
            let inputs = Inputs::read("do the list\n- one", "stub-local");

            let run = runtime
                .run(&policy_program(), &inputs, &Grant::all(), None)
                .await;
            assert_eq!(
                run.stopped.as_ref().map(|refused| refused.code.as_str()),
                Some(code),
                "policy {policy}"
            );
        }
    }

    /// And it binds what comes back: an answer reporting a model the
    /// set does not admit is refused, and a gated confidence under the
    /// abstention floor is the typed abstention — not a read a caller
    /// treats as an answer.
    #[tokio::test]
    async fn a_sets_policy_binds_the_answer() {
        // The requested model is admitted; the reported one is not.
        let questions_dir = tempfile::tempdir().unwrap();
        policy_questions(questions_dir.path(), r#"{"models": ["stub"]}"#);
        let port = serve_answer(json!({
            "model": "other-model",
            "answers": {"q": {"type": "noul", "noul": 0.9}}
        }))
        .await;
        let mut runtime = empty_runtime();
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.door = Some(
            jev::Client::new(jev::Config::local(
                format!("http://127.0.0.1:{port}"),
                "stub",
            ))
            .unwrap(),
        );
        let inputs = Inputs::read("do the list\n- one", "stub-local");
        let run = runtime
            .run(&policy_program(), &inputs, &Grant::all(), None)
            .await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some("unbound_model")
        );

        // A gated answer under the floor abstains, stated as such.
        let questions_dir = tempfile::tempdir().unwrap();
        policy_questions(questions_dir.path(), r#"{"abstain_below": 0.5, "v": 2}"#);
        let port = serve_answer(json!({
            "model": "stub",
            "answers": {"q": {"type": "noul", "noul": 0.3}}
        }))
        .await;
        let mut runtime = empty_runtime();
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.door = Some(
            jev::Client::new(jev::Config::local(
                format!("http://127.0.0.1:{port}"),
                "stub",
            ))
            .unwrap(),
        );
        let run = runtime
            .run(&policy_program(), &inputs, &Grant::all(), None)
            .await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some("abstained")
        );
    }

    /// The work is the list the sentence carries, in the order it was
    /// written, and the prose around it is not work.
    #[test]
    fn a_request_supplies_its_task_list() {
        let inputs = Inputs::read(
            "Delegate six instances of Devin, one for each of these read-only questions.\n\
             \n\
             1. How many crates are in this workspace?\n\
             2) What does ROUNDS_MAX do?\n\
             - Which kinds does NIP-PRG define?\n\
             * How many estimators does lev have?\n\
             \n\
             Report back when they are all in.",
            "devin-local",
        );

        assert_eq!(inputs.tasks.len(), 4, "four items, not six lines of prose");
        assert_eq!(
            inputs.tasks[0].prompt,
            "How many crates are in this workspace?"
        );
        assert_eq!(inputs.tasks[3].prompt, "How many estimators does lev have?");
        assert_eq!(inputs.executor, "devin-local");
        assert!(
            inputs.tasks.iter().all(|task| !task.writes),
            "a listed task reads unless something else says otherwise"
        );
    }

    /// A sentence with no list supplies no work. The `query` step refuses
    /// rather than inventing any, which is what keeps a wrongly selected
    /// program from fanning out over something nobody named.
    #[test]
    fn a_request_with_no_list_supplies_no_work() {
        for request in [
            "What does ROUNDS_MAX do?",
            "Delegate six instances of Devin.",
            "*args is how Python spells it",
            "2026 was the year",
            "-",
        ] {
            let inputs = Inputs::read(request, "devin-local");
            assert!(inputs.tasks.is_empty(), "{request:?} listed no work");
        }
    }

    /// The selection is a slug or nothing, and a caller can ask which
    /// without matching on prose.
    #[test]
    fn a_selection_names_a_program_or_none() {
        assert_eq!(Selected::None.program(), None);
        assert_eq!(
            Selected::Program("delegate-fan-out".to_string()).program(),
            Some("delegate-fan-out")
        );
    }

    /// A program the grant does not name refuses before its first step,
    /// and one whose declared effects exceed the grant's ceiling names
    /// the step and the missing effect. An undeclared executor is the
    /// wider set — subprocesses and spend — because the refusal that
    /// names it reads the same survey.
    #[test]
    fn a_program_is_held_to_the_operators_grant() {
        let runtime = empty_runtime();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "work", "kind": "delegate", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let refused = runtime
            .authorize(&program, &inputs, &Grant::none())
            .expect_err("a grant of nothing runs nothing");
        assert_eq!(refused.code, program_authority::UNAUTHORIZED);
        assert!(refused.reason.contains("burn-down"), "{refused}");

        let grant = Grant::selected(Some("burn-down"), Some("reads"));
        let refused = runtime
            .authorize(&program, &inputs, &grant)
            .expect_err("reads alone does not delegate");
        assert_eq!(refused.step, "work");
        assert_eq!(refused.code, program_authority::UNAUTHORIZED);
        assert!(refused.reason.contains("delegation"), "{refused}");

        runtime
            .authorize(&program, &inputs, &Grant::all())
            .expect("a full grant admits what the host admitted");
    }

    /// The run ids under a runstate directory — one `*.jsonl` file per
    /// claimed run.
    fn claimed(dir: &Path) -> Vec<String> {
        let mut runs: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".jsonl").map(str::to_string)
            })
            .collect();
        runs.sort();
        runs
    }

    /// With a runstate directory set, a finished run leaves a claimed
    /// and settled record: the run id it claimed, the names and digests
    /// the claim pinned, each step's marks, and what the run came to.
    #[tokio::test]
    async fn a_run_leaves_a_settled_runstate_record() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime().with_runstate(dir.path());
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");
        let logs = tempfile::tempdir().unwrap();
        let mut recorder =
            Recorder::open(logs.path(), "fixture", "fixture", "fixture-repo").unwrap();
        let trace = recorder.path().to_string_lossy().into_owned();

        let run = runtime
            .run(&program, &inputs, &Grant::all(), Some(&mut recorder))
            .await;
        assert!(run.finished(), "{:?}", run.stopped);

        let mut store = Store::open(dir.path()).unwrap();
        // A settled run is complete: recovery surfaces nothing.
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "one run, one record file: {ids:?}");
        assert!(ids[0].starts_with("run-burn-down-"), "{}", ids[0]);
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Answered));
        // The claim pins: the program by its canonical digest, this
        // host's missing base commit as empty, and the request source
        // the query step read.
        assert_eq!(record.program, crate::child::digest(&program));
        assert_eq!(record.base, "");
        assert_eq!(record.sources, ["request"]);
        assert!(record.questions.is_empty());
        // The result reference is the trace, because a recorder exists.
        assert_eq!(record.result.as_deref(), Some(trace.as_str()));
        assert_eq!(record.steps.len(), 1);
        assert_eq!(record.steps[0].step, "select");
        assert_eq!(record.steps[0].state, State::Answered);
    }

    /// A run refused at admission or by the grant claims nothing: a
    /// refused run holds no run id, and the directory is never created.
    #[tokio::test]
    async fn a_refused_run_leaves_no_runstate_record() {
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "unknown-bound",
            "steps": [{"name": "work", "kind": "delegate", "bounds": {"memory_mb": 128}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime().with_runstate(dir.path().join("runstate"));
        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some("bound_unenforceable")
        );
        assert!(!dir.path().join("runstate").exists());

        // And a program the grant does not cover refuses just as early.
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime().with_runstate(dir.path().join("runstate"));
        let run = runtime.run(&program, &inputs, &Grant::none(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(program_authority::UNAUTHORIZED)
        );
        assert!(!dir.path().join("runstate").exists());
    }

    /// A run a step stops mid-way leaves the run's dispatched record,
    /// each step's own record — answered for the one that ran, refused
    /// for the one that stopped it — settled as refused.
    #[tokio::test]
    async fn a_run_stopping_midway_settles_refused() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime().with_runstate(dir.path());
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "narrow", "kind": "query",
                 "bounds": {"max_results": 1, "on_overflow": "refuse"}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one\n- two", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some("too_many_results")
        );
        assert_eq!(run.step_names(), ["select"]);

        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Refused));
        // No recorder exists: the result reference is the program slug.
        assert_eq!(record.result.as_deref(), Some("burn-down"));
        assert_eq!(record.steps.len(), 2);
        let select = record
            .steps
            .iter()
            .find(|step| step.step == "select")
            .unwrap();
        let narrow = record
            .steps
            .iter()
            .find(|step| step.step == "narrow")
            .unwrap();
        assert_eq!(select.state, State::Answered);
        assert_eq!(narrow.state, State::Refused);
    }

    /// A run the budget stops settles cancelled: the step past the
    /// budget and every step after it mark cancelled, and the record
    /// says the caller ended it — not a refusal the work gave.
    #[tokio::test]
    async fn a_run_past_its_budget_settles_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime()
            .with_runstate(dir.path())
            .with_budget(Budget {
                deadline: None,
                max_steps: Some(1),
                spend: None,
            });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "narrow", "kind": "query", "bounds": {}},
                {"name": "last", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(BUDGET_EXCEEDED)
        );
        assert_eq!(run.step_names(), ["select"]);

        let mut store = Store::open(dir.path()).unwrap();
        // A settled run is complete — cancelled or not, recovery
        // surfaces nothing.
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        assert_eq!(record.steps.len(), 3);
        let select = record
            .steps
            .iter()
            .find(|step| step.step == "select")
            .unwrap();
        let narrow = record
            .steps
            .iter()
            .find(|step| step.step == "narrow")
            .unwrap();
        let last = record
            .steps
            .iter()
            .find(|step| step.step == "last")
            .unwrap();
        assert_eq!(select.state, State::Answered);
        assert_eq!(narrow.state, State::Cancelled);
        assert_eq!(last.state, State::Cancelled);
    }

    /// A step reached after the deadline never dispatches: the budget is
    /// spent before the first boundary, the step's only mark is the
    /// cancellation, and the run settles cancelled.
    #[tokio::test]
    async fn a_step_past_the_deadline_never_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime()
            .with_runstate(dir.path())
            .with_budget(Budget {
                deadline: Some(Duration::ZERO),
                max_steps: None,
                spend: None,
            });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(BUDGET_EXCEEDED)
        );
        assert!(run.steps.is_empty());

        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        let select = record
            .steps
            .iter()
            .find(|step| step.step == "select")
            .unwrap();
        assert_eq!(select.state, State::Cancelled);
        // On disk the step's first and only line is the cancelled mark:
        // it was never dispatched.
        let text = std::fs::read_to_string(dir.path().join(format!("{}.jsonl", ids[0]))).unwrap();
        let marks: Vec<&str> = text
            .lines()
            .filter(|line| line.contains("\"step\":\"select\""))
            .collect();
        assert_eq!(marks.len(), 1, "{marks:?}");
        assert!(marks[0].contains("\"state\":\"cancelled\""), "{}", marks[0]);
    }

    /// A cancelled run and a crashed one read differently on disk: the
    /// cancelled run settled — deliberate, recorded, absent from
    /// recovery — while the run that simply stopped mid-way is the one
    /// recovery marks unknown.
    #[tokio::test]
    async fn a_cancelled_run_is_not_a_crashed_one() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime()
            .with_runstate(dir.path())
            .with_budget(Budget {
                deadline: None,
                max_steps: Some(0),
                spend: None,
            });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");
        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert!(!run.finished());

        // A second run claimed and abandoned in the same store is the
        // crash: dispatched and never settled.
        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-crashed",
                base: "",
                program: "burn-down",
                questions: &[],
                sources: &[],
            })
            .unwrap();
        store
            .advance("run-crashed", Mark::run(State::Dispatched))
            .unwrap();

        // Recovery returns the crash, and only the crash.
        let recovered = store.recover().unwrap();
        assert_eq!(recovered.len(), 1, "{recovered:?}");
        assert_eq!(recovered[0].run, "run-crashed");
        assert_eq!(recovered[0].state, State::Unknown);
        assert_eq!(claimed(dir.path()).len(), 2);
    }

    /// A runtime carrying no budget runs exactly as it always has:
    /// every step dispatches, and the run settles answered.
    #[tokio::test]
    async fn a_run_without_a_budget_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime().with_runstate(dir.path());
        assert!(runtime.budget.is_none());
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "again", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(run.step_names(), ["select", "again"]);

        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Answered));
        assert!(
            record
                .steps
                .iter()
                .all(|step| step.state == State::Answered)
        );
    }

    /// A cancelled run's record is deliberate end to end: `settled`
    /// with `cancelled` as what it came to, written on disk — never
    /// `unknown`, which is only ever what a crash leaves behind.
    #[tokio::test]
    async fn a_cancelled_run_records_deliberate_settlement() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime()
            .with_runstate(dir.path())
            .with_budget(Budget {
                deadline: Some(Duration::ZERO),
                max_steps: None,
                spend: None,
            });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");
        runtime.run(&program, &inputs, &Grant::all(), None).await;

        let mut store = Store::open(dir.path()).unwrap();
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        // The last run line on disk is the settle, carrying the
        // cancelled outcome — an end someone chose, not a mark
        // recovery wrote.
        let text = std::fs::read_to_string(dir.path().join(format!("{}.jsonl", ids[0]))).unwrap();
        let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last["record"], json!("run"));
        assert_eq!(last["state"], json!("settled"));
        assert_eq!(last["outcome"], json!("cancelled"));
        assert!(store.recover().unwrap().is_empty());
    }

    /// Writes an executable stub that stands in for the executor, the
    /// way `crate::delegate`'s tests stand one in.
    fn stub(dir: &Path, name: &str, script: &str) -> PathBuf {
        use std::io::Write as _;
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

    /// A checkout with one commit, for the cases a worktree comes from.
    /// `None` on a machine with no working version control.
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

    /// A probed capability over `binary`, the shape `Survey::read`
    /// reports: the probe's path is the binary a delegation runs, and
    /// the manifest's `invoke` contributes no arguments — the prompt a
    /// delegation appends is the argv's last word, which the stub
    /// ignores.
    fn stub_capability(binary: &Path) -> capability::Found {
        capability::Found {
            manifest: capability::Manifest {
                v: 1,
                slug: "stub-local".to_string(),
                name: "A stub".to_string(),
                summary: String::new(),
                transport: capability::SUBPROCESS.to_string(),
                detect: capability::Detect {
                    binary: binary.display().to_string(),
                    ..Default::default()
                },
                enforces: vec!["minutes".to_string()],
                cannot_enforce: Vec::new(),
                sees_repository: true,
                concurrent_max: Some(2),
                cost: "local".to_string(),
                isolation: vec!["directory".to_string(), "worktree".to_string()],
                invoke: vec!["stub-local".to_string()],
                invoke_writing: Vec::new(),
                workspace_probe: None,
                refuses: Vec::new(),
            },
            presence: Presence::Present {
                version: "stub 1.0".to_string(),
                report: "stub 1.0".to_string(),
                path: binary.to_path_buf(),
            },
            workspace: PathBuf::new(),
            milliseconds: 0,
            proof: capability::Proof::Unconditional,
            source: capability::Source::Operator,
            digest: String::new(),
            path: PathBuf::new(),
        }
    }

    /// Whether this machine's boundary applies a profile, not just
    /// builds one — a nested sandbox cannot, and there the case is
    /// about an environment that cannot run a bounded command rather
    /// than about the code.
    fn boundary_enforces() -> bool {
        let Ok(boundary) = coder_boundary::Boundary::readonly().build() else {
            return false;
        };
        let Ok(mut command) = boundary.command("/usr/bin/true", Vec::<&std::ffi::OsStr>::new())
        else {
            return false;
        };
        command
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// A runtime whose survey holds the stub alone, over a real
    /// checkout, so a `delegate` step fans out for real.
    fn stub_runtime(repo: &Path, binary: &Path, runstate: &Path) -> Runtime {
        Runtime {
            survey: Survey {
                capabilities: vec![stub_capability(binary)],
                programs: crate::program::Registry::open(&[]),
                sources: source::Registry::open(&[]),
                workspace: repo.to_path_buf(),
            },
            questions: questions::Registry::open(&[]),
            door: None,
            door_error: None,
            relay: None,
            verification: None,
            review: None,
            runstate: Some(runstate.to_path_buf()),
            budget: None,
            repository: Some(repo.to_path_buf()),
            host: Host::with_repository(),
        }
    }

    /// A deadline that expires while a step awaits its dispatch ends
    /// the step where it stands — the dispatch is dropped, the same
    /// cancel path supervise holds a spawned job to — and the step
    /// marks `cancelled`, not a refusal the step gave. The settled
    /// record is the partial completion: the step that answered stays
    /// answered, the step the deadline ended and the one it never
    /// reached are cancelled.
    #[tokio::test]
    async fn a_deadline_expiring_mid_step_cancels_the_step() {
        let questions_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            questions_dir.path().join("hang.json"),
            serde_json::to_string(&json!({
                "v": 1,
                "id": "test.hang.v1",
                "name": "A door that never answers",
                "questions": {
                    "q": {"type": "noul", "instructions": "Whether the work may run."}
                }
            }))
            .unwrap(),
        )
        .unwrap();
        // A listener that accepts and never replies: the door the step
        // awaits hangs until the run's bound ends the await.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let door = jev::Client::new(jev::Config::local(
            format!("http://127.0.0.1:{port}"),
            "stub",
        ))
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = empty_runtime()
            .with_runstate(dir.path())
            .with_budget(Budget {
                deadline: Some(Duration::from_secs(3)),
                max_steps: None,
                spend: None,
            });
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.door = Some(door);
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "judge", "kind": "decide", "question": "test.hang.v1", "bounds": {}},
                {"name": "after", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let started = Instant::now();
        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert!(
            started.elapsed() < Duration::from_secs(9),
            "the run outlived the deadline it carried"
        );
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(BUDGET_EXCEEDED)
        );
        assert_eq!(run.step_names(), ["select"]);

        // Partial completion, settled: `select` answered, `judge` and
        // `after` cancelled, and the outcome is the caller's end.
        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.state, State::Settled);
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        for (name, state) in [
            ("select", State::Answered),
            ("judge", State::Cancelled),
            ("after", State::Cancelled),
        ] {
            let step = record
                .steps
                .iter()
                .find(|step| step.step == name)
                .unwrap_or_else(|| panic!("no step record for {name}"));
            assert_eq!(step.state, state, "{name}");
        }
    }

    /// A deadline that expires while a `delegate` step's subprocesses
    /// run ends them through supervise's own bound: the delegation's
    /// wall is tightened to what the run has left, the process group
    /// goes down when it fires, and the step marks `cancelled`.
    #[tokio::test]
    async fn a_deadline_expiring_mid_step_stops_the_steps_subprocess() {
        if !boundary_supported() || !boundary_enforces() {
            return;
        }
        let Some(repo) = scratch_repository() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "slow", "sleep 60");
        let runtime = stub_runtime(repo.path(), &binary, dir.path()).with_budget(Budget {
            deadline: Some(Duration::from_secs(3)),
            max_steps: None,
            spend: None,
        });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "work", "kind": "delegate", "bounds": {"minutes": 5}}
            ]
        }))
        .unwrap();
        let inputs = Inputs {
            request: "sleep".to_string(),
            tasks: vec![Task::asking("the sixty-second task")],
            executor: "stub-local".to_string(),
        };

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(BUDGET_EXCEEDED),
            "{:?}",
            run.stopped
        );
        // The step's subprocess did not run out its stated five
        // minutes: the tightened wall ended its group inside the
        // deadline.
        assert_eq!(run.delegations.len(), 1);
        assert_eq!(
            run.delegations[0].status,
            Status::TimedOut,
            "{:?}",
            run.delegations[0].detail
        );
        assert!(
            run.delegations[0].elapsed < Duration::from_secs(30),
            "{:?}",
            run.delegations[0].elapsed
        );

        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        let record = store.get(&ids[0]).unwrap().unwrap();
        let work = record
            .steps
            .iter()
            .find(|step| step.step == "work")
            .unwrap();
        assert_eq!(work.state, State::Cancelled);
    }

    /// A cancelled run keeps what it claimed: the worktree a writing
    /// delegation retained is marked on the task's record under the
    /// cancelled mark — left on disk for a reconciler, never silently
    /// removed and never replayed.
    #[tokio::test]
    async fn a_cancelled_run_keeps_the_worktree_it_claimed() {
        if !boundary_supported() || !boundary_enforces() {
            return;
        }
        let Some(repo) = scratch_repository() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "slow", "sleep 60");
        let runtime = stub_runtime(repo.path(), &binary, dir.path()).with_budget(Budget {
            deadline: Some(Duration::from_secs(3)),
            max_steps: None,
            spend: None,
        });
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "work", "kind": "delegate",
                 "bounds": {"minutes": 5, "isolation": "worktree"}}
            ]
        }))
        .unwrap();
        let mut task = Task::asking("the sixty-second task");
        task.writes = true;
        let inputs = Inputs {
            request: "sleep".to_string(),
            tasks: vec![task],
            executor: "stub-local".to_string(),
        };

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert_eq!(
            run.stopped.as_ref().map(|refused| refused.code.as_str()),
            Some(BUDGET_EXCEEDED)
        );
        // The delegation claimed a checkout before the deadline ended
        // it, and a writing delegation's checkout is the reviewer's to
        // read — it stays where the executor left it.
        assert_eq!(run.delegations.len(), 1);
        let retained = run.delegations[0]
            .retained
            .clone()
            .expect("a writing delegation keeps its worktree");
        assert!(
            retained.exists(),
            "the claimed worktree is gone from where the record names it: {}",
            retained.display()
        );

        let mut store = Store::open(dir.path()).unwrap();
        assert!(store.recover().unwrap().is_empty());
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        let work = record
            .steps
            .iter()
            .find(|step| step.step == "work")
            .unwrap();
        assert_eq!(work.state, State::Cancelled);
        let attempt = record
            .tasks
            .iter()
            .find(|task| task.task == "t1")
            .expect("the claimed worktree is marked on the task's record");
        assert_eq!(attempt.state, State::Cancelled);
        assert_eq!(attempt.worktree.as_deref(), Some(retained.as_path()));
    }

    /// A cancelled parent's pending children are marked like every step
    /// the run never reached: the `program` step marks `cancelled`, and
    /// each of its child's steps marks `cancelled` under `parent/child`,
    /// the name the record keeps for them.
    #[tokio::test]
    async fn a_cancelled_parent_marks_its_pending_children_cancelled() {
        let programs = tempfile::tempdir().unwrap();
        std::fs::write(
            programs.path().join("child.json"),
            serde_json::to_string(&json!({
                "v": 1, "slug": "child-program",
                "steps": [
                    {"name": "gather", "kind": "query", "bounds": {}},
                    {"name": "ship", "kind": "query", "bounds": {}}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "select", "kind": "query", "bounds": {}},
                {"name": "call", "kind": "program", "program": "child-program", "bounds": {}},
                {"name": "after", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();

        // A record claimed and dispatched the way `run` leaves one,
        // then cancelled at the step after `select`.
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-parent",
                base: "",
                program: "parent-program",
                questions: &[],
                sources: &[],
            })
            .unwrap();
        let mut record = Some((store, "run-parent".to_string()));
        let run = Run::default();
        runtime.cancel_from(&mut record, &run, &parent, 1, None);

        let store = Store::open(dir.path()).unwrap();
        let record = store.get("run-parent").unwrap().unwrap();
        for name in ["call", "call/gather", "call/ship", "after"] {
            let step = record
                .steps
                .iter()
                .find(|step| step.step == name)
                .unwrap_or_else(|| panic!("no step record for {name}"));
            assert_eq!(step.state, State::Cancelled, "{name}");
        }
        // A step the parent never reached is nowhere: `select` was
        // never marked by the cancellation.
        assert!(record.steps.iter().all(|step| step.step != "select"));
    }

    /// A `program` step is held to the grant by what its composition
    /// declares: a parent whose child delegates is not a reads-only
    /// program, and a grant that allows only reads refuses it — naming
    /// the parent consents to the composition it pins, never to more.
    #[test]
    fn a_childs_declared_effects_are_held_to_the_grant() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "work", "kind": "delegate", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let grant = Grant::selected(Some("parent-program"), Some("reads"));
        let refused = runtime
            .authorize(&parent, &inputs, &grant)
            .expect_err("a reads-only grant does not cover a child's delegation");
        assert_eq!(refused.code, program_authority::UNAUTHORIZED);
        assert_eq!(refused.step, "call");
        runtime
            .authorize(&parent, &inputs, &Grant::all())
            .expect("a full grant admits the whole composition");
    }

    /// A resumer's rulings read the program the record pinned: the
    /// dispatched step's reference reattaches through the host's
    /// observation, and the `unknown` marks rule by the effects the
    /// pinned program declared — reads replay, and nothing guesses.
    #[test]
    fn rulings_read_the_pinned_programs_declared_effects() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "read-only",
            r#"[{"name": "look", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let program = runtime.survey.programs.get("read-only").unwrap().clone();

        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-1",
                base: "",
                program: &crate::child::digest(&program),
                questions: &[],
                sources: &["request".to_string()],
            })
            .unwrap();
        store
            .advance("run-1", Mark::run(State::Dispatched))
            .unwrap();
        store
            .advance(
                "run-1",
                Mark::step("look", State::Dispatched).result("job-7"),
            )
            .unwrap();
        let runs = store.recover().unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.steps[0].state, State::Unknown);

        let rulings = runtime.rulings(run, &Grant::all(), |_, _| {
            crate::reattach::Observation::Live {
                reference: "job-7",
                pins_match: true,
            }
        });
        // The live job under matching pins is observed to its end —
        // never run a second attempt alongside it.
        assert!(matches!(
            rulings.reattach.as_slice(),
            [(subject, crate::reattach::Reattachment::Observe { reference })]
                if subject == "step:look" && reference == "job-7"
        ));
        // A read-only step's unknown mark is replayable — reads could
        // not have changed the world — and so is the run's own.
        let look = rulings
            .reconcile
            .iter()
            .find(|ruling| ruling.subject == "step:look")
            .unwrap();
        assert_eq!(look.ruling, crate::reconcile::Reconciliation::Replayable);
        let whole = rulings
            .reconcile
            .iter()
            .find(|ruling| ruling.subject == "run")
            .unwrap();
        assert_eq!(whole.ruling, crate::reconcile::Reconciliation::Replayable);
    }

    /// A record whose pin names no program this host holds declares
    /// nothing, and undeclared effects are never replayable — the
    /// ruling is `NeedsDecision`, stated rather than guessed. The same
    /// unknown record under a narrower grant than its declared work
    /// rules `OutsideAuthority`: a crash does not widen authority.
    #[test]
    fn rulings_refuse_what_the_pin_or_grant_cannot_cover() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "writes-work",
            r#"[{"name": "work", "kind": "delegate", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);

        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-2",
                base: "",
                program: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                questions: &[],
                sources: &[],
            })
            .unwrap();
        store
            .advance("run-2", Mark::run(State::Dispatched))
            .unwrap();
        store
            .advance("run-2", Mark::step("work", State::Dispatched))
            .unwrap();
        let run = store.recover().unwrap().remove(0);

        let rulings = runtime.rulings(&run, &Grant::all(), |_, _| {
            crate::reattach::Observation::Unsupported
        });
        let step = rulings
            .reconcile
            .iter()
            .find(|ruling| ruling.subject == "step:work")
            .unwrap();
        assert_eq!(step.ruling, crate::reconcile::Reconciliation::NeedsDecision);
        assert_eq!(step.effects, None);

        // The same shape, pinned to the real program: a delegate step's
        // unknown mark under a grant without delegation refuses replay.
        let program = runtime.survey.programs.get("writes-work").unwrap().clone();
        store
            .claim(&Claim {
                run: "run-3",
                base: "",
                program: &crate::child::digest(&program),
                questions: &[],
                sources: &[],
            })
            .unwrap();
        store
            .advance("run-3", Mark::run(State::Dispatched))
            .unwrap();
        store
            .advance("run-3", Mark::step("work", State::Dispatched))
            .unwrap();
        let run = store
            .recover()
            .unwrap()
            .into_iter()
            .find(|run| run.run == "run-3")
            .unwrap();
        let rulings = runtime.rulings(
            &run,
            &Grant::selected(Some("writes-work"), Some("reads")),
            |_, _| crate::reattach::Observation::Unsupported,
        );
        let step = rulings
            .reconcile
            .iter()
            .find(|ruling| ruling.subject == "step:work")
            .unwrap();
        assert_eq!(
            step.ruling,
            crate::reconcile::Reconciliation::OutsideAuthority
        );
    }

    /// A program file under `dir`, for a registry to open.
    fn stage_program(dir: &Path, slug: &str, steps: &str) {
        std::fs::write(
            dir.join(format!("{slug}.json")),
            serde_json::to_string(&json!({
                "v": 1, "slug": slug, "name": slug,
                "steps": serde_json::from_str::<Value>(steps).unwrap()
            }))
            .unwrap(),
        )
        .unwrap();
    }

    /// The `program` step's JSON with the propagation table every
    /// well-formed one states.
    fn call_step(address: &str, extra: &str) -> String {
        format!(
            r#"{{"name": "call", "kind": "program", "program": "{address}", "bounds": {{}},
                "propagation": {{"completed": "success", "failed": "failure", "refused": "refusal"}}{extra}}}"#
        )
    }

    /// A `program` step runs the child its address resolves to: the
    /// child's steps dispatch under `step/child` marks, report into the
    /// one run's record, and the step answers with what the child came
    /// to — the whole composition in one run.
    #[tokio::test]
    async fn a_program_step_runs_its_child_under_prefixed_marks() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime.with_runstate(dir.path());
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}},
                {"name": "after", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        // The child's step finished before the step that ran it, and the
        // step after it ran under the parent's own name.
        assert_eq!(run.step_names(), ["call/gather", "call", "after"]);
        let output: Value = serde_json::from_str(&run.steps[1].output).unwrap();
        assert_eq!(output["child"], "child-program");
        assert_eq!(output["outcome"], "completed");

        let store = Store::open(dir.path()).unwrap();
        let ids = claimed(dir.path());
        assert_eq!(ids.len(), 1, "{ids:?}");
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.outcome, Some(runstate::Outcome::Answered));
        for name in ["call", "call/gather", "after"] {
            let step = record
                .steps
                .iter()
                .find(|step| step.step == name)
                .unwrap_or_else(|| panic!("no step record for {name}"));
            assert_eq!(step.state, State::Answered, "{name}");
        }
    }

    /// A child that ends refused lands on the parent step where the
    /// propagation table says: `refused` mapped to `failure` stops the
    /// run at the step that called it.
    #[tokio::test]
    async fn a_childs_refusal_lands_where_the_table_maps_it() {
        let programs = tempfile::tempdir().unwrap();
        // The child's query reads the request source; a request with no
        // task list refuses `no_tasks` at dispatch.
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "failure"}},
                {"name": "after", "kind": "query", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("a sentence with no list", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run stopped");
        assert_eq!(stopped.step, "call");
        assert_eq!(stopped.code, "child_failed");
        assert!(run.step_names().iter().all(|name| name != "after"));
    }

    /// The table may just as honestly map `refused` to `success`: the
    /// parent's run goes on, and the step's output says what the child
    /// came to rather than pretending it completed.
    #[tokio::test]
    async fn a_refused_child_mapped_to_success_lets_the_parent_go_on() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "success"}},
                {"name": "after", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "success"}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("a sentence with no list", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(run.step_names(), ["call", "after"]);
        let output: Value = serde_json::from_str(&run.steps[0].output).unwrap();
        assert_eq!(output["outcome"], "refused");
    }

    /// A composition the check cannot account for refuses at admission —
    /// before the first step, before any record exists: a chain that
    /// returns to itself names the whole cycle.
    #[tokio::test]
    async fn a_composition_cycle_is_refused_before_anything_ran() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "a",
            &format!("[{}]", call_step("b@1.0.0", "")),
        );
        stage_program(
            programs.path(),
            "b",
            &format!("[{}]", call_step("a@1.0.0", "")),
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime.with_runstate(dir.path());
        let parent = runtime.survey.programs.get("a").unwrap().clone();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run refused");
        assert_eq!(stopped.code, "composition_refused");
        assert!(stopped.reason.contains("a -> b -> a"), "{stopped}");
        assert!(!dir.path().exists() || claimed(dir.path()).is_empty());
    }

    /// Every way a child address fails is refused the same way — before
    /// a step runs: a digest nothing answers, a bare name nobody pinned,
    /// and a release that is not in the registry.
    #[tokio::test]
    async fn a_child_address_the_registry_cannot_pin_is_refused() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let inputs = Inputs::read("do the list\n- one", "stub-local");
        for address in [
            format!("sha256:{}", "0".repeat(64)),
            "child-program".to_string(),
            "missing-child@9.9.9".to_string(),
        ] {
            let parent: Program = serde_json::from_value(json!({
                "v": 1, "slug": "parent-program",
                "steps": [serde_json::from_str::<Value>(&call_step(&address, "")).unwrap()]
            }))
            .unwrap();
            let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
            let stopped = run.stopped.as_ref().expect("the run refused");
            assert_eq!(stopped.code, "composition_refused", "{address}");
        }
    }

    /// A child bound wider than what the composition has left refuses —
    /// narrowed is admitted, widened is never clamped.
    #[tokio::test]
    async fn a_child_bound_wider_than_the_composition_has_left_is_refused() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0",
                 "bounds": {"steps": 256},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run refused");
        assert_eq!(stopped.code, "composition_refused");
        assert!(stopped.reason.contains("narrows"), "{stopped}");
    }

    /// A `program` step that states no propagation table refuses: the
    /// mapping is the document's to state and the host's never to
    /// default.
    #[tokio::test]
    async fn a_program_step_without_propagation_is_refused() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run refused");
        assert_eq!(stopped.code, "composition_refused");
        assert!(stopped.reason.contains("propagation"), "{stopped}");
    }

    /// A step kind the host does not run refuses the whole composition
    /// at admission — a `module` step inside a child is the parent's
    /// refusal before the first step, not the child's mid-run.
    #[tokio::test]
    async fn a_module_step_inside_a_child_refuses_the_composition() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "plug", "kind": "module", "module": {"sha256": "abc"}, "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [serde_json::from_str::<Value>(&call_step("child-program@1.0.0", "")).unwrap()]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run refused");
        assert_eq!(stopped.code, "step_kind_unavailable");
        assert!(stopped.reason.contains("module"), "{stopped}");
    }

    /// The caller's budget spends across the composition: a step count
    /// the child's second step would pass cancels the run there — the
    /// answered child step keeps its mark, the pending one and the
    /// program step mark `cancelled`, and nothing is relabelled.
    #[tokio::test]
    async fn a_budget_spent_inside_a_child_cancels_the_run() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[
                {"name": "one", "kind": "query", "bounds": {}},
                {"name": "two", "kind": "query", "bounds": {}}
            ]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime.with_runstate(dir.path()).with_budget(Budget {
            max_steps: Some(1),
            deadline: None,
            spend: None,
        });
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [serde_json::from_str::<Value>(&call_step("child-program@1.0.0", "")).unwrap()]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run cancelled");
        assert_eq!(stopped.code, BUDGET_EXCEEDED);

        let store = Store::open(dir.path()).unwrap();
        let ids = claimed(dir.path());
        let record = store.get(&ids[0]).unwrap().unwrap();
        assert_eq!(record.outcome, Some(runstate::Outcome::Cancelled));
        for (name, state) in [
            ("call/one", State::Answered),
            ("call/two", State::Cancelled),
            ("call", State::Cancelled),
        ] {
            let step = record
                .steps
                .iter()
                .find(|step| step.step == name)
                .unwrap_or_else(|| panic!("no step record for {name}"));
            assert_eq!(step.state, state, "{name}");
        }
    }

    /// A spend ceiling holds only where the work prices itself: a hard
    /// bound over lanes this host cannot meter refuses at admission,
    /// before a step marks anything — the same door every other
    /// unenforceable bound answers at.
    #[tokio::test]
    async fn a_hard_spend_ceiling_over_unpriced_lanes_refuses_at_admission() {
        let (dir, program) = asked();
        let mut runtime = empty_runtime().with_budget(Budget {
            deadline: None,
            max_steps: None,
            spend: Some(crate::spend::Bound::Hard(1_000_000)),
        });
        runtime.questions = questions::Registry::open(&[dir.path().to_path_buf()]);
        let refused = runtime.admit(&program).unwrap_err();
        assert_eq!(refused.step, "judge");
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains("cannot price"), "{refused}");
    }

    /// A soft bound asks for nothing it cannot get: the book opens,
    /// each priced step's charge records `unknown` — never zero, never
    /// an estimate — and the run keeps what the ledger held even when
    /// the step itself could not answer.
    #[tokio::test]
    async fn a_soft_spend_bound_records_what_nobody_could_price() {
        let (dir, program) = asked();
        let mut runtime = empty_runtime()
            .asking(Some(dead_door()))
            .with_budget(Budget {
                deadline: None,
                max_steps: None,
                spend: Some(crate::spend::Bound::Soft(1_000_000)),
            });
        runtime.questions = questions::Registry::open(&[dir.path().to_path_buf()]);
        let run = runtime
            .run(
                &program,
                &Inputs::read("judge it", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        // The door at 127.0.0.1:1 never answers, but the charge was
        // booked before the step dispatched.
        let book = run
            .spend
            .iter()
            .find(|book| book.lane() == crate::spend::Lane::Decision)
            .expect("a decision lane's book");
        assert_eq!(book.unknown_charges(), 1);
        assert_eq!(book.metered_micros(), 0);
        assert!(!book.over_soft());
    }

    /// A child's declared ceiling answers before one of its steps
    /// marks: an ask wider than the room the run's bound leaves is the
    /// child's refusal — the propagation table's `refused` row —
    /// never a silent clamp.
    #[tokio::test]
    async fn a_child_spend_ceiling_wider_than_the_room_is_the_childs_refusal() {
        let (questions_dir, _) = asked();
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "priced-child",
            r#"[{"name": "judge", "kind": "decide", "question": "test.profile-door.v1", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime()
            .asking(Some(dead_door()))
            .with_budget(Budget {
                deadline: None,
                max_steps: None,
                spend: Some(crate::spend::Bound::Soft(100)),
            });
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "priced-child@1.0.0",
                 "bounds": {"spend": 200},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();

        let run = runtime
            .run(
                &parent,
                &Inputs::read("do it", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        let stopped = run.stopped.expect("the widened ask stops the run");
        assert_eq!(stopped.code, "child_refused");
        assert!(stopped.reason.contains("wider than"), "{stopped}");
    }

    /// A ceiling over lanes nobody can price is a guarantee the host
    /// cannot give: the child's declared spend refuses before a child
    /// step marks, and the table's `refused` row decides what the run
    /// does with that answer.
    #[tokio::test]
    async fn a_child_spend_ceiling_over_unpriced_lanes_is_refused() {
        let (questions_dir, _) = asked();
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "priced-child",
            r#"[{"name": "judge", "kind": "decide", "question": "test.profile-door.v1", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime().asking(Some(dead_door()));
        runtime.questions = questions::Registry::open(&[questions_dir.path().to_path_buf()]);
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "priced-child@1.0.0",
                 "bounds": {"spend": 50},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();

        let run = runtime
            .run(
                &parent,
                &Inputs::read("do it", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        let stopped = run.stopped.expect("the unpriceable ceiling refuses");
        assert_eq!(stopped.code, "child_refused");
        assert!(stopped.reason.contains("cannot price"), "{stopped}");
    }

    /// A spend ceiling over steps that price nothing holds vacuously:
    /// no lane opens, nothing is refused, and the child runs.
    #[tokio::test]
    async fn a_spend_ceiling_over_steps_that_price_nothing_holds() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "free-child",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "free-child@1.0.0",
                 "bounds": {"spend": 100},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();

        let run = runtime
            .run(
                &parent,
                &Inputs::read("do the list\n- one", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
    }

    /// A child that writes runs its delegation inside the parent's
    /// run: the steps mark `call/pick` and `call/work` on the shared
    /// record, the delegation lands on the run's own list, and the
    /// checkout the work kept is a claim the run reports where it left
    /// it.
    #[tokio::test]
    async fn a_child_delegates_writing_work_inside_the_parents_run() {
        if !boundary_supported() || !boundary_enforces() {
            return;
        }
        let Some(repo) = scratch_repository() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "answers", "echo done");
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "writer-child",
            r#"[
                {"name": "pick", "kind": "query", "bounds": {}},
                {"name": "work", "kind": "delegate",
                 "bounds": {"minutes": 5, "isolation": "worktree"}}
            ]"#,
        );
        let mut runtime = stub_runtime(repo.path(), &binary, dir.path());
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "writer-child@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
            ]
        }))
        .unwrap();
        let mut task = Task::asking("write it");
        task.writes = true;
        let inputs = Inputs {
            request: "write".to_string(),
            tasks: vec![task],
            executor: "stub-local".to_string(),
        };

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(run.step_names(), ["call/pick", "call/work", "call"]);
        assert_eq!(run.delegations.len(), 1);
        assert_eq!(run.delegations[0].status, Status::Answered);

        let store = Store::open(dir.path()).unwrap();
        let ids = claimed(dir.path());
        let record = store.get(&ids[0]).unwrap().unwrap();
        for name in ["call/pick", "call/work", "call"] {
            let step = record
                .steps
                .iter()
                .find(|step| step.step == name)
                .unwrap_or_else(|| panic!("no step record for {name}"));
            assert_eq!(step.state, State::Answered, "{name}");
        }
    }

    /// `spend` and `minutes` are bounds a `program` step may declare —
    /// a ceiling in micros and the child's own deadline — each a count,
    /// and anything else refuses at admission.
    #[test]
    fn a_program_step_admits_spend_and_minutes_bounds() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent = |bounds: Value| -> Program {
            serde_json::from_value(json!({
                "v": 1, "slug": "parent-program",
                "steps": [
                    {"name": "call", "kind": "program", "program": "child-program@1.0.0",
                     "bounds": bounds,
                     "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"}}
                ]
            }))
            .unwrap()
        };
        runtime
            .admit(&parent(json!({"spend": 500, "minutes": 5})))
            .unwrap();
        for bad in [
            json!({"spend": "lots"}),
            json!({"spend": 0}),
            json!({"minutes": "soon"}),
        ] {
            let refused = runtime.admit(&parent(bad)).unwrap_err();
            assert_eq!(refused.code, "bound_unenforceable", "{refused}");
        }
    }

    /// The inputs a `program` step's binding states are the child's own:
    /// a `request` literal is the sentence the child's `query` step
    /// reads, list and all.
    #[tokio::test]
    async fn a_program_steps_declared_inputs_project_the_childs_request() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"},
                 "binding": {"inputs": {"request": {"from": "literal", "value": "child work\n- the one the parent projected"}}}}
            ]
        }))
        .unwrap();
        // The parent's own request carries no work at all: the only list
        // the child's `query` could read is the projected one.
        let inputs = Inputs::read("no list here", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        let gather = run
            .steps
            .iter()
            .find(|step| step.name == "call/gather")
            .expect("the child's step ran");
        assert!(gather.output.contains("1 of 1"), "{gather:?}");
    }

    /// A field a `program` step produces is one a later step's binding
    /// reads: `outcome` produced by the first call feeds the second
    /// call's projected request, and the run goes on because the
    /// producer declared it.
    #[tokio::test]
    async fn a_produced_field_flows_into_the_next_childs_inputs() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "s1", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "failure"},
                 "binding": {"produces": {"outcome": "word"}, "exposes": ["outcome"]}},
                {"name": "s2", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "success"},
                 "binding": {"inputs": {"request": {"from": "step", "step": "s1", "field": "outcome"}}}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        // s1 produced `outcome` — completed — and s2's child read it as
        // its request, which holds no list, so the child refused and the
        // table mapped that to success.
        let s1 = run
            .steps
            .iter()
            .find(|step| step.name == "s1")
            .expect("s1 ran");
        let output: Value = serde_json::from_str(&s1.output).unwrap();
        assert_eq!(output["outcome"], "completed");
        let s2 = run
            .steps
            .iter()
            .find(|step| step.name == "s2")
            .expect("s2 ran");
        let output: Value = serde_json::from_str(&s2.output).unwrap();
        assert_eq!(output["outcome"], "refused");
    }

    /// A `produces` field nothing in the child produced is the contract
    /// the document stated and the run could not meet — refused, never
    /// filled with a value nobody produced.
    #[tokio::test]
    async fn a_produces_field_nothing_produced_is_refused() {
        let programs = tempfile::tempdir().unwrap();
        stage_program(
            programs.path(),
            "child-program",
            r#"[{"name": "gather", "kind": "query", "bounds": {}}]"#,
        );
        let mut runtime = empty_runtime();
        runtime.survey.programs = crate::program::Registry::open(&[programs.path().to_path_buf()]);
        let parent: Program = serde_json::from_value(json!({
            "v": 1, "slug": "parent-program",
            "steps": [
                {"name": "call", "kind": "program", "program": "child-program@1.0.0", "bounds": {},
                 "propagation": {"completed": "success", "failed": "failure", "refused": "refusal"},
                 "binding": {"produces": {"missing": "word"}}}
            ]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&parent, &inputs, &Grant::all(), None).await;
        let stopped = run.stopped.as_ref().expect("the run refused");
        assert_eq!(stopped.code, "output_unproduced");
        assert!(stopped.reason.contains("missing"), "{stopped}");
    }

    /// A delegation that comes back as the harness's — a cleanup that
    /// failed, a binary that never spawned — is its own mark on the
    /// task's record: `unverifiable`, what nobody can read an answer out
    /// of. It never smudges the step's `cancelled`, which stays the end
    /// the caller chose.
    #[tokio::test]
    async fn a_cleanup_failure_is_its_own_mark() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "work", "kind": "delegate", "bounds": {}}]
        }))
        .unwrap();
        let mut delegation = delegation(Task::asking("anything"), "");
        delegation.status = Status::Harness("the checkout would not close".to_string());
        delegation.retained = Some(PathBuf::from("/claimed/checkout"));
        let run = Run {
            program: Some("burn-down".to_string()),
            delegations: vec![delegation],
            ..Run::default()
        };

        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-cleanup",
                base: "",
                program: "burn-down",
                questions: &[],
                sources: &[],
            })
            .unwrap();
        let mut record = Some((store, "run-cleanup".to_string()));
        runtime.cancel_from(&mut record, &run, &program, 0, None);

        let store = Store::open(dir.path()).unwrap();
        let record = store.get("run-cleanup").unwrap().unwrap();
        let step = record
            .steps
            .iter()
            .find(|step| step.step == "work")
            .unwrap();
        assert_eq!(step.state, State::Cancelled);
        let attempt = record
            .tasks
            .iter()
            .find(|task| task.task == "t1")
            .expect("the harness's delegation is marked on the task's record");
        assert_eq!(attempt.state, State::Unverifiable);
        assert_eq!(
            attempt.worktree.as_deref(),
            Some(Path::new("/claimed/checkout"))
        );
    }

    /// Without a runstate directory a run opens no store: nothing is
    /// created anywhere the run could reach.
    #[tokio::test]
    async fn a_run_without_runstate_creates_nothing() {
        let workspace = tempfile::tempdir().unwrap();
        let survey = Survey {
            capabilities: Vec::new(),
            programs: crate::program::Registry::open(&[]),
            sources: source::Registry::open(&[]),
            workspace: workspace.path().into(),
        };
        let runtime = Runtime::using(survey, None);
        assert!(runtime.runstate.is_none());
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::all(), None).await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert!(
            std::fs::read_dir(workspace.path())
                .unwrap()
                .next()
                .is_none()
        );
    }

    /// An unauthorized run stops before the first step — and before the
    /// lookup that would read the request — and the record says the grant
    /// was the reason.
    #[tokio::test]
    async fn an_ungranted_program_stops_before_it_reads_anything() {
        let runtime = empty_runtime();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "select", "kind": "query", "bounds": {}}]
        }))
        .unwrap();
        let inputs = Inputs::read("do the list\n- one", "stub-local");

        let run = runtime.run(&program, &inputs, &Grant::none(), None).await;
        assert!(run.steps.is_empty(), "{:?}", run.step_names());
        let stopped = run.stopped.expect("nothing was granted");
        assert_eq!(stopped.code, program_authority::UNAUTHORIZED);
    }

    /// A bounded fixture suite adapter and the plan that runs it, shaped
    /// the way `crate::verification`'s tests shape theirs.
    fn suite_fixture(
        command: &str,
        acceptance: crate::verification::Acceptance,
    ) -> (
        tempfile::TempDir,
        tempfile::TempDir,
        crate::verification::Plan,
    ) {
        let host = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("candidate.txt"), "immutable\n").unwrap();
        let manifest = host.path().join("suite.json");
        std::fs::write(
            &manifest,
            serde_json::to_vec(&json!({
                "v":1,"slug":"suite-fixture","name":"Suite fixture","transport":"subprocess",
                "detect":{"binary":"/bin/sh","version":["/bin/sh","--version"]},
                "enforces":[],"cannot_enforce":[],"sees_repository":true,
                "cost":"local","invoke":["/bin/sh"],"isolation":["directory"]
            }))
            .unwrap(),
        )
        .unwrap();
        let entry = capability::Entry::load(&manifest, capability::Source::Operator).unwrap();
        let plan = crate::verification::Plan {
            schema: crate::verification::SCHEMA.into(),
            input_digest: "candidate-digest".into(),
            seconds: 10,
            allow_unrestricted_reads: true,
            allow_network: true,
            checks: vec![crate::verification::Check {
                id: "suite".into(),
                manifest,
                manifest_digest: entry.digest,
                arguments: vec!["-c".into(), command.into()],
                seconds: 3,
                output_bytes: 4096,
                acceptance,
            }],
        };
        (host, workspace, plan)
    }

    /// The repository's run-suite step shape: one gated check requiring
    /// typed suite evidence under a stated test budget.
    fn suite_program() -> Program {
        serde_json::from_value(json!({
            "v":1,"slug":"run-suite",
            "steps":[{"name":"score","kind":"check",
                      "bounds":{"refuse_on":"gate_not_met","acceptance":"suite","max_tests":4}}]
        }))
        .unwrap()
    }

    fn suite_acceptance() -> crate::verification::Acceptance {
        crate::verification::Acceptance::Suite {
            suite_digest: "suite-1".into(),
            input_digest: "candidate-digest".into(),
        }
    }

    /// The adapter's complete stdout: typed suite evidence in the shape
    /// `crate::verification::SuiteEvidence` judges.
    fn suite_evidence(suite: &str, input: &str, verdict: &str) -> String {
        let evidence = json!({
            "schema": crate::verification::SCHEMA,
            "suite_digest": suite,
            "input_digest": input,
            "verdict": verdict,
        });
        format!("printf '%s' '{evidence}'")
    }

    fn suite_runtime(
        workspace: &Path,
        plan: crate::verification::Plan,
        trust: capability::Trust,
    ) -> Runtime {
        let survey = Survey {
            capabilities: Vec::new(),
            programs: crate::program::Registry::open(&[]),
            sources: source::Registry::open(&[]),
            workspace: workspace.into(),
        };
        Runtime::using(survey, None).with_verification(workspace.into(), plan, trust)
    }

    fn supported() -> bool {
        if crate::delegate::boundary_supported() && boundary_enforces() {
            true
        } else {
            eprintln!("skipping: verification needs an enforcing filesystem boundary");
            false
        }
    }

    /// A gated check runs only the evidence the step asks for. No plan, a
    /// plan whose checks answer with the wrong kind, and a plan naming
    /// more than one suite all refuse before any check runs — a bare exit
    /// status cannot satisfy a requested typed suite.
    #[test]
    fn a_requested_suite_refuses_plans_that_cannot_answer_with_it() {
        let program = suite_program();
        let refused = empty_runtime().admit(&program).unwrap_err();
        assert_eq!(refused.step, "score");
        assert_eq!(refused.code, "check_unavailable");

        let (_host, workspace, plan) =
            suite_fixture("true", crate::verification::Acceptance::ExitSuccess);
        let refused = suite_runtime(workspace.path(), plan, capability::Trust::everything())
            .admit(&program)
            .unwrap_err();
        assert_eq!(refused.code, "check_unavailable");
        assert!(refused.reason.contains("suite"), "{refused}");

        let (_host, workspace, mut plan) = suite_fixture("true", suite_acceptance());
        let mut second = plan.checks[0].clone();
        second.id = "other".into();
        second.acceptance = crate::verification::Acceptance::Suite {
            suite_digest: "suite-2".into(),
            input_digest: "candidate-digest".into(),
        };
        plan.checks.push(second);
        let refused = suite_runtime(workspace.path(), plan, capability::Trust::everything())
            .admit(&program)
            .unwrap_err();
        assert_eq!(refused.code, "check_unavailable");
        assert!(refused.reason.contains("more than one suite"), "{refused}");
    }

    /// The requirement reads both ways: suite evidence cannot satisfy a
    /// step that asked for a reviewed command's exit status either.
    #[test]
    fn a_requested_exit_status_refuses_suite_evidence() {
        let program: Program = serde_json::from_value(json!({
            "v":1,"slug":"verify-fixture",
            "steps":[{"name":"gate","kind":"check",
                      "bounds":{"refuse_on":"gate_not_met","acceptance":"exit-success"}}]
        }))
        .unwrap();
        let (_host, workspace, plan) = suite_fixture("true", suite_acceptance());
        let refused = suite_runtime(workspace.path(), plan, capability::Trust::everything())
            .admit(&program)
            .unwrap_err();
        assert_eq!(refused.code, "check_unavailable");
    }

    /// The new bounds are held the way every other bound is: words and
    /// counts the host cannot read refuse at admission, the test budget
    /// holds the plan's check count, and evidence bounds mean nothing on
    /// the delegation admission check.
    #[test]
    fn suite_bounds_refuse_what_the_host_cannot_hold() {
        for bounds in [
            json!({"refuse_on":"gate_not_met","acceptance":"vibes"}),
            json!({"refuse_on":"gate_not_met","max_tests":0}),
            json!({"refuse_on":"gate_not_met","max_tests":"four"}),
        ] {
            let program: Program = serde_json::from_value(json!({
                "v":1,"slug":"run-suite",
                "steps":[{"name":"score","kind":"check","bounds":bounds}]
            }))
            .unwrap();
            assert_eq!(
                empty_runtime().admit(&program).unwrap_err().code,
                "bound_unenforceable",
                "{bounds}"
            );
        }
        let program: Program = serde_json::from_value(json!({
            "v":1,"slug":"run-suite",
            "steps":[{"name":"score","kind":"check",
                      "bounds":{"refuse_on":"gate_not_met","acceptance":"suite","max_tests":1}}]
        }))
        .unwrap();
        let (_host, workspace, mut plan) = suite_fixture("true", suite_acceptance());
        let mut second = plan.checks[0].clone();
        second.id = "other".into();
        plan.checks.push(second);
        let refused = suite_runtime(workspace.path(), plan, capability::Trust::everything())
            .admit(&program)
            .unwrap_err();
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains("test budget"), "{refused}");

        let program: Program = serde_json::from_value(json!({
            "v":1,"slug":"pairing",
            "steps":[
                {"name":"admit","kind":"check",
                 "bounds":{"refuse_on":"cannot_enforce_intersection","acceptance":"suite"}},
                {"name":"work","kind":"delegate","bounds":{}}]
        }))
        .unwrap();
        let refused = empty_runtime().admit(&program).unwrap_err();
        assert_eq!(refused.step, "admit");
        assert_eq!(refused.code, "bound_unenforceable");
    }

    /// A typed suite passes on matching evidence and stops the program on
    /// a failed or unverifiable verdict, and neither becomes a pass.
    #[tokio::test]
    async fn a_typed_suite_passes_fails_and_stays_unverifiable_on_its_evidence() {
        if !supported() {
            return;
        }
        let program = suite_program();
        let inputs = Inputs::read("Run the pinned suite.", "");
        for (verdict, expected, finished) in [
            ("passed", crate::verification::Verdict::Passed, true),
            ("failed", crate::verification::Verdict::Failed, false),
            (
                "unverifiable",
                crate::verification::Verdict::Unverifiable,
                false,
            ),
        ] {
            let (_host, workspace, plan) = suite_fixture(
                &suite_evidence("suite-1", "candidate-digest", verdict),
                suite_acceptance(),
            );
            let run = suite_runtime(workspace.path(), plan, capability::Trust::everything())
                .run(&program, &inputs, &Grant::all(), None)
                .await;
            assert_eq!(run.finished(), finished, "{verdict}: {:?}", run.stopped);
            assert_eq!(run.verification.len(), 1);
            assert_eq!(run.verification[0].verdict, expected, "{verdict}");
            if !finished {
                assert_eq!(run.stopped.as_ref().unwrap().code, "gate_not_met");
            }
        }
    }

    /// Evidence that is missing or names another suite or input is
    /// unverifiable, never a pass — a successful exit cannot stand in for
    /// the pinned identities.
    #[tokio::test]
    async fn missing_or_mismatched_suite_identity_never_passes() {
        if !supported() {
            return;
        }
        let program = suite_program();
        let inputs = Inputs::read("Run the pinned suite.", "");
        for command in [
            "true".to_string(),
            suite_evidence("another-suite", "candidate-digest", "passed"),
            suite_evidence("suite-1", "another-input", "passed"),
        ] {
            let (_host, workspace, plan) = suite_fixture(&command, suite_acceptance());
            let run = suite_runtime(workspace.path(), plan, capability::Trust::everything())
                .run(&program, &inputs, &Grant::all(), None)
                .await;
            let stopped = run.stopped.expect("mismatched evidence cannot pass");
            assert_eq!(stopped.code, "gate_not_met", "{command}");
            assert_eq!(
                run.verification[0].verdict,
                crate::verification::Verdict::Unverifiable,
                "{command}"
            );
        }
    }

    /// The suite's adapter is a capability: without an approval covering
    /// it the check refuses before it runs, and the run records no verdict.
    #[tokio::test]
    async fn an_unapproved_suite_adapter_never_runs() {
        if !supported() {
            return;
        }
        let program = suite_program();
        let inputs = Inputs::read("Run the pinned suite.", "");
        let (_host, workspace, plan) = suite_fixture(
            &suite_evidence("suite-1", "candidate-digest", "passed"),
            suite_acceptance(),
        );
        let run = suite_runtime(workspace.path(), plan, capability::Trust::empty())
            .run(&program, &inputs, &Grant::all(), None)
            .await;
        let stopped = run.stopped.expect("an unapproved adapter cannot run");
        assert_eq!(stopped.code, "verification_unverifiable");
        assert!(run.verification.is_empty());
    }

    /// The program the repository ships asks for typed suite evidence: a
    /// host with no plan refuses it at admission, and a host with a
    /// matching suite plan runs it end to end.
    #[tokio::test]
    async fn the_shipped_run_suite_runs_a_pinned_suite() {
        let program = crate::program::Program::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../programs/run-suite.json"),
        )
        .expect("the repository's run-suite reads");
        assert_eq!(program.step_names(), ["score"]);
        let refused = empty_runtime().admit(&program).unwrap_err();
        assert_eq!(refused.step, "score");
        assert_eq!(refused.code, "check_unavailable");
        if !supported() {
            return;
        }
        let inputs = Inputs::read("Run the pinned suite.", "");
        let (_host, workspace, plan) = suite_fixture(
            &suite_evidence("suite-1", "candidate-digest", "passed"),
            suite_acceptance(),
        );
        let run = suite_runtime(workspace.path(), plan, capability::Trust::everything())
            .run(&program, &inputs, &Grant::all(), None)
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(
            run.verification[0].verdict,
            crate::verification::Verdict::Passed
        );
    }
}
