//! A real Rekor entry under Sigstore's key, RFC 9162 proofs over generated
//! trees, and local fixtures: a synthetic log, publisher, builders, a
//! person's approval set, and a sealed-job receipt with a fake payment.
use crate::approvals::{ApprovalSet, Decision};
use crate::rekor::{self, Checkpoint, Entry, Hash, LogKey, Witness, leaf_hash, node_hash};
use crate::{Evidence, Policy, check_release, logged_digest, rebuild, receipt};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nostr::att;
use nostr::domain::{Event, RelaySigner, Tag};
use p256::ecdsa::{SigningKey, signature::Signer};
use p256::pkcs8::EncodePublicKey;
use serde_json::json;

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;

// ---- RFC 9162 reference tree (MTH, PATH, PROOF) ----

fn mth(leaves: &[Vec<u8>]) -> Hash {
    match leaves.len() {
        0 => <sha2::Sha256 as sha2::Digest>::digest(b"").into(),
        1 => leaf_hash(&leaves[0]),
        n => {
            let k = split(n);
            node_hash(&mth(&leaves[..k]), &mth(&leaves[k..]))
        }
    }
}

fn split(n: usize) -> usize {
    let mut k = 1;
    while k * 2 < n {
        k *= 2;
    }
    k
}

fn path(m: usize, leaves: &[Vec<u8>]) -> Vec<Hash> {
    let n = leaves.len();
    if n <= 1 {
        return Vec::new();
    }
    let k = split(n);
    if m < k {
        let mut p = path(m, &leaves[..k]);
        p.push(mth(&leaves[k..]));
        p
    } else {
        let mut p = path(m - k, &leaves[k..]);
        p.push(mth(&leaves[..k]));
        p
    }
}

fn subproof(m: usize, leaves: &[Vec<u8>], whole: bool) -> Vec<Hash> {
    let n = leaves.len();
    if m == n {
        return if whole { Vec::new() } else { vec![mth(leaves)] };
    }
    let k = split(n);
    if m <= k {
        let mut p = subproof(m, &leaves[..k], whole);
        p.push(mth(&leaves[k..]));
        p
    } else {
        let mut p = subproof(m - k, &leaves[k..], false);
        p.push(mth(&leaves[..k]));
        p
    }
}

fn leaves(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| format!("leaf {i}").into_bytes()).collect()
}

#[test]
fn inclusion_and_consistency_proofs_follow_rfc_9162() {
    for n in 1..=24 {
        let all = leaves(n);
        let root = mth(&all);
        for m in 0..n {
            let proof = path(m, &all);
            rekor::verify_inclusion(m as u64, n as u64, &leaf_hash(&all[m]), &proof, &root)
                .unwrap();
            assert!(
                rekor::verify_inclusion(m as u64, n as u64, &leaf_hash(b"other"), &proof, &root)
                    .is_err()
            );
            if !proof.is_empty() {
                let mut bad = proof.clone();
                bad[0][0] ^= 1;
                assert!(
                    rekor::verify_inclusion(m as u64, n as u64, &leaf_hash(&all[m]), &bad, &root)
                        .is_err()
                );
            }
        }
        for m in 1..=n {
            let proof = subproof(m, &all, true);
            let old = mth(&all[..m]);
            rekor::verify_consistency(m as u64, n as u64, &old, &root, &proof).unwrap();
            if m < n {
                let forked = mth(&leaves(m)
                    .into_iter()
                    .map(|mut l| {
                        l.push(b'!');
                        l
                    })
                    .collect::<Vec<_>>());
                assert!(
                    rekor::verify_consistency(m as u64, n as u64, &forked, &root, &proof).is_err()
                );
            }
        }
    }
}

