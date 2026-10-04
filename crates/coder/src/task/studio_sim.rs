//! The Agent Studio's simulated team: a scripted `sim` route that drives
//! the coordinator ([`super::studio`]) against a scratch repository with
//! real worktrees, commits, reviews, and landings, on a fixed clock
//! (`docs/verse/agent-studio.md`, "Simulated team").
//!
//! The script ([`SCRIPT`]) takes one goal from submission to done: the
//! lead asks the person a question, then plans three tasks; a person's
//! message steers a worker; two workers edit the same line in parallel,
//! so the second landing meets a merge conflict that its seat resolves;
//! a reviewer requests a change, after which a merge at the earlier
//! review is refused as stale; and the last task asks for an approval
//! before it writes. No model is called, and every push stays in the
//! fixture's directory.
//!
//! The route is a test fixture. It runs only inside a [`Fixture`]: an
//! explicit configuration that creates its own bare `origin`, a checkout
//! of it, and a task store, and marks them. [`admit`] refuses the route
//! for any other repository, and [`ROUTE`] is not a provider that
//! `coder host autostart` or a seat's route accepts, so neither the
//! owner's auto-start policy nor a person can select it on a real
//! repository. The fixture's [`Team`] never notes a task eligible for
//! auto-start; its [`SimInbox`] stands in for the task inbox and is the
//! only thing that runs the fixture's tasks.
//!
//! The fixture's seats record a Codex-shaped route with the model
//! [`MODEL`], so the coordinator steers them as it steers a loop engine.
//! No engine reads that route.
//!
//! The same script also runs on a scratch host (#10572), so Verse and
//! `openagents studio` act on it through the host's real paths: the
//! studio intents, the coordinator's sweep, reviews, merges, and the
//! conflict flow. A [`Scratch`] makes the host's directories, all under
//! one empty directory: the scratch repository, a host root that names it
//! as the host's one workspace, and a task store that admits scripted
//! turns ([`owner::allow_scripted`]). Its [`Engine`] ends each queued
//! studio task's turn from the script through [`owner::scripted`], which
//! the task owner records like any run, with no model and no spend. The
//! engine refuses any root a [`Scratch`] did not mark, any store that
//! does not admit scripted turns (this computer's own store never does),
//! and a root whose auto-start policy is on, so it never runs beside a
//! real engine. `coder host serve --studio-sim` and
//! `openagents studio up --sim` start it ([`open_host`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use coder_host::access::review::TaskReview;
use serde::{Deserialize, Serialize};

use super::studio::{
    self, Delivery, Inbox, MemoryKind, NewGoal, Party, Progress, Repository, Role, Seat, Studio,
    View,
};
use super::{
    Action, COMMAND_SCHEMA, Command, Execution, Status, Store, Task, autostart, checks,
    interaction, owner, remote,
};

/// The route a seat would name to ask for the simulated team. No route
/// parser accepts it; only a [`Fixture`] runs it.
pub const ROUTE: &str = "sim:script";
/// The model the fixture's seats record.
pub const MODEL: &str = "studio-sim";
/// The marker document a fixture writes in its directory.
pub const MARKER: &str = "studio-sim.json";
/// The marker's schema.
pub const FIXTURE_SCHEMA: &str = "openagents.coder.studio-sim-fixture.v1";
/// The fixed clock's first reading, in Unix seconds.
pub const START: u64 = 1_790_000_000;
/// How far the fixed clock moves at each step, in seconds.
pub const TICK: u64 = 60;
/// The branch the scratch repository lands on.
pub const BRANCH: &str = "main";
/// The scratch repository's workspace label.
pub const LABEL: &str = "scratch";
/// The goal the script submits.
pub const GOAL: &str = "Greet with \"Hello, studio\" and document the greeting.";
/// The person's answer to the lead's question.
pub const ANSWER: &str = "Use \"Hello, studio\" exactly, with a capital H.";
/// The person's message to a worker while its task is queued.
pub const STEER: &str = "Keep the README's status line to one line.";
/// The reviewer's request for changes.
pub const CHANGES: &str = "Name greeting.txt in a page under docs/.";
/// The person's answer to the approval.
pub const ALLOW: &str = "Allow once.";
/// The line both workers edit, so the second landing conflicts.
const STATUS_LINE: &str = "Status: draft";
/// The most diff bytes a review reads.
const REVIEW_MAX: usize = 32 * 1024;

