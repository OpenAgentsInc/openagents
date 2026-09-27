//! The embedded facilitator: stateless `verify`, then `settle`, which inserts
//! the replay key exactly once before it reports success. It never pays.

use serde_json::Value;

use nostr::x402::{
    PaymentError, PaymentRequirements, SupportedProfiles, ValidatedPaymentProof,
    validate_paid_proof,
};

use crate::replay::{ReplayEntry, ReplayError, ReplayStore};
use crate::wire::{PaymentPayload, SettlementResponse};

pub const DUPLICATE_SETTLEMENT: &str = "duplicate_settlement";

/// Upstream `errorReason` for a stateless proof failure.
///
/// `validate_paid_proof` collapses several term mismatches into
/// `PaymentError::Terms`; `verify` tells the mismatching field apart before it
/// calls into the validator so each reason is the upstream one.
pub fn error_reason(error: PaymentError) -> &'static str {
    match error {
        PaymentError::Terms => "invalid_exact_lnbtc_extra_mismatch",
        PaymentError::Network => "unsupported_network",
        PaymentError::Amount => "invalid_exact_lnbtc_amount",
        PaymentError::Payee => "invalid_exact_lnbtc_pay_to_malformed",
        PaymentError::Binding => "invalid_exact_lnbtc_request_binding",
        PaymentError::Profile => "invalid_exact_lnbtc_request_binding",
        PaymentError::InvoiceMissing => "invalid_exact_lnbtc_invoice_missing",
        PaymentError::InvoiceDecode => "invalid_exact_lnbtc_invoice_decode_failed",
        PaymentError::InvoiceSignature => "invalid_exact_lnbtc_invoice_decode_failed",
        PaymentError::InvoiceDescription => "invalid_exact_lnbtc_invoice_description",
        PaymentError::InvoiceRequest => "invalid_exact_lnbtc_invoice_request_mismatch",
        PaymentError::InvoicePayee => "invalid_exact_lnbtc_invoice_payee_mismatch",
        PaymentError::InvoiceCurrency => "invalid_exact_lnbtc_invoice_currency_mismatch",
        PaymentError::InvoiceAmount => "invalid_exact_lnbtc_invoice_amount_mismatch",
        PaymentError::InvoiceExpiry => "invalid_exact_lnbtc_invoice_expiry_mismatch",
        PaymentError::InvoiceFuture => "invalid_exact_lnbtc_invoice_created_in_future",
        PaymentError::InvoiceExpired => "invalid_exact_lnbtc_invoice_expired",
        PaymentError::Preimage => "invalid_exact_lnbtc_preimage_malformed",
        PaymentError::PreimageHash => "invalid_exact_lnbtc_preimage_hash_mismatch",
        PaymentError::Overflow => "invalid_exact_lnbtc_max_timeout",
    }
}

fn field_mismatch(
    requirements: &PaymentRequirements,
    accepted: &PaymentRequirements,
) -> Option<&'static str> {
    if requirements.scheme != "exact" || accepted.scheme != "exact" {
        return Some("unsupported_scheme");
    }
    if requirements.network != accepted.network {
        return Some("network_mismatch");
    }
    if requirements.asset != "BTC" || accepted.asset != "BTC" {
        return Some("invalid_exact_lnbtc_asset");
    }
    if requirements.amount != accepted.amount {
        return Some("invalid_exact_lnbtc_amount_mismatch");
    }
    if requirements.pay_to != accepted.pay_to {
        return Some("invalid_exact_lnbtc_pay_to_mismatch");
    }
    if requirements.max_timeout_seconds != accepted.max_timeout_seconds {
        return Some("invalid_exact_lnbtc_max_timeout_mismatch");
    }
    fn flow(r: &PaymentRequirements) -> Option<&str> {
        r.extra.get("paymentFlow").and_then(Value::as_str)
    }
    if flow(requirements) != Some("upfront") || flow(accepted) != Some("upfront") {
        return Some("invalid_exact_lnbtc_payment_flow");
    }
    fn method(r: &PaymentRequirements) -> bool {
        r.extra
            .get("assetTransferMethod")
            .is_none_or(|v| v.as_str() == Some("bolt11"))
    }
    if !method(requirements) || !method(accepted) {
        return Some("invalid_exact_lnbtc_asset_transfer_method");
    }
    for key in [
        "requestHash",
        "requestBindingProfile",
        "requestBindingParams",
    ] {
        if requirements.extra.get(key) != accepted.extra.get(key) {
            return Some("invalid_exact_lnbtc_request_mismatch");
        }
    }
    None
}

