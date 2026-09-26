use super::invoice::test_invoice;
use super::*;
use serde_json::json;

const INVOICE: &str = "lnbc250n1pj48ugqpp54y3u9s8ylemsv8l3ewyzzu0klhujvuvmkl6llchq23vy8rzjsf0qsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygshp5p4nz8am4uqj4q8a87z3sk4x6yk4dv2mvel34epw68qqkwy0xcqvqxqzfvcqpjr4rx6ls6j5rpwknuea64evlk7yfx56wmqcer5eerekdsn9tlv6v4ex9mlz5dtm9qapl3svwlqcf7837dmjkru9z9w4h2rvm0md52w2sqxrwu5f";
const PREIMAGE: &str = "0001020304050607080900010203040506070809000102030405060708090102";
const HASH: &str = "0d6623f775e025501fa7f0a30b54da25aad62b6ccfe35c85da38016711e6c018";
const PAYEE: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn requirements() -> PaymentRequirements {
    serde_json::from_value(json!({"scheme":"exact","network":MAINNET,"amount":"25000","asset":"BTC","payTo":PAYEE,"maxTimeoutSeconds":300,"extra":{"assetTransferMethod":"bolt11","paymentFlow":"upfront","requestHash":HASH,"requestBindingProfile":"http:1","requestBindingParams":{"headers":[]},"invoice":INVOICE}})).unwrap()
}

#[test]
fn pinned_upstream_invoice_verifies_crypto_exact_terms_and_preimage() {
    let req = requirements();
    let invoice =
        validate_challenge(&req, HASH, 1_700_000_000, 60, SupportedProfiles::default()).unwrap();
    assert_eq!(invoice.amount_msat(), 25000);
    assert_eq!(invoice.created_at(), 1_700_000_000);
    assert_eq!(invoice.expiry_seconds(), 300);
    assert_eq!(hex(&invoice.payee()), PAYEE);
    let proof = validate_paid_proof(
        &req,
        &req,
        PREIMAGE,
        1_700_000_000,
        60,
        SupportedProfiles::default(),
    )
    .unwrap();
    assert_eq!(
        proof.payment_hash,
        digest(&hex_bytes::<32>(PREIMAGE).unwrap())
    );
    assert_eq!(
        proof.consumption_key,
        format!("{MAINNET}:{}", proof.payment_hash)
    );
    assert_eq!(proof.retain_until, 1_700_003_960);
}

#[test]
fn payment_time_and_paid_retry_grace_have_different_inclusive_boundaries() {
    let req = requirements();
    assert!(
        validate_challenge(&req, HASH, 1_700_000_299, 60, SupportedProfiles::default()).is_ok()
    );
    assert!(matches!(
        validate_challenge(&req, HASH, 1_700_000_300, 60, SupportedProfiles::default()),
        Err(PaymentError::InvoiceExpired)
    ));
    assert!(
        validate_paid_proof(
            &req,
            &req,
            PREIMAGE,
            1_700_000_360,
            60,
            SupportedProfiles::default()
        )
        .is_ok()
    );
    assert_eq!(
        validate_paid_proof(
            &req,
            &req,
            PREIMAGE,
            1_700_000_361,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::InvoiceExpired)
    );
    assert!(
        validate_challenge(&req, HASH, 1_699_999_940, 60, SupportedProfiles::default()).is_ok()
    );
    assert!(matches!(
        validate_challenge(&req, HASH, 1_699_999_939, 60, SupportedProfiles::default()),
        Err(PaymentError::InvoiceFuture)
    ));
}

#[test]
fn paid_original_invoice_survives_dynamic_new_challenge_but_no_term_changes() {
    let accepted = requirements();
    let mut fresh = accepted.clone();
    let invoice = test_invoice::signed(
        "lnbc250n",
        test_invoice::fields(hex_bytes::<32>(HASH).unwrap()),
        false,
        false,
    );
    fresh.extra.insert("invoice".into(), invoice.into());
    // The facilitator decodes the paid accepted invoice, not the current
    // challenge. Issuance validates that new challenge separately.
    assert!(
        validate_paid_proof(
            &fresh,
            &accepted,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        )
        .is_ok()
    );
    fresh.extra.insert("merchant_policy".into(), json!({"v":1}));
    assert_eq!(
        validate_paid_proof(
            &fresh,
            &accepted,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::Terms)
    );
    let mut extra_client = accepted.clone();
    extra_client
        .extra
        .insert("client_note".into(), json!("inert"));
    assert!(
        validate_paid_proof(
            &accepted,
            &extra_client,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        )
        .is_ok()
    );
    let mut omitted_method = accepted.clone();
    omitted_method.extra.remove("assetTransferMethod");
    assert!(
        validate_paid_proof(
            &accepted,
            &omitted_method,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        )
        .is_ok()
    );
}

