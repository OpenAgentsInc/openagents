//! K7: the accounting transactions conserve allocation allowance.
//!
//! This models transaction boundaries, duplicate requests, crashes, lost
//! provider answers, and late observations. The PostgreSQL tests separately
//! check that real concurrent transactions implement those boundaries.
use stateright::{Checker, Model, Property};

const LIMIT: u8 = 6;
const MAXIMUM: u8 = 3;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
struct Slot {
    reserved: bool,
    started: bool,
    stopped: bool,
    unknown: bool,
    used: Option<u8>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
struct State {
    slots: [Slot; 3],
    crashed: bool,
}
impl State {
    fn committed(&self) -> u8 {
        self.slots
            .iter()
            .filter(|s| s.reserved)
            .map(|s| s.used.unwrap_or(MAXIMUM))
            .sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Reserve(usize),
    Start(usize),
    LoseReply(usize),
    Stop(usize),
    Resolve(usize, u8),
    Crash,
}
struct Accounting;
impl Model for Accounting {
    type State = State;
    type Action = Action;
    fn init_states(&self) -> Vec<State> {
        vec![State::default()]
    }
    fn actions(&self, _: &State, actions: &mut Vec<Action>) {
        actions.push(Action::Crash);
        for i in 0..3 {
            actions.extend([
                Action::Reserve(i),
                Action::Start(i),
                Action::LoseReply(i),
                Action::Stop(i),
            ]);
            actions.extend((0..=MAXIMUM).map(|used| Action::Resolve(i, used)));
        }
    }
    fn next_state(&self, state: &State, action: Action) -> Option<State> {
        let mut next = state.clone();
        match action {
            Action::Reserve(i) => {
                // A retry reuses the operation; it never adds another hold.
                if !next.slots[i].reserved && next.committed() + MAXIMUM <= LIMIT {
                    next.slots[i].reserved = true;
                }
            }
            Action::Start(i) if next.slots[i].reserved && next.slots[i].used.is_none() => {
                next.slots[i].started = true;
            }
            Action::LoseReply(i) if next.slots[i].started && next.slots[i].used.is_none() => {
                next.slots[i].unknown = true;
            }
            Action::Stop(i) if next.slots[i].started => {
                // A late stop records an observation, not a second settlement.
                next.slots[i].stopped = true;
            }
            Action::Resolve(i, used) if next.slots[i].stopped && next.slots[i].used.is_none() => {
                next.slots[i].used = Some(used);
            }
            // Crashing has no accounting effect. There is no timeout release.
            Action::Crash => next.crashed = true,
            _ => {}
        }
        (next != *state).then_some(next)
    }
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always(
                "K7 reserved plus used fits the quota",
                |_: &Accounting, s: &State| s.committed() <= LIMIT,
            ),
            Property::always(
                "a provider effect always has a reservation",
                |_: &Accounting, s: &State| s.slots.iter().all(|r| !r.started || r.reserved),
            ),
            Property::always(
                "settlement requires observed stop",
                |_: &Accounting, s: &State| s.slots.iter().all(|r| r.used.is_none() || r.stopped),
            ),
            Property::always(
                "unknown outcomes remain held until resolved",
                |_: &Accounting, s: &State| s.slots.iter().all(|r| !r.unknown || r.reserved),
            ),
            Property::sometimes(
                "a crash and late answer can resolve all operations",
                |_: &Accounting, s: &State| s.crashed && s.slots.iter().all(|r| r.used.is_some()),
            ),
        ]
    }
}

#[test]
fn accounting_conserves_allowance_across_crashes_and_late_answers() {
    let checker = Accounting.checker().spawn_bfs().join();
    eprintln!("K7: {} states", checker.unique_state_count());
    checker.assert_properties();
}
