//! The live service: the Computers screens over the resident host client.
//!
//! [`Live`] implements [`ComputersService`] with `coder_host::client`, the
//! `coder-access` device client, `coder-reach` presence, and one `coder-link`
//! registry that supervises a connection per enrolled host:
//!
//! - Redeeming a `coder-host:` invitation saves the host's grant through a
//!   [`Store`] the platform supplies, adds the host to the registry, and asks
//!   its supervisor to connect.
//! - A row's transport status is the supervisor's. A background pump drains
//!   connector reports into the registry and ticks its timers, so the status
//!   moves without a screen call.
//! - Data freshness is separate. After each connection, and again every
//!   [`Settings::refresh_every`], the pump reads the host's presence,
//!   activity summaries, enrolled devices (with `access_read`), and pending
//!   enrollment requests (with `access_admin`), then reports the data
//!   current, or reports that the subscription failed.
//! - An access operation runs over the host's current link, direct or relay.
//!   The host checks its own grant record every time; a signed `revoked`
//!   refusal, or a channel the host closed as revoked, blocks the supervisor
//!   and marks the host revoked on this device.
//!
//! - With the owner key, the pump also reads the owner host directory. Its
//!   labels and weights apply to enrolled hosts, and a listed host this
//!   device holds no grant for shows as not enrolled. Adding an enrolled host
//!   to the directory publishes the next revision. See [`Saved::owner`] for
//!   how a device holds the owner key.
//!   The owner can also relabel, reweigh, or remove a listed host, and
//!   settle a conflict by republishing the version this device trusted.
//! - With the `ssh` feature, [`ComputersService::connect_ssh`] installs,
//!   starts or adopts a host with `coder-ssh`, redeems its invitation, and
//!   opens an SSH tunnel whose loopback port the connector tries first.
//!   [`ComputersService::remove_ssh`] runs `coder-ssh`'s explicit remove.
//!
//! The device key never leaves this process. The saved grants are private:
//! the mobile store encrypts them under the device key, and [`FileStore`]
//! writes them owner-only.
use crate::model::{
    Compatibility, CreatedInvitation, DeviceList, DeviceRow, DirectoryState, Enrollment,
    HostRecord, Listing, LocalHost, PendingEnrollment, Platform, ServiceState, Snapshot,
    SshAttempt,
};
use crate::service::{ComputersService, Result};
use coder_access::client::{OpenedEnrollment, pending_enrollments, redeem};
use coder_access::protocol::{DeviceEntry, QueueEdit, TaskCommand, TaskCreate, TaskQueue};
use coder_access::{Access, Code, Error, Operation, Outcome, RelayPolicy, Right, Rights};
use coder_host::client::iroh::{DEFAULT_RELAY, Dialer, EnrollError, IrohRoute};
use coder_host::client::{
    Connector, Device, Link, Reports, Route, fetch_directory_revisions, fetch_reach,
    fetch_summaries, watch_summaries,
};
use coder_link::{
    BlockReason, ConnectionId, Failure, HostKey, Phase, Policy, Registry, Report, Signal,
    SystemClock,
};
use coder_reach::directory::Directory;
use coder_reach::hints::Class;
pub use coder_reach::hints::Locality;
use coder_reach::presence::{ClientProfile, Received, VersionRange};
use nostr::activity_summary::{self, ActivitySummary};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// The saved record's schema.
pub const SAVED_SCHEMA: &str = "openagents.coder.computers.v1";
/// How often the pump ticks supervisor timers.
const TICK: Duration = Duration::from_millis(200);
/// A snapshot asks for fresh data when the last read is older than this.
const NUDGE_AFTER: Duration = Duration::from_secs(2);
/// How long one summary watch stays open before it is opened again; a relay
/// connection lives at most 120 seconds.
const WATCH_FOR: Duration = Duration::from_secs(110);
/// How long relays keep a directory revision this device publishes.
const DIRECTORY_RETENTION: u64 = 365 * 86_400;

/// Everything this device keeps about its computers. It holds grants: keep
/// it only in the platform's protected store.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub v: String,
    pub hosts: Vec<SavedHost>,
    pub first_run_complete: bool,
    /// The person chose to run this client with no local host.
    pub client_only: bool,
    /// Owner authority on this device, under the NIP-REACH rule: the device
    /// reads and updates the owner directory only with the owner key held
    /// here. It holds that key when its own device key is the owner a held
    /// grant names, or when the person entered the owner key on this device.
    /// The key never arrives over a relay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<SavedOwner>,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            v: SAVED_SCHEMA.into(),
            hosts: Vec::new(),
            first_run_complete: false,
            client_only: false,
            owner: None,
        }
    }
}

/// The owner key this device holds, and the directory it last trusted.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedOwner {
    /// The owner's secret key in hex. `None` when the device key is the
    /// owner key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// The mailbox the owner's directory revisions use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox: Option<String>,
    /// The highest directory revision this device read or published. A
    /// lower revision never replaces it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<Directory>,
}

impl std::fmt::Debug for SavedOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedOwner")
            .field("secret", &self.secret.as_ref().map(|_| "[redacted]"))
            .field("mailbox", &self.mailbox)
            .field("directory", &self.directory)
            .finish()
    }
}

/// One enrolled host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedHost {
    /// The verified access record: the host-signed grant and its envelope.
    pub access: Access,
    pub label: String,
    /// Switched on. Switching off keeps the host and its grant.
    pub enabled: bool,
    /// The host revoked this device, as its signed refusal or channel
    /// closing said.
    pub revoked: bool,
    /// The SSH destination this device set the host up through, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    /// The owner removed the host from the directory. It stays reachable
    /// with its grant and leaves placement.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub delisted: bool,
    /// The computer's iroh endpoint, from the code it was connected with.
    /// Routing only: the NIP-REACH handshake and the grant decide access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iroh: Option<IrohRoute>,
}

/// Where a platform keeps [`Saved`].
pub trait Store: Send {
    /// # Errors
    /// Reports an unreadable or unauthenticated record.
    fn load(&mut self) -> std::result::Result<Option<Saved>, String>;
    /// # Errors
    /// Reports a failed write; the caller keeps its previous record.
    fn save(&mut self, saved: &Saved) -> std::result::Result<(), String>;
}

/// An in-memory store for tests.
#[derive(Default)]
pub struct MemoryStore(pub Option<Saved>);

impl Store for MemoryStore {
    fn load(&mut self) -> std::result::Result<Option<Saved>, String> {
        Ok(self.0.clone())
    }
    fn save(&mut self, saved: &Saved) -> std::result::Result<(), String> {
        self.0 = Some(saved.clone());
        Ok(())
    }
}

/// An owner-only JSON file, for the terminal and desktop clients.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    /// A store at `directory/computers.json`. The directory is created
    /// owner-only.
    ///
    /// # Errors
    /// Reports a directory that cannot be created.
    pub fn open(directory: &Path) -> std::result::Result<Self, String> {
        private_dir(directory)?;
        Ok(Self {
            path: directory.join("computers.json"),
        })
    }
}

impl Store for FileStore {
    fn load(&mut self) -> std::result::Result<Option<Saved>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| "the saved computers record is unreadable".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("the saved computers record cannot be read".into()),
        }
    }
    fn save(&mut self, saved: &Saved) -> std::result::Result<(), String> {
        let bytes =
            serde_json::to_vec_pretty(saved).map_err(|_| "cannot encode computers record")?;
        write_private(&self.path, &bytes)
    }
}

/// Load this client's device key from `directory/device.key`, or create it
/// owner-only on first use. Only a missing file is a first use.
///
/// # Errors
/// Reports an unreadable or malformed key file.
pub fn load_or_create_key(directory: &Path) -> std::result::Result<SecretKey, String> {
    private_dir(directory)?;
    let path = directory.join("device.key");
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .trim()
            .parse::<SecretKey>()
            .map_err(|_| "the device key file is malformed".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let secret = SecretKey::new(&mut secp256k1::rand::rng());
            write_private(&path, secret.display_secret().to_string().as_bytes())?;
            Ok(secret)
        }
        Err(_) => Err("the device key file cannot be read".into()),
    }
}

/// This device's iroh secret key from `directory/iroh.key`, created on
/// first use (owner-only). It is transport only: an iroh `EndpointId`
/// never admits anything, so with it a terminal client reaches a computer
/// paired by connect code directly, as the apps do, rather than only
/// through the Nostr relay.
///
/// # Errors
/// The directory cannot be created, or the file is unreadable or
/// malformed.
pub fn load_or_create_iroh_key(directory: &Path) -> std::result::Result<[u8; 32], String> {
    private_dir(directory)?;
    let path = directory.join("iroh.key");
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .trim()
            .parse::<SecretKey>()
            .map(|secret| secret.secret_bytes())
            .map_err(|_| "the iroh key file is malformed".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let secret = SecretKey::new(&mut secp256k1::rand::rng());
            write_private(&path, secret.display_secret().to_string().as_bytes())?;
            Ok(secret.secret_bytes())
        }
        Err(_) => Err("the iroh key file cannot be read".into()),
    }
}

