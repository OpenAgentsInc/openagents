use std::fs;
use std::path::PathBuf;

use openagents_all_work_contract::{
    canonical_json_bytes, ContractValidate, WorkIndexReadRequest, WorkIndexSubscriptionEvent,
    WorkIndexSubscriptionRequest, WorkReadRequestFrame, WorkSnapshot, WorkSummary,
};
use serde::de::DeserializeOwned;

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/all-work-contract")
}

fn bytes(path: &str) -> Vec<u8> {
    fs::read(package_root().join(path)).expect("fixture must exist")
}

fn decode<T: DeserializeOwned + ContractValidate>(path: &str) -> Result<T, String> {
    let value: T = serde_json::from_slice(&bytes(path)).map_err(|error| error.to_string())?;
    value.validate().map_err(|error| error.to_string())?;
    Ok(value)
}

#[test]
fn accepts_shared_positive_fixtures() {
    decode::<WorkSummary>("fixtures/valid/work-summary.json").expect("valid Work summary");
    decode::<WorkSnapshot>("fixtures/valid/work-snapshot.json").expect("valid Work snapshot");
    decode::<WorkIndexReadRequest>("fixtures/valid/work-index-request-absent.json")
        .expect("valid absent cursor");
    decode::<WorkIndexReadRequest>("fixtures/valid/work-index-request-null.json")
        .expect("valid null cursor");
    decode::<WorkIndexSubscriptionRequest>("fixtures/valid/work-index-subscription-request.json")
        .expect("valid subscription request");
    decode::<WorkIndexSubscriptionEvent>("fixtures/valid/work-index-subscription-gap.json")
        .expect("valid subscription event");
    decode::<WorkReadRequestFrame>("fixtures/valid/request-v2-index.json")
        .expect("valid v2 request");
    decode::<WorkReadRequestFrame>("fixtures/valid/request-v1-negotiate.json")
        .expect("valid explicit v1 negotiation");
}

#[test]
fn rejects_shared_negative_fixtures() {
    for path in [
        "fixtures/invalid/work-summary-unknown-field.json",
        "fixtures/invalid/work-summary-unsafe-integer.json",
        "fixtures/invalid/work-summary-bad-ref.json",
        "fixtures/invalid/work-summary-unknown-state.json",
        "fixtures/invalid/work-summary-missing-required-nullable.json",
    ] {
        assert!(
            decode::<WorkSummary>(path).is_err(),
            "accepted invalid fixture {path}"
        );
    }
    assert!(
        decode::<WorkReadRequestFrame>("fixtures/invalid/request-unknown-method.json").is_err()
    );
}

#[test]
fn preserves_absent_and_null_and_matches_canonical_bytes() {
    let absent = decode::<WorkIndexReadRequest>("fixtures/valid/work-index-request-absent.json")
        .expect("absent cursor");
    let explicit_null =
        decode::<WorkIndexReadRequest>("fixtures/valid/work-index-request-null.json")
            .expect("null cursor");
    assert_eq!(absent.cursor, None);
    assert_eq!(explicit_null.cursor, Some(None));

    let summary = decode::<WorkSummary>("fixtures/valid/work-summary.json").expect("summary");
    assert_eq!(
        canonical_json_bytes(&summary).expect("canonical JSON"),
        bytes("generated/canonical/work-summary.canonical.json")
    );
}
