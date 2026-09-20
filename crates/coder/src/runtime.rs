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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use atif::{Call, Decision, Outcome};
use jev::{Answer, SystemOneRequest};
use serde_json::{Map, Value, json};

use crate::delegate::{Bounds, Delegation, Delegator, Isolation, Task};
use crate::program::{Kind, Program, Step};
use crate::questions::{self, Fill, Set};
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

/// The bounds a `delegate` step names that the **host** holds to, rather
/// than handing to the executor.
///
/// Everything else on a `delegate` step is a bound the executor is asked
/// to keep, which is what the admission check tests against the
/// capability's `cannot_enforce`.
const HOST_BOUNDS: &[&str] = &["concurrent_max", "isolation"];

/// The one condition a `check` step's `refuse_on` may name here.
const INTERSECTION: &str = "cannot_enforce_intersection";

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
    /// The executor declares it keeps it.
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
        Kind::Decide => &["refuse_below", "requires_calibration", "per_requirement"],
        Kind::Check => &["refuse_on"],
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

    /// What the run comes to, in the sentence a reader sees last.
    #[must_use]
    pub fn summary(&self) -> String {
        let program = self.program.as_deref().unwrap_or("no program");
        let Some(stopped) = &self.stopped else {
            let (right, graded) = self.correct();
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
                "{program} ran its {} steps: {} delegations, {} answered, {right} of {graded} correct, \
                 in {wall:.1} seconds of wall clock against {summed:.1} seconds of summed agent time.",
                self.steps.len(),
                self.delegations.len(),
                self.answered(),
            );
        };
        format!("{program} stopped at {stopped}.")
    }
}

/// One machine, running programs.
pub struct Runtime {
    survey: Survey,
    questions: questions::Registry,
    door: Option<jev::Client>,
    repository: Option<PathBuf>,
    host: Host,
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
        Runtime {
            survey,
            questions: questions::Registry::open(&questions::search(repository)),
            door: jev::Client::from_env().ok(),
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
        Runtime {
            repository: Some(survey.workspace.clone()),
            survey,
            questions,
            door: jev::Client::from_env().ok(),
            host,
        }
    }

    /// Answers the decision questions through this door rather than the
    /// one the environment names.
    #[must_use]
    pub fn asking(mut self, door: Option<jev::Client>) -> Self {
        self.door = door;
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
                Some(INTERSECTION) => Ok(()),
                other => refuse(format!(
                    "this host runs no check that refuses on {}",
                    other.unwrap_or("that")
                )),
            },
            "on_overflow" => match value.as_str().and_then(OnOverflow::named) {
                Some(_) => Ok(()),
                None => refuse(format!(
                    "a lookup that answers with more than its bound truncates or refuses, and this step names {value}"
                )),
            },
            "max_results" | "concurrent_max" | "minutes" => match value.as_u64() {
                Some(count) if count > 0 => Ok(()),
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
            "requires_calibration" | "per_requirement" => match value.is_boolean() {
                true => Ok(()),
                false => refuse(format!(
                    "{bound} is true or false, and this step names {value}"
                )),
            },
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
        if per_requirement != set.templated() {
            return refuse(
                "bound_unenforceable",
                match per_requirement {
                    true => format!(
                        "{id} is one fixed set of questions, so it cannot be asked once per requirement"
                    ),
                    false => {
                        format!("{id} is asked once per requirement, and this step did not say to")
                    }
                },
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
        match self.door.is_some() {
            true => Ok(()),
            false => refuse(
                "door_unavailable",
                format!("no decision door is configured, and {id} has to be asked of one"),
            ),
        }
    }

    /// Whether this host can run what a `check` step names.
    fn admit_check(&self, program: &Program, step: &Step) -> Result<(), Refused> {
        if step.bounds.get("refuse_on").and_then(Value::as_str) != Some(INTERSECTION) {
            return Err(Refused::at(
                &step.name,
                "check_unavailable",
                "this host runs one check, and it is the one refuse_on names".to_string(),
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
    /// for an executor that is not here. `run-suite` names a check this
    /// host does not run and `review-changes` names a question set it has
    /// no wording for, so a machine carrying all four programs offers two.
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

    /// Runs one program, step by step, from its definition.
    ///
    /// Admission comes first, so a program this host cannot hold to fails
    /// before it has done anything. After that each step runs in the order
    /// the program lists, and the first refusal stops the rest.
    pub async fn run(
        &self,
        program: &Program,
        inputs: &Inputs,
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
                Kind::Check => self.check(step, program, inputs, trace.as_deref_mut()),
                Kind::Delegate => {
                    self.delegate(step, inputs, &selection, &mut run, trace.as_deref_mut())
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

    /// Selects the program a request asks for and runs it.
    ///
    /// A request that asks for no program stops here, reported the way any
    /// other run that did nothing is. A caller that has an ordinary turn to
    /// fall back on wants [`Runtime::select`] instead, so it can tell
    /// `none` from a refusal.
    pub async fn apply(&self, inputs: &Inputs, mut trace: Option<&mut Recorder>) -> Run {
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
        self.run(&program, inputs, trace).await
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

        // `requires_calibration` is the host promising the answer can be
        // scored later: a probability rather than a label, from a named
        // model, beside the digest of the wording that produced it. The
        // first two are checked here; the digest is recorded either way.
        if step
            .bounds
            .get("requires_calibration")
            .and_then(Value::as_bool)
            == Some(true)
        {
            let scorable = !gate.is_empty()
                && response.answers.get(&gate).and_then(probability).is_some()
                && !response.model.is_empty();
            if !scorable {
                return Err(Refused::at(
                    &step.name,
                    "uncalibrated",
                    format!(
                        "{id} must answer with a probability from a named model to be scored against an outcome, and this answer cannot be"
                    ),
                ));
            }
        }
        let Some(floor) = floor else {
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
                bound if manifest.enforces.iter().any(|named| named == bound) => {
                    Enforcement::Executor
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
                "refused: {} declares neither way about {{{}}}, and a bound nobody named is not enforced by having gone unmentioned",
                inputs.executor,
                unknown.join(", ")
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
        let Some(executor) = self.survey.executor(&inputs.executor) else {
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
            .unwrap_or(1) as usize;
        let minutes = step.bounds.get("minutes").and_then(Value::as_u64);
        let bounded: Vec<Task> = selection
            .tasks()
            .into_iter()
            .map(|task| {
                let mut task = task;
                task.isolation = isolation;
                if let Some(minutes) = minutes {
                    task.bounds = Bounds::minutes(minutes);
                }
                task
            })
            .collect();
        let delegator = Delegator::new(executor)
            .in_repository(
                self.repository
                    .clone()
                    .unwrap_or(self.survey.workspace.clone()),
            )
            .bounded_to(width);
        let delegations = delegator.fan_out(bounded).await;
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
    use super::*;

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
}
