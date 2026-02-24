use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use openagents_l402::Bolt11;

use crate::artifacts::sign_receipt_sha256;
use crate::bridge::{
    BridgeNostrPublisher, LiquidityReceiptPointerV1, bridge_relays_from_env,
    build_liquidity_receipt_pointer_event,
};
use crate::liquidity::store::{LiquidityStore, LiquidityStoreError, PaymentFinalizeInput};
use crate::liquidity::types::{
    INVOICE_PAY_RECEIPT_SCHEMA_V1, InvoicePayReceiptV1, LIQUIDITY_STATUS_SCHEMA_V1,
    LiquidityPaymentRow, LiquidityQuoteRow, LiquidityReceiptRow, LiquidityStatusResponseV1,
    PAY_REQUEST_SCHEMA_V1, PAY_RESPONSE_SCHEMA_V1, PayRequestV1, PayResponseV1,
    QUOTE_PAY_REQUEST_SCHEMA_V1, QUOTE_PAY_RESPONSE_SCHEMA_V1, QuotePayRequestV1,
    QuotePayResponseV1,
};

#[derive(Debug, thiserror::Error)]
pub enum LiquidityError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl LiquidityError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::DependencyUnavailable(_) => "dependency_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message)
            | Self::Conflict(message)
            | Self::DependencyUnavailable(message)
            | Self::Internal(message) => message.clone(),
            Self::NotFound => "not found".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct LiquidityService {
    store: Arc<dyn LiquidityStore>,
    wallet_executor_base_url: Option<String>,
    wallet_executor_auth_token: Option<String>,
    wallet_executor_timeout_ms: u64,
    quote_ttl_seconds: u64,
    receipt_signing_key: Option<[u8; 32]>,
}

impl LiquidityService {
    pub fn new(
        store: Arc<dyn LiquidityStore>,
        wallet_executor_base_url: Option<String>,
        wallet_executor_auth_token: Option<String>,
        wallet_executor_timeout_ms: u64,
        quote_ttl_seconds: u64,
        receipt_signing_key: Option<[u8; 32]>,
    ) -> Self {
        Self {
            store,
            wallet_executor_base_url,
            wallet_executor_auth_token,
            wallet_executor_timeout_ms: wallet_executor_timeout_ms.max(250).min(120_000),
            quote_ttl_seconds: quote_ttl_seconds.max(5).min(3600),
            receipt_signing_key,
        }
    }

    // Idempotency contract:
    // - same idempotency_key + equivalent request fingerprint => replay existing quote
    // - same idempotency_key + different fingerprint => Conflict (HTTP 409 at handler layer)
    pub async fn quote_pay(
        &self,
        body: QuotePayRequestV1,
    ) -> Result<QuotePayResponseV1, LiquidityError> {
        if body.schema.trim() != QUOTE_PAY_REQUEST_SCHEMA_V1 {
            return Err(LiquidityError::InvalidRequest(format!(
                "schema must be {QUOTE_PAY_REQUEST_SCHEMA_V1}"
            )));
        }
        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(LiquidityError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }

        let invoice = body.invoice.trim().to_string();
        if invoice.is_empty() {
            return Err(LiquidityError::InvalidRequest(
                "invoice is required".to_string(),
            ));
        }
        let host = sanitize_host(&body.host);
        if host.is_empty() {
            return Err(LiquidityError::InvalidRequest(
                "host is required".to_string(),
            ));
        }

        let quoted_amount_msats = Bolt11::amount_msats(invoice.as_str()).ok_or_else(|| {
            LiquidityError::InvalidRequest(
                "invoice must include an amount (amountless invoices not supported)".to_string(),
            )
        })?;

        if quoted_amount_msats > body.max_amount_msats {
            return Err(LiquidityError::InvalidRequest(format!(
                "max_amount_msats {} is below invoice amount {}",
                body.max_amount_msats, quoted_amount_msats
            )));
        }

        let policy_context_json = body.policy_context;
        let policy_context_sha256 = canonical_sha256(&policy_context_json)
            .map_err(|error| LiquidityError::Internal(error))?;

        #[derive(Serialize)]
        struct QuoteFingerprint<'a> {
            schema: &'a str,
            invoice_hash: &'a str,
            host: &'a str,
            quoted_amount_msats: u64,
            max_amount_msats: u64,
            max_fee_msats: u64,
            urgency: Option<&'a str>,
            policy_context_sha256: &'a str,
        }

