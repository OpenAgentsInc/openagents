//! Provider effects behind one trait. Implementations perform exactly one
//! effect per call and report a typed [`Outcome`]; they decide nothing.
//!
//! The trait deliberately has no operation that reads files out of a
//! computer or checkpoint: OpenAgents services cannot read a user's sign-ins
//! through it.
//!
//! [`Commands`] adds identified, at-most-once commands for a dedicated
//! environment setup computer. It returns a command's own output streams,
//! never arbitrary files.

use crate::{Checkpoint, Computer, ServiceDecl};
use coder_environment::{ImagePin, Provider as ProviderKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One command, identified before it is started. The provider runs a given
/// `id` at most once on a resource: repeating a start with the same `id`
/// never runs the command again, so a lost start reply can be retried
/// safely after a read reports it [`CommandProgress::Absent`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub id: String,
    /// Exact shell text, run with `sh -c`.
    pub command: String,
    /// Source-relative working directory.
    pub cwd: String,
    /// The only computer credentials this command may see, by name. Every
    /// other selected credential is removed from its environment.
    pub credential_names: BTreeSet<String>,
    /// Additional non-secret environment (for example ephemeral Git auth
    /// configuration that names a credential variable, never its value).
    pub env: BTreeMap<String, String>,
    pub timeout_seconds: u64,
    /// Digest of everything above; a read reports it back so a reconciled
    /// command is proven to be this one.
    pub digest: String,
}

/// Bytes of each output stream already delivered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandCursor {
    pub stdout: u64,
    pub stderr: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandProgress {
    /// No command with this identity ever started on the resource.
    Absent,
    Running,
    Exited {
        code: i64,
    },
    /// The process is gone without a recorded exit (machine stopped or
    /// restarted underneath it).
    Lost,
}

/// One read of a command: its state first, then output from the cursor.
/// When `progress` is `Exited`, the bytes are the end of the output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRead {
    pub progress: CommandProgress,
    /// The spec digest the provider retained for this identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Identified commands on a provider resource.
