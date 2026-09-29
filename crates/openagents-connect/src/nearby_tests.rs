use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use iroh::SecretKey;
use iroh::address_lookup::{EndpointData, EndpointInfo};
use serde_json::json;
use tokio::io::{DuplexStream, ReadHalf, WriteHalf, duplex, split};

use super::*;

const NOW: u64 = 1_790_000_000;

fn key(byte: u8) -> Key {
    [byte; 32]
}

fn transcript() -> Transcript {
    Transcript {
        host_endpoint: key(1),
        device_endpoint: key(2),
        host_nostr: key(3),
        device_nostr: key(4),
        host_nonce: Nonce(key(5)),
        device_nonce: Nonce(key(6)),
    }
}

type Half = (ReadHalf<DuplexStream>, WriteHalf<DuplexStream>);

fn pipe() -> (Half, Half) {
    let (a, b) = duplex(64 * 1024);
    (split(a), split(b))
}

const HOST: HostKeys = HostKeys {
    endpoint: [10; 32],
    nostr: [11; 32],
};
const DEVICE: DeviceKeys = DeviceKeys {
    endpoint: [20; 32],
    nostr: [21; 32],
};

/// Admits everything; the ticket records the code it was shown and
/// answers with a fixed verdict.
struct Clicker {
    verdict: Verdict,
    shown: Arc<Mutex<Option<Code>>>,
}

struct ClickTicket {
    verdict: Verdict,
    shown: Arc<Mutex<Option<Code>>>,
}

impl Admission for Clicker {
    type Ticket = ClickTicket;
    fn admit(&self, _: &NearbyRequest) -> Result<ClickTicket, Refusal> {
        Ok(ClickTicket {
            verdict: self.verdict.clone(),
            shown: self.shown.clone(),
        })
    }
}

impl Ticket for ClickTicket {
    async fn decide(self, code: Code) -> Verdict {
        *self.shown.lock().unwrap() = Some(code);
        self.verdict
    }
}

fn clicker(verdict: Verdict) -> (Clicker, Arc<Mutex<Option<Code>>>) {
    let shown = Arc::new(Mutex::new(None));
    (
        Clicker {
            verdict,
            shown: shown.clone(),
        },
        shown,
    )
}

fn grant() -> serde_json::Value {
    json!({"kind": 3188, "content": "sealed grant"})
}

fn request_message(label: &str, nonce: &Nonce) -> NearbyRequestMessage {
    NearbyRequestMessage {
        v: REQUEST.into(),
        requires: vec![],
        device: hex(&DEVICE.nostr),
        label: label.into(),
        commitment: hex(&commitment(nonce)),
    }
}

fn reveal_message(nonce: &Nonce) -> NearbyReveal {
    NearbyReveal {
        v: REVEAL.into(),
        requires: vec![],
        nonce: hex(&nonce.0),
    }
}

#[test]
fn code_is_six_digits_in_two_groups() {
    assert_eq!(Code(482_913).to_string(), "482 913");
    assert_eq!(Code(7).to_string(), "000 007");
    assert_eq!(Code(7).digits(), "000007");
    assert_eq!(Code::new(1_000_000), None);
    assert!(sas_code(&transcript()).value() < 1_000_000);
    assert_eq!(sas_code(&transcript()), sas_code(&transcript()));
}

#[test]
fn every_key_and_nonce_changes_the_code() {
    let base = sas_code(&transcript());
    for field in 0..6 {
        let mut t = transcript();
        let slot = match field {
            0 => &mut t.host_endpoint,
            1 => &mut t.device_endpoint,
            2 => &mut t.host_nostr,
            3 => &mut t.device_nostr,
            4 => &mut t.host_nonce.0,
            _ => &mut t.device_nonce.0,
        };
        slot[31] ^= 1;
        assert_ne!(sas_code(&t), base, "field {field}");
    }
    // Swapping the roles is a different transcript too.
    let t = transcript();
    let swapped = Transcript {
        host_endpoint: t.device_endpoint,
        device_endpoint: t.host_endpoint,
        host_nostr: t.device_nostr,
        device_nostr: t.host_nostr,
        host_nonce: t.device_nonce,
        device_nonce: t.host_nonce,
    };
    assert_ne!(sas_code(&swapped), base);
}

