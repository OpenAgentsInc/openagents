#![allow(clippy::unwrap_used, clippy::expect_used)]

use nostr::domain::{RelaySigner, Tag};
use nostr::pylon::{
    Beacon, Class, Family, Lane, RECEIPT_V, Receipt, Service, Slots, Status, Tier, UnitKind, Units,
    aggregate_event, beacon_event, compute_aggregate, receipt_event, sha256_hex,
};

use super::*;

pub(super) const NOW: u64 = 1_791_400_000;

pub(super) fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
}

pub(super) fn beacon(provider: &RelaySigner, observed_at: u64, free: u32) -> Beacon {
    Beacon {
        v: pylon::BEACON_V.into(),
        requires: Vec::new(),
        meta: None,
        provider: provider.pubkey().into(),
        pylon: "studio-4080".into(),
        label: "studio-4080".into(),
        status: Status::Online,
        generation: 1,
        since: observed_at - 600,
        observed_at,
        valid_until: observed_at + 240,
        class: Class {
            family: Family::Gpu,
            tier: Tier::Medium,
            memory_gb: 16,
        },
        slots: Slots { total: 2, free },
        services: vec![Service {
            capability: format!("{}:pylon/text-generation", provider.pubkey()),
            model: "qwen3.5-0.8b-q8_0".into(),
            lanes: vec![Lane::CjConversation],
            offering: None,
            price_hint_msat: None,
        }],
        settlement: vec!["free-v1".into()],
        pools: vec!["everglade".into()],
    }
}

pub(super) fn receipt(
    buyer: &RelaySigner,
    provider: &RelaySigner,
    request: &str,
    at: u64,
) -> Event {
    let body = Receipt {
        v: RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: buyer.pubkey().into(),
        provider: provider.pubkey().into(),
        pylon: "studio-4080".into(),
        lane: Lane::CjConversation,
        capability: format!("{}:pylon/text-generation", provider.pubkey()),
        request: request.into(),
        request_digest: sha256_hex(b"request"),
        result_digest: Some(sha256_hex(b"result")),
        started_at: at - 2,
        finished_at: at,
        units: Units {
            kind: UnitKind::Tokens,
            count: 40,
        },
        outcome: Outcome::Accepted,
        payment: None,
    };
    receipt_event(buyer, &body, at).unwrap()
}

#[test]
fn a_live_field_shows_verified_beacons_and_turns_stale_ones_unknown() {
    let provider = signer(1);
    let mut live = Live::new(Some("everglade"));
    assert!(live.offer(
        beacon_event(&provider, &beacon(&provider, NOW, 1)).unwrap(),
        NOW
    ));
    let pylons = live.pylons(NOW + 5);
    assert_eq!(pylons.len(), 1);
    assert_eq!(pylons[0].state.status, "online");
    assert_eq!(pylons[0].state.busy, 1);
    assert_eq!(pylons[0].memory_gb, 16);
    // Past its validity, the same beacon is unknown, never online.
    let stale = live.pylons(NOW + 241);
    assert_eq!(stale[0].state.status, "unknown");
    assert_eq!(stale[0].state.busy, 0);

    // A tampered beacon is refused, and so is one from the future.
    let mut tampered = beacon_event(&provider, &beacon(&provider, NOW + 10, 0)).unwrap();
    tampered.content = tampered.content.replace("\"free\":0", "\"free\":2");
    assert!(!live.offer(tampered, NOW + 10));
    let ahead = beacon_event(&provider, &beacon(&provider, NOW + 100, 0)).unwrap();
    assert!(!live.offer(ahead, NOW));
    // A beacon for another pool counts but isn't drawn.
    let other = signer(2);
    let mut elsewhere = beacon(&other, NOW, 2);
    elsewhere.pools = vec!["elsewhere".into()];
    assert!(live.offer(beacon_event(&other, &elsewhere).unwrap(), NOW));
    assert_eq!(live.pylons(NOW).len(), 1);
}

