//! ATT end to end across its owners: a release checked with its rebuild and
//! the person's approval (`att-release`), the endpoint's raw SEV-SNP
//! evidence verified to a pinned AMD chain (`att-platforms`, synthetic
//! roots), its HPKE key admitted and a gift-wrapped request sealed to it
//! (`att-seal`), the answer streamed in sealed chunks, and the public
//! sealed-job receipt over ciphertext (`att-release` + `nostr::pylon`) with
//! a fake regtest payment. Interrupted, disconnected, revoked and
//! competing-controller cases included.
use att_release::approvals::{ApprovalSet, Decision};
use att_seal::KeyPair;
use att_seal::keys::{Admitted, KeyBook, WorkerKeys};
use att_seal::request::{self, Ask, Identity, ReplayGuard};
use att_seal::stream::Stop;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nostr::att::{self, Level};
use nostr::domain::{Event, RelaySigner, Tag};
use p384::pkcs8::DecodePrivateKey as _;
use serde_json::json;

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;
const ARK: &str = include_str!("../../att-platforms/vectors/snp/ark.pem");
const ASK: &str = include_str!("../../att-platforms/vectors/snp/ask.pem");
const VCEK: &[u8] = include_bytes!("../../att-platforms/vectors/snp/vcek.der");
const VCEK_KEY: &str = include_str!("../../att-platforms/vectors/snp/vcek_key.pem");
const CHIP: &str = include_str!("../../att-platforms/vectors/snp/chip_id.hex");
const CRL: &[u8] = include_bytes!("../../att-platforms/vectors/snp/crl.der");
const MEASUREMENT: [u8; 48] = [0x31; 48];
const IMAGE: &str = "sha256:abababababababababababababababababababababababababababababababab";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn report(report_data: &[u8]) -> Vec<u8> {
    use p384::ecdsa::{SigningKey, signature::Signer};
    let mut r = vec![0u8; 1184];
    r[0..4].copy_from_slice(&2u32.to_le_bytes());
    r[52..56].copy_from_slice(&1u32.to_le_bytes());
    r[8..16].copy_from_slice(&0x30000u64.to_le_bytes());
    r[80..112].copy_from_slice(report_data);
    r[144..192].copy_from_slice(&MEASUREMENT);
    r[384..392].copy_from_slice(&[3, 0, 0, 0, 0, 0, 8, 115]);
    r[416..480].copy_from_slice(&unhex(CHIP.trim()));
    let key = SigningKey::from(p384::SecretKey::from_pkcs8_pem(VCEK_KEY).unwrap());
    let sig: p384::ecdsa::Signature = key.sign(&r[..0x2A0]);
    let bytes = sig.to_bytes();
    for i in 0..48 {
        r[0x2A0 + i] = bytes[47 - i];
        r[0x2A0 + 72 + i] = bytes[95 - i];
    }
    r
}

struct World {
    publisher: RelaySigner,
    builder: RelaySigner,
    endpoint: RelaySigner,
    endpoint_secret: secp256k1::SecretKey,
    release_event: Event,
    head_event: Event,
    release: att::Release,
    head: att::Head,
}

fn world() -> World {
    let publisher = RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
    let builder = RelaySigner::from_secret_hex(&"0b".repeat(32)).unwrap();
    let release: att::Release = serde_json::from_value(json!({"v": att::RELEASE_V, "requires": [], "workload": "sealed-chat",
        "publisher": publisher.pubkey(), "image": {"reference": "example.test/sealed-chat", "digest": IMAGE},
        "platforms": [{"kind": "sev-snp", "hwmodel": "", "support": ""}],
        "measurements": [{"register": "MEASUREMENT", "value": hex(&MEASUREMENT)}],
        "gpu": null, "models": [{"id": "clef-small", "digest": format!("sha256:{}", "cd".repeat(32))}], "components": [],
        "source": {"repo": "https://example.test/repo", "commit": "ab".repeat(20), "recipe": "deploy/att/build.sh", "recipe_digest": format!("sha256:{}", "bb".repeat(32))},
        "rebuilds": [{"builder": builder.pubkey(), "digest": IMAGE}], "transparency": [],
        "changes": "Fixture release.", "published_at": NOW - 10 * DAY})).unwrap();
    let release_event = att::release_event(&publisher, &release, NOW - 10 * DAY).unwrap();
    let head: att::Head = serde_json::from_value(json!({"v": att::HEAD_V, "requires": [], "workload": "sealed-chat", "generation": 2,
        "notice_seconds": 7 * DAY, "admitted": [{"release": release_event.id, "effective_at": NOW - 2 * DAY, "retire_at": null}], "emergency": null})).unwrap();
    let head_event = att::head_event(&publisher, &head, NOW - 2 * DAY).unwrap();
    World {
        publisher,
        builder,
        endpoint: RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap(),
        endpoint_secret: secp256k1::SecretKey::from_byte_array([2; 32]).unwrap(),
        release_event,
        head_event,
        release,
        head,
    }
}

