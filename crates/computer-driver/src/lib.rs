//! The one driver contract for every computer (#11624).
//!
//! A driver maps a small set of lifecycle operations onto one backend: our
//! GCE sandboxes through `oa-boat` ([`boat::BoatDriver`], feature `boat`),
//! an in-memory machine for tests ([`fake::FakeDriver`]), and later a
//! person's own Mac, a Pylon contributor, or a Kubernetes sandbox. The
//! lifecycle layer (`coder-working-computer`'s decide/apply, run by
//! `oa-computers`) never learns which.
//!
//! Every driver declares what it supports as [`Profile`]s in its
//! [`Capabilities`]:
//!
//! - **Core** (required): create under an idempotent operation identity,
//!   observe, run a command, put and get a file, meter, delete.
//! - **Suspend**: stop and resume the same computer, keeping its disk.
//! - **Snapshot**: save a computer's disk under a name and create a new
//!   computer from it.
//! - **Fork**: copy a computer into a new one.
//!
//! A caller checks a request against the declared profiles with [`admit`]
//! *before* it writes any state, so an unsupported request is a clean
//! refusal and never a half-built computer. Drivers also refuse an
//! undeclared operation themselves with a definite [`Outcome::Failed`]
//! (the default trait bodies), so a caller that skipped [`admit`] still
//! changes nothing. `computer-conformance` checks both, black-box.
//!
//! Effects report a typed [`Outcome`]: `Done`, `Failed` (definitely did not
//! happen), or `Unknown` (the reply was lost). Unknown stays unknown until a
//! definite observation or an idempotent repeat settles it.
//!
//! Secrets: [`Exec::secret_env`] carries a credential for one command only.
//! Drivers pass it to that process's environment and keep it nowhere else:
//! not in a file, not in the computer's per-boot environment, not in a
//! snapshot. The value is zeroed when the request is dropped.

pub mod identified;
mod storage;
pub use storage::*;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[cfg(feature = "boat")]
pub mod boat;
pub mod fake;
#[cfg(feature = "space")]
pub mod space;

/// The `GET /capabilities` document's schema.
pub const CAPABILITIES_SCHEMA: &str = "openagents.computer_driver.capabilities.v1";

/// What the reason of a refused, undeclared operation starts with.
pub const UNSUPPORTED: &str = "unsupported:";

// ---------------------------------------------------------------------
// Profiles and capabilities.

fn is_false(value: &bool) -> bool {
    !*value
}

/// An image reference belongs to one provider's image format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    GceImage,
    SpaceTemplate,
    Test,
}

/// A resolved base image. When present, `provider_identity` is the image ID
/// or template digest reported by the provider. A numeric ID is not a content hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gib: Option<u64>,
}
impl Artifact {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.reference.is_empty()
            || self.reference.len() > 2048
            || self.reference.contains(['?', '#'])
            || self
                .reference
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            || self.provider_identity.as_deref().is_some_and(|value| {
                value.is_empty() || value.len() > 256 || value.chars().any(char::is_control)
            })
        {
            return Err("The image reference is invalid.");
        }
        if self.kind == ArtifactKind::SpaceTemplate {
            let digest = self.reference.strip_prefix("sha256:").or_else(|| {
                self.reference
                    .rsplit_once("@sha256:")
                    .map(|(_, digest)| digest)
            });
            if !digest.is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
            }) {
                return Err("Use a template pinned by its SHA-256 digest.");
            }
        }
        Ok(())
    }
}

/// The resources behind a size class, independent of a provider's machine name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    pub vcpus: u32,
    pub memory_mib: u64,
    /// Images may require a larger disk. This is the declared lower bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_disk_gib: Option<u64>,
}

