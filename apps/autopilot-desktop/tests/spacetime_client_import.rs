use autopilot_spacetime::client::ProtocolVersion;

#[test]
fn desktop_imports_spacetime_client_protocol_types() {
    assert_eq!(ProtocolVersion::V2Bsatn.as_str(), "v2.bsatn.spacetimedb");
    assert_eq!(ProtocolVersion::V1Bsatn.as_str(), "v1.bsatn.spacetimedb");
}