fn private_dir(directory: &Path) -> std::result::Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|_| "cannot create the computers directory".to_owned())
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(directory)
            .map_err(|_| "cannot create the computers directory".into())
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::result::Result<(), String> {
    use std::io::Write;
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "cannot write the computers record")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "cannot write the computers record")?;
    std::fs::rename(&temporary, path).map_err(|_| "cannot replace the computers record".into())
}

/// How a live service runs.
#[derive(Clone, Debug)]
pub struct Settings {
    pub platform: Platform,
    /// The relay policy invitations and grants must satisfy. Only a fixture
    /// or a local test run uses `LoopbackTest`.
    pub policy: RelayPolicy,
    /// Where this client runs relative to its hosts. Claim
    /// `Locality::SameMachine` only from local evidence; it allows loopback
    /// routes.
    pub locality: Locality,
    /// Each host supervisor's retry and timeout policy.
    pub link: Policy,
    /// How often a connected host's data, and the owner directory, are read
    /// again.
    pub refresh_every: Duration,
    /// Unix seconds.
    pub now: fn() -> u64,
    /// How this client starts hosts over SSH. `None` offers no SSH setup.
    #[cfg(feature = "ssh")]
    pub ssh: Option<SshSetup>,
    /// This device's iroh secret key, from the platform's protected store.
    /// With it, connecting a computer and its channel go over iroh first;
    /// without it, over the Nostr relay only.
    pub iroh_secret: Option<[u8; 32]>,
    /// How the iroh endpoint binds, in place of the OpenAgents relay. Only
    /// a test sets it, to loopback with relays disabled.
    pub iroh_loopback: bool,
    /// The Nostr relay a connect code is redeemed for, and on when iroh
    /// cannot connect: the relay a desktop app host's invitations name.
    pub connect_relay: String,
}

impl Settings {
    /// Production relays, another machine, default timing, no SSH setup.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            policy: RelayPolicy::Production,
            locality: Locality::OtherMachine,
            link: Policy::default(),
            refresh_every: Duration::from_secs(30),
            now: unix_now,
            #[cfg(feature = "ssh")]
            ssh: None,
            iroh_secret: None,
            iroh_loopback: false,
            connect_relay: DEFAULT_RELAY.to_owned(),
        }
    }
}

/// What an SSH setup installs and runs on the remote machine.
#[cfg(feature = "ssh")]
#[derive(Clone, Debug)]
pub struct SshSetup {
    /// The pinned `coder` release archives.
    pub release: coder_ssh::Release,
    /// The serve and invite commands.
    pub runner: coder_ssh::Runner,
    /// The `ssh` program, when not the one on `PATH`.
    pub program: Option<PathBuf>,
}

#[cfg(feature = "ssh")]
impl SshSetup {
    /// Run the `coder` binary's host: `coder host serve --loopback --owner
    /// OWNER --relay RELAY` and `coder host invite --relay RELAY --rights
    /// all`. The person who can log in over SSH already controls that
    /// account, so the invitation grants every right. `loopback_test` adds
    /// `--loopback-test` for a local test relay.
    ///
    /// # Errors
    /// Refuses arguments `coder-ssh` cannot send.
    pub fn coder(
        release: coder_ssh::Release,
        owner: &str,
        relay: &str,
        loopback_test: bool,
    ) -> std::result::Result<Self, coder_ssh::Error> {
        let mut serve: Vec<String> = ["host", "serve", "--loopback", "--owner", owner, "--relay"]
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect();
        serve.push(relay.to_owned());
        let mut invite: Vec<String> = ["host", "invite", "--relay", relay, "--rights", "all"]
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect();
        if loopback_test {
            serve.push("--loopback-test".into());
            invite.push("--loopback-test".into());
        }
        Ok(Self {
            release,
            runner: coder_ssh::Runner::new(serve, invite)?,
            program: None,
        })
    }
}

/// The least time between two relay nudges to one host.
const NUDGE_EVERY: Duration = Duration::from_secs(120);
/// How many times, three seconds apart, a nudge watches for fresh presence.
const NUDGE_WATCH: usize = 30;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// What this client knows about one host beyond its saved record.
#[derive(Default)]
struct HostLive {
    key: Option<HostKey>,
    device: Option<Arc<Device>>,
    compatibility: Option<Compatibility>,
    presence: Option<Received>,
    devices: Option<(Vec<DeviceEntry>, u64)>,
    enrollments: Vec<OpenedEnrollment>,
    activity: Vec<ActivitySummary>,
    /// The host's `workspace.list` answer, with `operate`.
    workspaces: Option<Vec<String>>,
    /// The background watchers its `background.list` names, with `observe`.
    watchers: Option<Vec<String>>,
    /// The newest background notice in that answer.
    background: Option<(u64, String)>,
    /// The connection the last catch-up read, and when it finished.
    caught_up: Option<(u64, Instant)>,
    catching_up: bool,
    /// A relay subscription to the host's summaries is open.
    watching: bool,
    nudged: bool,
    /// When this device last left the host a relay nudge.
    woken_at: Option<Instant>,
}

/// The owner directory's read state.
struct DirectoryLive {
    state: DirectoryState,
    reading: bool,
    read_at: Option<Instant>,
    nudged: bool,
}

impl Default for DirectoryLive {
    fn default() -> Self {
        Self {
            state: DirectoryState::Loading,
            reading: false,
            read_at: None,
            nudged: false,
        }
    }
}

struct State {
    saved: Saved,
    hosts: BTreeMap<String, HostLive>,
    directory: DirectoryLive,
    ssh: Option<SshAttempt>,
    /// The SSH prompt waiting for an answer, where the answer goes, and the
    /// stage to resume once it is answered.
    #[cfg(feature = "ssh")]
    prompt: Option<(
        u64,
        std::sync::mpsc::Sender<Option<coder_ssh::Secret>>,
        crate::model::SshStage,
    )>,
    #[cfg(feature = "ssh")]
    prompts: u64,
    /// SSH tunnels by host key. Each lives only in this process.
    #[cfg(feature = "ssh")]
    tunnels: BTreeMap<String, ssh::TunnelLive>,
}

/// State the service and its pump share. Lock order: registry, state,
/// store.
struct Shared {
    settings: Settings,
    secret: SecretKey,
    key: String,
    registry: Mutex<Registry<SystemClock, Connector>>,
    state: Mutex<State>,
    store: Mutex<Box<dyn Store>>,
    /// Called after background work changes what a snapshot shows.
    changed: Mutex<Option<Changed>>,
    /// This device's iroh endpoint, when it has an iroh key.
    dialer: Option<Arc<Dialer>>,
}

/// A caller's hook for background changes; see [`Live::on_change`].
pub type Changed = Arc<dyn Fn() + Send + Sync>;

/// The live Computers service.
pub struct Live {
    shared: Arc<Shared>,
    runtime: Handle,
    pump: JoinHandle<()>,
}

impl Live {
    /// Open the saved record, register every enrolled host, and start
    /// supervising. Blocking calls run on the caller's thread with
    /// `runtime`; call them from outside the runtime's own threads.
    ///
    /// # Errors
    /// Reports an unreadable store or an invalid link policy.
    pub fn open(
        settings: Settings,
        secret: SecretKey,
        mut store: Box<dyn Store>,
        runtime: Handle,
    ) -> Result<Self> {
        let mut saved = store
            .load()
            .map_err(|message| Error::new(Code::Unavailable, message))?
            .filter(|saved| saved.v == SAVED_SCHEMA)
            .unwrap_or_default();
        // A saved directory counts only for the owner key this device holds.
        let owner = owner_key(&saved, &secret).map(|owner| coder_reach::pubkey(&owner));
        if let Some(record) = &mut saved.owner {
            record.directory = record.directory.take().filter(|directory| {
                owner.as_deref() == Some(directory.owner.as_str()) && directory.validate().is_ok()
            });
        }
        let (mut connector, reports) = Connector::new(runtime.clone(), settings.locality);
        let dialer = settings.iroh_secret.map(|secret| {
            Arc::new(if settings.iroh_loopback {
                Dialer::loopback(secret)
            } else {
                Dialer::new(secret)
            })
        });
        if let Some(dialer) = &dialer {
            connector.set_iroh(dialer.clone());
        }
        let registry = Registry::new(SystemClock::new(), connector, settings.link.clone())
            .map_err(|error| Error::new(Code::Unavailable, error.to_string()))?;
        let shared = Arc::new(Shared {
            key: coder_reach::pubkey(&secret),
            settings,
            secret,
            registry: Mutex::new(registry),
            state: Mutex::new(State {
                saved,
                hosts: BTreeMap::new(),
                directory: DirectoryLive::default(),
                ssh: None,
                #[cfg(feature = "ssh")]
                prompt: None,
                #[cfg(feature = "ssh")]
                prompts: 0,
                #[cfg(feature = "ssh")]
                tunnels: BTreeMap::new(),
            }),
            store: Mutex::new(store),
            changed: Mutex::new(None),
            dialer,
        });
        let hosts: Vec<SavedHost> = lock(&shared.state).saved.hosts.clone();
        for host in hosts {
            shared.supervise(&host);
        }
        let pump = runtime.spawn(pump(shared.clone(), reports));
        Ok(Self {
            shared,
            runtime,
            pump,
        })
    }

