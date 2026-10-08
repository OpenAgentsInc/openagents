#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

const NOW: u64 = 1_791_400_000;

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
}

fn beacon(provider: &RelaySigner, observed_at: u64) -> Beacon {
    Beacon {
        v: BEACON_V.into(),
        requires: Vec::new(),
        meta: None,
        provider: provider.pubkey().into(),
        pylon: "coderos-4080".into(),
        label: "RTX 4080".into(),
        status: Status::Online,
        generation: 1,
        since: observed_at - 100,
        observed_at,
        valid_until: observed_at + 240,
        class: Class {
            family: Family::Gpu,
            tier: Tier::for_gpu(16),
            memory_gb: 16,
        },
        slots: Slots { total: 2, free: 1 },
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

fn receipt(buyer: &RelaySigner, provider: &RelaySigner, request: &str, at: u64) -> Receipt {
    Receipt {
        v: RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: buyer.pubkey().into(),
        provider: provider.pubkey().into(),
        pylon: "coderos-4080".into(),
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
    }
}

fn resign(signer: &RelaySigner, event: &Event, content: String, tags: Vec<Tag>) -> Event {
    signer.sign(event.created_at, event.kind, tags, content)
}

#[test]
fn a_beacon_round_trips_and_judges_freshness() {
    let provider = signer(1);
    let body = beacon(&provider, NOW);
    let event = beacon_event(&provider, &body).unwrap();
    assert_eq!(event.kind, BEACON_KIND);
    assert_eq!(parse_beacon(&event).unwrap(), body);
    assert_eq!(freshness(&body, NOW + 10), Freshness::Fresh);
    assert_eq!(freshness(&body, NOW + 241), Freshness::Stale);
    assert_eq!(freshness(&body, NOW - 31), Freshness::Future);
    assert_eq!(project(&body, NOW + 10, 3).status, "online");
    assert_eq!(project(&body, NOW + 10, 3).busy, 1);
    let stale = project(&body, NOW + 1_000, 3);
    assert_eq!(stale.status, "unknown");
    assert_eq!(stale.busy, 0);
}

#[test]
fn a_beacon_refuses_each_validation_rule() {
    let provider = signer(1);
    let other = signer(2);
    let body = beacon(&provider, NOW);
    let event = beacon_event(&provider, &body).unwrap();

    // Tampered content under the original signature.
    let mut tampered = event.clone();
    tampered.content = tampered.content.replace("\"free\":1", "\"free\":2");
    assert!(parse_beacon(&tampered).is_err());

    // Re-signed by another key: the provider is not the signer.
    let stolen = resign(&other, &event, event.content.clone(), event.tags.clone());
    assert!(
        parse_beacon(&stolen)
            .unwrap_err()
            .contains("not the signer")
    );

    // An `x` tag that does not match.
    let mut tags = event.tags.clone();
    tags[2] = Tag::new(vec!["x".into(), sha256_hex(b"other")]);
    let bad_x = resign(&provider, &event, event.content.clone(), tags);
    assert!(parse_beacon(&bad_x).unwrap_err().contains("`x`"));

    // A `d` tag that differs from the slug.
    let mut tags = event.tags.clone();
    tags[0] = Tag::new(vec!["d".into(), "other".into()]);
    assert!(
        parse_beacon(&resign(&provider, &event, event.content.clone(), tags))
            .unwrap_err()
            .contains("`d`")
    );

    // An expiration that differs from valid_until.
    let mut tags = event.tags.clone();
    tags[3] = Tag::new(vec!["expiration".into(), "1".into()]);
    assert!(
        parse_beacon(&resign(&provider, &event, event.content.clone(), tags))
            .unwrap_err()
            .contains("expiration")
    );

    // Body rules: free above total, validity too long, unknown field.
    let mut wide = body.clone();
    wide.slots.free = 3;
    assert!(beacon_event(&provider, &wide).is_err());
    let mut long = body.clone();
    long.valid_until = long.observed_at + 301;
    assert!(beacon_event(&provider, &long).is_err());
    let mut value: Value = serde_json::from_str(&event.content).unwrap();
    value["gpu_util"] = Value::from(90);
    let content = String::from_utf8(jcs(&value).unwrap()).unwrap();
    let mut tags = event.tags.clone();
    tags[2] = Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]);
    assert!(
        parse_beacon(&resign(&provider, &event, content, tags))
            .unwrap_err()
            .contains("bad body")
    );

    // Someone else cannot sign a beacon for the provider.
    assert!(beacon_event(&other, &body).is_err());
}

#[test]
fn the_book_keeps_the_newest_beacon_and_refuses_rollback() {
    let provider = signer(1);
    let mut book = BeaconBook::default();
    let first = beacon(&provider, NOW);
    assert!(book.offer(beacon_event(&provider, &first).unwrap(), first.clone()));
    let older = beacon(&provider, NOW - 10);
    assert!(!book.offer(beacon_event(&provider, &older).unwrap(), older));
    let mut rolled = beacon(&provider, NOW + 10);
    rolled.generation = 0;
    assert!(!book.offer(beacon_event(&provider, &rolled).unwrap(), rolled));
    let newer = beacon(&provider, NOW + 60);
    assert!(book.offer(beacon_event(&provider, &newer).unwrap(), newer));
    assert_eq!(book.get(&first.address()).unwrap().1.observed_at, NOW + 60);
}

