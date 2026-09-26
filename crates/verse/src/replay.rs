//! Run replays: a retained Microcoder run plays as the player's agent
//! visiting places in the world, beside a ghost of Fable 5.1 low's cheapest
//! winning run on the same task. It is the Gym's `W` head-to-head,
//! spatialized.
//!
//! The data comes from the Gym's own readers, so a replay and `gym runs`
//! agree on every number: [`gym::runs_microcoder`] reads the run and its
//! labels, [`gym::runs_beats_winner`] names Fable's cheapest winning run,
//! and [`gym::runs_replay`] with [`gym::runs_phases`] reads Fable's
//! trajectory into placed steps.
//!
//! Each recorded event becomes a [`Visit`] to a [`Place`]:
//!
//! | Event | Place |
//! | --- | --- |
//! | A model step, or a command it ran | the workbench |
//! | A Jev question | the oracle |
//! | Knowledge retrieval, or an entry shown | the library |
//! | Acceptance tests, or the task's verifier | the proving ground |
//! | The finish | the plaza |
//!
//! Fable's trajectory is mapped coarsely: a tool call is the workbench,
//! and a step the Gym's rules place as a test or a check is the proving
//! ground.
//!
//! **Timing.** An event is written when its work ends, so a visit spans
//! from the previous event to its own: during `(previous, at]` the agent is
//! at the visit's place. Before the first event and after the last, it is
//! on the plaza.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use coder_terminal::Intensity;
use glam::Vec3;
use gym::runs::{Outcome, Run};
use gym::runs_microcoder::{self as mc, CostBasis, Knowledge, Manifest};
use gym::runs_phases::{self, ActionKind, Phase, Step};
use gym::runs_replay::{self as gr, Source};
use serde_json::Value;

use crate::world;

/// Playback speeds, as multiples of recorded time.
pub const SPEEDS: [u32; 3] = [1, 10, 60];
/// How fast the point an agent chases moves between places, in meters per
/// second of real time.
const CARROT_SPEED: f32 = 26.0;
/// How far apart the agent and the ghost stand at one place, in meters.
const SIDE: f32 = 1.8;

/// A place an agent visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The plaza's center: before a run starts and after it finishes.
    Plaza,
    /// Model steps and the commands they run.
    Workbench,
    /// Jev's typed questions.
    Oracle,
    /// Knowledge retrieval.
    Library,
    /// Acceptance tests and the task's verifier.
    ProvingGround,
}

impl Place {
    /// The places with a landmark.
    pub const LANDMARKS: [Place; 4] = [
        Place::Workbench,
        Place::Oracle,
        Place::Library,
        Place::ProvingGround,
    ];

    /// The place's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Place::Plaza => "plaza",
            Place::Workbench => "workbench",
            Place::Oracle => "oracle",
            Place::Library => "library",
            Place::ProvingGround => "proving ground",
        }
    }

    /// Where the place is on the ground.
    #[must_use]
    pub fn position(self) -> Vec3 {
        match self {
            Place::Plaza => world::PLAZA,
            Place::Workbench => world::WORKBENCH,
            Place::Oracle => world::ORACLE,
            Place::Library => world::LIBRARY,
            Place::ProvingGround => world::PROVING_GROUND,
        }
    }

    /// Where an agent hovers there: in front of the landmark, on the
    /// plaza's side, the ghost to the right of the player's agent.
    #[must_use]
    pub fn stand(self, ghost: bool) -> Vec3 {
        let at = self.position();
        let toward = (world::PLAZA - at).with_y(0.0);
        // The library's shelves face the spawn side, along -Z; every other
        // landmark faces the plaza.
        let front = if self == Place::Library || toward.length() < 1.0 {
            Vec3::NEG_Z
        } else {
            toward.normalize()
        };
        let right = front.cross(Vec3::Y);
        let reach = if self == Place::Plaza { 0.0 } else { 3.2 };
        let side = if ghost { SIDE } else { -SIDE };
        at + front * reach + right * side + Vec3::Y * crate::agent::HOVER
    }
}

/// One visit: when its event was recorded, where, what, and what it cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Visit {
    /// When the event was recorded, in milliseconds since the side started.
    pub at_ms: u64,
    pub place: Place,
    /// What happened, in a few words.
    pub what: String,
    /// What the event cost, when it records a cost.
    pub usd: Option<f64>,
}