#[test]
fn a_real_rekor_entry_verifies_under_the_pinned_sigstore_key() {
    let key = LogKey::sigstore().unwrap();
    let entry: Entry =
        serde_json::from_str(include_str!("../vectors/rekor-entry-3250000000.json")).unwrap();
    let mut witness = Witness::default();
    let included = rekor::verify_entry(&entry, &key, &mut witness, None).unwrap();
    assert_eq!(included.log_index, 3_250_000_000);
    assert_eq!(included.body["kind"], "dsse");
    // Seeing it again is fine; a tampered body, timestamp or checkpoint is not.
    rekor::verify_entry(&entry, &key, &mut witness, None).unwrap();
    let mut body = entry.clone();
    body.body = STANDARD.encode(b"{}");
    assert!(rekor::verify_entry(&body, &key, &mut Witness::default(), None).is_err());
    let mut time = entry.clone();
    time.integrated_time += 1;
    assert!(
        rekor::verify_entry(&time, &key, &mut Witness::default(), None)
            .unwrap_err()
            .contains("timestamp")
    );
    let mut note = entry.clone();
    note.verification.inclusion_proof.checkpoint = note
        .verification
        .inclusion_proof
        .checkpoint
        .replacen("3150433924", "3150433925", 1);
    assert!(rekor::verify_entry(&note, &key, &mut Witness::default(), None).is_err());
    // A different key refuses everything.
    let other = synthetic_log().0;
    assert!(rekor::verify_entry(&entry, &other, &mut Witness::default(), None).is_err());
}

// ---- A synthetic log ----

const LOG_NAME: &str = "synthetic.log";

fn synthetic_log() -> (LogKey, SigningKey) {
    let signing = SigningKey::from_slice(&[0x5a; 32]).unwrap();
    let pem = signing
        .verifying_key()
        .to_public_key_pem(Default::default())
        .unwrap();
    (LogKey::from_pem(LOG_NAME, &pem).unwrap(), signing)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn note(key: &LogKey, signing: &SigningKey, size: usize, root: &Hash) -> String {
    let body = format!("{LOG_NAME} - 1\n{size}\n{}\n", STANDARD.encode(root));
    let sig: p256::ecdsa::Signature = signing.sign(body.as_bytes());
    let mut raw = key.log_id[..4].to_vec();
    raw.extend(sig.to_der().as_bytes());
    format!("{body}\n\u{2014} {LOG_NAME} {}\n", STANDARD.encode(raw))
}

/// An entry for `body` at index 5 of a log of `size` leaves.
fn entry(body: &[u8], size: usize, integrated_time: u64, salt: u8) -> (Entry, Vec<Vec<u8>>) {
    let (key, signing) = synthetic_log();
    let mut all: Vec<Vec<u8>> = (0..size)
        .map(|i| format!("other {i} {salt}").into_bytes())
        .collect();
    all[5] = body.to_vec();
    let root = mth(&all);
    let b64 = STANDARD.encode(body);
    let set = json!({"body": b64, "integratedTime": integrated_time, "logID": hex(&key.log_id), "logIndex": 5});
    let sig: p256::ecdsa::Signature = signing.sign(&serde_json::to_vec(&set).unwrap());
    let entry: Entry = serde_json::from_value(json!({
        "body": b64, "integratedTime": integrated_time, "logID": hex(&key.log_id), "logIndex": 5,
        "verification": {"signedEntryTimestamp": STANDARD.encode(sig.to_der().as_bytes()),
            "inclusionProof": {"checkpoint": note(&key, &signing, size, &root), "hashes": path(5, &all).iter().map(|h| hex(h)).collect::<Vec<_>>(),
                "logIndex": 5, "rootHash": hex(&root), "treeSize": size}}})).unwrap();
    (entry, all)
}

fn hashedrekord(digest: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"apiVersion": "0.0.1", "kind": "hashedrekord",
        "spec": {"data": {"hash": {"algorithm": "sha256", "value": digest}}, "signature": {"content": "", "publicKey": {"content": ""}}}})).unwrap()
}

// ---- Publisher, builders, release, head ----

struct World {
    publisher: RelaySigner,
    builders: [RelaySigner; 2],
    release: att::Release,
    release_event: Event,
    head: att::Head,
    head_event: Event,
}