        let invoice_hash = hash_invoice(invoice.as_str());
        let request_fingerprint_sha256 = canonical_sha256(&QuoteFingerprint {
            schema: QUOTE_PAY_REQUEST_SCHEMA_V1,
            invoice_hash: invoice_hash.as_str(),
            host: host.as_str(),
            quoted_amount_msats,
            max_amount_msats: body.max_amount_msats,
            max_fee_msats: body.max_fee_msats,
            urgency: body
                .urgency
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty()),
            policy_context_sha256: policy_context_sha256.as_str(),
        })
        .map_err(|error| LiquidityError::Internal(error))?;

        let now = Utc::now();
        let valid_until = now + Duration::seconds(self.quote_ttl_seconds as i64);

        let quote = LiquidityQuoteRow {
            quote_id: format!("liq_quote_{}", uuid::Uuid::now_v7()),
            idempotency_key: idempotency_key.clone(),
            request_fingerprint_sha256,
            invoice,
            invoice_hash: invoice_hash.clone(),
            host: host.clone(),
            quoted_amount_msats,
            max_amount_msats: body.max_amount_msats,
            max_fee_msats: body.max_fee_msats,
            urgency: body
                .urgency
                .map(|value| value.trim().to_string())
                .filter(|v| !v.is_empty()),
            policy_context_json,
            policy_context_sha256: policy_context_sha256.clone(),
            valid_until,
            created_at: now,
        };

        let stored = self
            .store
            .create_or_get_quote(quote)
            .await
            .map_err(map_store_error)?;

        Ok(QuotePayResponseV1 {
            schema: QUOTE_PAY_RESPONSE_SCHEMA_V1.to_string(),
            quote_id: stored.quote_id,
            idempotency_key: stored.idempotency_key,
            invoice_hash: stored.invoice_hash,
            host: stored.host,
            quoted_amount_msats: stored.quoted_amount_msats,
            max_amount_msats: stored.max_amount_msats,
            max_fee_msats: stored.max_fee_msats,
            policy_context_sha256: stored.policy_context_sha256,
            valid_until: stored.valid_until,
            created_at: stored.created_at,
        })
    }

    pub async fn status(&self) -> LiquidityStatusResponseV1 {
        let mut response = LiquidityStatusResponseV1 {
            schema: LIQUIDITY_STATUS_SCHEMA_V1.to_string(),
            wallet_executor_configured: false,
            wallet_executor_reachable: false,
            receipt_signing_enabled: self.receipt_signing_key.is_some(),
            quote_ttl_seconds: self.quote_ttl_seconds,
            wallet_status: None,
            error_code: None,
            error_message: None,
        };

        let Some(base_url) = self
            .wallet_executor_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            response.error_code = Some("wallet_executor_not_configured".to_string());
            response.error_message = Some("wallet executor base url missing".to_string());
            return response;
        };
        let Some(token) = self
            .wallet_executor_auth_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            response.error_code = Some("wallet_executor_not_configured".to_string());
            response.error_message = Some("wallet executor auth token missing".to_string());
            return response;
        };

        response.wallet_executor_configured = true;
        let url = format!("{}/status", base_url.trim_end_matches('/'));
        match reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_millis(
                self.wallet_executor_timeout_ms,
            ))
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    let parsed = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| {
                        serde_json::json!({
                            "status": "ok",
                            "raw": body,
                        })
                    });
                    response.wallet_executor_reachable = true;
                    response.wallet_status = Some(parsed);
                } else {
                    response.error_code = Some(format!("wallet_executor_http_{}", status.as_u16()));
                    response.error_message = Some(if body.trim().is_empty() {
                        "wallet executor status request failed".to_string()
                    } else {
                        body
                    });
                }
            }
            Err(error) => {
                response.error_code = Some("wallet_executor_transport_error".to_string());
                response.error_message = Some(error.to_string());
            }
        }

        response
    }

    // Idempotency contract:
    // - first caller for a quote_id creates/finalizes payment lane
    // - concurrent caller while lane is in_flight => Conflict (HTTP 409)
    // - caller after finalization => deterministic replay of stored payment + receipt
    pub async fn pay(&self, body: PayRequestV1) -> Result<PayResponseV1, LiquidityError> {
        if body.schema.trim() != PAY_REQUEST_SCHEMA_V1 {
            return Err(LiquidityError::InvalidRequest(format!(
                "schema must be {PAY_REQUEST_SCHEMA_V1}"
            )));
        }

        let quote_id = body.quote_id.trim().to_string();
        if quote_id.is_empty() {
            return Err(LiquidityError::InvalidRequest(
                "quote_id is required".to_string(),
            ));
        }

        let quote = self
            .store
            .get_quote(quote_id.as_str())
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityError::NotFound)?;

        let now = Utc::now();
        if quote.valid_until < now {
            return Err(LiquidityError::InvalidRequest("quote expired".to_string()));
        }

        let run_id = body
            .run_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let trajectory_hash = body
            .trajectory_hash
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        #[derive(Serialize)]
        struct PayFingerprint<'a> {
            schema: &'a str,
            quote_id: &'a str,
            run_id: Option<&'a str>,
            trajectory_hash: Option<&'a str>,
        }

        let request_fingerprint_sha256 = canonical_sha256(&PayFingerprint {
            schema: PAY_REQUEST_SCHEMA_V1,
            quote_id: quote_id.as_str(),
            run_id: run_id.as_deref(),
            trajectory_hash: trajectory_hash.as_deref(),
        })
        .map_err(LiquidityError::Internal)?;

        let wallet_request_id = format!("liqpay:{quote_id}");

        let (payment, payment_created) = self
            .store
            .create_or_get_payment_in_flight(
                quote_id.as_str(),
                request_fingerprint_sha256.as_str(),
                run_id.clone(),
                trajectory_hash.clone(),
                wallet_request_id.as_str(),
                now,
            )
            .await
            .map_err(map_store_error)?;

        if payment.status != "in_flight" {
            return self
                .build_response_from_stored(quote_id.as_str(), &payment, quote)
                .await;
        }

        // Only the first caller should enter the external execution lane; everyone else should
        // retry once the in-flight row is finalized.
        if !payment_created {
            return Err(LiquidityError::Conflict(
                "payment already in flight for quote_id".to_string(),
            ));
        }

        let max_amount_forwarded_msats = quote.max_amount_msats.min(
            quote
                .quoted_amount_msats
                .saturating_add(quote.max_fee_msats),
        );
        if max_amount_forwarded_msats < quote.quoted_amount_msats {
            return Err(LiquidityError::InvalidRequest(
                "max_amount_msats/max_fee_msats yields max_amount_forwarded below invoice amount"
                    .to_string(),
            ));
        }

        let started = std::time::Instant::now();
        let (wallet_response_json, outcome) = self
            .wallet_executor_pay_bolt11(
                wallet_request_id.as_str(),
                quote.invoice.as_str(),
                max_amount_forwarded_msats,
                quote.host.as_str(),
            )
            .await;
        let latency_ms = started.elapsed().as_millis() as u64;
        let completed_at = Utc::now();

        let (status, wallet_receipt_sha256, preimage_sha256, paid_at_ms, error_code, error_message) =
            outcome;

        let receipt = build_invoice_pay_receipt(
            &quote,
            status.as_str(),
            max_amount_forwarded_msats,
            wallet_receipt_sha256.as_deref(),
            preimage_sha256.as_deref(),
            paid_at_ms,
            error_code.as_deref(),
            error_message.as_deref(),
            run_id.as_deref(),
            trajectory_hash.as_deref(),
            completed_at,
            self.receipt_signing_key.as_ref(),
        )?;

        let receipt_json = serde_json::to_value(&receipt)
            .map_err(|error| LiquidityError::Internal(error.to_string()))?;
        let signature_json = receipt
            .signature
            .as_ref()
            .map(|sig| serde_json::to_value(sig).unwrap_or(Value::Null))
            .filter(|value| !value.is_null());

        let store_receipt = LiquidityReceiptRow {
            quote_id: quote_id.clone(),
            schema: receipt.schema.clone(),
            canonical_json_sha256: receipt.canonical_json_sha256.clone(),
            signature_json,
            receipt_json: receipt_json.clone(),
            created_at: receipt.created_at,
        };

        let (updated_payment, _stored_receipt_row) = self
            .store
            .finalize_payment(PaymentFinalizeInput {
                quote_id: quote_id.clone(),
                status: status.clone(),
                completed_at,
                latency_ms,
                wallet_response_json: wallet_response_json.clone(),
                wallet_receipt_sha256: wallet_receipt_sha256.clone(),
                preimage_sha256: preimage_sha256.clone(),
                paid_at_ms,
                error_code: error_code.clone(),
                error_message: error_message.clone(),
                receipt: store_receipt,
            })
            .await
            .map_err(map_store_error)?;

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(&receipt);

        Ok(PayResponseV1 {
            schema: PAY_RESPONSE_SCHEMA_V1.to_string(),
            quote_id,
            status: updated_payment.status,
            wallet_receipt_sha256,
            preimage_sha256,
            paid_at_ms,
            error_code,
            error_message,
            receipt,
            payment_proof: wallet_response_json,
        })
    }

    fn maybe_spawn_nostr_liquidity_receipt_pointer_mirror(&self, receipt: &InvoicePayReceiptV1) {
        let relays = bridge_relays_from_env();
        if relays.is_empty() {
            return;
        }
        let Some(secret_key) = self.receipt_signing_key else {
            return;
        };

        let payload = LiquidityReceiptPointerV1 {
            receipt_id: receipt.receipt_id.clone(),
            pool_id: None,
            lp_id: None,
            deposit_id: None,
            withdrawal_id: None,
            quote_id: Some(receipt.quote_id.clone()),
            receipt_sha256: receipt.canonical_json_sha256.clone(),
            receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
        };

        tokio::spawn(async move {
            let event = match build_liquidity_receipt_pointer_event(&secret_key, None, &payload) {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!(reason = %error, "bridge nostr mirror failed to build liquidity receipt pointer");
                    return;
                }
            };
            let publisher = BridgeNostrPublisher::new(relays);
            if let Err(error) = publisher.connect().await {
                tracing::warn!(reason = %error, "bridge nostr mirror failed to connect to relays");
                return;
            }
            if let Err(error) = publisher.publish(&event).await {
                tracing::warn!(reason = %error, "bridge nostr mirror failed to publish liquidity receipt pointer");
            }
        });
    }

    async fn build_response_from_stored(
        &self,
        quote_id: &str,
        payment: &LiquidityPaymentRow,
        quote: LiquidityQuoteRow,
    ) -> Result<PayResponseV1, LiquidityError> {
        let receipt_row = self
            .store
            .get_receipt(quote_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| LiquidityError::Internal("missing receipt for quote_id".to_string()))?;

        let receipt: InvoicePayReceiptV1 = serde_json::from_value(receipt_row.receipt_json.clone())
            .map_err(|error| LiquidityError::Internal(format!("parse stored receipt: {error}")))?;

        Ok(PayResponseV1 {
            schema: PAY_RESPONSE_SCHEMA_V1.to_string(),
            quote_id: quote_id.to_string(),
            status: payment.status.clone(),
            wallet_receipt_sha256: payment.wallet_receipt_sha256.clone(),
            preimage_sha256: payment.preimage_sha256.clone(),
            paid_at_ms: payment.paid_at_ms,
            error_code: payment.error_code.clone(),
            error_message: payment.error_message.clone(),
            receipt,
            payment_proof: payment.wallet_response_json.clone().or_else(|| {
                Some(json!({
                    "schema": "openagents.liquidity.payment_proof_placeholder.v1",
                    "note": "payment_proof missing (older record)",
                    "quoteId": quote.quote_id,
                }))
            }),
        })
    }

    async fn wallet_executor_pay_bolt11(
        &self,
        request_id: &str,
        invoice: &str,
        max_amount_msats: u64,
        host: &str,
    ) -> (
        Option<Value>,
        (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
    ) {
        let Some(base_url) = self
            .wallet_executor_base_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return (
                None,
                (
                    "failed".to_string(),
                    None,
                    None,
                    None,
                    Some("wallet_executor_not_configured".to_string()),
                    Some("wallet executor base url missing".to_string()),
                ),
            );
        };
        let Some(token) = self
            .wallet_executor_auth_token
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return (
                None,
                (
                    "failed".to_string(),
                    None,
                    None,
                    None,
                    Some("wallet_executor_not_configured".to_string()),
                    Some("wallet executor auth token missing".to_string()),
                ),
            );
        };

        let url = format!("{}/pay-bolt11", base_url.trim_end_matches('/'));
        let resp = reqwest::Client::new()
            .post(url.as_str())
            .timeout(std::time::Duration::from_millis(
                self.wallet_executor_timeout_ms,
            ))
            .header("authorization", format!("Bearer {token}"))
            .json(&json!({
                "requestId": request_id,
                "payment": {
                    "invoice": invoice,
                    "maxAmountMsats": max_amount_msats,
                    "host": host,
                }
            }))
            .send()
            .await;

        match resp {
            Ok(resp) => {
                let status = resp.status();
                let json = resp.json::<Value>().await.unwrap_or(Value::Null);
                if status.is_success() {
                    let wallet_receipt_sha256 = json
                        .pointer("/result/receipt/canonicalJsonSha256")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let preimage_sha256 = json
                        .pointer("/result/receipt/preimageSha256")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let paid_at_ms = json
                        .pointer("/result/receipt/paidAtMs")
                        .and_then(Value::as_i64);

                    (
                        Some(json),
                        (
                            "succeeded".to_string(),
                            wallet_receipt_sha256,
                            preimage_sha256,
                            paid_at_ms,
                            None,
                            None,
                        ),
                    )
                } else {
                    let code = json
                        .pointer("/error/code")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("http_{}", status.as_u16()));
                    let message = json
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| "wallet executor error".to_string());
                    (
                        Some(json),
                        (
                            "failed".to_string(),
                            None,
                            None,
                            None,
                            Some(code),
                            Some(message),
                        ),
                    )
                }
            }
            Err(error) => (
                None,
                (
                    "failed".to_string(),
                    None,
                    None,
                    None,
                    Some("wallet_executor_transport_error".to_string()),
                    Some(error.to_string()),
                ),
            ),
        }
    }
}

