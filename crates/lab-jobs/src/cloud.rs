//! One computer per bounded cloud worker, reached only through the public SDK.
//! Save this launcher's private state directory until cleanup finishes. A lost
//! response keeps the same request; a saved command is observed without rerun.
use crate::{Capabilities, EffectiveRequirements, Placement, authority::Enrolled, worker};
use fs2::FileExt;
use openagents_sdk::{
    CancellationToken, Client, Session,
    computers::{
        CommandOptions, CommandRef, CommandRequest, ComputerRef, CreateOptions, CreateRequest,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// Operator-selected image and installed worker. Jobs cannot choose these values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub environment: String,
    pub version: String,
    pub size: String,
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub workspace: PathBuf,
    pub handlers: BTreeMap<String, worker::Handler>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Check the cloud worker's settings and job key.")]
    Settings,
    #[error("Keep the cloud worker's state in a private directory.")]
    Storage,
    #[error("This cloud worker is already being managed.")]
    Busy,
    #[error("{0}")]
    Computers(#[from] openagents_sdk::computers::Error),
}
type Result<T> = std::result::Result<T, Error>;

/// Progress describes an observation, not a job result or a provider charge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Creating,
    Starting,
    Running,
    Cleaning,
    Finished { command_state: Option<String> },
}

#[derive(Clone, Serialize, Deserialize)]
enum Stage {
    Prepared,
    Creating,
    Waiting,
    Starting(CommandRequest),
    Running(CommandRef),
    Deleting { sent: bool },
    Finished,
}
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    schema: String,
    create: CreateRequest,
    computer: Option<ComputerRef>,
    bootstrap: Option<String>,
    command_state: Option<String>,
    cleanup_requested: bool,
    stage: Stage,
    deadline_unix_ms: u64,
    command_key: String,
}

/// An exclusive lock covers state updates and SDK requests, including restart.
/// Debug and errors omit the saved command and the guest's scoped credential.
pub struct Launcher {
    directory: PathBuf,
    saved: Saved,
    _lock: File,
}
impl std::fmt::Debug for Launcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudLauncher").finish_non_exhaustive()
    }
}

fn private_directory(directory: &Path) -> Result<()> {
    if !directory.exists() {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory).map_err(|_| Error::Storage)?;
    }
    let metadata = fs::symlink_metadata(directory).map_err(|_| Error::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Error::Storage);
        }
    }
    Ok(())
}
fn lock(directory: &Path) -> Result<File> {
    private_directory(directory)?;
    let path = directory.join("lock");
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Storage);
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|_| Error::Storage)?;
    file.try_lock_exclusive().map_err(|_| Error::Busy)?;
    Ok(file)
}
fn write(directory: &Path, saved: &Saved) -> Result<()> {
    let bytes = serde_json::to_vec(saved).map_err(|_| Error::Storage)?;
    let pending = directory.join("state.pending");
    if pending.exists() {
        fs::remove_file(&pending).map_err(|_| Error::Storage)?;
    }
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&pending).map_err(|_| Error::Storage)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::Storage)?;
    fs::rename(pending, directory.join("state.json")).map_err(|_| Error::Storage)?;
    File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Storage)
}
fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|p| !p.chars().any(char::is_control))
        && path
            .components()
            .all(|c| !matches!(c, std::path::Component::ParentDir))
}
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}
fn stable(run: &str, action: &str) -> String {
    format!("lab-{:x}", Sha256::digest(format!("{run}\0{action}")))
}

