//! A Stateright model of the working-computer lifecycle on the real pure
//! core (verification target 2, `docs/verification/targets.md`; #11839).
//!
//! Every step is the production code: `decide` picks the next effect from
//! the record and `apply` changes the record, in the order
//! `driver::execute` uses (request, provider effect, observation). Around
//! it the model puts what production cannot control:
//!
//! * two reconcilers, each `read+decide -> effect -> deliver`, interleaved
//!   with each other and with the owner (`leased: false`), or one at a time
//!   as the store's lease enforces (`leased: true`);
//! * up to two crashes, each losing a reconciler's pending step;
//! * a provider whose every answer may be done, failed or unknown, an
//!   unknown effect having happened or not (a bounded number of faults);
//! * the owner's prompt, stop and delete.
//!
//! Properties, always:
//!
//! * K1, one computer per intent: at most one live provider resource.
//! * K2, delete is final: a deleted record stays deleted and nothing of it
//!   is left alive at the provider.
//! * K3, no done without observing: every `Done` fact in the record
//!   (create, resource stop, deletion) is true of the provider.
//! * K4, one owner: every checkpoint is in this computer's and owner's
//!   custody.
//!
//! and, sometimes: awake, stopped and woken again, deleted.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use coder_working_computer::provider::{CheckpointEvidence, Inspection, Meter, Outcome};
use coder_working_computer::{
    Applied, Bounds, Command, Computer, Custody, Decision, Fact, Phase, Principal, Spec,
    StopReason, Trigger, apply, decide,
};
use stateright::{Checker, HasDiscoveries, Model, Property};

const NOW: u64 = 1_000;
const MAX_PROMPTS: u64 = 1;

fn owner() -> Principal {
    Principal {
        workspace: "ws-1".into(),
        principal: "user-1".into(),
    }
}

fn computer() -> Computer {
    Computer::new(
        Spec {
            id: "computer-1".into(),
            owner: owner(),
            chat: "chat-1".into(),
            project: coder_environment::ProjectLink {
                workspace: "ws-1".into(),
                project: "proj-1".into(),
            },
            source: coder_environment::SourcePin {
                repository: Some("example/repo".into()),
                revision: "a".repeat(40),
                digest: "b".repeat(64),
            },
            base: None,
            size: "small".into(),
            credential_names: Default::default(),
            services: vec![],
            bounds: Bounds {
                idle_ms: 600_000,
                observed_extension_ms: 300_000,
                absolute_ms: 7_200_000,
            },
        },
        0,
    )
    .unwrap()
}

/// One provider resource, keyed by the create operation that made it
/// (creates are idempotent per operation identity).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Res {
    running: bool,
    deleted: bool,
}

fn resource_id(op: &str) -> String {
    format!("res-{op}")
}

fn op_of(resource: &str) -> &str {
    resource.strip_prefix("res-").expect("model resource id")
}

/// How the provider answers one call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Answer {
    Done,
    /// Definitely not performed.
    Failed,
    /// Performed, but the owner could not learn it.
    UnknownApplied,
    /// Not performed, and the owner could not learn that either.
    UnknownNot,
}

impl Answer {
    fn applied(self) -> bool {
        matches!(self, Answer::Done | Answer::UnknownApplied)
    }
    fn outcome<T>(self, value: T) -> Outcome<T> {
        match self {
            Answer::Done => Outcome::done(value),
            Answer::Failed => Outcome::failed("model: failed"),
            Answer::UnknownApplied | Answer::UnknownNot => Outcome::Unknown {
                reason: "model: unknown".into(),
            },
        }
    }
}

/// A reconciler between its read and its observation.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// Decided (and any request command retained); the effect is next.
    Effect(Decision, Computer),
    /// The effect happened; this observation is next.
    Deliver(Command),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    record: Computer,
    world: BTreeMap<String, Res>,
    steps: [Option<Step>; 2],
    crashes: u8,
    faults: u8,
    owner_stopped: bool,
    delete_wanted: bool,
    /// The record has been `Deleted` at some point.
    ever_deleted: bool,
    /// The record has been `Stopped` and later `Awake` again.
    stopped_once: bool,
    woke_again: bool,
}