#[test]
fn receipts_count_jobs_once_and_drive_the_rate() {
    let provider = signer(1);
    let buyer = signer(3);
    let mut live = Live::new(Some("everglade"));
    live.offer(
        beacon_event(&provider, &beacon(&provider, NOW, 2)).unwrap(),
        NOW,
    );
    assert!(live.offer(receipt(&buyer, &provider, &"aa".repeat(32), NOW - 10), NOW));
    // The same request twice counts once.
    assert!(!live.offer(receipt(&buyer, &provider, &"aa".repeat(32), NOW - 5), NOW));
    assert!(live.offer(
        receipt(&buyer, &provider, &"bb".repeat(32), NOW - 3_000),
        NOW
    ));
    // A tampered receipt is refused.
    let mut tampered = receipt(&buyer, &provider, &"cc".repeat(32), NOW - 5);
    tampered
        .tags
        .push(Tag::new(vec!["t".into(), "extra".into()]));
    assert!(!live.offer(tampered, NOW));
    assert_eq!(live.pylons(NOW)[0].state.jobs, 2);
    // One finished in the last minute.
    assert_eq!(live.rate(NOW), 1);
    assert_eq!(live.rate(NOW + 120), 0);
    // A day later both have aged out.
    live.settle(NOW + RECEIPT_WINDOW_SECS + 1);
    assert_eq!(live.pylons(NOW + RECEIPT_WINDOW_SECS + 1)[0].state.jobs, 0);
}

#[test]
fn an_aggregate_drives_the_rate_and_lights_only_when_it_recomputes() {
    let provider = signer(1);
    let buyer = signer(3);
    let aggregator = signer(4);
    let mut live = Live::new(Some("everglade"));
    let beacon = beacon_event(&provider, &beacon(&provider, NOW - 30, 2)).unwrap();
    let receipts = [
        receipt(&buyer, &provider, &"aa".repeat(32), NOW - 20),
        receipt(&buyer, &provider, &"bb".repeat(32), NOW - 15),
    ];
    live.offer(beacon.clone(), NOW);
    for r in &receipts {
        live.offer(r.clone(), NOW);
    }
    let window = crate::pool::window(NOW, 60);
    let policy = PoolPolicy::open("everglade", crate::pool::SLICES);
    let body = compute_aggregate(
        aggregator.pubkey(),
        &policy,
        window,
        &AggregateInputs {
            beacons: std::slice::from_ref(&beacon),
            receipts: &receipts,
            checks: &[],
        },
        NOW,
    )
    .unwrap();
    let event = aggregate_event(&aggregator, &body).unwrap();

    // A tampered aggregate is refused outright.
    let mut tampered = body.clone();
    tampered.totals.jobs.accepted = 900;
    let forged = aggregate_event(&aggregator, &tampered).unwrap();
    let mut forged_sig = event.clone();
    forged_sig.content = forged.content.clone();
    assert!(!live.offer(forged_sig, NOW));

    // The honest one verifies against what the field holds.
    assert!(live.offer(event, NOW));
    live.settle(NOW);
    let (held, verified) = live.aggregate(NOW).unwrap();
    assert_eq!(held.totals.jobs.accepted, 2);
    assert!(verified);
    // Two jobs in the newest five-minute slice: one a minute, rounded up.
    assert_eq!(live.rate(NOW), 1);

    // A signed aggregate whose totals don't recompute counts for the rate
    // but never lights the rim.
    let mut live = Live::new(Some("everglade"));
    live.offer(beacon, NOW);
    let mut inflated = body;
    inflated.generated_at += 1;
    inflated.valid_until += 1;
    inflated.totals.jobs.accepted = 900;
    *inflated.rate.last_mut().unwrap() = 900;
    live.offer(aggregate_event(&aggregator, &inflated).unwrap(), NOW);
    live.settle(NOW);
    assert!(!live.aggregate(NOW).unwrap().1);
    // Past its validity it drives nothing.
    assert!(live.aggregate(NOW + 400).is_none());
    assert_eq!(live.rate(NOW + 400), 0);
}

#[test]
fn a_live_field_holds_a_bounded_number_of_pylons() {
    let mut live = Live::new(None);
    for i in 0..=MAX_LIVE_PYLONS {
        let mut secret = [0_u8; 32];
        secret[30] = 1 + (i / 200) as u8;
        secret[31] = 1 + (i % 200) as u8;
        let provider = RelaySigner::from_secret_hex(
            &secret
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        )
        .unwrap();
        let taken = live.offer(
            beacon_event(&provider, &self::beacon(&provider, NOW, 2)).unwrap(),
            NOW,
        );
        assert_eq!(taken, i < MAX_LIVE_PYLONS, "{i}");
    }
    assert_eq!(live.pylons(NOW).len(), MAX_LIVE_PYLONS);
}