/// One turn the scripted engine takes for a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// End the turn with a question for the person.
    Ask(&'static str),
    /// End the turn asking the person to approve a step.
    Approve(&'static str),
    /// End the turn with the team's plan as its reply.
    Plan,
    /// Write files in the task's worktree, commit them under the seat's
    /// identity, and finish.
    Edit {
        files: &'static [(&'static str, &'static str)],
        message: &'static str,
    },
}

/// One step of the script, each at the next tick of the fixed clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The person submits [`GOAL`].
    Submit,
    /// The engine takes the next turn of the task with this plan
    /// identity (`lead` for the lead's task).
    Run(&'static str),
    /// The person answers the task's open question or approval.
    Answer(&'static str, &'static str),
    /// The person messages a seat.
    Message(&'static str, &'static str),
    /// The host's coordinator pass.
    Reconcile,
    /// The host restarts: the coordinator is dropped and opened again.
    Restart,
    /// The person reads the task's review at its current revisions.
    Review(&'static str),
    /// The person merges the task at the review read last.
    Merge(&'static str),
    /// The person tries to merge at the first review read for the task,
    /// which must be refused as stale.
    MergeStale(&'static str),
    /// The person requests changes at the review read last.
    RequestChanges(&'static str, &'static str),
}

/// The lead's turns.
const LEAD: &[Turn] = &[Turn::Ask("Which greeting should the team use?"), Turn::Plan];
/// The greeting task's turns.
const GREET: &[Turn] = &[Turn::Edit {
    files: &[
        ("greeting.txt", "Hello, studio\n"),
        (
            "README.md",
            "# Scratch\n\nStatus: greets people\n\nA scratch repository for the simulated team.\n",
        ),
    ],
    message: "Greet with Hello, studio",
}];
/// The documentation task's turns: the second answers the review.
const DOCS: &[Turn] = &[
    Turn::Edit {
        files: &[(
            "README.md",
            "# Scratch\n\nStatus: documented\n\nA scratch repository for the simulated team.\n",
        )],
        message: "Document the greeting",
    },
    Turn::Edit {
        files: &[(
            "docs/greeting.md",
            "# Greeting\n\nThe greeting lives in greeting.txt.\n",
        )],
        message: "Name greeting.txt in the documentation",
    },
];
/// How the documentation seat resolves its landing's conflict.
const DOCS_RESOLUTION: &[(&str, &str)] = &[(
    "README.md",
    "# Scratch\n\nStatus: greets people, documented\n\nA scratch repository for the simulated team.\n",
)];
/// The release task's turns.
const RELEASE: &[Turn] = &[
    Turn::Approve("Add CHANGELOG.md at the repository root?"),
    Turn::Edit {
        files: &[(
            "CHANGELOG.md",
            "# Changelog\n\n## 0.1.0\n\n- Greet with \"Hello, studio\".\n",
        )],
        message: "Add the changelog",
    },
];

/// The script, from submission to a goal whose tasks have all landed.
pub const SCRIPT: &[Step] = &[
    Step::Submit,
    Step::Run("lead"),
    Step::Answer("lead", ANSWER),
    Step::Run("lead"),
    Step::Reconcile,
    Step::Restart,
    Step::Message("grace", STEER),
    Step::Run("greet"),
    Step::Run("docs"),
    Step::Review("greet"),
    Step::Merge("greet"),
    Step::Review("docs"),
    Step::RequestChanges("docs", CHANGES),
    Step::Run("docs"),
    Step::MergeStale("docs"),
    Step::Review("docs"),
    Step::Merge("docs"),
    Step::Reconcile,
    Step::Run("release"),
    Step::Answer("release", ALLOW),
    Step::Run("release"),
    Step::Review("release"),
    Step::Merge("release"),
    Step::Reconcile,
];

/// The turns of the task with plan identity `key`, in the order the
/// script takes them. A view that replays the script, such as Verse's
/// Everglade fixture, reads what each turn does from here.
#[must_use]
pub fn turns(key: &str) -> &'static [Turn] {
    match key {
        "lead" => LEAD,
        "greet" => GREET,
        "docs" => DOCS,
        "release" => RELEASE,
        _ => &[],
    }
}

/// How the seat of `key` resolves a landing conflict, if it can.
fn resolution(key: &str) -> &'static [(&'static str, &'static str)] {
    match key {
        "docs" => DOCS_RESOLUTION,
        _ => &[],
    }
}

/// The lead's plan.
#[must_use]
pub fn plan() -> String {
    serde_json::json!({
        "schema": studio::PLAN_SCHEMA,
        "tasks": [
            {
                "id": "greet",
                "title": "Change the greeting",
                "description": "Make greeting.txt say Hello, studio, and update the status line.",
                "seat": "ada",
            },
            {
                "id": "docs",
                "title": "Document the greeting",
                "description": "Describe the greeting in the README's status line.",
                "seat": "grace",
            },
            {
                "id": "release",
                "title": "Write the changelog",
                "description": "Add CHANGELOG.md once both changes land.",
                "depends_on": ["greet", "docs"],
                "seat": "ada",
            },
        ],
    })
    .to_string()
}

/// A closed refusal classification.
#[derive(Debug)]
pub enum Error {
    /// The sim route was asked for outside its fixture.
    Refused(String),
    /// A merge decision named revisions the worktree no longer has.
    Stale {
        task: String,
    },
    /// A task is not in the state the step needs.
    State(String),
    Studio(studio::Error),
    Tasks(super::Error),
    Git(String),
    /// The landing did not land; the change stays in its worktree.
    Landing(String),
}

impl Error {
    /// Stable refusal codes for machine-readable callers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Refused(_) => "sim_refused",
            Self::Stale { .. } => "stale_review",
            Self::State(_) => "invalid_state",
            Self::Studio(error) => error.code(),
            Self::Tasks(error) => error.code(),
            Self::Git(_) => "git_failed",
            Self::Landing(_) => "not_landed",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(message)
            | Self::State(message)
            | Self::Git(message)
            | Self::Landing(message) => formatter.write_str(message),
            Self::Stale { task } => write!(
                formatter,
                "task {task}'s worktree changed after the review was read; read it again"
            ),
            Self::Studio(error) => write!(formatter, "{error}"),
            Self::Tasks(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<studio::Error> for Error {
    fn from(error: studio::Error) -> Self {
        Self::Studio(error)
    }
}

impl From<super::Error> for Error {
    fn from(error: super::Error) -> Self {
        Self::Tasks(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Tasks(super::Error::Io(error))
    }
}

/// The marker a fixture writes, naming the checkout it created.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: String,
    checkout: String,
}

/// The explicit fixture configuration the sim route runs in: a bare
/// `origin`, a checkout of it, worktrees, and a task store, all under one
/// directory the caller owns.
#[derive(Clone, Debug)]
pub struct Fixture {
    pub dir: PathBuf,
    /// The bare repository the landings push to.
    pub origin: PathBuf,
    /// The person's checkout: the repository the goal names.
    pub checkout: PathBuf,
    /// Where each task's worktree is made.
    pub worktrees: PathBuf,
    /// The task store the coordinator keeps its document in.
    pub store: PathBuf,
}

impl Fixture {
    fn at(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            origin: dir.join("origin.git"),
            checkout: dir.join("checkout"),
            worktrees: dir.join("worktrees"),
            store: dir.join("tasks"),
        }
    }

    /// Create the scratch repository in `dir`, which must be absent or
    /// empty: a bare `origin` seeded with a greeting and a README, the
    /// person's checkout of it, and the marker.
    ///
    /// # Errors
    /// `dir` is in use, or Git fails.
    pub fn create(dir: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(dir)?;
        if std::fs::read_dir(dir)?.next().is_some() {
            return Err(Error::Refused(format!(
                "{} is not empty; a fixture starts in an empty directory",
                dir.display()
            )));
        }
        let dir = dir.canonicalize()?;
        let fixture = Self::at(&dir);
        std::fs::create_dir_all(&fixture.origin)?;
        git(
            &fixture.origin,
            &["init", "-q", "--bare", "-b", BRANCH],
            START,
            None,
        )?;
        let seed = dir.join("seed");
        std::fs::create_dir_all(&seed)?;
        git(&seed, &["init", "-q", "-b", BRANCH], START, None)?;
        write(&seed, "greeting.txt", "hello\n")?;
        write(
            &seed,
            "README.md",
            &format!(
                "# Scratch\n\n{STATUS_LINE}\n\nA scratch repository for the simulated team.\n"
            ),
        )?;
        git(&seed, &["add", "-A"], START, None)?;
        git(
            &seed,
            &["commit", "-q", "-m", "Seed the scratch repository"],
            START,
            Some("fixture"),
        )?;
        let origin = fixture.origin.to_string_lossy().into_owned();
        git(
            &seed,
            &["push", "-q", &origin, &format!("HEAD:refs/heads/{BRANCH}")],
            START,
            None,
        )?;
        std::fs::remove_dir_all(&seed)?;
        let checkout = fixture.checkout.to_string_lossy().into_owned();
        git(&dir, &["clone", "-q", &origin, &checkout], START, None)?;
        // Only an approved landing carries the person's identity.
        for (key, value) in [
            ("user.name", "person"),
            ("user.email", "person@studio.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            git(&fixture.checkout, &["config", key, value], START, None)?;
        }
        std::fs::create_dir_all(&fixture.worktrees)?;
        let marker = Marker {
            schema: FIXTURE_SCHEMA.into(),
            checkout: checkout.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&marker)
            .map_err(|error| Error::Git(format!("the marker could not be encoded: {error}")))?;
        std::fs::write(dir.join(MARKER), bytes)?;
        Ok(fixture)
    }
}

/// Whether the sim route may run against `repository`: only inside the
/// explicit `fixture` that created it.
///
/// # Errors
/// No fixture is configured, or `repository` is not the fixture's
/// marked checkout.
pub fn admit(repository: &Path, fixture: Option<&Fixture>) -> Result<(), Error> {
    let Some(fixture) = fixture else {
        return Err(Error::Refused(
            "the sim route runs only in a test or an explicit fixture configuration".into(),
        ));
    };
    let refused = || {
        Error::Refused(format!(
            "the sim route runs only against its fixture's scratch repository, not {}",
            repository.display()
        ))
    };
    let marker: Marker = std::fs::read(fixture.dir.join(MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(refused)?;
    let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if marker.schema != FIXTURE_SCHEMA
        || !same(Path::new(&marker.checkout), &fixture.checkout)
        || !same(repository, &fixture.checkout)
    {
        return Err(refused());
    }
    Ok(())
}

/// The task inbox as the simulated team keeps it, in memory: the inbox's
/// own transitions, exact-byte retries, and the run results an owner
/// would record.
#[derive(Debug, Default)]
pub struct SimInbox {
    tasks: BTreeMap<String, Task>,
    /// Every accepted command's identity, task, and exact bytes.
    commands: BTreeMap<String, (String, Vec<u8>)>,
}

impl Inbox for SimInbox {
    fn apply(&mut self, bytes: &[u8]) -> Result<(), super::Error> {
        let command = super::parse_command(bytes)?;
        if let Some((task, accepted)) = self.commands.get(&command.command_id) {
            return if *task == command.task_id && accepted.as_slice() == bytes {
                Ok(())
            } else {
                Err(super::Error::Conflict)
            };
        }
        let sequence = self.commands.len() as u64 + 1;
        let digest = nostr::contracts::digest_bytes(bytes);
        super::transition(&command, &digest, sequence, &mut self.tasks)?;
        self.commands
            .insert(command.command_id, (command.task_id, bytes.to_vec()));
        Ok(())
    }

    fn task(&self, task_id: &str) -> Option<Task> {
        self.tasks.get(task_id).cloned()
    }
}

impl SimInbox {
    /// Every task, in identity order.
    #[must_use]
    pub fn tasks(&self) -> Vec<Task> {
        self.tasks.values().cloned().collect()
    }

    /// Start the queued task `task_id`'s turn, as an owner would.
    pub(crate) fn start(&mut self, task_id: &str) -> Result<(), Error> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::State(format!("the inbox holds no task {task_id}")))?;
        if task.status != Status::Queued {
            return Err(Error::State(format!("task {task_id} is not queued")));
        }
        task.status = Status::Running;
        task.execution = Execution::Running;
        Ok(())
    }

    /// End the running turn of `task_id` with `ending`, as an owner would.
    pub(crate) fn end(&mut self, task_id: &str, ending: &str) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = Status::Finished;
            task.execution = Execution::Finished;
            task.run = Some(run(task, ending));
        }
    }

    /// Record `verdict` as the independent check of `task_id`'s ended
    /// turn, with `reason`, as its owner would.
    #[cfg(test)]
    pub(crate) fn checked(&mut self, task_id: &str, verdict: super::Checks, reason: &str) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.checks = verdict;
            if let Some(run) = task.run.as_mut() {
                run.check_report = Some(checks::Report {
                    schema: "openagents.coder.task-checks.v1".into(),
                    requirements_digest: String::new(),
                    context_digest: String::new(),
                    candidate_snapshot: None,
                    verdict,
                    evidence: None,
                    reason: (!reason.is_empty()).then(|| reason.to_owned()),
                });
            }
        }
    }
}