    /// Call `changed` whenever background work changes what a snapshot
    /// shows: a host's activity summary arrives, a catch-up ends, or a
    /// connection's state moves. It runs on the runtime's threads and must
    /// not block; a platform host uses it to redraw at once instead of on a
    /// timer.
    pub fn on_change(&self, changed: Changed) {
        *lock(&self.shared.changed) = Some(changed);
    }

    /// This device's public key.
    #[must_use]
    pub fn device(&self) -> &str {
        &self.shared.key
    }

    /// The supervised link to `host` while it is connected, direct or relay,
    /// for NIP-TERM requests (`Link::terminal`, `Link::next_frame`) and any
    /// NIP-HOST call. The host checks this device's grant on every message.
    ///
    /// # Errors
    /// Refuses a host this device does not supervise or that is not
    /// connected now.
    pub fn host_link(&self, host: &str) -> Result<Arc<Link>> {
        self.shared.link(host).map(|(_, _, link)| link)
    }

    /// A handle that connects computers from scanned or pasted codes, in the
    /// background ([`Pairing::pair`]).
    #[must_use]
    pub fn pairing(&self) -> Pairing {
        Pairing {
            shared: self.shared.clone(),
        }
    }

    /// A handle that reads each host's current link, for terminal sessions.
    /// It stays valid while the service runs and keeps no link alive.
    #[must_use]
    pub fn terminals(&self) -> Terminals {
        Terminals {
            shared: self.shared.clone(),
        }
    }

    /// One NIP-HOST `computer` request over the host's current link: a
    /// screenshot, the open apps, or one file chunk
    /// (`coder_access::computer`). The host checks `terminal` on every one.
    ///
    /// # Errors
    /// The host's refusal or a transport failure.
    pub fn computer(
        &self,
        host: &str,
        request: coder_access::computer::Request,
    ) -> Result<coder_access::computer::Answer> {
        match self.call(host, Operation::Computer { computer: request })? {
            Outcome::Computer { computer } => Ok(computer),
            _ => Err(Error::new(
                Code::Malformed,
                "the host did not answer the computer request",
            )),
        }
    }

    /// Run one access operation over the host's current link.
    fn call(&self, host: &str, op: Operation) -> Result<Outcome> {
        let (key, connection, link) = self.shared.link(host)?;
        let result = self.runtime.block_on(link.call(op));
        result.map_err(|error| {
            let error = access_error(error);
            if error.code == Code::Revoked {
                self.shared.revoked(host, Some((&key, connection)));
            }
            error
        })
    }

    /// Run a task operation and return the host's receipt reference. The
    /// host publishes the task's new summary after it replies, so the next
    /// catch-up is asked for at once.
    fn dispatched(&self, host: &str, op: Operation) -> Result<String> {
        let reference =
            coder_access::client::tasks::Tasks::new(|op| self.call(host, op)).dispatch(op)?;
        if let Some(live) = lock(&self.shared.state).hosts.get_mut(host) {
            live.nudged = true;
        }
        Ok(reference)
    }

    fn save(&self) -> Result<()> {
        let state = lock(&self.shared.state);
        self.shared.save(&state.saved)
    }

    /// Whether this client can start hosts over SSH.
    fn ssh_ready(&self) -> bool {
        #[cfg(feature = "ssh")]
        {
            self.shared.settings.platform != Platform::Phone && self.shared.settings.ssh.is_some()
        }
        #[cfg(not(feature = "ssh"))]
        {
            false
        }
    }
}

/// Reads the current link to each host the live service supervises, so a
/// terminal session follows the supervisor's reconnects and route changes.
#[derive(Clone)]
pub struct Terminals {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Terminals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminals").finish_non_exhaustive()
    }
}

impl Terminals {
    /// The current link to `host`. A host that is not connected answers a
    /// transport error, which a session waits out.
    #[must_use]
    pub fn links(&self, host: &str) -> crate::terminal::session::Links {
        let shared = self.shared.clone();
        let host = host.to_owned();
        Arc::new(move || shared.link(&host).map(|(_, _, link)| link))
    }
}

mod directory;
mod nearby;
#[cfg(feature = "ssh")]
mod ssh;

impl Drop for Live {
    fn drop(&mut self) {
        self.pump.abort();
        let mut registry = lock(&self.shared.registry);
        let keys: Vec<HostKey> = registry.statuses().map(|(key, _)| key.clone()).collect();
        for key in keys {
            let _ = registry.remove(&key, |_, _| {});
        }
    }
}

async fn pump(shared: Arc<Shared>, mut reports: Reports) {
    let mut ticker = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            report = reports.recv() => match report {
                Some((host, report)) => shared.report(&host, report),
                None => return,
            },
            _ = ticker.tick() => lock(&shared.registry).tick(),
        }
        shared.schedule();
        shared.renewals();
        #[cfg(feature = "ssh")]
        ssh::watch(&shared);
    }
}

impl Shared {
    /// Store each renewed grant a host sent on its channel, after checking
    /// that it keeps the grant's terms, so a paired phone never has to pair
    /// again while it keeps connecting. A renewal that fails the check is
    /// dropped and the current grant stays.
    fn renewals(&self) {
        let hosts: Vec<String> = lock(&self.state).hosts.keys().cloned().collect();
        for host in hosts {
            let Ok((_, _, link)) = self.link(&host) else {
                continue;
            };
            let Some(envelope) = link.take_renewal() else {
                continue;
            };
            let now = (self.settings.now)();
            let saved = {
                let mut state = lock(&self.state);
                let Some(saved) = state
                    .saved
                    .hosts
                    .iter_mut()
                    .find(|saved| saved.access.grant.host == host)
                else {
                    continue;
                };
                let Ok(renewed) =
                    saved
                        .access
                        .renewed(envelope, &self.secret, now, self.settings.policy)
                else {
                    continue;
                };
                saved.access = renewed;
                let saved = saved.clone();
                let record = state.saved.clone();
                drop(state);
                if self.save(&record).is_err() {
                    continue;
                }
                saved
            };
            // Later connections use the renewed grant; the open channel
            // already follows it.
            if let Ok(device) =
                Device::new(saved.access.clone(), self.secret, self.settings.policy).map(Arc::new)
            {
                let _ = lock(&self.registry).connector_mut().add(device.clone());
                if let Some(live) = lock(&self.state).hosts.get_mut(&host) {
                    live.device = Some(device);
                }
            }
            self.changed();
        }
    }

    fn save(&self, saved: &Saved) -> Result<()> {
        lock(&self.store)
            .save(saved)
            .map_err(|message| Error::new(Code::Unavailable, message))
    }

    /// Register a saved host with the registry and connect it when it is
    /// switched on, enrolled, and not revoked.
    fn supervise(&self, host: &SavedHost) {
        let name = host.access.grant.host.clone();
        let device = Device::new(host.access.clone(), self.secret, self.settings.policy)
            .ok()
            .map(Arc::new);
        let mut registry = lock(&self.registry);
        let mut live = HostLive {
            device: device.clone(),
            ..HostLive::default()
        };
        if let (Some(device), false) = (device, host.revoked) {
            let registered = registry
                .connector_mut()
                .add(device)
                .ok()
                .filter(|key| registry.register(key.clone(), Default::default()).is_ok());
            if let Some(key) = registered {
                registry
                    .connector_mut()
                    .set_iroh_route(&key, host.iroh.clone());
                if host.enabled {
                    let _ = registry.signal(&key, Signal::Connect);
                } else {
                    let _ = registry.switch_off(&key);
                }
                live.key = Some(key);
            }
        }
        lock(&self.state).hosts.insert(name, live);
    }

    /// Stop connecting to a host and drop it from this device's list. The
    /// host keeps this device's grant, and an SSH host keeps running.
    fn forget(&self, host: &str) -> Result<()> {
        #[cfg(feature = "ssh")]
        ssh::close_tunnel(self, host);
        self.unsupervise(host);
        let saved = {
            let mut state = lock(&self.state);
            state
                .saved
                .hosts
                .retain(|saved| saved.access.grant.host != host);
            state.saved.clone()
        };
        self.save(&saved)
    }

    /// Stop supervising a host and drop what this client knows about it.
    fn unsupervise(&self, host: &str) {
        let mut registry = lock(&self.registry);
        let mut state = lock(&self.state);
        if let Some(key) = state.hosts.remove(host).and_then(|live| live.key) {
            let _ = registry.remove(&key, |_, _| {});
        }
    }

