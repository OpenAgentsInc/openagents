use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeClientError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("api error ({status}): {code}: {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Deserialize)]
struct RuntimeErrorEnvelope {
    error: RuntimeErrorBody,
}

#[derive(Debug, Deserialize)]
struct RuntimeErrorBody {
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeInternalApiClient {
    base_url: String,
    auth_token: Option<String>,
    http: reqwest::Client,
}

impl RuntimeInternalApiClient {
    #[must_use]
    pub fn new(base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            auth_token: auth_token
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            http: reqwest::Client::new(),
        }
    }

    pub async fn quote_pay(
        &self,
        body: LiquidityQuotePayRequestV1,
    ) -> Result<LiquidityQuotePayResponseV1, RuntimeClientError> {
        self.post_json("/internal/v1/liquidity/quote_pay", &body)
            .await
    }

    pub async fn pay(
        &self,
        body: LiquidityPayRequestV1,
    ) -> Result<LiquidityPayResponseV1, RuntimeClientError> {
        self.post_json("/internal/v1/liquidity/pay", &body).await
    }

    pub async fn credit_offer(
        &self,
        body: CreditOfferRequestV1,
    ) -> Result<CreditOfferResponseV1, RuntimeClientError> {
        self.post_json("/internal/v1/credit/offer", &body).await
    }

    pub async fn credit_envelope(
        &self,
        body: CreditEnvelopeRequestV1,
    ) -> Result<CreditEnvelopeResponseV1, RuntimeClientError> {
        self.post_json("/internal/v1/credit/envelope", &body).await
    }

    pub async fn credit_settle(
        &self,
        body: CreditSettleRequestV1,
    ) -> Result<CreditSettleResponseV1, RuntimeClientError> {
        self.post_json("/internal/v1/credit/settle", &body).await
    }

    async fn post_json<TReq, TRes>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes, RuntimeClientError>
    where
        TReq: Serialize + ?Sized,
        TRes: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut request = self.http.post(url).json(body);
        if let Some(token) = self.auth_token.as_ref() {
            request = request.header("authorization", format!("Bearer {token}"));
        }

        let response = request
            .send()
            .await
            .map_err(|error| RuntimeClientError::Transport(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let status_u16 = status.as_u16();
            let body_bytes = response
                .bytes()
                .await
                .map_err(|error| RuntimeClientError::Transport(error.to_string()))?;
            if let Ok(parsed) = serde_json::from_slice::<RuntimeErrorEnvelope>(&body_bytes) {
                return Err(RuntimeClientError::Api {
                    status: status_u16,
                    code: parsed.error.code,
                    message: parsed.error.message,
                });
            }
            let text = String::from_utf8_lossy(body_bytes.as_ref()).to_string();
            return Err(RuntimeClientError::Api {
                status: status_u16,
                code: "runtime_error".to_string(),
                message: text,
            });
        }

        response
            .json::<TRes>()
            .await
            .map_err(|error| RuntimeClientError::Parse(error.to_string()))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LiquidityQuotePayRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub invoice: String,
    pub host: String,
    pub max_amount_msats: u64,
    pub max_fee_msats: u64,
    pub urgency: Option<String>,
    pub policy_context: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiquidityQuotePayResponseV1 {
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

#[derive(Debug, Clone, Serialize)]
pub struct LiquidityPayRequestV1 {
    pub schema: String,
    pub quote_id: String,
    pub run_id: Option<String>,
    pub trajectory_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiquidityPayResponseV1 {
    pub schema: String,
    pub quote_id: String,
    pub status: String,
    pub wallet_receipt_sha256: Option<String>,
    pub preimage_sha256: Option<String>,
    pub paid_at_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub receipt: LiquidityInvoicePayReceiptV1,
    pub payment_proof: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiquidityInvoicePayReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditOfferRequestV1 {
    pub schema: String,
    pub agent_id: String,
    pub pool_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: u64,
    pub fee_bps: u32,
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditOfferResponseV1 {
    pub schema: String,
    pub offer: CreditOfferRowV1,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditOfferRowV1 {
    pub offer_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub fee_bps: i32,
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditEnvelopeRequestV1 {
    pub schema: String,
    pub offer_id: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditEnvelopeResponseV1 {
    pub schema: String,
    pub envelope: CreditEnvelopeRowV1,
    pub receipt: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditEnvelopeRowV1 {
    pub envelope_id: String,
    pub offer_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub fee_bps: i32,
    pub exp: DateTime<Utc>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditSettleRequestV1 {
    pub schema: String,
    pub envelope_id: String,
    pub verification_passed: bool,
    pub verification_receipt_sha256: String,
    pub provider_invoice: String,
    pub provider_host: String,
    pub max_fee_msats: u64,
    pub policy_context: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditSettleResponseV1 {
    pub schema: String,
    pub envelope_id: String,
    pub outcome: String,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub verification_receipt_sha256: String,
    pub liquidity_receipt_sha256: Option<String>,
    pub receipt: Value,
}