#[test]
fn a_receipt_round_trips_and_refuses_each_validation_rule() {
    let provider = signer(1);
    let buyer = signer(3);
    let owner = signer(4);
    let body = receipt(&buyer, &provider, &"ab".repeat(32), NOW);
    let event = receipt_event(&buyer, &body, NOW).unwrap();
    assert_eq!(parse_receipt(&event, None).unwrap(), body);

    // The provider's own receipt never counts.
    let own = receipt(&provider, &provider, &"ab".repeat(32), NOW);
    assert!(receipt_event(&provider, &own, NOW).is_err());
    // Nor the beacon owner's.
    let by_owner = receipt(&owner, &provider, &"ab".repeat(32), NOW);
    let by_owner = receipt_event(&owner, &by_owner, NOW).unwrap();
    assert!(parse_receipt(&by_owner, Some(owner.pubkey())).is_err());

    // Tampered content under the original signature.
    let mut tampered = event.clone();
    tampered.content = tampered.content.replace("\"count\":40", "\"count\":4000");
    assert!(parse_receipt(&tampered, None).is_err());

    // A `p` tag that disagrees with the body.
    let mut tags = event.tags.clone();
    tags[0] = Tag::new(vec!["p".into(), owner.pubkey().into()]);
    assert!(
        parse_receipt(&resign(&buyer, &event, event.content.clone(), tags), None)
            .unwrap_err()
            .contains("`p`")
    );

    // A preimage that does not hash to its payment hash.
    let mut paid = body.clone();
    paid.payment = Some(Payment {
        profile: "lightning-bolt11".into(),
        network: "regtest".into(),
        amount_msat: 2_000,
        payment_hash: sha256_hex(&[7; 32]),
        preimage: "08".repeat(32),
    });
    assert!(
        receipt_event(&buyer, &paid, NOW)
            .unwrap_err()
            .contains("preimage")
    );
    paid.payment.as_mut().unwrap().preimage = "07".repeat(32);
    assert!(receipt_event(&buyer, &paid, NOW).is_ok());
}

fn pool_inputs() -> (RelaySigner, Vec<Event>, Vec<Event>) {
    let provider = signer(1);
    let buyer = signer(3);
    let to = 1_791_400_200; // a whole minute
    let beacons = vec![beacon_event(&provider, &beacon(&provider, to - 30)).unwrap()];
    let mut receipts = Vec::new();
    for (i, at) in [to - 3_000, to - 1_000, to - 10].iter().enumerate() {
        let body = receipt(&buyer, &provider, &format!("{i:02x}").repeat(32), *at);
        receipts.push(receipt_event(&buyer, &body, *at).unwrap());
    }
    // A second receipt for the first request: ignored.
    let again = receipt(&buyer, &provider, &"00".repeat(32), to - 5);
    receipts.push(receipt_event(&buyer, &again, to).unwrap());
    // A paid receipt on regtest stays apart from bitcoin.
    let mut paid = receipt(&buyer, &provider, &"0f".repeat(32), to - 20);
    paid.payment = Some(Payment {
        profile: "lightning-bolt11".into(),
        network: "regtest".into(),
        amount_msat: 2_000,
        payment_hash: sha256_hex(&[7; 32]),
        preimage: "07".repeat(32),
    });
    receipts.push(receipt_event(&buyer, &paid, to - 20).unwrap());
    (signer(9), beacons, receipts)
}

#[test]
fn an_aggregate_recomputes_and_refuses_tampering() {
    let (aggregator, beacons, receipts) = pool_inputs();
    let policy = PoolPolicy::open("everglade", 12);
    let to = 1_791_400_200;
    let window = Window {
        from: to - 3_600,
        to,
    };
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
    };
    let aggregate = compute_aggregate(aggregator.pubkey(), &policy, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.pylons_online, 1);
    assert_eq!(aggregate.totals.by_family.gpu, 1);
    assert_eq!(aggregate.totals.slots_free, 1);
    assert_eq!(aggregate.totals.jobs.accepted, 4);
    assert_eq!(aggregate.inputs.receipts.count, 4);
    assert_eq!(aggregate.totals.units.tokens, 160);
    assert_eq!(aggregate.totals.paid_msat.regtest, 2_000);
    assert_eq!(aggregate.totals.paid_msat.bitcoin, 0);
    assert_eq!(aggregate.rate.iter().sum::<u64>(), 4);
    let event = aggregate_event(&aggregator, &aggregate).unwrap();
    assert_eq!(
        verify_aggregate(&event, &policy, &inputs).unwrap(),
        aggregate
    );

    // Inflated totals, signed by the aggregator itself, do not recompute.
    let mut inflated = aggregate.clone();
    inflated.totals.jobs.accepted = 400;
    let forged = aggregate_event(&aggregator, &inflated).unwrap();
    assert!(
        verify_aggregate(&forged, &policy, &inputs)
            .unwrap_err()
            .contains("totals")
    );

    // A wrong input digest does not recompute.
    let mut wrong = aggregate.clone();
    wrong.inputs.receipts.digest = sha256_hex(b"other");
    let forged = aggregate_event(&aggregator, &wrong).unwrap();
    assert!(
        verify_aggregate(&forged, &policy, &inputs)
            .unwrap_err()
            .contains("digests")
    );

    // A tampered receipt in the inputs drops out, so the claim no longer
    // matches what the reader can verify.
    let mut tampered = receipts.clone();
    tampered[1].content = tampered[1].content.replace("\"count\":40", "\"count\":41");
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &tampered,
    };
    assert!(verify_aggregate(&event, &policy, &inputs).is_err());

    // A different policy refuses.
    let other = PoolPolicy::open("everglade", 6);
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
    };
    assert!(verify_aggregate(&event, &other, &inputs).is_err());
}