fn preimage_reason(preimage: Option<&str>) -> Result<&str, &'static str> {
    let Some(preimage) = preimage else {
        return Err("invalid_exact_lnbtc_preimage_missing");
    };
    if !preimage
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("invalid_exact_lnbtc_preimage_malformed");
    }
    if preimage.len() != 64 {
        return Err("invalid_exact_lnbtc_preimage_length");
    }
    Ok(preimage)
}

/// Stateless verification: the proof is cryptographically sound for these
/// exact terms. This is not consumption; call [`settle`] before execution.
pub fn verify(
    requirements: &PaymentRequirements,
    payload: &PaymentPayload,
    now: u64,
    skew: u64,
) -> Result<ValidatedPaymentProof, &'static str> {
    if let Some(reason) = field_mismatch(requirements, &payload.accepted) {
        return Err(reason);
    }
    let preimage = preimage_reason(payload.preimage())?;
    validate_paid_proof(
        requirements,
        &payload.accepted,
        preimage,
        now,
        skew,
        SupportedProfiles {
            http: true,
            mcp: false,
            native: false,
        },
    )
    .map_err(error_reason)
}

/// What a successful settlement admits. The caller may execute now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    pub proof: ValidatedPaymentProof,
    pub response: SettlementResponse,
}

/// Verify, then insert the replay key. A second settlement of the same
/// invoice returns `duplicate_settlement`; a store failure is reported as
/// such and never as success.
pub fn settle<S: ReplayStore + ?Sized>(
    store: &S,
    requirements: &PaymentRequirements,
    payload: &PaymentPayload,
    purchase: &str,
    now: u64,
    skew: u64,
) -> Result<Admission, SettlementResponse> {
    let proof = verify(requirements, payload, now, skew)
        .map_err(|reason| SettlementResponse::failed(&requirements.network, reason))?;
    let entry = ReplayEntry {
        key: proof.consumption_key.clone(),
        network: proof.network.clone(),
        payment_hash: proof.payment_hash.clone(),
        amount_msat: proof.invoice_amount_msat,
        consumed_at: now,
        retain_until: proof.retain_until,
        purchase: purchase.to_string(),
    };
    match store.insert(&entry) {
        Ok(()) => {}
        Err(ReplayError::Duplicate(_)) => {
            return Err(SettlementResponse::failed(
                &proof.network,
                DUPLICATE_SETTLEMENT,
            ));
        }
        Err(_) => {
            return Err(SettlementResponse::failed(
                &proof.network,
                "replay_store_unavailable",
            ));
        }
    }
    let response = SettlementResponse {
        success: true,
        error_reason: None,
        transaction: proof.payment_hash.clone(),
        network: proof.network.clone(),
        amount: Some(proof.invoice_amount_msat.to_string()),
    };
    Ok(Admission { proof, response })
}

/// A facilitator bound to one store and clock-skew allowance.
pub struct Facilitator<S: ReplayStore> {
    store: S,
    skew: u64,
}

