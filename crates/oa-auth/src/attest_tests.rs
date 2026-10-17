use super::*;

/// A real device's attestation and assertion (see the fixture's `source`).
struct Fixture {
    app_id: String,
    at: i64,
    key_id: Vec<u8>,
    attestation: Vec<u8>,
    attestation_hash: [u8; 32],
    public_key: Vec<u8>,
    assertion: Vec<u8>,
    assertion_hash: [u8; 32],
}

fn fixture() -> Fixture {
    let text = include_str!("../fixtures/app-attest/device.json");
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    let bytes = |name: &str| decode_base64(value[name].as_str().unwrap()).unwrap();
    let public_key = value["public_key_hex"].as_str().unwrap();
    Fixture {
        app_id: value["app_id"].as_str().unwrap().to_owned(),
        at: value["verified_at_unix"].as_i64().unwrap(),
        key_id: bytes("key_id"),
        attestation: bytes("attestation_object"),
        attestation_hash: client_data_hash(&bytes("attestation_client_data")),
        public_key: (0..public_key.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&public_key[i..i + 2], 16).unwrap())
            .collect(),
        assertion: bytes("assertion"),
        assertion_hash: client_data_hash(&bytes("assertion_client_data")),
    }
}

#[test]
fn a_real_attestation_verifies_against_apples_root() {
    let f = fixture();
    let attested = Verifier::apple(&f.app_id, true)
        .attestation(&f.attestation, &f.key_id, &f.attestation_hash, f.at)
        .unwrap();
    assert_eq!(attested.public_key, f.public_key);
    assert_eq!(attested.key_id, f.key_id);
    // The fixture comes from a development build.
    assert_eq!(attested.environment, Environment::Development);
    assert!(!attested.receipt.is_empty());
}

#[test]
fn a_development_key_is_refused_unless_allowed() {
    let f = fixture();
    let refused = Verifier::apple(&f.app_id, false)
        .attestation(&f.attestation, &f.key_id, &f.attestation_hash, f.at)
        .unwrap_err();
    assert_eq!(refused, AttestError::Environment);
}

#[test]
fn an_attestation_for_another_app_is_refused() {
    let f = fixture();
    let refused = Verifier::apple(APP_ID, true)
        .attestation(&f.attestation, &f.key_id, &f.attestation_hash, f.at)
        .unwrap_err();
    assert_eq!(refused, AttestError::AppId);
}

#[test]
fn an_attestation_out_of_date_is_refused() {
    let f = fixture();
    let refused = Verifier::apple(&f.app_id, true)
        .attestation(
            &f.attestation,
            &f.key_id,
            &f.attestation_hash,
            1_792_000_000,
        )
        .unwrap_err();
    assert_eq!(refused, AttestError::Expired);
}

#[test]
fn an_attestation_over_another_challenge_is_refused() {
    let f = fixture();
    let refused = Verifier::apple(&f.app_id, true)
        .attestation(
            &f.attestation,
            &f.key_id,
            &client_data_hash(b"another challenge"),
            f.at,
        )
        .unwrap_err();
    assert_eq!(refused, AttestError::Nonce);
}

#[test]
fn an_attestation_naming_another_key_is_refused() {
    let f = fixture();
    let mut key_id = f.key_id.clone();
    key_id[0] ^= 1;
    let refused = Verifier::apple(&f.app_id, true)
        .attestation(&f.attestation, &key_id, &f.attestation_hash, f.at)
        .unwrap_err();
    assert_eq!(refused, AttestError::KeyId);
}

#[test]
fn an_attestation_under_an_untrusted_root_is_refused() {
    let f = fixture();
    // Trust only the attestation's own intermediate's sibling: nothing.
    let refused = Verifier::with_roots(&f.app_id, Vec::new(), false)
        .attestation(&f.attestation, &f.key_id, &f.attestation_hash, f.at)
        .unwrap_err();
    assert_eq!(refused, AttestError::Chain);
}

#[test]
fn a_damaged_attestation_is_refused() {
    let f = fixture();
    let verifier = Verifier::apple(&f.app_id, true);
    // Flip a byte in the credential certificate, the intermediate, and the
    // authenticator data at the end: every change is refused. (Apple's
    // receipt, between them, isn't checked here.)
    let len = f.attestation.len();
    for at in [100, 400, 900, len - 1, len - 20, len - 100] {
        let mut damaged = f.attestation.clone();
        damaged[at] ^= 0x01;
        assert!(
            verifier
                .attestation(&damaged, &f.key_id, &f.attestation_hash, f.at)
                .is_err(),
            "byte {at}"
        );
    }
    assert!(
        verifier
            .attestation(
                &f.attestation[..f.attestation.len() - 1],
                &f.key_id,
                &f.attestation_hash,
                f.at
            )
            .is_err()
    );
}

#[test]
fn a_real_assertion_verifies_and_advances_the_counter() {
    let f = fixture();
    let verifier = Verifier::apple(&f.app_id, true);
    let counter = verifier
        .assertion(&f.assertion, &f.assertion_hash, &f.public_key, 0)
        .unwrap();
    assert_eq!(counter, 3);
    // A replay, or one that doesn't move the counter forward, is refused.
    assert_eq!(
        verifier
            .assertion(&f.assertion, &f.assertion_hash, &f.public_key, 3)
            .unwrap_err(),
        AttestError::Counter
    );
}

#[test]
fn an_assertion_over_other_data_or_for_another_app_is_refused() {
    let f = fixture();
    assert_eq!(
        Verifier::apple(&f.app_id, true)
            .assertion(&f.assertion, &client_data_hash(b"x"), &f.public_key, 0)
            .unwrap_err(),
        AttestError::Signature
    );
    assert_eq!(
        Verifier::apple(APP_ID, true)
            .assertion(&f.assertion, &f.assertion_hash, &f.public_key, 0)
            .unwrap_err(),
        AttestError::AppId
    );
}

#[test]
fn the_book_binds_one_install_to_one_account() {
    let f = fixture();
    let attested = Verifier::apple(&f.app_id, true)
        .attestation(&f.attestation, &f.key_id, &f.attestation_hash, f.at)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut book = Book::open(dir.path()).unwrap();
    assert!(!book.is_attested("acct_a"));
    book.bind(&attested, "acct_a", 10).unwrap();
    book.bind(&attested, "acct_a", 11).unwrap();
    assert_eq!(
        book.bind(&attested, "acct_b", 12).unwrap_err(),
        BookRefusal::TakenByOther
    );
    book.advance(&f.key_id, 3).unwrap();
    assert_eq!(
        book.advance(b"unknown", 1).unwrap_err(),
        BookRefusal::Unknown
    );

    let book = Book::open(dir.path()).unwrap();
    assert!(book.is_attested("acct_a"));
    assert!(!book.is_attested("acct_b"));
    let record = book.key(&f.key_id).unwrap();
    assert_eq!(record.counter, 3);
    assert_eq!(record.attested_at, 10);
    assert_eq!(record.environment, Environment::Development);
}

#[test]
fn base64_reads_every_common_spelling() {
    for text in ["+/8=", "+/8", "-_8=", "-_8"] {
        assert_eq!(decode_base64(text).unwrap(), vec![0xfb, 0xff]);
    }
    assert!(decode_base64("not base64!").is_none());
}

#[test]
fn the_pinned_root_parses_as_a_ca() {
    let der = pem_der(APPLE_ROOT_PEM).unwrap();
    let cert = parse_cert(&der).unwrap();
    assert!(cert.is_ca());
    assert_eq!(cert.issuer().as_raw(), cert.subject().as_raw());
}
