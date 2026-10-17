//! Playback: the run's events applied one at a time on a clock, the short
//! animations each one starts, and what the page says about them.
//!
//! The replay checks the whole run when it loads, so applying an event here
//! never fails. When the run ends the stage waits a few seconds and starts
//! again: the demo with the next seed, the real run from its start.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use crate::demo;
use crate::layertree::model::{Change, Event, Run, Snapshot};
use crate::layout::Shape;
use crate::words::{self, Source};

/// The real signet run: a tree opened, members joined, moved a key, left,
/// the signer refreshed the output and passed its checks, then it was
/// switched off and every member was paid by the timed unroll.
const SIGNET: &str = include_str!("../../layertree-tee-lab/fixtures/events/signet-kill-paid.jsonl");

/// How long the stage rests on the last event before starting again.
const REST: f32 = 8.0;
/// The ticker's length.
const TICKER: usize = 7;
/// The most animations kept at once; the oldest go first.
const MAX_EFFECTS: usize = 360;

/// What an animation shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    /// A new member drops into its slot; its value climbs to the root.
    Join { pos: u64 },
    /// A member's share moves to a new key.
    Update { pos: u64 },
    /// Value moves from one member to another.
    Claim { from: u64, to: u64 },
    /// A member's share splits into a new slot.
    Split { from: u64, to: u64 },
    /// A member leaves; the value flies out to Bitcoin.
    Exit { pos: u64, amount: u64 },
    /// The signer renews the shared output: a wave up the tree from its root.
    Refresh,
    /// The signer signs a transition: a pulse to the shared output.
    Sign,
    /// The signer passes its check.
    Check { ok: bool },
    /// A light client checks a block.
    Follow { member: u64, ok: bool },
    /// A block arrives.
    Block { hit: bool },
    /// The signer is switched off.
    Kill,
    /// An unroll transaction is broadcast.
    Broadcast { level: u32 },
    /// A member is paid back by the unroll.
    Payout { pos: u64, amount: u64 },
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub fx: Fx,
    pub start: f32,
    pub length: f32,
}

impl Effect {
    /// How far along the animation is at `now`, from 0 to 1.
    #[must_use]
    pub fn progress(&self, now: f32) -> f32 {
        ((now - self.start) / self.length).clamp(0.0, 1.0)
    }
}

/// The run being played and everything drawn from it.
pub struct Stage {
    pub source: Source,
    pub seed: u64,
    events: Vec<Event>,
    pub state: Snapshot,
    next: usize,
    wait: f32,
    pub time: f32,
    pub shape: Shape,
    pub effects: Vec<Effect>,
    /// The caption and when it was set.
    pub caption: (String, f32),
    /// The earliest time a passing event may replace the caption.
    hold_until: f32,
    /// The caption is the unroll's payout counter, which updates in place.
    counting: bool,
    /// Recent transactions, newest last, with when they happened.
    pub ticker: VecDeque<(String, f32)>,
    /// `sums[height][index]`: each node's balance.
    pub sums: Vec<Vec<u64>>,
    dirty: bool,
    /// When each member slot last changed.
    pub changed: HashMap<u64, f32>,
    /// Member slots paid back by the unroll.
    pub paid: HashSet<u64>,
    owed: HashMap<(String, u64), Vec<u64>>,
    /// Members who run a light client.
    pub followers: Vec<u64>,
    /// Each light client's last result and when it came.
    pub follow: HashMap<u64, (bool, f32)>,
    /// When the signer was switched off.
    pub killed_at: Option<f32>,
    /// When the last event played.
    pub done_at: Option<f32>,
    /// Transactions so far, not counting refreshes.
    pub transactions: u64,
    /// Members when the signer went off, and the balance owed to them.
    pub owed_members: usize,
    /// Unroll transactions seen per level.
    pub broadcasts: [usize; 3],
}

fn op(event: &Event) -> &str {
    event.data.get("op").and_then(Value::as_str).unwrap_or("")
}