const IMAGE: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn world(emergency: bool) -> World {
    let publisher = RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
    let builders = [
        RelaySigner::from_secret_hex(&"0b".repeat(32)).unwrap(),
        RelaySigner::from_secret_hex(&"0c".repeat(32)).unwrap(),
    ];
    let release: att::Release = serde_json::from_value(json!({"v": att::RELEASE_V, "requires": [], "workload": "vault",
        "publisher": publisher.pubkey(), "image": {"reference": "example.test/vault", "digest": IMAGE},
        "platforms": [{"kind": "gcp-confidential-space", "hwmodel": "GCP_INTEL_TDX", "support": "STABLE"}], "measurements": [],
        "gpu": null, "models": [], "components": [],
        "source": {"repo": "https://example.test/repo", "commit": "ab".repeat(20), "recipe": "deploy/build.sh", "recipe_digest": format!("sha256:{}", "bb".repeat(32))},
        "rebuilds": [{"builder": builders[0].pubkey(), "digest": IMAGE}, {"builder": builders[1].pubkey(), "digest": IMAGE}],
        "transparency": [{"log": "rekor", "url": "https://synthetic.log", "index": 5}],
        "changes": "Fixture release.", "published_at": NOW - 10 * DAY})).unwrap();
    let release_event = att::release_event(&publisher, &release, NOW - 10 * DAY).unwrap();
    let head: att::Head = serde_json::from_value(json!({"v": att::HEAD_V, "requires": [], "workload": "vault", "generation": 3,
        "notice_seconds": 7 * DAY, "admitted": [{"release": release_event.id, "effective_at": NOW - 2 * DAY, "retire_at": null}],
        "emergency": if emergency { json!({"release": release_event.id, "reason": "fixture emergency"}) } else { json!(null) }})).unwrap();
    let head_event = att::head_event(&publisher, &head, NOW - 2 * DAY).unwrap();
    World {
        publisher,
        builders,
        release,
        release_event,
        head,
        head_event,
    }
}

fn rebuilt(builder: &RelaySigner, w: &World, digest: &str) -> Event {
    rebuild::event(
        builder,
        &rebuild::Rebuild {
            v: rebuild::REBUILD_V.into(),
            requires: vec![],
            workload: "vault".into(),
            source: w.release.source.clone(),
            image_digest: digest.into(),
            components: vec![],
            built_at: NOW - 9 * DAY,
        },
    )
    .unwrap()
}

fn good_entry(w: &World) -> Entry {
    entry(
        &hashedrekord(&logged_digest(&w.release).unwrap()),
        9,
        NOW - 9 * DAY,
        0,
    )
    .0
}

#[test]
fn a_release_with_its_log_entry_rebuilds_and_approval_is_admitted() {
    let w = world(false);
    let (key, _) = synthetic_log();
    let entry = good_entry(&w);
    let rebuilds = [
        rebuilt(&w.builders[0], &w, IMAGE),
        rebuilt(&w.builders[1], &w, IMAGE),
    ];
    let mut set = ApprovalSet::default();
    let policy = Policy {
        publisher: w.publisher.pubkey(),
        workload: "vault",
        log: Some(&key),
        min_rebuilds: 2,
        seen_generation: Some(3),
    };
    let evidence = |set: Option<&ApprovalSet>, rebuilds: &[Event]| -> Result<Decision, String> {
        let ev = Evidence {
            release: &w.release_event,
            head: &w.head_event,
            entry: Some(&entry),
            consistency: None,
            rebuilds,
            approvals: set,
        };
        check_release(&ev, &policy, &mut Witness::default(), NOW)
    };
    assert_eq!(evidence(None, &rebuilds).unwrap(), Decision::Admitted);
    // With the approval set on: awaiting, approved, revoked.
    assert_eq!(
        evidence(Some(&set), &rebuilds).unwrap(),
        Decision::AwaitingApproval { emergency: false }
    );
    set.approve(&w.release_event.id, "vault", NOW - 60);
    assert_eq!(evidence(Some(&set), &rebuilds).unwrap(), Decision::Admitted);
    set.revoke(&w.release_event.id, NOW - 1).unwrap();
    assert_eq!(evidence(Some(&set), &rebuilds).unwrap(), Decision::Revoked);
    // Too few rebuilds, a changed build, an unlisted builder, the publisher as builder.
    assert!(
        evidence(None, &rebuilds[..1])
            .unwrap_err()
            .contains("needs 2")
    );
    let changed = [
        rebuilt(&w.builders[0], &w, &format!("sha256:{}", "cc".repeat(32))),
        rebuilds[1].clone(),
    ];
    assert!(
        evidence(None, &changed)
            .unwrap_err()
            .contains("build changed")
    );
    let stranger = RelaySigner::from_secret_hex(&"0d".repeat(32)).unwrap();
    assert!(
        evidence(None, &[rebuilt(&stranger, &w, IMAGE), rebuilds[1].clone()])
            .unwrap_err()
            .contains("does not list")
    );
    assert!(
        evidence(None, &[rebuilt(&w.publisher, &w, IMAGE)])
            .unwrap_err()
            .contains("publisher")
    );
    // A head rollback.
    let rollback = Policy {
        seen_generation: Some(4),
        ..policy
    };
    let ev = Evidence {
        release: &w.release_event,
        head: &w.head_event,
        entry: Some(&entry),
        consistency: None,
        rebuilds: &rebuilds,
        approvals: None,
    };
    assert!(check_release(&ev, &rollback, &mut Witness::default(), NOW).is_err());
}

