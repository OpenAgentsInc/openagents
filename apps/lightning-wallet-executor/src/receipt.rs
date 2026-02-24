use serde::Serialize;
use sha2::{Digest, Sha256};

const RECEIPT_VERSION: &str = "openagents.lightning.wallet_receipt.v1";
const RECEIPT_RAIL: &str = "lightning";
const RECEIPT_ASSET_ID: &str = "BTC_LN";

const INVOICE_RECEIPT_VERSION: &str = "openagents.lightning.invoice_receipt.v1";

#[derive(Debug, Clone)]
pub struct WalletExecutionReceiptInput {
    pub request_id: String,
    pub wallet_id: String,
    pub host: String,
    pub payment_id: String,
    pub invoice_hash: String,
    pub quoted_amount_msats: u64,
    pub settled_amount_msats: u64,
    pub preimage_hex: String,
    pub paid_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct InvoiceReceiptInput {
    pub request_id: String,
    pub wallet_id: String,
    pub invoice_hash: String,
    pub amount_msats: u64,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletExecutionReceipt {
    pub receipt_version: String,
    pub receipt_id: String,
    pub request_id: String,
    pub wallet_id: String,
    pub host: String,
    pub payment_id: String,
    pub invoice_hash: String,
    pub quoted_amount_msats: u64,
    pub settled_amount_msats: u64,
    pub preimage_sha256: String,
    pub paid_at_ms: i64,
    pub rail: String,
    pub asset_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceReceipt {
    pub receipt_version: String,
    pub receipt_id: String,
    pub request_id: String,
    pub wallet_id: String,
    pub invoice_hash: String,
    pub amount_msats: u64,
    pub created_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPayload {
    receipt_version: String,
    request_id: String,
    wallet_id: String,
    host: String,
    payment_id: String,
    invoice_hash: String,
    quoted_amount_msats: u64,
    settled_amount_msats: u64,
    preimage_sha256: String,
    paid_at_ms: i64,
    rail: String,
    asset_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalInvoicePayload {
    receipt_version: String,
    request_id: String,
    wallet_id: String,
    invoice_hash: String,
    amount_msats: u64,
    created_at_ms: i64,
    expires_at_ms: Option<i64>,
}

pub fn canonicalize_wallet_execution_receipt(input: &WalletExecutionReceiptInput) -> String {
    let payload = canonical_payload(input);
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

pub fn canonicalize_invoice_receipt(input: &InvoiceReceiptInput) -> String {
    let payload = canonical_invoice_payload(input);
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

pub fn canonical_wallet_execution_receipt_hash(input: &WalletExecutionReceiptInput) -> String {
    sha256_hex(canonicalize_wallet_execution_receipt(input).as_bytes())
}

pub fn canonical_invoice_receipt_hash(input: &InvoiceReceiptInput) -> String {
    sha256_hex(canonicalize_invoice_receipt(input).as_bytes())
}

pub fn build_wallet_execution_receipt(
    input: &WalletExecutionReceiptInput,
) -> WalletExecutionReceipt {
    let payload = canonical_payload(input);
    let canonical_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let canonical_json_sha256 = sha256_hex(canonical_json.as_bytes());

    WalletExecutionReceipt {
        receipt_version: payload.receipt_version,
        receipt_id: format!("lwr_{}", &canonical_json_sha256[..24]),
        request_id: payload.request_id,
        wallet_id: payload.wallet_id,
        host: payload.host,
        payment_id: payload.payment_id,
        invoice_hash: payload.invoice_hash,
        quoted_amount_msats: payload.quoted_amount_msats,
        settled_amount_msats: payload.settled_amount_msats,
        preimage_sha256: payload.preimage_sha256,
        paid_at_ms: payload.paid_at_ms,
        rail: payload.rail,
        asset_id: payload.asset_id,
        canonical_json_sha256,
    }
}

pub fn build_invoice_receipt(input: &InvoiceReceiptInput) -> InvoiceReceipt {
    let payload = canonical_invoice_payload(input);
    let canonical_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let canonical_json_sha256 = sha256_hex(canonical_json.as_bytes());

    InvoiceReceipt {
        receipt_version: payload.receipt_version,
        receipt_id: format!("lir_{}", &canonical_json_sha256[..24]),
        request_id: payload.request_id,
        wallet_id: payload.wallet_id,
        invoice_hash: payload.invoice_hash,
        amount_msats: payload.amount_msats,
        created_at_ms: payload.created_at_ms,
        expires_at_ms: payload.expires_at_ms,
        canonical_json_sha256,
    }
}

fn canonical_payload(input: &WalletExecutionReceiptInput) -> CanonicalPayload {
    let paid_at_ms = input.paid_at_ms.max(0);

    CanonicalPayload {
        receipt_version: RECEIPT_VERSION.to_string(),
        request_id: input.request_id.trim().to_string(),
        wallet_id: input.wallet_id.trim().to_string(),
        host: input.host.trim().to_ascii_lowercase(),
        payment_id: input.payment_id.trim().to_string(),
        invoice_hash: input.invoice_hash.trim().to_ascii_lowercase(),
        quoted_amount_msats: input.quoted_amount_msats,
        settled_amount_msats: input.settled_amount_msats,
        preimage_sha256: sha256_hex(input.preimage_hex.trim().to_ascii_lowercase().as_bytes()),
        paid_at_ms,
        rail: RECEIPT_RAIL.to_string(),
        asset_id: RECEIPT_ASSET_ID.to_string(),
    }
}

fn canonical_invoice_payload(input: &InvoiceReceiptInput) -> CanonicalInvoicePayload {
    let created_at_ms = input.created_at_ms.max(0);
    let expires_at_ms = input.expires_at_ms.map(|value| value.max(0));

    CanonicalInvoicePayload {
        receipt_version: INVOICE_RECEIPT_VERSION.to_string(),
        request_id: input.request_id.trim().to_string(),
        wallet_id: input.wallet_id.trim().to_string(),
        invoice_hash: input.invoice_hash.trim().to_ascii_lowercase(),
        amount_msats: input.amount_msats,
        created_at_ms,
        expires_at_ms,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_for_identical_payment_facts() {
        let input = WalletExecutionReceiptInput {
            request_id: "req-123".to_string(),
            wallet_id: "wallet-ep212".to_string(),
            host: "SATS4AI.COM".to_string(),
            payment_id: "pay-123".to_string(),
            invoice_hash: "ABCDEF1234".to_string(),
            quoted_amount_msats: 45_000,
            settled_amount_msats: 45_000,
            preimage_hex: "A".repeat(64),
            paid_at_ms: 1_777_000_000_000,
        };

        let first = build_wallet_execution_receipt(&input);
        let second = build_wallet_execution_receipt(&WalletExecutionReceiptInput {
            host: "sats4ai.com".to_string(),
            invoice_hash: "abcdef1234".to_string(),
            preimage_hex: "a".repeat(64),
            ..input
        });

        assert_eq!(first.canonical_json_sha256, second.canonical_json_sha256);
        assert_eq!(first.receipt_id, second.receipt_id);
        assert_eq!(first.host, "sats4ai.com");
        assert_eq!(first.invoice_hash, "abcdef1234");
        assert!(first.preimage_sha256.len() == 64);
        assert!(first.canonical_json_sha256.len() == 64);
        assert!(first.receipt_id.starts_with("lwr_"));
    }

    #[test]
    fn hash_changes_when_settled_amount_changes() {
        let baseline = canonical_wallet_execution_receipt_hash(&WalletExecutionReceiptInput {
            request_id: "req-200".to_string(),
            wallet_id: "wallet-ep212".to_string(),
            host: "sats4ai.com".to_string(),
            payment_id: "pay-200".to_string(),
            invoice_hash: "hash-200".to_string(),
            quoted_amount_msats: 50_000,
            settled_amount_msats: 50_000,
            preimage_hex: "b".repeat(64),
            paid_at_ms: 1_777_000_000_100,
        });

        let changed = canonical_wallet_execution_receipt_hash(&WalletExecutionReceiptInput {
            request_id: "req-200".to_string(),
            wallet_id: "wallet-ep212".to_string(),
            host: "sats4ai.com".to_string(),
            payment_id: "pay-200".to_string(),
            invoice_hash: "hash-200".to_string(),
            quoted_amount_msats: 50_000,
            settled_amount_msats: 49_000,
            preimage_hex: "b".repeat(64),
            paid_at_ms: 1_777_000_000_100,
        });

        assert_ne!(baseline, changed);
    }

    #[test]
    fn invoice_receipt_hash_is_stable_for_identical_facts() {
        let input = InvoiceReceiptInput {
            request_id: "req-invoice-1".to_string(),
            wallet_id: "wallet-ep212".to_string(),
            invoice_hash: "ABCDEF1234".to_string(),
            amount_msats: 42_000,
            created_at_ms: 1_777_000_000_000,
            expires_at_ms: Some(1_777_000_060_000),
        };

        let first = build_invoice_receipt(&input);
        let second = build_invoice_receipt(&InvoiceReceiptInput {
            invoice_hash: "abcdef1234".to_string(),
            ..input
        });

        assert_eq!(first.canonical_json_sha256, second.canonical_json_sha256);
        assert_eq!(first.receipt_id, second.receipt_id);
        assert_eq!(first.invoice_hash, "abcdef1234");
        assert!(first.receipt_id.starts_with("lir_"));
    }
}