/// The endpoint's record with `hpke`, verified by `att-platforms`, then
/// its key admitted.
fn endpoint(w: &World, hpke: [u8; 32], issued_at: u64, instance: &str) -> Admitted {
    let binding = att::binding(w.endpoint.pubkey(), Some(&hpke), &w.release_event.id).unwrap();
    let body: att::Endpoint = serde_json::from_value(json!({"v": att::ENDPOINT_V, "requires": [], "release": w.release_event.id,
        "endpoint": w.endpoint.pubkey(), "hpke": STANDARD.encode(hpke), "binding": binding,
        "evidence": [{"kind": "sev-snp-report", "format": "raw", "token": STANDARD.encode(report(&unhex(&binding)))}],
        "operator": "03".repeat(32), "issued_at": issued_at, "valid_until": issued_at + 3000})).unwrap();
    let event = att::endpoint_event(
        &w.endpoint,
        &body,
        instance,
        &w.head.address(w.publisher.pubkey()),
    )
    .unwrap();
    let policy = oa_att::Policy {
        publisher: w.publisher.pubkey().into(),
        workload: "sealed-chat".into(),
        required: Level::Tee,
        seen_generation: Some(2),
    };
    let records = oa_att::Records {
        release: w.release_event.clone(),
        head: w.head_event.clone(),
        endpoint: event,
        beacon: None,
    };
    let ark = att_platforms::pki::pem_certs(ARK).unwrap().remove(0);
    let roots = att_platforms::Roots {
        intel: vec![],
        amd: vec![att_platforms::pki::fingerprint(&ark)],
        nvidia: vec![],
    };
    let collateral = att_platforms::Collateral {
        tdx: None,
        snp: Some(att_platforms::snp::Collateral {
            vcek: VCEK,
            ask: ASK,
            ark: ARK,
            crl: CRL,
        }),
        nras_jwks: None,
    };
    let verified =
        att_platforms::verify(&records, &policy, &roots, &collateral, issued_at + 10).unwrap();
    assert_eq!(verified.level, Level::Tee);
    Admitted::new(
        &oa_att::parse(&records, &policy, issued_at + 10).unwrap(),
        verified.level,
    )
    .unwrap()
}

fn release_decision(w: &World, approvals: &ApprovalSet) -> Result<Decision, String> {
    let rebuild = att_release::rebuild::event(
        &w.builder,
        &att_release::rebuild::Rebuild {
            v: att_release::rebuild::REBUILD_V.into(),
            requires: vec![],
            workload: "sealed-chat".into(),
            source: w.release.source.clone(),
            image_digest: IMAGE.into(),
            components: vec![],
            built_at: NOW - 9 * DAY,
        },
    )?;
    let evidence = att_release::Evidence {
        release: &w.release_event,
        head: &w.head_event,
        entry: None,
        consistency: None,
        rebuilds: &[rebuild],
        approvals: Some(approvals),
    };
    let policy = att_release::Policy {
        publisher: w.publisher.pubkey(),
        workload: "sealed-chat",
        log: None,
        min_rebuilds: 1,
        seen_generation: Some(2),
    };
    att_release::check_release(
        &evidence,
        &policy,
        &mut att_release::rekor::Witness::default(),
        NOW,
    )
}

const PROMPT: &[u8] = b"private question with canary 3f9a";

