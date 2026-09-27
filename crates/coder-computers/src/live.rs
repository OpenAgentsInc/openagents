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
//! The device key never leaves this process. The saved grants are private:
//! the mobile store encrypts them under the device key, and [`FileStore`]
//! writes them owner-only.
use crate::model::{
    Compatibility, CreatedInvitation, DeviceList, DeviceRow, Enrollment, HostRecord, LocalHost,
    PendingEnrollment, Platform, ServiceState, Snapshot,
};
use crate::service::{ComputersService, Result};
use coder_access::client::{OpenedEnrollment, pending_enrollments, redeem};
use coder_access::protocol::DeviceEntry;
use coder_access::{Access, Code, Error, Operation, Outcome, RelayPolicy, Right, Rights};
use coder_host::client::{Connector, Device, Link, Reports, Route, fetch_reach, fetch_summaries};
use coder_link::{
    BlockReason, ConnectionId, Failure, HostKey, Phase, Policy, Registry, Report, Signal,
    SystemClock,
};
use coder_reach::hints::Class;
pub use coder_reach::hints::Locality;
use coder_reach::presence::{ClientProfile, VersionRange};
use nostr::activity_summary::ActivitySummary;
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
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            v: SAVED_SCHEMA.into(),
            hosts: Vec::new(),
            first_run_complete: false,
            client_only: false,
        }
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
    /// How often a connected host's data is read again.
    pub refresh_every: Duration,
    /// Unix seconds.
    pub now: fn() -> u64,
}

impl Settings {
    /// Production relays, another machine, default timing.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            policy: RelayPolicy::Production,
            locality: Locality::OtherMachine,
            link: Policy::default(),
            refresh_every: Duration::from_secs(30),
            now: unix_now,
        }
    }
}

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
    devices: Option<(Vec<DeviceEntry>, u64)>,
    enrollments: Vec<OpenedEnrollment>,
    activity: Vec<ActivitySummary>,
    /// The connection the last catch-up read, and when it finished.
    caught_up: Option<(u64, Instant)>,
    catching_up: bool,
    nudged: bool,
}

struct State {
    saved: Saved,
    hosts: BTreeMap<String, HostLive>,
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
}

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
        let saved = store
            .load()
            .map_err(|message| Error::new(Code::Unavailable, message))?
            .filter(|saved| saved.v == SAVED_SCHEMA)
            .unwrap_or_default();
        let (connector, reports) = Connector::new(runtime.clone(), settings.locality);
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
            }),
            store: Mutex::new(store),
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

    /// This device's public key.
    #[must_use]
    pub fn device(&self) -> &str {
        &self.shared.key
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

    fn save(&self) -> Result<()> {
        let state = lock(&self.shared.state);
        self.shared.save(&state.saved)
    }
}

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
    }
}

impl Shared {
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
    let compatibility = fetch_reach(&device, &relay)
        .await
        .ok()
        .map(|reach| Compatibility::judge(&reach.presence.presence, &client));
    let summaries = fetch_summaries(&device, &relay).await;
    let mut revoked = false;
    let devices = if rights.contains(Right::AccessRead) {
        match link.call(Operation::ListDevices {}).await {
            Ok(Outcome::Devices { devices }) => Some(devices),
            Err(error) => {
                revoked = access_error(error).code == Code::Revoked;
                None
            }
            Ok(_) => None,
        }
    } else {
        None
    };
    let enrollments = if rights.contains(Right::AccessAdmin) {
        pending_enrollments(&relay, &shared.secret, &host, shared.settings.policy)
            .await
            .ok()
    } else {
        None
    };
    let now = (shared.settings.now)();
    {
        let mut state = lock(&shared.state);
        if let Some(live) = state.hosts.get_mut(&host) {
            live.catching_up = false;
            live.caught_up = Some((connection.0, Instant::now()));
            if compatibility.is_some() {
                live.compatibility = compatibility;
            }
            if let Ok(summaries) = &summaries {
                live.activity.clone_from(summaries);
            }
            if let Some(devices) = devices {
                live.devices = Some((devices, now));
            }
            if let Some(enrollments) = enrollments {
                live.enrollments = enrollments;
            }
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
        let State { saved, hosts } = &mut *state;
        let records = saved
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
                    Some(route_class(
                        registry.connector().link(key, connection)?.route(),
                    ))
                });
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
                HostRecord {
                    key: host,
                    label: saved.label.clone(),
                    enrollment,
                    link,
                    route,
                    compatibility: live.compatibility.unwrap_or(Compatibility::Unknown),
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
                }
            })
            .collect();
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
        self.shared.unsupervise(host);
        lock(&self.shared.state)
            .saved
            .hosts
            .retain(|saved| saved.access.grant.host != host);
        self.save()
    }

    fn redeem_invitation(&mut self, invitation: &str) -> Result<String> {
        let access = self.runtime.block_on(redeem(
            invitation,
            &self.shared.secret,
            self.shared.settings.policy,
        ))?;
        let host = access.grant.host.clone();
        self.shared.unsupervise(&host);
        let saved = {
            let mut state = lock(&self.shared.state);
            let label = state
                .saved
                .hosts
                .iter()
                .find(|saved| saved.access.grant.host == host)
                .map_or_else(|| format!("Computer {}", short(&host)), |s| s.label.clone());
            state
                .saved
                .hosts
                .retain(|saved| saved.access.grant.host != host);
            let saved = SavedHost {
                access,
                label,
                enabled: true,
                revoked: false,
            };
            state.saved.hosts.push(saved.clone());
            saved
        };
        self.save()?;
        self.shared.supervise(&saved);
        Ok(host)
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

    fn connect_ssh(&mut self, _: &str) -> Result<()> {
        Err(Error::new(
            Code::Unavailable,
            "this client does not start hosts over SSH",
        ))
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
}
