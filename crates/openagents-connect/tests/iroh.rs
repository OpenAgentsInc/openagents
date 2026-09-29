//! Two iroh endpoints on loopback with relays disabled: the NIP-REACH
//! channel on `openagents/reach/1`, enrollment on `openagents/enroll/1`, and
//! refusals.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_reach::Refusal;
use coder_reach::channel::{Acceptor, ClientConfig, GrantCheck, GrantRefusal, MAX_DATA_BYTES};
use iroh::protocol::Router;
use openagents_connect::code::{CodeParts, ConnectCode};
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig, host_alpns};
use openagents_connect::enroll::{self, EnrollCall, EnrollProtocol, EnrollReply, EnrollRequest};
use openagents_connect::ledger::{Ledger, Redeem};
use openagents_connect::reach::{self, ReachProtocol, ReachSession};
use openagents_connect::{Code, ENROLL_ALPN, REACH_ALPN};
use secp256k1::SecretKey;
use tokio::sync::mpsc;

const NOW: u64 = 1_800_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
const GENERATION: u64 = 7;

fn nostr_key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}

fn iroh_key(n: u8) -> iroh::SecretKey {
    iroh::SecretKey::from_bytes(&[n; 32])
}

type GrantTable = HashMap<String, (String, u64, bool)>;

/// A stand-in for the host's grant store: device -> (grant, epoch, revoked).
#[derive(Clone, Default)]
struct Grants(Arc<Mutex<GrantTable>>);

impl Grants {
    fn admit(&self, device: &SecretKey, grant: &str, epoch: u64) {
        self.0.lock().unwrap().insert(
            coder_reach::pubkey(device),
            (grant.to_owned(), epoch, false),
        );
    }
    fn revoke(&self, device: &SecretKey) {
        if let Some(entry) = self.0.lock().unwrap().get_mut(&coder_reach::pubkey(device)) {
            entry.2 = true;
        }
    }
}

impl GrantCheck for Grants {
    fn check(&self, device: &str, grant: &str, epoch: u64, _now: u64) -> Result<(), GrantRefusal> {
        match self.0.lock().unwrap().get(device) {
            Some((id, _, _)) if id != grant => Err(GrantRefusal::Unknown),
            Some((_, _, true)) => Err(GrantRefusal::Revoked),
            Some((_, current, false)) if *current != epoch => Err(GrantRefusal::EpochMismatch),
            Some(_) => Ok(()),
            None => Err(GrantRefusal::Unknown),
        }
    }
}

struct Host {
    router: Router,
    endpoint: ConnectEndpoint,
    sessions: mpsc::Receiver<ReachSession>,
    calls: mpsc::Receiver<EnrollCall>,
    grants: Grants,
}

async fn host() -> Host {
    let endpoint = ConnectEndpoint::bind(iroh_key(1), EndpointConfig::loopback(host_alpns()))
        .await
        .unwrap();
    let grants = Grants::default();
    let acceptor = Arc::new(Acceptor::new(
        nostr_key(2),
        GENERATION,
        grants.clone(),
        TIMEOUT,
    ));
    let (session_tx, sessions) = mpsc::channel(4);
    let (call_tx, calls) = mpsc::channel(4);
    let router = Router::builder(endpoint.endpoint.clone())
        .accept(
            REACH_ALPN,
            ReachProtocol::with_clock(acceptor, session_tx, Arc::new(|| NOW)),
        )
        .accept(ENROLL_ALPN, EnrollProtocol::new(call_tx))
        .spawn();
    Host {
        router,
        endpoint,
        sessions,
        calls,
        grants,
    }
}

async fn device() -> ConnectEndpoint {
    ConnectEndpoint::bind(iroh_key(8), EndpointConfig::loopback(vec![]))
        .await
        .unwrap()
}

