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
        group: None,
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

    let mut paid = body.clone();
    paid.payment = Some(Payment {
        profile: "lightning-bolt11".into(),
        network: "regtest".into(),
        amount_msat: 2_000,
        settlement: "07".repeat(32),
        acknowledgment: None,
    });
    assert!(
        receipt_event(&buyer, &paid, NOW)
            .unwrap_err()
            .contains("acknowledgment")
    );
    acknowledge_payment(&provider, &mut paid).unwrap();
    let signed = receipt_event(&buyer, &paid, NOW).unwrap();
    assert_eq!(parse_receipt(&signed, None).unwrap(), paid);
    assert!(!signed.content.contains("preimage"));
    assert!(!signed.content.contains("payment_hash"));
    let mut changed = paid.clone();
    changed.request = "cd".repeat(32);
    assert!(receipt_event(&buyer, &changed, NOW).is_err());
    changed = paid.clone();
    changed.payment.as_mut().unwrap().amount_msat += 1;
    assert!(receipt_event(&buyer, &changed, NOW).is_err());
    changed = paid.clone();
    changed.outcome = Outcome::Failed;
    assert!(receipt_event(&buyer, &changed, NOW).is_err());
    assert!(acknowledge_payment(&owner, &mut changed).is_err());
    changed = paid.clone();
    let ack = changed
        .payment
        .as_mut()
        .unwrap()
        .acknowledgment
        .as_mut()
        .unwrap();
    ack.content.push(' ');
    assert!(receipt_event(&buyer, &changed, NOW).is_err());
    changed = paid.clone();
    let ack = changed
        .payment
        .as_mut()
        .unwrap()
        .acknowledgment
        .as_mut()
        .unwrap();
    *ack = owner.sign(
        ack.created_at,
        ack.kind,
        ack.tags.clone(),
        ack.content.clone(),
    );
    assert!(receipt_event(&buyer, &changed, NOW).is_err());
    for field in ["network", "settlement"] {
        changed = paid.clone();
        let payment = changed.payment.as_mut().unwrap();
        if field == "network" {
            payment.network = "bitcoin".into();
        } else {
            payment.settlement = "08".repeat(32);
        }
        assert!(receipt_event(&buyer, &changed, NOW).is_err());
    }
    let mut legacy = serde_json::to_value(&paid).unwrap();
    legacy["payment"]["preimage"] = serde_json::json!("07".repeat(32));
    assert!(serde_json::from_value::<Receipt>(legacy).is_err());
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
        settlement: "07".repeat(32),
        acknowledgment: None,
    });
    acknowledge_payment(&provider, &mut paid).unwrap();
    receipts.push(receipt_event(&buyer, &paid, to - 20).unwrap());
    (signer(9), beacons, receipts)
}

#[test]
fn an_aggregate_recomputes_and_refuses_tampering() {
    let (aggregator, beacons, receipts) = pool_inputs();
    let policy =
        PoolPolicy::open("everglade", 12).trusting_payments([signer(1).pubkey().to_string()]);
    let to = 1_791_400_200;
    let window = Window {
        from: to - 3_600,
        to,
    };
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
        checks: &[],
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
        checks: &[],
    };
    assert!(verify_aggregate(&event, &policy, &inputs).is_err());

    // A different policy refuses.
    let other = PoolPolicy::open("everglade", 6);
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
        checks: &[],
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
        checks: &[],
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
        checks: &[],
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