fn clip(text: &str, limit: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= limit {
        one
    } else {
        let cut: String = one.chars().take(limit.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// One Microcoder event as a visit, or `None` for an event with no place
/// or no time.
#[must_use]
pub fn microcoder_visit(event: &Value) -> Option<Visit> {
    let seconds = event["seconds"].as_f64()?;
    let at_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let step = event["step"]
        .as_u64()
        .map_or_else(String::new, |n| format!("step {n}: "));
    let visit = |place: Place, what: String, usd: Option<f64>| {
        Some(Visit {
            at_ms,
            place,
            what,
            usd,
        })
    };
    match event["event"].as_str()? {
        "retrieved" => {
            let retrieval = &event["retrieval"];
            let count = |key: &str| retrieval[key].as_array().map_or(0, Vec::len);
            let shown = count("expanded");
            visit(
                Place::Library,
                format!(
                    "{step}knowledge: kept {}{}",
                    count("kept"),
                    if shown > 0 {
                        format!(", {shown} shown in full")
                    } else {
                        String::new()
                    }
                ),
                None,
            )
        }
        "judged" => visit(
            Place::Oracle,
            format!("{step}Jev: done, progressing, or repeating?"),
            event["judgment"]["usd"].as_f64(),
        ),
        kind @ ("disputed" | "covered" | "conformed") => visit(
            Place::Oracle,
            format!(
                "{step}Jev: {}",
                match kind {
                    "disputed" => "is a failing frozen test wrong?",
                    "covered" => "do the passing tests leave the task uncovered?",
                    _ => "does the code contradict a relevant entry?",
                }
            ),
            event["judgment"]["usd"].as_f64(),
        ),
        "generated" => {
            let generated = &event["generated"];
            let what = generated["action"]["Ok"]["rationale"]
                .as_str()
                .map_or_else(|| "a model step".to_owned(), |r| clip(r, 90));
            visit(
                Place::Workbench,
                format!("{step}{what}"),
                generated["usd"].as_f64(),
            )
        }
        "ran" => visit(
            Place::Workbench,
            format!(
                "{step}$ {}",
                clip(event["result"]["command"].as_str().unwrap_or_default(), 90)
            ),
            None,
        ),
        "tested" => {
            let results = event["results"].as_array().map_or(&[][..], Vec::as_slice);
            let passed = results.iter().filter(|r| r["exit"] == 0).count();
            visit(
                Place::ProvingGround,
                format!(
                    "{step}acceptance tests{}: {passed} of {} pass",
                    if event["froze"] == true {
                        ", frozen"
                    } else {
                        ""
                    },
                    results.len()
                ),
                None,
            )
        }
        "verified" => visit(
            Place::ProvingGround,
            format!(
                "the task's verifier: reward {}",
                event["reward"]
                    .as_f64()
                    .map_or_else(|| "none".to_owned(), |r| format!("{r}"))
            ),
            None,
        ),
        "ended" => visit(
            Place::Plaza,
            format!(
                "finished: {}",
                event["outcome"]["ending"]["reason"]
                    .as_str()
                    .unwrap_or("reason not recorded")
            ),
            None,
        ),
        _ => None,
    }
}

/// A Microcoder run's events as visits, in time order.
///
/// The task's verifier grades after the loop, and its record carries no
/// loop time in the retained runs (`seconds` is 0). A `verified` event
/// recorded before the events ahead of it is left off the timeline; the
/// HUD's result line carries its reward.
#[must_use]
pub fn microcoder_visits(events: &[Value]) -> Vec<Visit> {
    let mut latest = 0;
    let mut visits: Vec<Visit> = events
        .iter()
        .filter_map(|event| {
            let visit = microcoder_visit(event)?;
            if event["event"] == "verified" && visit.at_ms < latest {
                return None;
            }
            latest = latest.max(visit.at_ms);
            Some(visit)
        })
        .collect();
    visits.sort_by_key(|v| v.at_ms);
    visits
}

/// Fable's placed trajectory steps as visits: its finish call is the
/// plaza, a step the rules place as a test or a check is the proving
/// ground, and every other tool call is the workbench.
#[must_use]
pub fn fable_visits(steps: &[Step]) -> Vec<Visit> {
    let mut visits: Vec<Visit> = steps
        .iter()
        .map(|step| {
            let place = if step.kind == ActionKind::Finish {
                Place::Plaza
            } else if matches!(step.phase, Some(Phase::Test | Phase::Verify)) {
                Place::ProvingGround
            } else {
                Place::Workbench
            };
            let input = clip(&step.input, 90);
            let mut what = if input.is_empty() {
                step.tool.clone()
            } else {
                format!("{}: {input}", step.tool)
            };
            if step.by == "jev" {
                what.push_str(" (phase: Jev)");
            }
            Visit {
                at_ms: step.elapsed_ms,
                place,
                what,
                usd: None,
            }
        })
        .collect();
    visits.sort_by_key(|v| v.at_ms);
    visits
}

/// One side of a replay.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Who it is: `Microcoder · gpt-6-luna` or `Fable 5.1 low · Claude Code`.
    pub who: String,
    pub visits: Vec<Visit>,
    /// Before this, the side hasn't started: setup records it doesn't
    /// place, such as a public trial's container start.
    pub start_ms: u64,
    pub duration_ms: u64,
    /// What the time measures.
    pub time_basis: &'static str,
    /// The whole run's cost, when known.
    pub total_usd: Option<f64>,
    /// `billed cost`, `list-price cost`, or `reported cost`.
    pub cost_basis: String,
    /// Whether events record costs, so a running cost means something.
    pub per_event_cost: bool,
    /// `passed`, `failed`, or `not graded`, with the reward.
    pub result: String,
}

impl Track {
    /// The visit in progress at `t_ms`: the first whose event is recorded
    /// at or after it. `None` before the side starts and after its last
    /// event, when it is on the plaza.
    #[must_use]
    pub fn current(&self, t_ms: f64) -> Option<usize> {
        if t_ms <= self.start_ms as f64 {
            return None;
        }
        self.visits.iter().position(|v| v.at_ms as f64 >= t_ms)
    }

    /// Where the side is at `t_ms`.
    #[must_use]
    pub fn place_at(&self, t_ms: f64) -> Place {
        self.current(t_ms)
            .map_or(Place::Plaza, |i| self.visits[i].place)
    }

    /// What the events recorded at or before `t_ms` cost.
    #[must_use]
    pub fn cost_at(&self, t_ms: f64) -> f64 {
        self.visits
            .iter()
            .take_while(|v| v.at_ms as f64 <= t_ms)
            .filter_map(|v| v.usd)
            .sum()
    }

    /// Whether the side has finished by `t_ms`.
    #[must_use]
    pub fn done(&self, t_ms: f64) -> bool {
        t_ms >= self.duration_ms as f64
    }
}

/// The playback clock both sides share: elapsed time since each side's
/// start, at 1×, 10×, or 60×.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub elapsed_ms: f64,
    pub duration_ms: u64,
    pub playing: bool,
    pub speed: u32,
}

impl Clock {
    /// A clock at zero, playing at 1×.
    #[must_use]
    pub fn new(duration_ms: u64) -> Self {
        Self {
            elapsed_ms: 0.0,
            duration_ms,
            playing: true,
            speed: SPEEDS[0],
        }
    }

    /// Advances by `dt` seconds of real time at the current speed, and
    /// stops at the end.
    pub fn advance(&mut self, dt: f32) {
        if self.playing {
            self.elapsed_ms = (self.elapsed_ms + f64::from(dt) * 1000.0 * f64::from(self.speed))
                .min(self.duration_ms as f64);
        }
        if self.elapsed_ms >= self.duration_ms as f64 {
            self.playing = false;
        }
    }

    /// Sets the speed to `speed`, when it is one of [`SPEEDS`].
    pub fn set_speed(&mut self, speed: u32) {
        if SPEEDS.contains(&speed) {
            self.speed = speed;
        }
    }

    /// Plays or pauses; at the end, plays again from the start.
    pub fn toggle(&mut self) {
        if !self.playing && self.elapsed_ms >= self.duration_ms as f64 {
            self.elapsed_ms = 0.0;
        }
        self.playing = !self.playing;
    }

    /// Jumps to `ms`, within the replay.
    pub fn seek(&mut self, ms: f64) {
        self.elapsed_ms = ms.clamp(0.0, self.duration_ms as f64);
    }
}

/// A replay: the run, its ghost, and the shared clock.
#[derive(Clone, Debug)]
pub struct Replay {
    pub task: String,
    /// The Gym's run ID: `microcoder/<run directory>`.
    pub run_id: String,
    /// The labels every number about the run carries.
    pub labels: String,
    pub mine: Track,
    /// Fable's cheapest winning run, or why there is no ghost.
    pub ghost: Result<Track, String>,
    pub clock: Clock,
    /// The points the two agents chase, moving between places at a
    /// bounded speed.
    carrots: [Vec3; 2],
}

/// Unix milliseconds now.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The knowledge base the Gym reads a run's labels against.
fn knowledge() -> Knowledge {
    Knowledge::read(&knowledge::default_dir())
}

/// Finds a Microcoder run by its directory, its Gym run ID
/// (`microcoder/<name>`), or its directory name, in this computer's runs
/// and the checkout's retained runs.
///
/// # Errors
///
/// Returns a message when no such run exists.
pub fn find(arg: &str) -> Result<Run, String> {
    let path = Path::new(arg);
    if path.join("events.jsonl").is_file() {
        let manifest = path.parent().and_then(Manifest::read);
        return Ok(mc::read(path, &knowledge(), manifest.as_ref(), now_ms()));
    }
    let name = arg
        .strip_prefix(&format!("{}/", mc::JOB))
        .unwrap_or(arg)
        .trim_end_matches('/');
    for dir in mc::standard_dirs() {
        let candidate = dir.join(name);
        if candidate.join("events.jsonl").is_file() {
            let manifest = Manifest::read(&dir);
            return Ok(mc::read(
                &candidate,
                &knowledge(),
                manifest.as_ref(),
                now_ms(),
            ));
        }
    }
    Err(format!(
        "no Microcoder run {arg}: give a run directory with an events.jsonl, or a Gym run ID such as microcoder/<task>-<stamp> from ~/.openagents/microcoder/runs or {}",
        mc::RETAINED
    ))
}

/// The retained host directories in the checkout.
fn retained_dirs() -> Vec<PathBuf> {
    mc::standard_dirs()
        .into_iter()
        .filter(|dir| dir.to_string_lossy().contains(mc::RETAINED))
        .collect()
}

/// One retained run the `beats-winner` rule cites, for the replay list.
#[derive(Clone, Debug)]
pub struct Choice {
    pub run: Run,
    /// The run, and its cost and time against Fable's cheapest winning
    /// run.
    pub line: String,
    /// The labels its numbers carry.
    pub labels: String,
}

/// The retained Microcoder passes the Gym's `beats-winner` rule cites:
/// cheaper than Fable 5.1 low's cheapest winning run on the task, or
/// faster than its fastest. Newest first within a task.
#[must_use]
pub fn beats_winner_runs() -> Vec<Choice> {
    let runs = mc::read_all(&retained_dirs(), &knowledge(), now_ms());
    let answers = HashMap::new();
    let marks = gym::runs_marks::Marks::default();
    let inputs = gym::runs_highlights::Inputs {
        runs: &runs,
        answers: &answers,
        reference: None,
        marks: &marks,
        fable: gym::runs_beats_winner::default_manifest(),
    };
    let usd = gym::runs_analysis::usd;
    let mut choices = Vec::new();
    for highlight in gym::runs_beats_winner::beats_winner(&inputs) {
        let Some(detail) = &highlight.detail else {
            continue;
        };
        let cheapest = &detail["reference"]["cheapest"];
        let mut passes: Vec<&Value> = detail["passes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["cheaper"] == true || p["faster"] == true)
            .collect();
        passes.sort_by(|a, b| b["run"].as_str().cmp(&a["run"].as_str()));
        for pass in passes {
            let Some(run) = runs
                .iter()
                .find(|r| Some(r.id().as_str()) == pass["run"].as_str())
            else {
                continue;
            };
            let money = |v: &Value| v.as_f64().map_or_else(|| "cost unknown".to_owned(), usd);
            let time = |v: &Value| {
                v.as_f64().map_or_else(
                    || "time unknown".to_owned(),
                    |s| gym::runs::duration((s * 1000.0).round() as u64),
                )
            };
            let line = format!(
                "{} · {} vs {} · {} vs {}",
                run.trial,
                money(&pass["cost_usd"]),
                money(&cheapest["cost_usd"]),
                time(&pass["seconds"]),
                time(&cheapest["seconds"]),
            );
            choices.push(Choice {
                run: run.clone(),
                line,
                labels: run
                    .microcoder
                    .as_deref()
                    .map_or_else(String::new, mc::Microcoder::labels),
            });
        }
    }
    choices
}

fn outcome_text(outcome: &Outcome, reward: Option<f64>) -> String {
    match reward {
        Some(r) => format!("{}, reward {r}", outcome.word()),
        None => outcome.word().to_owned(),
    }
}

/// The Microcoder side of a run.
///
/// # Errors
///
/// Returns a message when the run has no event log.
pub fn microcoder_track(run: &Run) -> Result<Track, String> {
    let path = run
        .files
        .live
        .as_ref()
        .ok_or_else(|| format!("{} has no events.jsonl", run.id()))?;
    let (events, _) = mc::events(path);
    let visits = microcoder_visits(&events);
    if visits.is_empty() {
        return Err(format!("{} records no events to replay", run.id()));
    }
    let m = run.microcoder.as_deref();
    let last = visits.last().map_or(0, |v| v.at_ms);
    Ok(Track {
        who: format!(
            "Microcoder · {}",
            run.model.as_deref().unwrap_or("model unknown")
        ),
        visits,
        start_ms: 0,
        duration_ms: run.agent_ms.unwrap_or(0).max(last),
        time_basis: "loop time",
        total_usd: run.cost_usd,
        cost_basis: m
            .and_then(|m| m.cost_basis)
            .unwrap_or(CostBasis::Unknown)
            .phrase()
            .to_owned(),
        per_event_cost: true,
        result: outcome_text(&run.outcome, run.reward),
    })
}

/// The ghost: Fable 5.1 low's cheapest winning run on `task`, read from
/// the public replay cache.
///
/// # Errors
///
/// Returns why there is no ghost: no winning run, or its transcript isn't
/// on this computer.
pub fn fable_track(task: &str) -> Result<Track, String> {
    let manifest_path = gym::runs_analysis::fable_manifest_path();
    let manifest = gym::runs_beats_winner::default_manifest()
        .ok_or_else(|| format!("can't read {}", manifest_path.display()))?;
    let reference = gym::runs_beats_winner::reference_task(manifest, task);
    let winner = reference.cheapest().ok_or_else(|| {
        format!(
            "{} has no winning run with a cost on {task} ({} attempts)",
            gym::runs_beats_winner::REFERENCE,
            reference.attempts
        )
    })?;
    let cache = gr::cache_dir();
    let source = gr::public_sources(&cache, &manifest_path)?
        .into_iter()
        .find(|s| s.id() == winner.id)
        .ok_or_else(|| format!("the replay manifest doesn't list trial {}", winner.id))?;
    let Source::Public { trial, .. } = &source else {
        return Err("not a public trial".to_owned());
    };
    if !cache.join(&trial.file).is_file() {
        return Err(format!(
            "Fable's transcript {} isn't on this computer; fetch it with `cd bench/terminal-bench && uv run python -m tbench.public_replays`",
            trial.id
        ));
    }
    let replay = gr::Replay::load(&source)?;
    // Steps the rules leave unplaced take Jev's stored phase when `gym runs
    // fingerprint` or `gym runs moves` asked for one; a replay never asks.
    let mut steps = runs_phases::steps(&replay);
    let store = runs_phases::StepStore::open(runs_phases::default_dir());
    runs_phases::apply(&mut steps, task, &store);
    let visits = fable_visits(&steps);
    Ok(Track {
        who: format!("{} · {}", gym::runs_beats_winner::REFERENCE, trial.agent),
        start_ms: replay.events.first().map_or(0, |e| e.elapsed_ms),
        visits,
        duration_ms: replay.duration_ms,
        time_basis: "trial wall time",
        total_usd: trial.cost_usd,
        cost_basis: "reported cost".to_owned(),
        per_event_cost: false,
        result: match trial.reward {
            Some(r) if r >= 1.0 => format!("passed, reward {r}"),
            Some(r) => format!("failed, reward {r}"),
            None => "not graded".to_owned(),
        },
    })
}

/// `2:21`, or `1:02:03` past an hour.
fn clock_text(ms: f64) -> String {
    let s = (ms / 1000.0).floor() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

impl Replay {
    /// Loads a replay of `run` and its ghost. The player's agent starts
    /// from `from`.
    ///
    /// # Errors
    ///
    /// Returns a message when the run can't be replayed.
    pub fn load(run: &Run, from: Vec3) -> Result<Self, String> {
        let mine = microcoder_track(run)?;
        let ghost = fable_track(&run.task);
        let labels = run
            .microcoder
            .as_deref()
            .map_or_else(|| "labels unknown".to_owned(), mc::Microcoder::labels);
        Ok(Self::new(
            [run.task.clone(), run.id(), labels],
            mine,
            ghost,
            from,
        ))
    }

    /// A replay of two tracks already read. `names` is the task, the Gym
    /// run ID, and the labels.
    #[must_use]
    pub fn new(names: [String; 3], mine: Track, ghost: Result<Track, String>, from: Vec3) -> Self {
        let duration = mine
            .duration_ms
            .max(ghost.as_ref().map_or(0, |g| g.duration_ms));
        let [task, run_id, labels] = names;
        Self {
            task,
            run_id,
            labels,
            mine,
            ghost,
            clock: Clock::new(duration),
            carrots: [from, Place::Plaza.stand(true)],
        }
    }

    /// Where each side wants to be now: the player's agent, then the ghost.
    #[must_use]
    pub fn places(&self) -> (Place, Option<Place>) {
        let t = self.clock.elapsed_ms;
        (
            self.mine.place_at(t),
            self.ghost.as_ref().ok().map(|g| g.place_at(t)),
        )
    }

    /// Advances the clock by `dt` seconds of real time and moves the
    /// points the agents chase toward their places.
    pub fn tick(&mut self, dt: f32) {
        self.clock.advance(dt);
        let (mine, ghost) = self.places();
        let goals = [mine.stand(false), ghost.unwrap_or(Place::Plaza).stand(true)];
        for (carrot, goal) in self.carrots.iter_mut().zip(goals) {
            let d = goal - *carrot;
            let step = CARROT_SPEED * dt;
            *carrot = if d.length() <= step {
                goal
            } else {
                *carrot + d.normalize() * step
            };
        }
    }

    /// Puts both chased points at their places now, as a capture needs.
    pub fn settle(&mut self) {
        let (mine, ghost) = self.places();
        self.carrots = [mine.stand(false), ghost.unwrap_or(Place::Plaza).stand(true)];
    }

    /// The points the player's agent and the ghost chase.
    #[must_use]
    pub fn carrots(&self) -> [Vec3; 2] {
        self.carrots
    }

    /// The HUD lines: the task and labels, each side's time, cost, place,
    /// and step, each side's result once it finishes, and the keys.
    #[must_use]
    pub fn hud_lines(&self) -> Vec<(String, Intensity)> {
        let usd = gym::runs_analysis::usd;
        let t = self.clock.elapsed_ms;
        let state = if self.clock.playing {
            "playing"
        } else if t >= self.clock.duration_ms as f64 {
            "ended"
        } else {
            "paused"
        };
        let mut out = vec![
            (
                format!("REPLAY · {} · {}× · {state}", self.task, self.clock.speed),
                Intensity::Full,
            ),
            (format!("[{}]", self.labels), Intensity::ThreeQuarters),
        ];
        let mut side = |track: &Track, ghost: bool| {
            let shown = t.min(track.duration_ms as f64);
            let cost = if track.per_event_cost {
                format!("{} so far", usd(track.cost_at(t)))
            } else {
                track.total_usd.map_or_else(
                    || "cost unknown".to_owned(),
                    |c| format!("{} total, not per step", usd(c)),
                )
            };
            let (place, step) = match track.current(t) {
                Some(i) => (track.visits[i].place, track.visits[i].what.as_str()),
                None if track.done(t) || t > track.start_ms as f64 => (Place::Plaza, "finished"),
                None => (Place::Plaza, "not started"),
            };
            out.push((
                format!(
                    "{}{} · {} / {} {} · {cost} · at the {}",
                    track.who,
                    if ghost { " (ghost)" } else { "" },
                    clock_text(shown),
                    clock_text(track.duration_ms as f64),
                    track.time_basis,
                    place.name()
                ),
                if ghost {
                    Intensity::ThreeQuarters
                } else {
                    Intensity::Full
                },
            ));
            out.push((format!("  {step}"), Intensity::Half));
        };
        side(&self.mine, false);
        match &self.ghost {
            Ok(ghost) => side(ghost, true),
            Err(why) => out.push((format!("No ghost: {why}"), Intensity::Half)),
        }
        let result = |track: &Track| {
            if !track.done(t) {
                return format!("{}: running", track.who);
            }
            format!(
                "{}: {} · {} {} · {} {}",
                track.who,
                track.result,
                track
                    .total_usd
                    .map_or_else(|| "cost unknown".to_owned(), usd),
                track.cost_basis,
                gym::runs::duration(track.duration_ms),
                track.time_basis
            )
        };
        out.push((
            format!("Result · {}", result(&self.mine)),
            Intensity::ThreeQuarters,
        ));
        if let Ok(ghost) = &self.ghost {
            out.push((
                format!("Result · {}", result(ghost)),
                Intensity::ThreeQuarters,
            ));
        }
        out.push((
            "Times differ in kind: Microcoder's loop time, Fable's public trial wall time with setup and grading."
                .to_owned(),
            Intensity::Quarter,
        ));
        out.push((
            "1 / 2 / 3: 1× 10× 60× · P pause · Home restart · R runs · Esc stops".to_owned(),
            Intensity::Quarter,
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn track(visits: Vec<Visit>, duration_ms: u64) -> Track {
        Track {
            who: "test".into(),
            visits,
            start_ms: 0,
            duration_ms,
            time_basis: "loop time",
            total_usd: Some(0.01),
            cost_basis: "billed cost".into(),
            per_event_cost: true,
            result: "passed, reward 1".into(),
        }
    }

    fn at(at_ms: u64, place: Place, usd: Option<f64>) -> Visit {
        Visit {
            at_ms,
            place,
            what: String::new(),
            usd,
        }
    }

    #[test]
    fn each_microcoder_event_visits_its_place() {
        let events = [
            json!({"event": "started", "task": "t"}),
            json!({"event": "retrieved", "seconds": 0.8, "step": 1,
                   "retrieval": {"kept": [{"id": "a"}], "expanded": [["a", 1]]}}),
            json!({"event": "judged", "seconds": 0.9, "step": 1, "judgment": {"usd": 0.001}}),
            json!({"event": "generated", "seconds": 3.5, "step": 1,
                   "generated": {"usd": 0.004, "action": {"Ok": {"rationale": "Look at the files."}}}}),
            json!({"event": "ran", "seconds": 3.6, "step": 1, "result": {"command": "ls -la"}}),
            json!({"event": "tested", "seconds": 9.0, "step": 1, "froze": true,
                   "results": [{"exit": 0}, {"exit": 1}]}),
            json!({"event": "disputed", "seconds": 9.2, "step": 1, "judgment": {"usd": 0.002}}),
            json!({"event": "ended", "seconds": 12.0, "outcome": {"ending": {"reason": "finished"}}}),
            json!({"event": "verified", "seconds": 20.0, "reward": 1.0}),
            json!({"event": "something-new", "seconds": 21.0}),
            // An untimed verifier record, as the retained runs write it.
            json!({"event": "verified", "seconds": 0.0, "reward": 1.0}),
        ];
        let visits = microcoder_visits(&events);
        let places: Vec<Place> = visits.iter().map(|v| v.place).collect();
        assert_eq!(
            places,
            [
                Place::Library,
                Place::Oracle,
                Place::Workbench,
                Place::Workbench,
                Place::ProvingGround,
                Place::Oracle,
                Place::Plaza,
                Place::ProvingGround,
            ]
        );
        assert_eq!(visits[0].at_ms, 800);
        assert_eq!(visits[0].what, "step 1: knowledge: kept 1, 1 shown in full");
        assert_eq!(visits[2].what, "step 1: Look at the files.");
        assert_eq!(visits[3].what, "step 1: $ ls -la");
        assert_eq!(
            visits[4].what,
            "step 1: acceptance tests, frozen: 1 of 2 pass"
        );
        assert_eq!(visits[6].what, "finished: finished");
        assert_eq!(visits[2].usd, Some(0.004));
    }

    #[test]
    fn fable_tool_calls_are_the_workbench_and_tests_the_proving_ground() {
        let step = |ms: u64, phase: Option<Phase>, kind: ActionKind| Step {
            n: 1,
            event: 0,
            elapsed_ms: ms,
            tool: "Bash".into(),
            input: "pytest -q".into(),
            failed: None,
            rule: phase,
            phase,
            by: "rule",
            writes: Vec::new(),
            executes: false,
            repeats_failed: false,
            jev: None,
            output: String::new(),
            kind,
        };
        let visits = fable_visits(&[
            step(5_000, Some(Phase::Read), ActionKind::Read("a.py".into())),
            step(2_000, Some(Phase::Orient), ActionKind::List),
            step(
                9_000,
                Some(Phase::Edit),
                ActionKind::Edit(vec!["a.py".into()]),
            ),
            step(12_000, Some(Phase::Test), ActionKind::Command),
            step(14_000, Some(Phase::Verify), ActionKind::Command),
            step(15_000, None, ActionKind::Other),
            step(16_000, Some(Phase::Finish), ActionKind::Finish),
        ]);
        let places: Vec<(u64, Place)> = visits.iter().map(|v| (v.at_ms, v.place)).collect();
        assert_eq!(
            places,
            [
                (2_000, Place::Workbench),
                (5_000, Place::Workbench),
                (9_000, Place::Workbench),
                (12_000, Place::ProvingGround),
                (14_000, Place::ProvingGround),
                (15_000, Place::Workbench),
                (16_000, Place::Plaza),
            ]
        );
        assert_eq!(visits[0].what, "Bash: pytest -q");
    }

    #[test]
    fn a_visit_spans_from_the_previous_event_to_its_own() {
        let t = track(
            vec![
                at(1_000, Place::Library, None),
                at(3_000, Place::Oracle, Some(0.001)),
                at(8_000, Place::Workbench, Some(0.004)),
            ],
            10_000,
        );
        assert_eq!(
            t.place_at(0.0),
            Place::Plaza,
            "on the plaza before it starts"
        );
        assert_eq!(t.place_at(500.0), Place::Library);
        assert_eq!(
            t.place_at(1_000.0),
            Place::Library,
            "the event ends its visit"
        );
        assert_eq!(t.place_at(1_001.0), Place::Oracle);
        assert_eq!(t.place_at(3_000.0), Place::Oracle);
        assert_eq!(t.place_at(5_000.0), Place::Workbench);
        assert_eq!(
            t.place_at(8_001.0),
            Place::Plaza,
            "back after its last event"
        );
        assert!(t.cost_at(2_999.0).abs() < 1e-12);
        assert!((t.cost_at(3_000.0) - 0.001).abs() < 1e-12);
        assert!((t.cost_at(10_000.0) - 0.005).abs() < 1e-12);
        assert!(!t.done(9_999.0) && t.done(10_000.0));
    }

    #[test]
    fn a_side_that_starts_late_waits_on_the_plaza() {
        let mut t = track(vec![at(40_000, Place::Workbench, None)], 60_000);
        t.start_ms = 30_000;
        assert_eq!(t.place_at(20_000.0), Place::Plaza);
        assert_eq!(t.place_at(35_000.0), Place::Workbench);
    }

    #[test]
    fn the_clock_plays_at_1_10_and_60_times() {
        let mut clock = Clock::new(600_000);
        clock.advance(1.0);
        assert!((clock.elapsed_ms - 1_000.0).abs() < 1e-6);
        clock.set_speed(10);
        clock.advance(1.0);
        assert!((clock.elapsed_ms - 11_000.0).abs() < 1e-3);
        clock.set_speed(60);
        clock.advance(0.5);
        assert!((clock.elapsed_ms - 41_000.0).abs() < 1e-3);
        clock.set_speed(7);
        assert_eq!(clock.speed, 60, "only the listed speeds");
        clock.toggle();
        clock.advance(5.0);
        assert!((clock.elapsed_ms - 41_000.0).abs() < 1e-3, "paused");
        clock.toggle();
        clock.advance(100.0);
        assert_eq!(clock.elapsed_ms, 600_000.0, "stops at the end");
        assert!(!clock.playing);
        clock.toggle();
        assert_eq!(clock.elapsed_ms, 0.0, "playing at the end restarts");
    }

    #[test]
    fn the_agents_travel_at_a_bounded_speed_and_arrive() {
        let mine = track(vec![at(1_000, Place::ProvingGround, None)], 60_000);
        let names = [
            "t".to_owned(),
            "microcoder/t-1".to_owned(),
            "labels".to_owned(),
        ];
        let mut replay = Replay::new(names, mine, Err("none".into()), Place::Plaza.stand(false));
        replay.clock.set_speed(60);
        let from = replay.carrots()[0];
        replay.tick(0.1);
        let moved = replay.carrots()[0].distance(from);
        assert!(moved <= CARROT_SPEED * 0.1 + 1e-3, "moved {moved}");
        for _ in 0..600 {
            replay.tick(1.0 / 60.0);
            if replay.clock.elapsed_ms > 1_000.0 {
                break;
            }
        }
        // Past its only event it heads back to the plaza.
        for _ in 0..600 {
            replay.tick(1.0 / 60.0);
        }
        assert!(replay.carrots()[0].distance(Place::Plaza.stand(false)) < 1e-3);
    }

    #[test]
    fn places_are_apart_and_the_ghost_stands_beside_the_agent() {
        for a in Place::LANDMARKS {
            for b in Place::LANDMARKS {
                if a != b {
                    assert!(a.position().distance(b.position()) > 12.0, "{a:?} {b:?}");
                }
            }
            let d = a.stand(true).distance(a.stand(false));
            assert!((d - 2.0 * SIDE).abs() < 1e-3);
        }
    }

    fn retained(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(mc::RETAINED)
            .join("coderos-4080")
            .join(name)
    }

    #[test]
    fn a_retained_win_replays_through_the_gym_reader() {
        let dir = retained("embedding-drift-monitor-1790393791");
        let run = find(dir.to_str().unwrap()).expect("the retained run reads");
        assert_eq!(run.task, "embedding-drift-monitor");
        let mine = microcoder_track(&run).unwrap();
        // Its first event is step 1's retrieval, 0.82 s in.
        assert_eq!(mine.visits[0].place, Place::Library);
        assert_eq!(mine.visits[0].at_ms, 823);
        assert_eq!(mine.place_at(900.0), Place::Oracle);
        assert!(mine.visits.iter().any(|v| v.place == Place::ProvingGround));
        // Its verifier record is untimed, so the loop ends on the plaza.
        assert_eq!(mine.visits.last().unwrap().place, Place::Plaza);
        assert_eq!(mine.duration_ms, run.agent_ms.unwrap());
        let by_id = find("microcoder/embedding-drift-monitor-1790393791").unwrap();
        assert_eq!(by_id.id(), run.id());
        assert!(find("microcoder/no-such-run-1").is_err());
    }

    #[test]
    fn the_list_is_the_retained_passes_that_beat_fable() {
        let choices = beats_winner_runs();
        let names: Vec<&str> = choices.iter().map(|c| c.run.trial.as_str()).collect();
        for task in [
            "embedding-drift-monitor",
            "gsea-proteomics",
            "fin-saccr-rwa",
        ] {
            assert!(
                names.iter().any(|n| n.starts_with(task)),
                "{task} has a retained win: {names:?}"
            );
        }
        for choice in &choices {
            assert_eq!(choice.run.outcome, Outcome::Passed);
            assert!(choice.run.retained);
            assert!(
                choice.labels.contains("knowledge-assisted"),
                "{}",
                choice.labels
            );
            assert!(choice.line.contains(" vs "), "{}", choice.line);
        }
    }

    #[test]
    fn the_ghost_is_fables_cheapest_winning_run_when_its_transcript_is_here() {
        match fable_track("embedding-drift-monitor") {
            Ok(ghost) => {
                assert_eq!(ghost.time_basis, "trial wall time");
                assert!(!ghost.per_event_cost);
                assert!(ghost.result.starts_with("passed"));
                assert!(!ghost.visits.is_empty());
                assert!(ghost.visits.windows(2).all(|w| w[0].at_ms <= w[1].at_ms));
                assert!(ghost.start_ms <= ghost.visits[0].at_ms);
            }
            // Transcripts are fetched per computer; the reason says how.
            Err(why) => assert!(why.contains("tbench.public_replays"), "{why}"),
        }
    }
}