fn client(host: &SecretKey, grant: &str) -> ClientConfig {
    ClientConfig {
        device: nostr_key(9),
        host: coder_reach::pubkey(host),
        grant: grant.to_owned(),
        epoch: 0,
        generation: GENERATION,
        timeout: TIMEOUT,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn reach_channel_over_iroh_exchanges_frames_both_ways() {
    let mut host = host().await;
    let grant = coder_reach::new_id();
    host.grants.admit(&nostr_key(9), &grant, 0);
    let device = device().await;
    let addr = host.endpoint.local_addr();
    assert!(addr.relay_urls().next().is_none(), "relays are disabled");

    let mut channel = reach::dial(&device, addr, &client(&nostr_key(2), &grant), NOW)
        .await
        .unwrap();
    let ReachSession {
        remote,
        channel: mut served,
    } = tokio::time::timeout(TIMEOUT, host.sessions.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(remote, device.endpoint.id());
    assert_eq!(served.binding(), channel.binding());
    assert_eq!(served.binding().client, coder_reach::pubkey(&nostr_key(9)));
    assert_eq!(served.binding().host, coder_reach::pubkey(&nostr_key(2)));
    assert_eq!(served.binding().generation, GENERATION);

    channel.send(b"hello host").await.unwrap();
    assert_eq!(served.recv().await.unwrap().unwrap(), b"hello host");
    served.send(b"hello device").await.unwrap();
    assert_eq!(channel.recv().await.unwrap().unwrap(), b"hello device");

    // The largest data frame, and many frames in order.
    let big = vec![0x5a; MAX_DATA_BYTES];
    channel.send(&big).await.unwrap();
    assert_eq!(served.recv().await.unwrap().unwrap(), big);
    for i in 0..50u8 {
        served.send(&[i; 100]).await.unwrap();
    }
    for i in 0..50u8 {
        assert_eq!(channel.recv().await.unwrap().unwrap(), vec![i; 100]);
    }

    // A split channel reads and writes from separate tasks.
    let (mut reader, mut writer) = served.into_split();
    let echo = tokio::spawn(async move {
        while let Some(data) = reader.recv().await.unwrap() {
            writer.send(&data).await.unwrap();
        }
    });
    channel.send(b"echo").await.unwrap();
    assert_eq!(channel.recv().await.unwrap().unwrap(), b"echo");

    // Close is delivered; the host's reader sees the end.
    channel.close().await.unwrap();
    tokio::time::timeout(TIMEOUT, echo).await.unwrap().unwrap();
    host.router.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_host_key_is_refused_as_identity_mismatch() {
    let mut host = host().await;
    let grant = coder_reach::new_id();
    host.grants.admit(&nostr_key(9), &grant, 0);
    let device = device().await;
    // The device expects another host's Nostr key at this endpoint ID.
    let error = reach::dial(
        &device,
        host.endpoint.local_addr(),
        &client(&nostr_key(3), &grant),
        NOW,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Refusal::IdentityMismatch);
    assert_eq!(error.detail, coder_reach::channel::UNAUTHENTICATED);
    assert!(host.sessions.try_recv().is_err());
    host.router.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_a_grant_is_refused_after_the_host_proves_itself() {
    let mut host = host().await;
    let device = device().await;
    let addr = host.endpoint.local_addr();
    // An iroh key the host has never seen, holding no grant.
    let error = reach::dial(
        &device,
        addr.clone(),
        &client(&nostr_key(2), &coder_reach::new_id()),
        NOW,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Refusal::NotAdmitted);
    assert_ne!(error.detail, coder_reach::channel::UNAUTHENTICATED);

    // Revocation: the same device refused as `revoked`.
    let grant = coder_reach::new_id();
    host.grants.admit(&nostr_key(9), &grant, 0);
    host.grants.revoke(&nostr_key(9));
    let error = reach::dial(&device, addr.clone(), &client(&nostr_key(2), &grant), NOW)
        .await
        .unwrap_err();
    assert_eq!(error.code, Refusal::Revoked);

    // A stale epoch.
    host.grants.admit(&nostr_key(9), &grant, 3);
    let error = reach::dial(&device, addr, &client(&nostr_key(2), &grant), NOW)
        .await
        .unwrap_err();
    assert_eq!(error.code, Refusal::Stale);
    assert!(host.sessions.try_recv().is_err());
    host.router.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hello_outside_the_clock_window_is_refused() {
    let host = host().await;
    let grant = coder_reach::new_id();
    host.grants.admit(&nostr_key(9), &grant, 0);
    let device = device().await;
    let error = reach::dial(
        &device,
        host.endpoint.local_addr(),
        &client(&nostr_key(2), &grant),
        NOW - 600,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Refusal::Stale);
    host.router.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_alpns_are_refused_and_devices_answer_none() {
    let host = host().await;
    let device = device().await;
    let addr = host.endpoint.local_addr();
    assert!(
        device
            .endpoint
            .connect(addr.clone(), b"openagents/unknown/1")
            .await
            .is_err()
    );
    assert!(
        device
            .endpoint
            .connect(addr, b"openagents/reach/2")
            .await
            .is_err()
    );
    // A device endpoint answers no ALPN, so nothing can dial it: the
    // attempt fails or never completes.
    let other = ConnectEndpoint::bind(iroh_key(5), EndpointConfig::loopback(vec![]))
        .await
        .unwrap();
    let attempt = tokio::time::timeout(
        Duration::from_secs(2),
        other.endpoint.connect(device.local_addr(), REACH_ALPN),
    )
    .await;
    assert!(!matches!(attempt, Ok(Ok(_))));
    host.router.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remembered_address_lets_a_device_dial_by_id_alone() {
    let mut host = host().await;
    let grant = coder_reach::new_id();
    host.grants.admit(&nostr_key(9), &grant, 0);
    let device = device().await;
    device.remember(host.endpoint.local_addr());
    let mut channel = reach::dial(
        &device,
        host.endpoint.endpoint.id(),
        &client(&nostr_key(2), &grant),
        NOW,
    )
    .await
    .unwrap();
    let mut served = host.sessions.recv().await.unwrap().channel;
    channel.send(b"by id").await.unwrap();
    assert_eq!(served.recv().await.unwrap().unwrap(), b"by id");
    host.router.shutdown().await.unwrap();
}

/// Enrollment from a scanned code: the device parses the code, dials the
/// endpoint it names, and the host answers once per invitation.
#[tokio::test(flavor = "multi_thread")]
async fn enrollment_from_a_code_is_single_use() {
    let mut host = host().await;
    let local = host.endpoint.local_addr();
    let code = ConnectCode::issue(CodeParts {
        host: coder_reach::pubkey(&nostr_key(2)),
        endpoint: host.endpoint.endpoint.id(),
        issued_at: NOW,
        relay: None,
        addrs: local.ip_addrs().copied().collect(),
        label: "Test Mac".into(),
    })
    .unwrap();
    let mut ledger = Ledger::new();
    ledger.issue(&code, NOW).unwrap();

    // The host: a fake that redeems through the ledger. The request here
    // is `invitation capability device`, standing in for the signed
    // `enroll.redeem` artifact the real host verifies.
    let server = tokio::spawn(async move {
        while let Some(call) = host.calls.recv().await {
            let mut parts = call.request.request.split(' ');
            let (id, cap, device) = (
                parts.next().unwrap().to_owned(),
                parts.next().unwrap().to_owned(),
                parts.next().unwrap().to_owned(),
            );
            let outcome = ledger.redeem(&id, &cap, &device, NOW, NOW + 5);
            let reply = match outcome {
                Redeem::Admitted | Redeem::Retry => Some(format!("grant for {device}")),
                Redeem::Refused(code) => Some(format!("refused {}", code.as_str())),
                Redeem::Silent => None,
            };
            let _ = call.reply.send(EnrollReply::new(NOW + 5, reply));
        }
    });

    let scanned = ConnectCode::parse(&code.encode(), NOW + 1).unwrap();
    let phone = device().await;
    let request = |device: &str| {
        EnrollRequest::new(format!(
            "{} {} {device}",
            scanned.invitation(),
            scanned.capability()
        ))
    };
    let first = enroll::redeem(&phone, scanned.endpoint_addr(), &request("phone"))
        .await
        .unwrap();
    assert_eq!(first.reply.as_deref(), Some("grant for phone"));
    assert_eq!(first.now, NOW + 5);
    assert_eq!(openagents_connect::clock_warning(first.now, NOW + 1), None);

    // A retry from the same device gets the same grant.
    let again = enroll::redeem(&phone, scanned.endpoint_addr(), &request("phone"))
        .await
        .unwrap();
    assert_eq!(again.reply.as_deref(), Some("grant for phone"));

    // Anyone else who photographed the code is refused.
    let thief = ConnectEndpoint::bind(iroh_key(6), EndpointConfig::loopback(vec![]))
        .await
        .unwrap();
    let stolen = enroll::redeem(&thief, scanned.endpoint_addr(), &request("thief"))
        .await
        .unwrap();
    assert_eq!(stolen.reply.as_deref(), Some("refused forbidden"));

    // A wrong capability earns no signed reply.
    let guess = EnrollRequest::new(format!(
        "{} {} thief",
        scanned.invitation(),
        "ab".repeat(32)
    ));
    let silent = enroll::redeem(&thief, scanned.endpoint_addr(), &guess)
        .await
        .unwrap();
    assert_eq!(silent.reply, None);

    server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_enroll_request_of_another_version_gets_no_call() {
    let mut host = host().await;
    let device = device().await;
    let mut request = EnrollRequest::new("x".into());
    request.v = "openagents.connect-enroll-request.v2".into();
    let error = enroll::redeem(&device, host.endpoint.local_addr(), &request)
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::Unavailable);
    assert!(host.calls.try_recv().is_err());
    host.router.shutdown().await.unwrap();
}