impl Hash for State {
    fn hash<H: Hasher>(&self, h: &mut H) {
        serde_json::to_string(&self.record).unwrap().hash(h);
        self.world.hash(h);
        for s in &self.steps {
            match s {
                None => 0u8.hash(h),
                Some(Step::Effect(d, c)) => {
                    1u8.hash(h);
                    format!("{d:?}").hash(h);
                    serde_json::to_string(c).unwrap().hash(h);
                }
                Some(Step::Deliver(cmd)) => {
                    2u8.hash(h);
                    serde_json::to_string(cmd).unwrap().hash(h);
                }
            }
        }
        (
            self.crashes,
            self.faults,
            self.owner_stopped,
            self.delete_wanted,
            self.ever_deleted,
            self.stopped_once,
            self.woke_again,
        )
            .hash(h);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    /// Reconciler `r` reads the record, decides, and retains any request.
    Decide(usize, Trigger),
    /// Reconciler `r` performs its decided effect; the provider answers.
    Effect(usize, Answer),
    /// Reconciler `r` applies the observation to the current record.
    Deliver(usize),
    /// Reconciler `r` crashes and loses its pending step.
    Crash(usize),
    /// The engine finishes the running turn.
    FinishTurn,
    OwnerStop,
    OwnerDelete,
    /// The owner's delete, once the computer is at rest.
    RequestDelete,
}

struct Lifecycle {
    /// Reconcilers take turns, as the store's lease makes them.
    leased: bool,
    /// Non-done provider answers in one history.
    faults: u8,
    crashes: u8,
    /// Boots and create attempts the record may reach.
    boots: usize,
    creates: usize,
}

/// `apply`, keeping the record on refusal or no change. The revision
/// counter is the store's compare-and-swap token; neither `decide` nor
/// `apply` reads it, so the model holds it at zero to keep the state space
/// finite.
fn applied(c: &Computer, cmd: &Command) -> Option<Computer> {
    match apply(c, cmd, NOW) {
        Ok(Applied::Changed(n)) => {
            let mut n = *n;
            n.revision = 0;
            Some(n)
        }
        Ok(Applied::Unchanged) => Some(c.clone()),
        Err(_) => None,
    }
}

fn running(c: &Computer) -> bool {
    matches!(
        c.phase,
        Phase::Awake | Phase::Turn { .. } | Phase::Booting | Phase::Checkpointing { .. }
    )
}

/// Decisions that produce a provider effect.
fn has_effect(d: &Decision) -> bool {
    !matches!(
        d,
        Decision::Skip
            | Decision::Wait(_)
            | Decision::Refuse(_)
            | Decision::Dispatch { .. }
            | Decision::Stop { .. }
    )
}

/// Reads change nothing at the provider, so "unknown but applied" is the
/// same as "unknown".
fn is_read(d: &Decision) -> bool {
    matches!(d, Decision::CheckMeter { .. } | Decision::Inspect)
}

impl Lifecycle {
    fn note(&self, mut s: State) -> State {
        if matches!(s.record.phase, Phase::Deleted) {
            s.ever_deleted = true;
        }
        if matches!(s.record.phase, Phase::Stopped) {
            s.stopped_once = true;
        }
        if s.stopped_once && matches!(s.record.phase, Phase::Awake) {
            s.woke_again = true;
        }
        s
    }