#[test]
fn an_approved_attested_release_answers_a_sealed_request_with_a_public_receipt() {
    let w = world();
    // The person approves the release; only then does the client seal to it.
    let mut approvals = ApprovalSet::default();
    assert_eq!(
        release_decision(&w, &approvals).unwrap(),
        Decision::AwaitingApproval { emergency: false }
    );
    approvals.approve(&w.release_event.id, "sealed-chat", NOW - 100);
    assert_eq!(
        release_decision(&w, &approvals).unwrap(),
        Decision::Admitted
    );
    let worker_key = KeyPair::derive(b"endpoint hpke one");
    let admitted = endpoint(&w, worker_key.public(), NOW - 60, &"04".repeat(32));
    let mut book = KeyBook::default();
    book.admit(admitted.clone(), NOW).unwrap();

    // The client gift-wraps the sealed request; the worker opens it.
    let caller = RelaySigner::from_secret_hex(&"05".repeat(32)).unwrap();
    let caller_secret = secp256k1::SecretKey::from_byte_array([5; 32]).unwrap();
    let ask = Ask {
        request: [7; 32],
        reply_to: caller.pubkey(),
        expires_at: NOW + 300,
        max_output_bytes: 1024,
    };
    let (sealed, mut reader) =
        request::seal(&admitted, &ask, PROMPT, NOW, &mut att_seal::OsRandom).unwrap();
    let wrap = request::gift_wrap(
        &caller_secret,
        &secp256k1::SecretKey::from_byte_array([9; 32]).unwrap(),
        &sealed,
        NOW,
        [[1; 32], [2; 32]],
    )
    .unwrap();
    assert!(
        !serde_json::to_string(&wrap)
            .unwrap()
            .contains(caller.pubkey())
    );
    let keys = WorkerKeys::new(KeyPair::derive(b"endpoint hpke one"));
    let me = Identity {
        address: &admitted.address,
        release: &admitted.release,
        endpoint: &admitted.endpoint,
        binding: admitted.binding,
        max_output_bytes: 4096,
    };
    let (carried, carrier) = request::unwrap(&wrap, &w.endpoint_secret).unwrap();
    reader.answering(&carrier);
    let mut guard = ReplayGuard::default();
    let mut opened = request::open(&carried, &carrier, &me, &keys, &mut guard, NOW).unwrap();
    assert_eq!(opened.plaintext.as_slice(), PROMPT);

    // Interrupted: the worker restarts with its persisted replay guard and
    // refuses to answer the same request twice.
    let stored = serde_json::to_string(&guard).unwrap();
    let mut restored: ReplayGuard = serde_json::from_str(&stored).unwrap();
    assert!(
        request::open(&carried, &carrier, &me, &keys, &mut restored, NOW + 5)
            .unwrap_err()
            .contains("already")
    );

    // The answer streams in sealed chunks.
    let mut answer = String::new();
    let mut chunk_events = Vec::new();
    for (i, (text, stop)) in [("Yes.", None), (" Sealed.", Some(Stop::Complete))]
        .into_iter()
        .enumerate()
    {
        let chunk = opened.stream.chunk(text, stop).unwrap();
        let event = opened
            .stream
            .event(&w.endpoint, &carrier, &chunk, NOW + 1 + i as u64)
            .unwrap();
        answer.push_str(&reader.accept(&event, NOW + 2).unwrap().text);
        chunk_events.push(event);
    }
    assert_eq!(reader.finish().unwrap(), Stop::Complete);
    assert_eq!(answer, "Yes. Sealed.");

    // The result the receipt references is the last chunk's ciphertext event.
    let result = chunk_events.last().unwrap();
    let attested = att_release::receipt::Attested {
        endpoint: admitted.address.clone(),
        release: admitted.release.clone(),
        level: "tee".into(),
        measurement: hex(&MEASUREMENT),
    };
    let base = nostr::pylon::Receipt {
        v: nostr::pylon::RECEIPT_V.into(),
        requires: vec![],
        meta: None,
        buyer: caller.pubkey().into(),
        provider: w.endpoint.pubkey().into(),
        pylon: "sealed-chat".into(),
        lane: nostr::pylon::Lane::CjConversation,
        capability: format!("{}:pylon/text-generation", w.endpoint.pubkey()),
        request: carrier.clone(),
        request_digest: "00".repeat(32),
        result_digest: None,
        started_at: NOW,
        finished_at: NOW + 3,
        units: nostr::pylon::Units {
            kind: nostr::pylon::UnitKind::Tokens,
            count: 4,
        },
        outcome: nostr::pylon::Outcome::Accepted,
        payment: Some(nostr::pylon::Payment {
            profile: "lightning-bolt11".into(),
            network: "regtest".into(),
            amount_msat: 2_000,
            settlement: nostr::pylon::settlement_reference("regtest", &"66".repeat(32)),
            acknowledgment: None,
        }),
    };
    // The receipt references the carried request (the rumor): its digest is
    // over the request ciphertext, never the prompt.
    let request_event = Event {
        id: carrier.clone(),
        pubkey: caller.pubkey().into(),
        created_at: NOW,
        kind: request::REQUEST_KIND,
        tags: vec![Tag::new(vec!["p".into(), w.endpoint.pubkey().into()])],
        content: serde_json::to_string(&carried).unwrap(),
        sig: "00".repeat(64),
    };
    let mut receipt = att_release::receipt::seal(base, &attested, &request_event, result).unwrap();
    nostr::pylon::acknowledge_payment(&w.endpoint, &mut receipt).unwrap();
    let published = att_release::receipt::publish(&caller, &receipt, NOW + 4).unwrap();
    att_release::receipt::verify(
        &published,
        &attested,
        w.endpoint.pubkey(),
        &request_event,
        result,
    )
    .unwrap();
    att_release::receipt::check_private(&published, &[PROMPT, answer.as_bytes()]).unwrap();
}