fn map_store_error(error: LiquidityStoreError) -> LiquidityError {
    match error {
        LiquidityStoreError::Conflict(message) => LiquidityError::Conflict(message),
        LiquidityStoreError::NotFound(_) => LiquidityError::NotFound,
        LiquidityStoreError::Db(message) => LiquidityError::Internal(message),
    }
}

fn sanitize_host(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn hash_invoice(invoice: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(invoice.as_bytes());
    hex::encode(hasher.finalize())
}

fn canonical_sha256(value: &impl Serialize) -> Result<String, String> {
    let canonical_json =
        protocol::hash::canonical_json(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

fn build_invoice_pay_receipt(
    quote: &LiquidityQuoteRow,
    status: &str,
    max_amount_forwarded_msats: u64,
    wallet_receipt_sha256: Option<&str>,
    preimage_sha256: Option<&str>,
    paid_at_ms: Option<i64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
    run_id: Option<&str>,
    trajectory_hash: Option<&str>,
    created_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<InvoicePayReceiptV1, LiquidityError> {
    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        quote_id: &'a str,
        invoice_hash: &'a str,
        host: &'a str,
        quoted_amount_msats: u64,
        max_amount_msats: u64,
        max_fee_msats: u64,
        max_amount_forwarded_msats: u64,
        status: &'a str,
        wallet_receipt_sha256: Option<&'a str>,
        preimage_sha256: Option<&'a str>,
        paid_at_ms: Option<i64>,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
        policy_context_sha256: &'a str,
        run_id: Option<&'a str>,
        trajectory_hash: Option<&'a str>,
        created_at: &'a DateTime<Utc>,
    }

    let hash_input = ReceiptHashInput {
        schema: INVOICE_PAY_RECEIPT_SCHEMA_V1,
        quote_id: quote.quote_id.as_str(),
        invoice_hash: quote.invoice_hash.as_str(),
        host: quote.host.as_str(),
        quoted_amount_msats: quote.quoted_amount_msats,
        max_amount_msats: quote.max_amount_msats,
        max_fee_msats: quote.max_fee_msats,
        max_amount_forwarded_msats,
        status,
        wallet_receipt_sha256,
        preimage_sha256,
        paid_at_ms,
        error_code,
        error_message,
        policy_context_sha256: quote.policy_context_sha256.as_str(),
        run_id,
        trajectory_hash,
        created_at: &created_at,
    };

    let canonical_json_sha256 =
        canonical_sha256(&hash_input).map_err(|error| LiquidityError::Internal(error))?;
    let receipt_id = format!("lipr_{}", &canonical_json_sha256[..24]);

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(InvoicePayReceiptV1 {
        schema: INVOICE_PAY_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        quote_id: quote.quote_id.clone(),
        invoice_hash: quote.invoice_hash.clone(),
        host: quote.host.clone(),
        quoted_amount_msats: quote.quoted_amount_msats,
        max_amount_msats: quote.max_amount_msats,
        max_fee_msats: quote.max_fee_msats,
        max_amount_forwarded_msats,
        status: status.to_string(),
        wallet_receipt_sha256: wallet_receipt_sha256.map(str::to_string),
        preimage_sha256: preimage_sha256.map(str::to_string),
        paid_at_ms,
        error_code: error_code.map(str::to_string),
        error_message: error_message.map(str::to_string),
        policy_context_sha256: quote.policy_context_sha256.clone(),
        run_id: run_id.map(str::to_string),
        trajectory_hash: trajectory_hash.map(str::to_string),
        created_at,
        canonical_json_sha256,
        signature,
    })
}
