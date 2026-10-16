//! RFC 9180 vectors, and local end-to-end fixtures: an SEV-SNP endpoint
//! verified by `att-platforms` (synthetic AMD chain), its HPKE key admitted,
//! a request sealed, carried, opened, and answered in sealed chunks.
use crate::keys::{Admitted, KeyBook, WorkerKeys};
use crate::request::{self, Ask, Identity, ReplayGuard};
use crate::stream::{CHUNK_V, Chunk, Stop};
use crate::{KeyPair, b64, hex};
use hpke::{
    Deserializable as _, OpModeR, OpModeS, Serializable as _, aead::AesGcm256, kdf::HkdfSha256,
};
use nostr::att::{self, Level};
use nostr::domain::RelaySigner;
use p384::pkcs8::DecodePrivateKey as _;
use serde_json::{Value, json};

const NOW: u64 = 1_800_000_000;
const VECTOR: &str = include_str!("../vectors/rfc9180-base-x25519-sha256-aes256gcm.json");

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// Returns fixed bytes: the vector's ephemeral IKM.
struct Fixed(Vec<u8>);
impl hpke::rand_core::RngCore for Fixed {
    fn next_u32(&mut self) -> u32 {
        unimplemented!()
    }
    fn next_u64(&mut self) -> u64 {
        unimplemented!()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.copy_from_slice(&self.0[..dest.len()]);
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), hpke::rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl hpke::rand_core::CryptoRng for Fixed {}

#[test]
fn the_suite_matches_the_rfc_9180_vectors() {
    let v: Value = serde_json::from_str(VECTOR).unwrap();
    let s = |k: &str| unhex(v[k].as_str().unwrap());
    let recipient = KeyPair::derive(&s("ikmR"));
    assert_eq!(recipient.public().to_vec(), s("pkRm"));
    assert_eq!(recipient.private().to_vec(), s("skRm"));
    let pk = <crate::Kem as hpke::Kem>::PublicKey::from_bytes(&s("pkRm")).unwrap();
    let (enc, mut sender) = hpke::setup_sender::<AesGcm256, HkdfSha256, crate::Kem, _>(
        &OpModeS::Base,
        &pk,
        &s("info"),
        &mut Fixed(s("ikmE")),
    )
    .unwrap();
    assert_eq!(enc.to_bytes().to_vec(), s("enc"));
    let sk = <crate::Kem as hpke::Kem>::PrivateKey::from_bytes(&s("skRm")).unwrap();
    let mut receiver = hpke::setup_receiver::<AesGcm256, HkdfSha256, crate::Kem>(
        &OpModeR::Base,
        &sk,
        &enc,
        &s("info"),
    )
    .unwrap();
    for e in v["encryptions"].as_array().unwrap() {
        let h = |k: &str| unhex(e[k].as_str().unwrap());
        assert_eq!(sender.seal(&h("pt"), &h("aad")).unwrap(), h("ct"));
        assert_eq!(receiver.open(&h("ct"), &h("aad")).unwrap(), h("pt"));
    }
    for x in v["exports"].as_array().unwrap() {
        let mut out = vec![0u8; x["L"].as_u64().unwrap() as usize];
        sender
            .export(&unhex(x["exporter_context"].as_str().unwrap()), &mut out)
            .unwrap();
        assert_eq!(out, unhex(x["exported_value"].as_str().unwrap()));
    }
}

// ---- A verified SEV-SNP endpoint with an HPKE key (synthetic AMD chain) ----

const ARK: &str = include_str!("../../att-platforms/vectors/snp/ark.pem");
const ASK: &str = include_str!("../../att-platforms/vectors/snp/ask.pem");
const VCEK: &[u8] = include_bytes!("../../att-platforms/vectors/snp/vcek.der");
const VCEK_KEY: &str = include_str!("../../att-platforms/vectors/snp/vcek_key.pem");
const CHIP: &str = include_str!("../../att-platforms/vectors/snp/chip_id.hex");
const CRL: &[u8] = include_bytes!("../../att-platforms/vectors/snp/crl.der");
const MEASUREMENT: [u8; 48] = [0x31; 48];
const TCB: [u8; 8] = [3, 0, 0, 0, 0, 0, 8, 115];

fn report(report_data: &[u8]) -> Vec<u8> {
    use p384::ecdsa::{SigningKey, signature::Signer};
    let mut r = vec![0u8; 1184];
    r[0..4].copy_from_slice(&2u32.to_le_bytes());
    r[52..56].copy_from_slice(&1u32.to_le_bytes());
    r[8..16].copy_from_slice(&0x30000u64.to_le_bytes());
    r[80..112].copy_from_slice(report_data);
    r[144..192].copy_from_slice(&MEASUREMENT);
    r[384..392].copy_from_slice(&TCB);
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

struct Fixture {
    publisher: RelaySigner,
    endpoint: RelaySigner,
    endpoint_secret: secp256k1::SecretKey,
}

fn fixture() -> Fixture {
    Fixture {
        publisher: RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap(),
        endpoint: RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap(),
        endpoint_secret: secp256k1::SecretKey::from_byte_array([2; 32]).unwrap(),
    }
}

/// Records for an endpoint publishing `hpke`, issued at `issued_at`, and the
/// attested key admitted from them after `att-platforms` verifies them.
fn verified(f: &Fixture, hpke: [u8; 32], issued_at: u64) -> Result<Admitted, String> {
    verified_bound(f, hpke, hpke, issued_at)
}

/// As [`verified`], with the evidence and record bound to `bound` while the
/// record publishes `hpke`.
fn verified_bound(
    f: &Fixture,
    hpke: [u8; 32],
    bound: [u8; 32],
    issued_at: u64,
) -> Result<Admitted, String> {
    let release: att::Release = serde_json::from_value(json!({"v":att::RELEASE_V,"requires":[],"workload":"sealed-fixture",
        "publisher":f.publisher.pubkey(),"image":{"reference":"example.test/sealed","digest":format!("sha256:{}","aa".repeat(32))},
        "platforms":[{"kind":"sev-snp","hwmodel":"","support":""}],"measurements":[{"register":"MEASUREMENT","value":hex(&MEASUREMENT)}],
        "gpu":null,"models":[],"components":[],"source":{"repo":"https://example.test","commit":"ab".repeat(20),"recipe":"build.sh",
        "recipe_digest":format!("sha256:{}","bb".repeat(32))},"rebuilds":[],"transparency":[],"changes":"fixture","published_at":NOW-200_000})).unwrap();
    let release_event = att::release_event(&f.publisher, &release, NOW - 200_000).unwrap();
    let head: att::Head = serde_json::from_value(json!({"v":att::HEAD_V,"requires":[],"workload":"sealed-fixture","generation":1,
        "notice_seconds":86_400,"admitted":[{"release":release_event.id,"effective_at":NOW-100_000,"retire_at":null}],"emergency":null})).unwrap();
    let head_event = att::head_event(&f.publisher, &head, NOW - 100_000).unwrap();
    let binding = att::binding(f.endpoint.pubkey(), Some(&bound), &release_event.id).unwrap();
    let evidence = vec![att::Evidence {
        kind: "sev-snp-report".into(),
        format: "raw".into(),
        token: b64(&report(&unhex(&binding))),
    }];
    let body: att::Endpoint = serde_json::from_value(
        json!({"v":att::ENDPOINT_V,"requires":[],"release":release_event.id,
        "endpoint":f.endpoint.pubkey(),"hpke":b64(&hpke),"binding":binding,"evidence":evidence,
        "operator":"03".repeat(32),"issued_at":issued_at,"valid_until":issued_at+3000}),
    )
    .unwrap();
    let endpoint_event = att::endpoint_event(
        &f.endpoint,
        &body,
        &"04".repeat(32),
        &head.address(f.publisher.pubkey()),
    )?;
    let policy = oa_att::Policy {
        publisher: f.publisher.pubkey().into(),
        workload: "sealed-fixture".into(),
        required: Level::Tee,
        seen_generation: None,
    };
    let records = oa_att::Records {
        release: release_event,
        head: head_event,
        endpoint: endpoint_event,
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
    let checked = att_platforms::verify(&records, &policy, &roots, &collateral, issued_at + 10)?;
    let parsed = oa_att::parse(&records, &policy, issued_at + 10).map_err(|e| e.0)?;
    Admitted::new(&parsed, checked.level)
}

fn identity(key: &Admitted) -> Identity<'_> {
    Identity {
        address: &key.address,
        release: &key.release,
        endpoint: &key.endpoint,
        binding: key.binding,
        max_output_bytes: 4096,
    }
}

const PROMPT: &[u8] = b"sealed prompt: the canary is 7c1f0e";

fn caller() -> (RelaySigner, secp256k1::SecretKey) {
    (
        RelaySigner::from_secret_hex(&"05".repeat(32)).unwrap(),
        secp256k1::SecretKey::from_byte_array([5; 32]).unwrap(),
    )
}

fn ask<'a>(reply_to: &'a str, id: u8, max: u64) -> Ask<'a> {
    Ask {
        request: [id; 32],
        reply_to,
        expires_at: NOW + 300,
        max_output_bytes: max,
    }
}

