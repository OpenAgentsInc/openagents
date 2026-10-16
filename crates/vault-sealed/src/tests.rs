use crate::{
    Signed,
    client::{Client, VerifiedEndpoint},
    kms::LocalKms,
    workload::{SizeWork, Workload},
};
use oa_vault::{Vmk, sealed::KeyPair};
use serde_json::json;
use std::collections::BTreeMap;

const VAULT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OBJ_A: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const OBJ_B: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const SLOT: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const RELEASE: &str = "5555555555555555555555555555555555555555555555555555555555555555";
const RUN: &str = "6666666666666666666666666666666666666666666666666666666666666666";

fn secret(n: u8) -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([n; 32]).unwrap()
}

struct Fx {
    workload: Workload,
    endpoint: VerifiedEndpoint,
    client: Client,
    enrolled: BTreeMap<String, String>,
    now: u64,
}

fn fx(egress: bool) -> Fx {
    let now = 1_800_000_000;
    let session = KeyPair::derive(&[0x71; 32]);
    let event = [0x72; 32];
    let mut workload = Workload::new(
        secret(1),
        session,
        event,
        RELEASE,
        Box::new(LocalKms::new([0x73; 32], RELEASE)),
        Box::new(SizeWork),
        egress,
    );
    let endpoint = VerifiedEndpoint::assume_for_fixture(
        &workload.endpoint_pubkey(),
        workload.session_public(),
        event,
        RELEASE,
        now + 3600,
    );
    let mut client = Client::new(Vmk::from_bytes([0x74; 32]), SLOT, secret(2));
    let cert = workload.system_key(VAULT).unwrap();
    client.accept_system_key(&endpoint, &cert, now).unwrap();
    let enrolled = BTreeMap::from([(SLOT.to_string(), client.slot_pubkey())]);
    Fx {
        workload,
        endpoint,
        client,
        enrolled,
        now,
    }
}

fn read() -> serde_json::Value {
    json!({"read":true,"write_derived":false,"export":false})
}

#[test]
fn a_sealed_object_opens_only_inside_an_admitted_session() {
    let mut fx = fx(false);
    let a = fx
        .client
        .seal_object(VAULT, OBJ_A, b"lab results", true, fx.now)
        .unwrap();
    // The device copy still opens on the person's own device.
    assert_eq!(
        fx.client.open_on_device(&a).unwrap().as_slice(),
        b"lab results"
    );
    let lease = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, pending) = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    let objects = vec![(OBJ_A.to_string(), a.clone())];
    let replies = fx
        .workload
        .serve(&enc, &request, &objects, &fx.enrolled, fx.now)
        .unwrap();
    let answers = fx.client.open_replies(&pending, &replies).unwrap();
    let answer: serde_json::Value = serde_json::from_slice(&answers[0]).unwrap();
    assert_eq!(answer["bytes"], json!([11]));
    // A replayed request after its one use is refused.
    assert!(
        fx.workload
            .serve(&enc, &request, &objects, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("conflict")
    );
    // The audit log names leases and objects, never content or keys.
    let log = serde_json::to_string(fx.workload.log()).unwrap();
    assert!(log.contains("granted") && log.contains(OBJ_A));
    for secret in [
        oa_vault::b64(Vmk::from_bytes([0x74; 32]).user_share().as_ref()),
        "lab results".into(),
    ] {
        assert!(!log.contains(&secret));
    }
}

#[test]
fn a_wrong_share_or_wrong_enclave_key_opens_nothing() {
    let mut fx = fx(false);
    let a = fx
        .client
        .seal_object(VAULT, OBJ_A, b"secret", false, fx.now)
        .unwrap();
    // Another person's client (a different share) with a validly enrolled key.
    let mut other = Client::new(Vmk::from_bytes([0x99; 32]), SLOT, secret(2));
    let cert = fx.workload.system_key(VAULT).unwrap();
    other
        .accept_system_key(&fx.endpoint, &cert, fx.now)
        .unwrap();
    let lease = other
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = other.request(&fx.endpoint, VAULT, &lease, fx.now).unwrap();
    let objects = vec![(OBJ_A.to_string(), a.clone())];
    assert!(
        fx.workload
            .serve(&enc, &request, &objects, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("identity_mismatch")
    );
    // A rotated system key (another enclave key) cannot open the old wrap.
    fx.workload.rotate(VAULT).unwrap();
    let lease = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &objects, &fx.enrolled, fx.now)
            .is_err()
    );
    // An object without a device copy does not open on the device.
    assert!(fx.client.open_on_device(&a).is_err());
}

#[test]
fn stale_or_forged_attestation_and_certificates_are_refused() {
    let mut fx = fx(false);
    let lease = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let stale = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now + 3600);
    assert!(matches!(stale, Err(why) if why.starts_with("stale")));
    // A certificate signed by a key other than the attested endpoint's.
    let mut forged = fx.workload.system_key(VAULT).unwrap();
    forged = Signed::sign(forged.body, &secret(9)).unwrap();
    let mut client = Client::new(Vmk::from_bytes([0x74; 32]), SLOT, secret(2));
    assert!(
        client
            .accept_system_key(&fx.endpoint, &forged, fx.now)
            .is_err()
    );
    // A certificate for another release.
    let mut other = fx.workload.system_key(VAULT).unwrap().body;
    other["release"] = json!("77".repeat(32));
    let resigned = Signed::sign(other, &secret(1)).unwrap();
    assert!(
        client
            .accept_system_key(&fx.endpoint, &resigned, fx.now)
            .is_err()
    );
    // A rolled-back epoch.
    fx.workload.rotate(VAULT).unwrap();
    let newer = fx.workload.system_key(VAULT).unwrap();
    fx.client
        .accept_system_key(&fx.endpoint, &newer, fx.now)
        .unwrap();
    let mut older = newer.body.clone();
    older["epoch"] = json!(1);
    let older = Signed::sign(older, &secret(1)).unwrap();
    assert!(
        fx.client
            .accept_system_key(&fx.endpoint, &older, fx.now)
            .unwrap_err()
            .starts_with("stale")
    );
    // An attestation past its validity refuses a certificate too.
    assert!(
        fx.client
            .accept_system_key(&fx.endpoint, &newer, fx.now + 3600)
            .is_err()
    );
}

