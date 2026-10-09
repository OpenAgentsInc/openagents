//! Repository environment owners packaged beside the resident operator
//! (ENV-08).
//!
//! The setup (ENV-03), build (ENV-04), and verify (ENV-05) owners run as
//! one service next to the coder-cloud operator. They share the
//! operator's private state directory ([`Layout`]), so the operator's
//! environment panel (ENV-07) reads the same records they write:
//!
//! | Path under the operator state | Owner | Contents |
//! | --- | --- | --- |
//! | `environments/` | ENV-01 | environment records the operator admits jobs from |
//! | `environment-setup/sessions/` | setup | setup session records |
//! | `environment-setup/evidence/<session>/<segment>` | setup | setup tool evidence |
//! | `environment-setup/blobs/<digest>` | setup | install scripts the setup wrote |
//! | `environment-build/jobs/` | build | build jobs |
//! | `environment-build/evidence/<job>/<segment>` | build | build evidence |
//! | `environment-verify/jobs/` | verify | verification jobs |
//! | `environment-verify/evidence/<job>` | verify | run evidence the panel pages ([`coder_cloud::operator::VERIFY_EVIDENCE`]) |
//! | `environment-verify/artifacts/<digest>` | verify | protected check plans, check scripts, and sealed install scripts |
//! | `environment-computers/` | all | setup, builder, and verifier computer records |
//!
//! [`attach`] composes the setup owner with the operator through
//! [`coder_cloud::operator::Operator::with_setup`], so the panel reads and
//! steers setup sessions instead of showing setup as unavailable, and
//! starts the owners' loop ([`Service`]). The loop wakes a session the
//! panel steered ([`coder_environment_setup::service::Setup::resume`]),
//! and on start and on every tick runs [`Owners::recover`]: it advances
//! builds and verifications that are not finished or whose machines are
//! not confirmed deleted, times out setup commands past their deadline,
//! wakes a session whose steering arrived while the owner was down, and
//! retries the cleanup of ended sessions. Every record is on disk, so a
//! restarted operator recovers the same sessions, jobs, and versions; an
//! owner restart that loses a live evidence recorder discloses it (a new
//! setup or build segment, an incomplete verification), never hides it.
//!
//! The service stops when the operator that holds it is dropped, or
//! explicitly with [`Service::stop`].

use coder_cloud::operator::{Operator, SetupSessions, VERIFY_EVIDENCE};
use coder_environment::evidence::Redactor;
use coder_environment::{digest, valid_digest};
use coder_environment_build::service::{BuildError, BuildRequest, Builder};
use coder_environment_setup::panel::Panel;
use coder_environment_setup::service::Setup;
use coder_environment_setup::transition::SetupState;
use coder_environment_verify::service::{Verifier, VerifyError, VerifyRequest};
use coder_working_computer::driver::Driver;
use coder_working_computer::provider::{Commands, Images};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod activity;
pub mod agent;
pub mod boat;
pub mod studio;

pub const SCHEMA: &str = "openagents.environment.owners.v1";
/// The default interval between recovery visits.
pub const DEFAULT_TICK_SECONDS: u64 = 15;

/// Where each owner keeps its state under the operator's private state
/// directory. See the crate documentation for the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    state: PathBuf,
}
impl Layout {
    pub fn under(state: impl Into<PathBuf>) -> Self {
        Self {
            state: state.into(),
        }
    }
    pub fn state(&self) -> &Path {
        &self.state
    }
    pub fn environments(&self) -> PathBuf {
        self.state.join("environments")
    }
    pub fn setup(&self) -> PathBuf {
        self.state.join("environment-setup")
    }
    pub fn sessions(&self) -> PathBuf {
        self.setup().join("sessions")
    }
    pub fn setup_blobs(&self) -> PathBuf {
        self.setup().join("blobs")
    }
    pub fn build(&self) -> PathBuf {
        self.state.join("environment-build")
    }
    pub fn build_jobs(&self) -> PathBuf {
        self.build().join("jobs")
    }
    pub fn verify(&self) -> PathBuf {
        self.state.join("environment-verify")
    }
    pub fn verify_jobs(&self) -> PathBuf {
        self.verify().join("jobs")
    }
    /// The verifier's run evidence, exactly where the operator's panel
    /// reads it.
    pub fn verify_evidence(&self) -> PathBuf {
        self.state.join(VERIFY_EVIDENCE)
    }
    /// The protected artifact store: written only by this package, never
    /// by a setup or build machine.
    pub fn artifacts(&self) -> PathBuf {
        self.verify().join("artifacts")
    }
    pub fn computers(&self) -> PathBuf {
        self.state.join("environment-computers")
    }

