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

pub(crate) mod activity;
mod cj;
mod direct;
pub(crate) mod dispatch;
pub(crate) mod iroh;
pub mod keys;
pub(crate) mod nearby;
mod relay;
mod standing;
mod terminal;
mod threads;
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
/// History lines each terminal's emulator keeps.
const TERMINAL_HISTORY: usize = 1000;

/// Runs `shell` for a terminal's `shell` launch, as a login shell, with the
/// shell-integration hooks when it is zsh, bash, or fish, and answers the
/// hooks' files for the host to keep. The user's own startup files come
/// from the terminals' `HOME`.
fn terminal_shell(
    terminals: &mut coder_pty::host::Config,
    shell: Option<&Path>,
) -> Option<terminal_core::integration::Hooks> {
    let shell = shell?;
    terminals.shell = shell.to_path_buf();
    terminals.shell_args = vec!["-l".into()];
    terminals
        .base_env
        .push(("SHELL".into(), shell.display().to_string()));
    let home = terminals
        .base_env
        .iter()
        .find(|(name, _)| name == "HOME")
        .map(|(_, home)| PathBuf::from(home))?;
    let (hooks, start) = terminal_core::integration::Hooks::for_shell(shell, &home)?;
    terminals.shell_args = start.args;
    terminals.base_env.extend(start.env);
    Some(hooks)
}

/// State every serving task shares.
pub(crate) struct Shared {
    /// The shell-integration files terminals start with, kept while the
    /// host runs.
    pub(crate) _shell_hooks: Option<terminal_core::integration::Hooks>,
    pub(crate) local_handoffs: tokio::sync::Mutex<()>,
    pub(crate) local_tasks: std::sync::Mutex<()>,
    pub(crate) local_history: std::sync::Mutex<Option<coder_connect::client::Client>>,
    pub(crate) chats: std::sync::Mutex<Option<openagents_chat::basic_chats::BasicChats>>,
    pub(crate) config: Config,
    pub(crate) authority: Arc<Authority>,
    pub(crate) secret: SecretKey,
    pub(crate) host_key: String,
    pub(crate) owner: String,
    pub(crate) pty: Arc<coder_pty::host::Host>,
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
    /// Terminal session records, beside the access store.
    pub(crate) sessions: crate::terminal_sessions::Book,
    /// The iroh endpoint, once bound.
    pub(crate) iroh: std::sync::OnceLock<iroh::Listener>,
    /// Woken when a local action changed the grants, so open channels
    /// recheck at once instead of at their next tick.
    pub(crate) grants_changed: tokio::sync::Notify,
    /// Set when a local action changed what the host serves, such as its
    /// projects, and the host must start again to serve it.
    pub(crate) restart: tokio::sync::watch::Sender<bool>,
    /// The coding agents on this computer as presence capabilities
    /// (`engine-<state>-<engine>`, #10119), read off the runtime by the
    /// presence loop; empty until the first reading, and always where the
    /// serving program lists none.
    pub(crate) engines: std::sync::Mutex<Vec<String>>,
    /// Tailnet admission as the control socket's `tailnet_status` reports
    /// it (#10125): `None` until the serving program says.
    pub(crate) tailnet: std::sync::Mutex<Option<Tailnet>>,
    /// The Agent Studio stream `studio.snapshot` and `studio.update`
    /// answer from: one per process, so a restart reads as a new stream.
    pub(crate) studio: std::sync::Mutex<coder_access::studio::Stream>,
    /// Requests in flight on every path, and when the last one ended: what
    /// a restart for an update waits out.
    pub(crate) activity: activity::Activity,
    /// Set when the host stops taking control requests, to stop or start
    /// again ([`Running::stop_taking_requests`]).
    pub(crate) closing: std::sync::atomic::AtomicBool,
    /// A second descriptor for the control socket's listener, so a restart
    /// hands the bound socket to the new program: a client that connects
    /// meanwhile waits in its queue instead of finding nothing there.
    #[cfg(unix)]
    pub(crate) control_listener: std::sync::Mutex<Option<std::os::fd::OwnedFd>>,
}

/// Tailnet admission's state, as the serving program started it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tailnet {
    /// Serving on this tailnet address.
    On { address: SocketAddr, chats: bool },
    /// Configured, but it could not start.
    Off { reason: String },
}

