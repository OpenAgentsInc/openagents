#[test]
fn checked_in_document_schema_matches_rust_types() {
    let saved: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/documents.v1.json")).unwrap();
    let actual = serde_json::to_value(schemars::schema_for!(robot_lab::Document)).unwrap();
    assert_eq!(
        saved, actual,
        "Regenerate with cargo run -p robot-lab --example schema -- crates/robot-lab/schemas/documents.v1.json"
    );
}