/// The run an owner would record for `task`'s current turn, ended with
/// `ending`.
pub(crate) fn run(task: &Task, ending: &str) -> owner::Run {
    let grant = owner::Grant {
        schema: owner::GRANT_SCHEMA.into(),
        task_id: task.task_id.clone(),
        intent_digest: task.intent_digest.clone(),
        expected_revision: task.revision,
        expected_source_snapshot: None,
        program: MODEL.into(),
        arguments: Vec::new(),
        write_workspace: true,
        wall_seconds: 0,
        stream_bytes: 1024,
        memory_bytes: 64 * 1024 * 1024,
        requirements: None,
        adapter_configuration: None,
    };
    owner::Run {
        epoch: task.turn() as u64,
        admission: owner::Admission {
            grant,
            grant_digest: String::new(),
            grant_request: String::new(),
            workspace: PathBuf::from(&task.intent.workspace.path),
            source_revision: String::new(),
            source_snapshot: String::new(),
            program_digest: String::new(),
            adapter: task.intent.configuration.adapter.clone(),
            network: String::new(),
            read_scope: String::new(),
            authority: String::new(),
            trace_file: task.trace_file(task.turn()),
            context: checks::Context {
                schema: String::new(),
                task_revision: task.revision,
                prompt: String::new(),
                instructions: Vec::new(),
                suites: Vec::new(),
                lineage: checks::Lineage::default(),
                knowledge: Vec::new(),
                digest: String::new(),
            },
        },
        effect_id: None,
        result: Some(owner::ResultRecord {
            ending: ending.into(),
            exit_code: Some(0),
            stop_requested: false,
            group_clear: true,
            elapsed_ms: TICK * 1000,
            trace_digest: String::new(),
            candidate_snapshot: None,
            artifact_file: None,
            artifact_digest: None,
            output_incomplete: false,
            cost_status: "priced".into(),
            cost_microusd: Some(0),
            engine_microusd: Some(0),
            jev_microusd: Some(0),
            payer: None,
            payer_keys: Vec::new(),
        }),
        process_id: None,
        recovery_reason: None,
        check_report: None,
    }
}

/// The seats the script names: the lead `lead` and the workers `ada` and
/// `grace`, each on the Codex-shaped route with the model [`MODEL`].
///
/// # Errors
/// The coordinator refuses a seat.
pub fn seat_team(studio: &mut Studio) -> Result<(), Error> {
    let route = studio::parse_route(&format!("codex:{MODEL}"))?;
    for (desk, (name, role)) in [
        ("lead", Role::Lead),
        ("ada", Role::Worker),
        ("grace", Role::Worker),
    ]
    .into_iter()
    .enumerate()
    {
        studio.set_seat(Seat {
            name: name.into(),
            role,
            route: route.clone(),
            look: "default".into(),
            desk: desk as u32,
        })?;
    }
    Ok(())
}

