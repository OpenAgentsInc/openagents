//! Provider effects behind one trait. Implementations perform exactly one
//! effect per call and report a typed [`Outcome`]; they decide nothing.
//!
//! The trait deliberately has no operation that reads files out of a
//! computer or checkpoint: OpenAgents services cannot read a user's sign-ins
//! through it.

use crate::{Checkpoint, Computer, ServiceDecl};
use serde::{Deserialize, Serialize};

/// The result of one provider effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome<T> {
    Done {
        value: T,
    },
    /// The provider definitely did not perform the effect.
    Failed {
        reason: String,
    },
    /// The owner could not learn whether the effect happened.
    Unknown {
        reason: String,
    },
}
impl<T> Outcome<T> {
    pub fn done(value: T) -> Self {
        Self::Done { value }
    }
    pub fn failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            reason: reason.into(),
        }
    }
    pub fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
}

/// A completed filesystem checkpoint: provider snapshot identity only, never
/// file contents or credential values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointEvidence {
    pub snapshot: String,
    /// The provider stop operation when the checkpoint quiesced by stopping
    /// the resource (Boat). `None` leaves the resource running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meter {
    pub running: bool,
    pub evidence: String,
}

/// A read-only provider view used to reconcile an unknown outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    /// `None` when the provider reports the resource gone.
    pub running: Option<bool>,
    /// The completed stop operation, if the provider reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_snapshot: Option<String>,
}

#[allow(async_fn_in_trait)]
pub trait Provider {
    /// Create under a retained operation identity; repeating the same
    /// identity must return the same resource, never a second one.
    async fn create(&self, computer: &Computer, operation: &str) -> Outcome<String>;
    /// Resume the retained stopped filesystem. Starts no services and
    /// applies no credentials.
    async fn restore(
        &self,
        computer: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String>;
    /// Apply the computer's selected credentials for this boot only.
    async fn apply_credentials(&self, computer: &Computer, resource: &str) -> Outcome<String>;
    /// Start one declared service and wait for its health rule.
    async fn start_service(
        &self,
        computer: &Computer,
        resource: &str,
        service: &ServiceDecl,
    ) -> Outcome<String>;
    /// Quiesce and snapshot the filesystem for one completed turn.
    async fn checkpoint(
        &self,
        computer: &Computer,
        resource: &str,
        generation: u64,
    ) -> Outcome<CheckpointEvidence>;
    /// Stop declared services and engine processes.
    async fn shutdown_processes(&self, computer: &Computer, resource: &str) -> Outcome<String>;
    async fn stop(&self, computer: &Computer, resource: &str) -> Outcome<String>;
    async fn meter(&self, computer: &Computer, resource: &str) -> Outcome<Meter>;
    async fn delete(&self, computer: &Computer, resource: &str) -> Outcome<String>;
    async fn inspect(&self, computer: &Computer, resource: &str) -> Outcome<Inspection>;
}

/// An in-memory provider for tests: files per resource, snapshots, per-boot
/// environment, services, and injectable failed, unknown, or lost-reply
/// outcomes.
pub mod fake {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    use std::sync::Mutex;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Inject {
        /// Do nothing and report failure.
        Failed,
        /// Do nothing and report unknown.
        Unknown,
        /// Perform the effect and lose the reply (report unknown).
        LostReply,
    }

    #[derive(Clone, Debug, Default)]
    pub struct Machine {
        pub running: bool,
        pub files: BTreeMap<String, String>,
        /// Per-boot process environment; not part of the filesystem.
        pub env: BTreeMap<String, String>,
        pub services: BTreeSet<String>,
        pub meter_running: bool,
        pub stop: Option<String>,
        pub latest_snapshot: Option<String>,
    }

    #[derive(Default)]
    pub struct State {
        pub machines: BTreeMap<String, Machine>,
        pub snapshots: BTreeMap<String, BTreeMap<String, String>>,
        pub operations: BTreeMap<String, String>,
        pub calls: Vec<String>,
        pub inject: BTreeMap<&'static str, VecDeque<Inject>>,
        pub broken_services: BTreeSet<String>,
        /// The meter keeps running after stop (a provider fault).
        pub sticky_meter: bool,
        pub counter: u64,
    }

    pub struct FakeProvider {
        pub state: Mutex<State>,
        /// Values the operator's credential custody would supply.
        pub credential_values: BTreeMap<String, String>,
        /// Whether a checkpoint stops the resource, like Boat.
        pub checkpoint_stops: bool,
    }

