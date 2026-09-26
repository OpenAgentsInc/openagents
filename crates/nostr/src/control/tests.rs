use super::*;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::json;

fn key(n: u8) -> String {
    SecretKey::from_byte_array([n; 32])
        .unwrap()
        .x_only_public_key(&Secp256k1::new())
        .0
        .to_string()
}
fn reference(schema: &str) -> Value {
    json!({"digest":format!("sha256:{}", "a".repeat(64)),"size":10,"media_type":"application/json","schema":schema})
}
fn scope() -> Value {
    json!({"task":"1".repeat(64),"controller":key(1),"generation":0})
}
fn fixtures() -> Vec<Value> {
    let policy = reference("fixture.policy.v1");
    let common = json!({"owner":key(1),"authority":key(1),"client":key(2),"scope":scope(),"rights":["observe","steer","cancel"],"policy":policy,"issued_at":10,"expires_at":20});
    let mut invitation = common.clone();
    invitation["v"] = json!(INVITATION);
    invitation["requires"] = json!([]);
    invitation["invitation"] = json!("2".repeat(64));
    invitation["challenge"] = json!("3".repeat(64));
    let mut grant = common;
    grant["v"] = json!(GRANT);
    grant["requires"] = json!([]);
    grant["grant"] = json!("4".repeat(64));
    grant["epoch"] = json!(0);
    grant["invitation"] = reference(INVITATION);
    grant["pairing"] = reference(PAIRING);
    grant["admission"] = reference("fixture.admission.v1");
    vec![
        invitation,
        json!({"v":PAIRING,"requires":[],"invitation":reference(INVITATION),"client":key(2),"challenge":"3".repeat(64),"rights":["observe"],"accepted":true}),
        grant,
        json!({"v":ACCESS_RESULT,"requires":[],"request":reference(PAIRING),"status":"granted","access":reference(GRANT),"reason":null}),
        json!({"v":REVOKE,"requires":[],"request":"5".repeat(64),"grant":reference(GRANT),"reason":"owner request"}),
        json!({"v":REVOCATION,"requires":[],"grant":reference(GRANT),"epoch":1,"authority":key(1),"request":reference(REVOKE),"authorization":reference("fixture.authorization.v1"),"revoked_at":15}),
        json!({"v":COMMAND,"requires":[],"command":"6".repeat(64),"grant":reference(GRANT),"epoch":0,"scope":scope(),"expected_revision":1,"issued_at":10,"expires_at":20,"action":"steer","payload":{"message":reference("fixture.text.v1"),"replaces":[]}}),
        json!({"v":COMMAND_RESULT,"requires":[],"command":reference(COMMAND),"status":"accepted","receipt":reference(COMMAND_RECEIPT),"reason":null}),
        json!({"v":COMMAND_RECEIPT,"requires":[],"command":reference(COMMAND),"authority":key(1),"admitted_at":12,"disposition":"correction_recorded"}),
        json!({"v":READ,"requires":[],"request":"7".repeat(64),"grant":reference(GRANT),"epoch":0,"scope":scope(),"view":"history","after":null,"max_items":256,"max_bytes":1048576}),
        json!({"v":VIEW,"requires":[],"request":reference(READ),"authority":key(1),"scope":scope(),"captured_at":13,"policy":policy,"items":[{"kind":"projection","artifact":reference(PROJECTION),"provenance":reference("openagents.evidence.v1")}],"next":null,"coverage":"partial"}),
        json!({"v":PROJECTION,"requires":[],"content":reference("fixture.display.v1"),"sources":[],"reason":"redacted","coverage":"unknown"}),
    ]
}
fn fixture(schema: &str) -> Value {
    fixtures()
        .into_iter()
        .find(|value| value["v"] == schema)
        .unwrap()
}
#[test]
fn all_twelve_shapes_round_trip_and_allow_only_inert_metadata() {
    for mut value in fixtures() {
        validate(&value).unwrap();
        assert_eq!(parse(&contracts::jcs(&value).unwrap()).unwrap(), value);
        value["meta"] = json!({"display":"untrusted inert metadata","approve":true});
        validate(&value).unwrap();
    }
}
#[test]
fn every_shape_rejects_missing_extra_version_and_required_feature_fields() {
    for value in fixtures() {
        for key in value.as_object().unwrap().keys() {
            let mut bad = value.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(validate(&bad).is_err(), "{} missing {key}", value["v"]);
        }
        for (key, replacement) in [
            ("extra", json!(true)),
            ("v", json!("future.v2")),
            ("requires", json!(["approve"])),
        ] {
            let mut bad = value.clone();
            bad[key] = replacement;
            assert!(validate(&bad).is_err(), "{} extra {key}", value["v"]);
        }
    }
}
#[test]
fn strict_json_rejects_duplicate_fields_depth_bytes_and_unsafe_integers() {
    assert!(parse(br#"{"v":"x","v":"y"}"#).is_err());
    let mut value = fixture(INVITATION);
    value["issued_at"] = json!(9_007_199_254_740_992u64);
    assert!(validate(&value).is_err());
    let mut nested = json!(null);
    for _ in 0..65 {
        nested = json!([nested]);
    }
    value["meta"] = nested;
    assert_eq!(
        validate(&value).unwrap_err().code,
        RefusalCode::LimitExceeded
    );
    value = fixture(INVITATION);
    value["meta"] = json!("x".repeat(contracts::MAX_BODY_BYTES));
    assert_eq!(
        validate(&value).unwrap_err().code,
        RefusalCode::LimitExceeded
    );
}
#[test]
fn identities_principals_scope_and_rights_remain_narrow() {
    for rights in [json!(["observe"]), json!(["steer"]), json!(["cancel"])] {
        let mut value = fixture(GRANT);
        value["rights"] = rights;
        validate(&value).unwrap();
    }
    for rights in [
        json!([]),
        json!(["observe", "observe"]),
        json!(["approve"]),
        json!(["execute"]),
    ] {
        let mut value = fixture(GRANT);
        value["rights"] = rights;
        assert!(validate(&value).is_err());
    }
    for field in ["owner", "authority", "client"] {
        let mut value = fixture(INVITATION);
        value[field] = json!("f".repeat(64));
        assert!(validate(&value).is_err());
    }
    let mut value = fixture(INVITATION);
    value["client"] = value["authority"].clone();
    assert!(validate(&value).is_err());
    value = fixture(INVITATION);
    value["scope"]["controller"] = json!(key(3));
    assert!(validate(&value).is_err());
    value = fixture(INVITATION);
    value["scope"]["all_tasks"] = json!(true);
    assert!(validate(&value).is_err());
    value = fixture(INVITATION);
    value["scope"]["task"] = json!("*".repeat(64));
    assert!(validate(&value).is_err());
    value = fixture(INVITATION);
    value["challenge"] = value["invitation"].clone();
    assert!(validate(&value).is_err());
}
#[test]
fn epochs_and_windows_are_structural_not_host_admission() {
    for schema in [GRANT, COMMAND, READ] {
        let mut value = fixture(schema);
        value["epoch"] = json!(1);
        assert!(validate(&value).is_err());
    }
    let mut value = fixture(REVOCATION);
    value["epoch"] = json!(0);
    assert!(validate(&value).is_err());
    for schema in [INVITATION, GRANT, COMMAND] {
        let mut value = fixture(schema);
        value["expires_at"] = value["issued_at"].clone();
        assert!(validate(&value).is_err());
    }
    // Historical valid shapes parse; the host supplies current time and policy.
    validate(&fixture(INVITATION)).unwrap();
    let mut declined = fixture(PAIRING);
    declined["accepted"] = json!(false);
    validate(&declined).unwrap();
}
#[test]
fn command_payloads_cannot_encode_approval_or_change_authority() {
    let mut value = fixture(COMMAND);
    value["action"] = json!("approve");
    assert!(validate(&value).is_err());
    value = fixture(COMMAND);
    value["payload"]["grant"] = reference(GRANT);
    assert!(validate(&value).is_err());
    value = fixture(COMMAND);
    value["payload"]["replaces"] = json!([
        reference("fixture.objective.v1"),
        reference("fixture.objective.v1")
    ]);
    assert!(validate(&value).is_err());
    value = fixture(COMMAND);
    value["action"] = json!("cancel");
    value["payload"] = json!({"reason":"é".repeat(512)});
    validate(&value).unwrap();
    value["payload"]["reason"] = json!("é".repeat(513));
    assert!(validate(&value).is_err());
    value["payload"] = json!({"reason":"stop","message":"new command"});
    assert!(validate(&value).is_err());
}
#[test]
fn access_and_command_results_cannot_lose_required_receipts_or_reasons() {
    let mut value = fixture(ACCESS_RESULT);
    value["access"] = Value::Null;
    assert!(validate(&value).is_err());
    value["status"] = json!("refused");
    assert!(validate(&value).is_err());
    value["reason"] = json!("revoked");
    validate(&value).unwrap();
    value["access"] = reference(GRANT);
    assert!(validate(&value).is_err());
    value = fixture(ACCESS_RESULT);
    value["status"] = json!("duplicate");
    value["access"] = reference(REVOCATION);
    validate(&value).unwrap();
    for status in ["accepted", "duplicate"] {
        let mut value = fixture(COMMAND_RESULT);
        value["status"] = json!(status);
        value["receipt"] = Value::Null;
        assert!(validate(&value).is_err());
    }
    for status in ["conflict", "refused"] {
        let mut value = fixture(COMMAND_RESULT);
        value["status"] = json!(status);
        assert!(validate(&value).is_err());
    }
    let mut value = fixture(COMMAND_RESULT);
    value["status"] = json!("unknown");
    value["receipt"] = Value::Null;
    validate(&value).unwrap();
    value["reason"] = json!("probably_passed");
    assert!(validate(&value).is_err());
}
#[test]
fn read_cursor_and_byte_limits_cannot_expand_scope() {
    for (field, replacement) in [
        ("max_items", json!(0)),
        ("max_items", json!(257)),
        ("max_bytes", json!(0)),
        ("max_bytes", json!(1048577)),
        ("after", json!("é".repeat(513))),
    ] {
        let mut value = fixture(READ);
        value[field] = replacement;
        assert!(validate(&value).is_err());
    }
    let mut value = fixture(READ);
    value["after"] = json!("é".repeat(512));
    validate(&value).unwrap();
    value["view"] = json!("state");
    assert!(validate(&value).is_err());
    value["after"] = Value::Null;
    validate(&value).unwrap();
}
#[test]
fn views_require_typed_originals_or_honest_projections() {
    let mut value = fixture(VIEW);
    value["items"][0]["kind"] = json!("frame");
    assert!(validate(&value).is_err());
    value["items"][0]["artifact"] = reference("openagents.task-frame.v1");
    validate(&value).unwrap();
    value["items"][0]["provenance"] = reference("fixture.self-assertion.v1");
    assert!(validate(&value).is_err());
    value = fixture(VIEW);
    value["items"] = json!(vec![value["items"][0].clone(); 257]);
    assert!(validate(&value).is_err());
    value = fixture(VIEW);
    value["items"] = json!([]);
    value["coverage"] = json!("unknown");
    validate(&value).unwrap();
    value = fixture(PROJECTION);
    value["coverage"] = json!("complete");
    assert!(validate(&value).is_err());
    value = fixture(PROJECTION);
    value["content"].as_object_mut().unwrap().remove("schema");
    assert!(validate(&value).is_err());
}