#[test]
fn request_payee_network_amount_and_proof_mutations_are_rejected() {
    let req = requirements();
    for (field, value, expected) in [
        (
            "requestHash",
            json!("11".repeat(32)),
            PaymentError::InvoiceRequest,
        ),
        (
            "requestBindingProfile",
            json!("unknown"),
            PaymentError::Profile,
        ),
    ] {
        let mut changed = req.clone();
        changed.extra.insert(field.into(), value);
        assert_eq!(
            validate_paid_proof(
                &changed,
                &changed,
                PREIMAGE,
                1_700_000_000,
                60,
                SupportedProfiles::default()
            ),
            Err(expected)
        );
    }
    let mut changed = req.clone();
    changed.network = TESTNET.into();
    assert_eq!(
        validate_paid_proof(
            &changed,
            &changed,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::InvoiceCurrency)
    );
    changed = req.clone();
    changed.amount = "25001".into();
    assert_eq!(
        validate_paid_proof(
            &changed,
            &changed,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::InvoiceAmount)
    );
    changed = req.clone();
    changed.pay_to = format!("03{}", &PAYEE[2..]);
    assert_eq!(
        validate_paid_proof(
            &changed,
            &changed,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::InvoicePayee)
    );
    changed = req.clone();
    changed.max_timeout_seconds = 301;
    assert_eq!(
        validate_paid_proof(
            &changed,
            &changed,
            PREIMAGE,
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::InvoiceExpiry)
    );
    assert_eq!(
        validate_paid_proof(
            &req,
            &req,
            &"11".repeat(32),
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::PreimageHash)
    );
    assert_eq!(
        validate_paid_proof(
            &req,
            &req,
            &"AA".repeat(32),
            1_700_000_000,
            60,
            SupportedProfiles::default()
        ),
        Err(PaymentError::Preimage)
    );
}