/// A task's worktree and the commit it started from.
#[derive(Clone, Debug)]
struct Worktree {
    path: PathBuf,
    base: String,
}

/// One thing the script did, for a reader of the run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// The fixed clock's reading.
    pub at: u64,
    pub text: String,
}

/// The simulated team over one [`Fixture`].
pub struct Team {
    fixture: Fixture,
    inbox: SimInbox,
    studio: Option<Studio>,
    clock: u64,
    goal_id: Option<String>,
    /// Turns taken per plan identity.
    taken: BTreeMap<String, usize>,
    /// Each task's last reply, by task identity.
    replies: BTreeMap<String, String>,
    worktrees: BTreeMap<String, Worktree>,
    /// Every review read per plan identity, oldest first.
    reviews: BTreeMap<String, Vec<TaskReview>>,
    /// The landed commit per plan identity.
    landed: BTreeMap<String, String>,
    events: Vec<Event>,
}

impl Team {
    /// Seat the team in `fixture`: a lead and two workers on the sim
    /// route.
    ///
    /// # Errors
    /// The fixture does not admit its own checkout, or the coordinator
    /// cannot open.
    pub fn new(fixture: Fixture) -> Result<Self, Error> {
        admit(&fixture.checkout, Some(&fixture))?;
        let mut studio = Studio::open(&fixture.store)?;
        seat_team(&mut studio)?;
        Ok(Self {
            fixture,
            inbox: SimInbox::default(),
            studio: Some(studio),
            clock: START,
            goal_id: None,
            taken: BTreeMap::new(),
            replies: BTreeMap::new(),
            worktrees: BTreeMap::new(),
            reviews: BTreeMap::new(),
            landed: BTreeMap::new(),
            events: Vec::new(),
        })
    }

    /// Run every step of [`SCRIPT`].
    ///
    /// # Errors
    /// A step failed; the steps before it stay done.
    pub fn run_script(&mut self) -> Result<(), Error> {
        for step in SCRIPT {
            self.step(*step)?;
        }
        Ok(())
    }

    /// The coordinator joined with the inbox, as a view draws it.
    #[must_use]
    pub fn view(&self) -> View {
        self.studio().view(&self.inbox)
    }

    /// The coordinator.
    #[must_use]
    pub fn studio(&self) -> &Studio {
        self.studio
            .as_ref()
            .expect("the coordinator is open between steps")
    }

    #[must_use]
    pub fn inbox(&self) -> &SimInbox {
        &self.inbox
    }

    #[must_use]
    pub fn fixture(&self) -> &Fixture {
        &self.fixture
    }

    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The submitted goal's identity.
    #[must_use]
    pub fn goal_id(&self) -> Option<&str> {
        self.goal_id.as_deref()
    }

    /// The commit each landed task's change landed as, by plan identity.
    #[must_use]
    pub fn landed(&self) -> &BTreeMap<String, String> {
        &self.landed
    }

    /// Every review the person read of plan identity `key`, oldest first.
    #[must_use]
    pub fn reviews(&self, key: &str) -> &[TaskReview] {
        self.reviews.get(key).map_or(&[], Vec::as_slice)
    }

    /// The task identity of plan identity `key`.
    ///
    /// # Errors
    /// No goal was submitted, or its plan has no such task.
    pub fn task_id(&self, key: &str) -> Result<String, Error> {
        let goal_id = self
            .goal_id
            .as_deref()
            .ok_or_else(|| Error::State("no goal was submitted".into()))?;
        let goal = self
            .studio()
            .state()
            .goal(goal_id)
            .ok_or_else(|| Error::State(format!("the coordinator has no goal {goal_id}")))?;
        if key == "lead" {
            return Ok(goal.lead.task_id.clone());
        }
        goal.plan
            .iter()
            .find(|entry| entry.id == key)
            .map(|entry| entry.slot.task_id.clone())
            .ok_or_else(|| Error::State(format!("the plan has no task `{key}`")))
    }

    fn note(&mut self, text: String) {
        self.events.push(Event {
            at: self.clock,
            text,
        });
    }

    fn progress(&self, task_id: &str) -> Progress {
        self.inbox
            .task(task_id)
            .map_or(Progress::Missing, |task| studio::progress_of(&task))
    }

    /// Take one step at the next tick.
    ///
    /// # Errors
    /// The step's task is not in the state it needs, the coordinator or
    /// Git refused, or a merge was not refused where the script expects
    /// a stale refusal.
    pub fn step(&mut self, step: Step) -> Result<(), Error> {
        self.clock += TICK;
        let now = self.clock;
        match step {
            Step::Submit => {
                let goal = NewGoal {
                    text: GOAL.into(),
                    repository: Repository {
                        label: LABEL.into(),
                        path: self.fixture.checkout.to_string_lossy().into_owned(),
                    },
                    lead: None,
                };
                let studio = self.studio.as_mut().expect("open");
                let (goal_id, lead) = studio.submit_goal(&mut self.inbox, goal, now)?;
                self.goal_id = Some(goal_id.clone());
                self.note(format!("goal {goal_id} submitted; {} queued", lead.task_id));
            }
            Step::Run(key) => self.take_turn(key)?,
            Step::Answer(key, text) => {
                let task_id = self.task_id(key)?;
                if self.progress(&task_id) != Progress::Waiting {
                    return Err(Error::State(format!("task {task_id} is not waiting")));
                }
                self.follow_up(&task_id, "answer", text)?;
                self.note(format!("the person answered {task_id}: {text}"));
            }
            Step::Message(seat, text) => {
                let studio = self.studio.as_mut().expect("open");
                let written = studio.message(
                    &self.inbox,
                    Party::Person,
                    Party::Seat { name: seat.into() },
                    text,
                    now,
                )?;
                for message in written {
                    let how = match message.delivery {
                        Delivery::Steered { task_id } => format!("steered into {task_id}"),
                        other => format!("{other:?}"),
                    };
                    self.note(format!("the person messaged @{seat}: {how}"));
                }
            }
            Step::Reconcile => {
                let studio = self.studio.as_mut().expect("open");
                let replies = &self.replies;
                let reply = |task: &str| replies.get(task).cloned();
                let released = studio.reconcile(&mut self.inbox, now, &reply)?;
                for item in released {
                    self.note(format!("{} released to @{}", item.task_id, item.seat));
                }
            }
            Step::Restart => {
                self.studio = None;
                self.studio = Some(Studio::open(&self.fixture.store)?);
                self.note("the host restarted".into());
            }
            Step::Review(key) => {
                let task_id = self.task_id(key)?;
                let worktree = self.worktree(&task_id)?;
                let review =
                    super::review::read(&task_id, &worktree.path, &worktree.base, REVIEW_MAX)
                        .map_err(Error::Git)?;
                self.note(format!(
                    "the person read {task_id}'s review: {} file(s), +{} -{}",
                    review.files_total, review.added, review.removed
                ));
                self.reviews.entry(key.into()).or_default().push(review);
            }
            Step::Merge(key) => {
                let review = self.last_review(key)?;
                self.merge(key, &review)?;
            }
            Step::MergeStale(key) => {
                let review = self
                    .reviews
                    .get(key)
                    .and_then(|reviews| reviews.first())
                    .cloned()
                    .ok_or_else(|| Error::State(format!("no review of `{key}` was read")))?;
                match self.merge(key, &review) {
                    Err(Error::Stale { task }) => {
                        self.note(format!(
                            "a merge of {task} at an earlier review was refused"
                        ));
                    }
                    Err(other) => return Err(other),
                    Ok(()) => {
                        return Err(Error::State(format!(
                            "a merge of `{key}` at an earlier review was not refused"
                        )));
                    }
                }
            }
            Step::RequestChanges(key, text) => {
                let review = self.last_review(key)?;
                let task_id = self.decidable(key, &review)?;
                self.follow_up(&task_id, "changes", text)?;
                self.note(format!("the person requested changes on {task_id}: {text}"));
            }
        }
        Ok(())
    }