#[test]
fn approval_never_shortens_notice_and_emergencies_are_flagged() {
    let w = world(true);
    let mut set = ApprovalSet::default();
    set.approve(&w.release_event.id, "vault", NOW - 3 * DAY);
    // Before the head's effective time, approval does not admit.
    assert!(
        set.decide(&w.head, &w.release_event.id, &w.release, NOW - 3 * DAY)
            .is_err()
    );
    assert_eq!(
        set.decide(&w.head, &w.release_event.id, &w.release, NOW)
            .unwrap(),
        Decision::Admitted
    );
    assert_eq!(
        ApprovalSet::default()
            .decide(&w.head, &w.release_event.id, &w.release, NOW)
            .unwrap(),
        Decision::AwaitingApproval { emergency: true }
    );
    // The set is stored sealed to its owner, and a rolled-back set refuses.
    let me = secp256k1::SecretKey::from_byte_array([0x0e; 32]).unwrap();
    let sealed = set.seal(&me, &"aa".repeat(32), NOW, [3; 32]).unwrap();
    assert!(!sealed.content.contains(&w.release_event.id));
    assert_eq!(ApprovalSet::open(&sealed, &me, Some(1)).unwrap(), set);
    assert!(
        ApprovalSet::open(&sealed, &me, Some(2))
            .unwrap_err()
            .contains("older")
    );
    let other = secp256k1::SecretKey::from_byte_array([0x0f; 32]).unwrap();
    assert!(ApprovalSet::open(&sealed, &other, None).is_err());
}

#[test]
fn missing_stale_forked_and_mismatched_transparency_refuses() {
    let w = world(false);
    let (key, signing) = synthetic_log();
    let policy = Policy {
        publisher: w.publisher.pubkey(),
        workload: "vault",
        log: Some(&key),
        min_rebuilds: 0,
        seen_generation: None,
    };
    let check = |entry: Option<&Entry>, witness: &mut Witness, consistency: Option<&[Hash]>| {
        let ev = Evidence {
            release: &w.release_event,
            head: &w.head_event,
            entry,
            consistency,
            rebuilds: &[],
            approvals: None,
        };
        check_release(&ev, &policy, witness, NOW)
    };
    let digest = logged_digest(&w.release).unwrap();
    let (good, all) = entry(&hashedrekord(&digest), 9, NOW - 9 * DAY, 0);
    // Missing.
    assert!(
        check(None, &mut Witness::default(), None)
            .unwrap_err()
            .contains("no transparency")
    );
    // The witness saw a larger tree: stale.
    let mut bigger = all.clone();
    bigger.extend((0..3).map(|i| format!("later {i}").into_bytes()));
    let big = Checkpoint {
        origin: format!("{LOG_NAME} - 1"),
        size: 12,
        root: STANDARD.encode(mth(&bigger)),
    };
    let mut witness = Witness::default();
    witness.logs.insert(big.origin.clone(), big.clone());
    assert!(
        check(Some(&good), &mut witness, None)
            .unwrap_err()
            .contains("stale")
    );
    // The witness saw the same size with another root: a fork.
    let (forked, _) = entry(&hashedrekord(&digest), 9, NOW - 9 * DAY, 1);
    let mut witness = Witness::default();
    check(Some(&good), &mut witness, None).unwrap();
    assert!(
        check(Some(&forked), &mut witness, None)
            .unwrap_err()
            .contains("forked")
    );
    // Growth needs a consistency proof; with one, the witness advances.
    let mut witness = Witness::default();
    let small: Vec<Vec<u8>> = all[..7].to_vec();
    let seen = Checkpoint {
        origin: format!("{LOG_NAME} - 1"),
        size: 7,
        root: STANDARD.encode(mth(&small)),
    };
    witness.logs.insert(seen.origin.clone(), seen);
    assert!(
        check(Some(&good), &mut witness, None)
            .unwrap_err()
            .contains("consistency")
    );
    let proof = subproof(7, &all, true);
    check(Some(&good), &mut witness, Some(&proof)).unwrap();
    assert_eq!(witness.logs[&format!("{LOG_NAME} - 1")].size, 9);
    // An entry logging other bytes (the release changed after logging).
    let (other, _) = entry(&hashedrekord(&"dd".repeat(32)), 9, NOW - 9 * DAY, 0);
    assert!(
        check(Some(&other), &mut Witness::default(), None)
            .unwrap_err()
            .contains("another release")
    );
    // Logged only after the release took effect.
    let (late, _) = entry(&hashedrekord(&digest), 9, NOW - DAY, 0);
    assert!(
        check(Some(&late), &mut Witness::default(), None)
            .unwrap_err()
            .contains("after it took effect")
    );
    // A checkpoint signed by another key.
    let mut unsigned = good.clone();
    let rogue = SigningKey::from_slice(&[0x77; 32]).unwrap();
    unsigned.verification.inclusion_proof.checkpoint = note(&key, &rogue, 9, &mth(&all));
    assert!(check(Some(&unsigned), &mut Witness::default(), None).is_err());
    let _ = signing;
}

