//! One episode: server up, bot in, tasks attempted, everything traced.
//!
//! The paper's loop is a curriculum agent proposing tasks, an action
//! agent writing code, and a critic deciding success. Phase 1 keeps the
//! loop and swaps the model halves for honest stand-ins:
//!
//! - **Curriculum** is a fixed small program — survey, explore, gather —
//!   because a deterministic plan is what a harness needs before it can
//!   judge anything smarter.
//! - **Actions** are the bridge's typed ops, so what may run is a
//!   vocabulary the host owns, not generated text.
//! - **Critic** is mechanical: exploration must move the bot, gathering
//!   must change the inventory. A task that cannot prove itself fails,
//!   and the failure is recorded, not smoothed over.
//!
//! Every exchange and every event lands in an `atif` log inside the run
//! directory, beside `server.log` and the world data — an episode is a
//! thing a person can inspect afterwards, not a terminal scroll.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use atif::document::{Call, Outcome, Session, Source, Step};
use serde_json::{Map, Value, json};

use crate::bridge::{Bridge, Event};
use crate::error::{Error, Result};
use crate::server::Server;
use crate::state::AgentState;
use crate::world::World;

/// How far an explore task travels before it times out.
const EXPLORE_SECONDS: u64 = 45;
/// The blocks a survey scans, in each direction.
const SURVEY_RADIUS: u32 = 24;
/// How many log blocks the gather task asks for.
const GATHER_COUNT: u32 = 3;
/// How long the gather task may take, in all.
const GATHER_SECONDS: u64 = 120;
/// Log kinds worth digging, in rough order of usefulness.
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
/// the trace, and the counters the bounds check.
struct Runner<'a> {
    world: &'a World,
    plan: &'a Plan,
    server: Server,
    bridge: Bridge,
    log: atif::log::Log,
    run_dir: PathBuf,
    trace: PathBuf,
    actions: usize,
    seen: BTreeSet<String>,
    started: Instant,
    progress: &'a dyn Fn(&str),
}

/// Runs one episode of `world` under `plan`, narrating to `progress`.
///
/// The sequence is: server ready, bot joined, survey, explore, survey,
/// gather, report, disconnect. Any fault — a server that will not boot,
/// a helper that breaks the protocol — stops the episode and is still
/// traced.
///
/// # Errors
///
/// Returns the first [`Error`] the episode hits; the run directory and
/// trace are complete up to that point.
pub fn run(world: &World, plan: &Plan, progress: impl Fn(&str)) -> Result<Report> {
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
    progress("starting the minecraft server");
    let server = Server::start(world, &plan.jar, &plan.java, &server_dir, plan.port)?;

    let session = Session::opening(
        &run_dir
            .file_name()
            .expect("a run dir has a name")
            .to_string_lossy(),
        concat!("voyager/", env!("CARGO_PKG_VERSION")),
        "minecraft-local",
        &world.digest,
        env!("CARGO_PKG_VERSION"),
    );
    let log = atif::log::Log::create_at(&trace, &session)?;
    let mut runner = Runner {
        world,
        plan,
        server,
        bridge: Bridge::start(&plan.bridge)?,
        log,
        run_dir,
        trace,
        actions: 0,
        seen: BTreeSet::new(),
        started: Instant::now(),
        progress: &progress,
    };
    runner.note(Source::System, "server ready", json!({"port": plan.port}));

    let result = runner.episode();
    // The shutdown is part of the record too.
    runner.note(Source::System, "episode over", json!({}));
    let _ = runner.bridge.shutdown();
    let _ = runner.server.stop();
    let _ = runner.log.finish(if result.is_ok() {
        "ended"
    } else {
        "interrupted"
    });
    result
}

