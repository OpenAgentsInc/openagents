//! The resident host: one process that answers enrollment, publishes
//! presence and reachability hints, accepts direct channels and
//! relay-carried operations, serves terminals, hands task operations to the
//! task owner, and publishes activity summaries.
//!
//! Every path reaches the same authority. A NIP-HOST request is admitted by
//! `coder-access` whether it arrives over a relay or a direct channel. A
//! terminal operation is admitted by `coder-pty`, whose rights check reads
//! the same grants. A direct channel is admitted by `coder-reach`, whose
//! grant check reads them too, and it is closed when the grant stops
//! admitting it.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_access::Right;
use coder_reach::channel::Acceptor;
use coder_reach::hints::v2::HintsV2;
use coder_reach::hints::{Class, Hint, Hints, Status, Transport};
use coder_reach::presence::{Presence, VersionRange};
use nostr::activity_summary::{self, Attention, Phase, SubjectKind, SummaryDraft};
use secp256k1::{SecretKey, XOnlyPublicKey};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::authority::{Authority, Grants};
use crate::config::Config;
use crate::mailbox::{self, Stream};
use crate::publish::Publisher;
use crate::tasks::{TaskRef, Tasks};
use crate::{CAPABILITIES, Error, PROTOCOL_VERSION, Result, unix_time};

mod cj;
mod direct;
mod dispatch;
pub(crate) mod iroh;
pub mod keys;
pub(crate) mod nearby;
mod relay;
mod standing;
mod terminal;
mod websocket;

/// The runtime record schema SSH launchers read.
pub const RUNTIME_SCHEMA: &str = "openagents.coder.host-runtime.v1";
/// The ready record schema the host service reads.
pub const READY_SCHEMA: &str = "openagents.coder.host-ready.v1";
/// How long hint sets claim validity.
const HINT_LIFETIME: u64 = 60 * 60;
/// How long summaries are retained after their state.
const SUMMARY_RETENTION: u64 = 24 * 60 * 60;
/// How long start waits for the first relay subscription.
const RELAY_READY_WAIT: Duration = Duration::from_secs(10);

/// State every serving task shares.
pub(crate) struct Shared {
    pub(crate) config: Config,
    pub(crate) authority: Arc<Authority>,
    pub(crate) secret: SecretKey,
    pub(crate) host_key: String,
    pub(crate) owner: String,
    pub(crate) pty: coder_pty::host::Host,
    pub(crate) tasks: Arc<dyn Tasks>,
    pub(crate) publisher: Publisher,
    pub(crate) listen: SocketAddr,
    /// The bound WebSocket listener, when one is configured.
    pub(crate) listen_websocket: Option<SocketAddr>,
    /// The workspace a NIP-HOST `terminal.open` uses: the first label.
    pub(crate) default_workspace: Option<String>,
    /// The nudges this process answered, so a relay reconnect's catch-up
    /// answers each once.
    pub(crate) nudges: std::sync::Mutex<relay::Answered>,
    /// The iroh endpoint, once bound.
    pub(crate) iroh: std::sync::OnceLock<iroh::Listener>,
    /// Woken when a local action changed the grants, so open channels
    /// recheck at once instead of at their next tick.
    pub(crate) grants_changed: tokio::sync::Notify,
    /// Set when a local action changed what the host serves, such as its
    /// projects, and the host must start again to serve it.
    pub(crate) restart: tokio::sync::watch::Sender<bool>,
}

/// A running host.
pub struct Running {
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Running")
            .field("host", &self.shared.host_key)
            .field("generation", &self.shared.config.generation)
            .field("listen", &self.shared.listen)
            .finish_non_exhaustive()
    }
}

