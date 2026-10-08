//! The pylon league on the Gym's EVALS board.
//!
//! While the player stands in the Gym, a background thread reads the
//! league with [`pylon::league::fetch`], the same call behind
//! `openagents pylon league`: per hardware class (family and tier), each
//! pylon's pass rate on its class's pinned suite, accepted jobs, median job
//! time, and cost per accepted job, all recomputed from verified beacons,
//! receipts, and the trusted checkers' verdicts. This module adds no
//! number of its own; it only words what the league holds. Leaving the Gym
//! stops the thread, and a Grid without a relay never starts one.

use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use pylon::identity::Identity;
use pylon::league::League;
use serde::Serialize;

/// How often the league is read again while the player stays.
const REFRESH: Duration = Duration::from_secs(30);

/// What the section says about itself.
pub const NOTE: &str = "Pylons ranked per hardware class on that class's pinned Gym suite, \
from signed beacons, receipts, and the trusted checkers' verdicts on the relay. A sigil \
marks passing checks and no failure.";

/// One pylon's line, worded from a [`pylon::league::Row`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Line {
    pub label: String,
    pub model: String,
    /// `100%`, or `-` before any decisive verdict on the suite.
    pub pass: String,
    /// `3 pass · 0 fail · 0 inconclusive` on the pinned suite.
    pub checks: String,
    pub jobs: u64,
    /// `4s`, or `-` without an accepted job.
    pub median: String,
    /// `120 msat per job`, or `free`.
    pub cost: String,
    /// `passing`, `failing`, or `unchecked`.
    pub standing: &'static str,
    pub sigil: bool,
}

/// One class's board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Board {
    /// `GPU · medium`.
    pub class: String,
    /// The pinned suite's ID and the digest's first 12 characters.
    pub suite: String,
    pub lines: Vec<Line>,
}

/// The section's screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub schema: &'static str,
    pub revision: u64,
    /// `offline`, `reading`, `ready`, or `unreachable`.
    pub state: &'static str,
    /// The relay's host.
    pub relay: String,
    /// How many checkers' verdicts count.
    pub checkers: usize,
    pub boards: Vec<Board>,
    /// What the section says instead of boards, when it has none.
    pub empty: Option<String>,
    pub note: &'static str,
}

/// Words the league for the board.
#[must_use]
pub fn boards(league: &League) -> Vec<Board> {
    league
        .classes
        .iter()
        .map(|class| Board {
            class: format!("{} · {}", family(class.family), tier(class.tier)),
            suite: format!(
                "suite {} · {}",
                class.suite_id,
                class.suite.get(..12).unwrap_or(&class.suite)
            ),
            lines: class
                .rows
                .iter()
                .map(|row| Line {
                    label: row.label.clone(),
                    model: row.model.clone(),
                    pass: row
                        .pass_rate
                        .map_or_else(|| "-".into(), |r| format!("{:.0}%", r * 100.0)),
                    checks: format!(
                        "{} pass · {} fail · {} inconclusive",
                        row.suite.pass, row.suite.fail, row.suite.inconclusive
                    ),
                    jobs: row.jobs,
                    median: row
                        .median_secs
                        .map_or_else(|| "-".into(), |s| format!("{s}s")),
                    cost: row
                        .msat_per_job
                        .map_or_else(|| "free".into(), |m| format!("{m} msat per job")),
                    standing: match row.standing {
                        nostr::pylon::Standing::Passing => "passing",
                        nostr::pylon::Standing::Failing => "failing",
                        nostr::pylon::Standing::Unchecked => "unchecked",
                    },
                    sigil: row.sigil,
                })
                .collect(),
        })
        .collect()
}

fn family(family: nostr::pylon::Family) -> &'static str {
    match family {
        nostr::pylon::Family::UnifiedMemory => "Unified memory",
        nostr::pylon::Family::Gpu => "GPU",
        nostr::pylon::Family::Cpu => "CPU",
    }
}