    fn last_review(&self, key: &str) -> Result<TaskReview, Error> {
        self.reviews
            .get(key)
            .and_then(|reviews| reviews.last())
            .cloned()
            .ok_or_else(|| Error::State(format!("no review of `{key}` was read")))
    }

    fn worktree(&self, task_id: &str) -> Result<Worktree, Error> {
        self.worktrees
            .get(task_id)
            .cloned()
            .ok_or_else(|| Error::State(format!("task {task_id} has no worktree")))
    }

    /// Start the next turn of `task_id` with `text`, as the existing
    /// `answer` command and a request for changes do.
    fn follow_up(&mut self, task_id: &str, what: &str, text: &str) -> Result<(), Error> {
        let task = self
            .inbox
            .task(task_id)
            .ok_or_else(|| Error::State(format!("the inbox holds no task {task_id}")))?;
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("sim-{what}-{task_id}-{}", task.revision),
            task_id: task_id.into(),
            expected_revision: Some(task.revision),
            action: Action::Continue {
                prompt: text.into(),
            },
        };
        let bytes = serde_json::to_vec(&command)
            .map_err(|error| Error::State(format!("a command could not be encoded: {error}")))?;
        self.inbox.apply(&bytes)?;
        Ok(())
    }

    /// The task of `key` if a decision at `review` may be made: the task
    /// is done and its worktree is still at the review's revisions.
    fn decidable(&self, key: &str, review: &TaskReview) -> Result<String, Error> {
        let task_id = self.task_id(key)?;
        if self.progress(&task_id) != Progress::Done {
            return Err(Error::State(format!("task {task_id} is not done")));
        }
        let worktree = self.worktree(&task_id)?;
        let now = super::review::head(&worktree.path).map_err(Error::Git)?;
        if review.task != task_id
            || review.base != worktree.base
            || review.head_commit != now.commit
            || review.head != now.tree
        {
            return Err(Error::Stale { task: task_id });
        }
        Ok(task_id)
    }

    /// Merge `key` at `review`: land its worktree's change on the
    /// scratch `origin`, with one conflict fix turn from its seat.
    fn merge(&mut self, key: &str, review: &TaskReview) -> Result<(), Error> {
        let task_id = self.decidable(key, review)?;
        let worktree = self.worktree(&task_id)?;
        let plan = super::landing::Plan {
            worktree: &worktree.path,
            branch: BRANCH,
            attempts: 3,
            backoff: super::landing::Backoff {
                first: Duration::from_millis(10),
                cap: Duration::from_millis(40),
            },
        };
        let mut hooks = Lander {
            worktree: worktree.path.clone(),
            resolution: resolution(key),
            notes: Vec::new(),
            repaired: false,
        };
        let landed = super::landing::land(&plan, &mut hooks)
            .map_err(|refused| Error::Landing(format!("{:?}", refused.failure)))?;
        for text in hooks.notes {
            self.note(format!("landing {task_id}: {text}"));
        }
        if hooks.repaired {
            self.note(format!("{task_id}'s seat resolved a merge conflict"));
        }
        let short: String = landed.commit.chars().take(12).collect();
        let goal_id = self.goal_id.clone();
        let studio = self.studio.as_mut().expect("open");
        studio.remember(
            MemoryKind::Decision,
            Party::Person,
            goal_id.as_deref(),
            &format!("Merged `{key}` at {short}."),
        )?;
        self.note(format!("the person merged {task_id} as {short}"));
        self.landed.insert(key.into(), landed.commit);
        Ok(())
    }

    /// The engine's next turn of `key`'s task.
    fn take_turn(&mut self, key: &str) -> Result<(), Error> {
        let task_id = self.task_id(key)?;
        let index = self.taken.get(key).copied().unwrap_or(0);
        let turn = *turns(key)
            .get(index)
            .ok_or_else(|| Error::State(format!("the script has no turn {index} for `{key}`")))?;
        let seat = self
            .studio()
            .state()
            .goals
            .iter()
            .flat_map(|goal| {
                std::iter::once(&goal.lead).chain(goal.plan.iter().map(|entry| &entry.slot))
            })
            .find(|slot| slot.task_id == task_id)
            .map(|slot| slot.seat.clone())
            .ok_or_else(|| Error::State(format!("no seat holds task {task_id}")))?;
        self.inbox.start(&task_id)?;
        let steered = super::steer::take(self.studio().store(), &task_id);
        for message in steered {
            self.note(format!("@{seat} read a steering message: {message}"));
        }
        let (ending, reply) = match turn {
            Turn::Ask(question) => (interaction::QUESTION_ENDING, question.to_owned()),
            Turn::Approve(step) => (interaction::APPROVAL_ENDING, step.to_owned()),
            Turn::Plan => (
                "model_finished",
                format!("I read the repository.\n\n```json\n{}\n```\n", plan()),
            ),
            Turn::Edit { files, message } => {
                let worktree = self.ensure_worktree(&task_id)?;
                for (path, text) in files {
                    write(&worktree.path, path, text)?;
                }
                git(&worktree.path, &["add", "-A"], self.clock, None)?;
                git(
                    &worktree.path,
                    &["commit", "-q", "-m", message],
                    self.clock,
                    Some(seat.as_str()),
                )?;
                ("model_finished", message.to_owned())
            }
        };
        self.inbox.end(&task_id, ending);
        self.replies.insert(task_id.clone(), reply);
        self.taken.insert(key.into(), index + 1);
        self.note(format!("@{seat} ended turn {} of {task_id}", index + 1));
        Ok(())
    }

    /// `task_id`'s worktree, made on its own branch from the newest
    /// `origin` the first time.
    fn ensure_worktree(&mut self, task_id: &str) -> Result<Worktree, Error> {
        if let Some(worktree) = self.worktrees.get(task_id) {
            return Ok(worktree.clone());
        }
        let checkout = &self.fixture.checkout;
        git(checkout, &["fetch", "-q", "origin"], self.clock, None)?;
        let path = self.fixture.worktrees.join(task_id);
        let at = path.to_string_lossy().into_owned();
        git(
            checkout,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("studio/{task_id}"),
                &at,
                &format!("origin/{BRANCH}"),
            ],
            self.clock,
            None,
        )?;
        let base = git(&path, &["rev-parse", "HEAD"], self.clock, None)?;
        let worktree = Worktree { path, base };
        self.worktrees.insert(task_id.into(), worktree.clone());
        Ok(worktree)
    }
}