    /// The registry key, current connection, and link for a host.
    fn link(&self, host: &str) -> Result<(HostKey, ConnectionId, Arc<Link>)> {
        let registry = lock(&self.registry);
        let key = lock(&self.state)
            .hosts
            .get(host)
            .and_then(|live| live.key.clone())
            .ok_or_else(|| Error::new(Code::Unavailable, "this computer is not connected"))?;
        let connection = registry
            .status(&key)
            .filter(|status| status.phase == Phase::Connected)
            .and_then(|status| status.connection)
            .ok_or_else(|| Error::new(Code::Transport, "this computer is not connected"))?;
        let link = registry
            .connector()
            .link(&key, connection)
            .ok_or_else(|| Error::new(Code::Transport, "this computer is not connected"))?;
        Ok((key, connection, link))
    }

    fn report(&self, key: &HostKey, report: Report) {
        let status = {
            let mut registry = lock(&self.registry);
            // A report about a superseded attempt changes nothing.
            let _ = registry.report(key, report);
            registry.status(key)
        };
        if status.is_some_and(|s| s.phase == Phase::Blocked(BlockReason::Revoked)) {
            self.revoked(key.as_str(), None);
        }
        self.changed();
    }

    /// Tell the caller's hook that a snapshot would show something new.
    fn changed(&self) {
        let hook = lock(&self.changed).clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    /// The host revoked this device. Record it, and block the supervisor
    /// when a connection is still up.
    fn revoked(&self, host: &str, connection: Option<(&HostKey, ConnectionId)>) {
        if let Some((key, connection)) = connection {
            let _ = lock(&self.registry).report(
                key,
                Report::ConnectionLost(connection, Failure::Blocked(BlockReason::Revoked)),
            );
        }
        let mut state = lock(&self.state);
        let Some(saved) = state
            .saved
            .hosts
            .iter_mut()
            .find(|saved| saved.access.grant.host == host)
        else {
            return;
        };
        if !saved.revoked {
            saved.revoked = true;
            let saved = state.saved.clone();
            let _ = self.save(&saved);
        }
    }

    /// Start a catch-up for every connected host whose data is due.
    fn schedule(self: &Arc<Self>) {
        let registry = lock(&self.registry);
        let mut state = lock(&self.state);
        let every = self.settings.refresh_every;
        for (host, live) in &mut state.hosts {
            let (Some(key), Some(device)) = (&live.key, &live.device) else {
                continue;
            };
            let Some(connection) = registry
                .status(key)
                .filter(|status| status.phase == Phase::Connected)
                .and_then(|status| status.connection)
            else {
                continue;
            };
            // Summaries arrive as the host publishes them while the host is
            // connected; the catch-up below reads what was published before.
            if !live.watching {
                live.watching = true;
                tokio::spawn(watch(self.clone(), host.clone(), device.clone()));
            }
            let due = live.nudged
                || live
                    .caught_up
                    .is_none_or(|(caught, at)| caught != connection.0 || at.elapsed() >= every);
            if live.catching_up || !due {
                continue;
            }
            let Some(link) = registry.connector().link(key, connection) else {
                continue;
            };
            live.catching_up = true;
            live.nudged = false;
            tokio::spawn(catch_up(
                self.clone(),
                host.clone(),
                key.clone(),
                connection,
                device.clone(),
                link,
            ));
        }
        drop(registry);
        let Some(owner) = owner_key(&state.saved, &self.secret) else {
            return;
        };
        let directory = &mut state.directory;
        let due = directory.nudged || directory.read_at.is_none_or(|at| at.elapsed() >= every);
        if directory.reading || !due {
            return;
        }
        directory.reading = true;
        directory.nudged = false;
        let relays = self.directory_relays(&state.saved, &owner);
        tokio::spawn(read_directory(self.clone(), owner, relays));
    }

    /// The relays the owner's directory lives on: those named by the grants
    /// this device holds for the owner's hosts, and by the entries of the
    /// directory it last trusted.
    fn directory_relays(&self, saved: &Saved, owner: &SecretKey) -> Vec<String> {
        let owner = coder_reach::pubkey(owner);
        let mut relays: Vec<String> = saved
            .hosts
            .iter()
            .filter(|host| host.access.grant.owner == owner)
            .map(|host| host.access.grant.relay.clone())
            .collect();
        if let Some(directory) = saved.owner.as_ref().and_then(|o| o.directory.as_ref()) {
            relays.extend(
                directory
                    .hosts
                    .iter()
                    .flat_map(|entry| entry.relays.iter().cloned()),
            );
        }
        relays.sort();
        relays.dedup();
        relays.retain(|relay| self.settings.policy.validate(relay).is_ok());
        relays
    }

    /// Redeem a `coder-host:` invitation, save the grant, and supervise the
    /// host. `label` names a new host; a host already saved keeps its label.
    fn redeem(
        &self,
        runtime: &Handle,
        invitation: &str,
        label: Option<String>,
        ssh: Option<String>,
    ) -> Result<String> {
        let access = runtime.block_on(redeem(invitation, &self.secret, self.settings.policy))?;
        self.adopt(access, label, ssh, None)
    }

    /// The name this device's list gives `host`.
    fn label(&self, host: &str) -> String {
        lock(&self.state)
            .saved
            .hosts
            .iter()
            .find(|saved| saved.access.grant.host == host)
            .map_or_else(
                || format!("Computer {}", short(host)),
                |saved| saved.label.clone(),
            )
    }

    /// Save a verified access record and start supervising its host. A
    /// computer paired before keeps its name and its SSH destination; a new
    /// iroh route replaces the old one.
    fn adopt(
        &self,
        access: Access,
        label: Option<String>,
        ssh: Option<String>,
        iroh: Option<IrohRoute>,
    ) -> Result<String> {
        let host = access.grant.host.clone();
        self.unsupervise(&host);
        let saved = {
            let mut state = lock(&self.state);
            let previous = state
                .saved
                .hosts
                .iter()
                .find(|saved| saved.access.grant.host == host)
                .cloned();
            let label = previous.as_ref().map_or_else(
                || label.unwrap_or_else(|| format!("Computer {}", short(&host))),
                |saved| saved.label.clone(),
            );
            state
                .saved
                .hosts
                .retain(|saved| saved.access.grant.host != host);
            let saved = SavedHost {
                access,
                label,
                enabled: true,
                revoked: false,
                delisted: previous.as_ref().is_some_and(|saved| saved.delisted),
                iroh: iroh.or_else(|| previous.as_ref().and_then(|saved| saved.iroh.clone())),
                ssh: ssh.or_else(|| previous.and_then(|saved| saved.ssh)),
            };
            state.saved.hosts.push(saved.clone());
            // A grant can make the device key the owner key.
            state.directory.nudged = true;
            let record = state.saved.clone();
            drop(state);
            self.save(&record)?;
            saved
        };
        self.supervise(&saved);
        Ok(host)
    }
}

/// Connects computers from scanned or pasted codes (**Connect a
/// computer**). It runs on the caller's runtime and never blocks it: a
/// platform host starts [`Pairing::pair`] in the background and shows
/// **Connecting** until it answers.
#[derive(Clone)]
pub struct Pairing {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Pairing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pairing").finish_non_exhaustive()
    }
}

impl Pairing {
    /// Pair with the computer a code names, and add it to this device's
    /// list. A connect code goes over iroh to the code's `EndpointId`, and
    /// on the Nostr relay when iroh cannot connect; a host invitation goes
    /// on the relay it names. The grant is kept only when it is signed by
    /// the code's host key, names this device, and carries the connect-code
    /// rights.
    ///
    /// # Errors
    /// Why the computer was not added, in words for the screen.
    pub async fn pair(
        &self,
        code: &str,
    ) -> std::result::Result<crate::connect::Paired, crate::connect::PairFailure> {
        use crate::connect::{PairFailure, Paired, PairedOver, Scanned, classify};
        let shared = &self.shared;
        let policy = shared.settings.policy;
        let (enrolled, over) = match classify(code).map_err(PairFailure::new)? {
            Scanned::HostInvitation(text) => {
                let access = redeem(&text, &shared.secret, policy)
                    .await
                    .map_err(|error| PairFailure::new(pair_message(&error)))?;
                let host = shared
                    .adopt(access, None, None, None)
                    .map_err(|error| PairFailure::new(crate::describe(&error)))?;
                return Ok(Paired {
                    label: shared.label(&host),
                    host,
                    over: PairedOver::Relay,
                    clock_off: None,
                    chats: None,
                });
            }
            Scanned::Connect(text) => {
                let iroh = match &shared.dialer {
                    Some(dialer) => {
                        coder_host::client::iroh::enroll(
                            dialer,
                            &text,
                            &shared.settings.connect_relay,
                            &shared.secret,
                            policy,
                        )
                        .await
                    }
                    None => Err(EnrollError::Unreachable),
                };
                match iroh {
                    Ok(enrolled) => (enrolled, PairedOver::Iroh),
                    Err(EnrollError::Unreachable) => (
                        coder_host::client::iroh::enroll_on_relay(
                            &text,
                            &shared.settings.connect_relay,
                            &shared.secret,
                            policy,
                        )
                        .await
                        .map_err(|error| PairFailure::new(host_message(&error, true)))?,
                        PairedOver::Relay,
                    ),
                    Err(EnrollError::Mismatch) => {
                        return Err(PairFailure::new(MISMATCH));
                    }
                    Err(EnrollError::Refused { error, clock_off }) => {
                        return Err(PairFailure {
                            message: host_message(&error, false),
                            clock_off,
                        });
                    }
                }
            }
        };
        let label = (!enrolled.label.is_empty()).then(|| enrolled.label.clone());
        let host = shared
            .adopt(enrolled.access, label, None, Some(enrolled.route))
            .map_err(|error| PairFailure::new(crate::describe(&error)))?;
        shared.changed();
        Ok(Paired {
            label: shared.label(&host),
            host,
            over,
            clock_off: enrolled.clock_off,
            chats: enrolled.chats,
        })
    }
}