#[test]
fn the_code_is_pinned() {
    // A fixture, so a change to the derivation is deliberate: both apps
    // and every host must agree on it (NIP-HOST, nearby approval).
    let mut hash = Sha256::new();
    hash.update(b"openagents.connect-sas.v1\0");
    for byte in 1..=6u8 {
        hash.update([byte; 32]);
    }
    let digest = hash.finalize();
    let head = u64::from_be_bytes(digest[..8].try_into().unwrap());
    assert_eq!(u64::from(sas_code(&transcript()).value()), head % 1_000_000);
    assert_eq!(
        sas_code(&transcript()).digits(),
        format!("{:06}", head % 1_000_000)
    );
}

#[tokio::test]
async fn both_screens_show_the_same_code_and_the_click_delivers_the_grant() {
    let ((dr, dw), (hr, hw)) = pipe();
    let (admission, host_shown) = clicker(Verdict::Connect { event: grant() });
    let device_shown = Arc::new(Mutex::new(None));
    let seen = device_shown.clone();
    let (host_out, device_out) = tokio::join!(
        host_session(
            hr,
            hw,
            HOST,
            DEVICE.endpoint,
            Nonce(key(12)),
            NOW,
            &admission
        ),
        device_session(
            dr,
            dw,
            DEVICE,
            HOST.endpoint,
            "Kai's iPhone\n",
            Nonce(key(22)),
            NOW - 30,
            move |code| *seen.lock().unwrap() = Some(code),
        ),
    );
    let host_code = host_shown.lock().unwrap().unwrap();
    assert_eq!(Some(host_code), *device_shown.lock().unwrap());
    let HostOutcome::Approved { request, code } = host_out.unwrap() else {
        panic!("host did not approve");
    };
    assert_eq!(code, host_code);
    assert_eq!(request.label, "Kai's iPhone");
    assert_eq!(request.device_nostr, DEVICE.nostr);
    assert_eq!(request.device_endpoint, DEVICE.endpoint);
    assert_eq!(
        device_out.unwrap(),
        DeviceOutcome::Approved {
            host_nostr: HOST.nostr,
            host_now: NOW,
            code,
            event: grant(),
        }
    );
}

