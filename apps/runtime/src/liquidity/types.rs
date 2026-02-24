use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::ReceiptSignatureV1;

pub const QUOTE_PAY_REQUEST_SCHEMA_V1: &str = "openagents.liquidity.quote_pay_request.v1";
pub const QUOTE_PAY_RESPONSE_SCHEMA_V1: &str = "openagents.liquidity.quote_pay_response.v1";
pub const PAY_REQUEST_SCHEMA_V1: &str = "openagents.liquidity.pay_request.v1";
pub const PAY_RESPONSE_SCHEMA_V1: &str = "openagents.liquidity.pay_response.v1";
pub const INVOICE_PAY_RECEIPT_SCHEMA_V1: &str = "openagents.liquidity.invoice_pay_receipt.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotePayRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub invoice: String,
    pub host: String,
    pub max_amount_msats: u64,
    pub max_fee_msats: u64,
    #[serde(default)]
    pub urgency: Option<String>,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotePayResponseV1 {
    pub schema: String,
    pub quote_id: String,
    pub idempotency_key: String,
    pub invoice_hash: String,
    pub host: String,
    pub quoted_amount_msats: u64,
    pub max_amount_msats: u64,
    pub max_fee_msats: u64,
    pub policy_context_sha256: String,
    pub valid_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayRequestV1 {
    pub schema: String,
    pub quote_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub trajectory_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayResponseV1 {
    pub schema: String,
    pub quote_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub receipt: InvoicePayReceiptV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_proof: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePayReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub quote_id: String,
    pub invoice_hash: String,
    pub host: String,
    pub quoted_amount_msats: u64,
    pub max_amount_msats: u64,
    pub max_fee_msats: u64,
    pub max_amount_forwarded_msats: u64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub policy_context_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone)]
pub struct LiquidityQuoteRow {
    pub quote_id: String,
    pub idempotency_key: String,
    pub request_fingerprint_sha256: String,
    pub invoice: String,
    pub invoice_hash: String,
    pub host: String,
    pub quoted_amount_msats: u64,
    pub max_amount_msats: u64,
    pub max_fee_msats: u64,
    pub urgency: Option<String>,
    pub policy_context_json: Value,
    pub policy_context_sha256: String,
    pub valid_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LiquidityPaymentRow {
    pub quote_id: String,
    pub status: String,
    pub request_fingerprint_sha256: String,
    pub run_id: Option<String>,
    pub trajectory_hash: Option<String>,
    pub wallet_request_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub latency_ms: Option<u64>,
    pub wallet_response_json: Option<Value>,
    pub wallet_receipt_sha256: Option<String>,
    pub preimage_sha256: Option<String>,
    pub paid_at_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LiquidityReceiptRow {
    pub quote_id: String,
    pub schema: String,
    pub canonical_json_sha256: String,
    pub signature_json: Option<Value>,
    pub receipt_json: Value,
    pub created_at: DateTime<Utc>,
}