    impl FakeProvider {
        pub fn new(credential_values: BTreeMap<String, String>, checkpoint_stops: bool) -> Self {
            Self {
                state: Mutex::new(State::default()),
                credential_values,
                checkpoint_stops,
            }
        }
        pub fn inject(&self, op: &'static str, how: Inject) {
            self.state
                .lock()
                .unwrap()
                .inject
                .entry(op)
                .or_default()
                .push_back(how);
        }
        pub fn calls(&self) -> Vec<String> {
            self.state.lock().unwrap().calls.clone()
        }
        pub fn machine(&self, resource: &str) -> Option<Machine> {
            self.state.lock().unwrap().machines.get(resource).cloned()
        }
        /// The engine or user changes a file during a turn.
        pub fn write(&self, resource: &str, path: &str, contents: &str) {
            let mut s = self.state.lock().unwrap();
            let m = s.machines.get_mut(resource).expect("machine");
            assert!(m.running, "writes need a running machine");
            m.files.insert(path.into(), contents.into());
        }
        pub fn snapshot_files(&self, snapshot: &str) -> Option<BTreeMap<String, String>> {
            self.state.lock().unwrap().snapshots.get(snapshot).cloned()
        }

        /// Record the call and take any injected outcome for `op`.
        fn begin(&self, op: &'static str) -> Option<Inject> {
            let mut s = self.state.lock().unwrap();
            s.calls.push(op.into());
            s.inject.get_mut(op).and_then(VecDeque::pop_front)
        }
        fn next_id(s: &mut State, prefix: &str) -> String {
            s.counter += 1;
            format!("{prefix}-{}", s.counter)
        }
        fn finish<T>(inject: Option<Inject>, value: impl FnOnce() -> Outcome<T>) -> Outcome<T> {
            match inject {
                Some(Inject::Failed) => Outcome::failed("injected failure"),
                Some(Inject::Unknown) => Outcome::unknown("injected unknown"),
                Some(Inject::LostReply) => match value() {
                    Outcome::Done { .. } => Outcome::unknown("reply lost"),
                    other => other,
                },
                None => value(),
            }
        }
        fn with_machine<T>(
            &self,
            resource: &str,
            f: impl FnOnce(&mut State, &str) -> Outcome<T>,
        ) -> Outcome<T> {
            let mut s = self.state.lock().unwrap();
            if !s.machines.contains_key(resource) {
                return Outcome::failed("no such resource");
            }
            f(&mut s, resource)
        }
    }

