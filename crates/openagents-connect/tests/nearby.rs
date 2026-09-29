//! Nearby approval over real iroh endpoints on loopback, relays disabled:
//! the enroll ALPN hands a nearby request to the host, both sides compute
//! one code, and only the host's click sends the grant. Enrollment by
//! connect code keeps working on the same ALPN.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::protocol::Router;
use openagents_connect::ENROLL_ALPN;
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig, host_alpns};
use openagents_connect::enroll::{self, EnrollCall, EnrollProtocol, EnrollReply, EnrollRequest};
use openagents_connect::nearby::{
    self, Admission, Code, DeviceOutcome, HostKeys, HostOutcome, NearbyCall, NearbyComputer,
    NearbyRequest, Nonce, Refusal, Ticket, Verdict,
};
use tokio::sync::{mpsc, oneshot};

const NOW: u64 = 1_800_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
const HOST_NOSTR: [u8; 32] = [2; 32];
const DEVICE_NOSTR: [u8; 32] = [4; 32];

fn iroh_key(n: u8) -> iroh::SecretKey {
    iroh::SecretKey::from_bytes(&[n; 32])
}

/// Admits every request and answers with a fixed verdict, recording the
/// code it showed.
#[derive(Clone)]
struct Clicker {
    verdict: Verdict,
    shown: Arc<Mutex<Option<Code>>>,
}

impl Admission for Clicker {
    type Ticket = Clicker;
    fn admit(&self, _: &NearbyRequest) -> Result<Clicker, Refusal> {
        Ok(self.clone())
    }
}

impl Ticket for Clicker {
    async fn decide(self, code: Code) -> Verdict {
        *self.shown.lock().unwrap() = Some(code);
        self.verdict
    }
}

struct Host {
    router: Router,
    endpoint: ConnectEndpoint,
    calls: mpsc::Receiver<EnrollCall>,
    outcomes: mpsc::Receiver<HostOutcome>,
    shown: Arc<Mutex<Option<Code>>>,
}

async fn host(verdict: Verdict) -> Host {
    let endpoint = ConnectEndpoint::bind(iroh_key(1), EndpointConfig::loopback(host_alpns()))
        .await
        .unwrap();
    let (call_tx, calls) = mpsc::channel(4);
    let (nearby_tx, mut nearby_rx) = mpsc::channel::<NearbyCall>(1);
    let router = Router::builder(endpoint.endpoint.clone())
        .accept(
            ENROLL_ALPN,
            EnrollProtocol::new(call_tx).with_nearby(nearby_tx),
        )
        .spawn();
    let shown = Arc::new(Mutex::new(None));
    let clicker = Clicker {
        verdict,
        shown: shown.clone(),
    };
    let keys = HostKeys {
        endpoint: *endpoint.endpoint.id().as_bytes(),
        nostr: HOST_NOSTR,
    };
    let (outcome_tx, outcomes) = mpsc::channel(4);
    tokio::spawn(async move {
        while let Some(call) = nearby_rx.recv().await {
            let NearbyCall {
                remote,
                request,
                stream,
                done,
            } = call;
            let (reader, writer) = tokio::io::split(stream);
            let outcome = nearby::host_session_after(
                request,
                reader,
                writer,
                keys,
                *remote.as_bytes(),
                Nonce::random().unwrap(),
                NOW,
                &clicker,
            )
            .await
            .unwrap();
            drop(done);
            outcome_tx.send(outcome).await.unwrap();
        }
    });
    Host {
        router,
        endpoint,
        calls,
        outcomes,
        shown,
    }
}

async fn device() -> ConnectEndpoint {
    ConnectEndpoint::bind(iroh_key(3), EndpointConfig::loopback(vec![]))
        .await
        .unwrap()
}

fn listed(host: &Host) -> NearbyComputer {
    let addr = host.endpoint.local_addr();
    NearbyComputer {
        endpoint: addr.id,
        label: "Kai's Mac".into(),
        addrs: addr.ip_addrs().copied().collect(),
    }
}