/// The landing's hooks for a sim merge: the checks pass unless a
/// conflict marker is left, and the seat's one conflict fix turn writes
/// its scripted resolution.
struct Lander {
    worktree: PathBuf,
    resolution: &'static [(&'static str, &'static str)],
    notes: Vec<String>,
    repaired: bool,
}

impl super::landing::Hooks for Lander {
    fn check(&mut self) -> Vec<String> {
        let mut problems = Vec::new();
        for (path, _) in self.resolution {
            let text = std::fs::read_to_string(self.worktree.join(path)).unwrap_or_default();
            if text.contains("<<<<<<<") || text.contains(">>>>>>>") {
                problems.push(format!("{path} holds a conflict marker"));
            }
        }
        problems
    }

    fn fix_conflict(&mut self, _request: &str) -> Result<(), String> {
        if self.resolution.is_empty() {
            return Err("this seat has no scripted resolution".into());
        }
        for (path, text) in self.resolution {
            write(&self.worktree, path, text).map_err(|error| error.to_string())?;
        }
        self.repaired = true;
        Ok(())
    }

    fn note(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }

    fn stopping(&self) -> bool {
        false
    }
}

/// The marker a [`Scratch`] writes in its host root.
pub const SCRATCH_MARKER: &str = "studio-sim-host.json";
/// The scratch marker's schema.
pub const SCRATCH_SCHEMA: &str = "openagents.coder.studio-sim-host.v1";
/// The workspace label a scratch host serves its repository under.
pub const WORKSPACE: &str = LABEL;
/// How often a scratch host's [`Engine`] looks for a turn to end.
pub const ENGINE_EVERY: Duration = Duration::from_millis(250);
/// What a worker's conflict follow-up asks it to run, as the coordinator
/// words it ([`studio::flow`]): `(`git merge TARGET`)`.
const MERGE_HINT: &str = "(`git merge ";

/// The marker a scratch host's root holds: the task store and checkout a
/// [`Scratch`] made.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScratchMarker {
    schema: String,
    store: String,
    checkout: String,
}

/// A scratch host for the simulated team (#10572): every directory a host
/// keeps, under one directory the caller owns and never under the home
/// directory's Coder state.
#[derive(Clone, Debug)]
pub struct Scratch {
    pub dir: PathBuf,
    /// The scratch repository, its `origin`, and its marker.
    pub fixture: Fixture,
    /// The host root: `serve.json`, the studio's worktrees, and the
    /// scratch marker.
    pub root: PathBuf,
    /// The host's task store, which admits scripted turns.
    pub store: PathBuf,
    /// The host's access store.
    pub state: PathBuf,
    /// The host's file key source.
    pub keys: PathBuf,
    /// The host's control socket.
    pub socket: PathBuf,
}

impl Scratch {
    fn at(dir: &Path, fixture: Fixture) -> Self {
        Self {
            dir: dir.to_path_buf(),
            fixture,
            root: dir.join("host"),
            store: dir.join("tasks"),
            state: dir.join("coder-access"),
            keys: dir.join("connect"),
            socket: dir.join("control.sock"),
        }
    }

    /// Make a scratch host in `dir`, which must be absent or empty: the
    /// scratch repository ([`Fixture::create`]), a host root whose
    /// `serve.json` admits its checkout as workspace [`WORKSPACE`], and a
    /// task store that admits scripted turns. No seat is set; see
    /// [`Scratch::seat_team`].
    ///
    /// # Errors
    /// `dir` is in use, under the home directory's Coder state, or a file
    /// cannot be written.
    pub fn create(dir: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(dir)?;
        if std::fs::read_dir(dir)?.next().is_some() {
            return Err(Error::Refused(format!(
                "{} is not empty; a scratch host starts in an empty directory",
                dir.display()
            )));
        }
        let dir = dir.canonicalize()?;
        let fixture = Fixture::create(&dir.join("fixture"))?;
        let scratch = Self::at(&dir, fixture);
        crate::private::create_dir_all(&scratch.root)?;
        owner::allow_scripted(&scratch.store, "the Agent Studio's simulated team")?;
        let workspaces = BTreeMap::from([(WORKSPACE.to_owned(), scratch.fixture.checkout.clone())]);
        coder_host::settings::ServeSettings::new(Vec::new(), workspaces)
            .save(&scratch.root)
            .map_err(|error| Error::Refused(format!("cannot write the host settings: {error}")))?;
        let marker = ScratchMarker {
            schema: SCRATCH_SCHEMA.into(),
            store: scratch.store.canonicalize()?.to_string_lossy().into_owned(),
            checkout: scratch.fixture.checkout.to_string_lossy().into_owned(),
        };
        let bytes = serde_json::to_vec_pretty(&marker)
            .map_err(|error| Error::Refused(format!("the marker could not be encoded: {error}")))?;
        std::fs::write(scratch.root.join(SCRATCH_MARKER), bytes)?;
        Ok(scratch)
    }

    /// The scratch host [`Scratch::create`] made in `dir`.
    ///
    /// # Errors
    /// `dir` holds no scratch host.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let dir = dir.canonicalize()?;
        let fixture = Fixture::at(&dir.join("fixture"));
        let scratch = Self::at(&dir, fixture);
        let marker = read_marker(&scratch.root)?;
        if !same_path(Path::new(&marker.store), &scratch.store)
            || !same_path(Path::new(&marker.checkout), &scratch.fixture.checkout)
        {
            return Err(Error::Refused(format!(
                "{} holds no scratch host",
                dir.display()
            )));
        }
        Ok(scratch)
    }

    /// Seat the script's team in the scratch host's studio ([`seat_team`]).
    ///
    /// # Errors
    /// The coordinator cannot open or refuses a seat.
    pub fn seat_team(&self) -> Result<(), Error> {
        let mut studio = Studio::open(&self.store)?;
        seat_team(&mut studio)
    }
}

