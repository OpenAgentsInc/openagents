use openagents_proto::wire::openagents::lightning::v1::{
    WalletExecutionReceipt, WalletExecutorAuthAssertion, WalletExecutorAuthMode,
};
use prost::Message;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/protocol/fixtures/lightning-wallet-executor-receipt-v1.json"
    ))
    .expect("wallet executor fixture JSON must parse")
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn i64_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn parse_auth_mode(raw: &str) -> i32 {
    match raw {
        "WALLET_EXECUTOR_AUTH_MODE_DISABLED" => WalletExecutorAuthMode::Disabled as i32,
        "WALLET_EXECUTOR_AUTH_MODE_BEARER_STATIC" => WalletExecutorAuthMode::BearerStatic as i32,
        _ => WalletExecutorAuthMode::Unspecified as i32,
    }
}

#[test]
fn wallet_executor_fixture_matches_wire_contract() {
    let root = fixture();
    assert_eq!(
        root.get("schema").and_then(Value::as_str),
        Some("openagents.lightning.wallet_executor_fixture.v1")
    );

    let auth = root
        .get("auth_assertion")
        .expect("fixture must include auth_assertion");
    let receipt = root.get("receipt").expect("fixture must include receipt");

    assert!(
        auth.get("token").is_none(),
        "fixture must never include raw token material"
    );

    let wire_auth = WalletExecutorAuthAssertion {
        wallet_id: string_field(auth, "wallet_id"),
        auth_mode: parse_auth_mode(&string_field(auth, "auth_mode")),
        auth_enforced: bool_field(auth, "auth_enforced"),
        auth_token_version: u64_field(auth, "auth_token_version") as u32,
        token_fingerprint: string_field(auth, "token_fingerprint"),
        asserted_at_ms: i64_field(auth, "asserted_at_ms"),
    };

    let wire_receipt = WalletExecutionReceipt {
        receipt_version: string_field(receipt, "receipt_version"),
        receipt_id: string_field(receipt, "receipt_id"),
        request_id: string_field(receipt, "request_id"),
        wallet_id: string_field(receipt, "wallet_id"),
        host: string_field(receipt, "host"),
        payment_id: string_field(receipt, "payment_id"),
        invoice_hash: string_field(receipt, "invoice_hash"),
        quoted_amount_msats: u64_field(receipt, "quoted_amount_msats"),
        settled_amount_msats: u64_field(receipt, "settled_amount_msats"),
        preimage_sha256: string_field(receipt, "preimage_sha256"),
        paid_at_ms: i64_field(receipt, "paid_at_ms"),
        rail: string_field(receipt, "rail"),
        asset_id: string_field(receipt, "asset_id"),
        canonical_json_sha256: string_field(receipt, "canonical_json_sha256"),
    };

    assert_eq!(
        wire_auth.auth_mode,
        WalletExecutorAuthMode::BearerStatic as i32,
        "fixture auth mode mismatch"
    );
    assert!(wire_auth.auth_enforced, "auth should be enforced");
    assert!(!wire_auth.token_fingerprint.is_empty());

    assert_eq!(
        wire_receipt.receipt_version,
        "openagents.lightning.wallet_receipt.v1"
    );
    assert!(wire_receipt.receipt_id.starts_with("lwr_"));
    assert_eq!(wire_receipt.rail, "lightning");
    assert_eq!(wire_receipt.asset_id, "BTC_LN");
    assert_eq!(wire_receipt.quoted_amount_msats, wire_receipt.settled_amount_msats);

    let auth_roundtrip = WalletExecutorAuthAssertion::decode(wire_auth.encode_to_vec().as_slice())
        .expect("auth assertion should decode after encode");
    assert_eq!(auth_roundtrip.wallet_id, wire_auth.wallet_id);
    assert_eq!(auth_roundtrip.auth_token_version, wire_auth.auth_token_version);

    let receipt_roundtrip = WalletExecutionReceipt::decode(wire_receipt.encode_to_vec().as_slice())
        .expect("wallet receipt should decode after encode");
    assert_eq!(receipt_roundtrip.receipt_id, wire_receipt.receipt_id);
    assert_eq!(
        receipt_roundtrip.canonical_json_sha256,
        wire_receipt.canonical_json_sha256
    );
}