fn tier(tier: nostr::pylon::Tier) -> &'static str {
    match tier {
        nostr::pylon::Tier::Small => "small",
        nostr::pylon::Tier::Medium => "medium",
        nostr::pylon::Tier::Large => "large",
        nostr::pylon::Tier::Xl => "XL",
    }
}

struct Worker {
    rx: Receiver<Result<League, String>>,
    _stop: Stop,
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// The league's state on the game thread.
pub struct Reader {
    /// `None`: this Grid has no relay, and nothing is read.
    relay: Option<String>,
    checkers: BTreeSet<String>,
    worker: Option<Worker>,
    league: Option<League>,
    error: Option<String>,
    revision: u64,
}

impl Reader {
    /// A reader of `relay` that counts `checkers`' verdicts. It reads
    /// nothing until [`Self::set_active`].
    #[must_use]
    pub fn new(relay: Option<String>, checkers: BTreeSet<String>) -> Self {
        Self {
            relay,
            checkers,
            worker: None,
            league: None,
            error: None,
            revision: 1,
        }
    }

    /// The checkers this computer trusts, as `openagents pylon league`
    /// reads them: its own checker key and `OPENAGENTS_PYLON_CHECKERS`.
    #[must_use]
    pub fn trusted() -> BTreeSet<String> {
        pylon::check::trusted(&pylon::home())
    }

    /// Starts reading when the player enters the Gym and stops when they
    /// leave. The last league stays for the next visit.
    pub fn set_active(&mut self, active: bool) {
        match (active, self.worker.is_some(), &self.relay) {
            (true, false, Some(relay)) => {
                self.worker = Some(start(relay.clone(), self.checkers.clone()));
                self.revision += 1;
            }
            (false, true, _) => {
                self.worker = None;
                self.revision += 1;
            }
            _ => {}
        }
    }

    /// The relay read, when this Grid has one.
    #[must_use]
    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    /// Whether the reader runs.
    #[must_use]
    pub fn active(&self) -> bool {
        self.worker.is_some()
    }

    /// Takes whatever the reader thread sent since the last call.
    pub fn poll(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        let mut changed = false;
        for update in worker.rx.try_iter() {
            match update {
                Ok(league) => {
                    let same = self.league.as_ref().is_some_and(|l| {
                        l.classes == league.classes && l.checkers == league.checkers
                    });
                    changed |= !same || self.error.is_some();
                    self.league = Some(league);
                    self.error = None;
                }
                Err(error) => {
                    changed |= self.error.as_ref() != Some(&error);
                    self.error = Some(error);
                }
            }
        }
        if changed {
            self.revision += 1;
        }
    }

    /// Blocks until a league arrives or `timeout` passes. For tests and
    /// captures, never for a frame.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            self.poll();
            if self.league.is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// The last league read, once one arrived.
    #[must_use]
    pub fn league(&self) -> Option<&League> {
        self.league.as_ref()
    }