#[tokio::test]
async fn a_machine_in_the_middle_with_its_own_keys_gets_different_codes() {
    // The phone dials the impostor's advertised endpoint; the impostor
    // dials the real computer as a device. Each leg is an honest exchange
    // with the impostor's keys, so each screen shows its own leg's code.
    let ((dr, dw), (mr1, mw1)) = pipe();
    let ((mr2, mw2), (hr, hw)) = pipe();
    let impostor_as_host = HostKeys {
        endpoint: key(30),
        nostr: key(31),
    };
    let impostor_as_device = DeviceKeys {
        endpoint: key(40),
        nostr: key(41),
    };
    let (host_admission, host_shown) = clicker(Verdict::Decline);
    let (impostor_admission, _) = clicker(Verdict::Decline);
    let phone_shown = Arc::new(Mutex::new(None));
    let seen = phone_shown.clone();
    let (phone, _, _, host) = tokio::join!(
        device_session(
            dr,
            dw,
            DEVICE,
            impostor_as_host.endpoint,
            "Kai's iPhone",
            Nonce(key(22)),
            NOW,
            move |code| *seen.lock().unwrap() = Some(code),
        ),
        host_session(
            mr1,
            mw1,
            impostor_as_host,
            DEVICE.endpoint,
            Nonce(key(32)),
            NOW,
            &impostor_admission
        ),
        device_session(
            mr2,
            mw2,
            impostor_as_device,
            HOST.endpoint,
            "Kai's iPhone",
            Nonce(key(42)),
            NOW,
            |_| {},
        ),
        host_session(
            hr,
            hw,
            HOST,
            impostor_as_device.endpoint,
            Nonce(key(12)),
            NOW,
            &host_admission
        ),
    );
    let on_phone = phone_shown.lock().unwrap().unwrap();
    let on_computer = host_shown.lock().unwrap().unwrap();
    assert_ne!(on_phone, on_computer);
    // The person sees different codes and clicks Don't connect: no answer.
    assert_eq!(phone.unwrap(), DeviceOutcome::NotConnected);
    assert_eq!(host.unwrap(), HostOutcome::Declined);

    // Even an impostor that forwards the real nonces both ways cannot line
    // the codes up: its own keys are in both transcripts.
    let phone_view = sas_code(&Transcript {
        host_endpoint: impostor_as_host.endpoint,
        device_endpoint: DEVICE.endpoint,
        host_nostr: impostor_as_host.nostr,
        device_nostr: DEVICE.nostr,
        host_nonce: Nonce(key(12)),
        device_nonce: Nonce(key(22)),
    });
    let host_view = sas_code(&Transcript {
        host_endpoint: HOST.endpoint,
        device_endpoint: impostor_as_device.endpoint,
        host_nostr: HOST.nostr,
        device_nostr: impostor_as_device.nostr,
        host_nonce: Nonce(key(12)),
        device_nonce: Nonce(key(22)),
    });
    assert_ne!(phone_view, host_view);
}

#[tokio::test]
async fn a_reveal_that_does_not_open_the_commitment_is_refused() {
    let ((mut dr, mut dw), (hr, hw)) = pipe();
    let (admission, shown) = clicker(Verdict::Connect { event: grant() });
    let device = async move {
        write_frame(
            &mut dw,
            &request_message("phone", &Nonce(key(22))),
            MAX_MESSAGE,
        )
        .await
        .unwrap();
        let _: NearbyOffer = read_frame(&mut dr, MAX_MESSAGE).await.unwrap();
        // A nonce chosen after seeing the host's.
        write_frame(&mut dw, &reveal_message(&Nonce(key(23))), MAX_MESSAGE)
            .await
            .unwrap();
        let answer: Option<HostAnswer> = read_optional(&mut dr, MAX_FRAME).await.unwrap();
        answer
    };
    let (out, answer) = tokio::join!(
        host_session(
            hr,
            hw,
            HOST,
            DEVICE.endpoint,
            Nonce(key(12)),
            NOW,
            &admission
        ),
        device
    );
    assert_eq!(out, Err(NearbyError::Commitment));
    assert_eq!(answer, None, "a mismatch earns no answer");
    assert!(
        shown.lock().unwrap().is_none(),
        "no code reached the screen"
    );
}

#[tokio::test]
async fn a_refusal_finishes_the_stream_before_any_nonce() {
    struct Busy;
    impl Admission for Busy {
        type Ticket = ClickTicket;
        fn admit(&self, _: &NearbyRequest) -> Result<ClickTicket, Refusal> {
            Err(Refusal::Limited)
        }
    }
    let ((dr, dw), (hr, hw)) = pipe();
    let (h, d) = tokio::join!(
        host_session(hr, hw, HOST, DEVICE.endpoint, Nonce(key(12)), NOW, &Busy),
        device_session(
            dr,
            dw,
            DEVICE,
            HOST.endpoint,
            "p",
            Nonce(key(22)),
            NOW,
            |_| { panic!("no code without an admitted request") }
        ),
    );
    assert_eq!(h.unwrap(), HostOutcome::Refused(Refusal::Limited));
    assert_eq!(d.unwrap(), DeviceOutcome::Refused);
}

