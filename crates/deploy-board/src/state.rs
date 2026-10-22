//! The billboard's state machine: deploy statuses folded into one lane per
//! environment, each with its current run, its last result, and the runs
//! before it.
//!
//! Records may arrive in any order and more than once (a phase event and
//! its lane head carry the same phase), so a run keeps the set of phases it
//! has seen and every view is derived from that set.

use std::collections::{BTreeMap, BTreeSet};

use nostr::deploy::{Env, Phase, Record, Tier};

/// The progress strip's steps.
pub const STEPS: [&str; 5] = ["BUILD", "DEPLOY", "SMOKE", "SHIFT", "LIVE"];
/// The most runs a lane remembers.
pub const MAX_RUNS: usize = 24;
/// The most history rows a lane shows.
pub const HISTORY: usize = 3;
/// How long a flash lasts, s.
pub const FLASH_SECS: f64 = 6.0;
/// A run whose last phase is older than this is shown as stalled, s.
pub const STALL_SECS: u64 = 3 * 3600;
/// An ending this recent still flashes when it arrives, s: the relay's
/// stored events from long ago never do.
pub const FRESH_SECS: u64 = 120;
/// A livehost pointer within this long of an announced content deploy in
/// the same lane is the same deploy, s.
pub const SAME_DEPLOY_SECS: u64 = 600;

/// One step on the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Idle,
    Active,
    Done,
    Failed,
    /// Every step of a run that went live.
    Live,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Live,
    Failed,
    RolledBack,
    /// Another run started before this one ended.
    Stopped,
}

impl Outcome {
    /// The word the board shows for it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Live => "LIVE",
            Outcome::Failed => "FAILED",
            Outcome::RolledBack => "ROLLBACK",
            Outcome::Stopped => "STOPPED",
        }
    }
}

/// What a flash shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flash {
    Live,
    Failed,
}

#[derive(Debug, Clone)]
struct Run {
    tier: Tier,
    commit: Option<String>,
    subject: Option<String>,
    by: String,
    build: Option<String>,
    started: u64,
    /// Every phase seen, by when it happened.
    phases: BTreeSet<(u64, Phase)>,
}

impl Run {
    fn last(&self) -> (u64, Phase) {
        *self.phases.last().expect("a run has at least one phase")
    }

    fn ended(&self) -> Option<Outcome> {
        let (_, phase) = self.last();
        match phase {
            Phase::Live => Some(Outcome::Live),
            Phase::RolledBack => Some(Outcome::RolledBack),
            p if p.failed() => Some(Outcome::Failed),
            _ => None,
        }
    }

    fn steps(&self) -> [Step; 5] {
        let mut steps = [Step::Idle; 5];
        let done_through = |steps: &mut [Step; 5], last: usize| {
            for step in &mut steps[..=last] {
                if *step != Step::Failed {
                    *step = Step::Done;
                }
            }
        };
        let activate = |steps: &mut [Step; 5], at: usize| {
            if at > 0 {
                done_through(steps, at - 1);
            }
            if matches!(steps[at], Step::Idle | Step::Active) {
                steps[at] = Step::Active;
            }
        };
        let fail = |steps: &mut [Step; 5], at: usize| {
            if at > 0 {
                done_through(steps, at - 1);
            }
            steps[at] = Step::Failed;
        };
        for &(_, phase) in &self.phases {
            // Smoke tests after the traffic moved check the live service.
            let shifted = steps[3] == Step::Done;
            match phase {
                Phase::BuildStarted => activate(&mut steps, 0),
                Phase::BuildDone => done_through(&mut steps, 0),
                Phase::BuildFailed => fail(&mut steps, 0),
                Phase::Deploying => activate(&mut steps, 1),
                Phase::Promoted => done_through(&mut steps, 1),
                Phase::DeployFailed => fail(&mut steps, 1),
                Phase::SmokeRunning if shifted => activate(&mut steps, 4),
                Phase::SmokeRunning => activate(&mut steps, 2),
                Phase::SmokePassed if shifted => {}
                Phase::SmokePassed => done_through(&mut steps, 2),
                Phase::SmokeFailed if shifted => fail(&mut steps, 4),
                Phase::SmokeFailed => fail(&mut steps, 2),
                Phase::TrafficShifted => done_through(&mut steps, 3),
                Phase::RolledBack => {
                    if let Some(at) = steps.iter().rposition(|s| *s != Step::Idle) {
                        steps[at] = Step::Failed;
                    }
                }
                Phase::Live => steps = [Step::Live; 5],
            }
        }
        steps
    }
}