impl Stage {
    /// The simulated run for `seed`.
    ///
    /// # Errors
    /// The generated run does not replay (a generator bug).
    pub fn demo(seed: u64) -> Result<Self, String> {
        let config = demo::Config {
            seed,
            ..demo::Config::default()
        };
        let run = Run::parse(&demo::jsonl(&demo::generate(&config)), "demo".into())?;
        Ok(Self::new(Source::Demo, seed, run.events))
    }

    /// The real signet run.
    ///
    /// # Errors
    /// The fixture does not replay.
    pub fn signet() -> Result<Self, String> {
        let run = Run::parse(SIGNET, "signet-kill-paid.jsonl".into())?;
        Ok(Self::new(Source::Signet, 0, run.events))
    }

    /// The run named in a page's query string: `run=signet` for the real
    /// run, `seed=N` for another simulated one.
    ///
    /// # Errors
    /// As [`Self::demo`] and [`Self::signet`].
    pub fn from_query(query: &str) -> Result<Self, String> {
        let mut seed = 1;
        for part in query.trim_start_matches('?').split('&') {
            match part.split_once('=') {
                Some(("run", "signet")) => return Self::signet(),
                Some(("seed", n)) => seed = n.parse().unwrap_or(1),
                _ => {}
            }
        }
        Self::demo(seed)
    }

    fn new(source: Source, seed: u64, events: Vec<Event>) -> Self {
        Self {
            source,
            seed,
            events,
            state: Snapshot::default(),
            next: 0,
            wait: 0.0,
            time: 0.0,
            shape: Shape::new(4),
            effects: Vec::new(),
            caption: (String::new(), 0.0),
            hold_until: 0.0,
            counting: false,
            ticker: VecDeque::new(),
            sums: Vec::new(),
            dirty: true,
            changed: HashMap::new(),
            paid: HashSet::new(),
            owed: HashMap::new(),
            followers: Vec::new(),
            follow: HashMap::new(),
            killed_at: None,
            done_at: None,
            transactions: 0,
            owed_members: 0,
            broadcasts: [0; 3],
        }
    }

    /// Plays the other run.
    ///
    /// # Errors
    /// As [`Self::demo`] and [`Self::signet`].
    pub fn switch(&mut self) -> Result<(), String> {
        *self = match self.source {
            Source::Demo => Self::signet()?,
            Source::Signet => Self::demo(1)?,
        };
        Ok(())
    }

    /// The events in this run.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The number of events played.
    #[must_use]
    pub fn played(&self) -> usize {
        self.next
    }

    /// Runs the clock forward `dt` seconds.
    ///
    /// # Errors
    /// The next run could not start.
    pub fn advance(&mut self, dt: f32) -> Result<(), String> {
        self.time += dt.max(0.0);
        self.wait -= dt.max(0.0);
        while self.wait <= 0.0 && self.next < self.events.len() {
            let event = self.events[self.next].clone();
            self.next += 1;
            self.wait += self.dwell(&event);
            self.play(&event);
        }
        if self.next == self.events.len() && self.done_at.is_none() {
            self.done_at = Some(self.time);
            if !self.paid.is_empty() {
                self.say(words::EVERYONE_PAID.into(), true);
            }
        }
        if self.done_at.is_some_and(|at| self.time - at > REST) {
            let time = self.time;
            *self = match self.source {
                Source::Demo => Self::demo(self.seed.wrapping_add(1))?,
                Source::Signet => Self::signet()?,
            };
            self.time = time;
        }
        let now = self.time;
        self.effects.retain(|e| now - e.start < e.length);
        if self.dirty {
            self.refresh_sums();
        }
        Ok(())
    }

