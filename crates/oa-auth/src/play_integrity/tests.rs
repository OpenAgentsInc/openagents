use super::*;
use serde_json::{Value, json};

fn policy() -> Policy {
    Policy {
        package_name: "org.example.fixture".into(),
        certificate_sha256: vec!["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into()],
        minimum_version_code: 7,
    }
}
fn verdict() -> Value {
    json!({
        "requestDetails": {"requestPackageName": "org.example.fixture", "requestHash": request_hash("challenge", "key"), "timestampMillis": "1000000"},
        "appIntegrity": {"appRecognitionVerdict": "PLAY_RECOGNIZED", "packageName": "org.example.fixture", "certificateSha256Digest": ["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"], "versionCode": "7"},
        "deviceIntegrity": {"deviceRecognitionVerdict": ["MEETS_DEVICE_INTEGRITY"]},
        "accountDetails": {"appLicensingVerdict": "LICENSED"}
    })
}
#[test]
fn accepted_release_is_bound_to_identity_challenge_and_time() {
    let p = policy();
    p.validate().unwrap();
    assert_eq!(
        p.verify(&verdict(), &request_hash("challenge", "key"), 1_000_000),
        Ok(())
    );
    for hash in [
        request_hash("other", "key"),
        request_hash("challenge", "other"),
    ] {
        assert_eq!(
            p.verify(&verdict(), &hash, 1_000_000),
            Err(Refusal::Request)
        );
    }
    for now in [969_999, 1_120_001, u64::MAX] {
        assert_eq!(
            p.verify(&verdict(), &request_hash("challenge", "key"), now),
            Err(Refusal::Expired)
        );
    }
}
#[test]
fn wrong_release_licensing_device_and_replayed_verdicts_are_refused() {
    let p = policy();
    let hash = request_hash("challenge", "key");
    for (pointer, value, why) in [
        (
            "/requestDetails/requestPackageName",
            json!("org.other.app"),
            Refusal::Request,
        ),
        (
            "/appIntegrity/packageName",
            json!("org.other.app"),
            Refusal::App,
        ),
        (
            "/appIntegrity/appRecognitionVerdict",
            json!("UNEVALUATED"),
            Refusal::App,
        ),
        (
            "/appIntegrity/certificateSha256Digest",
            json!([]),
            Refusal::Certificate,
        ),
        (
            "/appIntegrity/certificateSha256Digest",
            json!(["other"]),
            Refusal::Certificate,
        ),
        ("/appIntegrity/versionCode", json!("6"), Refusal::Version),
        ("/appIntegrity/versionCode", json!("no"), Refusal::Version),
        (
            "/deviceIntegrity/deviceRecognitionVerdict",
            json!([]),
            Refusal::Device,
        ),
        (
            "/deviceIntegrity/deviceRecognitionVerdict",
            json!(["MEETS_VIRTUAL_INTEGRITY"]),
            Refusal::Device,
        ),
        (
            "/accountDetails/appLicensingVerdict",
            json!("UNLICENSED"),
            Refusal::License,
        ),
    ] {
        let mut bad = verdict();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert_eq!(p.verify(&bad, &hash, 1_000_000), Err(why), "{pointer}");
    }
    for missing in [
        "requestDetails",
        "appIntegrity",
        "deviceIntegrity",
        "accountDetails",
    ] {
        let mut bad = verdict();
        bad.as_object_mut().unwrap().remove(missing);
        assert_eq!(p.verify(&bad, &hash, 1_000_000), Err(Refusal::Malformed));
    }
}
#[test]
fn configuration_requires_release_pins() {
    let p = policy();
    for bad in [
        Policy {
            certificate_sha256: vec![],
            ..p.clone()
        },
        Policy {
            certificate_sha256: vec!["garbage".into()],
            ..p.clone()
        },
        Policy {
            minimum_version_code: 0,
            ..p.clone()
        },
        Policy {
            package_name: "../path".into(),
            ..p
        },
    ] {
        assert!(bad.validate().is_err());
    }
}