/// The word the board shows for a run's newest phase.
#[must_use]
pub fn word(phase: Phase) -> &'static str {
    match phase {
        Phase::BuildStarted => "BUILDING",
        Phase::BuildDone => "BUILT",
        Phase::BuildFailed => "BUILD FAILED",
        Phase::Deploying => "DEPLOYING",
        Phase::Promoted => "CANDIDATE READY",
        Phase::DeployFailed => "DEPLOY FAILED",
        Phase::SmokeRunning => "SMOKE TEST",
        Phase::SmokePassed => "SMOKE PASSED",
        Phase::SmokeFailed => "SMOKE FAILED",
        Phase::TrafficShifted => "TRAFFIC MOVED",
        Phase::RolledBack => "ROLLED BACK",
        Phase::Live => "LIVE",
    }
}

/// A lane's current run, as the board draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    pub tier: Tier,
    pub commit: Option<String>,
    pub subject: Option<String>,
    pub by: String,
    pub build: Option<String>,
    pub phase: Phase,
    /// The status line: [`word`] of the phase, or `STALLED`.
    pub word: &'static str,
    pub steps: [Step; 5],
    /// Seconds since the run started, or how long it took once it ended.
    pub elapsed: u64,
    pub outcome: Option<Outcome>,
    /// Whether the run is still going.
    pub running: bool,
}

/// One row of a lane's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Past {
    pub commit: Option<String>,
    pub tier: Tier,
    pub outcome: Outcome,
    /// Seconds since it ended.
    pub age: u64,
}

/// One lane as the board draws it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lane {
    pub current: Option<Current>,
    /// The last run that ended, and how long ago.
    pub last: Option<(Outcome, u64)>,
    /// The runs before the current one, newest first.
    pub history: Vec<Past>,
    /// A flash that is showing, and how far through it is (0 to 1).
    pub flash: Option<(Flash, f64)>,
    /// How far the history has moved since its newest row arrived (0 to 1;
    /// 1 when settled).
    pub scroll: f64,
}

#[derive(Debug, Default)]
struct LaneState {
    runs: BTreeMap<String, Run>,
    flash: Option<(Flash, f64)>,
    /// When the history last gained a row, wall seconds.
    shifted: f64,
    /// The run shown as current, to tell when it changes.
    current: Option<String>,
}

/// The board's state: one lane for staging and one for production.
#[derive(Debug)]
pub struct Board {
    lanes: [LaneState; 2],
    test: bool,
}

/// A livehost deploy pointer the board shows as a content deploy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    pub env: Env,
    /// The manifest's SHA-256, hex.
    pub manifest: String,
    /// The pointer's `created_at`.
    pub at: u64,
}

fn index(env: Env) -> usize {
    match env {
        Env::Staging => 0,
        Env::Production => 1,
    }
}

impl Board {
    /// An empty board. With `test`, it shows only test events (a check's
    /// own); without, it never shows them.
    #[must_use]
    pub fn new(test: bool) -> Self {
        Self {
            lanes: Default::default(),
            test,
        }
    }

    /// Folds in one checked record at wall time `wall` (Unix seconds, with
    /// a fraction). Returns whether the board changed.
    pub fn apply(&mut self, record: &Record, wall: f64) -> bool {
        let status = &record.status;
        if status.test != self.test {
            return false;
        }
        let lane = &mut self.lanes[index(status.env)];
        let run = lane.runs.entry(status.run.clone()).or_insert_with(|| Run {
            tier: status.tier,
            commit: status.commit.clone(),
            subject: status.subject.clone(),
            by: status.by.clone(),
            build: status.build.clone(),
            started: status.started,
            phases: BTreeSet::new(),
        });
        if !run.phases.insert((record.at, status.phase)) {
            return false;
        }
        // Later phases fill in what earlier ones lacked (a build ID known
        // only once the build ran).
        run.commit = run.commit.take().or_else(|| status.commit.clone());
        run.subject = run.subject.take().or_else(|| status.subject.clone());
        run.build = status.build.clone().or_else(|| run.build.take());
        run.started = run.started.min(status.started);
        let fresh = record.at + FRESH_SECS >= wall as u64;
        let newest = run.last() == (record.at, status.phase);
        if fresh && newest {
            if status.phase == Phase::Live {
                lane.flash = Some((Flash::Live, wall));
            } else if status.phase.failed() || status.phase == Phase::RolledBack {
                lane.flash = Some((Flash::Failed, wall));
            }
        }
        lane.trim();
        lane.note_current(wall);
        true
    }