#[test]
fn a_check_label_round_trips_and_refuses_each_rule() {
    let (_, _, receipts) = pool_inputs();
    let checker = signer(11);
    let target = &receipts[0];
    let suite = sha256_hex(b"suite");
    let event = check_event(
        &checker,
        Verdict::Fail,
        target,
        &sha256_hex(b"42"),
        &format!("canary exact-match suite:{suite}"),
        NOW,
    )
    .unwrap();
    let check = parse_check(&event).unwrap();
    assert_eq!(check.verdict, Verdict::Fail);
    assert_eq!(check.receipt, target.id);
    assert_eq!(check.provider, signer(1).pubkey());
    assert_eq!(check.suite(), Some(suite.as_str()));
    let receipt = parse_receipt(target, None).unwrap();
    bind_check(&check, &receipt).unwrap();

    // The receipt's own buyer or provider can't check it.
    for party in [signer(3), signer(1)] {
        assert!(check_event(&party, Verdict::Pass, target, &sha256_hex(b"x"), "m", NOW).is_err());
    }
    // Wrong namespace, an unknown verdict, two verdicts, and long content.
    let tags = |l: &str, mark: &str| {
        vec![
            Tag::new(vec!["L".into(), CHECK_NAMESPACE.into()]),
            Tag::new(vec!["l".into(), l.into(), mark.into()]),
            Tag::new(vec!["e".into(), target.id.clone()]),
            Tag::new(vec!["p".into(), signer(1).pubkey().into()]),
            Tag::new(vec!["x".into(), sha256_hex(b"42")]),
        ]
    };
    let ok = resign(
        &checker,
        &event,
        "m".into(),
        tags("check-pass", CHECK_NAMESPACE),
    );
    assert_eq!(parse_check(&ok).unwrap().verdict, Verdict::Pass);
    let bad = resign(&checker, &event, "m".into(), tags("check-pass", "other"));
    assert!(parse_check(&bad).is_err());
    let bad = resign(
        &checker,
        &event,
        "m".into(),
        tags("check-maybe", CHECK_NAMESPACE),
    );
    assert!(parse_check(&bad).is_err());
    let mut two = tags("check-pass", CHECK_NAMESPACE);
    two.push(Tag::new(vec![
        "l".into(),
        "check-fail".into(),
        CHECK_NAMESPACE.into(),
    ]));
    assert!(parse_check(&resign(&checker, &event, "m".into(), two)).is_err());
    let long = "m".repeat(513);
    let bad = resign(&checker, &event, long, tags("check-pass", CHECK_NAMESPACE));
    assert!(parse_check(&bad).is_err());
    // A tampered label no longer verifies.
    let mut tampered = event.clone();
    tampered.content = "redundant-3".into();
    assert!(parse_check(&tampered).is_err());
}

#[test]
fn a_failed_check_from_a_trusted_checker_leaves_the_pools_admission() {
    let (aggregator, beacons, receipts) = pool_inputs();
    let to = 1_791_400_200;
    let window = Window {
        from: to - 3_600,
        to,
    };
    let checker = signer(11);
    let stranger = signer(12);
    let digest = sha256_hex(b"42");
    let fail = check_event(
        &checker,
        Verdict::Fail,
        &receipts[2],
        &digest,
        "canary",
        to - 5,
    )
    .unwrap();
    let pass = check_event(
        &checker,
        Verdict::Pass,
        &receipts[1],
        &digest,
        "canary",
        to - 6,
    )
    .unwrap();
    let untrusted = check_event(
        &stranger,
        Verdict::Pass,
        &receipts[0],
        &digest,
        "canary",
        to - 7,
    )
    .unwrap();

    // Only the trusted checker's pass counts; the pylon stays admitted.
    let policy = PoolPolicy::open("everglade", 12).checked([checker.pubkey().to_string()]);
    let checks = vec![pass.clone(), untrusted.clone()];
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
        checks: &checks,
    };
    let aggregate = compute_aggregate(aggregator.pubkey(), &policy, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.checks.pass, 1);
    assert_eq!(aggregate.inputs.checks.count, 1);
    assert_eq!(aggregate.totals.pylons_online, 1);
    assert_eq!(aggregate.totals.jobs.accepted, 4);
    let event = aggregate_event(&aggregator, &aggregate).unwrap();
    assert_eq!(
        verify_aggregate(&event, &policy, &inputs).unwrap(),
        aggregate
    );

    // A counted fail marks the pylon failing: it leaves admission, so
    // neither its beacon nor its receipts count.
    let checks = vec![pass, fail, untrusted];
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
        checks: &checks,
    };
    let aggregate = compute_aggregate(aggregator.pubkey(), &policy, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.checks.fail, 1);
    assert_eq!(aggregate.totals.pylons_online, 0);
    assert_eq!(aggregate.totals.jobs.accepted, 0);
    let parsed: Vec<Check> = checks.iter().map(|e| parse_check(e).unwrap()).collect();
    let by_id: BTreeMap<String, Receipt> = receipts
        .iter()
        .map(|e| (e.id.clone(), parse_receipt(e, None).unwrap()))
        .collect();
    let trusted = BTreeSet::from([checker.pubkey().to_string()]);
    let records = standings(counted(&parsed, &by_id, &trusted), &by_id);
    let record = records.values().next().unwrap();
    assert_eq!(record.standing, Standing::Failing);
    assert_eq!(record.totals.pass, 1);

    // The open policy counts no verdicts and keeps the P1 digest.
    let open = PoolPolicy::open("everglade", 12);
    let aggregate = compute_aggregate(aggregator.pubkey(), &open, window, &inputs, to).unwrap();
    assert_eq!(aggregate.totals.checks, CheckTotals::default());
    assert_eq!(aggregate.totals.pylons_online, 1);
    assert!(!serde_json::to_string(&open).unwrap().contains("checkers"));
}