    /// Seconds to rest after `event` before the next.
    fn dwell(&self, event: &Event) -> f32 {
        let down = self.state.escape.down;
        let level = event.data.get("level").and_then(Value::as_u64).unwrap_or(0);
        match (self.source, event.kind.as_str()) {
            (_, "tree_open") => 4.0,
            (_, "enclave_down") => 5.0,
            (_, "measured") => 0.0,
            (Source::Demo, "attestation") => 0.9,
            (Source::Signet, "attestation") => 0.25,
            (Source::Demo, "block") if down => 0.55,
            (Source::Demo, "block") => 0.1,
            (Source::Demo, "light_client") => 0.02,
            (Source::Demo, "transition" | "simulation") if op(event) == "refresh" => 1.4,
            (Source::Demo, "transition" | "simulation") => 0.22,
            (Source::Signet, "transition") if op(event) == "refresh" => 0.6,
            (Source::Signet, "transition" | "simulation") => 2.2,
            (Source::Demo, "payout") if down => 0.008,
            (Source::Demo, "payout") => 0.0,
            (Source::Signet, "payout") => 1.5,
            (Source::Demo, "unroll_broadcast" | "unroll_confirmed") => match level {
                0 => 2.4,
                1 => 0.22,
                _ => 0.035,
            },
            (_, "unroll_broadcast" | "unroll_confirmed") => 2.5,
            _ => 0.4,
        }
    }