fn read_marker(root: &Path) -> Result<ScratchMarker, Error> {
    std::fs::read(root.join(SCRATCH_MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ScratchMarker>(&bytes).ok())
        .filter(|marker| marker.schema == SCRATCH_SCHEMA)
        .ok_or_else(|| {
            Error::Refused(format!(
                "the scripted engine runs only on a scratch host's root, not {}",
                root.display()
            ))
        })
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Whether the auto-start policy in `root` is on, or cannot be read.
fn policy_on(root: &Path) -> bool {
    match autostart::Policy::load(root) {
        Ok(policy) => policy.is_some_and(|policy| policy.enabled),
        Err(_) => true,
    }
}

/// What a studio task is to the script.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    /// A goal's lead task.
    Lead,
    /// Plan entry `key`'s task, worked by `seat`.
    Entry { key: String, seat: String },
    /// The lead's review of a plan entry's change.
    Review,
}

/// Every studio task the coordinator holds, by task identity.
fn parts(state: &studio::State) -> BTreeMap<String, Part> {
    let mut parts = BTreeMap::new();
    for goal in &state.goals {
        parts.insert(goal.lead.task_id.clone(), Part::Lead);
        for entry in &goal.plan {
            parts.insert(
                entry.slot.task_id.clone(),
                Part::Entry {
                    key: entry.id.clone(),
                    seat: entry.slot.seat.clone(),
                },
            );
            if let Some(review) = entry.flow.as_ref().and_then(|flow| flow.review.as_ref()) {
                parts.insert(review.task_id.clone(), Part::Review);
            }
        }
    }
    parts
}

/// The scripted engine of a scratch host (#10572): each pass runs the
/// coordinator's sweep, as the host's auto-start sweeper does, then ends
/// the turn of every queued studio task from the script through
/// [`owner::scripted`].
#[derive(Clone, Debug)]
pub struct Engine {
    root: PathBuf,
    store: PathBuf,
}

impl Engine {
    /// The engine for the scratch host with root `root` and task store
    /// `store`.
    ///
    /// # Errors
    /// `root` holds no [`Scratch`] marker naming `store`, `store` does not
    /// admit scripted turns, or `root`'s auto-start policy is on.
    pub fn open(root: &Path, store: &Path) -> Result<Self, Error> {
        let marker = read_marker(root)?;
        if !same_path(Path::new(&marker.store), store) {
            return Err(Error::Refused(format!(
                "the scratch host at {} keeps another task store than {}",
                root.display(),
                store.display()
            )));
        }
        if !owner::scripted_allowed(store) {
            return Err(Error::Refused(format!(
                "the task store {} does not admit scripted turns",
                store.display()
            )));
        }
        if policy_on(root) {
            return Err(Error::Refused(
                "the scratch host's auto-start policy is on; the scripted engine never runs beside a real one".into(),
            ));
        }
        Ok(Self {
            root: root.canonicalize()?,
            store: store.canonicalize()?,
        })
    }

    /// One pass: the coordinator's sweep, then one scripted turn for each
    /// queued studio task, then the sweep again so the coordinator sees
    /// them. Returns a sentence for each turn it ended. A turn the owner
    /// refuses is reported and left for the next pass.
    ///
    /// # Errors
    /// The auto-start policy turned on, or the store or the coordinator
    /// cannot be read.
    pub fn step(&self) -> Result<Vec<String>, Error> {
        if policy_on(&self.root) {
            return Err(Error::Refused(
                "the scratch host's auto-start policy is on; the scripted engine stops".into(),
            ));
        }
        studio::sweep(&self.store, &self.root, autostart::unix_now());
        if !Studio::present(&self.store) {
            return Ok(Vec::new());
        }
        let roles = {
            let studio = Studio::open(&self.store)?;
            parts(studio.state())
        };
        let queued: Vec<Task> = Store::open(&self.store)?
            .list()?
            .into_iter()
            .filter(|task| {
                task.status == Status::Queued
                    && task.run.is_none()
                    && roles.contains_key(&task.task_id)
            })
            .collect();
        let mut ended = Vec::new();
        for task in queued {
            let part = &roles[&task.task_id];
            match owner::scripted(&self.store, &task.task_id, |task, workspace| {
                host_turn(part, task, workspace)
            }) {
                Ok(done) => ended.push(format!(
                    "ended turn {} of {} ({})",
                    done.turn(),
                    done.task_id,
                    done.run
                        .as_ref()
                        .and_then(|run| run.result.as_ref())
                        .map_or("", |result| result.ending.as_str())
                )),
                Err(error) => eprintln!(
                    "openagents host: studio sim: task {} waits: {error}",
                    task.task_id
                ),
            }
        }
        if !ended.is_empty() {
            studio::sweep(&self.store, &self.root, autostart::unix_now());
        }
        Ok(ended)
    }

    /// Run a pass every `every` on a thread of its own until the returned
    /// handle is dropped or stopped.
    #[must_use]
    pub fn spawn(self, every: Duration) -> Running {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                if let Err(error) = self.step() {
                    eprintln!("openagents host: studio sim: {error}");
                }
                std::thread::sleep(every);
            }
        });
        Running {
            stop,
            handle: Some(handle),
        }
    }
}

/// A running [`Engine`]. Dropping it stops the engine after its pass.
#[derive(Debug)]
pub struct Running {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Running {
    /// Stop the engine and wait for its pass to end.
    pub fn stop(mut self) {
        self.halt();
    }