#[test]
fn paid_totals_require_trusted_issuers_and_deduplicate_settlements() {
    let (aggregator, beacons, mut receipts) = pool_inputs();
    let to = 1_791_400_200;
    let mut duplicate = parse_receipt(receipts.last().unwrap(), None).unwrap();
    duplicate.request = "f1".repeat(32);
    acknowledge_payment(&signer(1), &mut duplicate).unwrap();
    receipts.push(receipt_event(&signer(3), &duplicate, to - 15).unwrap());
    let inputs = AggregateInputs {
        beacons: &beacons,
        receipts: &receipts,
        checks: &[],
    };
    let window = Window {
        from: to - 3_600,
        to,
    };
    let open = PoolPolicy::open("everglade", 12);
    let plain = compute_aggregate(aggregator.pubkey(), &open, window, &inputs, to).unwrap();
    assert_eq!(plain.totals.paid_msat.regtest, 0);
    let trusted = open.trusting_payments([signer(1).pubkey().to_string()]);
    let counted = compute_aggregate(aggregator.pubkey(), &trusted, window, &inputs, to).unwrap();
    assert_eq!(counted.totals.paid_msat.regtest, 2_000);
    assert_ne!(counted.policy, plain.policy);
    // A buyer may sign as a broker, but trusting the provider does not trust that buyer.
    acknowledge_payment(&signer(3), &mut duplicate).unwrap();
    let events = [receipt_event(&signer(3), &duplicate, to - 15).unwrap()];
    let inputs = AggregateInputs {
        receipts: &events,
        ..inputs
    };
    assert_eq!(
        compute_aggregate(aggregator.pubkey(), &trusted, window, &inputs, to)
            .unwrap()
            .totals
            .paid_msat
            .regtest,
        0
    );
}

