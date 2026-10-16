//! Bounded transaction model: two writers, two readers, retries, checkpoints,
//! and two crashes before commit or after commit but before acknowledgment.
use stateright::{Checker, Model, Property};

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
struct State {
    // Durable sequence assignments survive compaction and crashes.
    batches: Vec<u8>,
    rows: Vec<u8>,
    floor: usize,
    pending: Option<u8>,
    acknowledged: [bool; 2],
    attempts: [u8; 2],
    readers: [usize; 2],
    previous: [usize; 2],
    crashes: u8,
}
#[derive(Clone, Debug, PartialEq)]
enum Action {
    Begin(u8),
    Commit,
    Ack(u8),
    Crash,
    Checkpoint(usize),
    Read(usize),
}
struct Room;
impl Model for Room {
    type State = State;
    type Action = Action;
    fn init_states(&self) -> Vec<State> {
        vec![State::default()]
    }
    fn actions(&self, s: &State, out: &mut Vec<Action>) {
        for w in 0..2 {
            if !s.acknowledged[w] && s.attempts[w] < 2 && s.pending.is_none() {
                out.push(Action::Begin(w as u8));
            }
            if s.batches.contains(&(w as u8)) && !s.acknowledged[w] {
                out.push(Action::Ack(w as u8));
            }
        }
        if s.pending.is_some() {
            out.push(Action::Commit);
        }
        if s.crashes < 2 {
            out.push(Action::Crash);
        }
        for c in s.floor + 1..=s.batches.len() {
            out.push(Action::Checkpoint(c));
        }
        for r in 0..2 {
            if s.readers[r] < s.batches.len() {
                out.push(Action::Read(r));
            }
        }
    }
    fn next_state(&self, s: &State, a: Action) -> Option<State> {
        let mut n = s.clone();
        match a {
            Action::Begin(w) => {
                n.pending = Some(w);
                n.attempts[w as usize] += 1;
            }
            Action::Commit => {
                let w = n.pending.take()?;
                if !n.batches.contains(&w) {
                    n.batches.push(w);
                    n.rows.push(w);
                }
            }
            Action::Ack(w) => n.acknowledged[w as usize] = true,
            Action::Crash => {
                n.pending = None;
                n.crashes += 1;
            }
            Action::Checkpoint(c) => {
                n.floor = c;
                n.rows = n.batches[c..].to_vec();
            }
            Action::Read(r) => {
                n.previous[r] = n.readers[r];
                n.readers[r] = n.floor.max(n.readers[r]);
                if n.readers[r] < n.batches.len() {
                    n.readers[r] += 1;
                }
            }
        }
        Some(n)
    }
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("gap-free sequence", |_: &Room, s: &State| {
                s.rows == s.batches[s.floor..]
            }),
            Property::always("batch applies once", |_: &Room, s: &State| {
                s.batches.iter().filter(|w| **w == 0).count() <= 1
                    && s.batches.iter().filter(|w| **w == 1).count() <= 1
            }),
            Property::always("checkpoint never ahead", |_: &Room, s: &State| {
                s.floor <= s.batches.len()
            }),
            Property::always(
                "reader cursor monotonic and bounded",
                |_: &Room, s: &State| {
                    (0..2).all(|r| s.previous[r] <= s.readers[r] && s.readers[r] <= s.batches.len())
                },
            ),
            Property::sometimes(
                "both readers catch up after crash and retry",
                |_: &Room, s: &State| {
                    s.crashes > 0
                        && s.attempts.contains(&2)
                        && s.batches.len() == 2
                        && s.readers == [2, 2]
                },
            ),
        ]
    }
}
#[test]
fn two_writers_two_readers_crashes_retries_and_compaction() {
    let checker = Room.checker().spawn_bfs().join();
    checker.assert_properties();
    eprintln!(
        "checked {} unique chat states",
        checker.unique_state_count()
    );
}
