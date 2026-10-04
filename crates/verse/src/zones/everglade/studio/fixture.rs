//! The Agent Studio's simulated team as a studio [`Source`]: the snapshot
//! fixture Everglade's tests and captures draw, with no network and no
//! model (`docs/verse/agent-studio.md`, "Simulated team").
//!
//! [`Recording::run`] drives `coder::task::studio_sim`'s scripted team
//! through its whole script against a scratch repository, with real
//! worktrees, commits, reviews, and landings, and keeps the studio's view
//! after every step, exactly as the host's `studio.snapshot` builds it.
//!
//! The scripted engine takes each turn inside one step, so between steps no
//! seat is mid-turn. To show seats at work, the recording also expands each
//! turn into the ATIF steps that turn would record (its reads, its writes,
//! its check, and its commit, or its question, approval, or plan) and keeps
//! a view after each one, with the seat's activity and station from
//! `atif::classify` and its log tail in the host's display form. Those
//! frames are presentation only; the script's own frames are the host's
//! views unchanged.
//!
//! [`Player`] plays a recording through the real NIP-HOST studio exchange:
//! a host-side `Stream` answers the operations a client `Mirror` asks for,
//! a full snapshot first and sequenced updates after, so the view Everglade
//! draws is the mirror's, as it would be from a host.

use super::Source;
use coder::task::studio_sim::{self, Step, Team, Turn};
use coder_access::review::TaskReview;
use coder_access::studio::{self as wire, Mirror, Snapshot, Stream, View};
use coder_access::{Error, Operation, Outcome};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// The stream identity the fixture's host side answers with.
const STREAM: &str = "e7e61ade";

/// One view of the studio in a recording.
#[derive(Clone, Debug)]
pub struct Frame {
    /// What led to it: a script step, or one step of a turn.
    pub label: String,
    pub view: View,
}

/// The simulated team's studio, after every step of its script.
#[derive(Clone, Debug)]
pub struct Recording {
    frames: Vec<Frame>,
    /// The newest review the person read of each task, by task identity.
    reviews: BTreeMap<String, TaskReview>,
}

impl Recording {
    /// Runs the simulated team's script in `dir`, which must be absent or
    /// empty, and keeps the studio's view after each step and each step of
    /// a turn.
    ///
    /// # Errors
    /// The scratch repository cannot be made, Git fails, or a script step
    /// is refused.
    pub fn run(dir: &Path) -> Result<Self, String> {
        let scratch = studio_sim::Fixture::create(dir).map_err(|e| e.to_string())?;
        let mut team = Team::new(scratch).map_err(|e| e.to_string())?;
        let mut frames = vec![Frame {
            label: "seated".into(),
            view: view_of(&team),
        }];
        let mut taken: BTreeMap<&str, usize> = BTreeMap::new();
        for step in studio_sim::SCRIPT {
            if let Step::Run(key) = *step {
                let index = taken.get(key).copied().unwrap_or(0);
                if let Some(turn) = studio_sim::turns(key).get(index) {
                    let task = team.task_id(key).map_err(|e| e.to_string())?;
                    let base = view_of(&team);
                    let steps = steps_of(turn);
                    for done in 1..=steps.len() {
                        let at = at_ms(frames.len());
                        frames.push(Frame {
                            label: format!("{key}: {}", steps[done - 1].0),
                            view: working(&base, &task, &steps[..done], at),
                        });
                    }
                }
                *taken.entry(key).or_default() += 1;
            }
            team.step(*step).map_err(|e| e.to_string())?;
            frames.push(Frame {
                label: format!("{step:?}"),
                view: view_of(&team),
            });
        }
        let mut reviews = BTreeMap::new();
        for key in ["lead", "greet", "docs", "release"] {
            if let Some(review) = team.reviews(key).last() {
                reviews.insert(review.task.clone(), review.clone());
            }
        }
        Ok(Self { frames, reviews })
    }

    /// Every frame, oldest first.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// The first frame whose view satisfies `test`.
    pub fn find(&self, test: impl Fn(&View) -> bool) -> Option<usize> {
        self.frames.iter().position(|frame| test(&frame.view))
    }

    /// The newest review read of `task`.
    #[must_use]
    pub fn review(&self, task: &str) -> Option<&TaskReview> {
        self.reviews.get(task)
    }
}

/// The team's studio as the host's `studio.snapshot` builds it.
fn view_of(team: &Team) -> View {
    team.studio().wire(team.inbox(), &team.fixture().store)
}

/// When the frame at `index` happened, in milliseconds since the epoch, on
/// the simulated team's fixed clock.
fn at_ms(index: usize) -> u64 {
    (studio_sim::START + index as u64 * 10) * 1000
}

fn call(name: &str, arguments: Value, purpose: &str, extra: Map<String, Value>) -> atif::Step {
    atif::Step::called(atif::Call {
        id: name.into(),
        name: name.into(),
        arguments,
        output: String::new(),
        outcome: atif::Outcome::Completed,
        milliseconds: 1,
        purpose: Some(purpose.into()),
        extra,
    })
}