#[test]
fn a_sealed_receipt_names_ciphertext_and_the_attested_endpoint_only() {
    let buyer = RelaySigner::from_secret_hex(&"21".repeat(32)).unwrap();
    let endpoint = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
    let address = format!("30203:{}:{}", endpoint.pubkey(), "44".repeat(32));
    let attested = receipt::Attested {
        endpoint: address.clone(),
        release: "55".repeat(32),
        level: "tee-cloud".into(),
        measurement: IMAGE.into(),
    };
    let prompt = b"my sealed prompt: canary 91f2";
    let answer = b"the sealed answer";
    // The events as they travelled: ciphertext only (stand-ins for NIP-44).
    let request = buyer.sign(
        NOW - 5,
        25_910,
        vec![Tag::new(vec!["p".into(), endpoint.pubkey().into()])],
        STANDARD.encode([7u8; 80]),
    );
    let result = endpoint.sign(
        NOW - 1,
        26_910,
        vec![Tag::new(vec!["e".into(), request.id.clone()])],
        STANDARD.encode([9u8; 60]),
    );
    let base = nostr::pylon::Receipt {
        v: nostr::pylon::RECEIPT_V.into(),
        requires: vec![],
        meta: None,
        buyer: buyer.pubkey().into(),
        provider: endpoint.pubkey().into(),
        pylon: "attested-vault".into(),
        lane: nostr::pylon::Lane::CjConversation,
        capability: format!("{}:pylon/text-generation", endpoint.pubkey()),
        request: request.id.clone(),
        request_digest: "00".repeat(32),
        result_digest: None,
        started_at: NOW - 5,
        finished_at: NOW - 1,
        units: nostr::pylon::Units {
            kind: nostr::pylon::UnitKind::Tokens,
            count: 12,
        },
        outcome: nostr::pylon::Outcome::Accepted,
        payment: Some(nostr::pylon::Payment {
            profile: "lightning-bolt11".into(),
            network: "regtest".into(),
            amount_msat: 21_000,
            settlement: nostr::pylon::settlement_reference("regtest", &"66".repeat(32)),
            acknowledgment: None,
        }),
    };
    let mut sealed = receipt::seal(base, &attested, &request, &result).unwrap();
    nostr::pylon::acknowledge_payment(&endpoint, &mut sealed).unwrap();
    let event = receipt::publish(&buyer, &sealed, NOW).unwrap();
    receipt::verify(&event, &attested, endpoint.pubkey(), &request, &result).unwrap();
    // No plaintext, plaintext digest, or payment hash on the wire.
    receipt::check_private(&event, &[prompt, answer]).unwrap();
    assert!(
        !serde_json::to_string(&event)
            .unwrap()
            .contains(&"66".repeat(32))
    );
    // A receipt built from plaintext digests is caught.
    let mut leaky = sealed.clone();
    leaky.request_digest = nostr::pylon::sha256_hex(prompt);
    leaky.payment.as_mut().unwrap().acknowledgment = None;
    nostr::pylon::acknowledge_payment(&endpoint, &mut leaky).unwrap();
    let leaked = receipt::publish(&buyer, &leaky, NOW).unwrap();
    assert!(
        receipt::check_private(&leaked, &[prompt])
            .unwrap_err()
            .contains("digest")
    );
    assert!(
        receipt::verify(&leaked, &attested, endpoint.pubkey(), &request, &result)
            .unwrap_err()
            .contains("request ciphertext")
    );
    // Another endpoint, release or level than the one verified.
    let other = receipt::Attested {
        endpoint: format!("30203:{}:{}", endpoint.pubkey(), "45".repeat(32)),
        ..attested.clone()
    };
    assert!(
        receipt::verify(&event, &other, endpoint.pubkey(), &request, &result)
            .unwrap_err()
            .contains("another endpoint")
    );
    let lower = receipt::Attested {
        level: "tee".into(),
        ..attested.clone()
    };
    assert!(receipt::verify(&event, &lower, endpoint.pubkey(), &request, &result).is_err());
    // A result not signed by the endpoint key.
    let forged = buyer.sign(NOW - 1, 26_910, vec![], result.content.clone());
    assert!(
        receipt::verify(&event, &attested, endpoint.pubkey(), &request, &forged)
            .unwrap_err()
            .contains("endpoint")
    );
}