#[test]
fn all_seven_retained_binding_vectors_match_without_claiming_artifact_authentication() {
    let file = include_str!("../../../../docs/protocol/fixtures/x402-lightning-bindings-v1.json");
    let value = crate::contracts::parse_strict(file.as_bytes()).unwrap();
    let vectors = value["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 7);
    for vector in vectors {
        let binding = &vector["binding"];
        assert_eq!(
            String::from_utf8(canonical(binding).unwrap()).unwrap(),
            vector["canonical_utf8"].as_str().unwrap()
        );
        assert_eq!(
            binding_hash(binding).unwrap(),
            vector["sha256"].as_str().unwrap()
        );
    }
    assert_eq!(
        binding_hash(&http_binding("GET", "https://api.example.com/article/A", b"", &[]).unwrap())
            .unwrap(),
        HASH
    );
    let mcp = mcp_binding(
        "https://api.example.com/mcp",
        &json!({"name":"get_article","arguments":{"article":"A"}}),
        &[],
    )
    .unwrap();
    assert_eq!(
        binding_hash(&mcp).unwrap(),
        vectors[2]["sha256"].as_str().unwrap()
    );
    let native = &vectors[3]["binding"];
    let built = native_binding(
        native["buyer"].as_str().unwrap(),
        native["provider"].as_str().unwrap(),
        native["purchase"].as_str().unwrap(),
        native["requestDigest"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(&built, native);
    let invalid = &vectors[6]["binding"];
    assert!(
        native_binding(
            invalid["buyer"].as_str().unwrap(),
            invalid["provider"].as_str().unwrap(),
            invalid["purchase"].as_str().unwrap(),
            invalid["requestDigest"].as_str().unwrap()
        )
        .is_err()
    );
}

#[test]
fn binding_preserves_raw_body_url_order_and_absent_empty_or_null_values() {
    let absent = http_binding(
        "POST",
        "https://example.com/x?b=2&a=1",
        b"{\"x\":1}",
        &[("authorization".into(), None)],
    )
    .unwrap();
    let empty = http_binding(
        "POST",
        "https://example.com/x?b=2&a=1",
        b"{\"x\":1}",
        &[("authorization".into(), Some("".into()))],
    )
    .unwrap();
    assert_ne!(
        binding_hash(&absent).unwrap(),
        binding_hash(&empty).unwrap()
    );
    let spaced = http_binding(
        "POST",
        "https://example.com/x?b=2&a=1",
        b"{ \"x\":1}",
        &[("authorization".into(), None)],
    )
    .unwrap();
    assert_ne!(
        binding_hash(&absent).unwrap(),
        binding_hash(&spaced).unwrap()
    );
    let names = vec!["account".into()];
    let absent = mcp_binding("urn:example:tool", &json!({"name":"x"}), &names).unwrap();
    let null = mcp_binding(
        "urn:example:tool",
        &json!({"name":"x","_meta":{"account":null}}),
        &names,
    )
    .unwrap();
    assert_ne!(binding_hash(&absent).unwrap(), binding_hash(&null).unwrap());
    let ignored = mcp_binding(
        "urn:example:tool",
        &json!({"name":"x","arguments":{},"_meta":{"progressToken":"new","x402/payment":{}}}),
        &names,
    )
    .unwrap();
    assert_eq!(absent, ignored);
    assert!(
        mcp_binding(
            "urn:example:tool",
            &json!({"name":"x","arguments":null}),
            &[]
        )
        .is_err()
    );
    assert!(http_binding("GET", "https://u:p@example.com/", b"", &[]).is_err());
    assert!(http_binding("GET", "https://example.com/#fragment", b"", &[]).is_err());
    assert!(
        http_binding(
            "GET",
            "https://example.com/",
            b"",
            &[("payment-signature".into(), None)]
        )
        .is_err()
    );
    assert!(
        mcp_binding(
            "urn:example:tool",
            &json!({"name":"x"}),
            &["progressToken".into()]
        )
        .is_err()
    );
}

#[test]
fn native_profile_requires_explicit_support_and_empty_parameters() {
    let mut req = requirements();
    req.extra
        .insert("requestBindingProfile".into(), json!("nostr:openagents:1"));
    req.extra.insert("requestBindingParams".into(), json!({}));
    assert_eq!(
        validate_requirements(&req, SupportedProfiles::default()),
        Err(PaymentError::Profile)
    );
    let profiles = SupportedProfiles {
        native: true,
        ..SupportedProfiles::default()
    };
    validate_requirements(&req, profiles).unwrap();
    req.extra
        .insert("requestBindingParams".into(), json!({"headers":[]}));
    assert_eq!(
        validate_requirements(&req, profiles),
        Err(PaymentError::Profile)
    );
}

#[test]
fn malformed_uri_authorities_and_non_ascii_or_unescaped_syntax_refuse() {
    for url in [
        "https://example.com/{a}",
        "https://example.com/<a>",
        "https://example.com/%x1",
        "https://[broken]/",
        "https://example.com:abc/",
        "https://:443/",
        "https://example.com:65536/",
        "https://example.com/é",
        "https://example.com/[a]",
        "https://example.com/?a[]=1",
    ] {
        assert!(http_binding("GET", url, b"", &[]).is_err(), "{url}");
    }
    assert!(http_binding("GET", "https://[::1]:443/a%20b", b"", &[]).is_ok());
    assert!(
        http_binding(
            "GET",
            "https://example.com/",
            b"",
            &[("example".into(), Some("a\tb".into()))]
        )
        .is_ok()
    );
}

#[test]
fn raw_wire_helpers_reject_duplicate_fields_before_maps_lose_them() {
    assert!(
        mcp_binding_from_bytes(
            "urn:example:tool",
            br#"{"name":"first","name":"second"}"#,
            &[]
        )
        .is_err()
    );
    assert!(
        mcp_binding_from_bytes(
            "urn:example:tool",
            br#"{"name":"x","arguments":{"a":1,"a":2}}"#,
            &[]
        )
        .is_err()
    );
    let bytes = serde_json::to_vec(&requirements()).unwrap();
    assert!(PaymentRequirements::parse(&bytes).is_ok());
    let duplicate = String::from_utf8(bytes)
        .unwrap()
        .replacen("{", "{\"scheme\":\"exact\",", 1);
    assert!(PaymentRequirements::parse(duplicate.as_bytes()).is_err());
    assert_eq!(
        mcp_binding_from_bytes("urn:example:tool", br#"{"name":"x"}"#, &[]).unwrap(),
        mcp_binding("urn:example:tool", &json!({"name":"x"}), &[]).unwrap()
    );
}