#[test]
fn a_sealed_request_and_its_streamed_answer_round_trip() {
    let f = fixture();
    let worker_key = KeyPair::derive(b"worker hpke key one");
    let keys = WorkerKeys::new(KeyPair::derive(b"worker hpke key one"));
    let admitted = verified(&f, worker_key.public(), NOW - 60).unwrap();
    assert_eq!(admitted.level, Level::Tee);
    let mut book = KeyBook::default();
    book.admit(admitted.clone(), NOW).unwrap();
    let key = book.current(&admitted.address, NOW).unwrap();
    let (caller, _) = caller();
    let (sealed, mut reader) = request::seal(
        key,
        &ask(caller.pubkey(), 1, 4096),
        PROMPT,
        NOW,
        &mut crate::OsRandom,
    )
    .unwrap();
    let event = request::event(&caller, &sealed, NOW).unwrap();
    // The wire never carries the prompt.
    assert!(!event.content.contains("canary"));
    reader.answering(&event.id);
    let (carried, carrier) = request::unwrap(&event, &f.endpoint_secret).unwrap();
    let mut replay = ReplayGuard::default();
    let mut opened = request::open(
        &carried,
        &carrier,
        &identity(&admitted),
        &keys,
        &mut replay,
        NOW,
    )
    .unwrap();
    assert_eq!(opened.plaintext.as_slice(), PROMPT);
    assert!(!format!("{opened:?}").contains("canary"));
    let mut text = String::new();
    for (i, (part, stop)) in [
        ("Yes", None),
        (", it is", None),
        (" sealed.", Some(Stop::Complete)),
    ]
    .into_iter()
    .enumerate()
    {
        let chunk = opened.stream.chunk(part, stop).unwrap();
        let ev = opened
            .stream
            .event(&f.endpoint, &carrier, &chunk, NOW + i as u64)
            .unwrap();
        let piece = reader.accept(&ev, NOW + 1).unwrap();
        text.push_str(&piece.text);
    }
    assert_eq!(text, "Yes, it is sealed.");
    assert_eq!(reader.finish().unwrap(), Stop::Complete);
    // Nothing follows the last chunk on the worker side either.
    assert!(opened.stream.chunk("more", None).is_err());
    // The same request again is a replay.
    assert!(
        request::open(
            &carried,
            &carrier,
            &identity(&admitted),
            &keys,
            &mut replay,
            NOW + 1
        )
        .unwrap_err()
        .contains("already")
    );
}