#[test]
fn a_sealed_job_receipt_carries_exactly_its_attested_block() {
    let (buyer, provider) = (signer(1), signer(2));
    let mut body = receipt(&buyer, &provider, &"ab".repeat(32), NOW);
    let block = serde_json::json!({"attested": {
        "endpoint": format!("30203:{}:{}", "cd".repeat(32), "ef".repeat(32)),
        "release": "12".repeat(32), "level": "tee-cloud", "measurement": format!("sha256:{}", "34".repeat(32))}});
    // The block without the feature, and the feature without the block, refuse.
    body.meta = Some(block.clone());
    assert!(body.validate().is_err());
    body.meta = None;
    body.requires = vec![crate::att::ATTESTED_FEATURE.into()];
    assert!(body.validate().is_err());
    body.meta = Some(block.clone());
    body.validate().unwrap();
    let event = receipt_event(&buyer, &body, NOW).unwrap();
    assert_eq!(parse_receipt(&event, None).unwrap(), body);
    // Extra fields, a lower level, and a malformed endpoint refuse.
    for broken in [
        serde_json::json!({"attested": {"endpoint": block["attested"]["endpoint"], "release": "12".repeat(32), "level": "tee", "measurement": "m", "prompt_digest": "x"}}),
        serde_json::json!({"attested": {"endpoint": block["attested"]["endpoint"], "release": "12".repeat(32), "level": "open", "measurement": "m"}}),
        serde_json::json!({"attested": {"endpoint": "30202:x:y", "release": "12".repeat(32), "level": "tee", "measurement": "m"}}),
        serde_json::json!({"attested": block["attested"], "other": 1}),
    ] {
        body.meta = Some(broken);
        assert!(body.validate().is_err());
    }
    // Another feature is still unknown.
    body.meta = Some(block);
    body.requires = vec!["openagents.other.v1".into()];
    assert!(body.validate().is_err());
}

const ARTIFACT: &str = "sha256:4c1f1a7e1b7d4a8c0e9f2b3d5a6c7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d";

/// A V4.1 group head: one serial slot, the exact artifact, a deadline
/// and an output limit.
fn head(provider: &RelaySigner, observed_at: u64) -> Beacon {
    let mut body = beacon(provider, observed_at);
    body.v = BEACON_V2.into();
    body.pylon = "v41-group".into();
    body.slots = Slots { total: 1, free: 1 };
    body.services[0].model = format!("deepseek-v4.1-flash@{ARTIFACT}");
    body.group = Some(Group {
        id: "v41-a".into(),
        role: GroupRole::Head,
        generation: 3,
        artifact: ARTIFACT.into(),
        readiness: Some(Readiness::Ready),
        deadline_secs: Some(600),
        max_output_tokens: Some(4_096),
        layers: None,
    });
    body
}

/// One stage of that group, holding layers 0 to 11.
fn stage(provider: &RelaySigner, observed_at: u64) -> Beacon {
    let mut body = beacon(provider, observed_at);
    body.v = BEACON_V2.into();
    body.pylon = "v41-stage-0".into();
    body.slots = Slots { total: 0, free: 0 };
    body.services = Vec::new();
    body.group = Some(Group {
        id: "v41-a".into(),
        role: GroupRole::Stage,
        generation: 3,
        artifact: ARTIFACT.into(),
        readiness: None,
        deadline_secs: None,
        max_output_tokens: None,
        layers: Some(Layers { from: 0, to: 11 }),
    });
    body
}

#[test]
fn group_beacons_round_trip_under_their_own_marker() {
    let provider = signer(1);
    for body in [head(&provider, NOW), stage(&provider, NOW)] {
        let event = beacon_event(&provider, &body).unwrap();
        assert_eq!(single_tag(&event, "t").unwrap(), GROUP_BEACON_MARKER);
        assert_eq!(parse_beacon(&event).unwrap(), body);
    }
    let head = head(&provider, NOW);
    assert_eq!(head.group_head().unwrap().id, "v41-a");
    assert!(!head.is_stage());
    let stage = stage(&provider, NOW);
    assert!(stage.group_head().is_none() && stage.is_stage());
    assert_eq!(project(&stage, NOW + 10, 0).total, 0);
    // A v1 beacon keeps its v1 marker and bytes.
    let plain = beacon_event(&provider, &beacon(&provider, NOW)).unwrap();
    assert_eq!(single_tag(&plain, "t").unwrap(), BEACON_MARKER);
    assert!(!plain.content.contains("group"));
}