#[test]
fn leases_are_exact_single_use_revocable_and_signed_by_an_enrolled_slot() {
    let mut fx = fx(false);
    let a = fx
        .client
        .seal_object(VAULT, OBJ_A, b"alpha", false, fx.now)
        .unwrap();
    let b = fx
        .client
        .seal_object(VAULT, OBJ_B, b"beta", false, fx.now)
        .unwrap();
    let both = vec![
        (OBJ_A.to_string(), a.clone()),
        (OBJ_B.to_string(), b.clone()),
    ];
    // Overbroad: the request names an object the lease does not.
    let lease = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &both, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("limit_exceeded")
    );
    // Expired.
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    let only_a = vec![(OBJ_A.to_string(), a.clone())];
    assert!(
        fx.workload
            .serve(&enc, &request, &only_a, &fx.enrolled, fx.now + 901)
            .unwrap_err()
            .starts_with("stale")
    );
    // Too long to issue at all.
    assert!(
        fx.client
            .lease(
                RUN,
                &[OBJ_A],
                "answer",
                "private",
                read(),
                86_401,
                1,
                fx.now
            )
            .is_err()
    );
    // Revoked.
    let lease = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 3, fx.now)
        .unwrap();
    let id = lease.body["lease"].as_str().unwrap().to_string();
    fx.workload
        .revoke(&fx.client.revoke(&id).unwrap(), &fx.enrolled)
        .unwrap();
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &only_a, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("revoked")
    );
    // An unknown slot key cannot authorize unattended decryption.
    let rogue = Client::new(Vmk::from_bytes([0x74; 32]), SLOT, secret(8));
    let mut rogue_client = rogue;
    rogue_client
        .accept_system_key(
            &fx.endpoint,
            &fx.workload.system_key(VAULT).unwrap(),
            fx.now,
        )
        .unwrap();
    let lease = rogue_client
        .lease(RUN, &[OBJ_A], "answer", "private", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = rogue_client
        .request(&fx.endpoint, VAULT, &lease, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &only_a, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("identity_mismatch")
    );
    let log = fx.workload.log();
    assert!(log.iter().any(|r| r["result"] == "released"));
    assert!(log.iter().all(|r| r["result"] != "granted"));
}

#[test]
fn routes_and_effects_follow_the_release_and_the_lease() {
    let mut fx = fx(false);
    let a = fx
        .client
        .seal_object(VAULT, OBJ_A, b"alpha", false, fx.now)
        .unwrap();
    let only_a = vec![(OBJ_A.to_string(), a.clone())];
    let fast = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "fast", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &fast, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &only_a, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    let device = fx
        .client
        .lease(RUN, &[OBJ_A], "answer", "device", read(), 900, 1, fx.now)
        .unwrap();
    let (enc, request, _) = fx
        .client
        .request(&fx.endpoint, VAULT, &device, fx.now)
        .unwrap();
    assert!(
        fx.workload
            .serve(&enc, &request, &only_a, &fx.enrolled, fx.now)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    let export = fx
        .client
        .lease(
            RUN,
            &[OBJ_A],
            "answer",
            "device",
            json!({"read":true,"write_derived":false,"export":true}),
            900,
            1,
            fx.now,
        )
        .unwrap();
    let (enc, request, pending) = fx
        .client
        .request(&fx.endpoint, VAULT, &export, fx.now)
        .unwrap();
    let replies = fx
        .workload
        .serve(&enc, &request, &only_a, &fx.enrolled, fx.now)
        .unwrap();
    assert_eq!(
        fx.client.open_replies(&pending, &replies).unwrap()[0].as_slice(),
        b"alpha"
    );
}

#[test]
fn the_system_key_is_released_only_to_its_attested_release() {
    use crate::kms::Kms;
    let mut kms = LocalKms::new([1; 32], RELEASE);
    let sealed = kms.seal(RELEASE, b"system key").unwrap();
    assert_eq!(
        kms.unseal(RELEASE, &sealed).unwrap().as_slice(),
        b"system key"
    );
    assert!(kms.unseal(&"88".repeat(32), &sealed).is_err());
    assert!(kms.seal(&"88".repeat(32), b"x").is_err());
}

#[test]
fn verification_requires_a_vault_workload_at_tee_cloud() {
    // The captured real endpoint verifies for its own workload but is not a
    // vault: it names another workload and carries no session key.
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../oa-att/tests/fixtures/confidential-space-tdx-2026-10-10.json"
    ))
    .unwrap();
    let event =
        |key: &str| -> nostr::domain::Event { serde_json::from_value(raw[key].clone()).unwrap() };
    let records = oa_att::Records {
        release: event("release"),
        head: event("head"),
        endpoint: event("endpoint"),
        beacon: None,
    };
    let refused = crate::client::verify_endpoint(
        &records,
        raw["publisher"].as_str().unwrap(),
        raw["captured_at"].as_u64().unwrap(),
    )
    .unwrap_err();
    assert!(refused.starts_with("not_admitted"), "{refused}");
}
