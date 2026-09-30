//! Connect a computer from a scanned code: the live service pairs with a
//! fake computer on loopback iroh (relays disabled), keeps the grant and the
//! computer's iroh route, and refuses an answer from another host key and an
//! expired code with the sentence the scanner shows.
#![cfg(feature = "live")]

use std::sync::Arc;

use base64::Engine as _;
use coder_access::host::{Host, Unconnected};
use coder_access::protocol::{HostInvitation, INVITATION_PREFIX};
use coder_access::{RelayPolicy, Rights};
use coder_computers::ComputersService;
use coder_computers::connect::PairedOver;
use coder_computers::live::{FileStore, Live, Saved, Settings, Store};
use coder_computers::model::Platform;
use coder_host::client::iroh::DEFAULT_RELAY;
use nostr::domain::Event;
use openagents_connect::ENROLL_ALPN;
use openagents_connect::code::{CodeParts, ConnectCode};
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig};
use openagents_connect::enroll::{EnrollProtocol, EnrollReply};
use openagents_connect::iroh::SecretKey as IrohSecret;
use openagents_connect::iroh::protocol::Router;
use secp256k1::SecretKey;
use tokio::sync::mpsc;

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn rights() -> Rights {
    Rights::pairing()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Honest,
    OtherKey,
}

struct Computer {
    _dirs: Vec<tempfile::TempDir>,
    host: Arc<Host>,
    endpoint: ConnectEndpoint,
    _router: Router,
}

async fn computer(answer: Answer, phone: SecretKey) -> Computer {
    let owner = coder_reach::pubkey(&key());
    let (dir, other_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let host = Arc::new(Host::new(
        dir.path().join("access"),
        RelayPolicy::Production,
    ));
    host.init(&owner).unwrap();
    let other = Host::new(other_dir.path().join("access"), RelayPolicy::Production);
    other.init(&owner).unwrap();
    let endpoint = ConnectEndpoint::bind(
        IrohSecret::generate(),
        EndpointConfig::loopback(vec![ENROLL_ALPN.to_vec()]),
    )
    .await
    .unwrap();
    let (calls, mut queue) = mpsc::channel(4);
    let router = Router::builder(endpoint.endpoint.clone())
        .accept(ENROLL_ALPN, EnrollProtocol::new(calls))
        .spawn();
    let answering = host.clone();
    tokio::spawn(async move {
        while let Some(call) = queue.recv().await {
            let reply = match answer {
                Answer::Honest => serde_json::from_str::<Event>(&call.request.request)
                    .ok()
                    .and_then(|event| {
                        answering
                            .handle(&event, DEFAULT_RELAY, now(), &mut Unconnected)
                            .ok()
                    }),
                Answer::OtherKey => {
                    let issued = other
                        .invite(DEFAULT_RELAY, rights(), now(), now() + 86_400)
                        .unwrap();
                    let invitation =
                        HostInvitation::parse(&issued.code, now(), RelayPolicy::Production)
                            .unwrap();
                    let pending = coder_access::client::prepare_redeem(
                        &invitation,
                        &phone,
                        now(),
                        RelayPolicy::Production,
                    )
                    .unwrap();
                    other
                        .handle(&pending.event, DEFAULT_RELAY, now(), &mut Unconnected)
                        .ok()
                }
            };
            let reply = reply.map(|event| serde_json::to_string(&event).unwrap());
            let _ = call.reply.send(EnrollReply::new(now(), reply));
        }
    });
    Computer {
        _dirs: vec![dir, other_dir],
        host,
        endpoint,
        _router: router,
    }
}

impl Computer {
    fn code(&self, issued_at: u64) -> String {
        let issued = self
            .host
            .invite(DEFAULT_RELAY, rights(), issued_at, issued_at + 86_400)
            .unwrap();
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(issued.code.strip_prefix(INVITATION_PREFIX).unwrap())
            .unwrap();
        ConnectCode::from_invitation(
            CodeParts {
                host: self.host.public_key().unwrap(),
                endpoint: self.endpoint.endpoint.id(),
                issued_at,
                relay: None,
                addrs: self.endpoint.local_addr().ip_addrs().copied().collect(),
                label: "Studio Mac".into(),
            },
            &issued.id,
            &hex(&bytes[65..97]),
        )
        .unwrap()
        .encode()
    }
}

struct Phone {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
    live: Live,
}

fn phone(secret: SecretKey, runtime: &tokio::runtime::Runtime) -> Phone {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("computers");
    let mut settings = Settings::new(Platform::Phone);
    settings.iroh_secret = Some(key().secret_bytes());
    settings.iroh_loopback = true;
    let live = Live::open(
        settings,
        secret,
        Box::new(FileStore::open(&path).unwrap()),
        runtime.handle().clone(),
    )
    .unwrap();
    Phone {
        _dir: dir,
        path,
        live,
    }
}

fn saved(path: &std::path::Path) -> Saved {
    FileStore::open(path).unwrap().load().unwrap().unwrap()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_scanned_code_connects_the_computer_over_iroh_and_keeps_its_route() {
    let runtime = runtime();
    let secret = key();
    let computer = runtime.block_on(computer(Answer::Honest, secret));
    let mut phone = phone(secret, &runtime);
    let code = computer.code(now());
    let paired = runtime
        .block_on(phone.live.pairing().pair(&code))
        .unwrap_or_else(|failure| panic!("{failure:?}"));
    let host = computer.host.public_key().unwrap();
    assert_eq!(paired.host, host);
    assert_eq!(paired.label, "Studio Mac");
    assert_eq!(paired.over, PairedOver::Iroh);
    // The list shows it, and the saved record keeps its iroh route beside
    // the grant.
    let snapshot = phone.live.snapshot().unwrap();
    assert!(snapshot.hosts.iter().any(|record| record.key == host));
    let saved = saved(&phone.path);
    let record = saved
        .hosts
        .iter()
        .find(|saved| saved.access.grant.host == host)
        .unwrap();
    assert_eq!(record.label, "Studio Mac");
    let route = record.iroh.as_ref().expect("an iroh route");
    assert_eq!(route.id().unwrap(), computer.endpoint.endpoint.id());
}

#[test]
fn an_answer_from_another_host_key_adds_nothing_and_says_why() {
    let runtime = runtime();
    let secret = key();
    let computer = runtime.block_on(computer(Answer::OtherKey, secret));
    let mut phone = phone(secret, &runtime);
    let failure = runtime
        .block_on(phone.live.pairing().pair(&computer.code(now())))
        .unwrap_err();
    assert!(
        failure.message.contains("didn't come from the computer"),
        "{failure:?}"
    );
    assert!(phone.live.snapshot().unwrap().hosts.is_empty());
}

#[test]
fn an_expired_code_is_refused_with_a_clear_sentence() {
    let runtime = runtime();
    let secret = key();
    let computer = runtime.block_on(computer(Answer::Honest, secret));
    let mut phone = phone(secret, &runtime);
    let failure = runtime
        .block_on(phone.live.pairing().pair(&computer.code(now() - 400)))
        .unwrap_err();
    assert_eq!(
        failure.message,
        "This code has expired. Show a new code on your computer and scan again."
    );
    assert!(phone.live.snapshot().unwrap().hosts.is_empty());
}

#[test]
fn a_code_that_is_not_a_computers_is_refused_before_dialing() {
    let runtime = runtime();
    let phone = phone(key(), &runtime);
    let failure = runtime
        .block_on(phone.live.pairing().pair("coder-pair:AAAA"))
        .unwrap_err();
    assert!(failure.message.contains("Chats pairing code"));
}