#[tokio::test]
async fn a_phone_clock_far_off_stops_before_the_reveal() {
    let ((dr, dw), (hr, hw)) = pipe();
    let (admission, shown) = clicker(Verdict::Connect { event: grant() });
    let (h, d) = tokio::join!(
        host_session(
            hr,
            hw,
            HOST,
            DEVICE.endpoint,
            Nonce(key(12)),
            NOW,
            &admission
        ),
        device_session(
            dr,
            dw,
            DEVICE,
            HOST.endpoint,
            "p",
            Nonce(key(22)),
            NOW - MAX_SKEW - 1,
            |_| panic!("no code with a skewed clock")
        ),
    );
    assert_eq!(d.unwrap(), DeviceOutcome::ClockSkew { host_now: NOW });
    assert!(h.is_err(), "the host saw no reveal");
    assert!(shown.lock().unwrap().is_none());
}

#[tokio::test]
async fn a_device_that_leaves_withdraws_its_request() {
    struct Never;
    struct NeverTicket;
    impl Admission for Never {
        type Ticket = NeverTicket;
        fn admit(&self, _: &NearbyRequest) -> Result<NeverTicket, Refusal> {
            Ok(NeverTicket)
        }
    }
    impl Ticket for NeverTicket {
        async fn decide(self, _: Code) -> Verdict {
            std::future::pending().await
        }
    }
    let ((mut dr, mut dw), (hr, hw)) = pipe();
    let nonce = Nonce(key(22));
    let device = async move {
        write_frame(&mut dw, &request_message("phone", &nonce), MAX_MESSAGE)
            .await
            .unwrap();
        let _: NearbyOffer = read_frame(&mut dr, MAX_MESSAGE).await.unwrap();
        write_frame(&mut dw, &reveal_message(&nonce), MAX_MESSAGE)
            .await
            .unwrap();
        drop((dr, dw));
    };
    let (out, ()) = tokio::join!(
        host_session(hr, hw, HOST, DEVICE.endpoint, Nonce(key(12)), NOW, &Never),
        device
    );
    assert_eq!(out.unwrap(), HostOutcome::Withdrawn);
}

#[tokio::test]
async fn the_host_refuses_a_malformed_request_with_no_answer() {
    let nonce = Nonce(key(22));
    let mut bad = Vec::new();
    let mut unsupported = request_message("phone", &nonce);
    unsupported.v = "openagents.connect-nearby-request.v2".into();
    bad.push(serde_json::to_value(unsupported).unwrap());
    let mut required = request_message("phone", &nonce);
    required.requires = vec!["x".into()];
    bad.push(serde_json::to_value(required).unwrap());
    bad.push(serde_json::to_value(request_message("bell\u{7}", &nonce)).unwrap());
    bad.push(serde_json::to_value(request_message(&"x".repeat(LABEL_MAX + 1), &nonce)).unwrap());
    let mut unknown = serde_json::to_value(request_message("phone", &nonce)).unwrap();
    unknown["extra"] = json!(1);
    bad.push(unknown);
    for message in bad {
        let ((mut dr, mut dw), (hr, hw)) = pipe();
        let (admission, _) = clicker(Verdict::Connect { event: grant() });
        let device = async move {
            write_frame(&mut dw, &message, MAX_MESSAGE).await.unwrap();
            let reply: Option<NearbyOffer> = read_optional(&mut dr, MAX_MESSAGE).await.unwrap();
            reply
        };
        let (out, reply) = tokio::join!(
            host_session(
                hr,
                hw,
                HOST,
                DEVICE.endpoint,
                Nonce(key(12)),
                NOW,
                &admission
            ),
            device
        );
        assert!(out.is_err());
        assert_eq!(reply, None);
    }
    // The enroll handler dispatches on `v` and hands over the parsed request.
    let value = serde_json::to_value(request_message("phone", &nonce)).unwrap();
    assert_eq!(value["v"], REQUEST);
    assert_eq!(
        NearbyRequestMessage::from_value(value).unwrap(),
        request_message("phone", &nonce)
    );
}