    /// The effect of `d` at the provider, and the observation to retain.
    fn effect(
        &self,
        world: &mut BTreeMap<String, Res>,
        c: &Computer,
        d: &Decision,
        a: Answer,
    ) -> Option<Command> {
        let res_of = |c: &Computer| c.resource().map(str::to_owned);
        Some(match d {
            Decision::Create { operation } => {
                let gone = world.get(operation).is_some_and(|r| r.deleted);
                // The driver contract: `Failed` means the effect definitely
                // did not happen. A retried create whose first attempt did
                // happen returns that resource; it never answers `Failed`.
                // (Without this assumption the checker finds a second live
                // resource: a lost Done, then a Failed retry, then a fresh
                // create.)
                let exists = world.get(operation).is_some_and(|r| !r.deleted);
                if exists && a == Answer::Failed {
                    return None;
                }
                let a = if gone { Answer::Failed } else { a };
                if a.applied() {
                    world.entry(operation.clone()).or_insert(Res {
                        running: true,
                        deleted: false,
                    });
                }
                Command::ObserveCreate {
                    operation: operation.clone(),
                    outcome: a.outcome(resource_id(operation)),
                }
            }
            Decision::Restore { .. } => {
                let r = res_of(c)?;
                let res = world.get_mut(op_of(&r))?;
                // The same contract for a retried resume: a computer an
                // earlier attempt already started is not `Failed`. (Without
                // it the checker finds a record that says stopped, "not
                // resumed", over a running computer.)
                if res.running && !res.deleted && a == Answer::Failed {
                    return None;
                }
                let a = if res.deleted { Answer::Failed } else { a };
                if a.applied() {
                    res.running = true;
                }
                Command::ObserveRestore {
                    boot: c.boot()?.number,
                    outcome: a.outcome("restored".into()),
                }
            }
            Decision::ApplyCredentials { boot } => Command::ObserveCredentials {
                boot: *boot,
                outcome: a.outcome("applied".into()),
            },
            Decision::Checkpoint { generation } => {
                let r = res_of(c)?;
                let res = world.get_mut(op_of(&r))?;
                let a = if res.deleted { Answer::Failed } else { a };
                // A Boat checkpoint quiesces by stopping the computer.
                if a.applied() {
                    res.running = false;
                }
                Command::ObserveCheckpoint {
                    generation: *generation,
                    outcome: a.outcome(CheckpointEvidence {
                        snapshot: format!("snap-{generation}"),
                        stopped: Some(format!("stop-ck-{generation}")),
                    }),
                }
            }
            Decision::ShutdownProcesses { boot } => Command::ObserveShutdown {
                boot: *boot,
                outcome: a.outcome("shutdown".into()),
            },
            Decision::StopResource { boot } => {
                let r = res_of(c)?;
                let res = world.get_mut(op_of(&r))?;
                if a.applied() {
                    res.running = false;
                }
                Command::ObserveStop {
                    boot: *boot,
                    outcome: a.outcome(format!("stop-{boot}")),
                }
            }
            Decision::CheckMeter { boot } => {
                let r = res_of(c)?;
                let res = world.get(op_of(&r))?;
                Command::ObserveMeter {
                    boot: *boot,
                    outcome: a.outcome(Meter {
                        running: res.running && !res.deleted,
                        evidence: format!("meter-{boot}"),
                        seconds: None,
                    }),
                }
            }
            Decision::DeleteResource { operation } => {
                let r = res_of(c)?;
                let res = world.get_mut(op_of(&r))?;
                if a.applied() {
                    res.running = false;
                    res.deleted = true;
                }
                Command::ObserveDeleteResource {
                    operation: operation.clone(),
                    outcome: a.outcome(format!("deleted-{operation}")),
                }
            }
            Decision::Inspect => {
                let outcome = match res_of(c).and_then(|r| world.get(op_of(&r))) {
                    Some(res) => a.outcome(Inspection {
                        running: (!res.deleted).then_some(res.running),
                        stop: None,
                        latest_snapshot: None,
                    }),
                    None => Outcome::failed("no resource to inspect"),
                };
                Command::ObserveInspection { outcome }
            }
            _ => return None,
        })
    }
}

impl Model for Lifecycle {
    type State = State;
    type Action = Action;

    fn init_states(&self) -> Vec<State> {
        vec![State {
            record: computer(),
            world: BTreeMap::new(),
            steps: [None, None],
            crashes: 0,
            faults: 0,
            owner_stopped: false,
            delete_wanted: false,
            ever_deleted: false,
            stopped_once: false,
            woke_again: false,
        }]
    }

    fn actions(&self, s: &State, out: &mut Vec<Action>) {
        let busy = s.steps.iter().any(Option::is_some);
        for r in 0..2 {
            match &s.steps[r] {
                None if !(self.leased && busy) => {
                    // A prompt outside `Awake` decides what a wake does.
                    out.push(Action::Decide(r, Trigger::Wake));
                    out.push(Action::Decide(r, Trigger::Tick));
                    if matches!(s.record.phase, Phase::Awake) {
                        out.push(Action::Decide(r, Trigger::Prompt));
                    }
                }
                None => {}
                Some(Step::Effect(d, _)) => {
                    out.push(Action::Effect(r, Answer::Done));
                    if s.faults < self.faults {
                        out.push(Action::Effect(r, Answer::Failed));
                        out.push(Action::Effect(r, Answer::UnknownNot));
                        if !is_read(d) {
                            out.push(Action::Effect(r, Answer::UnknownApplied));
                        }
                    }
                }
                Some(Step::Deliver(_)) => out.push(Action::Deliver(r)),
            }
            if s.steps[r].is_some() && s.crashes < self.crashes {
                out.push(Action::Crash(r));
            }
        }
        // The owner and the engine act on the record directly, under the
        // lease like everyone else.
        if !(self.leased && busy) {
            if matches!(s.record.phase, Phase::Turn { .. }) {
                out.push(Action::FinishTurn);
            }
            if !s.owner_stopped && running(&s.record) {
                out.push(Action::OwnerStop);
            }
            if !s.delete_wanted {
                out.push(Action::OwnerDelete);
            }
            if s.delete_wanted
                && matches!(
                    s.record.phase,
                    Phase::Stopped | Phase::Failed { .. } | Phase::New
                )
            {
                out.push(Action::RequestDelete);
            }
        }
    }