#[tokio::test]
async fn a_click_on_the_computer_sends_the_grant_over_iroh() {
    let grant = serde_json::json!({"kind": 3188, "content": "sealed"});
    let mut host = host(Verdict::Connect {
        event: grant.clone(),
    })
    .await;
    let device = device().await;
    let (seen_tx, seen) = oneshot::channel();
    let outcome = tokio::time::timeout(
        TIMEOUT,
        nearby::pair(
            &device,
            &listed(&host),
            DEVICE_NOSTR,
            "Kai's iPhone",
            NOW,
            |code| {
                seen_tx.send(code).unwrap();
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let on_phone = seen.await.unwrap();
    assert_eq!(
        Some(on_phone),
        *host.shown.lock().unwrap(),
        "one code on both screens"
    );
    assert_eq!(
        outcome,
        DeviceOutcome::Approved {
            host_nostr: HOST_NOSTR,
            host_now: NOW,
            code: on_phone,
            event: grant,
        }
    );
    let HostOutcome::Approved { request, code } = host.outcomes.recv().await.unwrap() else {
        panic!("host did not approve");
    };
    assert_eq!(code, on_phone);
    assert_eq!(request.label, "Kai's iPhone");
    assert_eq!(request.device_nostr, DEVICE_NOSTR);
    assert_eq!(
        request.device_endpoint,
        *device.endpoint.id().as_bytes(),
        "the device endpoint comes from QUIC"
    );
    assert!(host.calls.try_recv().is_err(), "no enroll call");
    host.router.shutdown().await.unwrap();
}

#[tokio::test]
async fn without_the_click_the_phone_gets_no_grant() {
    let mut host = host(Verdict::Decline).await;
    let device = device().await;
    let outcome = tokio::time::timeout(
        TIMEOUT,
        nearby::pair(&device, &listed(&host), DEVICE_NOSTR, "p", NOW, |_| {}),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(outcome, DeviceOutcome::NotConnected);
    assert_eq!(host.outcomes.recv().await.unwrap(), HostOutcome::Declined);
    host.router.shutdown().await.unwrap();
}

#[tokio::test]
async fn connect_code_enrollment_still_works_beside_nearby() {
    let mut host = host(Verdict::Decline).await;
    let device = device().await;
    let addr = host.endpoint.local_addr();
    let request = EnrollRequest::new("signed redeem".into());
    let answer = async {
        let call = host.calls.recv().await.unwrap();
        assert_eq!(call.request.request, "signed redeem");
        call.reply
            .send(EnrollReply::new(NOW, Some("signed reply".into())))
            .unwrap();
    };
    let (reply, ()) = tokio::join!(enroll::redeem(&device, addr, &request), answer);
    assert_eq!(reply.unwrap().reply.as_deref(), Some("signed reply"));
    assert!(host.outcomes.try_recv().is_err(), "no nearby session");
    host.router.shutdown().await.unwrap();
}

/// Real multicast on this machine's interfaces: the host's record lists it
/// with its label and addresses, and a device dials it from the list.
#[tokio::test]
#[ignore = "needs multicast on a local network interface"]
async fn live_mdns_lists_the_computer_and_the_phone_pairs_from_the_list() {
    use futures_util::StreamExt;
    use openagents_connect::endpoint::Relay;

    let config = |alpns| EndpointConfig {
        relay: Relay::Disabled,
        bind: vec![],
        alpns,
    };
    let (nearby_tx, mut nearby_rx) = mpsc::channel::<NearbyCall>(1);
    let (call_tx, _calls) = mpsc::channel(1);
    let host = ConnectEndpoint::bind(iroh_key(5), config(host_alpns()))
        .await
        .unwrap();
    let router = Router::builder(host.endpoint.clone())
        .accept(
            ENROLL_ALPN,
            EnrollProtocol::new(call_tx).with_nearby(nearby_tx),
        )
        .spawn();
    nearby::advertise(&host, "Kai's Mac").unwrap();
    let keys = HostKeys {
        endpoint: *host.endpoint.id().as_bytes(),
        nostr: HOST_NOSTR,
    };
    let clicker = Clicker {
        verdict: Verdict::Decline,
        shown: Arc::new(Mutex::new(None)),
    };
    tokio::spawn(async move {
        while let Some(NearbyCall {
            remote,
            request,
            stream,
            done,
        }) = nearby_rx.recv().await
        {
            let (reader, writer) = tokio::io::split(stream);
            let _ = nearby::host_session_after(
                request,
                reader,
                writer,
                keys,
                *remote.as_bytes(),
                Nonce::random().unwrap(),
                NOW,
                &clicker,
            )
            .await;
            drop(done);
        }
    });

    let phone = ConnectEndpoint::bind(iroh_key(6), config(vec![]))
        .await
        .unwrap();
    let browser = nearby::NearbyBrowser::start(phone.endpoint.id()).unwrap();
    let mut events = browser.events().await;
    let found = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(nearby::NearbyEvent::Found(computer)) = events.next().await
                && computer.endpoint == host.endpoint.id()
            {
                return computer;
            }
        }
    })
    .await
    .expect("the computer was listed");
    assert_eq!(found.label, "Kai's Mac");
    assert!(!found.addrs.is_empty());
    let outcome = tokio::time::timeout(
        TIMEOUT,
        nearby::pair(&phone, &found, DEVICE_NOSTR, "Kai's iPhone", NOW, |_| {}),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(outcome, DeviceOutcome::NotConnected);
    router.shutdown().await.unwrap();
}

#[test]
fn the_control_socket_carries_the_prompt_and_the_click() {
    use openagents_connect::control::{NearbyPrompt, Op, Reply, Request};
    let pending = serde_json::to_value(Request::new(1, Op::NearbyPending {})).unwrap();
    assert_eq!(pending["op"], serde_json::json!({"kind": "nearby_pending"}));
    let decide = serde_json::to_value(Request::new(
        2,
        Op::NearbyDecide {
            id: 7,
            connect: true,
            terminal: false,
        },
    ))
    .unwrap();
    assert_eq!(
        decide["op"],
        serde_json::json!({"kind": "nearby_decide", "id": 7, "connect": true, "terminal": false})
    );
    let reply = Reply::Nearby {
        pending: Some(NearbyPrompt {
            id: 7,
            label: "Kai's iPhone".into(),
            code: "482913".into(),
        }),
    };
    let text = serde_json::to_string(&reply).unwrap();
    assert_eq!(serde_json::from_str::<Reply>(&text).unwrap(), reply);
}
