//! Nearby approval against a running host: a phone dials the host's iroh
//! endpoint as a nearby computer, both sides show one code, and the host
//! grants only when the local operator clicks Connect over the control
//! socket. The grant then opens a direct channel like any other.

use std::time::Duration;

use coder_host::access::protocol::{Access, OriginKind};
use coder_host::reach::pubkey;
use nostr::domain::Event;
use openagents_connect::control::{NearbyPrompt, Op, Reply};
use openagents_connect::nearby::{self, DeviceOutcome, NearbyComputer};
use tokio::sync::oneshot;

#[path = "support/connect.rs"]
mod support;

use support::{Host, POLICY, Phone, call, host, now};

fn listed(host: &Host) -> NearbyComputer {
    let addr = host.addr();
    NearbyComputer {
        endpoint: addr.id,
        label: "Studio Mac".into(),
        addrs: addr.ip_addrs().copied().collect(),
    }
}

/// Waits for the prompt the desktop app would show.
async fn prompt(host: &Host) -> NearbyPrompt {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Reply::Nearby {
                pending: Some(prompt),
            } = call(&host.socket, Op::NearbyPending {}).await.unwrap()
            {
                return prompt;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("a prompt")
}

async fn pair(host: &Host, phone: &Phone, shown: oneshot::Sender<nearby::Code>) -> DeviceOutcome {
    let device = nearby::parse_key(&pubkey(&phone.secret)).unwrap();
    nearby::pair(
        &phone.endpoint,
        &listed(host),
        device,
        "Kai's iPhone",
        now(),
        move |code| {
            let _ = shown.send(code);
        },
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_click_on_the_computer_grants_the_nearby_phone() {
    let host = host().await;
    let phone = Phone::new().await;
    let (shown, on_phone) = oneshot::channel();
    let clicker = async {
        let prompt = prompt(&host).await;
        assert_eq!(prompt.label, "Kai's iPhone");
        // A click for another request answers nothing.
        let wrong = call(
            &host.socket,
            Op::NearbyDecide {
                id: prompt.id + 1,
                connect: true,
                terminal: true,
            },
        )
        .await
        .unwrap();
        assert!(matches!(wrong, Reply::Refused { ref code, .. } if code == "not_pending"));
        assert!(host.store.devices(now()).unwrap().is_empty());
        let reply = call(
            &host.socket,
            Op::NearbyDecide {
                id: prompt.id,
                connect: true,
                terminal: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(reply, Reply::Nearby { pending: None });
        prompt
    };
    let (outcome, prompt) = tokio::join!(pair(&host, &phone, shown), clicker);
    let on_phone = on_phone.await.unwrap();
    assert_eq!(prompt.code, on_phone.digits(), "both screens show one code");
    let DeviceOutcome::Approved {
        host_nostr,
        host_now,
        event,
        ..
    } = outcome
    else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(nearby::hex(&host_nostr), host.running.host_key());
    let event: Event = serde_json::from_value(event).unwrap();
    let access = Access::from_authorization(
        event,
        &phone.secret,
        host.running.host_key(),
        host_now,
        POLICY,
    )
    .unwrap();
    assert_eq!(access.grant.origin.kind, OriginKind::Approval);
    assert_eq!(access.grant.rights.to_list(), "observe,operate");
    assert_eq!(access.grant.relay, host.relay);
    // The grant opens a direct channel over iroh like a scanned code's.
    let device = phone.device(access);
    phone.link(&host, &device).await.unwrap();
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dont_connect_grants_nothing() {
    let host = host().await;
    let phone = Phone::new().await;
    let (shown, _) = oneshot::channel();
    let clicker = async {
        let prompt = prompt(&host).await;
        call(
            &host.socket,
            Op::NearbyDecide {
                id: prompt.id,
                connect: false,
                terminal: true,
            },
        )
        .await
        .unwrap()
    };
    let (outcome, _) = tokio::join!(pair(&host, &phone, shown), clicker);
    assert_eq!(outcome, DeviceOutcome::NotConnected);
    assert!(host.store.devices(now()).unwrap().is_empty());
    assert_eq!(
        call(&host.socket, Op::NearbyPending {}).await.unwrap(),
        Reply::Nearby { pending: None }
    );
    host.running.shutdown().await;
}
