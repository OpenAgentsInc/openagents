//! Explicit resident operator policy over the durable cloud job owner.

use crate::{Backend, Mode, Placement, Record, Spec, State, Store, workspace};
use coder_access::{Code, Operation, Outcome, cloud as dto};
use coder_host::Principal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};

pub type Authority = Arc<dyn Fn(&Principal) -> bool + Send + Sync>;
type Result<T> = std::result::Result<T, Code>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub device: String,
    pub workspace: String,
    pub project: String,
    pub profiles: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Adapter {
    Unavailable,
    Boat {
        origin: String,
        token_file: PathBuf,
    },
    Gce {
        pool_file: PathBuf,
        gcloud_binary: PathBuf,
        config_directory: PathBuf,
        credential_file: PathBuf,
        hosts: Vec<crate::pool::Host>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub workspace: String,
    pub project: String,
    pub cwd: PathBuf,
    pub source_revision: String,
    pub source_digest: String,
    /// Display metadata; source admission uses the revision and digest above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub paths: Vec<String>,
    pub include: Vec<String>,
    pub pool: String,
    pub placement: Placement,
    pub mode: Mode,
    pub executor: String,
    pub model: Option<String>,
    pub reasoning: Option<String>,
    pub max_timeout_seconds: u64,
    pub size: String,
    pub template: Option<String>,
    pub credentials: BTreeMap<String, PathBuf>,
    pub adapter: Adapter,
}
impl Profile {
    /// Whether this profile is an admitted Coder engine identity: Coder
    /// mode, a qualified engine, and only that engine's credential names
    /// plus GitHub tool tokens. Other owners (environment setup) reuse this
    /// admission instead of restating it.
    pub fn qualified_coder_identity(&self) -> bool {
        crate::operator_adapters::qualified_identity(self)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub operators: Vec<Assignment>,
    pub profiles: BTreeMap<String, Profile>,
}
impl Policy {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.schema != "openagents.coder.cloud-operator.v1"
            || self.operators.len() > 256
            || self.profiles.len() > 64
        {
            return Err("The operator policy schema or bounds are invalid.".into());
        }
        for (name, p) in &self.profiles {
            dto::alias(name).map_err(|_| "Invalid operator profile alias.")?;
            for value in [&p.workspace, &p.project, &p.pool, &p.executor] {
                dto::alias(value).map_err(|_| "Invalid operator profile binding.")?;
            }
            dto::digest(&p.source_digest).map_err(|_| "Invalid operator source digest.")?;
            if let Some(repository) = &p.repository {
                dto::repository(repository).map_err(|_| "Invalid operator repository label.")?;
            }
            if let Some(branch) = &p.branch {
                dto::branch(branch).map_err(|_| "Invalid operator branch label.")?;
            }
            if p.size.is_empty()
                || p.size.len() > 128
                || p.size.contains('\0')
                || p.template
                    .as_ref()
                    .is_some_and(|v| v.is_empty() || v.len() > 256 || v.contains('\0'))
            {
                return Err("The operator machine labels exceed their limits.".into());
            }
            if !p.cwd.is_absolute()
                || p.source_revision.len() != 40
                || !p.source_revision.bytes().all(|b| b.is_ascii_hexdigit())
                || p.credentials.len() > 32
                || p.paths.len() > 64
                || p.include.len() > 64
            {
                return Err(
                    "The operator source must bind an explicit directory and commit.".into(),
                );
            }
            for path in p.paths.iter().chain(&p.include) {
                workspace::validate_path(path)?;
            }
            // A subscription token is only ever a person's own release
            // (#11204); an operator profile never names one.
            if p.credentials.contains_key(crate::claude::OAUTH_TOKEN) {
                return Err(crate::claude::REFUSAL.into());
            }
            let spec = spec(p, "policy validation", p.max_timeout_seconds);
            spec.validate()?;
            match (&p.adapter, p.placement) {
                (Adapter::Boat { .. }, Placement::Boat)
                | (Adapter::Gce { .. }, Placement::Gce)
                | (Adapter::Unavailable, _) => {}
                _ => return Err("The operator adapter and placement differ.".into()),
            }
        }
        for a in &self.operators {
            if a.device.len() != 64
                || !a
                    .device
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || a.profiles.is_empty()
                || a.profiles.len() > 64
            {
                return Err("An operator assignment is invalid.".into());
            }
            dto::alias(&a.workspace).map_err(|_| "Invalid operator workspace.")?;
            dto::alias(&a.project).map_err(|_| "Invalid operator project.")?;
            for name in &a.profiles {
                let p = self
                    .profiles
                    .get(name)
                    .ok_or("An operator assignment names an unknown profile.")?;
                if p.workspace != a.workspace || p.project != a.project {
                    return Err("The operator profile changed project or workspace.".into());
                }
            }
        }
        Ok(())
    }
}
fn spec(p: &Profile, prompt: &str, timeout: u64) -> Spec {
    Spec {
        placement: p.placement,
        mode: p.mode,
        agent: p.executor.clone(),
        task: prompt.into(),
        model: p.model.clone(),
        reasoning: p.reasoning.clone(),
        cwd: p.cwd.clone(),
        timeout_seconds: timeout,
        size: p.size.clone(),
        template: p.template.clone(),
        credential_names: p.credentials.keys().cloned().collect(),
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", workspace::digest(bytes))
}
fn encoded(v: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|_| Code::Unavailable)
}

pub(crate) fn private(path: &Path, dir: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|_| Code::Unavailable)?;
    if meta.file_type().is_symlink() || (dir && !meta.is_dir()) || (!dir && !meta.is_file()) {
        return Err(Code::Forbidden);
    }
    let canonical = path.canonicalize().map_err(|_| Code::Unavailable)?;
    if canonical != path {
        return Err(Code::Forbidden);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o777 != if dir { 0o700 } else { 0o600 }
            || (!dir && meta.nlink() != 1)
        {
            return Err(Code::Forbidden);
        }
    }
    Ok(())
}
pub(crate) fn read_private(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = open_private(path)?;
    let before = file.metadata().map_err(|_| Code::Unavailable)?;
    let mut bytes = Vec::new();
    (&file)
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Code::Unavailable)?;
    if bytes.len() > max {
        return Err(Code::Bounds);
    }
    let after = file.metadata().map_err(|_| Code::Unavailable)?;
    let current = open_private(path)?
        .metadata()
        .map_err(|_| Code::Unavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let identity = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if identity(&before) != identity(&after) || identity(&after) != identity(&current) {
            return Err(Code::Stale);
        }
    }
    #[cfg(not(unix))]
    if before.len() != after.len() || after.len() != current.len() {
        return Err(Code::Stale);
    }
    Ok(bytes)
}
fn open_private(path: &Path) -> Result<fs::File> {
    #[cfg(unix)]
    {
        use std::{
            ffi::CString,
            os::{
                fd::{AsRawFd, FromRawFd},
                unix::{ffi::OsStrExt, fs::MetadataExt},
            },
            path::Component,
        };
        if !path.is_absolute() {
            return Err(Code::Forbidden);
        }
        let mut current = fs::File::open("/").map_err(|_| Code::Unavailable)?;
        let parts = path.components().skip(1).collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(Code::Forbidden);
        }
        for (index, component) in parts.iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(Code::Forbidden);
            };
            let name = CString::new(name.as_bytes()).map_err(|_| Code::Forbidden)?;
            let last = index + 1 == parts.len();
            let flags = libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if last { 0 } else { libc::O_DIRECTORY };
            // SAFETY: the directory descriptor and component name remain valid for this call.
            let descriptor = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
            if descriptor < 0 {
                return Err(Code::Unavailable);
            }
            // SAFETY: openat returned a new descriptor owned by this file.
            let file = unsafe { fs::File::from_raw_fd(descriptor) };
            let meta = file.metadata().map_err(|_| Code::Unavailable)?;
            if last {
                let parent = current.metadata().map_err(|_| Code::Unavailable)?;
                if !meta.is_file()
                    || meta.uid() != unsafe { libc::geteuid() }
                    || meta.mode() & 0o777 != 0o600
                    || meta.nlink() != 1
                    || parent.uid() != unsafe { libc::geteuid() }
                    || parent.mode() & 0o777 != 0o700
                {
                    return Err(Code::Forbidden);
                }
                return Ok(file);
            }
            current = file;
        }
        Err(Code::Forbidden)
    }
    #[cfg(not(unix))]
    {
        private(path.parent().ok_or(Code::Forbidden)?, true)?;
        private(path, false)?;
        fs::File::open(path).map_err(|_| Code::Unavailable)
    }
}
fn directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path).map_err(|_| Code::Unavailable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| Code::Unavailable)?;
        }
    }
    private(path, true)
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    directory(path.parent().ok_or(Code::Forbidden)?)?;
    if path.exists() {
        private(path, false)?;
    }
    static NEXT_WRITE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_WRITE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("writing-{}-{sequence}", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    let mut file = options.open(&temp).map_err(|_| Code::Unavailable)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Code::Unavailable)?;
    fs::rename(temp, path).map_err(|_| Code::Unavailable)?;
    fs::File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(|_| Code::Unavailable)
}
#[derive(Clone)]
enum PolicySource {
    Memory(Arc<Mutex<Policy>>),
    File(PathBuf),
}
impl PolicySource {
    fn read(&self) -> Result<Policy> {
        let p = match self {
            Self::Memory(p) => p.lock().map_err(|_| Code::Unavailable)?.clone(),
            Self::File(path) => serde_json::from_slice(&read_private(path, 128 * 1024)?)
                .map_err(|_| Code::Malformed)?,
        };
        p.validate().map_err(|_| Code::Malformed)?;
        Ok(p)
    }
}