#[tokio::test]
async fn frames_are_bounded_and_typed() {
    let (mut a, mut b) = duplex(64 * 1024);
    a.write_all(&(u32::try_from(MAX_MESSAGE).unwrap() + 1).to_be_bytes())
        .await
        .unwrap();
    let too_long: Result<NearbyOffer, _> = read_frame(&mut b, MAX_MESSAGE).await;
    assert_eq!(too_long, Err(NearbyError::Protocol("frame too long")));
    let big = "x".repeat(MAX_MESSAGE);
    let (mut a, _b) = duplex(64 * 1024);
    assert!(write_frame(&mut a, &big, MAX_MESSAGE).await.is_err());

    let (mut a, mut b) = duplex(64 * 1024);
    write_frame(&mut a, &reveal_message(&Nonce(key(1))), MAX_MESSAGE)
        .await
        .unwrap();
    let wrong: Result<NearbyOffer, _> = read_frame(&mut b, MAX_MESSAGE).await;
    assert_eq!(wrong, Err(NearbyError::Protocol("malformed frame")));

    assert!(parse_key(&"A".repeat(64)).is_err());
    assert_eq!(parse_key(&hex(&key(0xab))).unwrap(), key(0xab));
}

#[test]
fn labels_are_cleaned_and_bounded() {
    assert_eq!(clean_label("  Kai's\u{7} Mac \n"), "Kai's Mac");
    let long = "é".repeat(40);
    let cleaned = clean_label(&long);
    assert!(cleaned.len() <= LABEL_MAX);
    assert_eq!(cleaned, "é".repeat(24));
}

#[test]
fn the_advertisement_carries_only_addresses_and_the_label() {
    let ip: SocketAddr = "192.168.1.20:4433".parse().unwrap();
    let ip6: SocketAddr = "[fe80::1]:4433".parse().unwrap();
    let mut data = EndpointData::new(vec![TransportAddr::Ip(ip), TransportAddr::Ip(ip6)]);
    data.add_relay_url("https://iroh.openagents.com".parse().unwrap());
    data.set_user_data(Some("npub1secret".parse().unwrap()));

    let out = advertised(&data, "Kai's MacBook Pro\n");
    assert_eq!(out.relay_urls().count(), 0, "no relay URL");
    assert_eq!(out.addrs().count(), 2, "only the two IP addresses");
    assert!(out.addrs().all(|addr| matches!(addr, TransportAddr::Ip(_))));
    assert_eq!(
        out.user_data().map(AsRef::as_ref),
        Some("Kai's MacBook Pro"),
        "the label replaces any other user data"
    );

    // With no label, nothing but addresses.
    let bare = advertised(&data, " ");
    assert!(bare.user_data().is_none());
    assert_eq!(bare.relay_urls().count(), 0);

    // What a phone reads back from such a record: the EndpointId (the
    // record's instance name), addresses, and the label.
    let id = SecretKey::from_bytes(&key(9)).public();
    let event = DiscoveryEvent::Discovered {
        endpoint_info: EndpointInfo::from_parts(id, out),
        last_updated: None,
    };
    let Some(NearbyEvent::Found(found)) = nearby_event(event) else {
        panic!("not found");
    };
    assert_eq!(found.endpoint, id);
    assert_eq!(found.label, "Kai's MacBook Pro");
    assert_eq!(found.addrs.len(), 2);
    assert_eq!(found.dial_addr().id, id);
    assert!(found.dial_addr().addrs.iter().all(|a| !a.is_relay()));
}

#[test]
fn a_record_without_addresses_is_not_listed() {
    let id = SecretKey::from_bytes(&key(9)).public();
    let event = DiscoveryEvent::Discovered {
        endpoint_info: EndpointInfo::from_parts(id, EndpointData::new(vec![])),
        last_updated: None,
    };
    assert_eq!(nearby_event(event), None);
    assert_eq!(
        nearby_event(DiscoveryEvent::Expired { endpoint_id: id }),
        Some(NearbyEvent::Lost(id))
    );
}
