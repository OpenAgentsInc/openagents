use base64::Engine as _;
use playtest::testflight::{Image, Source};
use ring::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair as _,
    UnparsedPublicKey,
};

use super::*;

fn test_key() -> (Credentials, Vec<u8>) {
    let random = ring::rand::SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &random).unwrap();
    let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &random)
        .unwrap();
    let body = base64::engine::general_purpose::STANDARD.encode(pkcs8.as_ref());
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");
    (
        Credentials {
            key_id: "TESTKEY123".into(),
            issuer: "00000000-0000-0000-0000-000000000000".into(),
            private_key: pem,
        },
        pair.public_key().as_ref().to_vec(),
    )
}

fn part(token: &str, index: usize) -> Value {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token.split('.').nth(index).unwrap())
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn the_token_is_an_es256_jwt_for_app_store_connect() {
    let (credentials, public) = test_key();
    let jwt = token(&credentials, 1_790_000_000).unwrap();
    assert_eq!(
        part(&jwt, 0),
        json!({"alg": "ES256", "kid": "TESTKEY123", "typ": "JWT"})
    );
    let claims = part(&jwt, 1);
    assert_eq!(claims["aud"], "appstoreconnect-v1");
    assert_eq!(claims["iss"], credentials.issuer);
    assert_eq!(claims["exp"], 1_790_000_000 + 1_200);
    let (signing, signature) = jwt.rsplit_once('.').unwrap();
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(signature)
        .unwrap();
    assert_eq!(signature.len(), 64, "JWS ES256 is r || s");
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public)
        .verify(signing.as_bytes(), &signature)
        .expect("the signature verifies");
    // The key never shows in debug output.
    assert!(!format!("{credentials:?}").contains("PRIVATE KEY"));
    let broken = Credentials {
        private_key: "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----".into(),
        ..credentials
    };
    assert!(token(&broken, 0).is_err());
}

#[test]
fn credentials_come_from_an_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let (key, _) = test_key();
    let p8 = dir.path().join("AuthKey_TEST.p8");
    std::fs::write(&p8, &key.private_key).unwrap();
    let env = dir.path().join("asc.env");
    std::fs::write(
        &env,
        format!(
            "# App Store Connect\nASC_API_KEY_ID=TESTKEY123\nASC_API_ISSUER_ID=\"issuer-1\"\nASC_API_PRIVATE_KEY_PATH={}\nOTHER=x\n",
            p8.display()
        ),
    )
    .unwrap();
    let read = credentials(Some(&env)).unwrap();
    assert_eq!(read.key_id, "TESTKEY123");
    assert_eq!(read.issuer, "issuer-1");
    assert!(token(&read, 5).is_ok());
}

fn feedback(id: &str, source: Source) -> Feedback {
    Feedback {
        id: id.into(),
        source,
        created: "2026-09-28T14:40:18.809Z".into(),
        comment: Some("The door didn't open.".into()),
        device: "iPhone18_2".into(),
        os_version: "26.6.2".into(),
        build: Some("16".into()),
        build_id: Some("b".into()),
        app_version: Some("1.0.0".into()),
        images: if source == Source::Screenshot {
            vec![Image {
                url: "https://tf-feedback.example/1.jpg?Signature=secret".into(),
                width: 10,
                height: 20,
            }]
        } else {
            vec![]
        },
    }
}

#[test]
fn new_submissions_are_drafted_once_with_their_attachments() {
    let home = tempfile::tempdir().unwrap();
    let items = [
        feedback("sub-1", Source::Screenshot),
        feedback("sub-2", Source::Crash),
    ];
    let mut fetch = |what: &str| -> Result<Vec<u8>, String> {
        Ok(if what.starts_with("crash:") {
            b"Incident Identifier: X".to_vec()
        } else {
            b"\xff\xd8jpeg".to_vec()
        })
    };
    let first = ingest(home.path(), &items, 100, &mut fetch).unwrap();
    assert_eq!(first.new.len(), 2);
    let dir = drafts(home.path());
    let screenshot = &first.new[0];
    assert!(dir.join(format!("{screenshot}.jpg")).exists());
    assert!(dir.join(format!("{}.crash.txt", first.new[1])).exists());
    let draft = std::fs::read_to_string(dir.join(format!("{screenshot}.md"))).unwrap();
    assert!(!draft.contains("door"));
    // The private record keeps the comment but not the signed image link.
    let private =
        std::fs::read_to_string(dir.join(format!("{screenshot}.testflight.json"))).unwrap();
    assert!(private.contains("door") && !private.contains("Signature"));
    let log = load(home.path()).unwrap();
    assert_eq!(log.pending().len(), 2);
    // A second read finds them in the log.
    let again = ingest(home.path(), &items, 200, &mut fetch).unwrap();
    assert!(again.new.is_empty());
    assert_eq!(again.repeats, 2);
    // A failed download is counted, and the entry is still drafted.
    let mut failing = |_: &str| -> Result<Vec<u8>, String> { Err("expired".into()) };
    let third = ingest(
        home.path(),
        &[feedback("sub-3", Source::Screenshot)],
        300,
        &mut failing,
    )
    .unwrap();
    assert_eq!((third.new.len(), third.missing), (1, 1));
}