#[test]
fn group_beacons_refuse_each_group_rule() {
    let provider = signer(1);
    let refused = |body: Beacon, why: &str| {
        let err = beacon_event(&provider, &body).unwrap_err();
        assert!(err.contains(why), "{err} does not say {why}");
    };
    let mut v1_group = head(&provider, NOW);
    v1_group.v = BEACON_V.into();
    refused(v1_group, "carries no group");
    let mut v2_bare = beacon(&provider, NOW);
    v2_bare.v = BEACON_V2.into();
    refused(v2_bare, "carries its group");
    let mut two_slots = head(&provider, NOW);
    two_slots.slots = Slots { total: 2, free: 2 };
    refused(two_slots, "one serial slot");
    let mut other_model = head(&provider, NOW);
    other_model.services[0].model = "deepseek-r1-distill".into();
    refused(other_model, "<name>@<artifact>");
    let mut no_name = head(&provider, NOW);
    no_name.services[0].model = format!("@{ARTIFACT}");
    refused(no_name, "<name>@<artifact>");
    let mut not_ready = head(&provider, NOW);
    not_ready.group.as_mut().unwrap().readiness = None;
    refused(not_ready, "readiness");
    let mut no_deadline = head(&provider, NOW);
    no_deadline.group.as_mut().unwrap().deadline_secs = Some(0);
    refused(no_deadline, "deadline");
    let mut no_limit = head(&provider, NOW);
    no_limit.group.as_mut().unwrap().max_output_tokens = None;
    refused(no_limit, "output limit");
    let mut bad_artifact = head(&provider, NOW);
    bad_artifact.group.as_mut().unwrap().artifact = "sha256:abc".into();
    refused(bad_artifact, "artifact");
    let mut slotted_stage = stage(&provider, NOW);
    slotted_stage.slots = Slots { total: 1, free: 1 };
    refused(slotted_stage, "no slots");
    let mut serving_stage = stage(&provider, NOW);
    serving_stage.services = beacon(&provider, NOW).services;
    refused(serving_stage, "no services");
    let mut empty_layers = stage(&provider, NOW);
    empty_layers.group.as_mut().unwrap().layers = Some(Layers { from: 4, to: 4 });
    refused(empty_layers, "layers");
    let mut ready_stage = stage(&provider, NOW);
    ready_stage.group.as_mut().unwrap().readiness = Some(Readiness::Ready);
    refused(ready_stage, "only its layers");

    // A v2 body under the v1 marker, and the reverse, refuse.
    let event = beacon_event(&provider, &head(&provider, NOW)).unwrap();
    let mut tags = event.tags.clone();
    tags[1] = Tag::new(vec!["t".into(), BEACON_MARKER.into()]);
    assert!(
        parse_beacon(&resign(&provider, &event, event.content.clone(), tags))
            .unwrap_err()
            .contains("marker")
    );
    let event = beacon_event(&provider, &beacon(&provider, NOW)).unwrap();
    let mut tags = event.tags.clone();
    tags[1] = Tag::new(vec!["t".into(), GROUP_BEACON_MARKER.into()]);
    assert!(
        parse_beacon(&resign(&provider, &event, event.content.clone(), tags))
            .unwrap_err()
            .contains("marker")
    );
}

#[test]
fn an_aggregate_counts_one_slot_per_group() {
    let provider = signer(1);
    let mut inputs = vec![beacon_event(&provider, &head(&provider, NOW)).unwrap()];
    for (i, byte) in [3_u8, 4, 5].into_iter().enumerate() {
        let key = signer(byte);
        let mut body = stage(&key, NOW);
        body.pylon = format!("v41-stage-{i}");
        inputs.push(beacon_event(&key, &body).unwrap());
    }
    let window = Window {
        from: NOW - (NOW % 60),
        to: NOW - (NOW % 60) + 60,
    };
    let aggregate = compute_aggregate(
        &signer(9).pubkey().to_string(),
        &PoolPolicy::open("everglade", 1),
        window,
        &AggregateInputs {
            beacons: &inputs,
            receipts: &[],
            checks: &[],
        },
        window.to,
    )
    .unwrap();
    assert_eq!(aggregate.totals.pylons_online, 4);
    assert_eq!(aggregate.totals.slots_total, 1);
    assert_eq!(aggregate.totals.slots_free, 1);
}

