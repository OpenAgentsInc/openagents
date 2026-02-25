use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PAYMENT_ATTEMPT_RECEIPT_SCHEMA_V1: &str = "openagents.neobank.payment_attempt_receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentRouteKind {
    DirectLiquidity,
    CepEnvelope,
}

impl PaymentRouteKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectLiquidity => "direct_liquidity",
            Self::CepEnvelope => "cep_envelope",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeobankReceiptSignatureV1 {
    pub schema: String,
    pub scheme: String,
    pub signer: String,
    pub signed_sha256: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAttemptReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub idempotency_key: String,
    pub route_kind: String,
    pub status: String,
    pub invoice_hash: String,
    pub host: String,
    pub quoted_amount_msats: u64,
    pub max_fee_msats: u64,
    pub policy_context_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_quote_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_offer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_envelope_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_settlement_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_decision_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing_policy_notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_liquidity_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<NeobankReceiptSignatureV1>,
}

#[derive(Debug, Clone)]
pub struct PaymentAttemptReceiptInput {
    pub idempotency_key: String,
    pub route_kind: PaymentRouteKind,
    pub status: String,
    pub invoice_hash: String,
    pub host: String,
    pub quoted_amount_msats: u64,
    pub max_fee_msats: u64,
    pub policy_context_sha256: String,
    pub run_id: Option<String>,
    pub trajectory_hash: Option<String>,
    pub liquidity_quote_id: Option<String>,
    pub liquidity_receipt_sha256: Option<String>,
    pub credit_offer_id: Option<String>,
    pub credit_envelope_id: Option<String>,
    pub credit_settlement_receipt_sha256: Option<String>,
    pub routing_decision_sha256: Option<String>,
    pub routing_policy_notes: Vec<String>,
    pub routing_confidence: Option<f64>,
    pub routing_liquidity_score: Option<f64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub trait ReceiptSigner: Send + Sync {
    fn sign(&self, receipt_sha256_hex: &str) -> Result<NeobankReceiptSignatureV1, ReceiptError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptError {
    #[error("hash error: {0}")]
    Hash(String),
    #[error("sign error: {0}")]
    Sign(String),
}

pub fn canonical_sha256(value: &impl Serialize) -> Result<String, ReceiptError> {
    let canonical_json = protocol::hash::canonical_json(value)
        .map_err(|error| ReceiptError::Hash(error.to_string()))?;
    let digest = Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

pub fn build_payment_attempt_receipt(
    input: PaymentAttemptReceiptInput,
    signer: Option<&dyn ReceiptSigner>,
) -> Result<PaymentAttemptReceiptV1, ReceiptError> {
    #[derive(Serialize)]
    struct HashInput<'a> {
        schema: &'a str,
        idempotency_key: &'a str,
        route_kind: &'a str,
        status: &'a str,
        invoice_hash: &'a str,
        host: &'a str,
        quoted_amount_msats: u64,
        max_fee_msats: u64,
        policy_context_sha256: &'a str,
        run_id: Option<&'a str>,
        trajectory_hash: Option<&'a str>,
        liquidity_quote_id: Option<&'a str>,
        liquidity_receipt_sha256: Option<&'a str>,
        credit_offer_id: Option<&'a str>,
        credit_envelope_id: Option<&'a str>,
        credit_settlement_receipt_sha256: Option<&'a str>,
        routing_decision_sha256: Option<&'a str>,
        routing_policy_notes: &'a [String],
        routing_confidence: Option<f64>,
        routing_liquidity_score: Option<f64>,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
        created_at: &'a DateTime<Utc>,
    }

    let route_kind = input.route_kind.as_str();
    let hash_input = HashInput {
        schema: PAYMENT_ATTEMPT_RECEIPT_SCHEMA_V1,
        idempotency_key: input.idempotency_key.as_str(),
        route_kind,
        status: input.status.as_str(),
        invoice_hash: input.invoice_hash.as_str(),
        host: input.host.as_str(),
        quoted_amount_msats: input.quoted_amount_msats,
        max_fee_msats: input.max_fee_msats,
        policy_context_sha256: input.policy_context_sha256.as_str(),
        run_id: input.run_id.as_deref(),
        trajectory_hash: input.trajectory_hash.as_deref(),
        liquidity_quote_id: input.liquidity_quote_id.as_deref(),
        liquidity_receipt_sha256: input.liquidity_receipt_sha256.as_deref(),
        credit_offer_id: input.credit_offer_id.as_deref(),
        credit_envelope_id: input.credit_envelope_id.as_deref(),
        credit_settlement_receipt_sha256: input.credit_settlement_receipt_sha256.as_deref(),
        routing_decision_sha256: input.routing_decision_sha256.as_deref(),
        routing_policy_notes: &input.routing_policy_notes,
        routing_confidence: input.routing_confidence,
        routing_liquidity_score: input.routing_liquidity_score,
        error_code: input.error_code.as_deref(),
        error_message: input.error_message.as_deref(),
        created_at: &input.created_at,
    };

    let canonical_json_sha256 = canonical_sha256(&hash_input)?;
    let receipt_id = format!("nbr_{}", &canonical_json_sha256[..24]);

    let signature = match signer {
        Some(signer) => Some(signer.sign(canonical_json_sha256.as_str())?),
        None => None,
    };

    Ok(PaymentAttemptReceiptV1 {
        schema: PAYMENT_ATTEMPT_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        idempotency_key: input.idempotency_key,
        route_kind: route_kind.to_string(),
        status: input.status,
        invoice_hash: input.invoice_hash,
        host: input.host,
        quoted_amount_msats: input.quoted_amount_msats,
        max_fee_msats: input.max_fee_msats,
        policy_context_sha256: input.policy_context_sha256,
        run_id: input.run_id,
        trajectory_hash: input.trajectory_hash,
        liquidity_quote_id: input.liquidity_quote_id,
        liquidity_receipt_sha256: input.liquidity_receipt_sha256,
        credit_offer_id: input.credit_offer_id,
        credit_envelope_id: input.credit_envelope_id,
        credit_settlement_receipt_sha256: input.credit_settlement_receipt_sha256,
        routing_decision_sha256: input.routing_decision_sha256,
        routing_policy_notes: input.routing_policy_notes,
        routing_confidence: input.routing_confidence,
        routing_liquidity_score: input.routing_liquidity_score,
        error_code: input.error_code,
        error_message: input.error_message,
        created_at: input.created_at,
        canonical_json_sha256,
        signature,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{PaymentAttemptReceiptInput, PaymentRouteKind, build_payment_attempt_receipt};

    #[test]
    fn receipt_hash_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let now = Utc::now();
        let first = build_payment_attempt_receipt(
            PaymentAttemptReceiptInput {
                idempotency_key: "idem-1".to_string(),
                route_kind: PaymentRouteKind::DirectLiquidity,
                status: "succeeded".to_string(),
                invoice_hash: "abc".to_string(),
                host: "example.com".to_string(),
                quoted_amount_msats: 1000,
                max_fee_msats: 100,
                policy_context_sha256: "policy".to_string(),
                run_id: Some("run_1".to_string()),
                trajectory_hash: Some("traj_1".to_string()),
                liquidity_quote_id: Some("quote_1".to_string()),
                liquidity_receipt_sha256: Some("liq_1".to_string()),
                credit_offer_id: None,
                credit_envelope_id: None,
                credit_settlement_receipt_sha256: None,
                routing_decision_sha256: None,
                routing_policy_notes: Vec::new(),
                routing_confidence: None,
                routing_liquidity_score: None,
                error_code: None,
                error_message: None,
                created_at: now,
            },
            None,
        )?;
        let second = build_payment_attempt_receipt(
            PaymentAttemptReceiptInput {
                idempotency_key: "idem-1".to_string(),
                route_kind: PaymentRouteKind::DirectLiquidity,
                status: "succeeded".to_string(),
                invoice_hash: "abc".to_string(),
                host: "example.com".to_string(),
                quoted_amount_msats: 1000,
                max_fee_msats: 100,
                policy_context_sha256: "policy".to_string(),
                run_id: Some("run_1".to_string()),
                trajectory_hash: Some("traj_1".to_string()),
                liquidity_quote_id: Some("quote_1".to_string()),
                liquidity_receipt_sha256: Some("liq_1".to_string()),
                credit_offer_id: None,
                credit_envelope_id: None,
                credit_settlement_receipt_sha256: None,
                routing_decision_sha256: None,
                routing_policy_notes: Vec::new(),
                routing_confidence: None,
                routing_liquidity_score: None,
                error_code: None,
                error_message: None,
                created_at: now,
            },
            None,
        )?;
        assert_eq!(first.canonical_json_sha256, second.canonical_json_sha256);
        assert_eq!(first.receipt_id, second.receipt_id);
        Ok(())
    }
}