    /// Changes whenever [`Self::view`] would.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The section's screen.
    #[must_use]
    pub fn view(&self) -> View {
        let state = match (&self.relay, &self.worker, &self.league, &self.error) {
            (None, ..) => "offline",
            (Some(_), _, _, Some(_)) => "unreachable",
            (Some(_), _, Some(_), None) => "ready",
            (Some(_), Some(_), None, None) => "reading",
            (Some(_), None, None, None) => "offline",
        };
        let boards = self.league.as_ref().map(boards).unwrap_or_default();
        let empty = if !boards.is_empty() {
            None
        } else {
            Some(match state {
                "offline" if self.relay.is_none() => {
                    "This Grid is offline, so no pylon league is read. Join a relay to see it."
                        .to_owned()
                }
                "offline" => "The pylon league is read while you stand in the Gym.".to_owned(),
                "reading" => "Reading the pylon league from the relay…".to_owned(),
                "unreachable" => format!(
                    "The relay could not be read: {}",
                    self.error.as_deref().unwrap_or_default()
                ),
                _ => "No pylons are on this relay yet. Start one with `openagents pylon serve`."
                    .to_owned(),
            })
        };
        View {
            schema: "openagents.verse.gym-league.v1",
            revision: self.revision,
            state,
            relay: self
                .relay
                .as_deref()
                .map(host)
                .unwrap_or_default()
                .to_owned(),
            checkers: self.checkers.len(),
            boards,
            empty,
            note: NOTE,
        }
    }
}

fn host(relay: &str) -> &str {
    let rest = relay.split_once("://").map_or(relay, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}

fn start(relay: String, checkers: BTreeSet<String>) -> Worker {
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    std::thread::Builder::new()
        .name("verse-gym-league".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = tx.send(Err("could not start the league reader".into()));
                return;
            };
            // A fresh key per visit: reading the league names no one.
            let reader = Identity::generate();
            while !worker_stop.load(Ordering::Acquire) {
                let read = runtime.block_on(pylon::league::fetch(&reader, &relay, &checkers));
                if worker_stop.load(Ordering::Acquire) || tx.send(read).is_err() {
                    return;
                }
                let until = Instant::now() + REFRESH;
                while Instant::now() < until {
                    if worker_stop.load(Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .expect("the Gym league thread starts");
    Worker {
        rx,
        _stop: Stop(stop),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_grid_without_a_relay_reads_nothing_and_says_so() {
        let mut reader = Reader::new(None, BTreeSet::new());
        reader.set_active(true);
        assert!(!reader.active());
        let view = reader.view();
        assert_eq!(view.state, "offline");
        assert!(view.boards.is_empty());
        assert!(view.empty.unwrap().contains("offline"));
    }

    #[test]
    fn an_unreachable_relay_is_named_not_shown_as_an_empty_league() {
        let mut reader = Reader::new(Some("ws://127.0.0.1:1".into()), BTreeSet::new());
        reader.set_active(true);
        let start = Instant::now();
        while reader.view().state != "unreachable" && start.elapsed() < Duration::from_secs(10) {
            reader.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        let view = reader.view();
        assert_eq!(view.state, "unreachable");
        assert!(
            view.empty
                .unwrap()
                .starts_with("The relay could not be read")
        );
        reader.set_active(false);
        assert!(!reader.active());
    }

    #[test]
    fn the_board_words_the_same_league_the_cli_prints() {
        let home = tempfile::tempdir().unwrap();
        let fixture = pylon::fixture::League::start(home.path()).unwrap();
        let mut reader = Reader::new(Some(fixture.relay.clone()), fixture.checkers.clone());
        reader.set_active(true);
        assert!(reader.wait(Duration::from_secs(20)));
        let view = reader.view();
        assert_eq!(view.state, "ready");
        assert_eq!(view.checkers, 1);
        assert!(view.empty.is_none());
        let classes: Vec<&str> = view.boards.iter().map(|b| b.class.as_str()).collect();
        assert_eq!(
            classes,
            [
                "Unified memory · large",
                "GPU · medium",
                "GPU · large",
                "CPU · small"
            ]
        );
        // Every number is the league's own, row for row.
        let league = reader.league().unwrap();
        assert_eq!(boards(league), view.boards);
        let medium = &view.boards[1];
        assert_eq!(
            (medium.lines[0].pass.as_str(), medium.lines[0].sigil),
            ("100%", true)
        );
        assert_eq!(medium.lines[0].checks, "3 pass · 0 fail · 0 inconclusive");
        assert_eq!(
            (medium.lines[1].pass.as_str(), medium.lines[1].standing),
            ("0%", "failing")
        );
        let cpu = &view.boards[3].lines[0];
        assert_eq!(
            (cpu.pass.as_str(), cpu.jobs, cpu.standing, cpu.cost.as_str()),
            ("-", 2, "unchecked", "free")
        );
        assert!(pylon::league::render(league).contains("100%"));
    }
}
