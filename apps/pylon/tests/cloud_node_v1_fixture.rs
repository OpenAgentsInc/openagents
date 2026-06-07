#![allow(
    clippy::panic_in_result_fn,
    reason = "Fixture tests use assertions for clearer failure output."
)]

use serde_json::Value;

const CONTRIBUTOR_FIXTURE: &str =
    include_str!("../../../docs/pylon/fixtures/cloud_node_v1/contributor-pylon.json");

#[test]
fn contributor_cloud_node_v1_fixture_is_public_safe() -> Result<(), Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_str(CONTRIBUTOR_FIXTURE)?;

    assert_eq!(
        fixture.pointer("/contract_version").and_then(Value::as_str),
        Some("openagents.cloud_node.v1")
    );
    assert_eq!(
        fixture
            .pointer("/policy/settlement_policy")
            .and_then(Value::as_str),
        Some("contributor_wallet")
    );
    assert!(
        matches!(
            fixture.pointer("/capabilities/workroom_capacity"),
            Some(Value::Null)
        ),
        "contributor Pylon must not claim managed workroom capacity"
    );
    assert_eq!(
        fixture
            .pointer("/capabilities/ingress_support/ready")
            .and_then(Value::as_bool),
        Some(false)
    );

    pylon_core::assert_public_json_boundary(&fixture)?;

    Ok(())
}