#[test]
fn disconnection_revocation_and_competing_endpoints_refuse() {
    let w = world();
    let mut approvals = ApprovalSet::default();
    approvals.approve(&w.release_event.id, "sealed-chat", NOW - 100);
    // Revoked: the person's revocation stops new sealing at once.
    approvals.revoke(&w.release_event.id, NOW - 10).unwrap();
    assert_eq!(release_decision(&w, &approvals).unwrap(), Decision::Revoked);

    // Competing controllers of one endpoint address: an older record cannot
    // displace the current key, and two keys at one issue time conflict.
    let one = KeyPair::derive(b"endpoint hpke one");
    let two = KeyPair::derive(b"endpoint hpke two");
    let instance = "04".repeat(32);
    let current = endpoint(&w, two.public(), NOW - 60, &instance);
    let older = endpoint(&w, one.public(), NOW - 600, &instance);
    let rival = endpoint(&w, one.public(), NOW - 60, &instance);
    let mut book = KeyBook::default();
    book.admit(current.clone(), NOW).unwrap();
    assert!(book.admit(older, NOW).unwrap_err().contains("older"));
    assert!(
        book.admit(rival, NOW)
            .unwrap_err()
            .contains("same issue time")
    );

    // Disconnected: a lost chunk leaves the answer incomplete; the stream
    // never ends and no receipt can name a result.
    let caller = RelaySigner::from_secret_hex(&"05".repeat(32)).unwrap();
    let ask = Ask {
        request: [8; 32],
        reply_to: caller.pubkey(),
        expires_at: NOW + 300,
        max_output_bytes: 1024,
    };
    let (sealed, mut reader) =
        request::seal(&current, &ask, PROMPT, NOW, &mut att_seal::OsRandom).unwrap();
    let event = request::event(&caller, &sealed, NOW).unwrap();
    reader.answering(&event.id);
    let keys = WorkerKeys::new(KeyPair::derive(b"endpoint hpke two"));
    let me = Identity {
        address: &current.address,
        release: &current.release,
        endpoint: &current.endpoint,
        binding: current.binding,
        max_output_bytes: 4096,
    };
    let (carried, carrier) = request::unwrap(&event, &w.endpoint_secret).unwrap();
    let mut opened = request::open(
        &carried,
        &carrier,
        &me,
        &keys,
        &mut ReplayGuard::default(),
        NOW,
    )
    .unwrap();
    let first = opened.stream.chunk("part one", None).unwrap();
    let _lost = opened
        .stream
        .event(&w.endpoint, &carrier, &first, NOW + 1)
        .unwrap();
    let second = opened
        .stream
        .chunk(" part two", Some(Stop::Complete))
        .unwrap();
    let arrived = opened
        .stream
        .event(&w.endpoint, &carrier, &second, NOW + 2)
        .unwrap();
    assert!(
        reader
            .accept(&arrived, NOW + 3)
            .unwrap_err()
            .contains("missing")
    );
    assert!(reader.finish().is_err());
}
