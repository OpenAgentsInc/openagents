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
mod relay;
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
    let access = coder_access::host::Host::new(&config.access, config.policy);
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
    });

    let (ready, relay_ready) = tokio::sync::oneshot::channel();
    let mut tasks = vec![];
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
        for event in reach_events(shared, &device, now).unwrap_or_default() {
            shared.publisher.everywhere(&event).await;
        }
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
    Ok(vec![
        presence.seal(
            &shared.secret,
            device,
            &presence_box,
            now + shared.config.presence_every.as_secs() * 3 + 60,
        )?,
        hints.seal(&shared.secret, device, &hints_box)?,
    ])
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

/// The most task summaries the first sweep publishes.
const FIRST_SWEEP_TASKS: usize = 50;

/// Publish a summary whenever a task's revision changes outside a device
/// operation, such as an auto-started run starting or finishing. The first
/// sweep publishes the newest tasks so devices catch up after a restart.
async fn summary_loop(shared: Arc<Shared>) {
    let mut known: BTreeMap<String, u64> = BTreeMap::new();
    let mut first = true;
    let mut ticker = tokio::time::interval(Duration::from_secs(5));
    loop {
        ticker.tick().await;
        let tasks = shared.tasks.clone();
        let Ok(current) = tokio::task::spawn_blocking(move || tasks.current()).await else {
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

/// Publish an activity summary for a changed task to every device that
/// holds `observe`. The headline is the generic phrase for the phase: a
/// title comes from a device, and a summary never carries sent text.
pub(crate) async fn summarize(shared: &Shared, task: &TaskRef) {
    let Ok(now) = unix_time() else { return };
    let attention = match task.phase {
        Phase::Completed => Attention::Completed,
        Phase::Failed => Attention::Failed,
        _ => Attention::None,
    };
    let draft = SummaryDraft {
        host: &shared.host_key,
        subject_kind: SubjectKind::Task,
        subject: &task.task,
        sequence: task.revision,
        phase: task.phase,
        headline: activity_summary::generic_headline(SubjectKind::Task, task.phase),
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