    impl Provider for FakeProvider {
        async fn create(&self, _c: &Computer, operation: &str) -> Outcome<String> {
            let inject = self.begin("create");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let mut s = self.state.lock().unwrap();
            let resource = if let Some(r) = s.operations.get(operation) {
                r.clone()
            } else {
                let r = Self::next_id(&mut s, "box");
                s.operations.insert(operation.into(), r.clone());
                s.machines.insert(
                    r.clone(),
                    Machine {
                        running: true,
                        meter_running: true,
                        ..Default::default()
                    },
                );
                r
            };
            drop(s);
            Self::finish(inject, || Outcome::done(resource))
        }
        async fn restore(
            &self,
            _c: &Computer,
            resource: &str,
            checkpoint: Option<&Checkpoint>,
        ) -> Outcome<String> {
            let inject = self.begin("restore");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let snapshot = checkpoint.and_then(|c| c.fact.evidence().map(str::to_owned));
            let out = self.with_machine(resource, |s, r| {
                let files = match &snapshot {
                    Some(id) => match s.snapshots.get(id) {
                        Some(files) => Some(files.clone()),
                        None => return Outcome::failed("no such snapshot"),
                    },
                    None => None,
                };
                let m = s.machines.get_mut(r).unwrap();
                if let Some(files) = files {
                    m.files = files;
                }
                m.running = true;
                m.meter_running = true;
                m.env.clear();
                m.services.clear();
                m.stop = None;
                Outcome::done(format!("resumed:{r}"))
            });
            Self::finish(inject, || out)
        }
        async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
            let inject = self.begin("credentials");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let values = &self.credential_values;
            let out = self.with_machine(resource, |s, r| {
                let m = s.machines.get_mut(r).unwrap();
                if !m.running {
                    return Outcome::failed("not running");
                }
                for name in &c.credential_names {
                    match values.get(name) {
                        Some(v) => {
                            m.env.insert(name.clone(), v.clone());
                        }
                        None => return Outcome::failed("credential unavailable"),
                    }
                }
                let names: Vec<_> = c.credential_names.iter().cloned().collect();
                Outcome::done(format!("applied:{}", names.join(",")))
            });
            Self::finish(inject, || out)
        }
        async fn start_service(
            &self,
            _c: &Computer,
            resource: &str,
            service: &ServiceDecl,
        ) -> Outcome<String> {
            let inject = self.begin("service");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = self.with_machine(resource, |s, r| {
                if s.broken_services.contains(&service.name) {
                    return Outcome::failed("health check did not pass");
                }
                let m = s.machines.get_mut(r).unwrap();
                if !m.running {
                    return Outcome::failed("not running");
                }
                m.services.insert(service.name.clone());
                Outcome::done(format!("ready:{}", service.name))
            });
            Self::finish(inject, || out)
        }
        async fn checkpoint(
            &self,
            _c: &Computer,
            resource: &str,
            _generation: u64,
        ) -> Outcome<CheckpointEvidence> {
            let inject = self.begin("checkpoint");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let stops = self.checkpoint_stops;
            let out = self.with_machine(resource, |s, r| {
                let snapshot = Self::next_id(s, "snap");
                let stop = stops.then(|| Self::next_id(s, "stop"));
                let m = s.machines.get_mut(r).unwrap();
                let files = m.files.clone();
                m.latest_snapshot = Some(snapshot.clone());
                if let Some(stop) = &stop {
                    m.running = false;
                    m.env.clear();
                    m.services.clear();
                    m.stop = Some(stop.clone());
                }
                s.snapshots.insert(snapshot.clone(), files);
                Outcome::done(CheckpointEvidence {
                    snapshot,
                    stopped: stop,
                })
            });
            Self::finish(inject, || out)
        }
        async fn shutdown_processes(&self, _c: &Computer, resource: &str) -> Outcome<String> {
            let inject = self.begin("shutdown");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = self.with_machine(resource, |s, r| {
                let m = s.machines.get_mut(r).unwrap();
                m.services.clear();
                Outcome::done("processes stopped".into())
            });
            Self::finish(inject, || out)
        }
        async fn stop(&self, _c: &Computer, resource: &str) -> Outcome<String> {
            let inject = self.begin("stop");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let sticky = self.state.lock().unwrap().sticky_meter;
            let out = self.with_machine(resource, |s, r| {
                let id = match s.machines[r].stop.clone() {
                    Some(id) => id,
                    None => Self::next_id(s, "stop"),
                };
                let m = s.machines.get_mut(r).unwrap();
                m.running = false;
                m.env.clear();
                m.services.clear();
                m.meter_running = sticky;
                m.stop = Some(id.clone());
                Outcome::done(id)
            });
            Self::finish(inject, || out)
        }
        async fn meter(&self, _c: &Computer, resource: &str) -> Outcome<Meter> {
            let inject = self.begin("meter");
            let sticky = self.state.lock().unwrap().sticky_meter;
            let out = self.with_machine(resource, |s, r| {
                let m = s.machines.get_mut(r).unwrap();
                if !m.running && !sticky {
                    m.meter_running = false;
                }
                Outcome::done(Meter {
                    running: m.meter_running,
                    evidence: format!("usage:{r}"),
                })
            });
            Self::finish(inject, || out)
        }
        async fn delete(&self, _c: &Computer, resource: &str) -> Outcome<String> {
            let inject = self.begin("delete");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let mut s = self.state.lock().unwrap();
            let out = if s.machines.remove(resource).is_some() {
                Outcome::done(format!("deleted:{resource}"))
            } else {
                Outcome::done(format!("absent:{resource}"))
            };
            drop(s);
            Self::finish(inject, || out)
        }
        async fn inspect(&self, _c: &Computer, resource: &str) -> Outcome<Inspection> {
            let inject = self.begin("inspect");
            let s = self.state.lock().unwrap();
            let out = match s.machines.get(resource) {
                None => Outcome::done(Inspection {
                    running: None,
                    stop: None,
                    latest_snapshot: None,
                }),
                Some(m) => Outcome::done(Inspection {
                    running: Some(m.running),
                    stop: if m.running { None } else { m.stop.clone() },
                    latest_snapshot: m.latest_snapshot.clone(),
                }),
            };
            drop(s);
            Self::finish(inject, || out)
        }
    }

    fn unreachable_outcome<T>() -> Outcome<T> {
        Outcome::failed("not performed")
    }
}