/// What the screen says when an answer did not come from the computer that
/// showed the code.
const MISMATCH: &str = "This phone didn't connect: the answer didn't come from the computer that showed the code. Scan the code on your own computer again.";

/// The screen's sentence for a refused host invitation.
fn pair_message(error: &Error) -> String {
    match error.code {
        Code::Expired => {
            "This invitation has expired. Make a new one on your computer and scan again.".into()
        }
        Code::Forbidden | Code::Revoked => {
            "Your computer didn't accept this invitation. It may have been used already. Make a new one and scan again.".into()
        }
        Code::Transport | Code::Unavailable => {
            "Couldn't reach your computer. Check that it's on and online, then scan again.".into()
        }
        _ => format!("Couldn't connect: {}", error.message),
    }
}

/// The screen's sentence for a refused connect code. `relay` says the phone
/// already fell back to the Nostr relay.
fn host_message(error: &coder_host::Error, relay: bool) -> String {
    match error {
        coder_host::Error::Access(error) if error.code == Code::Expired => {
            "This code has expired. Show a new code on your computer and scan again.".into()
        }
        coder_host::Error::Access(error)
            if matches!(error.code, Code::Forbidden | Code::Revoked | Code::Denied) =>
        {
            "Your computer didn't accept this code. It may have been used already. Show a new code on your computer and scan again.".into()
        }
        coder_host::Error::Access(error)
            if matches!(error.code, Code::Transport | Code::Unavailable) || relay =>
        {
            "Couldn't reach your computer. Check that OpenAgents is open on it and that both are online, then scan again.".into()
        }
        coder_host::Error::Transport(_) | coder_host::Error::Closed(_) => {
            "Couldn't reach your computer. Check that OpenAgents is open on it and that both are online, then scan again.".into()
        }
        _ => "Couldn't connect to your computer. Show a new code on it and scan again.".into(),
    }
}

/// The owner key this device holds, if any: the owner key the person
/// entered here, or the device key when a held grant names it as the owner.
fn owner_key(saved: &Saved, device: &SecretKey) -> Option<SecretKey> {
    if let Some(secret) = saved
        .owner
        .as_ref()
        .and_then(|owner| owner.secret.as_deref())
    {
        return secret.parse().ok();
    }
    let key = coder_reach::pubkey(device);
    saved
        .hosts
        .iter()
        .any(|host| host.access.grant.owner == key)
        .then_some(*device)
}

/// Parse an owner secret key entered as 64 hex characters or `nsec`.
fn parse_secret(text: &str) -> Option<SecretKey> {
    if text.starts_with("nsec1") {
        let bytes = nostr::nip19::decode_nsec(text).ok()?;
        SecretKey::from_byte_array(bytes).ok()
    } else if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        text.to_ascii_lowercase().parse().ok()
    } else {
        None
    }
}

/// Read every retained directory revision from `relays` and keep the
/// current one. A lower revision than the one this device trusts never
/// replaces it, and two different bodies at the top revision are a conflict.
async fn read_directory(shared: Arc<Shared>, owner: SecretKey, relays: Vec<String>) {
    let mut found: Vec<(Directory, String)> = Vec::new();
    let mut answered = false;
    for relay in &relays {
        if let Ok(revisions) =
            fetch_directory_revisions(relay, &owner, shared.settings.policy).await
        {
            answered = true;
            found.extend(revisions);
        }
    }
    let now = (shared.settings.now)();
    let mut state = lock(&shared.state);
    state.directory.reading = false;
    state.directory.read_at = Some(Instant::now());
    // The owner key changed while this read ran; the next read starts over.
    if owner_key(&state.saved, &shared.secret).map(|key| key.secret_bytes())
        != Some(owner.secret_bytes())
    {
        return;
    }
    let held = state
        .saved
        .owner
        .as_ref()
        .and_then(|record| record.directory.clone());
    let held_revision = held.as_ref().map(|directory| directory.revision);
    if !answered {
        state.directory.state = DirectoryState::Failed {
            revision: held_revision,
        };
        return;
    }
    let mut candidates: Vec<Directory> = found.iter().map(|(d, _)| d.clone()).collect();
    candidates.extend(held.clone());
    let current = match Directory::current(&candidates) {
        Err(_) => {
            let top = candidates.iter().map(|d| d.revision).max().unwrap_or(0);
            state.directory.state = DirectoryState::Conflict { revision: top };
            return;
        }
        Ok(current) => current.cloned(),
    };
    let Some(current) = current else {
        state.directory.state = DirectoryState::Current {
            revision: None,
            as_of: now,
        };
        return;
    };
    state.directory.state = DirectoryState::Current {
        revision: Some(current.revision),
        as_of: now,
    };
    if held.as_ref() == Some(&current) {
        return;
    }
    let mailbox = found
        .iter()
        .find(|(directory, _)| *directory == current)
        .map(|(_, mailbox)| mailbox.clone());
    directory::mark_delisted(&mut state.saved, held.as_ref(), &current);
    let record = state.saved.owner.get_or_insert_with(SavedOwner::default);
    if mailbox.is_some() {
        record.mailbox = mailbox;
    }
    record.directory = Some(current);
    let saved = state.saved.clone();
    drop(state);
    let _ = shared.save(&saved);
}

