//! A host with an iroh endpoint (loopback, relays off) and a control
//! socket, and a phone that pairs with it over iroh, for the QR pairing
//! tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use coder_host::access::client::{finish_redeem, prepare_redeem};
use coder_host::access::protocol::{Access, HostInvitation};
use coder_host::access::{Code, RelayPolicy};
use coder_host::client::{Device, Link};
use coder_host::config::{Config, Control, Iroh};
use coder_host::reach::pubkey;
use coder_host::{NoTasks, Running};
use nostr::domain::Event;
use openagents_connect::code::ConnectCode;
use openagents_connect::control::{self, Op, Reply, Request};
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig};
use openagents_connect::enroll::{self, EnrollRequest};
use openagents_connect::stream::IrohStream;
use secp256k1::SecretKey;
#[cfg(unix)]
use tokio::net::UnixStream;

#[path = "../../../coder-control/src/tests/relay.rs"]
pub mod relay;

pub const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

pub fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

pub fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

pub struct Host {
    pub temp: tempfile::TempDir,
    pub relay: String,
    pub socket: PathBuf,
    pub root: PathBuf,
    pub store: coder_host::access::host::Host,
    pub running: Running,
    /// Every event the synthetic relay holds.
    pub events: relay::Events,
    _relay_task: tokio::task::JoinHandle<()>,
}

/// Options for [`host_with`].
pub struct Options {
    /// The user ID the control socket admits; the test's own by default.
    pub uid: u32,
    /// A workspace, for terminals.
    pub workspace: bool,
    /// The program that changes the auto-start policy.
    pub autostart: Option<PathBuf>,
    /// Serve read-only Coder chats on the relay and issue invitations to
    /// them: beside grants redeemed over iroh, and on `chats.invite`.
    pub chats: bool,
    /// The chat worker the host's threads ask; the public one by default,
    /// which a test never reaches.
    pub chat_door: Option<coder_host::config::ChatDoor>,
    /// The task owner. Unset, the host refuses task operations.
    pub tasks: Option<Arc<dyn coder_host::Tasks>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            uid: coder_host::control::own_uid(),
            workspace: true,
            autostart: None,
            chats: false,
            chat_door: None,
            tasks: None,
        }
    }
}

pub async fn host() -> Host {
    host_with(Options::default()).await
}

pub async fn host_with(options: Options) -> Host {
    let temp = tempfile::tempdir().unwrap();
    let (relay, relay_task, events) = relay::start().await;
    let access = temp.path().join("access");
    let store = coder_host::access::host::Host::new(&access, POLICY);
    store.init(&pubkey(&key())).unwrap();
    let root = temp.path().join("host");
    let socket = control_path(&temp);
    let mut config = Config::new(access, vec![relay.clone()], 3);
    config.policy = POLICY;
    config.iroh = Some(Iroh::loopback());
    config.control = Some(Control {
        path: socket.clone(),
        root: root.clone(),
        autostart: options.autostart,
        tasks: temp.path().join("tasks"),
        uid: options.uid,
    });
    config.label = "Studio Mac".into();
    config.chat_door = options.chat_door;
    config.recheck_every = Duration::from_secs(30);
    if options.workspace {
        let checkout = temp.path().join("checkout");
        std::fs::create_dir_all(&checkout).unwrap();
        config.workspaces = BTreeMap::from([(
            "checkout".to_owned(),
            std::fs::canonicalize(&checkout).unwrap(),
        )]);
    }
    if options.chats {
        let tasks = temp.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        config.chats = Some(coder_host::tailnet::Chats {
            observer: temp.path().join("observer"),
            sources: coder_history::Config {
                coder: Some(tasks),
                ..coder_history::Config::default()
            },
        });
        config.serve_chats = true;
    }
    let tasks = options
        .tasks
        .unwrap_or_else(|| Arc::new(NoTasks) as Arc<dyn coder_host::Tasks>);
    let running = coder_host::start(config, tasks).await.unwrap();
    Host {
        temp,
        relay,
        socket,
        root,
        store,
        running,
        events,
        _relay_task: relay_task,
    }
}

/// Where a test host's control channel is: a short socket path, since a
/// Unix socket path is bounded.
#[cfg(unix)]
pub fn control_path(temp: &tempfile::TempDir) -> PathBuf {
    temp.path().join("c/control.sock")
}