/// One conformance profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Core,
    Suspend,
    Snapshot,
    Fork,
}
impl Profile {
    pub const ALL: [Profile; 4] = [Self::Core, Self::Suspend, Self::Snapshot, Self::Fork];
    pub fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Suspend => "suspend",
            Self::Snapshot => "snapshot",
            Self::Fork => "fork",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == s)
    }
}
impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What a driver declares. Served as JSON at `GET /capabilities` by an
/// out-of-process driver (oa-boat), returned in process by the others.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub schema: String,
    /// The driver's stable name (`oa-boat`, `fake`, …).
    pub driver: String,
    pub profiles: BTreeSet<Profile>,
    /// Size classes a create may name; the first is the default.
    #[serde(default)]
    pub sizes: Vec<String>,
    /// Maximum bytes for protected workspace transfers, when supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_file_bytes: Option<usize>,
    /// Whether Suspend preserves running processes, beyond the disk contents.
    #[serde(default, skip_serializing_if = "is_false")]
    pub memory_suspend: bool,
    /// Create attaches an existing workspace; delete and lifecycle expiry detach it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub workspace_attachment: bool,
    /// Start, observe, and cancel a command under one retained command ID.
    #[serde(default, skip_serializing_if = "is_false")]
    pub command_sessions: bool,
    /// Clean builders can publish a reusable disk image and freshly verify it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub environment_artifacts: bool,
    /// Resolved provider default, so a new allocation need not follow a mutable family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shapes: std::collections::BTreeMap<String, Shape>,
}
impl Capabilities {
    pub fn new(driver: &str, profiles: impl IntoIterator<Item = Profile>, sizes: &[&str]) -> Self {
        let mut profiles: BTreeSet<Profile> = profiles.into_iter().collect();
        profiles.insert(Profile::Core);
        Self {
            memory_suspend: false,
            workspace_attachment: false,
            command_sessions: false,
            environment_artifacts: false,
            default_artifact: None,
            shapes: Default::default(),
            workspace_file_bytes: None,
            schema: CAPABILITIES_SCHEMA.into(),
            driver: driver.into(),
            profiles,
            sizes: sizes.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
    pub fn supports(&self, profile: Profile) -> bool {
        self.profiles.contains(&profile)
    }
    /// The declaration is usable: our schema, a name, and Core.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != CAPABILITIES_SCHEMA {
            return Err("The capabilities schema is not supported.");
        }
        if self.driver.is_empty() || self.driver.len() > 64 {
            return Err("A driver needs a name.");
        }
        if !self.supports(Profile::Core) {
            return Err("Every driver must support the Core profile.");
        }
        if self.workspace_attachment
            && (self.profiles != BTreeSet::from([Profile::Core])
                || self.memory_suspend
                || self.environment_artifacts
                || self.default_artifact.is_some()
                || !self.shapes.is_empty())
        {
            return Err(
                "Workspace connections support only Core operations without cloud images or sizes.",
            );
        }
        if self.memory_suspend && !self.supports(Profile::Suspend) {
            return Err("Keeping processes requires Suspend support.");
        }
        if let Some(artifact) = &self.default_artifact {
            artifact.validate()?;
        }
        if self.shapes.iter().any(|(name, shape)| {
            (!self.sizes.is_empty() && !self.sizes.contains(name))
                || shape.vcpus == 0
                || shape.memory_mib == 0
                || shape.min_disk_gib == Some(0)
        }) {
            return Err("The size details are invalid.");
        }
        Ok(())
    }
}

/// A request needs a profile the driver did not declare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    pub driver: String,
    pub profile: Profile,
}
impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "This computer can't {} (its driver, {}, doesn't support {}).",
            match self.profile {
                Profile::Core => "run",
                Profile::Suspend => "be stopped and resumed",
                Profile::Snapshot => "be saved and restored",
                Profile::Fork => "be copied",
            },
            self.driver,
            self.profile
        )
    }
}
impl std::error::Error for Unsupported {}

/// Check every profile a request needs before any state is written.
pub fn admit(caps: &Capabilities, needs: &[Profile]) -> Result<(), Unsupported> {
    match needs.iter().find(|p| !caps.supports(**p)) {
        Some(p) => Err(Unsupported {
            driver: caps.driver.clone(),
            profile: *p,
        }),
        None => Ok(()),
    }
}

/// The definite refusal a driver returns for an operation it did not
/// declare. Nothing was changed.
pub fn unsupported<T>(profile: Profile) -> Outcome<T> {
    Outcome::failed(format!(
        "{UNSUPPORTED} this driver does not support {profile}"
    ))
}

// ---------------------------------------------------------------------
// Outcomes and observations (shared with `coder-working-computer`).