#[test]
fn a_gift_wrapped_request_hides_the_caller_and_still_binds_the_answer() {
    let f = fixture();
    let keys = WorkerKeys::new(KeyPair::derive(b"worker hpke key one"));
    let admitted = verified(&f, keys.public(), NOW - 60).unwrap();
    let (caller, caller_secret) = caller();
    let (sealed, mut reader) = request::seal(
        &admitted,
        &ask(caller.pubkey(), 2, 4096),
        PROMPT,
        NOW,
        &mut crate::OsRandom,
    )
    .unwrap();
    let wrapper = secp256k1::SecretKey::from_byte_array([9; 32]).unwrap();
    let wrap =
        request::gift_wrap(&caller_secret, &wrapper, &sealed, NOW, [[7; 32], [8; 32]]).unwrap();
    assert_eq!(wrap.kind, 1_059);
    let wire = serde_json::to_string(&wrap).unwrap();
    assert!(!wire.contains(caller.pubkey()) && !wire.contains("canary"));
    let (carried, carrier) = request::unwrap(&wrap, &f.endpoint_secret).unwrap();
    reader.answering(&carrier);
    let mut opened = request::open(
        &carried,
        &carrier,
        &identity(&admitted),
        &keys,
        &mut ReplayGuard::default(),
        NOW,
    )
    .unwrap();
    let chunk = opened.stream.chunk("ok", Some(Stop::Complete)).unwrap();
    let ev = opened
        .stream
        .event(&f.endpoint, &carrier, &chunk, NOW)
        .unwrap();
    assert_eq!(&*reader.accept(&ev, NOW).unwrap().text, "ok");
    // Someone else's key cannot open the wrap.
    let other = secp256k1::SecretKey::from_byte_array([3; 32]).unwrap();
    assert!(request::unwrap(&wrap, &other).is_err());
}