    fn next_state(&self, s: &State, action: Action) -> Option<State> {
        let mut n = s.clone();
        match action {
            Action::Decide(r, trigger) => {
                let c = &s.record;
                if trigger == Trigger::Prompt && c.turn.dispatched >= MAX_PROMPTS {
                    return None;
                }
                if trigger == Trigger::Wake && c.boots.len() >= self.boots {
                    return None;
                }
                let d = decide(c, trigger, NOW);
                match &d {
                    Decision::Dispatch { generation } => {
                        n.record = applied(
                            c,
                            &Command::StartTurn {
                                generation: *generation,
                            },
                        )?;
                    }
                    Decision::Stop { reason } => {
                        n.record = applied(c, &Command::RequestStop { reason: *reason })?;
                    }
                    d if !has_effect(d) => return None,
                    // The request is retained before the provider is called
                    // (`driver::execute`).
                    Decision::Create { operation } => {
                        if c.creates.len() >= self.creates
                            && !c.creates.iter().any(|a| &a.operation == operation)
                        {
                            return None;
                        }
                        if !matches!(c.phase, Phase::Creating | Phase::Unknown { .. }) {
                            n.record = applied(
                                c,
                                &Command::RequestCreate {
                                    operation: operation.clone(),
                                },
                            )?;
                        }
                        n.steps[r] = Some(Step::Effect(d.clone(), n.record.clone()));
                    }
                    Decision::Restore { checkpoint } => {
                        if matches!(c.phase, Phase::Stopped) {
                            n.record = applied(
                                c,
                                &Command::RequestRestore {
                                    checkpoint: checkpoint.clone(),
                                },
                            )?;
                        }
                        n.steps[r] = Some(Step::Effect(d.clone(), n.record.clone()));
                    }
                    Decision::DeleteResource { operation } => {
                        let requested = c
                            .creates
                            .iter()
                            .any(|a| &a.operation == operation && a.deletion.is_some());
                        if !requested {
                            n.record = applied(c, &Command::RequestDeleteResource)?;
                        }
                        n.steps[r] = Some(Step::Effect(d.clone(), n.record.clone()));
                    }
                    _ => n.steps[r] = Some(Step::Effect(d.clone(), c.clone())),
                }
            }
            Action::Effect(r, answer) => {
                let Some(Step::Effect(d, seen)) = s.steps[r].clone() else {
                    return None;
                };
                if answer != Answer::Done {
                    n.faults += 1;
                }
                let cmd = self.effect(&mut n.world, &seen, &d, answer)?;
                n.steps[r] = Some(Step::Deliver(cmd));
            }
            Action::Deliver(r) => {
                let Some(Step::Deliver(cmd)) = s.steps[r].clone() else {
                    return None;
                };
                // A stale observation is refused by `apply`; the record stays.
                n.record = applied(&s.record, &cmd).unwrap_or_else(|| s.record.clone());
                n.steps[r] = None;
            }
            Action::Crash(r) => {
                n.steps[r] = None;
                n.crashes += 1;
            }
            Action::FinishTurn => {
                let Phase::Turn { generation } = s.record.phase else {
                    return None;
                };
                n.record = applied(&s.record, &Command::FinishTurn { generation })?;
            }
            Action::OwnerStop => {
                n.owner_stopped = true;
                n.record = applied(
                    &s.record,
                    &Command::RequestStop {
                        reason: StopReason::Owner,
                    },
                )?;
            }
            Action::OwnerDelete => {
                n.delete_wanted = true;
                if running(&s.record) {
                    n.record = applied(
                        &s.record,
                        &Command::RequestStop {
                            reason: StopReason::Owner,
                        },
                    )?;
                }
            }
            Action::RequestDelete => {
                n.record = applied(&s.record, &Command::RequestDelete)?;
            }
        }
        Some(self.note(n))
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always(
                "K1 at most one live resource",
                |_: &Lifecycle, s: &State| s.world.values().filter(|r| !r.deleted).count() <= 1,
            ),
            Property::always("K2 delete is final", |_: &Lifecycle, s: &State| {
                !s.ever_deleted
                    || (matches!(s.record.phase, Phase::Deleted)
                        && s.world.values().all(|r| r.deleted))
            }),
            Property::always(
                "K3 no done without observing",
                |_: &Lifecycle, s: &State| k3(s),
            ),
            Property::always(
                "K4 checkpoints stay with their owner",
                |_: &Lifecycle, s: &State| {
                    s.record.checkpoints.iter().all(|k| {
                        let Custody::UserPrivate {
                            principal,
                            computer,
                        } = &k.custody;
                        principal == &s.record.owner && computer == &s.record.id
                    })
                },
            ),
            Property::sometimes("awake", |_: &Lifecycle, s: &State| {
                matches!(s.record.phase, Phase::Awake)
            }),
            Property::sometimes("stopped and woken again", |_: &Lifecycle, s: &State| {
                s.woke_again
            }),
            Property::sometimes("deleted", |_: &Lifecycle, s: &State| s.ever_deleted),
        ]
    }
}