#[allow(async_fn_in_trait)]
pub trait Commands: Provider {
    /// Start `spec` detached; returns the provider operation.
    async fn start_command(
        &self,
        computer: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String>;
    /// Read a command's state and its output from `cursor`, at most
    /// `max_bytes` of each stream.
    async fn read_command(
        &self,
        computer: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead>;
    /// Signal a command and every process it started.
    async fn stop_command(&self, computer: &Computer, resource: &str, id: &str) -> Outcome<String>;
}

/// A provider's readiness for a captured image. Exit codes and request
/// acknowledgements are not readiness: only `Ready` with an immutable
/// snapshot identity is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageState {
    Pending,
    Ready,
    Failed { reason: String },
}

/// One captured output image under an owned name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRecord {
    pub name: String,
    /// The resource the provider captured it from.
    pub source: String,
    pub state: ImageState,
    /// The immutable snapshot behind the name, once known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// Immutable output images for a dedicated builder (ENV-04). There is no
/// replace, rename, or delete: a name, once captured, always means the
/// same snapshot.
#[allow(async_fn_in_trait)]
pub trait Images: Provider {
    /// Capture `resource`'s filesystem under `name`. Implementations read
    /// `name` first: an image of this resource is returned as it is, and
    /// one of another resource is refused, never replaced.
    async fn capture_image(
        &self,
        computer: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord>;
    /// Read an image by name; `None` when the provider has none.
    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>>;
    /// Whether every file a machine restored from its image is on disk.
    /// A machine booted from an image can be usable before that; this, not
    /// a boot or command exit, is restore readiness (ENV-05).
    async fn hydration(&self, computer: &Computer, resource: &str) -> Outcome<bool>;
}

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
    /// Which provider this is. Owners stamp it on the computers they create
    /// and on the image identities they seal.
    fn kind(&self) -> ProviderKind {
        ProviderKind::Boat
    }
    /// Whether a builder on this provider may start from `base`, a recipe's
    /// pinned base image. A recipe pinned to another provider is refused;
    /// a provider that boots a configured base also refuses any other one.
    fn admits_base(&self, base: &ImagePin) -> Result<(), &'static str> {
        if base.provider == self.kind() {
            Ok(())
        } else {
            Err("The recipe's base image belongs to another provider.")
        }
    }
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

/// One provider shared by several owners (setup, build, and verify over
/// the same provider state) through an `Arc`.
impl<P: Provider> Provider for std::sync::Arc<P> {
    fn kind(&self) -> ProviderKind {
        (**self).kind()
    }
    fn admits_base(&self, base: &ImagePin) -> Result<(), &'static str> {
        (**self).admits_base(base)
    }
    async fn create(&self, computer: &Computer, operation: &str) -> Outcome<String> {
        (**self).create(computer, operation).await
    }
    async fn restore(
        &self,
        computer: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        (**self).restore(computer, resource, checkpoint).await
    }
    async fn apply_credentials(&self, computer: &Computer, resource: &str) -> Outcome<String> {
        (**self).apply_credentials(computer, resource).await
    }
    async fn start_service(
        &self,
        computer: &Computer,
        resource: &str,
        service: &ServiceDecl,
    ) -> Outcome<String> {
        (**self).start_service(computer, resource, service).await
    }
    async fn checkpoint(
        &self,
        computer: &Computer,
        resource: &str,
        generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        (**self).checkpoint(computer, resource, generation).await
    }
    async fn shutdown_processes(&self, computer: &Computer, resource: &str) -> Outcome<String> {
        (**self).shutdown_processes(computer, resource).await
    }
    async fn stop(&self, computer: &Computer, resource: &str) -> Outcome<String> {
        (**self).stop(computer, resource).await
    }
    async fn meter(&self, computer: &Computer, resource: &str) -> Outcome<Meter> {
        (**self).meter(computer, resource).await
    }
    async fn delete(&self, computer: &Computer, resource: &str) -> Outcome<String> {
        (**self).delete(computer, resource).await
    }
    async fn inspect(&self, computer: &Computer, resource: &str) -> Outcome<Inspection> {
        (**self).inspect(computer, resource).await
    }
}
impl<P: Commands> Commands for std::sync::Arc<P> {
    async fn start_command(
        &self,
        computer: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        (**self).start_command(computer, resource, spec).await
    }
    async fn read_command(
        &self,
        computer: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        (**self)
            .read_command(computer, resource, id, cursor, max_bytes)
            .await
    }
    async fn stop_command(&self, computer: &Computer, resource: &str, id: &str) -> Outcome<String> {
        (**self).stop_command(computer, resource, id).await
    }
}
impl<P: Images> Images for std::sync::Arc<P> {
    async fn capture_image(
        &self,
        computer: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord> {
        (**self).capture_image(computer, resource, name).await
    }
    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        (**self).read_image(name).await
    }
    async fn hydration(&self, computer: &Computer, resource: &str) -> Outcome<bool> {
        (**self).hydration(computer, resource).await
    }
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
        /// Identified commands by id.
        pub processes: BTreeMap<String, FakeProcess>,
        /// The output image this machine booted from.
        pub image: Option<String>,
        /// Restored files are still copying in.
        pub hydrating: bool,
    }

    /// One identified command on a fake machine.
    #[derive(Clone, Debug, Default)]
    pub struct FakeProcess {
        pub spec: Option<CommandSpec>,
        /// The process environment the command saw.
        pub env: BTreeMap<String, String>,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub exit: Option<i64>,
        pub lost: bool,
        /// How many times the command body actually ran.
        pub runs: u32,
    }

    /// What a scripted command does when it runs.
    #[derive(Clone, Debug, Default)]
    pub struct FakeRun {
        pub stdout: String,
        pub stderr: String,
        /// `None` keeps it running until [`FakeProvider::finish_command`]
        /// or a stop.
        pub exit: Option<i64>,
    }
    impl FakeRun {
        pub fn exit(code: i64, stdout: &str, stderr: &str) -> Self {
            Self {
                stdout: stdout.into(),
                stderr: stderr.into(),
                exit: Some(code),
            }
        }
    }

    /// Scripts command behavior: the spec, the environment the command
    /// sees, and the machine's files (which it may change).
    pub type Handler = Box<
        dyn Fn(&CommandSpec, &BTreeMap<String, String>, &mut BTreeMap<String, String>) -> FakeRun
            + Send
            + Sync,
    >;

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
        /// Captured images by name, with the files they hold.
        pub images: BTreeMap<String, (ImageRecord, BTreeMap<String, String>)>,
        /// Newly captured images report `Pending` until
        /// [`FakeProvider::settle_images`].
        pub images_pending: bool,
        /// Machines booted from an image report un-hydrated until
        /// [`FakeProvider::settle_hydration`].
        pub hydration_pending: bool,
        pub counter: u64,
    }

    pub struct FakeProvider {
        pub state: Mutex<State>,
        /// Values the operator's credential custody would supply.
        pub credential_values: BTreeMap<String, String>,
        /// Whether a checkpoint stops the resource, like Boat.
        pub checkpoint_stops: bool,
        handler: Mutex<Option<Handler>>,
    }

    impl FakeProvider {
        pub fn new(credential_values: BTreeMap<String, String>, checkpoint_stops: bool) -> Self {
            Self {
                state: Mutex::new(State::default()),
                credential_values,
                checkpoint_stops,
                handler: Mutex::new(None),
            }
        }
        /// Script what identified commands do.
        pub fn on_command(&self, handler: Handler) {
            *self.handler.lock().unwrap() = Some(handler);
        }
        /// Finish a still-running command with more output.
        pub fn finish_command(&self, resource: &str, id: &str, code: i64, stdout: &str) {
            let mut s = self.state.lock().unwrap();
            let p = s
                .machines
                .get_mut(resource)
                .and_then(|m| m.processes.get_mut(id))
                .expect("process");
            p.stdout.extend_from_slice(stdout.as_bytes());
            p.exit = Some(code);
        }
        pub fn process(&self, resource: &str, id: &str) -> Option<FakeProcess> {
            self.state
                .lock()
                .unwrap()
                .machines
                .get(resource)
                .and_then(|m| m.processes.get(id))
                .cloned()
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
        /// Files held by a captured image.
        pub fn image_files(&self, name: &str) -> Option<BTreeMap<String, String>> {
            self.state
                .lock()
                .unwrap()
                .images
                .get(name)
                .map(|(_, f)| f.clone())
        }
        /// Mark every machine's restored files hydrated.
        pub fn settle_hydration(&self) {
            for m in self.state.lock().unwrap().machines.values_mut() {
                m.hydrating = false;
            }
        }
        /// Mark every pending image ready.
        pub fn settle_images(&self) {
            for (record, _) in self.state.lock().unwrap().images.values_mut() {
                if record.state == ImageState::Pending {
                    record.state = ImageState::Ready;
                }
            }
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
        async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
            let inject = self.begin("create");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let mut s = self.state.lock().unwrap();
            let resource = if let Some(r) = s.operations.get(operation) {
                r.clone()
            } else {
                // A verifier boots from exactly its sealed image.
                let files = match c.purpose.verify_image() {
                    None => BTreeMap::new(),
                    Some(name) => match s.images.get(name) {
                        Some((record, files)) if record.state == ImageState::Ready => files.clone(),
                        _ => return Outcome::failed("no such ready image"),
                    },
                };
                let r = Self::next_id(&mut s, "box");
                s.operations.insert(operation.into(), r.clone());
                let image = c.purpose.verify_image().map(str::to_owned);
                let hydrating = image.is_some() && s.hydration_pending;
                s.machines.insert(
                    r.clone(),
                    Machine {
                        running: true,
                        meter_running: true,
                        files,
                        image,
                        hydrating,
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
                // Processes do not survive a stop.
                for p in m.processes.values_mut().filter(|p| p.exit.is_none()) {
                    p.lost = true;
                }
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

    impl Commands for FakeProvider {
        async fn start_command(
            &self,
            _c: &Computer,
            resource: &str,
            spec: &CommandSpec,
        ) -> Outcome<String> {
            let inject = self.begin("command_start");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let handler = self.handler.lock().unwrap();
            let out = self.with_machine(resource, |s, r| {
                let m = s.machines.get_mut(r).unwrap();
                if !m.running {
                    return Outcome::failed("not running");
                }
                if m.processes.contains_key(&spec.id) {
                    // At most once per identity.
                    return Outcome::done(format!("process:{}", spec.id));
                }
                let mut env: BTreeMap<String, String> = m
                    .env
                    .iter()
                    .filter(|(k, _)| spec.credential_names.contains(*k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                env.extend(spec.env.clone());
                let run = match handler.as_ref() {
                    Some(h) => h(spec, &env, &mut m.files),
                    None => FakeRun::exit(0, "", ""),
                };
                m.processes.insert(
                    spec.id.clone(),
                    FakeProcess {
                        spec: Some(spec.clone()),
                        env,
                        stdout: run.stdout.into_bytes(),
                        stderr: run.stderr.into_bytes(),
                        exit: run.exit,
                        lost: false,
                        runs: 1,
                    },
                );
                Outcome::done(format!("process:{}", spec.id))
            });
            drop(handler);
            Self::finish(inject, || out)
        }
        async fn read_command(
            &self,
            _c: &Computer,
            resource: &str,
            id: &str,
            cursor: CommandCursor,
            max_bytes: u64,
        ) -> Outcome<CommandRead> {
            let inject = self.begin("command_read");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = self.with_machine(resource, |s, r| {
                let m = &s.machines[r];
                let Some(p) = m.processes.get(id) else {
                    return Outcome::done(CommandRead {
                        progress: CommandProgress::Absent,
                        digest: None,
                        stdout: vec![],
                        stderr: vec![],
                    });
                };
                let slice = |bytes: &[u8], from: u64| {
                    let from = (from as usize).min(bytes.len());
                    let to = (from + max_bytes as usize).min(bytes.len());
                    bytes[from..to].to_vec()
                };
                let progress = match p.exit {
                    Some(code) => CommandProgress::Exited { code },
                    None if p.lost || !m.running => CommandProgress::Lost,
                    None => CommandProgress::Running,
                };
                Outcome::done(CommandRead {
                    progress,
                    digest: p.spec.as_ref().map(|s| s.digest.clone()),
                    stdout: slice(&p.stdout, cursor.stdout),
                    stderr: slice(&p.stderr, cursor.stderr),
                })
            });
            Self::finish(inject, || out)
        }
        async fn stop_command(&self, _c: &Computer, resource: &str, id: &str) -> Outcome<String> {
            let inject = self.begin("command_stop");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = self.with_machine(resource, |s, r| {
                match s.machines.get_mut(r).unwrap().processes.get_mut(id) {
                    None => Outcome::done("absent".into()),
                    Some(p) if p.exit.is_some() => Outcome::done("exited".into()),
                    Some(p) => {
                        p.exit = Some(143);
                        Outcome::done("stopped".into())
                    }
                }
            });
            Self::finish(inject, || out)
        }
    }

    impl Images for FakeProvider {
        async fn capture_image(
            &self,
            _c: &Computer,
            resource: &str,
            name: &str,
        ) -> Outcome<ImageRecord> {
            let inject = self.begin("capture_image");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let mut s = self.state.lock().unwrap();
            let out = if let Some((record, _)) = s.images.get(name) {
                if record.source == resource {
                    Outcome::done(record.clone())
                } else {
                    Outcome::failed("the image name belongs to another source")
                }
            } else if let Some(m) = s.machines.get(resource) {
                let files = m.files.clone();
                let snapshot = Self::next_id(&mut s, "image-snap");
                let record = ImageRecord {
                    name: name.into(),
                    source: resource.into(),
                    state: if s.images_pending {
                        ImageState::Pending
                    } else {
                        ImageState::Ready
                    },
                    snapshot: Some(snapshot),
                    size_bytes: Some(files.values().map(|v| v.len() as u64).sum()),
                };
                s.images.insert(name.into(), (record.clone(), files));
                Outcome::done(record)
            } else {
                Outcome::failed("no such resource")
            };
            drop(s);
            Self::finish(inject, || out)
        }
        async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
            let inject = self.begin("read_image");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = Outcome::done(
                self.state
                    .lock()
                    .unwrap()
                    .images
                    .get(name)
                    .map(|(r, _)| r.clone()),
            );
            Self::finish(inject, || out)
        }
        async fn hydration(&self, _c: &Computer, resource: &str) -> Outcome<bool> {
            let inject = self.begin("hydration");
            if matches!(inject, Some(Inject::Failed | Inject::Unknown)) {
                return Self::finish(inject, || unreachable_outcome());
            }
            let out = self.with_machine(resource, |s, r| {
                let m = &s.machines[r];
                if !m.running {
                    return Outcome::failed("not running");
                }
                Outcome::done(!m.hydrating)
            });
            Self::finish(inject, || out)
        }
    }

    fn unreachable_outcome<T>() -> Outcome<T> {
        Outcome::failed("not performed")
    }
}