/// Start a host and return once it serves: the listener is bound, the first
/// relay subscription is up or its wait ended, and the ready and runtime
/// records, when configured, are written.
///
/// # Errors
/// Refuses an invalid configuration, an uninitialized access store, or a
/// listener that cannot bind.
pub async fn start(config: Config, tasks: Arc<dyn Tasks>) -> Result<Running> {
    config.validate()?;
    // Check the operator's TLS files before anything binds or publishes.
    let websocket_tls = config
        .websocket_tls
        .as_ref()
        .map(crate::tls::acceptor)
        .transpose()?;
    let access = match &config.keys {
        Some(keys) => coder_access::host::Host::with_keys(
            &config.access,
            config.policy,
            Arc::new(keys::HostKey(keys.0.clone())),
        ),
        None => coder_access::host::Host::new(&config.access, config.policy),
    };
    // A host whose keys another program keeps, such as the desktop app's
    // keychain, establishes its own owner on first start: there is no
    // owner step. Once the book exists, its owner never changes here.
    if let Some(keys) = &config.keys
        && !access.state_path().exists()
    {
        let owner = keys::owner(keys.0.as_ref())
            .map_err(|_| Error::Config("the owner key cannot be created".into()))?;
        coder_access::host::ensure_parent(&config.access)?;
        access.init(&coder_reach::pubkey(&owner))?;
    }
    let authority = Arc::new(Authority::open(access)?);
    let secret = authority.host().signing_key()?;
    let host_key = coder_reach::pubkey(&secret);
    let owner = authority.host().owner()?;

    let mut terminals = coder_pty::host::Config::new();
    terminals.generation = mailbox::terminal_generation(&host_key, config.generation);
    for (label, root) in &config.workspaces {
        terminals = terminals.workspace(mailbox::workspace_id(label), root);
    }
    let pty = coder_pty::host::Host::new(terminals, Arc::new(Grants(authority.clone())));

    let listener = TcpListener::bind(config.listen)
        .await
        .map_err(|_| Error::Config("the direct-channel listener cannot bind".into()))?;
    let listen = listener
        .local_addr()
        .map_err(|_| Error::Config("the listener has no local address".into()))?;
    let websocket = match config.listen_websocket {
        Some(address) => Some(websocket::bind(address).await?),
        None => None,
    };
    let listen_websocket = websocket.as_ref().map(|(_, address)| *address);
    let publisher = Publisher::spawn(&config.relays, secret);
    let acceptor = Arc::new(Acceptor::new(
        secret,
        config.generation,
        Grants(authority.clone()),
        config.handshake_timeout,
    ));
    let shared = Arc::new(Shared {
        default_workspace: config
            .workspaces
            .keys()
            .next()
            .map(|l| mailbox::workspace_id(l)),
        config,
        authority,
        secret,
        host_key,
        owner,
        pty,
        tasks,
        publisher,
        listen,
        listen_websocket,
        nudges: std::sync::Mutex::new(relay::Answered::default()),
        iroh: std::sync::OnceLock::new(),
        grants_changed: tokio::sync::Notify::new(),
        restart: tokio::sync::watch::channel(false).0,
    });

    let (ready, relay_ready) = tokio::sync::oneshot::channel();
    let mut tasks = vec![];
    if let Some(iroh_config) = shared.config.iroh.clone() {
        let secret = iroh_secret(&shared.config)?;
        let (listener, iroh_tasks) =
            iroh::start(&shared, acceptor.clone(), secret, &iroh_config).await?;
        let _ = shared.iroh.set(listener);
        tasks.extend(iroh_tasks);
    }
    let control = match shared.config.control.clone() {
        Some(control) => Some(crate::control::bind(&control).await?),
        None => None,
    };
    if let Some(bound) = control {
        tasks.push(tokio::spawn(crate::control::serve(shared.clone(), bound)));
    }
    if let Some((listener, _)) = websocket {
        tasks.push(tokio::spawn(websocket::listen(
            shared.clone(),
            listener,
            acceptor.clone(),
            websocket_tls,
        )));
    }
    tasks.extend([
        tokio::spawn(direct::listen(shared.clone(), listener, acceptor)),
        tokio::spawn(relay::serve(shared.clone(), ready)),
        tokio::spawn(presence_loop(shared.clone())),
        tokio::spawn(summary_loop(shared.clone())),
        tokio::spawn(spend_wake_loop(shared.clone())),
        tokio::spawn(cj::serve(shared.clone())),
    ]);
    let _ = tokio::time::timeout(RELAY_READY_WAIT, relay_ready).await;

    if let Some(path) = &shared.config.runtime {
        write_runtime(path, listen.port())?;
    }
    if let Some(ready) = &shared.config.ready {
        write_ready(&ready.file, shared.config.generation, &ready.version)?;
    }
    Ok(Running { shared, tasks })
}