    fn effect(&mut self, fx: Fx, length: f32) {
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Effect {
            fx,
            start: self.time,
            length,
        });
    }

    /// Sets the caption. A passing event (`major` false) waits until the
    /// last caption has been readable for a moment.
    fn say(&mut self, text: String, major: bool) {
        if !major && self.time < self.hold_until {
            return;
        }
        self.caption = (text, self.time);
        self.counting = false;
        self.hold_until = self.time + if major { 3.5 } else { 2.2 };
    }

    fn tick(&mut self, text: String) {
        self.ticker.push_back((text, self.time));
        while self.ticker.len() > TICKER {
            self.ticker.pop_front();
        }
    }

    fn play(&mut self, event: &Event) {
        // Cloning the snapshot per event would copy thousands of leaves.
        let unchecked = self.state.watch.attestation.result.is_empty();
        let paid_before = self.state.escape.paid.len();
        if let Err(error) = self.state.apply(event) {
            // The run was checked when it loaded; stop rather than draw a
            // state it never reached.
            self.say(error, true);
            self.next = self.events.len();
            return;
        }
        match event.kind.as_str() {
            "tree_open" => {
                self.shape = Shape::new(self.state.depth);
                self.followers = self
                    .state
                    .watch
                    .observers
                    .clone()
                    .unwrap_or_else(|| self.state.leaves.keys().copied().take(8).collect());
                self.dirty = true;
                let text = words::opened(self.state.leaves.len(), self.state.output_amount);
                self.say(text, true);
            }
            "transition" | "simulation" => self.transition(event),
            "attestation" => {
                let ok = self.state.watch.attestation.result == "attested";
                self.effect(Fx::Check { ok }, 2.0);
                if ok {
                    self.say(words::CHECKED.into(), unchecked);
                } else {
                    self.say(words::CHECK_FAILED.into(), true);
                }
            }
            "light_client" => {
                let member = event
                    .data
                    .get("member")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let ok = event.data.get("result").and_then(Value::as_str) == Some("ok");
                self.follow.insert(member, (ok, self.time));
                self.effect(Fx::Follow { member, ok }, 1.6);
                if ok {
                    self.say(words::FOLLOWED.into(), false);
                } else {
                    self.say(words::FOLLOW_ALARM.into(), true);
                }
            }
            "block" => {
                let hit = self.state.watch.blocks.last().is_some_and(|b| b.hit);
                self.effect(Fx::Block { hit }, 1.2);
                if self.state.escape.down && self.broadcasts[0] == 0 {
                    let left = self.state.escape.remaining_blocks();
                    self.say(words::waiting(left), true);
                }
            }
            "enclave_down" => {
                self.killed_at = Some(self.time);
                self.effect(Fx::Kill, 4.0);
                self.say(words::SWITCHED_OFF.into(), true);
                self.hold_until = self.time + 5.0;
                self.owed.clear();
                for (&pos, leaf) in &self.state.leaves {
                    self.owed
                        .entry((leaf.exit_key.clone(), leaf.amount))
                        .or_default()
                        .push(pos);
                }
                self.owed_members = self.state.leaves.len();
            }
            "unroll_broadcast" => {
                let level = event.data.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
                let seen = &mut self.broadcasts[level.min(2) as usize];
                *seen += 1;
                if *seen == 1 {
                    self.say(words::broadcast(level), true);
                }
                self.effect(Fx::Broadcast { level }, 1.4);
            }
            "unroll_confirmed" => {
                let level = event.data.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
                let txid = event.data.get("txid").and_then(Value::as_str).unwrap_or("");
                let first = self
                    .state
                    .escape
                    .steps
                    .iter()
                    .filter(|(id, s)| s.level == level && s.confirmed && id.as_str() != txid)
                    .count()
                    == 0;
                if first {
                    self.say(words::confirmed(level), true);
                }
            }
            "payout" if self.state.escape.paid.len() > paid_before => {
                if let Some(paid) = self.state.escape.paid.last() {
                    let amount = paid.amount;
                    let slot = self
                        .owed
                        .get_mut(&(paid.key.clone(), amount))
                        .and_then(Vec::pop);
                    if let Some(pos) = slot {
                        self.paid.insert(pos);
                        self.effect(Fx::Payout { pos, amount }, 1.5);
                    }
                }
                let text = words::payouts(
                    self.state.escape.paid.len(),
                    self.owed_members,
                    self.state.escape.total,
                );
                // The counter changes with every payout; it keeps the time it
                // first appeared so it doesn't fade in again each time.
                let since = if self.counting {
                    self.caption.1
                } else {
                    self.time
                };
                self.caption = (text, since);
                self.counting = true;
                self.hold_until = self.time + 0.5;
            }
            _ => {}
        }
    }

    fn transition(&mut self, event: &Event) {
        let op = op(event).to_owned();
        let changes: Vec<Change> = event.field("changes").unwrap_or_default();
        self.dirty = true;
        for change in &changes {
            self.changed.insert(change.pos, self.time);
        }
        self.effect(Fx::Sign, 1.0);
        let amount_of = |c: &Change| -> (u64, u64) {
            (
                c.old.as_ref().map_or(0, |l| l.amount),
                c.new.as_ref().map_or(0, |l| l.amount),
            )
        };
        let long = if self.source == Source::Signet {
            2.4
        } else {
            1.8
        };
        match op.as_str() {
            "refresh" | "migrate" => {
                self.effect(Fx::Refresh, 2.2);
                self.say(words::REFRESHED.into(), false);
                self.tick(words::ticker::REFRESHED.into());
                return;
            }
            "join" => {
                if let Some(c) = changes.first() {
                    let amount = amount_of(c).1;
                    self.effect(Fx::Join { pos: c.pos }, long);
                    self.say(words::joined(amount), false);
                    self.tick(words::ticker::joined(amount));
                }
            }
            "update" => {
                if let Some(c) = changes.first() {
                    self.effect(Fx::Update { pos: c.pos }, long);
                    self.say(words::UPDATED.into(), false);
                    self.tick(words::ticker::UPDATED.into());
                }
            }
            "exit" | "evict" | "detach" => {
                for c in &changes {
                    let amount = amount_of(c).0;
                    self.effect(Fx::Exit { pos: c.pos, amount }, long);
                    self.say(words::left(amount), false);
                    self.tick(words::ticker::left(amount));
                }
            }
            "claim" | "split" | "merge" | "rewrite" => {
                let from = changes.iter().find(|c| {
                    let (old, new) = amount_of(c);
                    new < old
                });
                let to = changes.iter().find(|c| {
                    let (old, new) = amount_of(c);
                    new > old
                });
                if let (Some(from), Some(to)) = (from, to) {
                    let amount = amount_of(to).1 - amount_of(to).0;
                    if op == "claim" {
                        self.effect(
                            Fx::Claim {
                                from: from.pos,
                                to: to.pos,
                            },
                            long,
                        );
                        self.say(words::paid(amount), false);
                        self.tick(words::ticker::paid(amount));
                    } else {
                        self.effect(
                            Fx::Split {
                                from: from.pos,
                                to: to.pos,
                            },
                            long,
                        );
                        self.say(words::split(amount), false);
                        self.tick(words::ticker::split(amount));
                    }
                }
            }
            _ => {}
        }
        self.transactions += 1;
    }

    fn refresh_sums(&mut self) {
        let depth = self.state.depth;
        self.sums = (0..=depth).map(|h| vec![0; 1 << (depth - h)]).collect();
        for (&pos, leaf) in &self.state.leaves {
            for h in 0..=depth {
                self.sums[h as usize][(pos >> h) as usize] += leaf.amount;
            }
        }
        self.dirty = false;
    }

    /// A node's balance, or 0 for an empty subtree.
    #[must_use]
    pub fn sum(&self, height: u32, index: u64) -> u64 {
        self.sums
            .get(height as usize)
            .and_then(|level| level.get(index as usize))
            .copied()
            .unwrap_or(0)
    }

    /// The light clients with their places in the ring.
    #[must_use]
    pub fn follower_count(&self) -> usize {
        self.followers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play_to_end(stage: &mut Stage, step: f32) -> Vec<String> {
        let mut captions = Vec::new();
        let mut guard = 0;
        while stage.done_at.is_none() {
            stage.advance(step).unwrap();
            if captions.last() != Some(&stage.caption.0) {
                captions.push(stage.caption.0.clone());
            }
            guard += 1;
            assert!(guard < 200_000, "the run never ends");
        }
        captions
    }

    #[test]
    fn the_demo_plays_every_event_and_pays_every_member() {
        let mut stage = Stage::demo(3).unwrap();
        let captions = play_to_end(&mut stage, 1.0 / 30.0);
        assert_eq!(stage.played(), stage.len());
        assert!(stage.killed_at.is_some());
        assert!(stage.owed_members > 2_000);
        assert_eq!(stage.paid.len(), stage.owed_members);
        assert!(stage.transactions > 300);
        assert_eq!(stage.broadcasts, [1, 8, 64]);
        assert_eq!(stage.caption.0, words::EVERYONE_PAID);
        assert!(captions.iter().any(|c| c == words::SWITCHED_OFF));
        assert!(captions.iter().any(|c| c.starts_with("Waiting ")));
        assert!(captions.iter().any(|c| c.contains("members paid back")));
        // A loop lasts a few minutes, with the switch-off after about two.
        let length = stage.done_at.unwrap();
        assert!((120.0..300.0).contains(&length), "{length}");
        let kill = stage.killed_at.unwrap();
        assert!((90.0..220.0).contains(&kill), "{kill}");
        // Every caption the run produced passes the copy guard.
        for caption in &captions {
            assert!(oa_copy::violations(caption, &[]).is_empty(), "{caption}");
        }
    }

    #[test]
    fn node_sums_match_the_tree_while_it_plays() {
        let mut stage = Stage::demo(5).unwrap();
        for _ in 0..600 {
            stage.advance(0.25).unwrap();
            let depth = stage.state.depth;
            assert_eq!(stage.sum(depth, 0), stage.state.output_amount);
            assert_eq!(stage.sums[0].iter().sum::<u64>(), stage.state.output_amount);
        }
        assert!(stage.effects.len() <= MAX_EFFECTS);
    }

    #[test]
    fn the_real_run_replays_and_starts_again() {
        let mut stage = Stage::signet().unwrap();
        assert_eq!(stage.source, Source::Signet);
        let captions = play_to_end(&mut stage, 0.1);
        assert_eq!(stage.state.network, "signet");
        assert!(stage.killed_at.is_some());
        assert_eq!(stage.paid.len(), stage.owed_members);
        assert!(captions.iter().any(|c| c == words::SWITCHED_OFF));
        let length = stage.done_at.unwrap();
        assert!((30.0..180.0).contains(&length), "{length}");
        stage.advance(REST + 1.0).unwrap();
        assert_eq!(stage.source, Source::Signet);
        assert!(stage.played() < stage.len());
        assert!(stage.done_at.is_none());
    }

    #[test]
    fn the_query_picks_the_run() {
        assert_eq!(
            Stage::from_query("?run=signet").unwrap().source,
            Source::Signet
        );
        let demo = Stage::from_query("?seed=9").unwrap();
        assert_eq!((demo.source, demo.seed), (Source::Demo, 9));
        let mut stage = Stage::from_query("").unwrap();
        assert_eq!(stage.seed, 1);
        stage.switch().unwrap();
        assert_eq!(stage.source, Source::Signet);
    }
}