impl Launcher {
    /// Persist a single-use launch before any computer is created. Only a
    /// job-only enrollment is serialized; the account's session stays outside.
    pub fn prepare(
        directory: &Path,
        client: &Client,
        session: &Session,
        run: &str,
        settings: Settings,
        requirements: &EffectiveRequirements,
        enrolled: Enrolled,
        now_unix_ms: u64,
    ) -> Result<Self> {
        let _lock = lock(directory)?;
        if directory.join("state.json").exists() {
            return Err(Error::Busy);
        }
        let caps: &Capabilities = &enrolled.worker.capabilities;
        let duration_ms = requirements
            .max_seconds
            .checked_mul(1000)
            .ok_or(Error::Settings)?;
        let deadline = now_unix_ms
            .checked_add(duration_ms)
            .and_then(|v| v.checked_add(60_000))
            .ok_or(Error::Settings)?;
        let origin = client.deployment().website();
        if run.is_empty()
            || run.len() > 256
            || enrolled.worker.account != session.account().id
            || enrolled.worker.revoked
            || enrolled.worker.issuer_session.is_none()
            || enrolled.worker.expires_unix_ms < deadline
            || enrolled.worker.expires_unix_ms > now_unix_ms.saturating_add(86_400_000)
            || !enrolled
                .token
                .starts_with(&format!("labw_{}.", enrolled.worker.id))
            || caps.placement != Placement::Cloud
            || !requirements.eligible(caps)
            || requirements.private_local
            || !requirements.local_assets.is_empty()
            || !caps.local_assets.is_empty()
            || !absolute(&settings.executable)
            || !absolute(&settings.workspace)
            || settings.environment.is_empty()
            || settings.version.is_empty()
            || settings.size.is_empty()
            || settings.executable_sha256.len() != 64
            || !settings
                .executable_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !settings.handlers.contains_key(&requirements.job)
            || settings
                .handlers
                .iter()
                .any(|(kind, handler)| !caps.jobs.contains(kind) || !absolute(&handler.executable))
            || origin.scheme() != "https"
        {
            return Err(Error::Settings);
        }
        let root = settings
            .workspace
            .join(format!(".{}", stable(run, "state")));
        let config = worker::Config {
            origin: origin.origin().ascii_serialization(),
            credential: root.join("credential.json"),
            state: root.join("state"),
            handlers: settings.handlers,
            local_assets: BTreeMap::new(),
            allow_loopback_http: false,
        };
        let credential = serde_json::to_string(&enrolled).map_err(|_| Error::Settings)?;
        let config_json = serde_json::to_string(&config).map_err(|_| Error::Settings)?;
        let executable = settings.executable.to_str().ok_or(Error::Settings)?;
        let root = root.to_str().ok_or(Error::Settings)?;
        // The installed executable is checked before it sees the scoped key.
        // umask and an exclusive directory keep both settings files private.
        let bootstrap = format!(
            "set -eu\numask 077\nprintf '%s  %s\\n' {} {} | sha256sum -c - >/dev/null\nmkdir {}\nchmod 700 {}\nprintf '%s' {} > {}/credential.json\nprintf '%s' {} > {}/config.json\nchmod 600 {}/credential.json {}/config.json\nexec {} --config {}/config.json --once",
            quote(&settings.executable_sha256),
            quote(executable),
            quote(root),
            quote(root),
            quote(&credential),
            quote(root),
            quote(&config_json),
            quote(root),
            quote(root),
            quote(root),
            quote(executable),
            quote(root),
        );
        let computers = client.computers(session)?;
        let create = computers.create_request(CreateOptions {
            environment: Some(settings.environment),
            version: Some(settings.version),
            size: Some(settings.size),
            placement: Some("cloud".into()),
            idempotency_key: Some(stable(run, "create")),
            ..Default::default()
        })?;
        let saved = Saved {
            schema: "openagents.labs.cloud-launch.v1".into(),
            create,
            computer: None,
            bootstrap: Some(bootstrap),
            command_state: None,
            cleanup_requested: false,
            stage: Stage::Prepared,
            deadline_unix_ms: deadline,
            command_key: stable(run, "command"),
        };
        write(directory, &saved)?;
        Ok(Self {
            directory: directory.into(),
            saved,
            _lock,
        })
    }

