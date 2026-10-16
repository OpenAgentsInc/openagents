//! Transaction-level model. Backend conformance separately checks atomic CAS.
use stateright::{Checker, Model, Property};

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
struct State {
    latest: u8,
    writers: [Option<u8>; 2],
    saved: [bool; 2],
    winners: [u8; 3],
    // 0 queued, 1 claimed, 2 started, 3 done, 4 uncertain.
    job: u8,
    attempt: u8,
    claims: [u8; 2],
    effects: u8,
    completed_by: u8,
    cursor: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Read(usize),
    Save(usize),
    Claim(usize),
    Start(usize),
    Complete(usize),
    Expire,
    Crash,
}
struct Storage;
impl Model for Storage {
    type State = State;
    type Action = Action;
    fn init_states(&self) -> Vec<State> {
        vec![State::default()]
    }
    fn actions(&self, _: &State, actions: &mut Vec<Action>) {
        actions.extend([Action::Expire, Action::Crash]);
        for i in 0..2 {
            actions.extend([
                Action::Read(i),
                Action::Save(i),
                Action::Claim(i),
                Action::Start(i),
                Action::Complete(i),
            ]);
        }
    }
    fn next_state(&self, state: &State, action: Action) -> Option<State> {
        let mut s = state.clone();
        match action {
            Action::Read(i) if !s.saved[i] && s.writers[i].is_none() => {
                s.writers[i] = Some(s.latest)
            }
            Action::Save(i) if !s.saved[i] && s.writers[i] == Some(s.latest) && s.latest < 2 => {
                s.latest += 1;
                s.winners[s.latest as usize] += 1;
                s.saved[i] = true;
                s.cursor += 1;
            }
            Action::Claim(i) if s.job == 0 && s.attempt < 2 => {
                s.attempt += 1;
                s.claims[i] = s.attempt;
                s.job = 1;
                s.cursor += 1;
            }
            Action::Start(i) if s.job == 1 && s.claims[i] == s.attempt => {
                s.job = 2;
                s.effects += 1;
            }
            Action::Complete(i) if s.job == 2 && s.claims[i] == s.attempt => {
                s.job = 3;
                s.completed_by = s.claims[i];
                s.cursor += 1;
            }
            Action::Expire if s.job == 1 => {
                s.job = 0;
                s.cursor += 1;
            }
            Action::Expire if s.job == 2 => {
                s.job = 4;
                s.cursor += 1;
            }
            // Process death cannot roll back a committed index or renew a lease.
            Action::Crash => (),
            _ => (),
        }
        (s != *state).then_some(s)
    }
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("one winner per base", |_: &Storage, s: &State| {
                s.winners.iter().all(|n| *n <= 1)
            }),
            Property::always(
                "published versions are contiguous",
                |_: &Storage, s: &State| (1..=s.latest).all(|n| s.winners[n as usize] == 1),
            ),
            Property::always(
                "started work is not automatically executed twice",
                |_: &Storage, s: &State| s.effects <= 1,
            ),
            Property::always(
                "only the current lease can complete",
                |_: &Storage, s: &State| s.completed_by == 0 || s.completed_by == s.attempt,
            ),
            Property::always(
                "unknown work stays unclaimable",
                |_: &Storage, s: &State| s.job != 4 || s.effects == 1,
            ),
            Property::sometimes(
                "an unused lease can expire and be replaced",
                |_: &Storage, s: &State| s.attempt == 2 && s.job == 3,
            ),
            Property::sometimes(
                "a lost started worker leaves an unknown result",
                |_: &Storage, s: &State| s.job == 4,
            ),
        ]
    }
}
#[test]
fn concurrent_versions_and_worker_loss() {
    let checker = Storage.checker().spawn_bfs().join();
    eprintln!("Lab storage model: {} states", checker.unique_state_count());
    checker.assert_properties();
}