/// The result of one driver effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome<T> {
    Done {
        value: T,
    },
    /// The driver definitely did not perform the effect.
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
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done { .. })
    }
    pub fn value(self) -> Option<T> {
        match self {
            Self::Done { value } => Some(value),
            _ => None,
        }
    }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Outcome<U> {
        match self {
            Self::Done { value } => Outcome::Done { value: f(value) },
            Self::Failed { reason } => Outcome::Failed { reason },
            Self::Unknown { reason } => Outcome::Unknown { reason },
        }
    }
    /// The refusal of an undeclared operation.
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Failed { reason } if reason.starts_with(UNSUPPORTED))
    }
}

/// A completed filesystem checkpoint: snapshot identity only, never file
/// contents or credential values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointEvidence {
    pub snapshot: String,
    /// The stop operation when the checkpoint quiesced by stopping the
    /// computer (Boat). `None` leaves it running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meter {
    pub running: bool,
    pub evidence: String,
    /// Seconds the backend counts as run, when it can say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u64>,
}

/// A read-only view used to reconcile an unknown outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    /// `None` when the backend reports the computer gone.
    pub running: Option<bool>,
    /// The completed stop operation, if the backend reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_snapshot: Option<String>,
}

/// One command, identified before it is started. A driver runs a given
/// `id` at most once on a computer.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub id: String,
    /// Exact shell text, run with `sh -c`.
    pub command: String,
    /// Source-relative working directory.
    pub cwd: String,
    /// The only computer credentials this command may see, by name.
    pub credential_names: BTreeSet<String>,
    /// Additional non-secret environment.
    pub env: BTreeMap<String, String>,
    pub timeout_seconds: u64,
    /// Digest of everything above.
    pub digest: String,
}
impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSpec")
            .field("id", &self.id)
            .field("credential_names", &self.credential_names)
            .finish_non_exhaustive()
    }
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
    /// No command with this identity ever started.
    Absent,
    Running,
    Exited {
        code: i64,
    },
    /// The process is gone without a recorded exit.
    Lost,
}

/// One read of a command: its state first, then output from the cursor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRead {
    /// The wrapper observed its deadline, rather than a shell exit with that code.
    #[serde(default)]
    pub timed_out: bool,
    pub progress: CommandProgress,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A captured image's readiness.
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
    pub source: String,
    pub state: ImageState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

// ---------------------------------------------------------------------
// The driver-level requests.

/// Create (or, under the same `operation`, find) one computer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRequest {
    /// The idempotency identity: repeating it returns the same computer.
    pub operation: String,
    /// A size class from [`Capabilities::sizes`]; `None` takes the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// The immutable image (an Environment build or a saved snapshot) to
    /// start from; `None` takes the driver's base image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Hard lifetime, after which the backend may stop or delete it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Non-secret labels the driver may record (owner digest, chat).
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

/// Where a computer stands as the backend sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Created and booting; commands are not accepted yet.
    Starting,
    Running,
    Stopping,
    Stopped,
    /// Deleted, or never existed.
    Gone,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A credential value that zeroes itself when dropped and never prints.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

/// Bounded output observed before a command finishes or its connection is lost.
#[derive(Clone, Default)]
pub struct OutputCapture(std::sync::Arc<std::sync::Mutex<(Vec<u8>, Vec<u8>)>>);

impl fmt::Debug for OutputCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OutputCapture(…)")
    }
}
impl OutputCapture {
    pub fn append(&self, stderr: bool, bytes: &[u8]) {
        let mut streams = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let stream = if stderr {
            &mut streams.1
        } else {
            &mut streams.0
        };
        stream.extend(
            bytes
                .iter()
                .take((256 * 1024usize).saturating_sub(stream.len())),
        );
    }
    pub fn read(&self) -> (Vec<u8>, Vec<u8>) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

/// One synchronous command.
#[derive(Clone, Debug, Default)]
pub struct Exec {
    pub command: String,
    /// Absolute, or relative to the computer's home.
    pub cwd: Option<String>,
    pub timeout_seconds: u64,
    /// Environment for this command only. Never written to a file.
    pub secret_env: BTreeMap<String, Secret>,
    pub output: OutputCapture,
}
impl Exec {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            cwd: None,
            timeout_seconds: 120,
            secret_env: BTreeMap::new(),
            output: OutputCapture::default(),
        }
    }
    pub fn secret(mut self, name: &str, value: &str) -> Self {
        self.secret_env.insert(name.into(), Secret::new(value));
        self
    }
}

