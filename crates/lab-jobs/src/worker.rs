//! An outbound-only worker. Operator-selected handlers run with an empty
//! environment, bounded output and deadlines; jobs never choose an executable.
use crate::{authority::Enrolled, protocol};
use fs2::FileExt;
use lab_kit::JobStatus;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_FILE: u64 = 12 * 1024 * 1024;
const MAX_STATE: u64 = 32 * 1024 * 1024;
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub &'static str);
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handler {
    pub executable: PathBuf,
    #[serde(default)]
    pub arguments: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub origin: String,
    pub credential: PathBuf,
    pub state: PathBuf,
    pub handlers: BTreeMap<String, Handler>,
    /// Checksum to existing local file. These paths never leave this process.
    #[serde(default)]
    pub local_assets: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub allow_loopback_http: bool,
}
#[derive(Serialize, Deserialize)]
pub struct HandlerInput {
    pub work: protocol::Work,
    pub local_assets: BTreeMap<String, PathBuf>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandlerOutput {
    #[serde(default)]
    pub outputs: Vec<protocol::Output>,
}
#[derive(Serialize, Deserialize)]
struct Pending {
    work: protocol::Work,
    /// Written and synced before spawning a child. On restart its effect is unknown.
    execution_marked: bool,
    result: Option<protocol::Finish>,
}
pub struct Worker {
    config: Config,
    credential: Enrolled,
    sdk: openagents_sdk::labs::worker::Client,
    _lock: File,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |v| v.as_millis().try_into().unwrap_or(u64::MAX))
}
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path).map_err(|_| Error("Open the worker settings file."))?;
    let metadata = file
        .metadata()
        .map_err(|_| Error("Read the worker settings file."))?;
    if !metadata.is_file() || metadata.len() > MAX_STATE {
        return Err(Error("Choose a smaller settings file."));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Error("Keep worker settings private to your user."));
        }
    }
    serde_json::from_reader(file).map_err(|_| Error("Check the worker settings file."))
}
fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error("Save the worker result."))?;
    if bytes.len() as u64 > MAX_STATE {
        return Err(Error("The worker result is too large to save."));
    }
    let temp = path.with_extension("pending");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // This temporary file belongs to the locked state directory. A previous
    // crash may leave it, but never authorizes repeating an execution.
    if temp.exists() {
        fs::remove_file(&temp).map_err(|_| Error("Save the worker result."))?;
    }
    let mut file = options
        .open(&temp)
        .map_err(|_| Error("Save the worker result."))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error("Save the worker result."))?;
    fs::rename(&temp, path).map_err(|_| Error("Save the worker result."))?;
    File::open(path.parent().ok_or(Error("Choose a state directory."))?)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error("Save the worker result."))
}
fn file_hash(path: &Path) -> Result<String> {
    if !fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(Error("Choose a private input file."));
    }
    let mut file = File::open(path).map_err(|_| Error("A private input is missing."))?;
    if !file
        .metadata()
        .map_err(|_| Error("Read the private input."))?
        .is_file()
    {
        return Err(Error("Choose a private input file."));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| Error("Read the private input."))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Worker {
    pub fn open(path: &Path) -> Result<Self> {
        let mut config: Config = read_json(path)?;
        if !config.state.is_absolute() || !config.credential.is_absolute() {
            return Err(Error("Choose absolute worker file paths."));
        }
        let deployment = openagents_sdk::Deployment::new(
            &format!("{}/v1/", config.origin.trim_end_matches('/')),
            &config.origin,
        )
        .map_err(|_| Error("Choose the OpenAgents HTTPS address."))?;
        let url = deployment.website();
        let loopback = matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        if (url.scheme() != "https"
            && !(config.allow_loopback_http && loopback && url.scheme() == "http"))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error("Choose the OpenAgents HTTPS address."));
        }
        config.origin = url.origin().ascii_serialization();
        if !config.state.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&config.state).map_err(|_| {
                Error("Create a private worker state directory under an existing directory.")
            })?;
        }
        let state_metadata = fs::symlink_metadata(&config.state)
            .map_err(|_| Error("Read the worker state directory."))?;
        if !state_metadata.is_dir() || state_metadata.file_type().is_symlink() {
            return Err(Error("Choose a private worker state directory."));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if state_metadata.permissions().mode() & 0o077 != 0
                || state_metadata.uid() != unsafe { libc::geteuid() }
            {
                return Err(Error("Choose a state directory private to this user."));
            }
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(config.state.join("lock"))
            .map_err(|_| Error("Open the worker lock."))?;
        lock.try_lock_exclusive()
            .map_err(|_| Error("This worker is already running."))?;
        let credential: Enrolled = read_json(&config.credential)?;
        if !credential
            .token
            .starts_with(&format!("labw_{}.", credential.worker.id))
            || credential.token.len() != 102
            || credential.worker.revoked
            || credential.worker.expires_unix_ms <= now()
            || credential.worker.capabilities.validate().is_err()
        {
            return Err(Error("Add this Lab worker again."));
        }
        if config
            .handlers
            .keys()
            .any(|k| !credential.worker.capabilities.jobs.contains(k))
            || config.handlers.is_empty()
            || credential
                .worker
                .capabilities
                .jobs
                .iter()
                .any(|k| !config.handlers.contains_key(k))
        {
            return Err(Error("Configure a handler for each declared job."));
        }
        for handler in config.handlers.values() {
            if !handler.executable.is_absolute()
                || !handler.executable.is_file()
                || handler.arguments.len() > 64
                || handler.arguments.iter().any(|a| a.len() > 4096)
            {
                return Err(Error("Choose an installed handler and fixed arguments."));
            }
        }
        if config
            .local_assets
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != credential.worker.capabilities.local_assets
        {
            return Err(Error("Declare the worker's local files again."));
        }
        for (hash, path) in &mut config.local_assets {
            *path = path
                .canonicalize()
                .map_err(|_| Error("A private input is missing."))?;
            if file_hash(path)? != *hash {
                return Err(Error("A private input has changed. Add it again."));
            }
        }
        let transport = openagents_sdk::Client::with_timeouts(
            deployment,
            Duration::from_secs(5),
            Duration::from_secs(30),
        )
        .map_err(|_| Error("Start the worker connection."))?;
        let sdk = transport
            .labs_worker(&credential.token)
            .map_err(sdk_error)?;
        Ok(Self {
            config,
            credential,
            sdk,
            _lock: lock,
        })
    }
    fn bound_work(&self, work: &protocol::Work, attempt: Option<&protocol::Attempt>) -> bool {
        let lease = &work.assignment.lease;
        lease.worker == self.credential.worker.id
            && lease.expires_unix_ms > now()
            && lease.job.spec.version == work.version.reference()
            && work.version.content_sha256 == lab_kit::digest(&work.version.content)
            && work.requirements.job
                == format!("{}/{}", lease.job.spec.kind, lease.job.spec.job_type)
            && work
                .requirements
                .eligible(&self.credential.worker.capabilities)
            && attempt.is_none_or(|a| a.job == lease.job.id && a.attempt == lease.attempt)
    }
    fn pending_path(&self) -> PathBuf {
        self.config.state.join("job.json")
    }
    fn forget(&self) -> Result<()> {
        fs::remove_file(self.pending_path())
            .and_then(|_| File::open(&self.config.state)?.sync_all())
            .map_err(|_| Error("Save the finished job."))
    }
    async fn finish(&self, pending: &mut Pending) -> Result<()> {
        let result = pending
            .result
            .as_ref()
            .ok_or(Error("Keep the unfinished job for review."))?;
        self.sdk
            .finish(result, &openagents_sdk::CancellationToken::new())
            .await
            .map_err(sdk_error)?;
        self.forget()
    }
    /// Resume a saved result without spawning its handler again. A crash after
    /// the synced execution marker is uncertain, even if the child never started.
    pub async fn run_once(&self) -> Result<bool> {
        if self.pending_path().exists() {
            let mut pending: Pending = read_json(&self.pending_path())?;
            if !pending.execution_marked && pending.result.is_none() {
                // The start request cannot have been sent before this marker.
                // Its unused claim may expire and be safely offered again.
                self.forget()?;
                return Ok(false);
            }
            if pending.result.is_none() {
                pending.result = Some(protocol::Finish {
                    job: pending.work.assignment.lease.job.id.clone(),
                    attempt: pending.work.assignment.lease.attempt,
                    status: JobStatus::Uncertain,
                    outputs: vec![],
                });
                save(&self.pending_path(), &pending)?;
            }
            self.finish(&mut pending).await?;
            return Ok(true);
        }
        let available = self
            .sdk
            .poll(
                &protocol::Poll { wait_seconds: 25 },
                &openagents_sdk::CancellationToken::new(),
            )
            .await
            .map_err(sdk_error)?;
        let Some(work) = available.work else {
            return Ok(false);
        };
        if !self.bound_work(&work, None) {
            return Err(Error("This computer cannot run that job."));
        }
        for hash in &work.requirements.local_assets {
            if self
                .config
                .local_assets
                .get(hash)
                .is_none_or(|p| file_hash(p).ok().as_ref() != Some(hash))
            {
                return Err(Error("A private input has changed. Add it again."));
            }
        }
        let mut pending = Pending {
            work,
            execution_marked: false,
            result: None,
        };
        save(&self.pending_path(), &pending)?;
        let attempt = protocol::Attempt {
            job: pending.work.assignment.lease.job.id.clone(),
            attempt: pending.work.assignment.lease.attempt,
        };
        // Once start may have been accepted, a reconnect must not repeat effects.
        pending.execution_marked = true;
        save(&self.pending_path(), &pending)?;
        let work = self
            .sdk
            .start(&attempt, &openagents_sdk::CancellationToken::new())
            .await
            .map_err(sdk_error)?;
        if !self.bound_work(&work, Some(&attempt)) || !work.assignment.lease.started {
            return Err(Error("Check this worker's job assignment."));
        }
        pending.work = work;
        save(&self.pending_path(), &pending)?;
        let result = self.execute(&pending.work, &attempt).await;
        pending.result = Some(protocol::Finish {
            job: attempt.job,
            attempt: attempt.attempt,
            status: if result.is_ok() {
                JobStatus::Done
            } else {
                JobStatus::Uncertain
            },
            outputs: result.unwrap_or_default(),
        });
        save(&self.pending_path(), &pending)?;
        self.finish(&mut pending).await?;
        Ok(true)
    }
    async fn execute(
        &self,
        work: &protocol::Work,
        attempt: &protocol::Attempt,
    ) -> Result<Vec<protocol::Output>> {
        let handler = self
            .config
            .handlers
            .get(&work.requirements.job)
            .ok_or(Error("Install this job's handler."))?;
        let cwd = self
            .config
            .state
            .join(format!("run-{}-{}", attempt.job, attempt.attempt));
        // Job identifiers are generated by the store, but are still not paths.
        if attempt
            .job
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && b != b'-')
        {
            return Err(Error("Check the job identifier."));
        }
        fs::create_dir(&cwd).map_err(|_| Error("Create this job's directory."))?;
        let input = HandlerInput {
            work: work.clone(),
            local_assets: work
                .requirements
                .local_assets
                .iter()
                .filter_map(|h| {
                    self.config
                        .local_assets
                        .get(h)
                        .map(|p| (h.clone(), p.clone()))
                })
                .collect(),
        };
        let bytes = serde_json::to_vec(&input).map_err(|_| Error("Read this job's inputs."))?;
        let mut command = Command::new(&handler.executable);
        command
            .args(&handler.arguments)
            .env_clear()
            .env("HOME", &cwd)
            .env("TMPDIR", &cwd)
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        limits(
            &mut command,
            work.requirements.memory_bytes,
            work.requirements.max_seconds,
        )?;
        let mut child = ChildGuard(
            command
                .spawn()
                .map_err(|_| Error("Start the job's handler."))?,
        );
        let stdin = child.stdin.take().ok_or(Error("Write the job's inputs."))?;
        let writer = std::thread::spawn(move || {
            let mut stdin = stdin;
            let _ = stdin.write_all(&bytes);
        });
        let stdout = child.stdout.take().ok_or(Error("Read the job's output."))?;
        let stderr = child.stderr.take().ok_or(Error("Read the job's output."))?;
        let output = std::thread::spawn(move || bounded_read(stdout, MAX_FILE));
        let errors = std::thread::spawn(move || bounded_read(stderr, 64 * 1024));
        let deadline = Instant::now() + Duration::from_secs(work.requirements.max_seconds);
        let mut lease_until = work.assignment.lease.expires_unix_ms;
        let mut next_heartbeat = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|_| Error("Check the job's handler."))?
            {
                break Some(status);
            }
            if Instant::now() >= deadline || now() >= lease_until {
                break None;
            }
            if Instant::now() >= next_heartbeat {
                let remaining = Duration::from_millis(lease_until.saturating_sub(now()))
                    .min(deadline.saturating_duration_since(Instant::now()));
                match tokio::time::timeout(
                    remaining,
                    self.sdk
                        .heartbeat(attempt, &openagents_sdk::CancellationToken::new()),
                )
                .await
                {
                    Ok(Ok(current)) if self.bound_work(&current, Some(attempt)) => {
                        lease_until = current.assignment.lease.expires_unix_ms
                    }
                    _ => break None,
                }
                next_heartbeat = Instant::now() + Duration::from_secs(5);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        kill_group(&mut child);
        let _ = child.wait();
        let _ = writer.join();
        let out = output
            .join()
            .map_err(|_| Error("Read the job's output."))??;
        let _ = errors.join();
        if !status.is_some_and(|s| s.success()) {
            return Err(Error("The job stopped before it finished."));
        }
        let result: HandlerOutput =
            serde_json::from_slice(&out).map_err(|_| Error("Read the job's output."))?;
        if result.outputs.len() > 16 {
            return Err(Error("The job produced too many files."));
        }
        use base64::Engine;
        let mut total = 0usize;
        for output in &result.outputs {
            if output.media_type.is_empty()
                || output.media_type.len() > 128
                || (work.requirements.private_local && output.role == lab_kit::ArtifactRole::Source)
            {
                return Err(Error("The job produced an unsupported output."));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&output.content_base64)
                .map_err(|_| Error("The job produced an invalid output."))?;
            total = total.saturating_add(bytes.len());
            if total > 8 * 1024 * 1024 {
                return Err(Error("The job produced too much output."));
            }
        }
        Ok(result.outputs)
    }
}
fn sdk_error(error: openagents_sdk::labs::Error) -> Error {
    Error(if matches!(error.status(), Some(401 | 403)) {
        "This worker no longer has permission. Add it again."
    } else {
        "The worker could not complete its request to OpenAgents."
    })
}
fn bounded_read(mut source: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let mut exceeded = false;
    loop {
        let n = source
            .read(&mut buffer)
            .map_err(|_| Error("Read the job's output."))?;
        if n == 0 {
            break;
        }
        if bytes.len().saturating_add(n) > limit as usize {
            exceeded = true;
        } else if !exceeded {
            bytes.extend_from_slice(&buffer[..n]);
        }
    }
    if exceeded {
        Err(Error("The job produced too much output."))
    } else {
        Ok(bytes)
    }
}
#[cfg(unix)]
fn limits(command: &mut Command, memory: u64, seconds: u64) -> Result<()> {
    use std::os::unix::process::CommandExt;
    // Only async-signal-safe syscalls run after fork. No allocator or locks.
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            if libc::getppid() != parent {
                return Err(std::io::Error::other("The worker stopped."));
            }
            for (resource, value) in [
                (libc::RLIMIT_AS, memory),
                (libc::RLIMIT_CPU, seconds.saturating_add(1)),
                (libc::RLIMIT_NOFILE, 64),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: value as libc::rlim_t,
                    rlim_max: value as libc::rlim_t,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}
#[cfg(not(unix))]
fn limits(_: &mut Command, _: u64, _: u64) -> Result<()> {
    Err(Error("This worker needs a Unix host for process limits."))
}
struct ChildGuard(Child);
impl std::ops::Deref for ChildGuard {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        kill_group(&mut self.0);
        let _ = self.0.wait();
    }
}
fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}
/// The built-in fixture prints content and input checksums, never private bytes.
pub fn echo_handler() -> Result<()> {
    use base64::Engine;
    let input: HandlerInput = serde_json::from_slice(&bounded_read(std::io::stdin(), MAX_FILE)?)
        .map_err(|_| Error("Read the echo job."))?;
    let mut checksums = Vec::new();
    for (expected, path) in &input.local_assets {
        let hash = file_hash(path)?;
        if &hash != expected {
            return Err(Error("The private input changed."));
        }
        checksums.push(hash);
    }
    let content=serde_json::to_vec(&serde_json::json!({"text":input.work.version.content.get("text").and_then(|v|v.as_str()).unwrap_or("").to_uppercase(),"input_checksums":checksums})).map_err(|_|Error("Write the echo result."))?;
    let output = HandlerOutput {
        outputs: vec![protocol::Output {
            role: lab_kit::ArtifactRole::Build,
            media_type: "application/json".into(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(content),
        }],
    };
    serde_json::to_writer(std::io::stdout(), &output).map_err(|_| Error("Write the echo result."))
}