#[test]
fn requests_bind_endpoint_release_key_expiry_and_limits() {
    let f = fixture();
    let keys = WorkerKeys::new(KeyPair::derive(b"worker hpke key one"));
    let admitted = verified(&f, keys.public(), NOW - 60).unwrap();
    let (caller, _) = caller();
    let me = identity(&admitted);
    let seal = |a: &Ask<'_>| {
        request::seal(&admitted, a, PROMPT, NOW, &mut crate::OsRandom)
            .unwrap()
            .0
    };
    let open = |s: &request::SealedRequest, me: &Identity<'_>, now: u64| {
        request::open(s, "carrier", me, &keys, &mut ReplayGuard::default(), now).map(|_| ())
    };
    let good = seal(&ask(caller.pubkey(), 3, 4096));
    open(&good, &me, NOW).unwrap();
    // Expired.
    assert!(open(&good, &me, NOW + 301).unwrap_err().contains("expired"));
    // Another release or endpoint.
    let other_release = "ff".repeat(32);
    assert!(
        open(
            &good,
            &Identity {
                release: &other_release,
                ..identity(&admitted)
            },
            NOW
        )
        .unwrap_err()
        .contains("release")
    );
    let other_address = format!("{}x", admitted.address);
    assert!(
        open(
            &good,
            &Identity {
                address: &other_address,
                ..identity(&admitted)
            },
            NOW
        )
        .unwrap_err()
        .contains("endpoint")
    );
    // A request for more output than the worker allows.
    assert!(
        open(&seal(&ask(caller.pubkey(), 4, 1 << 20)), &me, NOW)
            .unwrap_err()
            .contains("output")
    );
    // Any change to the authenticated header fails to open.
    let mut tampered = good.clone();
    tampered.header.max_output_bytes = 100;
    assert!(
        open(&tampered, &me, NOW)
            .unwrap_err()
            .contains("does not open")
    );
    // A binding for another release changes the HPKE context.
    let mut wrong_binding = identity(&admitted);
    wrong_binding.binding = [0; 32];
    assert!(
        open(&good, &wrong_binding, NOW)
            .unwrap_err()
            .contains("does not open")
    );
    // An unsigned or mis-authored carrier is refused.
    let mut event = request::event(&caller, &good, NOW).unwrap();
    event.content = event
        .content
        .replace("\"max_output_bytes\":4096", "\"max_output_bytes\":4095");
    assert!(request::unwrap(&event, &f.endpoint_secret).is_err());
    let other = RelaySigner::from_secret_hex(&"06".repeat(32)).unwrap();
    let forged = request::event(&other, &good, NOW).unwrap();
    assert!(
        request::unwrap(&forged, &f.endpoint_secret)
            .unwrap_err()
            .contains("caller")
    );
    // Oversized plaintext and an out-of-range expiry are refused when sealing.
    let big = vec![0u8; request::MAX_REQUEST_BYTES + 1];
    assert!(
        request::seal(
            &admitted,
            &ask(caller.pubkey(), 5, 10),
            &big,
            NOW,
            &mut crate::OsRandom
        )
        .is_err()
    );
    let far = Ask {
        expires_at: NOW + 10_000,
        ..ask(caller.pubkey(), 5, 10)
    };
    assert!(request::seal(&admitted, &far, PROMPT, NOW, &mut crate::OsRandom).is_err());
}