    /// Create every directory private (0700), refusing a symlink.
    fn prepare(&self) -> Result<(), String> {
        for dir in [
            self.environments(),
            self.setup(),
            self.sessions(),
            self.setup_blobs(),
            self.build(),
            self.build_jobs(),
            self.verify(),
            self.verify_jobs(),
            self.verify_evidence(),
            self.artifacts(),
            self.computers(),
        ] {
            private_dir(&dir)?;
        }
        Ok(())
    }
}

fn private_dir(dir: &Path) -> Result<(), String> {
    match fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
            return Err(format!(
                "{} must be a private directory, not a link or file.",
                dir.display()
            ));
        }
        Ok(_) => {}
        Err(_) => {
            fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Cannot protect {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Builds the redactor for a set of named credentials from operator
/// custody. Shared by the setup and build owners; values never enter a
/// record.
pub type Custody =
    Arc<dyn Fn(&BTreeSet<String>) -> Result<Redactor, String> + Send + Sync + 'static>;

/// One provider per owner. A verifier's provider must apply no
/// credentials; the setup and build providers apply only what a computer
/// names.
pub struct Providers<P> {
    pub setup: P,
    pub build: P,
    pub verify: P,
}

/// The three owners over one [`Layout`].
pub struct Owners<P> {
    pub layout: Layout,
    pub setup: Arc<Setup<P>>,
    pub builder: Arc<Builder<P>>,
    pub verifier: Arc<Verifier<P>>,
}

/// What one recovery visit did, by record ID. Errors are kept per record
/// and never stop the visit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Recovery {
    pub setup: Vec<String>,
    pub builds: Vec<String>,
    pub verifications: Vec<String>,
    pub errors: Vec<String>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl<P: Commands + Images> Owners<P> {
    /// Open the owners under `state`, the operator's private state
    /// directory, creating their private directories.
    pub fn open(state: &Path, providers: Providers<P>, custody: Custody) -> Result<Self, String> {
        let layout = Layout::under(state);
        layout.prepare()?;
        let envs = || coder_environment::store::Store::under(layout.environments());
        let computers = || coder_working_computer::store::Store::under(layout.computers());
        let setup_custody = custody.clone();
        let setup = Setup::new(
            layout.setup(),
            coder_environment_setup::store::Store::under(layout.sessions()),
            envs(),
            Driver::new(computers(), providers.setup),
            Box::new(move |names: &BTreeSet<String>| setup_custody(names)),
        );
        let builder = Builder::new(
            layout.build(),
            layout.setup_blobs(),
            coder_environment_build::store::Store::under(layout.build_jobs()),
            envs(),
            Driver::new(computers(), providers.build),
            Box::new(move |names: &BTreeSet<String>| custody(names)),
        );
        let verifier = Verifier::new(
            layout.verify(),
            layout.artifacts(),
            coder_environment_verify::store::Store::under(layout.verify_jobs()),
            envs(),
            coder_environment_build::store::Store::under(layout.build_jobs()),
            Driver::new(computers(), providers.verify),
        );
        Ok(Self {
            layout,
            setup: Arc::new(setup),
            builder: Arc::new(builder),
            verifier: Arc::new(verifier),
        })
    }

    /// Retain a protected artifact (a check plan or check script) by its
    /// digest. Only the operator writes here; machines never do.
    pub fn seal_artifact(&self, bytes: &[u8]) -> Result<String, String> {
        let d = digest(bytes);
        let path = self.layout.artifacts().join(&d);
        if fs::read(&path).is_ok_and(|b| digest(&b) == d) {
            return Ok(d);
        }
        let temp = self.layout.artifacts().join(format!("{d}.writing"));
        let mut file = fs::File::create(&temp).map_err(|_| "Cannot write an artifact.")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::rename(&temp, &path))
            .map_err(|_| "Cannot retain an artifact.")?;
        Ok(d)
    }

    /// Start (or continue) a clean build of the environment's draft.
    pub async fn build(
        &self,
        request: &BuildRequest,
        now_ms: u64,
    ) -> Result<coder_environment_build::BuildJob, BuildError> {
        self.builder.start(request, now_ms).await
    }

    /// Start (or continue) verifying a build. The build's exact install
    /// script is first copied by digest from the setup's blob store into
    /// the protected artifacts, so the idempotence fork reruns the same
    /// bytes and no setup or build machine can alter them afterwards.
    pub async fn verify(
        &self,
        request: &VerifyRequest,
        now_ms: u64,
    ) -> Result<coder_environment_verify::VerifyJob, VerifyError> {
        if let Ok(env) = self.verifier.environments.read(&request.environment)
            && let Some(build) = env.build(&request.build_id)
            && let Some(recipe) = env.recipe(build.recipe_revision)
        {
            let d = &recipe.recipe.install.digest;
            if valid_digest(d)
                && let Ok(bytes) = fs::read(self.layout.setup_blobs().join(d))
                && digest(&bytes) == *d
            {
                self.seal_artifact(&bytes)
                    .map_err(|_| VerifyError::Refused("Cannot seal the install script."))?;
            }
        }
        self.verifier.start(request, now_ms).await
    }

    /// One recovery visit over every retained record.
    pub async fn recover(&self, now_ms: u64) -> Recovery {
        let mut out = Recovery::default();
        match self.setup.sessions().list() {
            Ok(sessions) => {
                for s in sessions {
                    let r = if s.state.terminal() {
                        let deleted = self
                            .setup
                            .computers
                            .store
                            .read(&s.computer)
                            .ok()
                            .and_then(|c| c.deletion)
                            .is_some_and(|f| f.is_done());
                        if deleted {
                            continue;
                        }
                        self.setup.cleanup(&s.id, now_ms).await.map(|_| ())
                    } else if matches!(s.state, SetupState::AwaitingInput { .. })
                        && s.steering.last().is_some_and(|t| t.at_ms >= s.updated_ms)
                    {
                        // Steering retained while the owner was down.
                        self.setup.resume(&s.id, now_ms).await.map(|_| ())
                    } else {
                        self.setup.tick(&s.id, now_ms).await.map(|_| ())
                    };
                    match r {
                        Ok(()) => out.setup.push(s.id),
                        Err(e) => out.errors.push(format!("setup {}: {e}", s.id)),
                    }
                }
            }
            Err(e) => out.errors.push(format!("setup sessions: {e}")),
        }
        match self.builder.jobs.list() {
            Ok(jobs) => {
                for j in jobs {
                    let settled = j.phase.terminal()
                        && matches!(j.cleanup, coder_environment_build::Cleanup::Complete { .. });
                    if settled {
                        continue;
                    }
                    match self.builder.advance(&j.id, now_ms).await {
                        Ok(_) => out.builds.push(j.id),
                        Err(e) => out.errors.push(format!("build {}: {e}", j.id)),
                    }
                }
            }
            Err(e) => out.errors.push(format!("builds: {e}")),
        }
        match self.verifier.jobs.list() {
            Ok(jobs) => {
                for j in jobs {
                    use coder_environment_verify::Cleanup as C;
                    let settled = j.phase == coder_environment_verify::Phase::Done
                        && ![&j.baseline, &j.fork]
                            .iter()
                            .any(|m| matches!(m.cleanup, C::Requested | C::Unknown { .. }));
                    if settled {
                        continue;
                    }
                    match self.verifier.advance(&j.id, now_ms).await {
                        Ok(_) => out.verifications.push(j.id),
                        Err(e) => out.errors.push(format!("verify {}: {e}", j.id)),
                    }
                }
            }
            Err(e) => out.errors.push(format!("verifications: {e}")),
        }
        out
    }
}

/// How often the owners' loop visits retained records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadence {
    pub tick: Duration,
}
impl Default for Cadence {
    fn default() -> Self {
        Self {
            tick: Duration::from_secs(DEFAULT_TICK_SECONDS),
        }
    }
}