/// The v1 beacon body exactly as v1 readers declare it, before groups.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct V1Body {
    v: String,
    requires: Vec<String>,
    #[serde(default)]
    meta: Option<Value>,
    provider: String,
    pylon: String,
    label: String,
    status: Status,
    generation: u64,
    since: u64,
    observed_at: u64,
    valid_until: u64,
    class: Class,
    slots: Slots,
    services: Vec<Service>,
    settlement: Vec<String>,
    pools: Vec<String>,
}

/// What a v1 reader did with a fetched event: the v1 marker, then the v1
/// body, then the v1 version.
fn v1_reader(event: &Event) -> Result<V1Body, String> {
    if single_tag(event, "t")? != BEACON_MARKER {
        return Err("wrong `t` marker".into());
    }
    let body: V1Body = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    if body.v != BEACON_V {
        return Err("unknown beacon version".into());
    }
    Ok(body)
}

fn fixture(name: &str) -> Event {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/pylon/fixtures")
        .join(name);
    if std::env::var_os("PYLON_FIXTURES_WRITE").is_some() {
        let provider = signer(1);
        let body = match name {
            "beacon-v1.json" => beacon(&provider, NOW),
            "beacon-v2-head.json" => head(&provider, NOW),
            "beacon-v2-stage.json" => stage(&provider, NOW),
            _ => panic!("no fixture {name}"),
        };
        let event = beacon_event(&provider, &body).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&event).unwrap() + "\n").unwrap();
    }
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
fn a_recorded_v1_beacon_still_parses() {
    let event = fixture("beacon-v1.json");
    let body = parse_beacon(&event).unwrap();
    assert_eq!(body.v, BEACON_V);
    assert!(body.group.is_none());
    assert!(v1_reader(&event).is_ok());
}

#[test]
fn a_v1_reader_refuses_recorded_group_beacons() {
    for name in ["beacon-v2-head.json", "beacon-v2-stage.json"] {
        let event = fixture(name);
        assert!(parse_beacon(&event).unwrap().group.is_some());
        // Never fetched under the v1 marker, and refused if it arrives.
        assert_eq!(v1_reader(&event).unwrap_err(), "wrong `t` marker");
        // Its body refuses under the v1 shape too.
        assert!(serde_json::from_str::<V1Body>(&event.content).is_err());
    }
}

fn runner(provider: &RelaySigner, observed_at: u64) -> Beacon {
    let mut b = beacon(provider, observed_at);
    b.pylon = "kiln-runner".into();
    b.class.family = Family::Cpu;
    b.services[0].capability = format!("{}:{RUNNER_SERVICE}", provider.pubkey());
    b.services[0].model = "cargo".into();
    b.pools = Vec::new();
    b
}

#[test]
fn a_runner_beacon_lives_in_its_own_lane() {
    let key = signer(9);
    let event = runner_beacon_event(&key, &runner(&key, NOW)).unwrap();
    assert_eq!(parse_runner_beacon(&event).unwrap().pylon, "kiln-runner");
    // Readers of model pylons refuse it, and a runner reader refuses a
    // model pylon's beacon.
    assert!(parse_beacon(&event).is_err());
    let model = beacon_event(&key, &beacon(&key, NOW)).unwrap();
    assert!(parse_runner_beacon(&model).is_err());
    // Only cargo tests, in the conversation lane, under the provider's key.
    let mut other = runner(&key, NOW);
    other.services[0].capability = format!("{}:pylon/text-generation", key.pubkey());
    assert!(runner_beacon_event(&key, &other).is_err());
    let mut other = runner(&key, NOW);
    other.services[0].lanes = vec![Lane::CjExecution];
    assert!(runner_beacon_event(&key, &other).is_err());
    assert!(runner_beacon_event(&signer(8), &runner(&key, NOW)).is_err());
}