/// Keep the host's summaries for this device current as the relay delivers
/// them, until the watch ends; the next schedule opens another.
async fn watch(shared: Arc<Shared>, host: String, device: Arc<Device>) {
    let relay = device.relay().to_owned();
    let mut seen = |summary: ActivitySummary| {
        if let Some(live) = lock(&shared.state).hosts.get_mut(&host) {
            keep_newest(&mut live.activity, summary);
        }
        shared.changed();
    };
    let ended = watch_summaries(&device, &relay, WATCH_FOR, &mut seen).await;
    // A watch that failed at once waits before the next, so an unreachable
    // relay is not asked again on every tick.
    if ended.is_err() {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    if let Some(live) = lock(&shared.state).hosts.get_mut(&host) {
        live.watching = false;
    }
}

/// Hold `summary` unless a newer one for its subject is held.
fn keep_newest(activity: &mut Vec<ActivitySummary>, summary: ActivitySummary) {
    match activity
        .iter_mut()
        .find(|held| held.subject_kind == summary.subject_kind && held.subject == summary.subject)
    {
        Some(held) => {
            if activity_summary::supersedes(held, &summary).unwrap_or(false) {
                *held = summary;
            }
        }
        None => activity.push(summary),
    }
}

/// Read what a connected host holds for this device, then report whether
/// the data is current.
async fn catch_up(
    shared: Arc<Shared>,
    host: String,
    key: HostKey,
    connection: ConnectionId,
    device: Arc<Device>,
    link: Arc<Link>,
) {
    let relay = device.relay().to_owned();
    let rights = device.access().grant.rights.clone();
    let client = ClientProfile {
        protocol: coder_reach::PROTOCOL_VERSION,
        accepts: VersionRange {
            min: coder_reach::PROTOCOL_VERSION,
            max: coder_reach::PROTOCOL_VERSION,
        },
    };
    // The reads are independent, so they run at once: a catch-up takes as
    // long as its slowest read, not their sum.
    let reach = async {
        fetch_reach(&device, &relay)
            .await
            .ok()
            .map(|reach| reach.presence)
    };
    let summaries = fetch_summaries(&device, &relay);
    let devices = async {
        if !rights.contains(Right::AccessRead) {
            return (None, false);
        }
        match link.call(Operation::ListDevices {}).await {
            Ok(Outcome::Devices { devices }) => (Some(devices), false),
            Err(error) => (None, access_error(error).code == Code::Revoked),
            Ok(_) => (None, false),
        }
    };
    // A host that predates `workspace.list` refuses it; the order form then
    // asks for a workspace name.
    let workspaces = async {
        if !rights.contains(Right::Operate) {
            return None;
        }
        match link.call(Operation::ListWorkspaces {}).await {
            Ok(Outcome::Workspaces { workspaces }) => Some(workspaces),
            _ => None,
        }
    };
    // A host without a background runner (or one that predates
    // `background.*`) refuses it; the list then says nothing of watchers.
    let watchers = async {
        if !rights.contains(Right::Observe) {
            return None;
        }
        match link.call(Operation::ListBackground {}).await {
            Ok(Outcome::Background { background }) => Some((
                crate::model::watchers(&background, (shared.settings.now)()),
                crate::model::background_notice(&background),
            )),
            _ => None,
        }
    };
    let enrollments = async {
        if !rights.contains(Right::AccessAdmin) {
            return None;
        }
        pending_enrollments(&relay, &shared.secret, &host, shared.settings.policy)
            .await
            .ok()
    };
    let (presence, summaries, (devices, revoked), workspaces, watchers, enrollments) =
        tokio::join!(reach, summaries, devices, workspaces, watchers, enrollments);
    let compatibility = presence
        .as_ref()
        .map(|received| Compatibility::judge(&received.presence, &client));
    let now = (shared.settings.now)();
    {
        let mut state = lock(&shared.state);
        if let Some(live) = state.hosts.get_mut(&host) {
            live.catching_up = false;
            live.caught_up = Some((connection.0, Instant::now()));
            if compatibility.is_some() {
                live.compatibility = compatibility;
                live.presence = presence;
            }
            // A watch may already hold a summary newer than this read's.
            if let Ok(summaries) = &summaries {
                for summary in summaries {
                    keep_newest(&mut live.activity, summary.clone());
                }
            }
            if let Some(devices) = devices {
                live.devices = Some((devices, now));
            }
            if let Some(enrollments) = enrollments {
                live.enrollments = enrollments;
            }
            if workspaces.is_some() {
                live.workspaces = workspaces;
            }
            let (watchers, background) = watchers.unwrap_or_default();
            live.watchers = watchers;
            live.background = background;
        }
    }
    if revoked {
        shared.revoked(&host, Some((&key, connection)));
        return;
    }
    let report = if summaries.is_ok() {
        Report::DataCurrent(connection)
    } else {
        Report::SubscriptionFailed(connection)
    };
    shared.report(&key, report);
}

/// The class of the route a link uses.
fn route_class(route: &Route) -> Class {
    let address = match route {
        Route::Relay(_) => return Class::Relay,
        Route::Direct(address) => address,
    };
    // A WebSocket route is a `ws` or `wss` URL; classify its authority.
    let address = address
        .split_once("://")
        .map_or(address.as_str(), |(_, rest)| {
            rest.split(['/', '?', '#']).next().unwrap_or(rest)
        });
    match address.parse::<SocketAddr>().map(|a| a.ip()) {
        Ok(ip) if ip.is_loopback() => Class::Loopback,
        Ok(IpAddr::V4(ip)) => {
            let [a, b, ..] = ip.octets();
            // The shared address space tailnets draw from.
            if a == 100 && (64..128).contains(&b) {
                Class::Tailnet
            } else if ip.is_private() || ip.is_link_local() {
                Class::Lan
            } else {
                Class::Public
            }
        }
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            if first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80 {
                Class::Lan
            } else {
                Class::Public
            }
        }
        Err(_) => Class::Public,
    }
}

/// A host client failure as the stable access code the screens describe.
fn access_error(error: coder_host::Error) -> Error {
    use coder_host::Error as E;
    match error {
        E::Access(error) => error,
        E::Closed(Some(code)) if code == "revoked" || code == "stale" => {
            Error::new(Code::Revoked, "the host closed the channel as revoked")
        }
        E::Reach(error) if error.code == coder_reach::Refusal::Revoked => {
            Error::new(Code::Revoked, "the host refused the channel as revoked")
        }
        E::Closed(_) | E::Transport(_) | E::Reach(_) => {
            Error::new(Code::Transport, "the computer did not answer")
        }
        E::Config(_) | E::Terminal(_) => {
            Error::new(Code::Unavailable, "this client cannot reach the computer")
        }
    }
}

fn short(key: &str) -> &str {
    key.get(..8).unwrap_or(key)
}

impl ComputersService for Live {
    fn snapshot(&mut self) -> Result<Snapshot> {
        let now = (self.shared.settings.now)();
        let registry = lock(&self.shared.registry);
        let mut state = lock(&self.shared.state);
        let owner = owner_key(&state.saved, &self.shared.secret);
        #[cfg(feature = "ssh")]
        let tunnels = ssh::tunnels(&state);
        if owner.is_some()
            && state
                .directory
                .read_at
                .is_some_and(|at| at.elapsed() >= NUDGE_AFTER)
        {
            state.directory.nudged = true;
        }
        let State {
            saved,
            hosts,
            directory: directory_live,
            ssh,
            ..
        } = &mut *state;
        // The directory counts only while this device holds its owner key.
        let directory = owner
            .as_ref()
            .and(saved.owner.as_ref())
            .and_then(|record| record.directory.as_ref());
        let listing = |key: &str| {
            directory
                .and_then(|directory| directory.entry(key))
                .map(|entry| {
                    (
                        entry.label.clone(),
                        Listing {
                            weight: entry.weight,
                            added_at: entry.added_at,
                        },
                    )
                })
        };
        let mut records: Vec<HostRecord> = saved
            .hosts
            .iter()
            .map(|saved| {
                let host = saved.access.grant.host.clone();
                let live = hosts.entry(host.clone()).or_default();
                let link = live.key.as_ref().and_then(|key| registry.status(key));
                let route = live.key.as_ref().and_then(|key| {
                    let connection = link
                        .filter(|status| status.phase == Phase::Connected)?
                        .connection?;
                    Some(registry.connector().link(key, connection)?.route().clone())
                });
                #[cfg(feature = "ssh")]
                let tunnel = tunnels
                    .get(&host)
                    .map(|(open, address)| crate::model::Tunnel {
                        open: *open,
                        in_use: *open
                            && route.as_ref() == Some(&Route::Direct(address.to_string())),
                    });
                #[cfg(not(feature = "ssh"))]
                let tunnel = None;
                let route = route.as_ref().map(route_class);
                // A screen that polls asks for fresher data without waiting
                // for the background period.
                if live
                    .caught_up
                    .is_some_and(|(_, at)| at.elapsed() >= NUDGE_AFTER)
                {
                    live.nudged = true;
                }
                let grant = &saved.access.grant;
                let enrollment = if saved.revoked {
                    Enrollment::Revoked
                } else if grant.expires_at <= now {
                    Enrollment::Expired {
                        at: grant.expires_at,
                    }
                } else {
                    Enrollment::Enrolled {
                        grant: grant.grant.clone(),
                        rights: grant.rights.clone(),
                        epoch: grant.epoch,
                        expires_at: grant.expires_at,
                    }
                };
                let listed = listing(&host);
                HostRecord {
                    key: host,
                    label: listed
                        .as_ref()
                        .map_or_else(|| saved.label.clone(), |(label, _)| label.clone()),
                    delisted: saved.delisted && listed.is_none(),
                    listing: listed.map(|(_, listing)| listing),
                    ssh: saved.ssh.clone(),
                    tunnel,
                    enrollment,
                    link,
                    route,
                    compatibility: live.compatibility.unwrap_or(Compatibility::Unknown),
                    presence: live.presence.clone(),
                    devices: live.devices.as_ref().map_or(
                        DeviceList::NotLoaded,
                        |(devices, as_of)| DeviceList::Loaded {
                            devices: devices.iter().map(device_row).collect(),
                            as_of: *as_of,
                        },
                    ),
                    enrollments: live
                        .enrollments
                        .iter()
                        .map(|opened| PendingEnrollment {
                            enrollment: opened.enrollment.enrollment.clone(),
                            rights: opened.enrollment.rights.clone(),
                            expires_at: opened.enrollment.expires_at,
                        })
                        .collect(),
                    workspaces: live.workspaces.clone(),
                    watchers: live.watchers.clone(),
                    background: live.background.clone(),
                }
            })
            .collect();
        // Listed hosts this device holds no grant for: not enrolled. A
        // directory entry grants nothing.
        if let Some(directory) = directory {
            for entry in &directory.hosts {
                if records.iter().any(|record| record.key == entry.host) {
                    continue;
                }
                records.push(HostRecord {
                    key: entry.host.clone(),
                    label: entry.label.clone(),
                    listing: Some(Listing {
                        weight: entry.weight,
                        added_at: entry.added_at,
                    }),
                    delisted: false,
                    ssh: None,
                    tunnel: None,
                    enrollment: Enrollment::NotEnrolled,
                    link: None,
                    route: None,
                    compatibility: Compatibility::Unknown,
                    presence: None,
                    devices: DeviceList::NotLoaded,
                    enrollments: Vec::new(),
                    workspaces: None,
                    watchers: None,
                    background: None,
                });
            }
        }
        let activity = saved
            .hosts
            .iter()
            .filter_map(|saved| hosts.get(&saved.access.grant.host))
            .flat_map(|live| live.activity.iter().cloned())
            .collect();
        Ok(Snapshot {
            now,
            device: self.shared.key.clone(),
            owner: false,
            service: ServiceState::Ready,
            local_host: match (self.shared.settings.platform, saved.client_only) {
                (Platform::Phone, _) => LocalHost::NotSupported,
                (_, true) => LocalHost::ClientOnly,
                (_, false) => LocalHost::Undecided,
            },
            first_run_complete: saved.first_run_complete,
            hosts: records,
            activity,
            directory: if owner.is_some() {
                directory_live.state
            } else {
                DirectoryState::NoOwnerKey
            },
            ssh_ready: self.ssh_ready(),
            ssh: ssh.clone(),
        })
    }