/// The owners' running loop. Dropping the last handle (with the operator
/// that holds it) signals it to stop; [`Service::stop`] also waits for it.
pub struct Service {
    stop: tokio::sync::watch::Sender<bool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Service {
    /// Signal the loop and wait for its current visit to finish.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
        if let Some(t) = self.thread.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
    }
    pub fn running(&self) -> bool {
        self.thread
            .lock()
            .ok()
            .is_some_and(|t| t.as_ref().is_some_and(|t| !t.is_finished()))
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

/// The panel's setup owner, holding the service so that it lives and stops
/// with the operator.
struct Packaged<P> {
    panel: Panel<P>,
    _service: Arc<Service>,
}
impl<P: Commands + Send + Sync> SetupSessions for Packaged<P> {
    fn sessions(
        &self,
        environment: &str,
    ) -> Result<Vec<coder_access::environment::Setup>, coder_access::Code> {
        self.panel.sessions(environment)
    }
    fn steer(
        &self,
        environment: &str,
        session: &str,
        text: &str,
        now_ms: u64,
    ) -> Result<String, coder_access::Code> {
        self.panel.steer(environment, session, text, now_ms)
    }
}

/// Compose the owners with `operator` and start their loop on a thread of
/// its own. Returns the composed operator and the service handle.
pub fn attach<P>(
    owners: Arc<Owners<P>>,
    operator: Operator,
    cadence: Cadence,
) -> Result<(Operator, Arc<Service>), String>
where
    P: Commands + Images + Send + Sync + 'static,
{
    let (panel, mut wake) = Panel::new(owners.setup.clone());
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| format!("The environment owners' runtime failed: {e}"))?;
    let thread = std::thread::Builder::new()
        .name("environment-owners".into())
        .spawn(move || {
            runtime.block_on(async move {
                let mut tick = tokio::time::interval(cadence.tick);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tokio::select! {
                        biased;
                        _ = stopped.changed() => break,
                        Some(id) = wake.recv() => {
                            let _ = owners.setup.resume(&id, now_ms()).await;
                        }
                        // The first tick fires at once: the restart sweep.
                        _ = tick.tick() => {
                            owners.recover(now_ms()).await;
                        }
                    }
                    if *stopped.borrow() {
                        break;
                    }
                }
            })
        })
        .map_err(|e| format!("The environment owners' loop did not start: {e}"))?;
    let service = Arc::new(Service {
        stop,
        thread: Mutex::new(Some(thread)),
    });
    let operator = operator.with_setup(Arc::new(Packaged {
        panel,
        _service: service.clone(),
    }))?;
    Ok((operator, service))
}

