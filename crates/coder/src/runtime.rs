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
//! does not run — `program` and `module`, which are composition and
//! WebAssembly — is refused here, at admission. Neither is ever skipped: a
//! program whose unknown steps are skipped is a different program, and it
//! is the one a host would run by accident.
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
//! Four step kinds: `query`, `decide`, `check`, and `delegate`.
//! Composition, `module`, cycle detection, depth bounds, and fetching are
//! all specified and none is needed to run the first program. A runtime
//! that runs one program correctly is worth more than one that describes
//! five.
//!
//! A `check` step gated on `gate_not_met` runs the operator-installed
//! verification plan rather than anything the program carries. Its
//! `acceptance` bound names the evidence every check in the plan must
//! produce — a typed suite verdict, never a bare exit status where a
//! suite was asked for — and `max_tests` bounds how many checks the plan
//! may run. Admission holds the plan to both before any check executes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use atif::{Call, Decision, Outcome};
use jev::{Answer, SystemOneRequest};
use serde_json::{Map, Value, json};

use crate::capability::{self, Presence};
use crate::delegate::{
    Bounds, Delegation, Delegator, Isolation, Task, Verdict, boundary_supported,
};
use crate::program::{Kind, Program, Step};
use crate::program_authority::{self, Effects, Grant};
use crate::questions::{self, Fill, Set};
use crate::relay::RelayDoor;
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
        // Composition and WebAssembly are specified and not built. A host
        // that met one and ran the rest would be running a different
        // program.
        Kind::Program | Kind::Module => &[],
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
        for step in &program.steps {
            self.admit_step(program, step)?;
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
                &Fill::Options(options),
                trace,
                |read| format!("program {read}"),
            )
            .await
            .map_err(|reason| Refused::at("", "door_unavailable", reason))?;
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
        match self.survey.programs.get(&choice).is_some() {
            true => Ok(Selected::Program(choice)),
            false => Err(Refused::at(
                "",
                "no_program_chosen",
                format!("the door named {choice}, which this host did not resolve"),
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

    /// The effects each step of a program declares, derived from what the
    /// step does rather than stated in the program — a program's own
    /// words are a claim, and the grant is held against what the step
    /// will do.
    fn declared_effects<'a>(
        &self,
        program: &'a Program,
        inputs: &Inputs,
    ) -> Vec<(&'a Step, Effects)> {
        program
            .steps
            .iter()
            .map(|step| (step, self.step_effects(step, inputs)))
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
    fn step_effects(&self, step: &Step, inputs: &Inputs) -> Effects {
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
            // These kinds are refused at admission and never reach a grant.
            Kind::Program | Kind::Module => Effects::none(),
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
        let mut selection = Selection::default();
        for step in &program.steps {
            let outcome = match step.kind {
                Kind::Query => self
                    .look_up(step, inputs, trace.as_deref_mut())
                    .map(|found| {
                        let output = found.output();
                        selection = found.clone();
                        run.selection = Some(found);
                        output
                    }),
                Kind::Decide => {
                    self.decide(
                        step,
                        program,
                        inputs,
                        &selection,
                        &mut run,
                        trace.as_deref_mut(),
                    )
                    .await
                }
                Kind::Check
                    if step.bounds.get("refuse_on").and_then(Value::as_str)
                        == Some("gate_not_met") =>
                {
                    self.verify_step(step, &mut run, trace.as_deref_mut()).await
                }
                Kind::Check => self.check(step, program, inputs, trace.as_deref_mut()),
                Kind::Delegate => {
                    self.delegate(
                        step,
                        inputs,
                        &selection,
                        &mut run,
                        grant,
                        trace.as_deref_mut(),
                    )
                    .await
                }
                // Admission refused these before the first step ran.
                Kind::Program | Kind::Module => Err(Refused::at(
                    &step.name,
                    "step_kind_unavailable",
                    format!("this host does not run a {} step", step.kind.word()),
                )),
            };
            match outcome {
                Ok(output) => run.steps.push(Ran {
                    name: step.name.clone(),
                    kind: step.kind,
                    output,
                }),
                Err(refused) => {
                    run.stopped = Some(refused);
                    break;
                }
            }
        }
        self.report(&run, started, trace);
        run
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
            .await
            .map_err(|reason| Refused::at(&step.name, "door_unavailable", reason))?;
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
            Err(reason) => {
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
                return Err(Refused::at(&step.name, "door_unavailable", reason));
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
    async fn delegate(
        &self,
        step: &Step,
        inputs: &Inputs,
        selection: &Selection,
        run: &mut Run,
        grant: &Grant,
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
        let bounded: Vec<Task> = selection
            .tasks()
            .into_iter()
            .map(|task| {
                let mut task = task;
                task.isolation = isolation;
                if let Some(minutes) = minutes {
                    task.bounds = Bounds::minutes(minutes);
                }
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
    ) -> Result<jev::SystemOneResponse, String> {
        let door = self.door.as_ref().ok_or("no decision door is configured")?;
        let questions = set.build(fill)?;
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
                self.record_decision(trace, set, decision);
                Ok(response)
            }
            Err(error) => {
                decision.error = Some(error.to_string());
                self.record_decision(trace, set, decision);
                Err(error.to_string())
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
        if crate::delegate::boundary_supported() {
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
