//! One episode: server up, bot in, the Voyager loop, everything traced.
//!
//! The paper's loop is a curriculum proposing tasks, an action agent
//! writing programs, an interpreter running them, and a critic deciding
//! success — with failed programs fed back for repair and finished ones
//! banked as skills. Each piece is a module:
//!
//! - **Curriculum** — [`crate::curriculum`]: the manifest's declared
//!   list, an Open Responses door proposing what comes next, or the
//!   built-in starter tasks. Completed and failed history goes back in
//!   with the state, the way the paper's curriculum reads it.
//! - **Programs** — a task's own `script`, a banked `skill` by name, a
//!   skill the decision door retrieves for the goal, or a program the
//!   `act` door writes. A task that can get none of those fails as
//!   *unwritten*, not as silently skipped.
//! - **Execution** — [`crate::interpret`]: bounded Lua over the
//!   bridge's typed ops. A fault — parse, runtime, exhausted, timed
//!   out — feeds the `act` door for a rewrite, up to
//!   [`REFINE_ROUNDS`] attempts per task, the paper's self-correction
//!   loop.
//! - **Critic** — [`crate::critic`]: mechanical specs first, a `noul`
//!   call to the decision door where a spec names one. The verdict's
//!   evidence lands in the trace either way.
//! - **Skills** — [`crate::skills`]: a passing task marked `bank`
//!   writes its program to the digested store, where later tasks can
//!   retrieve it.
//!
//! Every exchange and every event lands in an `atif` log inside the run
//! directory, beside `server.log` and the world data — an episode is a
//! thing a person can inspect afterwards, not a terminal scroll.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use atif::document::{Call, Outcome, Session, Source, Step};
use serde_json::{Map, Value, json};

use crate::bridge::{Bridge, Event};
use crate::critic::{self, Readings};
use crate::curriculum::{Context, Curriculum, DecisionSection, Responses, Task};
use crate::decide::Door;
use crate::error::{Error, Result};
use crate::interpret::{self, Host, Limits, ScriptError};
use crate::server::Server;
use crate::skills::{self, SkillStore};
use crate::state::AgentState;
use crate::world::{Scenario, World};

/// The most program attempts one task may spend — the paper's four
/// rounds of write, run, and repair.
const REFINE_ROUNDS: u32 = 4;
/// The blocks a `state` call scans, in each direction.
const SURVEY_RADIUS: u32 = 24;
/// Log kinds worth reporting on at the close.
const LOG_KINDS: [&str; 8] = [
    "oak_log",
    "spruce_log",
    "birch_log",
    "jungle_log",
    "acacia_log",
    "dark_oak_log",
    "cherry_log",
    "mangrove_log",
];

/// Everything an episode needs that is not in the world manifest.
#[derive(Clone)]
pub struct Plan {
    /// The Minecraft server jar, fetched by `scripts/fetch-mc-server.sh`.
    pub jar: PathBuf,
    /// The `java` binary.
    pub java: PathBuf,
    /// The built `mc-bridge` binary.
    pub bridge: PathBuf,
    /// The directory runs are created under; the episode makes
    /// `<stamp>-<world>` inside it.
    pub runs: PathBuf,
    /// The port the server listens on.
    pub port: u16,
    /// The `nostr-relay` binary an ensemble world with guild channels
    /// spawns.
    pub relay_bin: PathBuf,
    /// The Postgres URL the relay stores into.
    pub relay_database: String,
    /// The repository root — a world manifest's relative paths, like a
    /// quest's `fixture`, resolve against it.
    pub repo: PathBuf,
    /// The `--scenario` override: `Some` wins over the manifest's own
    /// `scenario` field. A solo world ignores it either way.
    pub scenario: Option<Scenario>,
}

/// How one task ended.
#[derive(Debug)]
pub struct TaskResult {
    /// What the curriculum asked for.
    pub task: String,
    /// Whether the critic accepted it.
    pub ok: bool,
    /// What the critic saw.
    pub detail: String,
}

/// What an episode produced.
pub struct Report {
    /// The world it ran in.
    pub world: String,
    /// The world's manifest digest.
    pub digest: String,
    /// Every task, in order, with the critic's answer.
    pub tasks: Vec<TaskResult>,
    /// How many bridge calls ran.
    pub actions: usize,
    /// How many program attempts ran, summed over tasks — the paper's
    /// "prompting iterations" axis.
    pub attempts: usize,
    /// The `name@version` of every skill banked this episode.
    pub banked: Vec<String>,
    /// Distinct block kinds `state` reported — the paper's "unique
    /// items" axis, measured at block granularity.
    pub seen: usize,
    /// Horizontal blocks walked, summed over `goto` and `explore`
    /// answers — the paper's traversal axis.
    pub distance: f64,
    /// The directory holding the world data, the server log, and the trace.
    pub run_dir: PathBuf,
    /// The ATIF trace file.
    pub trace: PathBuf,
}