/// Where a test host's control channel is: a pipe of its own, so tests
/// running at once, and a real host, never share a name.
#[cfg(windows)]
pub fn control_path(_temp: &tempfile::TempDir) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    PathBuf::from(format!(
        r"\\.\pipe\openagents-control-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// One request on a fresh control connection.
#[cfg(unix)]
pub async fn call(socket: &std::path::Path, op: Op) -> openagents_connect::Result<Reply> {
    let mut stream = UnixStream::connect(socket).await.map_err(|_| {
        openagents_connect::Error::new(openagents_connect::Code::Unavailable, "connect")
    })?;
    control::call(&mut stream, &Request::new(7, op)).await
}

/// One request on a fresh control connection: the pipe, waiting while
/// every instance is busy.
#[cfg(windows)]
pub async fn call(pipe: &std::path::Path, op: Op) -> openagents_connect::Result<Reply> {
    use tokio::net::windows::named_pipe::ClientOptions;
    // ERROR_PIPE_BUSY: the host's next instance is not up yet.
    const BUSY: i32 = 231;
    let unavailable =
        || openagents_connect::Error::new(openagents_connect::Code::Unavailable, "connect");
    let mut tries = 0;
    let mut stream = loop {
        match ClientOptions::new().open(pipe) {
            Ok(stream) => break stream,
            Err(error) if error.raw_os_error() == Some(BUSY) && tries < 50 => {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(_) => return Err(unavailable()),
        }
    };
    control::call(&mut stream, &Request::new(7, op)).await
}

impl Host {
    /// Mint a connect code over the control socket.
    pub async fn code(&self) -> (String, ConnectCode) {
        let Reply::Invite {
            invitation, code, ..
        } = call(&self.socket, Op::InviteCreate {}).await.unwrap()
        else {
            panic!("an invitation")
        };
        let parsed = ConnectCode::parse(&code, now()).unwrap();
        assert_eq!(parsed.invitation(), invitation);
        (invitation, parsed)
    }

    /// The host's iroh address, as a phone on this machine dials it.
    pub fn addr(&self) -> openagents_connect::iroh::EndpointAddr {
        self.running.iroh_addr().unwrap()
    }
}

/// A phone: its device key and a dialing iroh endpoint.
pub struct Phone {
    pub secret: SecretKey,
    pub endpoint: ConnectEndpoint,
}

impl Phone {
    pub async fn new() -> Self {
        let endpoint = ConnectEndpoint::bind(
            openagents_connect::iroh::SecretKey::generate(),
            EndpointConfig::loopback(vec![]),
        )
        .await
        .unwrap();
        Self {
            secret: key(),
            endpoint,
        }
    }

    /// Redeem a scanned code over iroh at the phone's clock `at`. The
    /// phone signs the relay the host serves, which it knows here; the
    /// app uses its default relay, which a desktop host serves.
    pub async fn redeem(
        &self,
        code: &ConnectCode,
        relay: &str,
        at: u64,
    ) -> (Option<Event>, coder_host::access::Result<Access>, u64) {
        let (invitation, pending, answer) = self.answer(code, relay, at).await;
        let reply: Option<Event> = answer
            .reply
            .as_deref()
            .map(|text| serde_json::from_str(text).unwrap());
        let access = match &reply {
            Some(reply) => finish_redeem(&invitation, &pending, reply, &self.secret, at, POLICY),
            None => Err(coder_host::access::Error::new(Code::Transport, "no reply")),
        };
        (reply, access, answer.now)
    }

    /// Send a redemption of `code` over iroh and return the host's whole
    /// answer, beside what finishing it needs.
    pub async fn answer(
        &self,
        code: &ConnectCode,
        relay: &str,
        at: u64,
    ) -> (
        HostInvitation,
        coder_host::access::client::Pending,
        openagents_connect::enroll::EnrollReply,
    ) {
        let invitation = HostInvitation::from_parts(
            &code.host(),
            &code.invitation(),
            &code.capability(),
            relay,
            code.issued_at(),
            code.expires_at(),
            at,
            POLICY,
        )
        .unwrap();
        let pending = prepare_redeem(&invitation, &self.secret, at, POLICY).unwrap();
        let request = EnrollRequest::new(serde_json::to_string(&pending.event).unwrap());
        let answer = enroll::redeem(&self.endpoint, code.endpoint_addr(), &request)
            .await
            .unwrap();
        (invitation, pending, answer)
    }

    /// Open a direct channel over iroh as `device`.
    pub async fn link(&self, host: &Host, device: &Arc<Device>) -> coder_host::Result<Link> {
        let connection = self
            .endpoint
            .endpoint
            .connect(host.addr(), openagents_connect::REACH_ALPN)
            .await
            .unwrap();
        let stream = IrohStream::open(connection).await.unwrap();
        Link::direct(
            device.clone(),
            stream,
            "iroh".into(),
            host.running.generation(),
            Duration::from_secs(5),
        )
        .await
    }

    pub fn device(&self, access: Access) -> Arc<Device> {
        Arc::new(Device::new(access, self.secret, POLICY).unwrap())
    }
}

/// Ask the host for a chat invitation over `link` (`chats.invite`).
pub async fn invite_chats(link: &Link) -> coder_host::Result<(String, u64)> {
    match link
        .call(coder_host::access::protocol::Operation::InviteChats {})
        .await?
    {
        coder_host::access::protocol::Outcome::Chats {
            invitation,
            expires_at,
        } => Ok((invitation, expires_at)),
        other => panic!("not a chat invitation: {other:?}"),
    }
}

/// Redeem a `coder-pair:` invitation as `secret` against the host's chat
/// observer and read its Coder chat catalog, as the phone's Coder tab
/// does: the invitation is usable, not only well formed.
pub async fn read_chats(invitation: &str, secret: &SecretKey) -> coder_connect::ConnectionCode {
    let code = tokio::time::timeout(
        Duration::from_secs(30),
        coder_connect::pairing::redeem(invitation, secret, POLICY),
    )
    .await
    .expect("the observer answers")
    .expect("the chat invitation redeems");
    let client = coder_connect::Client::new_with_policy(code.clone(), *secret, POLICY).unwrap();
    let page = tokio::time::timeout(
        Duration::from_secs(30),
        client.observe(coder_connect::Query::Catalog(
            coder_history::CatalogRequest::default(),
        )),
    )
    .await
    .expect("the catalog answers")
    .expect("the catalog reads");
    assert!(matches!(page, coder_connect::Observation::Catalog(_)));
    code
}