/// Every `Done` the record holds is true at the provider.
fn k3(s: &State) -> bool {
    for a in &s.record.creates {
        let res = s.world.get(&a.operation);
        if a.fact.is_done() && res.is_none() {
            return false;
        }
        if a.deletion.as_ref().is_some_and(Fact::is_done) && res.is_some_and(|r| !r.deleted) {
            return false;
        }
    }
    // A record at rest says its resource is not running.
    if matches!(s.record.phase, Phase::Stopped | Phase::Deleted)
        && let Some(r) = s.record.resource()
        && s.world.get(op_of(r)).is_some_and(|r| r.running)
    {
        return false;
    }
    true
}

fn check(model: Lifecycle) -> impl Checker<Lifecycle> {
    check_until(model, HasDiscoveries::All)
}

fn check_until(model: Lifecycle, until: HasDiscoveries) -> impl Checker<Lifecycle> {
    model
        .checker()
        .threads(std::thread::available_parallelism().map_or(1, |n| n.get()))
        .finish_when(until)
        .spawn_bfs()
        .join()
}

/// One fault and one crash: what the default `cargo test` checks.
#[test]
fn leased_reconcilers_keep_k1_to_k4() {
    let c = check(Lifecycle {
        leased: true,
        faults: 1,
        crashes: 1,
        boots: 3,
        creates: 3,
    });
    eprintln!("leased: {} states", c.unique_state_count());
    c.assert_properties();
}

/// Two faults and two crashes, about 2.4 million states: run with
/// `cargo test --release -p coder-working-computer --test lifecycle_model
/// -- --ignored`.
#[test]
#[ignore = "about 2.4 million states; run in release"]
fn leased_reconcilers_keep_k1_to_k4_two_faults_two_crashes() {
    let c = check(Lifecycle {
        leased: true,
        faults: 2,
        crashes: 2,
        boots: 3,
        creates: 3,
    });
    eprintln!("leased, full bounds: {} states", c.unique_state_count());
    c.assert_properties();
}

/// The lease is load-bearing. Without it (a reconciler whose lease expired
/// while its provider call was still in flight), the checker finds a second
/// live resource: two reconcilers repeat one create, the first answers
/// `Failed` before the second's `Done` lands, the record believes the
/// failure, and a fresh create follows. `oa-computers` must fence a stale
/// holder before its observation counts (#11801, #11809).
#[test]
fn without_the_lease_two_reconcilers_can_orphan_a_resource() {
    let c = check_until(
        Lifecycle {
            leased: false,
            faults: 1,
            crashes: 1,
            boots: 2,
            creates: 2,
        },
        HasDiscoveries::AnyFailures,
    );
    let path = c
        .discovery("K1 at most one live resource")
        .expect("the unleased model breaks K1");
    let states = c.unique_state_count();
    let actions = path.into_actions();
    eprintln!("unleased: {states} states; K1 broken by {actions:?}");
    // Both reconcilers are mid-step at once, and one answer is `Failed`.
    assert!(actions.contains(&Action::Decide(0, Trigger::Wake)));
    assert!(actions.contains(&Action::Decide(1, Trigger::Wake)));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Effect(_, Answer::Failed)))
    );
}