    /// Leave the engine running for as long as the process runs.
    pub fn detach(mut self) {
        self.handle = None;
        self.stop = Arc::new(AtomicBool::new(false));
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.halt();
    }
}

/// The launcher a scratch host's auto-start holds: it starts nothing.
/// The host's root has no policy, so it is never asked; were it asked, it
/// would refuse rather than start a real engine.
struct NoEngine;

impl autostart::Launch for NoEngine {
    fn launch(
        &self,
        _engine: &autostart::Engine,
        _grant: &Path,
        _store: &Path,
    ) -> Result<autostart::Launched, String> {
        Err("the simulated studio starts no engine; its scripted engine ends its turns".into())
    }
}

/// The task inbox a scratch host serves: the host's own inbox over
/// `store`, with an auto-start whose launcher starts nothing, so studio
/// intents give each task its worktree under `root` as on any host.
#[must_use]
pub fn inbox(store: &Path, root: &Path, workspaces: &BTreeMap<String, PathBuf>) -> remote::Inbox {
    let autostart = Arc::new(autostart::Autostart::new(
        root.to_path_buf(),
        store.to_path_buf(),
        workspaces.clone(),
        Box::new(NoEngine),
        autostart::unix_now,
    ));
    remote::Inbox::new(store, workspaces.clone()).with_autostart(autostart)
}

/// Open a scratch host's tasks for `coder host serve --studio-sim`: the
/// [`inbox`], with the scripted [`Engine`] running for as long as the
/// process runs.
///
/// # Errors
/// The engine refuses the root or the store ([`Engine::open`]).
pub fn open_host(
    store: &Path,
    root: &Path,
    workspaces: &BTreeMap<String, PathBuf>,
) -> Result<Arc<dyn coder_host::Tasks>, String> {
    let engine = Engine::open(root, store).map_err(|error| error.to_string())?;
    let inbox = inbox(store, root, workspaces);
    engine.spawn(ENGINE_EVERY).detach();
    Ok(Arc::new(inbox) as Arc<dyn coder_host::Tasks>)
}

/// One scripted turn of a studio task on a scratch host.
fn host_turn(part: &Part, task: &Task, workspace: &Path) -> Result<owner::Scripted, String> {
    let finished = |reply: String| owner::Scripted {
        ending: "model_finished".into(),
        reply,
    };
    let prompt = task.effective_prompt();
    // The turns before this one that were the script's own, not a
    // conflict the coordinator sent back.
    let index = std::iter::once(task.intent.prompt.as_str())
        .chain(task.follow_ups.iter().map(|item| item.prompt.as_str()))
        .take(task.turn().saturating_sub(1))
        .filter(|prompt| !prompt.contains(MERGE_HINT))
        .count();
    let (key, seat) = match part {
        Part::Review => {
            let verdict = serde_json::json!({
                "schema": studio::flow::REVIEW_SCHEMA,
                "verdict": "approve",
                "notes": "The change does what its task asks.",
            });
            return Ok(finished(format!(
                "I read the change and its checks.\n\n```json\n{verdict}\n```\n"
            )));
        }
        Part::Lead => ("lead", "lead".to_owned()),
        Part::Entry { key, seat } => {
            if prompt.contains(MERGE_HINT) {
                return resolve_conflict(key, seat, prompt, workspace).map(finished);
            }
            (key.as_str(), seat.clone())
        }
    };
    let Some(turn) = turns(key).get(index) else {
        return Ok(finished("Nothing more to change.".into()));
    };
    Ok(match *turn {
        Turn::Ask(question) => owner::Scripted {
            ending: interaction::QUESTION_ENDING.into(),
            reply: question.into(),
        },
        Turn::Approve(step) => owner::Scripted {
            ending: interaction::APPROVAL_ENDING.into(),
            reply: step.into(),
        },
        Turn::Plan => finished(format!(
            "I read the repository.\n\n```json\n{}\n```\n",
            plan()
        )),
        Turn::Edit { files, message } => {
            for (path, text) in files {
                write(workspace, path, text).map_err(|error| error.to_string())?;
            }
            seat_git(workspace, &["add", "-A"], &seat, true)?;
            seat_git(workspace, &["commit", "-q", "-m", message], &seat, true)?;
            finished(message.to_owned())
        }
    })
}

/// A conflict follow-up: merge the branch the coordinator names into the
/// worktree, write the seat's scripted resolution, and commit the merge.
fn resolve_conflict(
    key: &str,
    seat: &str,
    prompt: &str,
    workspace: &Path,
) -> Result<String, String> {
    let files = resolution(key);
    if files.is_empty() {
        return Err(format!("seat {seat} has no scripted resolution"));
    }
    let target = prompt
        .split_once(MERGE_HINT)
        .and_then(|(_, rest)| rest.split_once("`)"))
        .map(|(target, _)| target.trim())
        .filter(|target| !target.is_empty() && !target.starts_with('-'))
        .unwrap_or(BRANCH)
        .to_owned();
    // A merge that conflicts leaves markers the resolution replaces.
    seat_git(workspace, &["merge", "--no-edit", &target], seat, false)?;
    for (path, text) in files {
        write(workspace, path, text).map_err(|error| error.to_string())?;
    }
    seat_git(workspace, &["add", "-A"], seat, true)?;
    let pending = seat_git(workspace, &["status", "--porcelain"], seat, true)?;
    let message = format!("Merge {target} and resolve the conflict");
    if !pending.trim().is_empty() || merge_in_progress(workspace, seat) {
        seat_git(workspace, &["commit", "-q", "-m", &message], seat, true)?;
    }
    let names: Vec<&str> = files.iter().map(|(path, _)| *path).collect();
    Ok(format!(
        "Merged {target} and resolved {}.",
        names.join(", ")
    ))
}

/// Whether a merge waits to be committed in the worktree at `dir`.
fn merge_in_progress(dir: &Path, seat: &str) -> bool {
    seat_git(
        dir,
        &["rev-parse", "-q", "--verify", "MERGE_HEAD"],
        seat,
        true,
    )
    .is_ok()
}

/// Run Git in `dir` as seat `seat` ([`studio::git::identity`]), with no
/// global or system configuration. Returns its trimmed standard output;
/// a failure is an error only when `strict`.
fn seat_git(dir: &Path, args: &[&str], seat: &str, strict: bool) -> Result<String, String> {
    let (name, email) = studio::git::identity(seat);
    let output = super::local::git()
        .arg("-C")
        .arg(dir)
        .arg("-c")
        .arg(format!("user.name={name}"))
        .arg("-c")
        .arg(format!("user.email={email}"))
        .arg("-c")
        .arg("commit.gpgsign=false")
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if output.status.success() || !strict {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn write(dir: &Path, path: &str, text: &str) -> Result<(), Error> {
    let file = dir.join(path);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, text)?;
    Ok(())
}

/// Run Git in `dir` at the fixed clock's `at`, committing as `who` when
/// named. Returns its trimmed standard output.
fn git(dir: &Path, args: &[&str], at: u64, who: Option<&str>) -> Result<String, Error> {
    let mut command = super::local::git();
    command.arg("-C").arg(dir);
    if let Some(name) = who {
        command
            .arg("-c")
            .arg(format!("user.name={name}"))
            .arg("-c")
            .arg(format!("user.email={name}@studio.invalid"))
            .arg("-c")
            .arg("commit.gpgsign=false");
    }
    let date = format!("@{at} +0000");
    let output = command
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .args(args)
        .output()
        .map_err(|error| Error::Git(format!("cannot run git: {error}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(Error::Git(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(test)]
#[path = "studio_sim_tests.rs"]
mod tests;