    fn set_enabled(&mut self, host: &str, enabled: bool) -> Result<()> {
        {
            let mut registry = lock(&self.shared.registry);
            let mut state = lock(&self.shared.state);
            if let Some(key) = state.hosts.get(host).and_then(|live| live.key.clone()) {
                let _ = if enabled {
                    registry.switch_on(&key)
                } else {
                    registry.switch_off(&key)
                };
                if enabled {
                    let _ = registry.signal(&key, Signal::Connect);
                }
            }
            let saved = state
                .saved
                .hosts
                .iter_mut()
                .find(|saved| saved.access.grant.host == host)
                .ok_or_else(|| Error::new(Code::Unavailable, "unknown computer"))?;
            saved.enabled = enabled;
        }
        self.save()
    }

    fn retry_now(&mut self, host: &str) -> Result<()> {
        let mut registry = lock(&self.shared.registry);
        let mut state = lock(&self.shared.state);
        let live = state
            .hosts
            .get_mut(host)
            .ok_or_else(|| Error::new(Code::Unavailable, "unknown computer"))?;
        live.nudged = true;
        if let Some(key) = &live.key {
            let _ = registry.signal(key, Signal::RetryNow);
        }
        Ok(())
    }

    fn forget(&mut self, host: &str) -> Result<()> {
        self.shared.forget(host)
    }

    fn redeem_invitation(&mut self, invitation: &str) -> Result<String> {
        self.shared.redeem(&self.runtime, invitation, None, None)
    }

    fn redeem_labeled(&mut self, invitation: &str, label: &str) -> Result<String> {
        self.shared
            .redeem(&self.runtime, invitation, Some(label.to_owned()), None)
    }

    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<()> {
        let opened = lock(&self.shared.state)
            .hosts
            .get(host)
            .and_then(|live| {
                live.enrollments
                    .iter()
                    .find(|opened| opened.enrollment.enrollment == enrollment)
                    .cloned()
            })
            .ok_or_else(|| Error::new(Code::Stale, "the request is no longer waiting"))?;
        let op = opened.approve(code, &self.shared.key, rights.clone(), grant_expires_at);
        let Outcome::Granted { authorization } = self.call(host, op)? else {
            return Err(Error::new(Code::Malformed, "the host did not grant access"));
        };
        let access = Access::from_authorization(
            *authorization,
            &self.shared.secret,
            host,
            (self.shared.settings.now)(),
            self.shared.settings.policy,
        )?;
        self.shared.unsupervise(host);
        let saved = {
            let mut state = lock(&self.shared.state);
            let saved = state
                .saved
                .hosts
                .iter_mut()
                .find(|saved| saved.access.grant.host == host)
                .ok_or_else(|| Error::new(Code::Stale, "unknown computer"))?;
            saved.access = access;
            saved.revoked = false;
            saved.clone()
        };
        self.save()?;
        self.shared.supervise(&saved);
        Ok(())
    }

    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Result<()> {
        let opened = lock(&self.shared.state)
            .hosts
            .get(host)
            .and_then(|live| {
                live.enrollments
                    .iter()
                    .find(|opened| opened.enrollment.enrollment == enrollment)
                    .cloned()
            })
            .ok_or_else(|| Error::new(Code::Stale, "the request is no longer waiting"))?;
        self.call(host, opened.deny())?;
        if let Some(live) = lock(&self.shared.state).hosts.get_mut(host) {
            live.enrollments
                .retain(|opened| opened.enrollment.enrollment != enrollment);
        }
        Ok(())
    }

    #[cfg(not(feature = "ssh"))]
    fn connect_ssh(&mut self, _: &str) -> Result<()> {
        Err(Error::new(
            Code::Unavailable,
            "this client does not start hosts over SSH",
        ))
    }

    #[cfg(feature = "ssh")]
    fn connect_ssh(&mut self, destination: &str) -> Result<()> {
        ssh::start(&self.shared, &self.runtime, destination)
    }

    #[cfg(feature = "ssh")]
    fn answer_ssh_prompt(&mut self, id: u64, answer: Option<&str>) -> Result<()> {
        ssh::answer(&self.shared, id, answer)
    }

    fn import_owner_key(&mut self, text: &str) -> Result<()> {
        let secret = parse_secret(text.trim())
            .ok_or_else(|| Error::new(Code::Malformed, "not a secret key"))?;
        let key = coder_reach::pubkey(&secret);
        {
            let mut state = lock(&self.shared.state);
            if !state
                .saved
                .hosts
                .iter()
                .any(|host| host.access.grant.owner == key)
            {
                return Err(Error::new(
                    Code::Malformed,
                    "no held grant names this key as the owner",
                ));
            }
            state.saved.owner = Some(SavedOwner {
                // The device key needs no second copy.
                secret: (secret.secret_bytes() != self.shared.secret.secret_bytes())
                    .then(|| secret.display_secret().to_string()),
                mailbox: None,
                directory: None,
            });
            state.directory = DirectoryLive {
                nudged: true,
                ..DirectoryLive::default()
            };
        }
        self.save()
    }

    fn list_in_directory(&mut self, host: &str, label: &str) -> Result<()> {
        directory::publish(
            &self.shared,
            &self.runtime,
            &directory::Edit::Add { host, label },
        )
    }

    fn edit_listing(
        &mut self,
        host: &str,
        revision: u64,
        change: &crate::model::ListingChange,
    ) -> Result<()> {
        directory::publish(
            &self.shared,
            &self.runtime,
            &directory::Edit::Change {
                host,
                revision,
                change,
            },
        )
    }

    fn remove_from_directory(&mut self, host: &str, revision: u64) -> Result<()> {
        directory::publish(
            &self.shared,
            &self.runtime,
            &directory::Edit::Remove { host, revision },
        )
    }

    fn keep_directory(&mut self, revision: u64) -> Result<()> {
        directory::publish(
            &self.shared,
            &self.runtime,
            &directory::Edit::Keep { revision },
        )
    }

    #[cfg(feature = "ssh")]
    fn remove_ssh(&mut self, host: &str) -> Result<()> {
        ssh::remove(&self.shared, host)
    }

    fn run_without_local_host(&mut self) -> Result<()> {
        if self.shared.settings.platform == Platform::Phone {
            return Err(Error::new(Code::Unsupported, "phones never run a host"));
        }
        lock(&self.shared.state).saved.client_only = true;
        self.save()
    }

    fn refresh_devices(&mut self, host: &str) -> Result<()> {
        let Outcome::Devices { devices } = self.call(host, Operation::ListDevices {})? else {
            return Err(Error::new(Code::Malformed, "the host did not list devices"));
        };
        let now = (self.shared.settings.now)();
        if let Some(live) = lock(&self.shared.state).hosts.get_mut(host) {
            live.devices = Some((devices, now));
        }
        Ok(())
    }