pub(crate) trait Driver: Send + Sync {
    fn run(
        &self,
        store: Store,
        record: Record,
        check: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> std::result::Result<(), String>;
}
pub(crate) struct BackendDriver<B>(pub(crate) B);
impl<B: Backend + Send + Sync + 'static> Driver for BackendDriver<B> {
    fn run(
        &self,
        store: Store,
        mut record: Record,
        check: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> std::result::Result<(), String> {
        loop {
            // A paused Claude job waits for its reset without holding the
            // job's lease, so reads, follows, and cancellation still work.
            if let Some(pause) = store
                .read(&record.id)
                .ok()
                .and_then(|r| crate::claude_task::pause(&r))
            {
                loop {
                    if store.cancellation_requested(&record.id).unwrap_or(false) {
                        break;
                    }
                    let now = crate::now_ms() / 1000;
                    if now >= pause.until {
                        // Resume only under the current policy. A refusal
                        // that persists leaves the job paused and retained:
                        // a follow or a restarted operator picks it up.
                        let admitted = (0..crate::claude_task::ADMIT_TRIES).any(|_| {
                            (check)() || {
                                std::thread::sleep(Duration::from_secs(1));
                                false
                            }
                        });
                        if !admitted {
                            return Err("The current operator policy, source, or native grant refused execution.".into());
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(
                        (pause.until - now).min(crate::claude_task::CHECK_EVERY_SECONDS),
                    ));
                }
            }
            let lease = match self.lease(&store, &record.id) {
                Ok(lease) => lease,
                // A paused job keeps trying: a reader, or a child process
                // that briefly inherited the lock, must not strand it.
                Err(_)
                    if store
                        .read(&record.id)
                        .is_ok_and(|r| r.state == State::Paused) =>
                {
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
                Err(error) => return Err(error),
            };
            record = lease.read(&record.id)?;
            if record.state == State::Paused {
                if record.cancel_requested || lease.cancelled() {
                    record.cancel_requested = true;
                    record.state = State::Cancelled;
                    record.updated_ms = crate::now_ms();
                    return lease.save(&record);
                }
                // A job on the user's released credential resumes only with a
                // fresh release; without one it stays paused (BYO-05).
                if crate::release::released_class(&record).is_some()
                    && !crate::release::armed(&record.id)
                {
                    return Err(crate::claude::REVOKED_REFUSAL.into());
                }
                if !crate::claude_task::resume(&mut record, crate::now_ms() / 1000) {
                    return Ok(());
                }
                lease.save(&record)?;
            }
            if crate::release::released_class(&record).is_some()
                && !crate::release::armed(&record.id)
            {
                return Err(crate::claude::REVOKED_REFUSAL.into());
            }
            let result = self.drive(&lease, &mut record, check.clone());
            // One release serves one turn.
            crate::release::disarm(&record.id);
            if result.is_err() {
                return result;
            }
            // A Claude turn that ended on a usage limit pauses until the
            // reset; one whose login needs the person stops with the
            // sign-in prompt (BYO-03).
            let root = store.root().parent().unwrap_or(store.root());
            let book = crate::claude_task::book(root, &record);
            match crate::claude_task::settle(&mut record, &book, crate::now_ms() / 1000) {
                Ok(Some(crate::claude_task::Outcome::Limited { .. })) => {
                    lease.save(&record)?;
                }
                Ok(Some(crate::claude_task::Outcome::SignIn)) => return lease.save(&record),
                Ok(None) | Err(_) => return result,
            }
        }
    }
}
impl<B: Backend + Send + Sync + 'static> BackendDriver<B> {
    fn lease(&self, store: &Store, id: &str) -> std::result::Result<crate::Lease, String> {
        let started = std::time::Instant::now();
        loop {
            match store.lease(id) {
                Ok(lease) => return Ok(lease),
                Err(error)
                    if error == "Another process is using this remote job."
                        && started.elapsed() < Duration::from_secs(2) =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn drive(
        &self,
        lease: &crate::Lease,
        record: &mut Record,
        check: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> std::result::Result<(), String> {
        let backend = Checked {
            inner: &self.0,
            check,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot start the explicit operator runtime.")?;
        let result = runtime.block_on(crate::drive(
            &backend,
            lease,
            record,
            &AtomicBool::new(false),
            Duration::from_millis(250),
            &mut |_| {},
        ));
        if result.is_err() && !record.state.terminal() {
            record.error=Some("The operator execution is unresolved. Reconcile this original job; another task was not submitted.".into());
            let _ = lease.save(record);
        }
        result
    }
}
struct Checked<'a, B> {
    inner: &'a B,
    check: Arc<dyn Fn() -> bool + Send + Sync>,
}
impl<B> Checked<'_, B> {
    fn admit(&self) -> std::result::Result<(), String> {
        if (self.check)() {
            Ok(())
        } else {
            Err("The current operator policy, source, or native grant refused execution.".into())
        }
    }
}
impl<B: Backend> Backend for Checked<'_, B> {
    async fn resolve(&self, r: &mut Record) -> crate::Result<()> {
        self.admit()?;
        self.inner.resolve(r).await
    }
    async fn provision(&self, r: &mut Record) -> crate::Result<String> {
        self.admit()?;
        self.inner.provision(r).await
    }
    async fn prepare(&self, r: &Record) -> crate::Result<()> {
        self.admit()?;
        self.inner.prepare(r).await
    }
    async fn dispatch(&self, r: &Record) -> crate::Result<crate::Task> {
        self.admit()?;
        self.inner.dispatch(r).await
    }
    async fn recover(&self, r: &Record) -> crate::Result<Option<crate::Task>> {
        self.admit()?;
        self.inner.recover(r).await
    }
    async fn poll(&self, r: &Record) -> crate::Result<crate::Observation> {
        self.admit()?;
        self.inner.poll(r).await
    }
    async fn cancel(&self, r: &Record) -> crate::Result<()> {
        self.admit()?;
        self.inner.cancel(r).await
    }
    async fn collect(&self, r: &Record) -> crate::Result<Option<Value>> {
        self.admit()?;
        self.inner.collect(r).await
    }
    async fn restart(&self, r: &Record) -> crate::Result<()> {
        self.admit()?;
        self.inner.restart(r).await
    }
    async fn cleanup(&self, r: &Record) -> crate::Result<Option<Value>> {
        self.admit()?;
        self.inner.cleanup(r).await
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmittedJob {
    principal: Principal,
    admission: dto::Admission,
    request_digest: String,
    input_digest: String,
    /// The digest of the account, workspace, and membership epoch whose
    /// released Claude credential this job first ran on (BYO-05). Only that
    /// owner may release into it again. Never a credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Effect {
    principal: Principal,
    admission: dto::Admission,
    digest: String,
    accepted: Option<dto::Accepted>,
}
struct RegisteredDriver {
    revision: String,
    driver: Arc<dyn Driver>,
}
struct Inner {
    root: PathBuf,
    policy: PolicySource,
    drivers: Mutex<BTreeMap<String, RegisteredDriver>>,
    authority: Authority,
    serial: Mutex<()>,
    setup: std::sync::OnceLock<Arc<dyn SetupSessions>>,
}
#[derive(Clone)]
pub struct Operator(Arc<Inner>);
impl std::fmt::Debug for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Operator { explicitly configured native cloud owner }")
    }
}
impl Operator {
    pub fn new(
        root: PathBuf,
        policy: Policy,
        authority: Authority,
    ) -> std::result::Result<Self, String> {
        policy.validate()?;
        directory(&root).map_err(|_| "The operator state must be a private explicit directory.")?;
        Ok(Self(Arc::new(Inner {
            root,
            policy: PolicySource::Memory(Arc::new(Mutex::new(policy))),
            drivers: Mutex::new(BTreeMap::new()),
            authority,
            serial: Mutex::new(()),
            setup: std::sync::OnceLock::new(),
        })))
    }
    pub fn load(
        policy_path: impl Into<PathBuf>,
        root: impl Into<PathBuf>,
        authority: Authority,
    ) -> std::result::Result<Self, String> {
        let path = policy_path.into();
        let source = PolicySource::File(path);
        let policy = source
            .read()
            .map_err(|_| "The operator policy must be an explicit private document.")?;
        let root = root.into();
        directory(&root).map_err(|_| "The operator state must be a private explicit directory.")?;
        let mut drivers = BTreeMap::new();
        for (name, p) in &policy.profiles {
            let revision = Self::profile_revision(p)
                .map_err(|_| "The explicit operator profile is unavailable.")?;
            if let Some(driver) = super::operator_adapters::configured(p)
                .map_err(|_| "The explicit operator adapter is invalid.")?
            {
                if revision
                    != Self::profile_revision(p)
                        .map_err(|_| "The explicit operator profile is unavailable.")?
                {
                    return Err("The operator profile changed while its adapter was loaded.".into());
                }
                drivers.insert(name.clone(), RegisteredDriver { revision, driver });
            }
        }
        let operator = Self(Arc::new(Inner {
            root,
            policy: source,
            drivers: Mutex::new(drivers),
            authority,
            serial: Mutex::new(()),
            setup: std::sync::OnceLock::new(),
        }));
        operator.resume_paused();
        Ok(operator)
    }
    /// Pick up every Claude job a usage limit paused, as a restarted
    /// operator does: each waits for its retained reset and then continues
    /// under the job's original admission (BYO-03). Returns how many.
    pub fn resume_paused(&self) -> usize {
        let Ok(records) = self
            .store()
            .and_then(|s| s.list().map_err(|_| Code::Unavailable))
        else {
            return 0;
        };
        let mut resumed = 0;
        for record in records {
            if record.state != State::Paused {
                continue;
            }
            if let Ok(a) = self.job_admission(&record.id) {
                self.start(&a.admission.profile.clone(), record, a);
                resumed += 1;
            }
        }
        resumed
    }
    /// Inject a backend without reading a login, environment variable, or pool.
    pub fn with_backend<B: Backend + Send + Sync + 'static>(
        self,
        profile: &str,
        backend: B,
    ) -> std::result::Result<Self, String> {
        if !self
            .0
            .policy
            .read()
            .map_err(|_| "Operator policy unavailable.")?
            .profiles
            .contains_key(profile)
        {
            return Err("The injected backend needs a configured profile.".into());
        }
        self.0
            .drivers
            .lock()
            .map_err(|_| "Operator driver lock failed.")?
            .insert(
                profile.into(),
                RegisteredDriver {
                    revision: Self::profile_revision(
                        &self
                            .0
                            .policy
                            .read()
                            .map_err(|_| "Operator policy unavailable.")?
                            .profiles[profile],
                    )
                    .map_err(|_| "Operator profile unavailable.")?,
                    driver: Arc::new(BackendDriver(backend)),
                },
            );
        Ok(self)
    }
    pub fn replace_policy(&self, policy: Policy) -> std::result::Result<(), String> {
        policy.validate()?;
        match &self.0.policy {
            PolicySource::Memory(p) => {
                *p.lock().map_err(|_| "Operator policy lock failed.")? = policy;
                Ok(())
            }
            _ => Err(
                "File-backed operator policies are updated through their explicit document.".into(),
            ),
        }
    }
    fn store(&self) -> Result<Store> {
        let path = self.0.root.join("jobs");
        directory(&path)?;
        Ok(Store::under(path))
    }
    fn profile(&self, device: &str, workspace: &str, project: &str, name: &str) -> Result<Profile> {
        let policy = self.0.policy.read()?;
        if !policy.operators.iter().any(|a| {
            a.device == device
                && a.workspace == workspace
                && a.project == project
                && a.profiles.iter().any(|p| p == name)
        }) {
            return Err(Code::Forbidden);
        }
        let p = policy.profiles.get(name).ok_or(Code::Forbidden)?.clone();
        let actual = workspace::source_identity(&p.cwd, &p.source_revision, &p.paths, &p.include)
            .map_err(|_| Code::Stale)?;
        if actual != p.source_digest {
            return Err(Code::Stale);
        }
        for path in p.credentials.values() {
            read_private(path, 1024 * 1024)?;
        }
        Ok(p)
    }
    fn profile_revision(p: &Profile) -> Result<String> {
        let mut pins = Vec::new();
        for path in p.credentials.values().chain(
            match &p.adapter {
                Adapter::Boat { token_file, .. } => Some(token_file),
                Adapter::Gce {
                    credential_file, ..
                } => Some(credential_file),
                _ => None,
            }
            .into_iter(),
        ) {
            let bytes = read_private(path, 1024 * 1024)?;
            pins.push(json!({"file":path,"digest":digest(&bytes)}));
        }
        if let Adapter::Gce { pool_file, .. } = &p.adapter {
            pins.push(json!({"file":pool_file,"digest":digest(&read_private(pool_file,65536)?)}));
        }
        Ok(digest(&encoded(
            &json!({"profile":p,"credential_files":pins}),
        )?))
    }
    fn driver_matches(&self, profile: &str, revision: &str) -> bool {
        self.0.drivers.lock().is_ok_and(|drivers| {
            drivers
                .get(profile)
                .is_some_and(|driver| driver.revision == revision)
        })
    }
    fn admitted(&self, device: &str, a: &dto::Admission) -> Result<Profile> {
        a.validate().map_err(|e| e.code)?;
        let p = self.profile(device, &a.workspace, &a.project, &a.profile)?;
        if Self::profile_revision(&p)? != a.profile_revision || p.source_digest != a.source_digest {
            return Err(Code::Stale);
        }
        if let Some(job) = &a.job {
            let saved = self.job_admission(job)?;
            if saved.admission.workspace != a.workspace
                || saved.admission.project != a.project
                || saved.admission.profile != a.profile
                || saved.admission.profile_revision != a.profile_revision
                || saved.admission.source_digest != a.source_digest
            {
                return Err(Code::Forbidden);
            }
            let r = self.store()?.read(job).map_err(|_| Code::Unavailable)?;
            let expected = spec(&p, &r.spec.task, r.spec.timeout_seconds);
            if r.spec != expected || r.spec.timeout_seconds > p.max_timeout_seconds {
                return Err(Code::Stale);
            }
            let source = r.workspace.as_ref().ok_or(Code::Stale)?;
            if source.revision != p.source_revision
                || source.caller_revision != p.source_revision
                || source.paths != p.paths
                || source.included != p.include
                || source.input_digest != saved.input_digest
                || source
                    .source_root
                    .join(&source.working_directory)
                    .canonicalize()
                    .ok()
                    != p.cwd.canonicalize().ok()
                || digest(&encoded(
                    &json!({"revision":p.source_revision,"changes":source.caller_changes_digest}),
                )?) != p.source_digest
            {
                return Err(Code::Stale);
            }
            source.input().map_err(|_| Code::Stale)?;
        }
        Ok(p)
    }
    /// The project's selected environment version for a new job, read once
    /// from `<state>/environments` (ENV-06). The job keeps this exact pin;
    /// existing and queued jobs never re-read it. A selected version only
    /// changes the image of a job the operator policy already admits.
    fn selected_environment(
        &self,
        a: &dto::Admission,
        p: &Profile,
    ) -> Result<Option<coder_environment::VersionPin>> {
        let root = self.0.root.join("environments");
        if fs::symlink_metadata(&root).is_err() {
            return Ok(None);
        }
        private(&root, true)?;
        let pin = coder_environment::store::Store::under(root)
            .selected(&coder_environment::ProjectLink {
                workspace: a.workspace.clone(),
                project: a.project.clone(),
            })
            .map_err(|e| match e {
                coder_environment::store::StoreError::Ambiguous => Code::Conflict,
                _ => Code::Unavailable,
            })?;
        if let Some(pin) = &pin {
            // Saved images are Boat images that carry the Coder runtime.
            if p.placement != Placement::Boat
                || p.mode != Mode::Coder
                || pin.image.provider != coder_environment::Provider::Boat
            {
                return Err(Code::Unsupported);
            }
        }
        Ok(pin)
    }
    fn job_admission(&self, id: &str) -> Result<AdmittedJob> {
        dto::alias(id).map_err(|e| e.code)?;
        serde_json::from_slice(&read_private(
            &self.0.root.join("admissions").join(format!("{id}.json")),
            32 * 1024,
        )?)
        .map_err(|_| Code::Malformed)
    }
    fn record(
        &self,
        device: &str,
        workspace: &str,
        project: &str,
        id: &str,
    ) -> Result<(Record, AdmittedJob, Vec<u8>)> {
        let a = self.job_admission(id)?;
        if a.admission.workspace != workspace || a.admission.project != project {
            return Err(Code::Forbidden);
        }
        self.admitted(device, &a.admission)?;
        let store = self.store()?;
        let record = store.read(id).map_err(|_| Code::Unavailable)?;
        let bytes = read_private(
            &store.root().join(format!("{id}.json")),
            crate::MAX_RECORD_BYTES,
        )?;
        if encoded(&record)? != bytes {
            return Err(Code::Malformed);
        }
        Ok((record, a, bytes))
    }
    fn scope(&self, r: &Record, a: &AdmittedJob, bytes: &[u8]) -> Result<dto::Scope> {
        let cancelled = self.cancellation_bytes(&r.id)?;
        Ok(dto::Scope {
            workspace: a.admission.workspace.clone(),
            project: a.admission.project.clone(),
            job: r.id.clone(),
            revision: digest(&encoded(
                &json!({"record":digest(bytes),"cancellation":cancelled.as_ref().map(|b|digest(b))}),
            )?),
            attempt: r.turns.len() as u64 + 1,
            profile: a.admission.profile.clone(),
            profile_revision: a.admission.profile_revision.clone(),
            source_digest: a.admission.source_digest.clone(),
        })
    }
    fn cancellation_bytes(&self, id: &str) -> Result<Option<Vec<u8>>> {
        dto::alias(id).map_err(|e| e.code)?;
        let path = self.0.root.join("jobs").join(format!("{id}.cancel"));
        match fs::symlink_metadata(&path) {
            Ok(_) => read_private(&path, 16 * 1024).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(Code::Unavailable),
        }
    }
    fn exact(&self, device: &str, s: &dto::Scope) -> Result<(Record, AdmittedJob, Vec<u8>)> {
        let (r, a, b) = self.record(device, &s.workspace, &s.project, &s.job)?;
        if self.scope(&r, &a, &b)? != *s {
            return Err(Code::Stale);
        }
        Ok((r, a, b))
    }
    fn original(source: &str, bytes: &[u8], media_type: &str) -> dto::Original {
        dto::Original {
            source: source.into(),
            digest: digest(bytes),
            bytes: bytes.len() as u64,
            media_type: media_type.into(),
        }
    }
    fn job(&self, device: &str, q: &dto::ReadQuery) -> Result<dto::Job> {
        let (r, a, b) = self.record(device, &q.workspace, &q.project, &q.job)?;
        let scope = self.scope(&r, &a, &b)?;
        if q.revision.as_ref().is_some_and(|v| *v != scope.revision) {
            return Err(Code::Stale);
        }
        let p = self.admitted(device, &a.admission)?;
        let damaged_artifacts = r.artifacts.as_ref().is_some_and(|manifest| {
            manifest.files.iter().any(|item| {
                self.original_bytes(&r, &format!("artifact:{}", item.name), &b)
                    .is_err()
            })
        });
        let mut originals = vec![Self::original("record", &b, "application/json")];
        if let Some(bytes) = self.cancellation_bytes(&r.id)? {
            originals.push(Self::original("cancellation", &bytes, "application/json"));
        }
        let mut omitted_originals = false;
        for turn in &r.turns {
            if let Some(items) = turn.get("originals").and_then(Value::as_array) {
                for item in items {
                    let original: dto::Original =
                        serde_json::from_value(item.clone()).map_err(|_| Code::Malformed)?;
                    original.validate().map_err(|e| e.code)?;
                    if originals.len() < 48 {
                        originals.push(original);
                    } else {
                        omitted_originals = true;
                    }
                }
            }
        }
        for (source, value) in [
            ("events", json!(r.events)),
            ("turns", json!(r.turns)),
            ("result", json!(r.result)),
            ("usage", json!(r.usage)),
        ] {
            originals.push(Self::original(
                source,
                &encoded(&value)?,
                "application/json",
            ));
        }
        if let Some(artifacts) = &r.artifacts {
            for item in &artifacts.files {
                if matches!(
                    item.name.as_str(),
                    "changes.patch" | "events.ndjson" | "trajectory.atif.json" | "result.json"
                ) {
                    originals.push(dto::Original {
                        source: format!("artifact:{}", item.name),
                        digest: format!("sha256:{}", item.digest),
                        bytes: item.bytes as u64,
                        media_type: if item.name == "changes.patch" {
                            "text/x-diff"
                        } else {
                            "application/json"
                        }
                        .into(),
                    });
                }
            }
        }
        let prompt_omitted = r.spec.task.len() > 16384;
        let error = r.error.as_ref().filter(|s| s.len() <= 4096).cloned();
        let usage = r
            .usage
            .clone()
            .filter(|v| encoded(v).is_ok_and(|b| b.len() <= 8192));
        let state = serde_json::to_value(r.state)
            .map_err(|_| Code::Unavailable)?
            .as_str()
            .unwrap()
            .to_owned();
        let mut job = dto::Job {
            scope,
            state,
            placement: if r.spec.placement == Placement::Boat {
                "boat"
            } else {
                "gce"
            }
            .into(),
            mode: if r.spec.mode == Mode::Integrated {
                "integrated"
            } else {
                "coder"
            }
            .into(),
            executor: r.spec.agent.clone(),
            model: r.spec.model.clone(),
            served_model: r
                .result
                .as_ref()
                .and_then(|v| v.get("model"))
                .and_then(Value::as_str)
                .filter(|s| s.len() <= 128)
                .map(str::to_owned),
            pool: p.pool,
            prompt: if prompt_omitted {
                String::new()
            } else {
                r.spec.task.clone()
            },
            prompt_omitted,
            credential_names: r.spec.credential_names.clone(),
            remote_task: r.remote_task.as_ref().map(|t| t.id.clone()),
            continuation: if r.state.terminal() && r.cleanup_complete && r.resource.is_some() {
                "available"
            } else {
                "unavailable"
            }
            .into(),
            cancellation: if r.cancel_requested
                || self
                    .store()?
                    .cancellation_requested(&r.id)
                    .map_err(|_| Code::Unavailable)?
            {
                "requested"
            } else {
                "not_requested"
            }
            .into(),
            cleanup: if r.cleanup_complete {
                "confirmed"
            } else if r.cleanup_error.is_some() {
                "unresolved"
            } else {
                "unknown"
            }
            .into(),
            artifact_state: if damaged_artifacts {
                "damaged"
            } else if r.artifacts.is_some() {
                "retained"
            } else if r.artifact_error.is_some() {
                "unresolved"
            } else {
                "unknown"
            }
            .into(),
            usage,
            error,
            details_omitted: omitted_originals
                || prompt_omitted
                || (r.usage.is_some()
                    && r.usage
                        .as_ref()
                        .is_some_and(|v| encoded(v).is_ok_and(|b| b.len() > 8192)))
                || r.error.as_ref().is_some_and(|s| s.len() > 4096),
            originals,
            environment: r.environment.as_ref().map(environment_panel::pin),
        };
        if job.validate().is_err() {
            job.usage = None;
            job.error = None;
            job.details_omitted = true;
        }
        job.validate().map_err(|e| e.code)?;
        Ok(job)
    }
    fn original_bytes(&self, r: &Record, source: &str, b: &[u8]) -> Result<Vec<u8>> {
        if source.starts_with("attempt:") {
            let parts = source.split(':').collect::<Vec<_>>();
            let attempt = parts
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|n| *n > 0 && *n <= r.turns.len())
                .ok_or(Code::Forbidden)?;
            let advertised = r.turns[attempt - 1]
                .get("originals")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find(|o| o.get("source").and_then(Value::as_str) == Some(source))
                })
                .ok_or(Code::Forbidden)?;
            let original: dto::Original =
                serde_json::from_value(advertised.clone()).map_err(|_| Code::Malformed)?;
            let name = match parts.as_slice() {
                ["attempt", _, "record"] => "record.json",
                ["attempt", _, "cancellation"] => "cancellation.json",
                ["attempt", _, "artifact", name]
                    if matches!(
                        *name,
                        "changes.patch" | "events.ndjson" | "trajectory.atif.json" | "result.json"
                    ) =>
                {
                    name
                }
                _ => return Err(Code::Forbidden),
            };
            let bytes = read_private(
                &self
                    .0
                    .root
                    .join("archives")
                    .join(&r.id)
                    .join(attempt.to_string())
                    .join(name),
                crate::MAX_RECORD_BYTES,
            )?;
            if bytes.len() as u64 != original.bytes || digest(&bytes) != original.digest {
                return Err(Code::Stale);
            }
            return Ok(bytes);
        }
        match source {
            "cancellation" => self.cancellation_bytes(&r.id)?.ok_or(Code::Unavailable),
            "record" => Ok(b.to_vec()),
            "events" => encoded(&r.events),
            "turns" => encoded(&r.turns),
            "result" => encoded(&r.result),
            "usage" => encoded(&r.usage),
            _ => {
                let name = source.strip_prefix("artifact:").ok_or(Code::Forbidden)?;
                if !matches!(
                    name,
                    "changes.patch" | "events.ndjson" | "trajectory.atif.json" | "result.json"
                ) {
                    return Err(Code::Forbidden);
                }
                let manifest = r.artifacts.as_ref().ok_or(Code::Unavailable)?;
                let item = manifest
                    .files
                    .iter()
                    .find(|f| f.name == name)
                    .ok_or(Code::Forbidden)?;
                let bytes = read_private(
                    &self
                        .store()?
                        .root()
                        .join(format!("{}.artifacts", r.id))
                        .join(name),
                    32 * 1024 * 1024,
                )?;
                if bytes.len() != item.bytes || workspace::digest(&bytes) != item.digest {
                    return Err(Code::Stale);
                }
                Ok(bytes)
            }
        }
    }
    fn archive(&self, record: &Record) -> Result<Vec<dto::Original>> {
        let attempt = record.turns.len() + 1;
        let root = self.0.root.join("archives");
        directory(&root)?;
        let root = root.join(&record.id);
        directory(&root)?;
        let root = root.join(attempt.to_string());
        directory(&root)?;
        let bytes = encoded(record)?;
        write(&root.join("record.json"), &bytes)?;
        let mut originals = vec![Self::original(
            &format!("attempt:{attempt}:record"),
            &bytes,
            "application/json",
        )];
        if let Some(cancellation) = self.cancellation_bytes(&record.id)? {
            write(&root.join("cancellation.json"), &cancellation)?;
            originals.push(Self::original(
                &format!("attempt:{attempt}:cancellation"),
                &cancellation,
                "application/json",
            ));
        }
        if let Some(manifest) = &record.artifacts {
            for item in &manifest.files {
                if !matches!(
                    item.name.as_str(),
                    "changes.patch" | "events.ndjson" | "trajectory.atif.json" | "result.json"
                ) {
                    return Err(Code::Malformed);
                }
                let bytes = self.original_bytes(record, &format!("artifact:{}", item.name), &[])?;
                write(&root.join(&item.name), &bytes)?;
                originals.push(Self::original(
                    &format!("attempt:{attempt}:artifact:{}", item.name),
                    &bytes,
                    "application/octet-stream",
                ));
            }
        }
        Ok(originals)
    }
    fn start(&self, profile: &str, record: Record, a: AdmittedJob) {
        let driver = self.0.drivers.lock().ok().and_then(|d| {
            d.get(profile)
                .filter(|d| d.revision == a.admission.profile_revision)
                .map(|d| d.driver.clone())
        });
        let Some(driver) = driver else { return };
        let owner = self.clone();
        let check_owner = owner.clone();
        let principal = a.principal.clone();
        let admission = a.admission.clone();
        let check: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || {
            (check_owner.0.authority)(&principal)
                && check_owner.admitted(&principal.device, &admission).is_ok()
                && check_owner.driver_matches(&admission.profile, &admission.profile_revision)
        });
        std::thread::spawn(move || {
            if let Ok(store) = owner.store() {
                let _ = driver.run(store, record, check);
            }
        });
    }
    /// Hold the user's released Claude credential, in memory only, for the
    /// next effect of the same job from the same device (BYO-05). Only a
    /// Claude Code profile that names no own credential takes one, and an
    /// existing job takes one only from the device that submitted it and
    /// the owner it first ran under.
    fn release(&self, principal: &Principal, intent: &dto::Release) -> Result<dto::Released> {
        let _serial = self.0.serial.lock().map_err(|_| Code::Unavailable)?;
        let p = self.admitted(&principal.device, &intent.admission())?;
        if p.executor != crate::claude::ENGINE
            || p.mode != Mode::Coder
            || crate::claude::sign_in(p.credentials.keys().map(String::as_str))
                != crate::claude::SignIn::PlanLogin
        {
            return Err(Code::Unsupported);
        }
        let path = self
            .0
            .root
            .join("admissions")
            .join(format!("{}.json", intent.job));
        if fs::symlink_metadata(&path).is_ok() {
            let saved = self.job_admission(&intent.job)?;
            if saved.principal.device != principal.device
                || saved.admission.workspace != intent.workspace
                || saved.admission.project != intent.project
                || saved.admission.profile != intent.profile
                || saved.owner.as_ref().is_some_and(|o| *o != intent.owner)
            {
                return Err(Code::Forbidden);
            }
        }
        let (class, credentials) = crate::release::credentials(&intent.name, &intent.value)
            .map_err(|_| Code::Malformed)?;
        let expires_at = crate::release::offer(
            &intent.job,
            &principal.device,
            &intent.owner,
            class,
            credentials,
            crate::now_ms() / 1000,
        );
        Ok(dto::Released {
            job: intent.job.clone(),
            name: intent.name.clone(),
            expires_at,
        })
    }
    fn effect(
        &self,
        request: &str,
        principal: &Principal,
        op: &Operation,
    ) -> Result<dto::Accepted> {
        let _serial = self.0.serial.lock().map_err(|_| Code::Unavailable)?;
        let lock_path = self.0.root.join("operator.lock");
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        if lock_path.exists() {
            private(&lock_path, false)?;
        }
        let lock = options.open(&lock_path).map_err(|_| Code::Unavailable)?;
        lock.lock().map_err(|_| Code::Unavailable)?;
        dto::alias(request).map_err(|e| e.code)?;
        let mut admission = dto::Admission::for_operation(op).ok_or(Code::Unsupported)?;
        let p = self.admitted(&principal.device, &admission)?;
        let digest = digest(&encoded(op)?);
        let path = self.0.root.join("requests").join(format!("{request}.json"));
        if path.exists() {
            let prior: Effect = serde_json::from_slice(&read_private(&path, 32 * 1024)?)
                .map_err(|_| Code::Malformed)?;
            if prior.principal != *principal
                || prior.digest != digest
                || prior.admission != admission
            {
                return Err(Code::Conflict);
            }
            return prior.accepted.ok_or(Code::Unavailable);
        }
        if !self.driver_matches(&admission.profile, &admission.profile_revision) {
            return Err(Code::Unavailable);
        }
        let store = self.store()?;
        let job = match op {
            Operation::CloudSubmit { .. } => request.to_owned(),
            _ => admission.job.clone().ok_or(Code::Malformed)?,
        };
        if let Operation::CloudCancel { intent } = op {
            let (record, a, bytes) = self.exact(&principal.device, &intent.scope)?;
            let mut journal = Effect {
                principal: principal.clone(),
                admission,
                digest,
                accepted: None,
            };
            write(&path, &encoded(&journal)?)?;
            store
                .cancel_exact_evidence(&job, &workspace::digest(&bytes),&json!({"schema":"openagents.coder.operator-cancel.v1","request":request,"scope":intent.scope,"reason":intent.reason}))
                .map_err(|_| Code::Stale)?;
            let accepted = dto::Accepted {
                request: request.into(),
                scope: self.scope(&record, &a, &bytes)?,
                action: "cancel".into(),
                state: "cancellation_requested".into(),
            };
            journal.accepted = Some(accepted.clone());
            write(&path, &encoded(&journal)?)?;
            self.start(&a.admission.profile.clone(), record, a);
            return Ok(accepted);
        }
        // A credential the user's server released for this job serves this
        // effect or none (BYO-05).
        let offer = (p.executor == crate::claude::ENGINE)
            .then(|| crate::release::claim(&job, &principal.device, crate::now_ms() / 1000))
            .flatten();
        let saved_owner = match op {
            Operation::CloudSubmit { .. } => None,
            _ => self.job_admission(&job)?.owner,
        };
        if let Some(offer) = &offer {
            if saved_owner.as_ref().is_some_and(|o| *o != offer.owner) {
                return Err(Code::Forbidden);
            }
        }
        // Profiles that name no own credential take the user's released one.
        let released = p.executor == crate::claude::ENGINE
            && p.mode == Mode::Coder
            && crate::claude::sign_in(p.credentials.keys().map(String::as_str))
                == crate::claude::SignIn::PlanLogin;
        let paused = match op {
            Operation::CloudFollow { intent } => {
                self.exact(&principal.device, &intent.scope)?.0.state == State::Paused
            }
            _ => false,
        };
        // The class the next turn runs on, when this effect starts one.
        let turn = (released
            && (paused
                || matches!(
                    op,
                    Operation::CloudSubmit { .. } | Operation::CloudContinue { .. }
                )))
        .then(|| {
            offer
                .as_ref()
                .map_or(crate::claude::SignIn::PlanLogin, |o| {
                    crate::claude::SignIn::Own(o.class)
                })
        });
        if matches!(
            op,
            Operation::CloudSubmit { .. } | Operation::CloudContinue { .. }
        ) && p.executor == crate::claude::ENGINE
            || turn.is_some()
        {
            // A Claude plan login runs one automated turn at a time; parallel
            // work needs the user's own key or cloud credential (BYO-04).
            let sign_in = turn.unwrap_or_else(|| {
                crate::claude::sign_in(p.credentials.keys().map(String::as_str))
            });
            let active = store
                .list()
                .map_err(|_| Code::Unavailable)?
                .iter()
                .filter(|r| {
                    r.id != job
                        && !r.state.terminal()
                        && crate::claude_task::turn_sign_in(r) == crate::claude::SignIn::PlanLogin
                        && self
                            .job_admission(&r.id)
                            .is_ok_and(|a| a.admission.profile == admission.profile)
                })
                .count();
            crate::claude::admit_turns(sign_in, active, 1).map_err(|_| Code::Conflict)?;
        }
        let lease = store.lease(&job).map_err(|_| Code::Conflict)?;
        let mut record = match op {
            Operation::CloudSubmit { intent } => {
                if intent.timeout_seconds > p.max_timeout_seconds || lease.exists() {
                    return Err(Code::Conflict);
                }
                let mut r = Record::new(&job, spec(&p, &intent.prompt, intent.timeout_seconds))
                    .map_err(|_| Code::Malformed)?;
                r.environment = self.selected_environment(&admission, &p)?;
                r
            }
            Operation::CloudContinue { intent } => {
                let (r, _, _) = self.exact(&principal.device, &intent.scope)?;
                if !r.state.terminal() || !r.cleanup_complete || r.resource.is_none() {
                    return Err(Code::Conflict);
                }
                r
            }
            Operation::CloudCancel { intent } => self.exact(&principal.device, &intent.scope)?.0,
            Operation::CloudFollow { intent } => self.exact(&principal.device, &intent.scope)?.0,
            _ => return Err(Code::Unsupported),
        };
        if matches!(op, Operation::CloudSubmit { .. }) {
            // Engine, pinned version, and credential type: never a credential.
            crate::claude_task::admit(&mut record, &admission.profile);
        }
        if let Some(turn) = turn {
            crate::claude_task::set_turn_sign_in(&mut record, turn);
        }
        let mut journal = Effect {
            principal: principal.clone(),
            admission: admission.clone(),
            digest: digest.clone(),
            accepted: None,
        };
        write(&path, &encoded(&journal)?)?;
        let action = match op {
            Operation::CloudSubmit { .. } => {
                let snapshot = workspace::capture(
                    &lease,
                    &p.cwd,
                    Some(&p.source_revision),
                    p.paths.clone(),
                    p.include.clone(),
                )
                .map_err(|_| Code::Stale)?;
                if workspace::source_identity(&p.cwd, &p.source_revision, &p.paths, &p.include)
                    .map_err(|_| Code::Stale)?
                    != p.source_digest
                {
                    return Err(Code::Stale);
                }
                record.workspace = Some(snapshot);
                "submit"
            }
            Operation::CloudContinue { intent } => {
                let originals = self.archive(&record)?;
                record.binding["continue_conversation"] = json!(
                    record
                        .remote_task
                        .as_ref()
                        .and_then(|t| t.conversation.clone())
                );
                record.binding["turn_start"] = json!(record.events.len());
                record.turns.push(json!({"task":record.spec.task,"result":record.result,"state":record.state,"usage":record.usage,"artifacts":record.artifacts,"originals":originals}));
                record
                    .events
                    .push(json!({"event":"user","text":intent.prompt}));
                record.spec.task = intent.prompt.clone();
                record.state = State::Resuming;
                record.created_ms = crate::now_ms();
                record.remote_task = None;
                if record.spec.mode == Mode::Coder {
                    record.cursor = None;
                }
                record.result = None;
                record.error = None;
                record.cancel_requested = false;
                record.cleanup_complete = false;
                record.cleanup_error = None;
                record.artifacts = None;
                record.artifact_error = None;
                lease.clear_cancel().map_err(|_| Code::Unavailable)?;
                "continue"
            }
            Operation::CloudCancel { .. } => {
                record.cancel_requested = true;
                "cancel"
            }
            Operation::CloudFollow { .. } => "follow",
            _ => return Err(Code::Unsupported),
        };
        record.updated_ms = crate::now_ms();
        lease.save(&record).map_err(|_| Code::Unavailable)?;
        admission.job = Some(job.clone());
        let a = if action == "submit" {
            let a = AdmittedJob {
                principal: principal.clone(),
                admission,
                request_digest: digest,
                input_digest: record
                    .workspace
                    .as_ref()
                    .ok_or(Code::Stale)?
                    .input_digest
                    .clone(),
                owner: offer.as_ref().map(|o| o.owner.clone()),
            };
            write(
                &self.0.root.join("admissions").join(format!("{job}.json")),
                &encoded(&a)?,
            )?;
            a
        } else {
            let mut a = self.job_admission(&job)?;
            if a.owner.is_none() {
                if let Some(o) = &offer {
                    // The first release into this job pins its owner.
                    a.owner = Some(o.owner.clone());
                    write(
                        &self.0.root.join("admissions").join(format!("{job}.json")),
                        &encoded(&a)?,
                    )?;
                }
            }
            a
        };
        // Arm the turn this effect starts with the released credential, or
        // leave it on the plan login. A follow of a running turn re-arms it
        // only with the same class.
        match (turn, &offer) {
            (Some(crate::claude::SignIn::Own(_)), Some(o)) => {
                crate::release::arm(&job, o.credentials.clone());
            }
            (Some(crate::claude::SignIn::PlanLogin), _) => crate::release::disarm(&job),
            (None, Some(o))
                if released
                    && crate::claude_task::turn_sign_in(&record)
                        == crate::claude::SignIn::Own(o.class) =>
            {
                crate::release::arm(&job, o.credentials.clone());
            }
            _ => {}
        }
        let accepted = dto::Accepted {
            request: request.into(),
            scope: self.scope(&record, &a, &encoded(&record)?)?,
            action: action.into(),
            state: "accepted".into(),
        };
        accepted.validate().map_err(|e| e.code)?;
        journal.accepted = Some(accepted.clone());
        write(&path, &encoded(&journal)?)?;
        drop(lease);
        self.start(&a.admission.profile.clone(), record, a);
        Ok(accepted)
    }
}
impl coder_host::cloud::Cloud for Operator {
    fn admit_recovery(&self, device: &str, a: &dto::Admission) -> Result<()> {
        self.admitted(device, a).map(|_| ())
    }
    fn execute(&self, request: &str, principal: &Principal, op: &Operation) -> Result<Outcome> {
        op.validate().map_err(|e| e.code)?;
        let device = &principal.device;
        let outcome = match op {
            Operation::CloudProjects { workspace } => {
                let p = self.0.policy.read()?;
                let mut projects = p
                    .operators
                    .iter()
                    .filter(|a| a.device == *device && a.workspace == *workspace)
                    .map(|a| a.project.clone())
                    .collect::<Vec<_>>();
                projects.sort();
                projects.dedup();
                if projects.is_empty() {
                    return Err(Code::Forbidden);
                }
                Outcome::CloudProjects {
                    projects: dto::Projects {
                        workspace: workspace.clone(),
                        digest: digest(&encoded(&projects)?),
                        projects,
                    },
                }
            }
            Operation::CloudCatalog { query } => {
                let policy = self.0.policy.read()?;
                let mut profiles = Vec::new();
                for (name, _) in &policy.profiles {
                    if let Ok(p) = self.profile(device, &query.workspace, &query.project, name) {
                        let ready = self.driver_matches(name, &Self::profile_revision(&p)?);
                        profiles.push(dto::Profile {
                            name: name.clone(),
                            revision: Self::profile_revision(&p)?,
                            source_revision: p.source_revision,
                            source_digest: p.source_digest,
                            repository: p.repository,
                            branch: p.branch,
                            template: p.template,
                            size: p.size,
                            placement: if p.placement == Placement::Boat {
                                "boat"
                            } else {
                                "gce"
                            }
                            .into(),
                            pool: p.pool,
                            mode: if p.mode == Mode::Integrated {
                                "integrated"
                            } else {
                                "coder"
                            }
                            .into(),
                            executor: p.executor,
                            model: p.model,
                            credential_names: p.credentials.keys().cloned().collect(),
                            max_timeout_seconds: p.max_timeout_seconds,
                            availability: if ready { "configured" } else { "unavailable" }.into(),
                        });
                    }
                }
                if profiles.is_empty() {
                    return Err(Code::Forbidden);
                }
                Outcome::CloudCatalog {
                    catalog: dto::Catalog {
                        workspace: query.workspace.clone(),
                        project: query.project.clone(),
                        profiles,
                    },
                }
            }
            Operation::CloudList { query } => {
                let policy = self.0.policy.read()?;
                if !policy.operators.iter().any(|a| {
                    a.device == *device
                        && a.workspace == query.workspace
                        && a.project == query.project
                }) {
                    return Err(Code::Forbidden);
                }
                let mut rows = Vec::new();
                let dir = self.0.root.join("admissions");
                if dir.exists() {
                    private(&dir, true)?;
                    for item in fs::read_dir(dir).map_err(|_| Code::Unavailable)? {
                        let path = item.map_err(|_| Code::Unavailable)?.path();
                        if path.extension().is_none_or(|s| s != "json") {
                            continue;
                        }
                        let id = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .ok_or(Code::Malformed)?;
                        let a = self.job_admission(id)?;
                        if a.admission.workspace == query.workspace
                            && a.admission.project == query.project
                        {
                            let j = self.job(
                                device,
                                &dto::ReadQuery {
                                    workspace: query.workspace.clone(),
                                    project: query.project.clone(),
                                    job: id.into(),
                                    revision: None,
                                },
                            )?;
                            rows.push(dto::Row {
                                scope: j.scope,
                                state: j.state,
                                executor: j.executor,
                                model: j.model,
                                cleanup: j.cleanup,
                            });
                        }
                    }
                }
                rows.sort_by(|a, b| a.scope.job.cmp(&b.scope.job));
                let pin = digest(&encoded(&rows)?);
                let start = query.cursor.as_ref().map_or(0, |c| c.next as usize);
                if query.cursor.as_ref().is_some_and(|c| c.digest != pin) || start > rows.len() {
                    return Err(Code::Stale);
                }
                let end = (start + query.limit as usize).min(rows.len());
                Outcome::CloudList {
                    jobs: dto::List {
                        workspace: query.workspace.clone(),
                        project: query.project.clone(),
                        digest: pin.clone(),
                        rows: rows[start..end].to_vec(),
                        next: (end < rows.len()).then(|| dto::ListCursor {
                            workspace: query.workspace.clone(),
                            project: query.project.clone(),
                            digest: pin,
                            next: end as u64,
                        }),
                        more_available: end < rows.len(),
                    },
                }
            }
            Operation::CloudRead { query } => Outcome::CloudRead {
                job: Box::new(self.job(device, query)?),
            },
            Operation::CloudOriginal { query } => {
                use base64::Engine;
                let (r, _, b) = self.exact(device, &query.scope)?;
                let current = self.job(
                    device,
                    &dto::ReadQuery {
                        workspace: query.scope.workspace.clone(),
                        project: query.scope.project.clone(),
                        job: query.scope.job.clone(),
                        revision: Some(query.scope.revision.clone()),
                    },
                )?;
                let advertised = current
                    .originals
                    .into_iter()
                    .find(|o| o.source == query.original.source)
                    .or_else(|| {
                        r.turns
                            .iter()
                            .filter_map(|t| t.get("originals").and_then(Value::as_array))
                            .flatten()
                            .filter_map(|v| serde_json::from_value::<dto::Original>(v.clone()).ok())
                            .find(|o| o.source == query.original.source)
                    })
                    .ok_or(Code::Forbidden)?;
                if advertised != query.original {
                    return Err(Code::Stale);
                }
                let bytes = self.original_bytes(&r, &query.original.source, &b)?;
                if Self::original(&query.original.source, &bytes, &query.original.media_type)
                    != query.original
                {
                    return Err(Code::Stale);
                }
                let start = query.cursor.as_ref().map_or(0, |c| c.next_byte as usize);
                if start > bytes.len()
                    || query
                        .cursor
                        .as_ref()
                        .is_some_and(|c| c.prefix_digest != digest(&bytes[..start]))
                {
                    return Err(Code::Stale);
                }
                let end = (start + query.limit as usize).min(bytes.len());
                Outcome::CloudOriginal {
                    chunk: dto::OriginalChunk {
                        scope: query.scope.clone(),
                        original: query.original.clone(),
                        start: start as u64,
                        data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
                        next: (end < bytes.len()).then(|| dto::OriginalCursor {
                            scope: query.scope.clone(),
                            source: query.original.source.clone(),
                            digest: query.original.digest.clone(),
                            next_byte: end as u64,
                            prefix_digest: digest(&bytes[..end]),
                        }),
                        more_available: end < bytes.len(),
                    },
                }
            }
            Operation::CloudSubmit { .. }
            | Operation::CloudContinue { .. }
            | Operation::CloudCancel { .. }
            | Operation::CloudFollow { .. } => Outcome::CloudAccepted {
                accepted: self.effect(request, principal, op)?,
            },
            Operation::CloudRelease { intent } => Outcome::CloudReleased {
                released: self.release(principal, intent)?,
            },
            Operation::EnvironmentRead { query } => Outcome::EnvironmentRead {
                view: Box::new(self.environment_read(device, query)?),
            },
            Operation::EnvironmentEvidence { query } => Outcome::EnvironmentEvidence {
                page: Box::new(self.environment_evidence(device, query)?),
            },
            Operation::EnvironmentPromote { .. }
            | Operation::EnvironmentSelect { .. }
            | Operation::EnvironmentSteer { .. } => Outcome::EnvironmentAccepted {
                accepted: {
                    let _serial = self.0.serial.lock().map_err(|_| Code::Unavailable)?;
                    self.environment_effect(request, principal, op)?
                },
            },
            _ => return Err(Code::Unsupported),
        };
        outcome.validate().map_err(|e| e.code)?;
        if !outcome.answers(op) {
            return Err(Code::Malformed);
        }
        Ok(outcome)
    }
}

#[path = "operator_environment.rs"]
mod environment_panel;
pub use environment_panel::{SetupSessions, VERIFY_EVIDENCE};

#[cfg(test)]
#[path = "operator_tests.rs"]
mod tests;