impl Report {
    /// Whether every attempted task succeeded.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.tasks.iter().all(|task| task.ok)
    }
}

/// A runner holds the live halves of an episode: the server, the bridge,
/// the trace, the loop's doors and store, and the counters the bounds
/// check.
struct Runner<'a> {
    world: &'a World,
    lease: Option<crate::wow_pool::AccountLease>,
    plan: &'a Plan,
    server: Option<Server>,
    bridge: Bridge,
    log: atif::log::Log,
    run_dir: PathBuf,
    trace: PathBuf,
    curriculum: Curriculum,
    /// The `act` door: writes a task's program and repairs a faulted
    /// one. `None` means tasks run only what they carry.
    writer: Option<Responses>,
    /// The decisions door: skill retrieval and `noul` checks.
    door: Option<Door>,
    /// The banked-skill store.
    store: SkillStore,
    /// Task ids that passed, in order — curriculum context.
    completed: Vec<String>,
    /// Task ids that failed with their endings — curriculum context.
    failed: Vec<(String, String)>,
    /// Event texts not yet handed to a script's `feedback()` call —
    /// the paper's bot narration channel.
    feedback: VecDeque<String>,
    actions: usize,
    /// Program attempts over the whole episode.
    attempts: usize,
    /// `name@version` banked this episode.
    banked: Vec<String>,
    /// Distinct block kinds `state` has reported.
    seen: BTreeSet<String>,
    /// Horizontal blocks walked.
    distance: f64,
    /// Program-generation calls made, for evidence file names.
    written: u64,
    started: Instant,
    progress: &'a dyn Fn(&str),
}

/// Runs one episode of `world` under `plan`, narrating to `progress`.
///
/// The sequence is: server ready, bot joined, then the curriculum loop
/// — propose a task, find or write its program, run it through the
/// bounded interpreter with repair rounds, check it against the
/// critic, and bank what passed. Any fault — a server that will not
/// boot, a helper that breaks the protocol — stops the episode and is
/// still traced.
///
/// # Errors
///
/// Returns the first [`Error`] the episode hits; the run directory and
/// trace are complete up to that point.
pub fn run(world: &World, plan: &Plan, progress: impl Fn(&str)) -> Result<Report> {
    let lease = if world.wow.is_some() {
        Some(crate::wow_pool::AccountLease::acquire(world)?)
    } else {
        None
    };
    let mut effective = world.clone();
    if let Some(lease) = &lease {
        let wow = effective.wow.as_mut().unwrap();
        wow.accounts = vec![lease.account.clone()];
        wow.character = lease.character.clone();
    }
    let world = &effective;
    let run_dir = plan.runs.join(format!(
        "{}-{}",
        atif::log::session_id(atif::document::now_ms()),
        world.name
    ));
    let server_dir = run_dir.join("server");
    std::fs::create_dir_all(&server_dir)?;
    let trace = run_dir.join("trace.jsonl");

    progress(&format!(
        "world {} ({}) in {}",
        world.name,
        world.digest,
        run_dir.display()
    ));
    progress(if world.wow.is_some() {
        "attaching to the private WoW realm"
    } else {
        "starting the minecraft server"
    });
    let server = if world.wow.is_some() {
        None
    } else {
        Some(Server::start(
            world,
            &plan.jar,
            &plan.java,
            &server_dir,
            plan.port,
        )?)
    };

    let section = world.curriculum.as_ref();
    let curriculum = match section {
        Some(section) => Curriculum::from_manifest(section)?,
        None => Curriculum::fallback(),
    };
    let writer = section
        .and_then(|section| section.act.as_ref())
        .map(Responses::from_manifest)
        .transpose()?;
    let door = section
        .and_then(|section| section.decisions.as_ref())
        .map(|section| decision_door(section, run_dir.join("decisions")))
        .transpose()?;
    let skill_dir = if let Some(wow) = &world.wow {
        SkillStore::default_dir()
            .join("wow")
            .join(world.digest.trim_start_matches("sha256:"))
            .join(&wow.accounts[0])
    } else {
        SkillStore::default_dir()
    };
    let store = SkillStore::open(skill_dir)?;

    let session = Session::opening(
        &run_dir
            .file_name()
            .expect("a run dir has a name")
            .to_string_lossy(),
        concat!("voyager/", env!("CARGO_PKG_VERSION")),
        if world.wow.is_some() {
            "wow-local"
        } else {
            "minecraft-local"
        },
        &world.digest,
        env!("CARGO_PKG_VERSION"),
    );
    let log = atif::log::Log::create_at(&trace, &session)?;
    let mut runner = Runner {
        world,
        lease,
        plan,
        server,
        bridge: Bridge::start(&plan.bridge)?,
        log,
        run_dir,
        trace,
        curriculum,
        writer,
        door,
        store,
        completed: Vec::new(),
        failed: Vec::new(),
        feedback: VecDeque::new(),
        actions: 0,
        attempts: 0,
        banked: Vec::new(),
        seen: BTreeSet::new(),
        distance: 0.0,
        written: 0,
        started: Instant::now(),
        progress: &progress,
    };
    runner.note(Source::System, "server ready", json!({"port": plan.port}));
    if let Some(lease) = &runner.lease {
        let detail = json!({"account":lease.account,"character":lease.character});
        runner.note(Source::System, "realm lease acquired", detail);
    }

    let mut result = runner.episode();
    // The shutdown is part of the record too.
    runner.note(Source::System, "episode over", json!({}));
    if world.wow.is_some() && runner.lease.as_mut().is_none_or(|l| l.alive()) {
        let lease = &mut runner.lease;
        let cleanup =
            runner
                .bridge
                .call_guarded("cleanup", json!({}), Duration::from_secs(60), || {
                    lease.as_mut().is_none_or(|l| l.alive())
                });
        let _ = runner.log.append(&Step::called(Call {
            id: "cleanup".into(),
            name: "wow-bridge:cleanup".into(),
            arguments: json!({}),
            output: match &cleanup {
                Ok(v) => v.to_string(),
                Err(e) => e.to_string(),
            },
            outcome: if cleanup.is_ok() {
                Outcome::Completed
            } else {
                Outcome::Failed
            },
            milliseconds: 0,
            purpose: Some("remove episode character".into()),
            extra: Map::new(),
        }));
        if let Err(e) = cleanup {
            if result.is_ok() {
                result = Err(e);
            }
        }
    }
    let _ = runner.bridge.shutdown();
    if let Some(server) = &mut runner.server {
        let _ = server.stop();
    }
    let _ = runner.log.finish(if result.is_ok() {
        "ended"
    } else {
        "interrupted"
    });
    result
}