impl Running {
    /// The bound direct-channel address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.shared.listen
    }

    /// The bound WebSocket direct-channel address, when one is configured.
    #[must_use]
    pub fn websocket_addr(&self) -> Option<SocketAddr> {
        self.shared.listen_websocket
    }

    /// The WebSocket listener's own hint URL: `ws://ADDR/`, or
    /// `wss://NAME:PORT/` when the listener terminates TLS.
    #[must_use]
    pub fn websocket_url(&self) -> Option<String> {
        self.shared
            .listen_websocket
            .map(|listen| websocket::url(listen, self.shared.config.websocket_tls.as_ref()))
    }

    /// The host key: the `coder-access` store's key.
    #[must_use]
    pub fn host_key(&self) -> &str {
        &self.shared.host_key
    }

    /// The owner key the access store was initialized with.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.shared.owner
    }

    /// The NIP-REACH host generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.shared.config.generation
    }

    /// The grant store this host serves from.
    #[must_use]
    pub fn authority(&self) -> &Arc<Authority> {
        &self.shared.authority
    }

    /// The iroh endpoint's address with the sockets it bound, when the
    /// host serves iroh: what a device on this machine dials.
    #[must_use]
    pub fn iroh_addr(&self) -> Option<openagents_connect::iroh::EndpointAddr> {
        self.shared
            .iroh
            .get()
            .map(|iroh| iroh.endpoint.local_addr())
    }

    /// The control socket's path, when the host serves one.
    #[must_use]
    pub fn control_path(&self) -> Option<&Path> {
        self.shared
            .config
            .control
            .as_ref()
            .map(|c| c.path.as_path())
    }

    /// Wait until a local action asks the host to start again, such as a
    /// project added over the control socket. The caller shuts this host
    /// down and starts it again.
    pub async fn restart_requested(&self) {
        let mut receiver = self.shared.restart.subscribe();
        let _ = receiver.wait_for(|wanted| *wanted).await;
    }

    /// Publish presence and hints to every enrolled device now.
    pub async fn publish_reach(&self) {
        publish_reach(&self.shared).await;
    }

    /// Stop serving: end every terminal's process tree, stop the listener
    /// and relay loops, and remove the runtime record.
    pub async fn shutdown(self) {
        for task in &self.tasks {
            task.abort();
        }
        let shared = self.shared.clone();
        let _ = tokio::task::spawn_blocking(move || shared.pty.shutdown()).await;
        if let Some(iroh) = self.shared.iroh.get() {
            iroh.shutdown().await;
        }
        if let Some(control) = &self.shared.config.control {
            let _ = std::fs::remove_file(&control.path);
        }
        if let Some(path) = &self.shared.config.runtime {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Republish presence and hints when the enrolled device set changes, and
/// otherwise at the configured period.
async fn presence_loop(shared: Arc<Shared>) {
    let mut last: Option<(Vec<String>, Instant)> = None;
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    loop {
        ticker.tick().await;
        let Ok(now) = unix_time() else { continue };
        let devices = shared.authority.active_devices(None, now);
        let due = last.as_ref().is_none_or(|(known, at)| {
            *known != devices || at.elapsed() >= shared.config.presence_every
        });
        if due {
            publish_reach(&shared).await;
            last = Some((devices, Instant::now()));
        }
    }
}

async fn publish_reach(shared: &Shared) {
    let Ok(now) = unix_time() else { return };
    for device in shared.authority.active_devices(None, now) {
        publish_reach_to(shared, &device, now).await;
    }
}

/// Publish presence and hints to one enrolled device now.
pub(crate) async fn publish_reach_to(shared: &Shared, device: &str, now: u64) {
    for event in reach_events(shared, device, now).unwrap_or_default() {
        shared.publisher.everywhere(&event).await;
    }
}

/// Presence and hints sealed to one device.
fn reach_events(shared: &Shared, device: &str, now: u64) -> Result<Vec<nostr::domain::Event>> {
    let presence = Presence {
        v: coder_reach::presence::SCHEMA.into(),
        requires: vec![],
        host: shared.host_key.clone(),
        owner: shared.owner.clone(),
        generation: shared.config.generation,
        protocol: PROTOCOL_VERSION,
        compatibility: VersionRange {
            min: PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
        capabilities: CAPABILITIES.iter().map(|c| (*c).to_owned()).collect(),
        observed_at: now,
        // Coarse telemetry lets placement rank this host. A value the host
        // cannot read withholds the whole sample; placement then skips it.
        telemetry: shared
            .config
            .telemetry
            .then(crate::telemetry::sample)
            .flatten(),
        meta: None,
    };
    let hints = Hints {
        v: coder_reach::hints::SCHEMA.into(),
        requires: vec![],
        host: shared.host_key.clone(),
        generation: shared.config.generation,
        issued_at: now,
        expires_at: now + HINT_LIFETIME,
        hints: hints(shared, now),
        meta: None,
    };
    let presence_box = mailbox::mailbox(&shared.secret, device, Stream::Presence)?;
    let hints_box = mailbox::mailbox(&shared.secret, device, Stream::Hints)?;
    let mut events = vec![
        presence.seal(
            &shared.secret,
            device,
            &presence_box,
            now + shared.config.presence_every.as_secs() * 3 + 60,
        )?,
        hints.seal(&shared.secret, device, &hints_box)?,
    ];
    // A host with an iroh endpoint also publishes the v2 record, which
    // alone carries the `iroh` hint; a v1 reader skips it.
    if let Some(v2) = hints_v2(shared, &hints, now) {
        events.push(v2.seal(&shared.secret, device, &hints_box)?);
    }
    Ok(events)
}

/// The v2 hint record: the v1 hints and the host's `iroh` hint, or `None`
/// when the host serves no iroh endpoint with anything to dial.
fn hints_v2(shared: &Shared, v1: &Hints, now: u64) -> Option<HintsV2> {
    let iroh = shared.iroh.get()?.hint(now)?;
    let mut record = HintsV2::from_v1(v1, Some(iroh));
    record.hints.truncate(coder_reach::hints::MAX_HINTS);
    record.validate().is_ok().then_some(record)
}

/// The iroh secret key: from the key source, or a file under
/// `~/.openagents/connect` beside the access store, created on first use.
fn iroh_secret(config: &Config) -> Result<openagents_connect::iroh::SecretKey> {
    use openagents_connect::keys::{FileKeySource, KeyName, iroh_key};
    let failed = |_| Error::Config("the iroh key cannot be read or created".into());
    match &config.keys {
        Some(keys) => iroh_key(keys.0.as_ref(), KeyName::HostIroh).map_err(failed),
        None => {
            let directory = config
                .access
                .parent()
                .map_or_else(|| PathBuf::from("connect"), |parent| parent.join("connect"));
            iroh_key(&FileKeySource::new(directory), KeyName::HostIroh).map_err(failed)
        }
    }
}

fn hints(shared: &Shared, now: u64) -> Vec<Hint> {
    let class = |listen: SocketAddr| {
        if listen.ip().is_loopback() {
            Class::Loopback
        } else {
            Class::Lan
        }
    };
    let listener = Hint {
        class: class(shared.listen),
        transport: Transport::Tcp,
        address: shared.listen.to_string(),
        status: Status::Reachable,
        observed_at: now,
    };
    // With TLS the hint names the certificate's DNS name. When that name and
    // the bound address disagree about loopback, as for a loopback listener
    // behind a forwarder, the hint fails validation below and is left out;
    // the operator advertises the forwarded endpoint instead.
    let websocket = shared.listen_websocket.map(|listen| Hint {
        class: class(listen),
        transport: Transport::Websocket,
        address: websocket::url(listen, shared.config.websocket_tls.as_ref()),
        status: Status::Reachable,
        observed_at: now,
    });
    let advertised: Vec<Hint> = shared
        .config
        .advertise
        .iter()
        .map(|a| Hint {
            class: a.class,
            transport: a.transport(),
            address: a.address.clone(),
            status: Status::Unknown,
            observed_at: now,
        })
        .collect();
    let relays = shared
        .config
        .relays
        .iter()
        .map(|relay| Hint {
            class: Class::Relay,
            transport: Transport::Nostr,
            address: relay.clone(),
            status: Status::Unknown,
            observed_at: now,
        })
        .collect();
    let listeners = if shared.config.advertise_listener {
        std::iter::once(listener).chain(websocket).collect()
    } else {
        Vec::new()
    };
    merge_hints(listeners, advertised, relays)
}

/// The listeners' own hints, then the advertised endpoints, then the relays,
/// each valid and each once. An advertised endpoint that names a listener's
/// own address, such as a tailnet `wss` URL for a listener bound to the
/// tailnet address, states its class, so the listener's copy is dropped: a
/// duplicate would invalidate the whole hint set.
fn merge_hints(listeners: Vec<Hint>, advertised: Vec<Hint>, relays: Vec<Hint>) -> Vec<Hint> {
    let listeners: Vec<Hint> = listeners
        .into_iter()
        .filter(|own| {
            !advertised
                .iter()
                .any(|a| a.transport == own.transport && a.address == own.address)
        })
        .collect();
    let mut merged: Vec<Hint> = Vec::new();
    for hint in listeners.into_iter().chain(advertised).chain(relays) {
        if hint.validate().is_ok()
            && !merged
                .iter()
                .any(|held| held.transport == hint.transport && held.address == hint.address)
        {
            merged.push(hint);
        }
    }
    merged.truncate(coder_reach::hints::MAX_HINTS);
    merged
}

/// Whether `principal` still holds `operate` under the same grant and epoch
/// now. The owner, with no grant, always does.
pub(crate) fn standing(authority: &Authority, principal: &crate::tasks::Principal) -> bool {
    match (&principal.grant, principal.epoch) {
        (None, None) => true,
        (Some(grant), Some(epoch)) => unix_time().is_ok_and(|now| {
            authority
                .check(&principal.device, grant, epoch, now)
                .is_ok_and(|rights| rights.contains(coder_access::Right::Operate))
        }),
        _ => false,
    }
}

/// The most task summaries the first sweep publishes.
const FIRST_SWEEP_TASKS: usize = 50;

/// How often the summary loop reads the task store's stamp.
const STAMP_EVERY: Duration = Duration::from_millis(250);
/// How often the summary loop sweeps whether or not the stamp moved, so
/// time-based command decisions, such as a lapsed queue lease, still run.
const SWEEP_EVERY: Duration = Duration::from_secs(5);

/// Publish a summary whenever a task's revision changes outside a device
/// operation, such as an auto-started run starting or finishing. The loop
/// sweeps as soon as the task store's stamp moves, and every
/// [`SWEEP_EVERY`] regardless. The first sweep publishes the newest tasks
/// so devices catch up after a restart.
async fn summary_loop(shared: Arc<Shared>) {
    let mut known: BTreeMap<String, u64> = BTreeMap::new();
    let mut first = true;
    let mut ticker = tokio::time::interval(STAMP_EVERY);
    let mut swept: Option<(Instant, Option<Vec<u8>>)> = None;
    loop {
        ticker.tick().await;
        let tasks = shared.tasks.clone();
        let stamp = tokio::task::spawn_blocking(move || tasks.stamp())
            .await
            .ok()
            .flatten();
        let due = swept.as_ref().is_none_or(|(at, seen)| {
            at.elapsed() >= SWEEP_EVERY || stamp.is_some() && *seen != stamp
        });
        if !due {
            continue;
        }
        swept = Some((Instant::now(), stamp));
        let tasks = shared.tasks.clone();
        let authority = shared.authority.clone();
        let Ok(current) = tokio::task::spawn_blocking(move || {
            // Held commands run only while their sender still holds
            // `operate` under the same grant and epoch.
            let standing = |principal: &crate::tasks::Principal| standing(&authority, principal);
            tasks.tick(&standing);
            tasks.current()
        })
        .await
        else {
            continue;
        };
        let mut changed: Vec<TaskRef> = current
            .into_iter()
            .filter(|task| known.get(&task.task) != Some(&task.revision))
            .collect();
        for task in &changed {
            known.insert(task.task.clone(), task.revision);
        }
        if first {
            changed.sort_by_key(|task| std::cmp::Reverse(task.revision));
            changed.truncate(FIRST_SWEEP_TASKS);
            first = false;
        }
        for task in &changed {
            summarize(&shared, task).await;
        }
    }
}

/// How often the host looks for new spend requests to wake a phone for.
const SPEND_WAKE_EVERY: Duration = Duration::from_secs(1);

/// Wake the phone a new spend request draws on: publish a spend wake
/// (`crate::spend::wake`) to each device that holds an open request no wake
/// went out for. Requests are recorded by other processes (`coder host spend
/// request`, the x402 phone payer), so the host watches the book's file and
/// reads it only when it moved.
async fn spend_wake_loop(shared: Arc<Shared>) {
    let book = crate::spend::Book::open(&shared.config.access);
    let mut seen = None;
    let mut ticker = tokio::time::interval(SPEND_WAKE_EVERY);
    loop {
        ticker.tick().await;
        let stamp = book.stamp();
        if stamp.is_none() || stamp == seen {
            continue;
        }
        seen = stamp;
        let Ok(now) = unix_time() else { continue };
        let reader = book.clone();
        let Ok(Ok(devices)) = tokio::task::spawn_blocking(move || reader.wakes(now)).await else {
            continue;
        };
        // Saving the book moved its stamp; that is not news.
        if !devices.is_empty() {
            seen = book.stamp();
        }
        let operators = shared.authority.active_devices(Some(Right::Operate), now);
        for device in devices {
            if !operators.contains(&device) {
                continue;
            }
            if let Ok(event) =
                crate::spend::wake::Wake::new(&shared.secret, &device, now).seal(&shared.secret)
            {
                shared.publisher.everywhere(&event).await;
            }
        }
    }
}

/// Publish an activity summary for a changed task to every device that
/// holds `observe`. The headline is the generic phrase for the phase, or
/// the host's typed note, such as a missing model capacity: a title comes
/// from a device, and a summary never carries sent text.
pub(crate) async fn summarize(shared: &Shared, task: &TaskRef) {
    let Ok(now) = unix_time() else { return };
    let tasks = shared.tasks.clone();
    let id = task.task.clone();
    let note = tokio::task::spawn_blocking(move || tasks.note(&id))
        .await
        .ok()
        .flatten();
    // A waiting question or approval asks for the device's attention; the
    // summary never carries the question itself.
    let attention = match (task.phase, note.and_then(crate::tasks::Note::attention)) {
        (Phase::Waiting, Some(attention)) => attention,
        (Phase::Completed, _) => Attention::Completed,
        (Phase::Failed, _) => Attention::Failed,
        _ => Attention::None,
    };
    let note = note.map(crate::tasks::Note::headline);
    let draft = SummaryDraft {
        host: &shared.host_key,
        subject_kind: SubjectKind::Task,
        subject: &task.task,
        sequence: task.revision,
        phase: task.phase,
        headline: note
            .as_deref()
            .unwrap_or_else(|| activity_summary::generic_headline(SubjectKind::Task, task.phase)),
        attention,
        updated_at: now,
    };
    let Ok(summary) = activity_summary::encode(&draft) else {
        return;
    };
    for device in shared.authority.active_devices(Some(Right::Observe), now) {
        let (Ok(key), Ok(mailbox)) = (
            XOnlyPublicKey::from_str(&device),
            mailbox::mailbox(&shared.secret, &device, Stream::Summaries),
        ) else {
            continue;
        };
        if let Ok(event) = activity_summary::seal(
            &summary,
            &shared.secret,
            &key,
            &mailbox,
            now + SUMMARY_RETENTION,
            coder_reach::random_bytes(),
        ) {
            shared.publisher.everywhere(&event).await;
        }
    }
}

/// Write the SSH runtime record: schema, process ID, and listener port.
fn write_runtime(path: &Path, port: u16) -> Result<()> {
    let body = format!(
        "schema={RUNTIME_SCHEMA}\npid={}\nport={port}\n",
        std::process::id()
    );
    write_private(path, body.as_bytes())
}

/// Write the host service's ready record.
fn write_ready(path: &Path, generation: u64, version: &str) -> Result<()> {
    let mut capabilities: Vec<&str> = CAPABILITIES.to_vec();
    capabilities.sort_unstable();
    let body = serde_json::json!({
        "schema": READY_SCHEMA,
        "generation": generation,
        "version": version,
        "protocol_version": PROTOCOL_VERSION,
        "capabilities": capabilities,
    });
    write_private(path, body.to_string().as_bytes())
}

/// Write a private file by a temporary name and a rename.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let failed = || Error::Config(format!("cannot write {}", path.display()));
    let parent = path.parent().ok_or_else(failed)?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|_| failed())?;
    let name = path.file_name().ok_or_else(failed)?.to_string_lossy();
    let temporary: PathBuf = parent.join(format!(".{name}.{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| failed())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| failed())?;
    std::fs::rename(&temporary, path).map_err(|_| failed())
}

#[cfg(test)]
mod hint_tests {
    use super::*;

    fn hint(class: Class, transport: Transport, address: &str) -> Hint {
        Hint {
            class,
            transport,
            address: address.into(),
            status: Status::Reachable,
            observed_at: 1,
        }
    }

    #[test]
    fn an_advertised_copy_of_a_listener_states_its_class_once() {
        let url = "wss://box.example.ts.net:47101/";
        let merged = merge_hints(
            vec![
                hint(Class::Loopback, Transport::Tcp, "127.0.0.1:47100"),
                hint(Class::Lan, Transport::Websocket, url),
            ],
            vec![
                hint(Class::Tailnet, Transport::Websocket, url),
                hint(Class::Tailnet, Transport::Websocket, url),
            ],
            vec![hint(Class::Relay, Transport::Nostr, "wss://relay.example/")],
        );
        let seen: Vec<(Class, &str)> = merged
            .iter()
            .map(|h| (h.class, h.address.as_str()))
            .collect();
        assert_eq!(
            seen,
            [
                (Class::Loopback, "127.0.0.1:47100"),
                (Class::Tailnet, url),
                (Class::Relay, "wss://relay.example/"),
            ]
        );
        let set = Hints {
            v: coder_reach::hints::SCHEMA.into(),
            requires: vec![],
            host: "ab".repeat(32),
            generation: 1,
            issued_at: 2,
            expires_at: 3,
            hints: merged,
            meta: None,
        };
        set.validate().unwrap();
    }
}