/// The ATIF steps `turn` would record, each with a short label.
fn steps_of(turn: &Turn) -> Vec<(String, atif::Step)> {
    let none = Map::new;
    match *turn {
        Turn::Ask(question) => vec![
            (
                "read README.md".into(),
                call(
                    "read_file",
                    json!({"path": "README.md"}),
                    "README.md",
                    none(),
                ),
            ),
            (
                "ask".into(),
                call("ask_user", json!({"question": question}), question, none()),
            ),
        ],
        Turn::Approve(step) => vec![(
            "ask for approval".into(),
            call("request_permission", json!({"step": step}), step, none()),
        )],
        Turn::Plan => vec![
            (
                "list files".into(),
                call("glob", json!({"pattern": "**/*"}), "**/*", none()),
            ),
            (
                "read README.md".into(),
                call(
                    "read_file",
                    json!({"path": "README.md"}),
                    "README.md",
                    none(),
                ),
            ),
            (
                "plan".into(),
                call("update_plan", json!({}), "Plan the team's tasks", none()),
            ),
        ],
        Turn::Edit { files, message } => {
            let mut steps = Vec::new();
            for (path, _) in files {
                steps.push((
                    format!("read {path}"),
                    call("read_file", json!({"path": path}), path, none()),
                ));
            }
            for (path, _) in files {
                steps.push((
                    format!("write {path}"),
                    call("write_file", json!({"path": path}), path, none()),
                ));
            }
            let mut check = Map::new();
            check.insert(atif::activity::CHECK.into(), Value::Bool(true));
            steps.push((
                "check".into(),
                call(
                    "shell",
                    json!({"command": "git diff --check"}),
                    "git diff --check",
                    check,
                ),
            ));
            steps.push((
                "commit".into(),
                call(
                    "shell",
                    json!({"command": format!("git commit -m '{message}'")}),
                    "git commit",
                    none(),
                ),
            ));
            steps
        }
    }
}

fn activity(activity: atif::Activity) -> wire::Activity {
    match activity {
        atif::Activity::Reading => wire::Activity::Reading,
        atif::Activity::Editing => wire::Activity::Editing,
        atif::Activity::Running => wire::Activity::Running,
        atif::Activity::Testing => wire::Activity::Testing,
        atif::Activity::Judging => wire::Activity::Judging,
        atif::Activity::Thinking => wire::Activity::Thinking,
        atif::Activity::Waiting => wire::Activity::Waiting,
        atif::Activity::Blocked => wire::Activity::Blocked,
        atif::Activity::Done => wire::Activity::Done,
        atif::Activity::Failed => wire::Activity::Failed,
    }
}

fn station(station: atif::Station) -> wire::Station {
    match station {
        atif::Station::Library => wire::Station::Library,
        atif::Station::Desk => wire::Station::Desk,
        atif::Station::Workbench => wire::Station::Workbench,
        atif::Station::ProvingGround => wire::Station::ProvingGround,
        atif::Station::Oracle => wire::Station::Oracle,
        atif::Station::Podium => wire::Station::Podium,
        atif::Station::Lounge => wire::Station::Lounge,
        atif::Station::TaskWall => wire::Station::TaskWall,
    }
}

/// `base` with `task`'s seat partway through its turn: running, at the
/// station its newest step classifies to, with a log line per step.
fn working(base: &View, task: &str, steps: &[(String, atif::Step)], at: u64) -> View {
    let mut view = base.clone();
    let Some(seat) = view.tasks.iter_mut().find(|t| t.task == task).map(|t| {
        t.status = wire::TaskStatus::Running;
        t.seat.clone()
    }) else {
        return view;
    };
    let lines: Vec<wire::LogLine> = steps
        .iter()
        .enumerate()
        .map(|(i, (_, step))| {
            let classified = atif::classify(step);
            let call = step.call.as_ref();
            let name = call.map_or("a tool", |c| c.name.as_str());
            let purpose = call.and_then(|c| c.purpose.as_deref()).unwrap_or_default();
            let text = if purpose.is_empty() {
                format!("{}: {name}", classified.activity.word())
            } else {
                format!("{}: {name} — {purpose}", classified.activity.word())
            };
            wire::LogLine {
                at: at - (steps.len() - 1 - i) as u64,
                activity: activity(classified.activity),
                text: wire::first_line(&text, wire::MAX_LINE),
            }
        })
        .collect();
    let newest = steps
        .last()
        .map_or(atif::Activity::Thinking.into(), |(_, step)| {
            atif::classify(step)
        });
    if let Some(found) = view.seats.iter_mut().find(|s| s.seat == seat) {
        found.activity = activity(newest.activity);
        found.station = station(newest.station);
        found.task = Some(task.to_owned());
    }
    let start = lines.len().saturating_sub(wire::MAX_LOG_LINES);
    let lines = lines[start..].to_vec();
    match view.logs.iter_mut().find(|log| log.seat == seat) {
        Some(log) => {
            log.task = Some(task.to_owned());
            log.lines = lines;
        }
        None => view.logs.push(wire::Log {
            seat,
            task: Some(task.to_owned()),
            lines,
        }),
    }
    view.canonicalize();
    view
}