/// The decisions door a `curriculum.decisions` section builds — a
/// loopback URL is a local `kev-serve`; anything else is the live
/// TypeSafe API and wants its credential.
fn decision_door(section: &DecisionSection, dir: PathBuf) -> Result<Door> {
    let model = section.model.as_deref().unwrap_or("kev-latest");
    let local = ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|host| section.url.contains(host));
    if local {
        Door::local(&section.url, model, dir)
    } else {
        Door::live(&section.url, model, dir)
    }
}

impl Runner<'_> {
    /// The bounded curriculum loop.
    fn episode(&mut self) -> Result<Report> {
        let mut tasks = Vec::new();

        // Join.
        (self.progress)("joining the world");
        let address = format!("127.0.0.1:{}", self.plan.port);
        let args = if let Some(wow) = &self.world.wow {
            json!({"auth":wow.auth,"account":wow.accounts[0],"character":wow.character,"reset":true,"create":{"race":wow.race,"class":wow.class}})
        } else {
            json!({"address": address,"username":self.world.agent.username})
        };
        let joined = self.call("join", args, Duration::from_secs(75), "join the world")?;
        if self.world.wow.is_some() {
            let state = self.state("verify fresh character")?;
            if !state.wow["quests"].as_array().is_some_and(Vec::is_empty)
                || state.wow["earned_xp"] != 0
                || state.wow["xp"] != 0
            {
                return Err(Error::episode(
                    "fresh character inherited quest or XP progress",
                ));
            }
        }
        if let Some(wow) = &self.world.wow
            && !wow.setup_commands.is_empty()
        {
            let end = Instant::now() + Duration::from_secs(75);
            let mut setup_lease = loop {
                self.bounded()?;
                if let Some(guard) =
                    crate::wow_pool::Lease::try_acquire("account-GYMSETUP", &wow.lease_host)?
                {
                    break guard;
                }
                if Instant::now() >= end {
                    return Err(Error::episode("trusted setup account is busy"));
                }
                std::thread::sleep(Duration::from_millis(250));
            };
            let mut setup = Bridge::start_mode(&self.plan.bridge, true)?;
            self.setup_call(&mut setup, &mut setup_lease,"join",json!({"auth":wow.auth,"account":"GYMSETUP","character":"Gymsetup","create":{"race":1,"class":1}}),Duration::from_secs(75))?;
            self.setup_call(
                &mut setup,
                &mut setup_lease,
                "gm",
                json!({"command":format!(".goname {}",wow.character)}),
                Duration::from_secs(10),
            )?;
            self.setup_call(
                &mut setup,
                &mut setup_lease,
                "target",
                json!({"guid":joined["guid"]}),
                Duration::from_secs(10),
            )?;
            for command in &wow.setup_commands {
                let command = command.replace("{character}", &wow.character);
                let output = self.setup_call(
                    &mut setup,
                    &mut setup_lease,
                    "gm",
                    json!({"command":command}),
                    Duration::from_secs(10),
                )?;
                self.note(
                    Source::System,
                    "trusted WoW setup",
                    json!({"command":command,"output":output}),
                );
            }
            setup.shutdown()?;
        }
        if let Some(wow) = &self.world.wow {
            let state = self.state("verify character template")?;
            let readings = Readings {
                before: &state,
                after: &state,
                blocks: &[],
            };
            let spec = critic::Spec::All {
                checks: vec![
                    critic::Spec::AtPosition {
                        map: wow.start.map,
                        position: wow.start.position,
                        radius: wow.start.radius,
                    },
                    critic::Spec::LevelAtLeast {
                        level: wow.start.level,
                    },
                ],
            };
            if !critic::check(&spec, &readings).is_some_and(|v| v.ok)
                || state.wow["level"].as_u64() != Some(wow.start.level)
            {
                return Err(Error::episode(
                    "character reset or start template verification failed",
                ));
            }
        }
        let name = self.world.agent.username.clone();
        self.say(&format!("{name} reporting."))?;
        self.bounded()?;

        // The curriculum loop: propose, attempt, record. A task that
        // fails goes into the history the next proposal reads.
        while self.bounded().is_ok() {
            let before = self.state("read the world")?;
            let context = Context {
                state: &before,
                completed: &self.completed,
                failed: &self.failed,
            };
            let Some(task) = self.curriculum.next(&context)? else {
                break;
            };
            (self.progress)(&format!("task: {} — {}", task.id, task.goal));
            let result = self.attempt(&task, &before)?;
            if result.ok {
                self.completed.push(task.id.clone());
            } else {
                self.failed.push((task.id.clone(), result.detail.clone()));
            }
            tasks.push(result);
        }

        // Report.
        let state = self.state("report")?;
        let logs = LOG_KINDS.iter().map(|kind| state.count(kind)).sum::<i64>();
        let detail = format!(
            "Episode done: {} tasks passed, {} failed, {} skills banked, {} logs held.",
            self.completed.len(),
            self.failed.len(),
            self.banked.len(),
            logs,
        );
        self.say(&detail)?;
        self.note(
            Source::Agent,
            "reported",
            json!({
                "completed": self.completed,
                "failed": self.failed,
                "banked": self.banked,
                "logs": logs,
                "seen": self.seen.len(),
                "distance": self.distance,
            }),
        );
        if self.world.wow.is_some() {
            let completed = state.wow["turned_in"].as_array().map(Vec::len).unwrap_or(0);
            let seconds = self.started.elapsed().as_secs_f64();
            let metrics = json!({"schema":"voyager.wow.metrics/v1","quests_completed":completed,
                "quests_per_hour":completed as f64*3600.0/seconds.max(0.001),"seconds":seconds,
                "deaths":state.wow["deaths"],"earned_xp":state.wow["earned_xp"],"actions":self.actions,
                "xp_per_action":state.wow["earned_xp"].as_u64().unwrap_or(0) as f64 / self.actions.max(1) as f64,
                "cost_per_completed_quest": if completed>0 && self.writer.is_none() && self.door.is_none() && self.world.curriculum.as_ref().is_none_or(|c|c.generate.is_none()) {json!(0)} else {Value::Null},
                "cost_scope":"model calls only; excludes host infrastructure"});
            std::fs::write(
                self.run_dir.join("wow-metrics.json"),
                serde_json::to_vec_pretty(&metrics)?,
            )?;
        }
        let _ = self.bridge.disconnect();

        Ok(self.report(tasks))
    }

    /// One task: resolve a program, run it through the interpreter
    /// with repair rounds, then let the critic judge. The before
    /// state is the curriculum's own read, reused so a task's delta
    /// measures what the task did.
    fn attempt(&mut self, task: &Task, before: &AgentState) -> Result<TaskResult> {
        let Some(mut source) = self.program(task, before)? else {
            return Ok(TaskResult {
                task: task.id.clone(),
                ok: false,
                detail: "no program: no script, no matching skill, and no act door".to_string(),
            });
        };
        let mut rounds = 0u32;
        let mut last_error = String::new();
        let mut ran = false;
        let mut blocks = Vec::new();
        for round in 1..=REFINE_ROUNDS {
            rounds = round;
            self.attempts += 1;
            match self.interpret(&task.id, &source, &mut blocks) {
                Ok(_) => {
                    ran = true;
                    break;
                }
                Err(error) => {
                    last_error = error.to_string();
                    self.note(
                        Source::System,
                        &format!("round {round} failed: {error}"),
                        json!({"task": task.id, "round": round}),
                    );
                    if self.writer.is_none() || round == REFINE_ROUNDS {
                        break;
                    }
                    match self.rewrite(task, before, &source, &error)? {
                        Some(fixed) => source = fixed,
                        None => break,
                    }
                }
            }
        }
        let after = self.state(&task.id)?;
        let readings = Readings {
            before,
            after: &after,
            blocks: &blocks,
        };
        let verdict = if ran {
            critic::verify(
                &task.verify,
                &readings,
                &task.goal,
                self.door.as_mut(),
                &task.id,
            )?
        } else {
            critic::Verdict {
                ok: false,
                detail: format!("the program never ran: {last_error}"),
            }
        };
        self.log.append(&Step::called(Call {
            id: format!("verify-{}", self.attempts),
            name: format!("voyager:verify:{}", task.id),
            arguments: json!({"before":before.wow,"after":after.wow}),
            output: verdict.detail.clone(),
            outcome: if verdict.ok {
                Outcome::Completed
            } else {
                Outcome::Failed
            },
            milliseconds: 0,
            purpose: Some(task.goal.clone()),
            extra: Map::new(),
        }))?;
        self.note(
            Source::System,
            &format!(
                "{}: {}",
                if verdict.ok { "passed" } else { "failed" },
                verdict.detail
            ),
            json!({"task": task.id, "rounds": rounds, "ok": verdict.ok}),
        );
        if verdict.ok && task.bank {
            let skill = self.store.add(&slug(&task.id), &task.goal, &source)?;
            let name = format!("{}@{}", skill.name, skill.version);
            self.note(
                Source::Agent,
                &format!("banked skill {name}"),
                json!({"digest": skill.digest}),
            );
            self.banked.push(name);
        }
        Ok(TaskResult {
            task: task.id.clone(),
            ok: verdict.ok,
            detail: verdict.detail,
        })
    }

    /// Where a task's program comes from, in order: a named skill, an
    /// inline script, a skill the decisions door retrieves for the
    /// goal, or a program the `act` door writes. `None` means the
    /// task could not be attempted at all.
    fn program(&mut self, task: &Task, before: &AgentState) -> Result<Option<String>> {
        if let Some(name) = &task.skill {
            let skill = skills::lookup(&self.plan.repo, name)?;
            self.note(
                Source::Agent,
                &format!("using skill {}@{}", skill.name, skill.version),
                json!({"task": task.id, "digest": skill.digest}),
            );
            return Ok(Some(skill.source));
        }
        if let Some(script) = &task.script {
            return Ok(Some(script.clone()));
        }
        if let Some(door) = self.door.as_mut()
            && let Some(skill) = self.store.retrieve(door, &task.goal, &task.id)?
        {
            self.note(
                Source::Agent,
                &format!("retrieved skill {}@{}", skill.name, skill.version),
                json!({"task": task.id, "digest": skill.digest}),
            );
            return Ok(Some(skill.source));
        }
        if self.writer.is_some() {
            return self.write_program(task, before, None);
        }
        Ok(None)
    }

    /// One `act` door call: the goal and the state go out, a program
    /// comes back. The exchange is evidence — it lands beside the
    /// decisions as `program-N.json`.
    fn write_program(
        &mut self,
        task: &Task,
        before: &AgentState,
        previous: Option<(&str, &ScriptError)>,
    ) -> Result<Option<String>> {
        let Some(writer) = self.writer.take() else {
            return Ok(None);
        };
        let input = json!({
            "goal": task.goal,
            "state": {
                "wow": before.wow,
                "position": before.position,
                "health": before.health,
                "food": before.food,
                "inventory": before.inventory,
                "nearby_blocks": before.nearby_blocks,
                "nearby_entities": before.nearby_entities,
            },
            "previous": previous.map(|(source, error)| json!({
                "program": source,
                "error": error.to_string(),
            })),
        });
        let asked = writer.ask(PROGRAM_INSTRUCTIONS, &input.to_string());
        self.writer = Some(writer);
        let text = asked?;
        self.written += 1;
        std::fs::write(
            self.run_dir.join(format!("program-{}.json", self.written)),
            serde_json::to_vec_pretty(&json!({
                "task": task.id,
                "request": {"instructions": PROGRAM_INSTRUCTIONS, "input": input},
                "answer": text,
            }))?,
        )?;
        let source = program_text(&text);
        if source.trim().is_empty() {
            self.note(
                Source::System,
                "the act door answered no program",
                json!({"task": task.id}),
            );
            return Ok(None);
        }
        Ok(Some(source))
    }

    /// A faulted program goes back to the `act` door with its error —
    /// the refinement step of the paper's iterative prompting.
    fn rewrite(
        &mut self,
        task: &Task,
        before: &AgentState,
        source: &str,
        error: &ScriptError,
    ) -> Result<Option<String>> {
        self.write_program(task, before, Some((source, error)))
    }

    /// Runs one program under the interpreter, with the runner as its
    /// host — every op is a traced, bound-checked bridge call. Block
    /// reads land in `blocks` for the critic's `block_at` specs.
    fn interpret(
        &mut self,
        task: &str,
        source: &str,
        blocks: &mut Vec<([i32; 3], String)>,
    ) -> std::result::Result<interpret::Outcome, ScriptError> {
        let mut limits = Limits::default();
        if self.world.wow.is_some() {
            limits.operations = 4_000_000;
            limits.wall = Duration::from_secs(self.world.episode.max_seconds)
                .saturating_sub(self.started.elapsed());
        }
        let mut host = TaskHost {
            runner: self,
            task,
            blocks,
        };
        let outcome = interpret::run(&mut host, source, &limits)?;
        self.note(
            Source::System,
            &format!("program ran: {} host calls", outcome.calls),
            json!({"task": task, "returned": outcome.returned}),
        );
        Ok(outcome)
    }

    /// Setup exchanges stay outside the agent vocabulary and share episode bounds.
    fn setup_call(
        &mut self,
        setup: &mut Bridge,
        setup_lease: &mut crate::wow_pool::Lease,
        op: &str,
        args: Value,
        deadline: Duration,
    ) -> Result<Value> {
        self.bounded()?;
        let left = Duration::from_secs(self.world.episode.max_seconds)
            .saturating_sub(self.started.elapsed());
        let started = Instant::now();
        let lease = &mut self.lease;
        let result = setup.call_guarded(op, args.clone(), deadline.min(left), || {
            setup_lease.alive() && lease.as_mut().is_none_or(|l| l.alive())
        });
        self.actions += 1;
        self.log.append(&Step::called(Call {
            id: format!("a{}", self.actions),
            name: format!("wow-setup:{op}"),
            arguments: args,
            output: match &result {
                Ok(v) => v.to_string(),
                Err(e) => e.to_string(),
            },
            outcome: if result.is_ok() {
                Outcome::Completed
            } else {
                Outcome::Failed
            },
            milliseconds: started.elapsed().as_millis() as u64,
            purpose: Some("trusted world setup".into()),
            extra: Map::new(),
        }))?;
        for event in setup.drain_events() {
            self.record_event(&event)?;
        }
        result
    }

    /// One `state` call, decoded.
    fn state(&mut self, task: &str) -> Result<AgentState> {
        let result = self.call(
            "state",
            json!({"radius": SURVEY_RADIUS}),
            Duration::from_secs(60),
            task,
        )?;
        let state = AgentState::from_result(&result);
        for name in &state.nearby_blocks {
            self.seen.insert(name.clone());
        }
        Ok(state)
    }

    /// The bot says something in the world. Minecraft chat caps a line
    /// at 256 characters; the helper enforces the same bound, so the
    /// runner trims before it speaks.
    fn say(&mut self, text: &str) -> Result<()> {
        let text = trim(text, 240);
        (self.progress)(&format!("bot says: {text}"));
        self.call("say", json!({"text": text}), Duration::from_secs(30), "say")?;
        Ok(())
    }

    /// One bridge exchange: recorded as a call step with its outcome,
    /// then the events it produced recorded as steps of their own.
    fn call(&mut self, op: &str, args: Value, deadline: Duration, task: &str) -> Result<Value> {
        self.bounded()?;
        let started = Instant::now();
        let remaining = Duration::from_secs(self.world.episode.max_seconds)
            .saturating_sub(self.started.elapsed());
        let lease = &mut self.lease;
        let outcome = self
            .bridge
            .call_guarded(op, args.clone(), deadline.min(remaining), || {
                lease.as_mut().is_none_or(|l| l.alive())
            });
        let milliseconds = started.elapsed().as_millis() as u64;
        self.actions += 1;
        let (output, outcome_result) = match &outcome {
            Ok(value) => (value.to_string(), Outcome::Completed),
            Err(error) => (error.to_string(), Outcome::Failed),
        };
        self.log.append(
            &Step::called(Call {
                id: format!("a{}", self.actions),
                name: format!(
                    "{}:{op}",
                    if self.world.wow.is_some() {
                        "wow-bridge"
                    } else {
                        "mc-bridge"
                    }
                ),
                arguments: args,
                output,
                outcome: outcome_result,
                milliseconds,
                purpose: Some(task.to_string()),
                extra: Map::new(),
            })
            .by(&format!(
                "{}/{}",
                if self.world.wow.is_some() {
                    "wow-bridge"
                } else {
                    "mc-bridge"
                },
                env!("CARGO_PKG_VERSION")
            )),
        )?;
        for event in self.bridge.drain_events() {
            if let Some(text) = event.text("text") {
                self.feedback.push_back(text.to_string());
            }
            self.record_event(&event)?;
        }
        outcome
    }

    /// A lifecycle or observation step.
    fn note(&mut self, source: Source, message: &str, extra: Value) {
        let _ = self
            .log
            .append(&Step::said(source, message).noting("detail", extra));
        (self.progress)(message);
    }

    /// What the bot reported on its own.
    fn record_event(&mut self, event: &Event) -> Result<()> {
        let message = match event.event.as_str() {
            "chat" => format!("chat: {}", event.text("text").unwrap_or_default()),
            "feedback" => format!("bot: {}", event.text("text").unwrap_or_default()),
            other => format!("bot event {other}: {}", Value::from(event.fields.clone())),
        };
        self.log.append(&Step::said(Source::System, &message))?;
        (self.progress)(&message);
        Ok(())
    }

    /// The episode bounds: actions and wall time. Checked before every
    /// exchange so a run can never outrun its manifest.
    fn bounded(&mut self) -> Result<()> {
        if self.lease.as_mut().is_some_and(|l| !l.alive()) {
            return Err(Error::episode("realm lease was lost"));
        }
        if self.actions >= self.world.episode.max_actions as usize {
            return Err(Error::episode(format!(
                "the episode reached its limit of {} actions",
                self.world.episode.max_actions
            )));
        }
        if self.started.elapsed() > Duration::from_secs(self.world.episode.max_seconds) {
            return Err(Error::episode(format!(
                "the episode reached its limit of {} seconds",
                self.world.episode.max_seconds
            )));
        }
        Ok(())
    }

    /// What the episode leaves behind.
    fn report(&self, tasks: Vec<TaskResult>) -> Report {
        Report {
            world: self.world.name.clone(),
            digest: self.world.digest.clone(),
            tasks,
            actions: self.actions,
            attempts: self.attempts,
            banked: self.banked.clone(),
            seen: self.seen.len(),
            distance: self.distance,
            run_dir: self.run_dir.clone(),
            trace: self.trace.clone(),
        }
    }
}