#[test]
fn key_admission_and_rotation_follow_the_attested_records() {
    let f = fixture();
    let one = KeyPair::derive(b"worker hpke key one");
    let two = KeyPair::derive(b"worker hpke key two");
    let first = verified(&f, one.public(), NOW - 600).unwrap();
    let second = verified(&f, two.public(), NOW - 60).unwrap();
    let mut book = KeyBook::default();
    book.admit(first.clone(), NOW).unwrap();
    book.admit(second.clone(), NOW).unwrap();
    assert_eq!(
        book.current(&second.address, NOW).unwrap().hpke,
        two.public()
    );
    // The older record cannot roll the key back, and expired records are refused.
    assert!(
        book.admit(first.clone(), NOW)
            .unwrap_err()
            .contains("older")
    );
    assert!(
        KeyBook::default()
            .admit(first.clone(), NOW + 3000)
            .unwrap_err()
            .contains("expired")
    );
    assert!(
        book.current(&second.address, NOW + 4000)
            .unwrap_err()
            .contains("expired")
    );
    // A record whose binding does not cover the key it publishes is refused
    // by the platform verifier before admission.
    let other = KeyPair::derive(b"other").public();
    assert!(
        verified_bound(&f, one.public(), other, NOW - 60)
            .unwrap_err()
            .contains("binding")
    );
    // The worker rotates: a request sealed to the old key opens until the old
    // record expires, then is refused; a key it never held is refused.
    let mut keys = WorkerKeys::new(KeyPair::derive(b"worker hpke key one"));
    keys.rotate(KeyPair::derive(b"worker hpke key two"), first.valid_until);
    let (caller, _) = caller();
    let old = request::seal(
        &first,
        &ask(caller.pubkey(), 6, 64),
        PROMPT,
        NOW,
        &mut crate::OsRandom,
    )
    .unwrap()
    .0;
    let me = identity(&first);
    request::open(&old, "c", &me, &keys, &mut ReplayGuard::default(), NOW).unwrap();
    let late = first.valid_until;
    let old_late = request::seal(
        &first,
        &Ask {
            expires_at: late + 100,
            ..ask(caller.pubkey(), 7, 64)
        },
        PROMPT,
        late,
        &mut crate::OsRandom,
    )
    .unwrap()
    .0;
    assert!(
        request::open(
            &old_late,
            "c",
            &me,
            &keys,
            &mut ReplayGuard::default(),
            late
        )
        .unwrap_err()
        .contains("retired")
    );
    let stranger = verified(&f, KeyPair::derive(b"someone else").public(), NOW - 60).unwrap();
    let foreign = request::seal(
        &stranger,
        &ask(caller.pubkey(), 8, 64),
        PROMPT,
        NOW,
        &mut crate::OsRandom,
    )
    .unwrap()
    .0;
    assert!(
        request::open(
            &foreign,
            "c",
            &identity(&stranger),
            &keys,
            &mut ReplayGuard::default(),
            NOW
        )
        .unwrap_err()
        .contains("does not hold")
    );
}

/// A started stream: the worker's writer, the client's reader, the carrier.
fn stream(
    f: &Fixture,
    id: u8,
    max: u64,
) -> (request::Opened, crate::stream::ClientStream, Admitted) {
    let keys = WorkerKeys::new(KeyPair::derive(b"worker hpke key one"));
    let admitted = verified(f, keys.public(), NOW - 60).unwrap();
    let (caller, _) = caller();
    let (sealed, mut reader) = request::seal(
        &admitted,
        &ask(caller.pubkey(), id, max),
        PROMPT,
        NOW,
        &mut crate::OsRandom,
    )
    .unwrap();
    let event = request::event(&caller, &sealed, NOW).unwrap();
    reader.answering(&event.id);
    let (carried, carrier) = request::unwrap(&event, &f.endpoint_secret).unwrap();
    let opened = request::open(
        &carried,
        &carrier,
        &identity(&admitted),
        &keys,
        &mut ReplayGuard::default(),
        NOW,
    )
    .unwrap();
    (opened, reader, admitted)
}

#[test]
fn chunks_are_verified_one_by_one_and_in_order() {
    let f = fixture();
    let (mut w, mut r, _) = stream(&f, 10, 64);
    let c: Vec<Chunk> = vec![
        w.stream.chunk("a", None).unwrap(),
        w.stream.chunk("b", None).unwrap(),
        w.stream.chunk("c", Some(Stop::Length)).unwrap(),
    ];
    let ev = |c: &Chunk| w.stream.event(&f.endpoint, &w.carrier, c, NOW).unwrap();
    // Missing: the second chunk is skipped.
    assert!(r.accept(&ev(&c[1]), NOW).unwrap_err().contains("missing"));
    r.accept(&ev(&c[0]), NOW).unwrap();
    // Duplicate.
    assert!(r.accept(&ev(&c[0]), NOW).unwrap_err().contains("repeated"));
    r.accept(&ev(&c[1]), NOW).unwrap();
    // Not yet ended.
    assert!(r.finish().is_err());
    // A chunk signed by someone other than the endpoint.
    let impostor = RelaySigner::from_secret_hex(&"07".repeat(32)).unwrap();
    assert!(
        r.accept(
            &w.stream.event(&impostor, &w.carrier, &c[2], NOW).unwrap(),
            NOW
        )
        .unwrap_err()
        .contains("endpoint")
    );
    // A chunk tagged as answering another request event.
    assert!(
        r.accept(
            &w.stream
                .event(&f.endpoint, &"ee".repeat(32), &c[2], NOW)
                .unwrap(),
            NOW
        )
        .unwrap_err()
        .contains("another request")
    );
    // Flipping the last flag breaks the authentication.
    let mut flipped = c[2].clone();
    flipped.last = false;
    assert!(r.accept(&ev(&flipped), NOW).unwrap_err().contains("key"));
    r.accept(&ev(&c[2]), NOW).unwrap();
    assert_eq!(r.finish().unwrap(), Stop::Length);
    // Nothing after the last chunk.
    let extra = Chunk {
        v: CHUNK_V.into(),
        request: c[2].request.clone(),
        seq: 3,
        last: true,
        ct: c[2].ct.clone(),
    };
    assert!(
        r.accept(&ev(&extra), NOW)
            .unwrap_err()
            .contains("after the last")
    );
}