/// The owners' configuration document (`--environment-owners`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    /// The only provider admitted in this release.
    pub provider: ProviderKind,
    /// The checkout directory inside each machine.
    pub workdir: String,
    /// The interactive runtime template fresh setup and builder machines
    /// start from; `None` uses the provider default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Credentials, by name, that setup and build machines may be granted.
    /// Values are read from the operator's process environment at start.
    /// Verifier machines never receive any.
    #[serde(default)]
    pub credential_names: BTreeSet<String>,
    #[serde(default = "default_tick")]
    pub tick_seconds: u64,
}
fn default_tick() -> u64 {
    DEFAULT_TICK_SECONDS
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Boat,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|_| "The environment owners' config is unreadable.")?;
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("The environment owners' config is invalid: {e}"))?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("The environment owners' schema must be {SCHEMA}."));
        }
        if !self.workdir.starts_with('/') {
            return Err("The machine workdir must be absolute.".into());
        }
        if let Some(n) = self
            .credential_names
            .iter()
            .find(|n| !coder_working_computer::credential_name_allowed(n))
        {
            return Err(format!("Credential name {n} is not admitted."));
        }
        if !(1..=3600).contains(&self.tick_seconds) {
            return Err("tick_seconds must be 1 to 3600.".into());
        }
        Ok(())
    }
    pub fn cadence(&self) -> Cadence {
        Cadence {
            tick: Duration::from_secs(self.tick_seconds),
        }
    }
}

/// Custody over the operator's process environment: each named
/// credential's value is selected for redaction. A named credential with
/// no value is refused.
pub fn environment_custody() -> Custody {
    Arc::new(|names: &BTreeSet<String>| {
        let mut r = Redactor::new();
        for n in names {
            let v = std::env::var(n).map_err(|_| format!("Credential {n} has no value."))?;
            r.select(&v).map_err(|e| e.to_string())?;
        }
        Ok(r)
    })
}

#[cfg(test)]
mod tests;