#[test]
fn the_cli_verifies_a_release_and_keeps_the_witness() {
    let w = world(false);
    let (key, signing) = synthetic_log();
    let dir = std::env::temp_dir().join(format!("att-release-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let put = |name: &str, value: &dyn erased::Ser| {
        let p = dir.join(name);
        std::fs::write(&p, value.json()).unwrap();
        p.to_string_lossy().to_string()
    };
    let release = put("release.json", &w.release_event);
    let head = put("head.json", &w.head_event);
    let entry_path = put("entry.json", &good_entry(&w));
    let pem = signing
        .verifying_key()
        .to_public_key_pem(Default::default())
        .unwrap();
    let key_path = dir.join("log.pem");
    std::fs::write(&key_path, pem).unwrap();
    let witness = dir.join("witness.json").to_string_lossy().to_string();
    let r0 = put("r0.json", &rebuilt(&w.builders[0], &w, IMAGE));
    let args = |extra: &[&str]| {
        let mut a: Vec<String> = [
            "verify",
            "--release",
            &release,
            "--head",
            &head,
            "--publisher",
            w.publisher.pubkey(),
            "--workload",
            "vault",
            "--now",
            &NOW.to_string(),
            "--rekor",
            &entry_path,
            "--log-key",
            key_path.to_str().unwrap(),
            "--log-name",
            LOG_NAME,
            "--witness",
            &witness,
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        a.extend(extra.iter().map(|s| (*s).to_string()));
        a
    };
    let out = crate_main::run(&args(&["--rebuild", &r0, "--min-rebuilds", "1"])).unwrap();
    assert!(out.starts_with("admitted"));
    assert!(
        std::fs::read_to_string(&witness)
            .unwrap()
            .contains("synthetic.log - 1")
    );
    assert!(
        crate_main::run(&args(&["--min-rebuilds", "1"]))
            .unwrap_err()
            .contains("needs 1")
    );
    let approvals = dir.join("approvals.json").to_string_lossy().to_string();
    assert!(
        crate_main::run(&args(&["--approvals", &approvals]))
            .unwrap_err()
            .contains("awaiting")
    );
    let _ = key;
    std::fs::remove_dir_all(dir).unwrap();
}

mod erased {
    pub trait Ser {
        fn json(&self) -> String;
    }
    impl<T: serde::Serialize> Ser for T {
        fn json(&self) -> String {
            serde_json::to_string(self).unwrap()
        }
    }
}

#[path = "main.rs"]
#[allow(dead_code)]
mod crate_main;
