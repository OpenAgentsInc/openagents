use std::sync::atomic::{AtomicUsize, Ordering};

use openagents_connect::nearby::{DeviceKeys, DeviceOutcome, device_session, host_session};
use tokio::io::{duplex, split};

use super::*;

const NOW: u64 = 1_790_000_000;

fn key(byte: u8) -> nearby::Key {
    [byte; 32]
}

struct Minted {
    gate: NearbyGate,
    calls: Arc<AtomicUsize>,
    rights: Arc<Mutex<Vec<(String, Rights)>>>,
}

fn gate() -> Minted {
    let calls = Arc::new(AtomicUsize::new(0));
    let rights = Arc::new(Mutex::new(Vec::new()));
    let (c, r) = (calls.clone(), rights.clone());
    let gate = NearbyGate::new(Arc::new(move |device: &str, granted: Rights| {
        c.fetch_add(1, Ordering::SeqCst);
        r.lock().unwrap().push((device.to_owned(), granted));
        Ok(serde_json::json!({"sealed": "grant"}))
    }));
    Minted {
        gate,
        calls,
        rights,
    }
}

fn request() -> NearbyRequest {
    NearbyRequest {
        device_endpoint: key(20),
        device_nostr: key(21),
        label: "Kai's iPhone".into(),
    }
}

const HOST: HostKeys = HostKeys {
    endpoint: [10; 32],
    nostr: [11; 32],
};
const DEVICE: DeviceKeys = DeviceKeys {
    endpoint: [20; 32],
    nostr: [21; 32],
};

/// Runs a full exchange; `click` answers the computer's prompt, or leaves
/// it unanswered when `None`.
async fn exchange(
    gate: &NearbyGate,
    click: Option<Choice>,
) -> (HostOutcome, DeviceOutcome, Option<Code>) {
    let (a, b) = duplex(64 * 1024);
    let ((dr, dw), (hr, hw)) = (split(a), split(b));
    let phone_code = Arc::new(Mutex::new(None));
    let seen = phone_code.clone();
    let mut shown = gate.watch();
    let clicker = {
        let gate = gate.clone();
        async move {
            let choice = click?;
            let pending = shown
                .wait_for(Option::is_some)
                .await
                .unwrap()
                .clone()
                .unwrap();
            gate.decide(pending.id, choice).unwrap();
            Some(pending)
        }
    };
    let (host, device, pending) = tokio::join!(
        host_session(hr, hw, HOST, DEVICE.endpoint, Nonce(key(12)), NOW, gate),
        device_session(
            dr,
            dw,
            DEVICE,
            HOST.endpoint,
            "Kai's iPhone",
            Nonce(key(22)),
            NOW,
            move |code| *seen.lock().unwrap() = Some(code),
        ),
        clicker,
    );
    let code = *phone_code.lock().unwrap();
    if let Some(pending) = pending {
        assert_eq!(Some(pending.code), code, "both screens show one code");
        assert_eq!(pending.label, "Kai's iPhone");
    }
    (host.unwrap(), device.unwrap(), code)
}

#[tokio::test(start_paused = true)]
async fn no_grant_without_the_click() {
    let minted = gate();
    let (host, device, code) = exchange(&minted.gate, None).await;
    assert!(code.is_some(), "the phone showed its code");
    assert_eq!(host, HostOutcome::Expired);
    assert_eq!(device, DeviceOutcome::NotConnected);
    assert_eq!(minted.calls.load(Ordering::SeqCst), 0, "nothing was minted");
    assert_eq!(minted.gate.pending(), None, "the prompt is gone");

    let (host, device, _) = exchange(&minted.gate, Some(Choice::Decline)).await;
    assert_eq!(host, HostOutcome::Declined);
    assert_eq!(device, DeviceOutcome::NotConnected);
    assert_eq!(
        minted.calls.load(Ordering::SeqCst),
        0,
        "a decline mints nothing"
    );
}