/// The interpreter's host over a live runner: a script's op is a
/// traced, bound-checked bridge call — `interpret` never touches the
/// wire itself. Movement answers feed the traversal counter, `state`
/// answers feed the seen-set, and `block_at` answers land in `blocks`
/// where the critic's spec reads them.
struct TaskHost<'a, 'b> {
    runner: &'a mut Runner<'b>,
    /// The task id, for call purposes in the trace.
    task: &'a str,
    /// `block_at` readings this program took.
    blocks: &'a mut Vec<([i32; 3], String)>,
}

impl Host for TaskHost<'_, '_> {
    fn op(&mut self, op: &str, args: &Value) -> Result<Value> {
        let seconds = if self.runner.world.wow.is_some() {
            if ![
                "state", "say", "goto", "wait", "target", "attack", "cast", "loot", "quest", "use",
                "vendor",
            ]
            .contains(&op)
            {
                return Err(Error::episode(format!(
                    "operation {op:?} is unavailable in WoW"
                )));
            }
            args["seconds"].as_u64().unwrap_or(30).min(120) + 5
        } else {
            match op {
                "state" | "say" | "block_at" | "players" => 30,
                "goto" | "explore" => 75,
                "mine" => 90,
                "wait" => 45,
                other => {
                    return Err(Error::episode(format!(
                        "unknown bridge operation {other:?}"
                    )));
                }
            }
        };
        let result = self
            .runner
            .call(op, args.clone(), Duration::from_secs(seconds), self.task)?;
        match op {
            "state" => {
                if let Some(list) = result.get("nearby_blocks").and_then(Value::as_array) {
                    for name in list.iter().filter_map(Value::as_str) {
                        self.runner.seen.insert(name.to_string());
                    }
                }
            }
            "goto" | "explore" => {
                let from = vec3(&result, "from");
                let to = vec3(&result, "to");
                if self.runner.world.wow.is_some() {
                    self.runner.distance += result["distance"].as_f64().unwrap_or(0.0);
                } else {
                    self.runner.distance +=
                        ((to[0] - from[0]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
                }
            }
            "block_at" => {
                let position = args
                    .get("position")
                    .and_then(Value::as_array)
                    .map(|pos| {
                        [
                            pos.first().and_then(Value::as_i64).unwrap_or(0) as i32,
                            pos.get(1).and_then(Value::as_i64).unwrap_or(0) as i32,
                            pos.get(2).and_then(Value::as_i64).unwrap_or(0) as i32,
                        ]
                    })
                    .unwrap_or_default();
                if let Some(kind) = result.get("kind").and_then(Value::as_str) {
                    self.blocks.push((position, kind.to_string()));
                }
            }
            _ => {}
        }
        Ok(result)
    }

    fn feedback(&mut self) -> Vec<String> {
        self.runner.feedback.drain(..).collect()
    }
}

/// The `act` door's standing instructions: what the action language
/// is and what a reply may contain. Programs are the only output —
/// prose around them is stripped before the interpreter sees it.
const PROGRAM_INSTRUCTIONS: &str = concat!(
    "You are a game agent. Write a program that ",
    "accomplishes the stated goal. The language is Lua with these host functions:\n",
    "For WoW observations (map, quests, nearby), use move_to({x=...,y=...,z=...,seconds=60}), target({entry=...}), attack({seconds=30}), cast({spell=...,target=\"current\"}), loot({all=true}), quest({entry=...,quest=...,action=\"accept\"}), use({bag=255,slot=...}). Quest actions are accept, complete, reward. GM is unavailable.\n",
    "  say(text)              — speak in world chat\n",
    "  walk(x, z)             — path to a position (y optional: walk(x, y, z))\n",
    "  explore(dir, distance) — walk a compass direction\n",
    "  mine(names, count)     — dig nearby blocks of the named kinds\n",
    "  mine_at(positions)     — dig exact [x,y,z] positions\n",
    "  players()              — who is online and where\n",
    "  state()                — position, health, inventory, nearby blocks\n",
    "  block_at(x, y, z)      — the kind one position holds\n",
    "  wait(seconds)          — pause, at most 30\n",
    "  feedback()             — the bot's narration since the last call\n",
    "Write only the program. Keep it short — a few calls, a loop where one helps. ",
    "If a previous program and its error are given, fix it rather than starting over."
);

/// A door answer as program text: a fenced block's contents when the
/// model wrapped its code in one, else the whole answer — the
/// interpreter is the parser of record, and a prose line is a parse
/// error the refinement loop can repair.
fn program_text(answer: &str) -> String {
    let mut fenced = String::new();
    let mut inside = false;
    let mut found = false;
    for line in answer.lines() {
        if line.trim_start().starts_with("```") {
            if inside {
                found = true;
                break;
            }
            inside = true;
            continue;
        }
        if inside {
            fenced.push_str(line);
            fenced.push('\n');
        }
    }
    if found { fenced } else { format!("{answer}\n") }
}

/// A task id as a skill-store name: word characters and dashes.
fn slug(id: &str) -> String {
    let mut name = String::with_capacity(id.len().min(40));
    for c in id.chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') && !name.is_empty() {
            name.push('-');
        }
        if name.len() >= 40 {
            break;
        }
    }
    let name = name.trim_end_matches('-').to_string();
    if name.is_empty() {
        "task".to_string()
    } else {
        name
    }
}

/// `x`/`y`/`z` out of a bridge result, or zeros.
fn vec3(result: &Value, key: &str) -> [f64; 3] {
    let value = &result[key];
    [
        value["x"].as_f64().unwrap_or(0.0),
        value["y"].as_f64().unwrap_or(0.0),
        value["z"].as_f64().unwrap_or(0.0),
    ]
}

/// A chat line that will not flood the in-game chat, cut on characters.
fn trim(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let cut: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{cut}…")
    } else {
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_id_slugs_to_a_skill_name() {
        assert_eq!(slug("gather wood"), "gather-wood");
        assert_eq!(slug("walk home!"), "walk-home");
        assert_eq!(slug("!!!"), "task");
    }

    #[test]
    fn a_fenced_answer_unfences() {
        let answer = "Here you go:\n```lua\nwalk(10, 0)\nsay(\"done\")\n```";
        assert_eq!(program_text(answer), "walk(10, 0)\nsay(\"done\")\n");
    }
}