    pub fn open(directory: &Path) -> Result<Self> {
        let _lock = lock(directory)?;
        let path = directory.join("state.json");
        let metadata = fs::symlink_metadata(&path).map_err(|_| Error::Storage)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > 4 * 1024 * 1024
        {
            return Err(Error::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(Error::Storage);
            }
        }
        let saved: Saved = serde_json::from_slice(&fs::read(path).map_err(|_| Error::Storage)?)
            .map_err(|_| Error::Storage)?;
        if saved.schema != "openagents.labs.cloud-launch.v1" {
            return Err(Error::Storage);
        }
        Ok(Self {
            directory: directory.into(),
            saved,
            _lock,
        })
    }
    fn save(&mut self, saved: Saved) -> Result<()> {
        write(&self.directory, &saved)?;
        self.saved = saved;
        Ok(())
    }
    /// The reference remains saved during uncertain deletion and restart.
    pub fn computer(&self) -> Option<&ComputerRef> {
        self.saved.computer.as_ref()
    }
    pub fn progress(&self) -> Progress {
        match self.saved.stage {
            Stage::Prepared | Stage::Creating => Progress::Creating,
            Stage::Waiting | Stage::Starting(_) => Progress::Starting,
            Stage::Running(_) => Progress::Running,
            Stage::Deleting { .. } => Progress::Cleaning,
            Stage::Finished => Progress::Finished {
                command_state: self.saved.command_state.clone(),
            },
        }
    }
    /// Request cleanup separately from local HTTP cancellation. A canceled
    /// observer must not imply that its remote process stopped.
    pub fn request_cleanup(&mut self) -> Result<()> {
        let mut next = self.saved.clone();
        next.cleanup_requested = true;
        if matches!(next.stage, Stage::Prepared) {
            next.stage = Stage::Finished;
            next.bootstrap = None;
        }
        self.save(next)
    }

    /// Advance one saved operation. Call again after a timeout or restart;
    /// this does not create a replacement command or computer.
    pub async fn step(
        &mut self,
        client: &Client,
        session: &Session,
        now_unix_ms: u64,
        cancel: &CancellationToken,
    ) -> Result<Progress> {
        let computers = client.computers(session)?;
        let mut next = self.saved.clone();
        if now_unix_ms >= next.deadline_unix_ms {
            next.cleanup_requested = true;
        }
        if next.cleanup_requested && matches!(next.stage, Stage::Prepared) {
            next.stage = Stage::Finished;
            next.bootstrap = None;
            self.save(next)?;
            return Ok(self.progress());
        }
        if next.cleanup_requested
            && next.computer.is_some()
            && !matches!(next.stage, Stage::Deleting { .. } | Stage::Finished)
        {
            next.stage = Stage::Deleting { sent: false };
            next.bootstrap = None;
            self.save(next.clone())?;
        }
        match next.stage.clone() {
            Stage::Prepared | Stage::Creating => {
                next.stage = Stage::Creating;
                self.save(next.clone())?;
                let computer = computers.create(&next.create, cancel).await?;
                next.computer = Some(computer.reference().clone());
                next.stage = Stage::Waiting;
                self.save(next)?;
            }
            Stage::Waiting => {
                let computer = next.computer.as_ref().ok_or(Error::Storage)?;
                let observed = computers.get(computer, cancel).await?;
                if observed.awake() {
                    let request = computers.command_request(
                        computer,
                        CommandOptions {
                            command: next.bootstrap.take().ok_or(Error::Storage)?,
                            idempotency_key: Some(next.command_key.clone()),
                            timeout_seconds: Some(
                                (next.deadline_unix_ms.saturating_sub(now_unix_ms) / 1000).max(1),
                            ),
                            ..Default::default()
                        },
                    )?;
                    next.stage = Stage::Starting(request);
                    self.save(next)?;
                }
            }
            Stage::Starting(request) => {
                let command = computers.execute(&request, cancel).await?;
                next.stage = Stage::Running(command.reference().clone());
                self.save(next)?;
            }
            Stage::Running(command) => {
                let observed = computers.get_command(&command, cancel).await?;
                if observed.outcome().is_some() {
                    next.command_state = Some(observed.data().state.clone());
                    next.stage = Stage::Deleting { sent: false };
                    self.save(next)?;
                }
            }
            Stage::Deleting { sent: false } => {
                let computer = next.computer.clone().ok_or(Error::Storage)?;
                next.stage = Stage::Deleting { sent: true };
                self.save(next)?;
                computers.delete(&computer, cancel).await?;
            }
            Stage::Deleting { sent: true } => {
                let computer = next.computer.as_ref().ok_or(Error::Storage)?;
                let deleted = match computers.get(computer, cancel).await {
                    Ok(value) => {
                        value.data().state == "deleted" && value.data().desired == "deleted"
                    }
                    Err(error) if error.status() == Some(404) => true,
                    Err(error) => return Err(error.into()),
                };
                if deleted {
                    next.stage = Stage::Finished;
                    self.save(next)?;
                }
            }
            Stage::Finished => {}
        }
        Ok(self.progress())
    }
}