impl<S: ReplayStore> Facilitator<S> {
    pub fn new(store: S, skew: u64) -> Self {
        Self { store, skew }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn settle(
        &self,
        requirements: &PaymentRequirements,
        payload: &PaymentPayload,
        purchase: &str,
        now: u64,
    ) -> Result<Admission, SettlementResponse> {
        settle(&self.store, requirements, payload, purchase, now, self.skew)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::FileReplayStore;
    use nostr::x402::MAINNET;
    use serde_json::{Map, json};

    // The pinned upstream vector also used by `crates/nostr`.
    const INVOICE: &str = "lnbc250n1pj48ugqpp54y3u9s8ylemsv8l3ewyzzu0klhujvuvmkl6llchq23vy8rzjsf0qsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygshp5p4nz8am4uqj4q8a87z3sk4x6yk4dv2mvel34epw68qqkwy0xcqvqxqzfvcqpjr4rx6ls6j5rpwknuea64evlk7yfx56wmqcer5eerekdsn9tlv6v4ex9mlz5dtm9qapl3svwlqcf7837dmjkru9z9w4h2rvm0md52w2sqxrwu5f";
    const PREIMAGE: &str = "0001020304050607080900010203040506070809000102030405060708090102";
    const HASH: &str = "0d6623f775e025501fa7f0a30b54da25aad62b6ccfe35c85da38016711e6c018";
    const PAYEE: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const NOW: u64 = 1_700_000_000;

    fn requirements() -> PaymentRequirements {
        serde_json::from_value(json!({"scheme":"exact","network":MAINNET,"amount":"25000","asset":"BTC","payTo":PAYEE,"maxTimeoutSeconds":300,"extra":{"assetTransferMethod":"bolt11","paymentFlow":"upfront","requestHash":HASH,"requestBindingProfile":"http:1","requestBindingParams":{"headers":[]},"invoice":INVOICE}})).unwrap()
    }

    fn payload(accepted: PaymentRequirements, preimage: Option<&str>) -> PaymentPayload {
        let mut map = Map::new();
        if let Some(preimage) = preimage {
            map.insert("preimage".into(), Value::String(preimage.into()));
        }
        PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted,
            payload: map,
            extensions: None,
        }
    }

    fn store() -> (FileReplayStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "x402-facilitator-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (FileReplayStore::open(&dir).unwrap(), dir)
    }

    #[test]
    fn settles_once_then_reports_duplicate() {
        let (store, dir) = store();
        let req = requirements();
        let pay = payload(req.clone(), Some(PREIMAGE));
        let admitted = settle(&store, &req, &pay, "purchase-1", NOW, 60).unwrap();
        assert!(admitted.response.success);
        assert_eq!(admitted.response.network, MAINNET);
        assert_eq!(admitted.response.transaction, admitted.proof.payment_hash);
        assert_eq!(admitted.response.amount.as_deref(), Some("25000"));
        assert_eq!(admitted.proof.retain_until, NOW + 300 + 60 + 3600);

        let again = settle(&store, &req, &pay, "purchase-2", NOW + 1, 60).unwrap_err();
        assert!(!again.success);
        assert_eq!(again.error_reason.as_deref(), Some(DUPLICATE_SETTLEMENT));
        assert_eq!(
            store
                .get(&admitted.proof.consumption_key)
                .unwrap()
                .unwrap()
                .purchase,
            "purchase-1"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refusals_use_the_upstream_vocabulary() {
        let req = requirements();
        let mut cases: Vec<(PaymentPayload, &str)> = vec![
            (
                payload(req.clone(), None),
                "invalid_exact_lnbtc_preimage_missing",
            ),
            (
                payload(req.clone(), Some("ZZ")),
                "invalid_exact_lnbtc_preimage_malformed",
            ),
            (
                payload(req.clone(), Some("00")),
                "invalid_exact_lnbtc_preimage_length",
            ),
            (
                payload(req.clone(), Some(&"00".repeat(32))),
                "invalid_exact_lnbtc_preimage_hash_mismatch",
            ),
        ];

        let mut accepted = req.clone();
        accepted.amount = "25001".into();
        cases.push((
            payload(accepted, Some(PREIMAGE)),
            "invalid_exact_lnbtc_amount_mismatch",
        ));

        let mut accepted = req.clone();
        accepted.pay_to = "03".repeat(33);
        cases.push((
            payload(accepted, Some(PREIMAGE)),
            "invalid_exact_lnbtc_pay_to_mismatch",
        ));

        let mut accepted = req.clone();
        accepted.network = nostr::x402::TESTNET.into();
        cases.push((payload(accepted, Some(PREIMAGE)), "network_mismatch"));

        let mut accepted = req.clone();
        accepted.max_timeout_seconds = 301;
        cases.push((
            payload(accepted, Some(PREIMAGE)),
            "invalid_exact_lnbtc_max_timeout_mismatch",
        ));

        let mut accepted = req.clone();
        accepted
            .extra
            .insert("requestHash".into(), Value::String("11".repeat(32)));
        cases.push((
            payload(accepted, Some(PREIMAGE)),
            "invalid_exact_lnbtc_request_mismatch",
        ));

        for (pay, reason) in cases {
            assert_eq!(verify(&req, &pay, NOW, 60).unwrap_err(), reason);
        }

        // The server-side expected hash differs from the signed one.
        let mut moved = req.clone();
        moved
            .extra
            .insert("requestHash".into(), Value::String("11".repeat(32)));
        let pay = payload(moved.clone(), Some(PREIMAGE));
        assert_eq!(
            verify(&moved, &pay, NOW, 60).unwrap_err(),
            "invalid_exact_lnbtc_invoice_request_mismatch"
        );

        let pay = payload(req.clone(), Some(PREIMAGE));
        assert_eq!(
            verify(&req, &pay, NOW + 300 + 61, 60).unwrap_err(),
            "invalid_exact_lnbtc_invoice_expired"
        );
        assert!(verify(&req, &pay, NOW + 300 + 60, 60).is_ok());
    }
}