    /// Folds in a livehost pointer as a content deploy that went live,
    /// unless an announced content deploy in the same lane already covers
    /// it. Returns whether the board changed.
    pub fn apply_pointer(&mut self, pointer: &Pointer, wall: f64) -> bool {
        if self.test {
            return false;
        }
        let lane = &mut self.lanes[index(pointer.env)];
        let covered = lane.runs.values().any(|run| {
            run.tier == Tier::Content
                && run
                    .phases
                    .iter()
                    .any(|(at, _)| at.abs_diff(pointer.at) <= SAME_DEPLOY_SECS)
        });
        let short: String = pointer.manifest.chars().take(10).collect();
        let key = format!("lh-{short}");
        if covered || lane.runs.contains_key(&key) {
            return false;
        }
        let mut phases = BTreeSet::new();
        phases.insert((pointer.at, Phase::Live));
        lane.runs.insert(
            key,
            Run {
                tier: Tier::Content,
                commit: None,
                subject: Some(format!("CONTENT {short}")),
                by: "livehost".into(),
                build: None,
                started: pointer.at,
                phases,
            },
        );
        if pointer.at + FRESH_SECS >= wall as u64 {
            lane.flash = Some((Flash::Live, wall));
        }
        lane.trim();
        lane.note_current(wall);
        true
    }

    /// The lane for `env` at wall time `wall`.
    #[must_use]
    pub fn lane(&self, env: Env, wall: f64) -> Lane {
        self.lanes[index(env)].view(wall)
    }
}

impl LaneState {
    fn trim(&mut self) {
        while self.runs.len() > MAX_RUNS {
            let oldest = self
                .runs
                .iter()
                .min_by_key(|(_, run)| run.last().0)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => self.runs.remove(&key),
                None => break,
            };
        }
    }

    fn ordered(&self) -> Vec<(&String, &Run)> {
        let mut runs: Vec<_> = self.runs.iter().collect();
        runs.sort_by(|a, b| b.1.last().cmp(&a.1.last()).then(a.0.cmp(b.0)));
        runs
    }

    fn note_current(&mut self, wall: f64) {
        let current = self.ordered().first().map(|(key, _)| (*key).clone());
        if current != self.current {
            if self.current.is_some() {
                self.shifted = wall;
            }
            self.current = current;
        }
    }

    fn view(&self, wall: f64) -> Lane {
        let now = wall.max(0.0) as u64;
        let ordered = self.ordered();
        let current = ordered.first().map(|(_, run)| {
            let (last_at, phase) = run.last();
            let outcome = run.ended();
            let stalled = outcome.is_none() && now.saturating_sub(last_at) > STALL_SECS;
            Current {
                tier: run.tier,
                commit: run.commit.clone(),
                subject: run.subject.clone(),
                by: run.by.clone(),
                build: run.build.clone(),
                phase,
                word: if stalled { "STALLED" } else { word(phase) },
                steps: run.steps(),
                elapsed: if outcome.is_some() || stalled {
                    last_at.saturating_sub(run.started)
                } else {
                    now.saturating_sub(run.started)
                },
                outcome,
                running: outcome.is_none() && !stalled,
            }
        });
        let history: Vec<Past> = ordered
            .iter()
            .skip(1)
            .take(HISTORY)
            .map(|(_, run)| Past {
                commit: run.commit.clone(),
                tier: run.tier,
                outcome: run.ended().unwrap_or(Outcome::Stopped),
                age: now.saturating_sub(run.last().0),
            })
            .collect();
        let last = ordered
            .iter()
            .find_map(|(_, run)| run.ended().map(|o| (o, now.saturating_sub(run.last().0))));
        let flash = self.flash.and_then(|(flash, since)| {
            let t = (wall - since) / FLASH_SECS;
            (0.0..1.0).contains(&t).then_some((flash, t))
        });
        Lane {
            current,
            last,
            history,
            flash,
            scroll: ((wall - self.shifted) / 0.8).clamp(0.0, 1.0),
        }
    }
}

/// A duration as the board shows it: `42S`, `7:05`, or `2H05`.
#[must_use]
pub fn clock(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}S"),
        60..3600 => format!("{}:{:02}", secs / 60, secs % 60),
        _ => format!("{}H{:02}", (secs / 3600).min(99), secs % 3600 / 60),
    }
}

/// An age as the board shows it: `12S`, `5M`, `3H`, or `2D`.
#[must_use]
pub fn age(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}S"),
        60..3600 => format!("{}M", secs / 60),
        3600..86_400 => format!("{}H", secs / 3600),
        _ => format!("{}D", (secs / 86_400).min(99)),
    }
}