impl Runner<'_> {
    /// The bounded task loop.
    fn episode(&mut self) -> Result<Report> {
        let mut tasks = Vec::new();

        // Join.
        (self.progress)("joining the world");
        let address = format!("127.0.0.1:{}", self.plan.port);
        self.call(
            "join",
            json!({"address": address, "username": self.world.agent.username}),
            Duration::from_secs(75),
            "join the world",
        )?;
        let name = self.world.agent.username.clone();
        self.say(&format!("{name} reporting. Surveying the area."))?;
        self.bounded()?;

        // Task 1: survey.
        let survey = self.task_survey("survey the spawn area")?;
        tasks.push(survey);
        self.bounded()?;

        // Task 2: explore.
        let direction = "north";
        self.say("Heading north to see what is out there.")?;
        let explore = self.task_explore("explore north", direction, 64)?;
        tasks.push(explore);
        self.bounded()?;

        // Task 3: survey again — what did the walk reveal?
        let survey = self.task_survey("survey after exploring")?;
        tasks.push(survey);
        self.bounded()?;

        // Task 4: gather wood. When nothing woody is in reach, walk one
        // more leg before giving up — a task may only fail once.
        self.say("Time to gather wood.")?;
        let mut gather = self.task_gather("gather wood", GATHER_COUNT)?;
        if !gather.ok {
            self.say("No logs in reach; trying west.")?;
            let explore = self.task_explore("explore west", "west", 64)?;
            tasks.push(explore);
            self.bounded()?;
            gather = self.task_gather("gather wood", GATHER_COUNT)?;
        }
        tasks.push(gather);
        self.bounded()?;

        // Report.
        let state = self.state("report")?;
        let logs = LOG_KINDS.iter().map(|kind| state.count(kind)).sum::<i64>();
        let detail = format!(
            "Episode done: {} logs held, {} block kinds seen.",
            logs,
            self.seen.len()
        );
        self.say(&detail)?;
        self.note(
            Source::Agent,
            "reported",
            json!({"logs": logs, "seen": self.seen.len()}),
        );
        let _ = self.bridge.disconnect();

        Ok(self.report(tasks))
    }

    /// Task: read the world and say what is there. The critic accepts a
    /// survey that produced a state; what the survey learned feeds the
    /// `seen` set so later surveys can say what is new.
    fn task_survey(&mut self, task: &str) -> Result<TaskResult> {
        let state = self.state(task)?;
        let fresh: Vec<String> = state
            .nearby_blocks
            .iter()
            .filter(|name| self.seen.insert((*name).clone()))
            .cloned()
            .collect();
        let news = if fresh.is_empty() {
            "nothing new".to_string()
        } else {
            format!("new to me: {}", fresh.join(", "))
        };
        self.say(&format!(
            "Survey: {}; {}",
            state.describe(),
            trim(&news, 200)
        ))?;
        Ok(TaskResult {
            task: task.to_string(),
            ok: true,
            detail: state.describe(),
        })
    }

    /// Task: walk a direction. The critic accepts the task when the bot
    /// actually moved — a goto that ends where it began did not explore.
    fn task_explore(&mut self, task: &str, direction: &str, distance: u32) -> Result<TaskResult> {
        let args =
            json!({"direction": direction, "distance": distance, "seconds": EXPLORE_SECONDS});
        let result = self.call(
            "explore",
            args,
            Duration::from_secs(EXPLORE_SECONDS + 15),
            task,
        )?;
        let from = vec3(&result, "from");
        let to = vec3(&result, "to");
        let moved = ((to[0] - from[0]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
        let ok = moved >= 8.0;
        let detail = format!("moved {moved:.1} blocks {direction}");
        if !ok {
            self.say(&format!("Could not get anywhere {direction}: {detail}"))?;
        }
        Ok(TaskResult {
            task: task.to_string(),
            ok,
            detail,
        })
    }

    /// Task: put logs in the inventory. The critic counts them — the
    /// bridge may report `mined` blocks that burned up, despawned, or
    /// went to another player, and only what is held is evidence.
    fn task_gather(&mut self, task: &str, count: u32) -> Result<TaskResult> {
        // Find which log kinds are actually in reach; a manifest that
        // spawns in a plains biome sees oak, a spruce taiga sees spruce.
        let state = self.state(task)?;
        let visible = state.matching("_log");
        if visible.is_empty() {
            return Ok(TaskResult {
                task: task.to_string(),
                ok: false,
                detail: "no logs within the survey radius".to_string(),
            });
        }
        let names: Vec<&str> = visible.iter().take(4).map(String::as_str).collect();
        self.say(&format!("Logs in reach: {}. Digging.", names.join(", ")))?;
        let before: i64 = LOG_KINDS.iter().map(|kind| state.count(kind)).sum();
        let args = json!({
            "names": names, "count": count, "radius": 32, "seconds": GATHER_SECONDS
        });
        match self.call("mine", args, Duration::from_secs(GATHER_SECONDS + 15), task) {
            Ok(_) | Err(Error::Refused { .. }) => {}
            Err(error) => return Err(error),
        }
        let after = self.state(task)?;
        let gained: i64 = LOG_KINDS.iter().map(|kind| after.count(kind)).sum::<i64>() - before;
        let ok = gained >= 1;
        let detail = format!("gathered {gained} logs (wanted {count})");
        self.say(&format!("Gather: {detail}"))?;
        Ok(TaskResult {
            task: task.to_string(),
            ok,
            detail,
        })
    }

    /// One `state` call, decoded.
    fn state(&mut self, task: &str) -> Result<AgentState> {
        let result = self.call(
            "state",
            json!({"radius": SURVEY_RADIUS}),
            Duration::from_secs(60),
            task,
        )?;
        Ok(AgentState::from_result(&result))
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
        let outcome = self.bridge.call(op, args.clone(), deadline);
        let milliseconds = started.elapsed().as_millis() as u64;
        self.actions += 1;
        let (output, outcome_result) = match &outcome {
            Ok(value) => (value.to_string(), Outcome::Completed),
            Err(error) => (error.to_string(), Outcome::Failed),
        };
        self.log.append(
            &Step::called(Call {
                id: format!("a{}", self.actions),
                name: format!("mc-bridge:{op}"),
                arguments: args,
                output,
                outcome: outcome_result,
                milliseconds,
                purpose: Some(task.to_string()),
                extra: Map::new(),
            })
            .by(concat!("mc-bridge/", env!("CARGO_PKG_VERSION"))),
        )?;
        for event in self.bridge.drain_events() {
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
        if self.actions >= self.world.episode.max_actions as usize {
            return Err(Error::episode(format!(
                "the episode used its {} actions",
                self.world.episode.max_actions
            )));
        }
        if self.started.elapsed() > Duration::from_secs(self.world.episode.max_seconds) {
            return Err(Error::episode(format!(
                "the episode used its {} seconds",
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
            run_dir: self.run_dir.clone(),
            trace: self.trace.clone(),
        }
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
