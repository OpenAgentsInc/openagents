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
        Kind::Query => &["max_results"],
        Kind::Decide => &["refuse_below", "requires_calibration", "per_requirement"],
        Kind::Check => &["refuse_on"],
        Kind::Delegate => &["concurrent_max", "isolation", "minutes"],
        // Composition and WebAssembly are specified and not built. A host
        // that met one and ran the rest would be running a different
        // program.
        Kind::Program | Kind::Module => &[],
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
        Runtime {
            survey: Survey::read(repository, workspace),
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
            Kind::Decide => self.admit_question(step),
            Kind::Check => self.admit_check(program, step),
            _ => Ok(()),
        }
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

    /// Asks which program a request wants, from the ones this host
    /// resolved.
    ///
    /// The option set is built from the registry rather than written down,
    /// so an operator whose machine resolved three programs is offered
    /// three. A choice naming nothing the registry holds is refused rather
    /// than guessed at.
    ///
    /// # Errors
    ///
    /// Returns why no program was selected.
    pub async fn select(
        &self,
        request: &str,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
        let options: Vec<(String, String)> = self
            .survey
            .programs
            .programs()
            .iter()
            .map(|program| (program.slug.clone(), program.summary.clone()))
            .collect();
        if options.is_empty() {
            return Err(Refused::at(
                "",
                "no_programs",
                "this host resolved no programs, so there is none to select",
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
                |choice| format!("run program {choice}"),
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
        match self.survey.programs.get(&choice).is_some() {
            true => Ok(choice),
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
        let mut tasks: Vec<Task> = Vec::new();
        for step in &program.steps {
            let outcome = match step.kind {
                Kind::Query => {
                    self.select_tasks(step, inputs, trace.as_deref_mut())
                        .map(|selected| {
                            let output = format!("{} tasks", selected.len());
                            tasks = selected;
                            output
                        })
                }
                Kind::Decide => {
                    self.decide(
                        step,
                        program,
                        inputs,
                        &tasks,
                        &mut run,
                        trace.as_deref_mut(),
                    )
                    .await
                }
                Kind::Check => self.check(step, program, inputs, trace.as_deref_mut()),
                Kind::Delegate => {
                    self.delegate(step, inputs, &tasks, &mut run, trace.as_deref_mut())
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
    pub async fn apply(&self, inputs: &Inputs, mut trace: Option<&mut Recorder>) -> Run {
        let slug = match self.select(&inputs.request, trace.as_deref_mut()).await {
            Ok(slug) => slug,
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
    /// A query naming no source reads the program's declared inputs, which
    /// for `delegate-fan-out` is the task list the operator's sentence
    /// carried. `max_results` is the bound, and it is applied rather than
    /// recorded: a lookup that answered with more than the step allows is
    /// a step that ran unbounded.
    fn select_tasks(
        &self,
        step: &Step,
        inputs: &Inputs,
        trace: Option<&mut Recorder>,
    ) -> Result<Vec<Task>, Refused> {
        if inputs.tasks.is_empty() {
            return Err(Refused::at(
                &step.name,
                "no_tasks",
                "the lookup found no work, and a fan-out over nothing is not a fan-out",
            ));
        }
        let max = step
            .bounds
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX) as usize;
        let found = inputs.tasks.len();
        let tasks: Vec<Task> = inputs.tasks.iter().take(max).cloned().collect();
        let mut extra = self.step_extra(step);
        extra.insert("found".to_string(), json!(found));
        extra.insert("selected".to_string(), json!(tasks.len()));
        extra.insert(
            "tasks".to_string(),
            json!(
                tasks
                    .iter()
                    .enumerate()
                    .map(|(n, task)| task_value(n, task))
                    .collect::<Vec<_>>()
            ),
        );
        self.record(
            trace,
            &format!(
                "Looked up the work the request names: {} tasks.",
                tasks.len()
            ),
            Call {
                id: String::new(),
                name: SELECT_CALL.to_string(),
                arguments: json!({ "source": "request", "max_results": max }),
                output: format!("{} of {found} tasks", tasks.len()),
                outcome: Outcome::Completed,
                milliseconds: 0,
                purpose: Some(
                    "Find the work before asking whether it may run at once.".to_string(),
                ),
                extra,
            },
        );
        Ok(tasks)
    }

    /// A `decide` step: one typed question set put to a decision door.
    async fn decide(
        &self,
        step: &Step,
        program: &Program,
        inputs: &Inputs,
        tasks: &[Task],
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
                    requirements_state(run, tasks),
                    Fill::Requirements(requirements),
                )
            }
            false => (plan_state(program, inputs, tasks), Fill::None),
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
        tasks: &[Task],
        run: &mut Run,
        trace: Option<&mut Recorder>,
    ) -> Result<String, Refused> {
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
        let bounded: Vec<Task> = tasks
            .iter()
            .map(|task| {
                let mut task = task.clone();
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
        trace.answer(&run.summary(), None, started.elapsed().as_millis() as u64);
    }
}

/// The state a plan question reads: named fields rather than a sentence,
/// so a question can point at the tasks directly.
fn plan_state(program: &Program, inputs: &Inputs, tasks: &[Task]) -> Value {
    json!({
        "plan": {
            "program": program.slug,
            "request": inputs.request,
            "executor": inputs.executor,
            "tasks": tasks.len(),
        },
        "tasks": tasks
            .iter()
            .enumerate()
            .map(|(n, task)| task_value(n, task))
            .collect::<Vec<_>>(),
    })
}

/// The state a per-requirement question reads: one entry per delegation,
/// under the name its question carries.
fn requirements_state(run: &Run, tasks: &[Task]) -> Value {
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
    json!({ "requirements": requirements, "tasks": tasks.len() })
}

/// The names a per-requirement question set is asked under.
fn requirement_names(run: &Run) -> Vec<String> {
    (0..run.delegations.len()).map(requirement_name).collect()
}

/// One requirement's name. `t1` is what the golden's delegation ids use.
fn requirement_name(n: usize) -> String {
    format!("t{}", n + 1)
}

/// One task, as a state object and a trace record spell it.
fn task_value(n: usize, task: &Task) -> Value {
    json!({
        "id": requirement_name(n),
        "prompt": task.prompt,
        "reads": task.reads,
        "writes": task.writes,
    })
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