/// Whether `name` may be an environment variable name.
pub fn env_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !name.as_bytes()[0].is_ascii_digit()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecOutput {
    pub code: i64,
    pub stdout: String,
    pub stderr: String,
}

// ---------------------------------------------------------------------
// The contract.

/// One backend's lifecycle operations. Each call performs exactly one
/// effect and reports what it observed; drivers decide nothing.
///
/// Optional-profile operations default to the [`unsupported`] refusal, so a
/// driver that declares only Core implements only the Core methods.
#[allow(async_fn_in_trait)]
pub trait Driver {
    fn capabilities(&self) -> Capabilities;

    /// Publish only a dedicated clean builder. Callers enforce the attempt's
    /// capture stage; implementations refuse a name owned by another resource.
    async fn capture_environment_image(
        &self,
        _computer: &str,
        _name: &str,
    ) -> Outcome<ImageRecord> {
        Outcome::failed("Environment images aren't supported on this computer.")
    }
    async fn read_environment_image(&self, _name: &str) -> Outcome<Option<ImageRecord>> {
        Outcome::failed("Environment images aren't supported on this computer.")
    }
    async fn delete_environment_image(&self, _name: &str) -> Outcome<bool> {
        Outcome::failed("Environment images aren't supported on this computer.")
    }
    async fn environment_image_hydrated(&self, _computer: &str) -> Outcome<bool> {
        Outcome::failed("Environment images aren't supported on this computer.")
    }

    // Core.
    /// Create under `request.operation`; repeating the same operation must
    /// return the same computer, never a second one.
    async fn create(&self, request: &CreateRequest) -> Outcome<String>;
    async fn observe(&self, computer: &str) -> Outcome<Observation>;
    async fn exec(&self, computer: &str, exec: &Exec) -> Outcome<ExecOutput>;
    /// Start one identity-bound command. Retry by observation, never by a new ID.
    async fn start_command(&self, _computer: &str, _spec: &CommandSpec) -> Outcome<String> {
        Outcome::failed("identified commands are unsupported")
    }
    async fn read_command(
        &self,
        _computer: &str,
        _id: &str,
        _cursor: CommandCursor,
        _max_bytes: u64,
    ) -> Outcome<CommandRead> {
        Outcome::failed("identified commands are unsupported")
    }
    async fn stop_command(&self, _computer: &str, _id: &str) -> Outcome<String> {
        Outcome::failed("identified commands are unsupported")
    }
    /// Protected workspace transfers are distinct from legacy unrestricted files.
    async fn read_workspace_file(
        &self,
        _computer: &str,
        _path: &str,
    ) -> Result<Vec<u8>, computer_files::FileError> {
        Err(computer_files::FileError::Unsupported)
    }
    async fn write_workspace_file(
        &self,
        _computer: &str,
        _path: &str,
        _bytes: &[u8],
        _overwrite: bool,
    ) -> Result<computer_files::FileInfo, computer_files::FileError> {
        Err(computer_files::FileError::Unsupported)
    }
    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()>;
    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>>;
    async fn meter(&self, computer: &str) -> Outcome<Meter>;
    /// Observe private storage without inferring bytes or cleanup from VM state.
    async fn storage(&self, _computer: &str) -> Outcome<StorageObservation> {
        Outcome::unknown("Storage information isn't available.")
    }
    /// Delete; deleting a deleted computer is `Done`.
    async fn delete(&self, computer: &str) -> Outcome<()>;

    // Suspend.
    async fn suspend(&self, _computer: &str) -> Outcome<String> {
        unsupported(Profile::Suspend)
    }
    async fn resume(&self, _computer: &str) -> Outcome<String> {
        unsupported(Profile::Suspend)
    }

    /// Repeat a lifecycle mutation under the same durable identity. Drivers
    /// with asynchronous teardown must await that operation before returning.
    async fn suspend_operation(&self, computer: &str, _operation: &str) -> Outcome<String> {
        self.suspend(computer).await
    }
    async fn resume_operation(&self, computer: &str, _operation: &str) -> Outcome<String> {
        self.resume(computer).await
    }