#[tokio::test(start_paused = true)]
async fn the_click_signs_one_grant_with_the_connect_code_rights() {
    let minted = gate();
    let (host, device, code) = exchange(&minted.gate, Some(Choice::Connect)).await;
    assert!(matches!(host, HostOutcome::Approved { .. }));
    assert_eq!(
        device,
        DeviceOutcome::Approved {
            host_nostr: HOST.nostr,
            host_now: NOW,
            code: code.unwrap(),
            event: serde_json::json!({"sealed": "grant"}),
        }
    );
    let (_, _, _) = exchange(&minted.gate, Some(Choice::Connect)).await;
    assert_eq!(minted.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        *minted.rights.lock().unwrap(),
        vec![
            (nearby::hex(&DEVICE.nostr), nearby_rights()),
            (nearby::hex(&DEVICE.nostr), nearby_rights())
        ],
        "the grant is for the device key the exchange named"
    );
    assert_eq!(
        nearby_rights().to_list(),
        "observe,operate,terminal,review,access_read,access_admin"
    );
    assert_eq!(minted.gate.pending(), None);
}

#[tokio::test(start_paused = true)]
async fn a_click_needs_the_shown_request() {
    let minted = gate();
    let ticket = minted.gate.admit(&request()).unwrap();
    // Admitted but its code is not on screen yet: nothing to click.
    assert_eq!(minted.gate.pending(), None);
    assert_eq!(
        minted.gate.decide(1, Choice::Connect),
        Err(DecideError::NotPending)
    );
    let decided = tokio::spawn(ticket.decide(Code::new(123_456).unwrap()));
    let pending = minted
        .gate
        .watch()
        .wait_for(Option::is_some)
        .await
        .unwrap()
        .clone()
        .unwrap();
    assert_eq!(pending.code.digits(), "123456");
    assert_eq!(
        minted.gate.decide(pending.id + 1, Choice::Connect),
        Err(DecideError::NotPending),
        "another request's ID does not approve this one"
    );
    minted.gate.decide(pending.id, Choice::Decline).unwrap();
    assert_eq!(
        minted.gate.decide(pending.id, Choice::Connect),
        Err(DecideError::NotPending),
        "one answer per request"
    );
    assert_eq!(decided.await.unwrap(), Verdict::Decline);
    assert_eq!(minted.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn one_pending_request_and_five_per_ten_minutes() {
    let minted = gate();
    let gate = &minted.gate;
    let start = Instant::now();

    let first = gate.admit_at(&request(), start).unwrap();
    assert!(matches!(
        gate.admit_at(&request(), start),
        Err(Refusal::Busy)
    ));
    drop(first);

    for _ in 1..MAX_PER_WINDOW {
        drop(gate.admit_at(&request(), start).unwrap());
    }
    let later = start + WINDOW - Duration::from_secs(1);
    assert!(
        matches!(gate.admit_at(&request(), later), Err(Refusal::Limited)),
        "a sixth request inside ten minutes is refused"
    );
    // Refusals do not count, so the window frees up on time, and the
    // limit applies again to the next ten minutes.
    for _ in 0..MAX_PER_WINDOW {
        drop(gate.admit_at(&request(), start + WINDOW).unwrap());
    }
    assert!(matches!(
        gate.admit_at(&request(), start + WINDOW),
        Err(Refusal::Limited)
    ));
}

#[tokio::test(start_paused = true)]
async fn the_rate_limit_holds_over_the_wire() {
    let minted = gate();
    for _ in 0..MAX_PER_WINDOW {
        let (host, _, _) = exchange(&minted.gate, Some(Choice::Decline)).await;
        assert_eq!(host, HostOutcome::Declined);
    }
    let (host, device, code) = exchange(&minted.gate, None).await;
    assert_eq!(host, HostOutcome::Refused(Refusal::Limited));
    assert_eq!(device, DeviceOutcome::Refused);
    assert_eq!(code, None, "a refused phone shows no code");
    tokio::time::advance(WINDOW).await;
    let (host, _, _) = exchange(&minted.gate, Some(Choice::Decline)).await;
    assert_eq!(host, HostOutcome::Declined);
}