/// Plays a [`Recording`] as a studio [`Source`], through a host-side
/// stream and a client mirror.
pub struct Player {
    recording: Recording,
    index: usize,
    /// Seconds each frame shows for, or `None` to hold the frame.
    period: Option<f32>,
    clock: f32,
    host: Stream,
    mirror: Mirror,
}

impl Player {
    /// Plays `recording` from frame `start`, moving to the next frame every
    /// `period` seconds, or holding `start` when `period` is `None`.
    #[must_use]
    pub fn new(recording: Recording, start: usize, period: Option<f32>) -> Self {
        let index = start.min(recording.frames.len().saturating_sub(1));
        Self {
            recording,
            index,
            period: period.filter(|p| p.is_finite() && *p > 0.0),
            clock: 0.0,
            host: Stream::new(STREAM),
            mirror: Mirror::default(),
        }
    }

    /// The frame showing.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Shows frame `index` from the next poll on.
    pub fn seek(&mut self, index: usize) {
        self.index = index.min(self.recording.frames.len().saturating_sub(1));
        self.clock = 0.0;
    }

    /// One NIP-HOST studio exchange: the host answers what the mirror asks
    /// for. Returns the mirror's studio when it changed.
    fn exchange(&mut self) -> Option<Snapshot> {
        let view = self.recording.frames.get(self.index)?.view.clone();
        let outcome = match self.mirror.next() {
            Operation::StudioSnapshot {} => Ok(Outcome::Studio {
                snapshot: Box::new(self.host.snapshot(view)),
            }),
            Operation::StudioUpdate { stream, since } => self
                .host
                .update(view, &stream, since)
                .map(|update| Outcome::StudioUpdate {
                    update: Box::new(update),
                })
                .map_err(|code| Error::new(code, "the studio update was refused")),
            _ => return None,
        };
        match outcome.and_then(|outcome| self.mirror.accept(&outcome)) {
            Ok(true) => self.mirror.snapshot().cloned(),
            Ok(false) => None,
            Err(error) => {
                self.mirror.refused(&error);
                None
            }
        }
    }
}

impl Source for Player {
    fn start(&mut self) {
        self.mirror = Mirror::default();
        self.clock = 0.0;
    }

    fn stop(&mut self) {
        self.mirror = Mirror::default();
    }

    fn poll(&mut self, dt: f32) -> Option<Snapshot> {
        if let Some(period) = self.period
            && dt.is_finite()
            && dt > 0.0
        {
            self.clock += dt;
            while self.clock >= period && self.index + 1 < self.recording.frames.len() {
                self.clock -= period;
                self.index += 1;
            }
        }
        self.exchange()
    }

    fn review(&mut self, task: &str) -> Option<TaskReview> {
        self.recording.review(task).cloned()
    }
}

/// The simulated team recorded off the frame: the first
/// [`Source::start`] runs [`Recording::run`] on its own thread in a scratch
/// directory under the system's temporary directory, and plays the
/// recording through a [`Player`] once it is ready. Nothing runs until the
/// player first enters Everglade.
pub struct Background {
    period: f32,
    pending: Option<std::sync::mpsc::Receiver<Result<Recording, String>>>,
    player: Option<Player>,
    error: Option<String>,
}

impl Background {
    /// Plays the recording a frame every `period` seconds, looping to the
    /// start after the last frame.
    #[must_use]
    pub fn new(period: f32) -> Self {
        Self {
            period,
            pending: None,
            player: None,
            error: None,
        }
    }

    /// Why the recording failed, if it did.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Source for Background {
    fn start(&mut self) {
        if let Some(player) = &mut self.player {
            player.start();
            return;
        }
        if self.pending.is_some() || self.error.is_some() {
            return;
        }
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("verse-studio-sim".into())
            .spawn(move || {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                let dir = std::env::temp_dir()
                    .join(format!("verse-studio-sim-{}-{nanos}", std::process::id()));
                let result = Recording::run(&dir);
                let _ = std::fs::remove_dir_all(&dir);
                let _ = send.send(result);
            });
        match spawned {
            Ok(_) => self.pending = Some(receive),
            Err(error) => self.error = Some(format!("the simulated team did not start: {error}")),
        }
    }

    fn stop(&mut self) {
        if let Some(player) = &mut self.player {
            player.stop();
        }
    }

    fn poll(&mut self, dt: f32) -> Option<Snapshot> {
        if let Some(receive) = &self.pending
            && let Ok(result) = receive.try_recv()
        {
            self.pending = None;
            match result {
                Ok(recording) => {
                    let mut player = Player::new(recording, 0, Some(self.period));
                    player.start();
                    self.player = Some(player);
                }
                Err(error) => self.error = Some(error),
            }
        }
        let player = self.player.as_mut()?;
        if player.index() + 1 >= player.recording.frames.len()
            && player.clock + dt.max(0.0) >= self.period
        {
            player.seek(0);
        }
        player.poll(dt)
    }

    fn review(&mut self, task: &str) -> Option<TaskReview> {
        self.player.as_mut().and_then(|player| player.review(task))
    }
}