#[test]
fn foreign_keys_unknown_stops_limits_and_expiry_are_refused() {
    let f = fixture();
    // A chunk from another request's stream, relabeled with this request's ID
    // and carrier, does not open under this request's key.
    let (mut a, mut ra, _) = stream(&f, 20, 64);
    let (mut b, _, _) = stream(&f, 21, 64);
    let mut foreign = b.stream.chunk("x", None).unwrap();
    let mine = a.stream.chunk("y", None).unwrap();
    foreign.request = mine.request.clone();
    let ev = a
        .stream
        .event(&f.endpoint, &a.carrier, &foreign, NOW)
        .unwrap();
    assert!(ra.accept(&ev, NOW).unwrap_err().contains("key"));
    // An unknown stop outcome, properly sealed, still refuses.
    let (w, mut r, _) = stream(&f, 22, 64);
    let ct = w
        .stream
        .seal(0, true, br#"{"text":"","stop":"mystery"}"#)
        .unwrap();
    let chunk = Chunk {
        v: CHUNK_V.into(),
        request: hex(&[22; 32]),
        seq: 0,
        last: true,
        ct: b64(&ct),
    };
    assert!(
        r.accept(
            &w.stream
                .event(&f.endpoint, &w.carrier, &chunk, NOW)
                .unwrap(),
            NOW
        )
        .unwrap_err()
        .contains("unknown stop")
    );
    // A stop outcome without the last flag disagrees.
    let (w, mut r, _) = stream(&f, 23, 64);
    let ct = w
        .stream
        .seal(0, false, br#"{"text":"","stop":"complete"}"#)
        .unwrap();
    let chunk = Chunk {
        v: CHUNK_V.into(),
        request: hex(&[23; 32]),
        seq: 0,
        last: false,
        ct: b64(&ct),
    };
    assert!(
        r.accept(
            &w.stream
                .event(&f.endpoint, &w.carrier, &chunk, NOW)
                .unwrap(),
            NOW
        )
        .unwrap_err()
        .contains("disagree")
    );
    // The worker will not exceed the caller's limit, and the reader refuses a
    // forged chunk that does.
    let (mut w, mut r, _) = stream(&f, 24, 4);
    assert!(w.stream.chunk("12345", None).unwrap_err().contains("limit"));
    let ct = w
        .stream
        .seal(0, false, br#"{"text":"12345","stop":null}"#)
        .unwrap();
    let chunk = Chunk {
        v: CHUNK_V.into(),
        request: hex(&[24; 32]),
        seq: 0,
        last: false,
        ct: b64(&ct),
    };
    assert!(
        r.accept(
            &w.stream
                .event(&f.endpoint, &w.carrier, &chunk, NOW)
                .unwrap(),
            NOW
        )
        .unwrap_err()
        .contains("limit")
    );
    // After the request expires, chunks are refused.
    let (mut w, mut r, _) = stream(&f, 25, 64);
    let chunk = w.stream.chunk("late", Some(Stop::Complete)).unwrap();
    assert!(
        r.accept(
            &w.stream
                .event(&f.endpoint, &w.carrier, &chunk, NOW + 400)
                .unwrap(),
            NOW + 400
        )
        .unwrap_err()
        .contains("expired")
    );
    // Redacted debug output.
    assert!(!format!("{r:?} {w:?}").contains("late"));
}