/// A running host.
pub struct Running {
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
    /// The control socket's accept loop, stopped first when the host
    /// stops or starts again.
    control: Option<JoinHandle<()>>,
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
    let access = keys::access_store(&config.access, config.policy, config.keys.as_ref());
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
    if config.keys.is_none() && !access.state_path().exists() {
        return Err(Error::Config(
            "the host access store is not initialized (no owner or host key). \
             Start a new host with `openagents host serve --keys \"$HOME/.openagents/connect\" --iroh`; \
             this creates its keys and owner on first start."
                .into(),
        ));
    }
    let authority = Arc::new(Authority::open(access)?);
    let secret = authority.signing_key()?;
    let host_key = coder_reach::pubkey(&secret);
    let owner = authority.owner()?;

    let mut terminals = coder_pty::host::Config::new();
    let shell_hooks = terminal_shell(&mut terminals, config.terminal_shell.as_deref());
    // One emulator per terminal answers its program's queries and owns its
    // side effects, so devices watching together never answer twice.
    terminals.emulator = Some(coder_vt::Authority::factory(TERMINAL_HISTORY));
    bundled_commands_first(&mut terminals.base_env);
    terminals.generation = mailbox::terminal_generation(&host_key, config.generation);
    for (label, root) in &config.workspaces {
        terminals = terminals.workspace(mailbox::workspace_id(label), root);
    }
    // Shares are sealed to their grantee under the host key.
    terminals.shares = Some(Arc::new(crate::share::Signer(authority.signing_key()?)));
    let pty = Arc::new(coder_pty::host::Host::new(
        terminals,
        Arc::new(Grants(authority.clone())),
    ));

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
    let sessions = crate::terminal_sessions::Book::beside(&config.access);
    let shared = Arc::new(Shared {
        _shell_hooks: shell_hooks,
        local_handoffs: tokio::sync::Mutex::new(()),
        local_tasks: std::sync::Mutex::new(()),
        local_history: std::sync::Mutex::new(None),
        chats: std::sync::Mutex::new(None),
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
        sessions,
        iroh: std::sync::OnceLock::new(),
        grants_changed: tokio::sync::Notify::new(),
        restart: tokio::sync::watch::channel(false).0,
        engines: std::sync::Mutex::new(Vec::new()),
        tailnet: std::sync::Mutex::new(None),
        studio: std::sync::Mutex::new(dispatch::studio_stream()),
        activity: activity::Activity::default(),
        closing: std::sync::atomic::AtomicBool::new(false),
        #[cfg(unix)]
        control_listener: std::sync::Mutex::new(None),
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
    let mut control_task = None;
    let control = match shared.config.control.clone() {
        Some(control) => Some(crate::control::bind(&control).await?),
        None => None,
    };
    if let Some(bound) = control {
        #[cfg(unix)]
        {
            *shared
                .control_listener
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = bound.keep();
        }
        // Threads `openagents chat` kept without a host join this host's
        // store before the socket answers, so the first list shows them.
        if let Some(home) = shared.config.chat_home.clone() {
            migrate_chat_home(&shared, &home);
        }
        control_task = Some(tokio::spawn(crate::control::serve(shared.clone(), bound)));
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
    Ok(Running {
        shared,
        tasks,
        control: control_task,
    })
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

    /// Connects a host-local agent producer to an owner's explicit terminal handoff.
    /// The producer receives no terminal reader or task execution authority.
    pub fn terminal_agent_producer(
        &self,
        agent: &str,
        terminal: &coder_pty::wire::TerminalRef,
        lease: &str,
    ) -> std::result::Result<coder_pty::host::AgentProducer, coder_pty::wire::Refusal> {
        self.shared.pty.agent_producer(agent, terminal, lease)
    }

    /// How many terminals the host holds, running or ended. A host-local
    /// diagnostic.
    #[must_use]
    pub fn terminals(&self) -> usize {
        self.shared.pty.terminals()
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

    /// Record whether tailnet admission serves, for `tailnet_status`.
    pub fn set_tailnet(&self, tailnet: Tailnet) {
        *self
            .shared
            .tailnet
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tailnet);
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

    /// Whether nobody is using the host now: no request in flight on any
    /// path or answered within [`activity::QUIET`], no chat reply
    /// streaming, and no device holding a terminal.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.shared.activity.quiet_for(activity::QUIET)
            && !chats_streaming(&self.shared)
            && self.shared.pty.terminals() == 0
    }

    /// Return once [`Running::idle`] holds.
    pub async fn until_idle(&self) {
        while !self.idle() {
            tokio::time::sleep(activity::LOOK_EVERY).await;
        }
    }

    /// Return once no request is in flight and no chat reply streams, or
    /// `most` has passed: what a stop waits out, so a request being
    /// answered gets its answer. `true` when it drained.
    pub async fn drain(&self, most: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + most;
        loop {
            if self.shared.activity.in_flight() == 0 && !chats_streaming(&self.shared) {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(activity::LOOK_EVERY).await;
        }
    }

    /// Stop taking control requests: no new connection is accepted (one
    /// that arrives waits in the socket's queue, for the program that
    /// serves it next), and an open connection ends before its next
    /// request. A request already begun is answered; [`Running::drain`]
    /// waits for it.
    pub fn stop_taking_requests(&self) {
        self.shared
            .closing
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(control) = &self.control {
            control.abort();
        }
    }

    /// Requests being answered now, on every path.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.shared.activity.in_flight()
    }

    /// Ready the control socket's listener for the program this one is
    /// replaced by (`exec`): the descriptor stays open across it, and the
    /// value for [`crate::control::HANDOVER_ENV`] names it. `None` when the
    /// host serves no control socket.
    #[cfg(unix)]
    #[must_use]
    pub fn hand_over_control(&self) -> Option<String> {
        let fd = self
            .shared
            .control_listener
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()?;
        crate::control::socket::hand_over(fd)
    }

    /// Publish presence and hints to every enrolled device now.
    pub async fn publish_reach(&self) {
        publish_reach(&self.shared).await;
    }

    /// Stop serving: end every terminal's process tree, stop the listener
    /// and relay loops, and remove the runtime record.
    pub async fn shutdown(self) {
        self.stop(false).await;
    }

    /// [`Running::shutdown`] for a restart that took the control socket
    /// ([`Running::hand_over_control`]): its file stays, for the program
    /// that serves it next.
    pub async fn shutdown_for_restart(self) {
        self.stop(true).await;
    }

    async fn stop(self, keep_socket: bool) {
        for task in self.tasks.iter().chain(&self.control) {
            task.abort();
        }
        let shared = self.shared.clone();
        let _ = tokio::task::spawn_blocking(move || shared.pty.shutdown()).await;
        if let Some(iroh) = self.shared.iroh.get() {
            iroh.shutdown().await;
        }
        if let Some(control) = &self.shared.config.control
            && !keep_socket
        {
            let _ = std::fs::remove_file(&control.path);
        }
        if let Some(path) = &self.shared.config.runtime {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Whether a chat reply streams in the host's store, or the worker ranks
/// suggestions. Replies that ended are moved into their threads first, as a
/// read would.
fn chats_streaming(shared: &Shared) -> bool {
    let mut state = shared
        .chats
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(chats) = state.as_mut() else {
        return false;
    };
    chats.settle(unix_time().unwrap_or(0));
    chats.streaming()
}

/// How often the presence loop reads this computer's coding agents again
/// (#10119). The serving program's own reading keeps its answer as long.
const ENGINES_EVERY: Duration = Duration::from_secs(15);

/// This computer's coding agents as presence capabilities (#10119), in the
/// order listed, within the bound a presence carries.
pub(crate) fn engine_capabilities(engines: &[openagents_chat::router::Engine]) -> Vec<String> {
    use coder_access::protocol::{MAX_ENGINE_FLAGS, engine_flag};
    engines
        .iter()
        .filter_map(|engine| engine_flag(&engine.engine, engine.state.word()))
        .take(MAX_ENGINE_FLAGS)
        .collect()
}

/// Republish presence and hints when the enrolled device set changes or
/// this computer's coding agents change (#10119: a sign-in, a sign-out, a
/// usage limit, or the settings), and otherwise at the configured period.
async fn presence_loop(shared: Arc<Shared>) {
    let mut last: Option<(Vec<String>, Instant)> = None;
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut engines_read: Option<Instant> = None;
    let mut reading: Option<JoinHandle<Vec<String>>> = None;
    loop {
        ticker.tick().await;
        let Ok(now) = unix_time() else { continue };
        // The listing probes logins, so it runs off the runtime.
        if reading.is_none() && engines_read.is_none_or(|at| at.elapsed() >= ENGINES_EVERY) {
            if let Some(list) = crate::control::local_engines() {
                reading = Some(tokio::task::spawn_blocking(move || {
                    engine_capabilities(&list())
                }));
            }
            engines_read = Some(Instant::now());
        }
        let mut engines_changed = false;
        if reading.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(handle) = reading.take()
            && let Ok(flags) = handle.await
        {
            let mut held = shared
                .engines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *held != flags {
                *held = flags;
                engines_changed = true;
            }
        }
        let devices = shared.authority.active_devices(None, now);
        let due = engines_changed
            || last.as_ref().is_none_or(|(known, at)| {
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

/// The host's own capabilities, then each one its task owner adds that is
/// not already there ([`Tasks::capabilities`]), within the presence bound.
fn presence_capabilities(added: Vec<String>) -> Vec<String> {
    let mut capabilities: Vec<String> = CAPABILITIES.iter().map(|c| (*c).to_owned()).collect();
    for capability in added {
        if capabilities.len() >= coder_reach::presence::MAX_CAPABILITIES {
            break;
        }
        if !capability.is_empty() && !capabilities.contains(&capability) {
            capabilities.push(capability);
        }
    }
    capabilities
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
        capabilities: presence_capabilities(
            shared
                .tasks
                .capabilities()
                .into_iter()
                .chain(
                    shared
                        .engines
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .iter()
                        .cloned(),
                )
                .collect(),
        ),
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
    // Each studio goal decision this host raised, by subject: the
    // coordinator sequence that opened it.
    let mut raised: BTreeMap<String, u64> = BTreeMap::new();
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
        let tasks = shared.tasks.clone();
        let Ok(open) = tokio::task::spawn_blocking(move || tasks.goal_decisions()).await else {
            continue;
        };
        let Ok(now) = unix_time() else { continue };
        for summary in goal_summaries(&shared.host_key, &mut raised, &open, now) {
            publish_summary(&shared, &summary, now).await;
        }
        let tasks = shared.tasks.clone();
        let Ok(reports) = tokio::task::spawn_blocking(move || tasks.agent_reports()).await else {
            continue;
        };
        for report in reports {
            agent_report(&shared, &report, now).await;
        }
    }
}

/// Carry a workshop agent's report to its thread and to every device that
/// holds `observe`: the full report as a reply in the agent's own chat
/// thread, which syncs to the desktop app and the phone, and a NIP-WS
/// activity summary whose headline is host state.
async fn agent_report(shared: &Arc<Shared>, report: &crate::tasks::AgentReport, now: u64) {
    let thread_shared = shared.clone();
    let thread = report.thread.clone();
    let title = report.agent.clone();
    let text = report.text.clone();
    let noted = tokio::task::spawn_blocking(move || {
        use openagents_chat::service::Command;
        let created = crate::control::apply_chat(
            &thread_shared,
            Command::Create {
                chat: thread.clone(),
            },
        );
        if created.as_ref().is_ok_and(|snapshot| snapshot.total == 0) {
            let _ = crate::control::apply_chat(
                &thread_shared,
                Command::Rename {
                    chat: thread.clone(),
                    title,
                },
            );
        }
        crate::control::apply_chat(&thread_shared, Command::Note { chat: thread, text }).is_ok()
    })
    .await
    .unwrap_or(false);
    if !noted {
        eprintln!(
            "openagents host: {}'s report did not reach her thread",
            report.agent
        );
    }
    let draft = SummaryDraft {
        host: &shared.host_key,
        subject_kind: SubjectKind::Task,
        subject: &report.subject,
        sequence: report.sequence,
        phase: report.phase,
        headline: &report.headline,
        attention: report.attention,
        updated_at: now,
    };
    if let Ok(summary) = activity_summary::encode(&draft) {
        publish_summary(shared, &summary, now).await;
    }
}

/// The headline of a goal decision's summary once it is answered.
const DECISION_ANSWERED: &str = "Decision answered";

/// The activity summaries studio goal decisions need now, given the ones
/// this host `raised` (updated in place): a newly open decision asks for
/// input (phase `waiting`, attention `input`) under the goal's own subject
/// with its headline from host state; one that closed is superseded by a
/// summary that asks for nothing. A goal decision opened at coordinator
/// sequence `s` is summarized at `2s` and closed at `2s + 1`, so each
/// supersedes the last and a later decision of the same goal supersedes
/// both. The summary sealed to a device is what wakes its phone through a
/// relay's NIP-PL executor, whose lease matches the device's `3188`
/// artifacts.
pub(crate) fn goal_summaries(
    host: &str,
    raised: &mut BTreeMap<String, u64>,
    open: &[crate::tasks::GoalDecision],
    now: u64,
) -> Vec<activity_summary::ActivitySummary> {
    let mut out = Vec::new();
    for decision in open {
        if raised.get(&decision.subject) == Some(&decision.sequence) {
            continue;
        }
        let draft = SummaryDraft {
            host,
            subject_kind: SubjectKind::Task,
            subject: &decision.subject,
            sequence: decision.sequence.saturating_mul(2),
            phase: Phase::Waiting,
            headline: &decision.headline,
            attention: Attention::Input,
            updated_at: now,
        };
        if let Ok(summary) = activity_summary::encode(&draft) {
            raised.insert(decision.subject.clone(), decision.sequence);
            out.push(summary);
        }
    }
    let closed: Vec<(String, u64)> = raised
        .iter()
        .filter(|(subject, _)| !open.iter().any(|decision| decision.subject == **subject))
        .map(|(subject, sequence)| (subject.clone(), *sequence))
        .collect();
    for (subject, sequence) in closed {
        raised.remove(&subject);
        let draft = SummaryDraft {
            host,
            subject_kind: SubjectKind::Task,
            subject: &subject,
            sequence: sequence.saturating_mul(2).saturating_add(1),
            phase: Phase::Running,
            headline: DECISION_ANSWERED,
            attention: Attention::None,
            updated_at: now,
        };
        if let Ok(summary) = activity_summary::encode(&draft) {
            out.push(summary);
        }
    }
    out
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
/// holds `observe`. The headline is the generic phrase for the phase, the
/// host's typed note, such as a missing model capacity, or, while a
/// studio task's question or approval waits, its seat and title: a title
/// comes from a device or the host's plan, and a summary never carries
/// sent text.
pub(crate) async fn summarize(shared: &Shared, task: &TaskRef) {
    let Ok(now) = unix_time() else { return };
    let tasks = shared.tasks.clone();
    let id = task.task.clone();
    let (note, headline) =
        tokio::task::spawn_blocking(move || (tasks.note(&id), tasks.decision_headline(&id)))
            .await
            .unwrap_or_default();
    let Some(summary) = activity(&shared.host_key, task, note, headline.as_deref(), now) else {
        return;
    };
    publish_summary(shared, &summary, now).await;
}

/// Seal `summary` to every device that holds `observe` and publish it.
async fn publish_summary(shared: &Shared, summary: &activity_summary::ActivitySummary, now: u64) {
    for device in shared.authority.active_devices(Some(Right::Observe), now) {
        let (Ok(key), Ok(mailbox)) = (
            XOnlyPublicKey::from_str(&device),
            mailbox::mailbox(&shared.secret, &device, Stream::Summaries),
        ) else {
            continue;
        };
        if let Ok(event) = activity_summary::seal(
            summary,
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

/// The same disclosed task state for paired devices and the local owner.
/// `decision` is the host-state headline of a waiting studio decision
/// ([`crate::tasks::Tasks::decision_headline`]); it replaces the note's
/// only while the summary asks for input or an approval.
pub(crate) fn activity(
    host: &str,
    task: &TaskRef,
    note: Option<crate::tasks::Note>,
    decision: Option<&str>,
    now: u64,
) -> Option<activity_summary::ActivitySummary> {
    // A waiting question or approval asks for the device's attention; the
    // summary never carries the question itself.
    let attention = match (task.phase, note.and_then(crate::tasks::Note::attention)) {
        (Phase::Waiting, Some(attention)) => attention,
        (Phase::Completed, _) => Attention::Completed,
        (Phase::Failed, _) => Attention::Failed,
        _ => Attention::None,
    };
    let note = match (attention, decision) {
        (Attention::Approval | Attention::Input, Some(decision)) => Some(decision.to_owned()),
        _ => note.map(crate::tasks::Note::headline),
    };
    let draft = SummaryDraft {
        host,
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
    activity_summary::encode(&draft).ok()
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
    #[cfg(unix)]
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let failed = || Error::Config(format!("cannot write {}", path.display()));
    let parent = path.parent().ok_or_else(failed)?;
    #[cfg(unix)]
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|_| failed())?;
    #[cfg(windows)]
    private_fs::create_dir_all(parent).map_err(|_| failed())?;
    let name = path.file_name().ok_or_else(failed)?.to_string_lossy();
    let temporary: PathBuf = parent.join(format!(".{name}.{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    // On Windows the file inherits its directory's owner-only DACL.
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary).map_err(|_| failed())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| failed())?;
    std::fs::rename(&temporary, path).map_err(|_| failed())
}

/// The desktop app's bundle ships the `openagents` command in
/// `Contents/Helpers`, beside this host's `Contents/MacOS` (not in it: on a
/// case-insensitive volume `MacOS/openagents` is the app's own `OpenAgents`).
/// Put that folder first on the `PATH` terminals get, so a phone's read-only
/// command card runs the command that came with the app, on a Mac where
/// nobody installed it. A host outside such a bundle keeps its `PATH`.
fn bundled_commands_first(env: &mut Vec<(String, String)>) {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| helpers_of(&exe))
    else {
        return;
    };
    let Some(dir) = dir.to_str().filter(|dir| !dir.contains(':')) else {
        return;
    };
    match env.iter_mut().find(|(name, _)| name == "PATH") {
        Some((_, path)) => *path = format!("{dir}:{path}"),
        None => env.push((
            "PATH".into(),
            format!("{dir}:/usr/bin:/bin:/usr/sbin:/sbin"),
        )),
    }
}

/// `Contents/Helpers` of the app bundle whose `Contents/MacOS` holds `exe`,
/// when it holds an `openagents` command.
fn helpers_of(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let helpers = macos.parent()?.join("Helpers");
    helpers.join("openagents").is_file().then_some(helpers)
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod summary_tests;

#[cfg(all(test, unix))]
#[path = "shell_tests.rs"]
mod shell_tests;

#[cfg(test)]
mod bundle_tests {
    use super::helpers_of;

    #[test]
    fn only_an_app_bundles_helpers_folder_goes_on_the_path() {
        let temp = tempfile::tempdir().unwrap();
        let contents = temp.path().join("OpenAgents.app/Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        std::fs::create_dir_all(contents.join("Helpers")).unwrap();
        let coder = contents.join("MacOS/coder");
        // No command in Helpers yet: nothing to add.
        assert_eq!(helpers_of(&coder), None);
        std::fs::write(contents.join("Helpers/openagents"), b"").unwrap();
        assert_eq!(helpers_of(&coder), Some(contents.join("Helpers")));
        // A host outside a bundle, even with a command beside it, keeps
        // its PATH.
        let loose = temp.path().join("bin");
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(loose.join("openagents"), b"").unwrap();
        assert_eq!(helpers_of(&loose.join("coder")), None);
    }
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

/// Move the threads kept in the chat home `home` without a host into this
/// host's store, when there are any. A failure leaves them where they were
/// (the next start, or `openagents chat`, tries again) and never stops the
/// host.
fn migrate_chat_home(shared: &Shared, home: &std::path::Path) {
    if !openagents_chat::migrate::pending(home) {
        return;
    }
    match crate::control::migrate_chats(shared, home) {
        Ok(report) if report.moved > 0 => eprintln!(
            "openagents host: moved {} chat threads from {} into this host",
            report.moved,
            home.display()
        ),
        Ok(_) => {}
        Err(error) => eprintln!(
            "openagents host: chat threads in {} stay there for now: {error:?}",
            home.display()
        ),
    }
}
