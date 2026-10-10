//! Merchant-hosted receivers (#11149): the buyer attestation against every
//! vector of `docs/payments/proposal/fixtures/buyer-attestation-v1.json`,
//! the pinned `payTo`, and the merchant's own sales record. Synthetic keys
//! and test invoices only; no wallet, no money.

use std::sync::{Arc, Mutex};

use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_x402::front::{Scheme, Settlement, SettlementSink};
use openagents_x402::merchant::{
    AttestationError, AttestationVerifier, BuyerAttestation, MerchantReceiver, MerchantSink, Msat,
    Presented, SaleObserver, SovereignProfile, SpendGrant,
};
use openagents_x402::server::Receiver;
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../docs/payments/proposal/fixtures/buyer-attestation-v1.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

/// One vector's input as the verifier sees it: what was presented, the
/// merchant's verifier, the price, and the time.
fn run(input: &Value) -> Result<openagents_x402::merchant::Attestation, AttestationError> {
    let presented = if input["attestation_present"] == Value::Bool(true) {
        let profile: SovereignProfile = serde_json::from_value(input["profile"].clone()).unwrap();
        let grant: SpendGrant = serde_json::from_value(input["grant"].clone()).unwrap();
        profile.check().unwrap();
        grant.check().unwrap();
        Some(Presented {
            profile,
            grant,
            signature_valid: input["signature_valid"].as_bool().unwrap(),
        })
    } else {
        None
    };
    let trusted = input["trusted_authorities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_owned());
    let reputation = input["reputation"]
        .as_u64()
        .map(|score| u32::try_from(score).unwrap());
    let min = u32::try_from(input["min_reputation"].as_u64().unwrap()).unwrap();
    let verifier =
        AttestationVerifier::new(trusted).with_reputation(min, Arc::new(move |_: &str| reputation));
    let price = Msat::parse(input["price_msat"].as_str().unwrap())
        .unwrap()
        .0;
    verifier.verify(presented.as_ref(), price, input["now"].as_u64().unwrap())
}

#[test]
fn every_buyer_attestation_vector_gets_its_verdict() {
    let fixture = fixture();
    assert_eq!(fixture["v"], "openagents.buyer-attestation-vectors.v1");
    let vocabulary: Vec<&str> = fixture["error_vocabulary"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code.as_str().unwrap())
        .collect();
    let vectors = fixture["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 12);
    let mut codes = std::collections::BTreeSet::new();
    for vector in vectors {
        let name = vector["name"].as_str().unwrap();
        let expect = &vector["expect"];
        let got = run(&vector["input"]);
        match (&got, expect.get("ok"), expect.get("error")) {
            (Ok(attestation), Some(ok), None) => {
                assert_eq!(attestation.buyer, ok["buyer"].as_str().unwrap(), "{name}");
                assert_eq!(attestation.owner, ok["owner"].as_str().unwrap(), "{name}");
                assert_eq!(
                    attestation.max_per_tx_msat.to_string(),
                    ok["max_per_tx_msat"].as_str().unwrap(),
                    "{name}"
                );
            }
            (Err(error), None, Some(code)) => {
                assert_eq!(error.code(), code.as_str().unwrap(), "{name}");
                assert!(vocabulary.contains(&error.code()), "{name}");
                codes.insert(error.code());
                if let Some(detail) = expect.get("detail") {
                    match error {
                        AttestationError::OverCeiling {
                            price_msat,
                            ceiling_msat,
                        } => {
                            assert_eq!(price_msat.to_string(), detail["price_msat"], "{name}");
                            assert_eq!(ceiling_msat.to_string(), detail["ceiling_msat"], "{name}");
                        }
                        AttestationError::ReputationTooLow { required, got } => {
                            assert_eq!(u64::from(*required), detail["required"], "{name}");
                            assert_eq!(u64::from(*got), detail["got"], "{name}");
                        }
                        other => panic!("{name}: no detail expected for {other:?}"),
                    }
                }
            }
            _ => panic!("{name}: got {got:?}, expected {expect}"),
        }
    }
    // Every code in the vocabulary is exercised.
    assert_eq!(codes.len(), vocabulary.len());
}

#[test]
fn the_vectors_follow_the_written_evaluation_order() {
    let fixture = fixture();
    let order: Vec<&str> = fixture["evaluation_order"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step["else"].as_str())
        .collect();
    let codes = [
        AttestationError::Missing,
        AttestationError::Signature,
        AttestationError::Authority,
        AttestationError::PrincipalMismatch,
        AttestationError::PurposeNotOperational,
        AttestationError::EffectAbsent,
        AttestationError::Expired,
        AttestationError::OverCeiling {
            price_msat: 0,
            ceiling_msat: 0,
        },
        AttestationError::ReputationTooLow {
            required: 0,
            got: 0,
        },
    ];
    let ours: Vec<&str> = codes.iter().map(AttestationError::code).collect();
    assert_eq!(order, ours);
}

#[test]
fn the_first_failure_wins_when_several_fail() {
    // Untrusted authority and an expired, evaluation-only grant over the
    // ceiling: the authority check comes first.
    let fixture = fixture();
    let mut input = fixture["vectors"][0]["input"].clone();
    input["profile"]["authority"] = Value::String("c6".repeat(32));
    input["grant"]["purpose"] = Value::String("evaluation".into());
    input["grant"]["expires_at"] = Value::from(1);
    input["price_msat"] = Value::String("60000000".into());
    assert_eq!(run(&input).unwrap_err(), AttestationError::Authority);
    // A bad signature outranks all of them.
    input["signature_valid"] = Value::Bool(false);
    assert_eq!(run(&input).unwrap_err(), AttestationError::Signature);
}

#[test]
fn a_grant_from_another_authority_is_refused() {
    // A trusted owner's agent presenting a grant some other key issued.
    let fixture = fixture();
    let mut input = fixture["vectors"][0]["input"].clone();
    input["grant"]["issuer"] = Value::String("c6".repeat(32));
    assert_eq!(run(&input).unwrap_err(), AttestationError::Authority);
}

#[test]
fn a_reputation_gate_refuses_an_agent_with_none() {
    let fixture = fixture();
    let mut input = fixture["vectors"][1]["input"].clone();
    input["reputation"] = Value::Null;
    assert_eq!(
        run(&input).unwrap_err(),
        AttestationError::ReputationTooLow {
            required: 50,
            got: 0
        }
    );
}

#[test]
fn malformed_attestations_are_not_presented() {
    let fixture = fixture();
    let input = &fixture["vectors"][0]["input"];
    let profile = input["profile"].to_string();
    let grant = input["grant"].to_string();
    assert!(Presented::parse(&profile, &grant, true).is_some());
    // A leading-zero amount, an unknown effect, a wrong schema, a short key.
    let mut bad = input["grant"].clone();
    bad["bounds"]["max_per_tx_msat"] = Value::String("050000000".into());
    assert!(Presented::parse(&profile, &bad.to_string(), true).is_none());
    let mut bad = input["grant"].clone();
    bad["effects"] = serde_json::json!(["spend", "teleport"]);
    assert!(Presented::parse(&profile, &bad.to_string(), true).is_none());
    let mut bad = input["profile"].clone();
    bad["v"] = Value::String("openagents.sovereign-profile.v0".into());
    assert!(Presented::parse(&bad.to_string(), &grant, true).is_none());
    let mut bad = input["profile"].clone();
    bad["agent"] = Value::String("f9".into());
    assert!(Presented::parse(&bad.to_string(), &grant, true).is_none());
}

const MERCHANT: [u8; 32] = [5; 32];
const OTHER: [u8; 32] = [6; 32];

/// A wallet that signs test invoices as `signer` while claiming `claims`.
struct TestWallet {
    signer: [u8; 32],
    claims: [u8; 32],
    /// Issue for this amount instead of the one asked, when set.
    amount: Option<u64>,
}

impl Receiver for TestWallet {
    fn pay_to(&self) -> String {
        hex::encode(payee_of(self.claims))
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        let amount = self.amount.unwrap_or(amount);
        let mut fields = tag(1, &words(&[3; 32]));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&request_hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        Ok(signed_by(
            self.signer,
            &format!("lnbc{}n", amount / 100),
            fields,
            false,
            false,
            1_800_000_000,
        ))
    }
}

#[test]
fn the_merchant_receiver_pins_its_own_pay_to() {
    let pinned = hex::encode(payee_of(MERCHANT));
    let honest = Arc::new(TestWallet {
        signer: MERCHANT,
        claims: MERCHANT,
        amount: None,
    });
    let receiver = MerchantReceiver::new(honest, &pinned).unwrap();
    assert_eq!(receiver.pay_to(), pinned);
    assert!(receiver.invoice(15_000, [7; 32], 300).is_ok());

    // A wallet that is another node can't be pinned to the merchant.
    let other = Arc::new(TestWallet {
        signer: OTHER,
        claims: OTHER,
        amount: None,
    });
    assert!(MerchantReceiver::new(other, &pinned).is_err());

    // A wallet that claims the merchant's key but signs under another is
    // refused at the invoice.
    let lying = Arc::new(TestWallet {
        signer: OTHER,
        claims: MERCHANT,
        amount: None,
    });
    let receiver = MerchantReceiver::new(lying, &pinned).unwrap();
    assert!(receiver.invoice(15_000, [7; 32], 300).is_err());

    // And one that issues for another amount.
    let short = Arc::new(TestWallet {
        signer: MERCHANT,
        claims: MERCHANT,
        amount: Some(10_000),
    });
    let receiver = MerchantReceiver::new(short, &pinned).unwrap();
    assert!(receiver.invoice(15_000, [7; 32], 300).is_err());

    // Not a node key at all.
    let honest = Arc::new(TestWallet {
        signer: MERCHANT,
        claims: MERCHANT,
        amount: None,
    });
    assert!(MerchantReceiver::new(honest, "ab").is_err());
}

#[derive(Default)]
struct Seen(Mutex<Vec<String>>);

impl SaleObserver for Seen {
    fn observe(&self, settlement: &Settlement) {
        self.0.lock().unwrap().push(settlement.payment_hash.clone());
    }
}

fn settlement(payment_hash: &str) -> Settlement {
    Settlement {
        payment_hash: payment_hash.into(),
        request_hash: "11".repeat(32),
        route: "generate".into(),
        resource: "acme.generate".into(),
        role: "merchant".into(),
        plugin: None,
        release: None,
        author: None,
        fee_msat: None,
        price_msat: 15_000,
        received_msat: 15_000,
        received_from_wallet: true,
        scheme: Scheme::X402,
        network: nostr::x402::MAINNET.into(),
        settled_at: 1_800_000_000,
    }
}

#[test]
fn the_merchant_sink_records_each_sale_once_and_no_shares() {
    let dir = tempfile::tempdir().unwrap();
    let seen = Arc::new(Seen::default());
    let sink = MerchantSink::open(&dir.path().join("sales.ndjson"))
        .unwrap()
        .observed(seen.clone());
    let sale = settlement(&"aa".repeat(32));
    sink.on_settled(&sale).unwrap();
    // The same payment again is the same sale: recorded once.
    sink.on_settled(&sale).unwrap();
    let text = std::fs::read_to_string(sink.path()).unwrap();
    assert_eq!(text.lines().count(), 1);
    let line: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(line["payment_hash"], "aa".repeat(32));
    // Nothing owed to anyone: no author, no fee share.
    assert!(line.get("author").is_none() && line.get("fee_msat").is_none());
    // A different record under the same payment hash is refused.
    let mut changed = sale.clone();
    changed.price_msat = 1;
    assert!(sink.on_settled(&changed).is_err());
    assert_eq!(seen.0.lock().unwrap().len(), 2);
}