    fn create_invitation(
        &mut self,
        host: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<CreatedInvitation> {
        let Outcome::Invitation {
            invitation,
            code,
            expires_at,
        } = self.call(
            host,
            Operation::Invite {
                rights: rights.clone(),
                grant_expires_at,
            },
        )?
        else {
            return Err(Error::new(Code::Malformed, "the host did not invite"));
        };
        Ok(CreatedInvitation {
            invitation,
            code,
            rights: rights.clone(),
            expires_at,
        })
    }

    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Result<()> {
        self.call(
            host,
            Operation::CancelInvite {
                invitation: invitation.into(),
            },
        )
        .map(|_| ())
    }

    fn revoke(&mut self, host: &str, device: &str) -> Result<()> {
        self.call(
            host,
            Operation::Revoke {
                device: device.into(),
            },
        )
        .map(|_| ())
    }

    fn refresh_workspaces(&mut self, host: &str) -> Result<()> {
        let Outcome::Workspaces { workspaces } = self.call(host, Operation::ListWorkspaces {})?
        else {
            return Err(Error::new(
                Code::Malformed,
                "the host did not list workspaces",
            ));
        };
        if let Some(live) = lock(&self.shared.state).hosts.get_mut(host) {
            live.workspaces = Some(workspaces);
        }
        Ok(())
    }

    fn put_artifact(
        &mut self,
        host: &str,
        put: &coder_access::media::ArtifactPut,
    ) -> Result<coder_access::media::ArtifactState> {
        match self.call(
            host,
            Operation::PutArtifact {
                artifact: put.clone(),
            },
        )? {
            Outcome::Artifact { artifact } => Ok(artifact),
            _ => Err(Error::new(
                Code::Malformed,
                "the host did not answer the image chunk",
            )),
        }
    }

    fn create_task(&mut self, host: &str, task: &TaskCreate) -> Result<String> {
        let reference = self.dispatched(host, Operation::CreateTask { task: task.clone() })?;
        Ok(reference)
    }

    fn steer_task(&mut self, host: &str, task: &str, revision: u64, prompt: &str) -> Result<()> {
        self.dispatched(
            host,
            Operation::SteerTask {
                task: task.into(),
                revision,
                prompt: prompt.into(),
            },
        )
        .map(|_| ())
    }

    fn cancel_task(&mut self, host: &str, task: &str, revision: u64, reason: &str) -> Result<()> {
        self.dispatched(
            host,
            Operation::CancelTask {
                task: task.into(),
                revision,
                reason: reason.into(),
            },
        )
        .map(|_| ())
    }

    fn command_task(&mut self, host: &str, command: &TaskCommand) -> Result<()> {
        self.dispatched(
            host,
            Operation::CommandTask {
                command: command.clone(),
            },
        )
        .map(|_| ())
    }

    fn archive_task(&mut self, host: &str, task: &str) -> Result<()> {
        self.dispatched(host, Operation::ArchiveTask { task: task.into() })
            .map(|_| ())
    }

    fn review_task(&mut self, host: &str, task: &str) -> Result<coder_access::review::TaskReview> {
        let op = Operation::ReviewTask { task: task.into() };
        match self.call(host, op)? {
            Outcome::Review { review } if review.task == task => Ok(*review),
            _ => Err(Error::new(
                Code::Malformed,
                "the host did not answer with the change",
            )),
        }
    }

    fn publish_task(
        &mut self,
        host: &str,
        task: &str,
        base: &str,
        head_commit: &str,
        head: &str,
    ) -> Result<coder_access::review::Publication> {
        let op = Operation::PublishTask {
            task: task.into(),
            base: base.into(),
            head_commit: head_commit.into(),
            head: head.into(),
        };
        match self.call(host, op.clone())? {
            Outcome::Published { publication }
                if Outcome::Published {
                    publication: publication.clone(),
                }
                .answers(&op) =>
            {
                if let Some(live) = lock(&self.shared.state).hosts.get_mut(host) {
                    live.nudged = true;
                }
                Ok(*publication)
            }
            _ => Err(Error::new(
                Code::Malformed,
                "the host did not answer with the publication",
            )),
        }
    }

    fn queue_task(&mut self, host: &str, task: &str, edit: &QueueEdit) -> Result<TaskQueue> {
        let op = Operation::QueueTask {
            task: task.into(),
            edit: edit.clone(),
        };
        let Outcome::Queue { queue } = self.call(host, op)? else {
            return Err(Error::new(
                Code::Malformed,
                "the host did not answer with the queue",
            ));
        };
        Ok(queue)
    }

    /// Leave the host a relay nudge, at most every [`NUDGE_EVERY`], then
    /// watch for the fresh presence it answers with. Fresh presence asks
    /// the host's supervisor to try at once and reads the host again.
    fn nudge_host(&mut self, host: &str) -> Result<()> {
        let device = {
            let mut state = lock(&self.shared.state);
            let Some(live) = state.hosts.get_mut(host) else {
                return Ok(());
            };
            if live.woken_at.is_some_and(|at| at.elapsed() < NUDGE_EVERY) {
                return Ok(());
            }
            let Some(device) = live.device.clone() else {
                return Ok(());
            };
            live.woken_at = Some(Instant::now());
            device
        };
        let shared = self.shared.clone();
        let host = host.to_owned();
        self.runtime.spawn(async move {
            let relay = device.relay().to_owned();
            let sent_at = unix_now();
            if coder_host::client::nudge(&device, &relay).await.is_err() {
                return;
            }
            for _ in 0..NUDGE_WATCH {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let Ok(reach) = fetch_reach(&device, &relay).await else {
                    continue;
                };
                if reach.presence.presence.observed_at < sent_at {
                    continue;
                }
                let key = {
                    let mut state = lock(&shared.state);
                    let Some(live) = state.hosts.get_mut(&host) else {
                        return;
                    };
                    live.presence = Some(reach.presence);
                    live.nudged = true;
                    live.key.clone()
                };
                if let Some(key) = key {
                    let _ = lock(&shared.registry).signal(&key, Signal::RetryNow);
                }
                return;
            }
        });
        Ok(())
    }

    fn complete_first_run(&mut self) -> Result<()> {
        lock(&self.shared.state).saved.first_run_complete = true;
        self.save()
    }

    fn application(&mut self, active: bool) -> Result<()> {
        lock(&self.shared.registry).signal_all(if active {
            Signal::ApplicationActive
        } else {
            Signal::ApplicationBackground
        });
        Ok(())
    }
}

fn device_row(entry: &DeviceEntry) -> DeviceRow {
    DeviceRow {
        device: entry.device.clone(),
        label: None,
        rights: entry.rights.clone(),
        state: entry.state,
        origin: entry.origin,
        expires_at: entry.expires_at,
        last_seen: entry.last_seen,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_name_their_class() {
        let direct = |address: &str| route_class(&Route::Direct(address.into()));
        assert_eq!(direct("127.0.0.1:4000"), Class::Loopback);
        assert_eq!(direct("[::1]:4000"), Class::Loopback);
        assert_eq!(direct("192.168.1.20:4000"), Class::Lan);
        assert_eq!(direct("10.0.0.2:4000"), Class::Lan);
        assert_eq!(direct("100.101.102.103:4000"), Class::Tailnet);
        assert_eq!(direct("100.128.0.1:4000"), Class::Public);
        assert_eq!(direct("[fd00::1]:4000"), Class::Lan);
        assert_eq!(direct("203.0.113.9:4000"), Class::Public);
        assert_eq!(direct("ws://127.0.0.1:4000/reach"), Class::Loopback);
        assert_eq!(direct("wss://192.168.1.20:4443/"), Class::Lan);
        assert_eq!(direct("wss://host.example/reach"), Class::Public);
        assert_eq!(
            route_class(&Route::Relay("wss://relay.example/".into())),
            Class::Relay
        );
    }

    #[test]
    fn host_failures_keep_revocation_distinct_from_transport() {
        use coder_host::Error as E;
        assert_eq!(
            access_error(E::Closed(Some("revoked".into()))).code,
            Code::Revoked
        );
        assert_eq!(access_error(E::Closed(None)).code, Code::Transport);
        assert_eq!(
            access_error(E::Transport("down".into())).code,
            Code::Transport
        );
        assert_eq!(
            access_error(E::Access(Error::missing(Right::AccessAdmin))).missing,
            Some(Right::AccessAdmin)
        );
    }

    #[test]
    fn file_store_round_trips_owner_only() {
        let dir =
            std::env::temp_dir().join(format!("coder-computers-store-{}", coder_reach::new_id()));
        let mut store = FileStore::open(&dir).unwrap();
        assert!(store.load().unwrap().is_none());
        let saved = Saved {
            first_run_complete: true,
            ..Saved::default()
        };
        store.save(&saved).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert!(loaded.first_run_complete);
        let key = load_or_create_key(&dir).unwrap();
        assert_eq!(load_or_create_key(&dir).unwrap(), key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode =
                |path: PathBuf| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(dir.join("computers.json")), 0o600);
            assert_eq!(mode(dir.join("device.key")), 0o600);
            assert_eq!(mode(dir.clone()), 0o700);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A watched summary and a catch-up's read merge by subject, and an
    /// older one never replaces a newer one.
    #[test]
    fn the_newest_summary_of_each_subject_is_held() {
        use nostr::activity_summary::{Attention, Phase, SubjectKind};
        let summary = |subject: &str, sequence: u64, phase: Phase| ActivitySummary {
            host: "a".repeat(64),
            subject_kind: SubjectKind::Task,
            subject: subject.repeat(64),
            sequence,
            phase,
            headline: "Task".into(),
            attention: Attention::None,
            updated_at: sequence,
        };
        let mut held = vec![summary("b", 5, Phase::Queued)];
        keep_newest(&mut held, summary("b", 6, Phase::Running));
        keep_newest(&mut held, summary("b", 5, Phase::Queued));
        keep_newest(&mut held, summary("c", 1, Phase::Queued));
        assert_eq!(held.len(), 2);
        assert_eq!((held[0].sequence, held[0].phase), (6, Phase::Running));
    }
}