    // Snapshot.
    /// Save the computer's disk as `name` (it may need to be stopped
    /// first; drivers say so with `Failed`).
    async fn snapshot(&self, _computer: &str, _name: &str) -> Outcome<String> {
        unsupported(Profile::Snapshot)
    }
    /// Whether a saved snapshot is ready to create from.
    async fn snapshot_ready(&self, _name: &str) -> Outcome<bool> {
        unsupported(Profile::Snapshot)
    }
    async fn delete_snapshot(&self, _name: &str) -> Outcome<()> {
        unsupported(Profile::Snapshot)
    }

    // Fork.
    /// Copy `computer` into a new one under `operation` (idempotent).
    async fn fork(&self, _computer: &str, _operation: &str) -> Outcome<String> {
        unsupported(Profile::Fork)
    }
}

impl<D: Driver> Driver for std::sync::Arc<D> {
    async fn capture_environment_image(&self, c: &str, name: &str) -> Outcome<ImageRecord> {
        (**self).capture_environment_image(c, name).await
    }
    async fn read_environment_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        (**self).read_environment_image(name).await
    }
    async fn delete_environment_image(&self, name: &str) -> Outcome<bool> {
        (**self).delete_environment_image(name).await
    }
    async fn environment_image_hydrated(&self, c: &str) -> Outcome<bool> {
        (**self).environment_image_hydrated(c).await
    }

    fn capabilities(&self) -> Capabilities {
        (**self).capabilities()
    }
    async fn create(&self, r: &CreateRequest) -> Outcome<String> {
        (**self).create(r).await
    }
    async fn observe(&self, c: &str) -> Outcome<Observation> {
        (**self).observe(c).await
    }
    async fn exec(&self, c: &str, e: &Exec) -> Outcome<ExecOutput> {
        (**self).exec(c, e).await
    }
    async fn start_command(&self, c: &str, s: &CommandSpec) -> Outcome<String> {
        (**self).start_command(c, s).await
    }
    async fn read_command(
        &self,
        c: &str,
        id: &str,
        cursor: CommandCursor,
        max: u64,
    ) -> Outcome<CommandRead> {
        (**self).read_command(c, id, cursor, max).await
    }
    async fn stop_command(&self, c: &str, id: &str) -> Outcome<String> {
        (**self).stop_command(c, id).await
    }
    async fn read_workspace_file(
        &self,
        c: &str,
        p: &str,
    ) -> Result<Vec<u8>, computer_files::FileError> {
        (**self).read_workspace_file(c, p).await
    }
    async fn write_workspace_file(
        &self,
        c: &str,
        p: &str,
        b: &[u8],
        overwrite: bool,
    ) -> Result<computer_files::FileInfo, computer_files::FileError> {
        (**self).write_workspace_file(c, p, b, overwrite).await
    }
    async fn put_file(&self, c: &str, p: &str, b: &[u8]) -> Outcome<()> {
        (**self).put_file(c, p, b).await
    }
    async fn get_file(&self, c: &str, p: &str) -> Outcome<Vec<u8>> {
        (**self).get_file(c, p).await
    }
    async fn meter(&self, c: &str) -> Outcome<Meter> {
        (**self).meter(c).await
    }
    async fn storage(&self, c: &str) -> Outcome<StorageObservation> {
        (**self).storage(c).await
    }
    async fn delete(&self, c: &str) -> Outcome<()> {
        (**self).delete(c).await
    }
    async fn suspend(&self, c: &str) -> Outcome<String> {
        (**self).suspend(c).await
    }
    async fn resume(&self, c: &str) -> Outcome<String> {
        (**self).resume(c).await
    }
    async fn suspend_operation(&self, c: &str, operation: &str) -> Outcome<String> {
        (**self).suspend_operation(c, operation).await
    }
    async fn resume_operation(&self, c: &str, operation: &str) -> Outcome<String> {
        (**self).resume_operation(c, operation).await
    }
    async fn snapshot(&self, c: &str, n: &str) -> Outcome<String> {
        (**self).snapshot(c, n).await
    }
    async fn snapshot_ready(&self, n: &str) -> Outcome<bool> {
        (**self).snapshot_ready(n).await
    }
    async fn delete_snapshot(&self, n: &str) -> Outcome<()> {
        (**self).delete_snapshot(n).await
    }
    async fn fork(&self, c: &str, o: &str) -> Outcome<String> {
        (**self).fork(c, o).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_attachments_cannot_advertise_cloud_lifecycle_or_resources() {
        let mut caps = Capabilities::new("enrolled", [], &["workspace"]);
        caps.workspace_attachment = true;
        assert!(caps.validate().is_ok());
        let mut cloud = caps.clone();
        cloud.profiles.insert(Profile::Suspend);
        assert!(cloud.validate().is_err());
        let mut cloud = caps.clone();
        cloud.default_artifact = Some(Artifact {
            kind: ArtifactKind::GceImage,
            reference: "cloud-image".into(),
            provider_identity: None,
            disk_gib: None,
        });
        assert!(cloud.validate().is_err());
        let mut cloud = caps;
        cloud.shapes.insert(
            "workspace".into(),
            Shape {
                vcpus: 1,
                memory_mib: 1024,
                min_disk_gib: None,
            },
        );
        assert!(cloud.validate().is_err());
    }

    #[test]
    fn admit_refuses_an_undeclared_profile_by_name() {
        let caps = Capabilities::new("x", [Profile::Suspend], &["small"]);
        assert!(caps.validate().is_ok());
        assert!(admit(&caps, &[Profile::Core, Profile::Suspend]).is_ok());
        let no = admit(&caps, &[Profile::Fork]).unwrap_err();
        assert_eq!(no.profile, Profile::Fork);
        assert!(no.to_string().contains("copied"), "{no}");
    }

    #[test]
    fn capabilities_round_trip_and_always_hold_core() {
        let caps = Capabilities::new("oa-boat", Profile::ALL, &["default"]);
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(
            json["profiles"],
            serde_json::json!(["core", "suspend", "snapshot", "fork"])
        );
        let back: Capabilities = serde_json::from_value(json).unwrap();
        assert_eq!(back, caps);
        let none = Capabilities::new("bare", [], &[]);
        assert!(none.supports(Profile::Core));
    }

    #[test]
    fn old_declarations_do_not_claim_images_shapes_or_memory_preservation() {
        let old: Capabilities = serde_json::from_value(serde_json::json!({
            "schema":CAPABILITIES_SCHEMA,"driver":"old","profiles":["core","suspend"],"sizes":["small"]
        })).unwrap();
        assert!(old.validate().is_ok());
        assert!(!old.memory_suspend && !old.command_sessions);
        assert!(old.shapes.is_empty() && old.default_artifact.is_none());
        let mut malformed = old.clone();
        malformed.memory_suspend = true;
        malformed.profiles.remove(&Profile::Suspend);
        assert!(malformed.validate().is_err());
        let mut malformed = old;
        malformed.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: 0,
                min_disk_gib: None,
            },
        );
        assert!(malformed.validate().is_err());
    }

    #[test]
    fn a_gce_image_cannot_be_admitted_as_a_pinned_space_template() {
        let mut artifact = Artifact {
            kind: ArtifactKind::SpaceTemplate,
            reference: "projects/p/global/images/i".into(),
            provider_identity: None,
            disk_gib: None,
        };
        assert!(artifact.validate().is_err());
        artifact.reference = format!("registry.example/template@sha256:{}", "a".repeat(64));
        assert!(artifact.validate().is_ok());
        artifact.reference.push_str("?credential=private-fixture");
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn a_secret_never_prints() {
        let e = Exec::new("true").secret("TOKEN", "sk-ant-oat01-abc");
        assert!(!format!("{e:?}").contains("abc"));
    }
}

/// Immutable authority for one clean Environment computer attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentRole {
    Setup,
    Builder,
    Verifier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentAttempt {
    pub environment: String,
    pub attempt: String,
    pub role: EnvironmentRole,
    pub source_digest: String,
    pub recipe_digest: String,
    pub inputs_digest: String,
    pub deadline_ms: u64,
    pub idle_ms: u64,
    pub image_name: Option<String>,
}
impl EnvironmentAttempt {
    pub fn validate(&self) -> Result<(), &'static str> {
        let id = |s: &str| {
            s.len() <= 128
                && !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        };
        let digest = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if !id(&self.environment)
            || !id(&self.attempt)
            || !digest(&self.source_digest)
            || !digest(&self.recipe_digest)
            || !digest(&self.inputs_digest)
            || self.deadline_ms == 0
            || self.idle_ms == 0
            || self.image_name.as_deref().is_some_and(|n| !id(n))
            || self.role == EnvironmentRole::Builder && self.image_name.is_none()
        {
            return Err("The Environment attempt is invalid.");
        }
        Ok(())
    }
}