#[test]
fn an_aggregate_counts_only_trusted_buyers() {
    let (aggregator, beacons, receipts) = pool_inputs();
    let mut policy = PoolPolicy::open("everglade", 12);
    policy.buyers = Some(BTreeSet::from([signer(5).pubkey().to_string()]));
    let to = 1_791_400_200;
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
    };
    let window = Window {
        from: to - 3_600,
        to,
    };
    let aggregate = compute_aggregate(aggregator.pubkey(), &policy, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.jobs.accepted, 0);
    assert_eq!(aggregate.inputs.receipts.digest, id_set_digest([]));
}

fn owner_credential(owner: u8, agent: &RelaySigner, conditions: &str) -> MintedOwnerAttestation {
    let secret = secp256k1::SecretKey::from_byte_array([owner; 32]).unwrap();
    crate::domain::mint_owner_attestation(&secret, agent.pubkey(), conditions).unwrap()
}

#[test]
fn a_beacon_carries_a_verified_nip_oa_owner() {
    let provider = signer(1);
    let owner = signer(7);
    let body = beacon(&provider, NOW);
    let credential = owner_credential(7, &provider, "kind=30200");
    let event = owned_beacon_event(&provider, &body, Some(&credential)).unwrap();
    let (parsed, found) = parse_owned_beacon(&event).unwrap();
    assert_eq!(parsed, body);
    assert_eq!(found.as_deref(), Some(owner.pubkey()));
    // A beacon without one is valid and shows no owner.
    let plain = beacon_event(&provider, &body).unwrap();
    assert_eq!(parse_owned_beacon(&plain).unwrap().1, None);

    // A credential minted for another key does not sign for this pylon.
    let stranger = owner_credential(7, &signer(2), "kind=30200");
    assert!(owned_beacon_event(&provider, &body, Some(&stranger)).is_err());
    let mut tags = plain.tags.clone();
    tags.push(stranger.tag());
    let borrowed = resign(&provider, &plain, plain.content.clone(), tags);
    assert!(parse_beacon(&borrowed).unwrap_err().contains("NIP-OA"));

    // A credential for another kind, or a forged owner, is refused.
    let receipts_only = owner_credential(7, &provider, "kind=3201");
    assert!(owned_beacon_event(&provider, &body, Some(&receipts_only)).is_err());
    let mut forged = credential.clone();
    forged.owner_pubkey = signer(8).pubkey().into();
    let mut tags = plain.tags.clone();
    tags.push(forged.tag());
    assert!(parse_beacon(&resign(&provider, &plain, plain.content.clone(), tags)).is_err());

    // Two auth tags are refused.
    let mut tags = event.tags.clone();
    tags.push(credential.tag());
    assert!(parse_beacon(&resign(&provider, &event, event.content.clone(), tags)).is_err());
}

#[test]
fn an_aggregate_never_counts_the_owners_receipts() {
    let provider = signer(1);
    let owner = signer(7);
    let buyer = signer(3);
    let to = 1_791_400_200;
    let credential = owner_credential(7, &provider, "kind=30200");
    let beacons = vec![
        owned_beacon_event(&provider, &beacon(&provider, to - 30), Some(&credential)).unwrap(),
    ];
    let mut receipts = Vec::new();
    for (i, who) in [&buyer, &owner, &provider].into_iter().enumerate() {
        let body = receipt(who, &provider, &format!("{i:02x}").repeat(32), to - 10);
        if let Ok(event) = receipt_event(who, &body, to - 10) {
            receipts.push(event);
        }
    }
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
    };
    let window = Window {
        from: to - 3_600,
        to,
    };
    let policy = PoolPolicy::open("everglade", 12);
    let aggregator = signer(9);
    let aggregate = compute_aggregate(aggregator.pubkey(), &policy, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.jobs.accepted, 1);
    let event = aggregate_event(&aggregator, &aggregate).unwrap();
    assert_eq!(
        verify_aggregate(&event, &policy, &inputs).unwrap(),
        aggregate
    );
}
