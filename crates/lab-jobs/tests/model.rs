//! Finite worker-loss model; eligibility calls the production placement policy.
use lab_jobs::{Capabilities, EffectiveRequirements, Placement};
use stateright::{Checker, Model, Property};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct State {
    phase: u8, // queued, claimed, started, done, cancelled, uncertain
    attempt: u8,
    claims: [u8; 3],
    worker: Option<usize>,
    effects: u8,
    completions: u8,
    unsafe_assignment: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Claim(usize),
    Start(usize),
    Finish(usize),
    Expire,
    Cancel,
    Crash,
}
struct Jobs;
fn eligible(i: usize) -> bool {
    let r = EffectiveRequirements {
        cpu_millis: 1000,
        memory_bytes: 64 * 1024 * 1024,
        gpu: false,
        tools: Default::default(),
        max_seconds: 30,
        private_local: true,
        local_assets: ["a".repeat(64)].into(),
        no_training: true,
        job: "echo/shout".into(),
    };
    let w = Capabilities {
        worker: format!("worker-{i}"),
        placement: if i == 0 {
            Placement::Cloud
        } else {
            Placement::Enrolled
        },
        cpu_millis: 1000,
        memory_bytes: 64 * 1024 * 1024,
        gpu: false,
        tools: Default::default(),
        jobs: ["echo/shout".into()].into(),
        local_assets: if i == 2 {
            ["a".repeat(64)].into()
        } else {
            Default::default()
        },
        max_seconds: 30,
    };
    r.eligible(&w)
}
impl Model for Jobs {
    type State = State;
    type Action = Action;
    fn init_states(&self) -> Vec<State> {
        vec![State::default()]
    }
    fn actions(&self, _: &State, a: &mut Vec<Action>) {
        a.extend([Action::Expire, Action::Cancel, Action::Crash]);
        for i in 0..3 {
            a.extend([Action::Claim(i), Action::Start(i), Action::Finish(i)]);
        }
    }
    fn next_state(&self, s: &State, a: Action) -> Option<State> {
        let mut n = s.clone();
        match a {
            Action::Claim(i) if n.phase == 0 && n.attempt < 2 && eligible(i) => {
                n.phase = 1;
                n.attempt += 1;
                n.claims[i] = n.attempt;
                n.worker = Some(i);
                n.unsafe_assignment |= i != 2;
            }
            Action::Start(i) if n.phase == 1 && n.worker == Some(i) && n.claims[i] == n.attempt => {
                n.phase = 2;
                n.effects += 1;
            }
            Action::Finish(i)
                if n.phase == 2 && n.worker == Some(i) && n.claims[i] == n.attempt =>
            {
                n.phase = 3;
                n.completions += 1;
            }
            Action::Expire if n.phase == 1 => {
                n.phase = 0;
                n.worker = None;
            }
            Action::Expire if n.phase == 2 => {
                n.phase = 5;
                n.worker = None;
            }
            Action::Cancel if n.phase <= 1 => {
                n.phase = 4;
                n.worker = None;
            }
            Action::Cancel if n.phase == 2 => {
                n.phase = 5;
                n.worker = None;
            }
            Action::Crash => (),
            _ => (),
        }
        (n != *s).then_some(n)
    }
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always(
                "private inputs stay on an enrolled host with the files",
                |_, s| !s.unsafe_assignment,
            ),
            Property::always(
                "worker loss and cancellation never repeat effects",
                |_, s| s.effects <= 1,
            ),
            Property::always("a job completes at most once", |_, s| s.completions <= 1),
            Property::sometimes("an unused lease can be replaced", |_, s| {
                s.attempt == 2 && s.phase == 3
            }),
            Property::sometimes("a started lost worker remains uncertain", |_, s| {
                s.phase == 5 && s.effects == 1
            }),
        ]
    }
}
#[test]
fn private_placement_leases_cancellation_and_worker_loss() {
    let c = Jobs.checker().spawn_bfs().join();
    eprintln!("Lab placement model: {} states", c.unique_state_count());
    c.assert_properties();
}
