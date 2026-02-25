use chrono::{DateTime, Utc};
use openagents_proto::hydra_credit::HydraCreditConversionError;
pub use openagents_proto::hydra_credit::{
    CreditEnvelopeRequestV1, CreditEnvelopeResponseV1, CreditOfferRequestV1, CreditOfferResponseV1,
    CreditScopeTypeV1, CreditSettleRequestV1, CreditSettleResponseV1,
};
use openagents_proto::hydra_routing::HydraRoutingConversionError;
pub use openagents_proto::hydra_routing::{
    RoutingCandidateQuoteV1, RoutingScoreRequestV1, RoutingScoreResponseV1,
};
use openagents_proto::wire::openagents::hydra::v1 as wire_hydra;
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
    #[error("contract error: {0}")]
    Contract(String),
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
        let body = normalize_credit_offer_request(body)?;
        let response = self.post_json("/internal/v1/credit/offer", &body).await?;
        normalize_credit_offer_response(response)
    }

    pub async fn credit_envelope(
        &self,
        body: CreditEnvelopeRequestV1,
    ) -> Result<CreditEnvelopeResponseV1, RuntimeClientError> {
        let body = normalize_credit_envelope_request(body)?;
        let response = self
            .post_json("/internal/v1/credit/envelope", &body)
            .await?;
        normalize_credit_envelope_response(response)
    }

    pub async fn credit_settle(
        &self,
        body: CreditSettleRequestV1,
    ) -> Result<CreditSettleResponseV1, RuntimeClientError> {
        let body = normalize_credit_settle_request(body)?;
        let response = self.post_json("/internal/v1/credit/settle", &body).await?;
        normalize_credit_settle_response(response)
    }

    pub async fn hydra_routing_score(
        &self,
        body: RoutingScoreRequestV1,
    ) -> Result<RoutingScoreResponseV1, RuntimeClientError> {
        let body = normalize_routing_score_request(body)?;
        let response = self
            .post_json("/internal/v1/hydra/routing/score", &body)
            .await?;
        normalize_routing_score_response(response)
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

fn map_credit_contract_error(error: HydraCreditConversionError) -> RuntimeClientError {
    RuntimeClientError::Contract(error.to_string())
}

fn map_routing_contract_error(error: HydraRoutingConversionError) -> RuntimeClientError {
    RuntimeClientError::Contract(error.to_string())
}

fn normalize_credit_offer_request(
    request: CreditOfferRequestV1,
) -> Result<CreditOfferRequestV1, RuntimeClientError> {
    let wire: wire_hydra::CreditOfferRequestV1 =
        request.try_into().map_err(map_credit_contract_error)?;
    CreditOfferRequestV1::try_from(wire).map_err(map_credit_contract_error)
}

fn normalize_credit_envelope_request(
    request: CreditEnvelopeRequestV1,
) -> Result<CreditEnvelopeRequestV1, RuntimeClientError> {
    let wire: wire_hydra::CreditEnvelopeRequestV1 = request.into();
    Ok(CreditEnvelopeRequestV1::from(wire))
}

fn normalize_credit_settle_request(
    request: CreditSettleRequestV1,
) -> Result<CreditSettleRequestV1, RuntimeClientError> {
    let wire: wire_hydra::CreditSettleRequestV1 =
        request.try_into().map_err(map_credit_contract_error)?;
    CreditSettleRequestV1::try_from(wire).map_err(map_credit_contract_error)
}

fn normalize_credit_offer_response(
    response: CreditOfferResponseV1,
) -> Result<CreditOfferResponseV1, RuntimeClientError> {
    let wire: wire_hydra::CreditOfferResponseV1 =
        response.try_into().map_err(map_credit_contract_error)?;
    CreditOfferResponseV1::try_from(wire).map_err(map_credit_contract_error)
}

fn normalize_credit_envelope_response(
    response: CreditEnvelopeResponseV1,
) -> Result<CreditEnvelopeResponseV1, RuntimeClientError> {
    let wire: wire_hydra::CreditEnvelopeResponseV1 =
        response.try_into().map_err(map_credit_contract_error)?;
    CreditEnvelopeResponseV1::try_from(wire).map_err(map_credit_contract_error)
}

fn normalize_credit_settle_response(
    response: CreditSettleResponseV1,
) -> Result<CreditSettleResponseV1, RuntimeClientError> {
    let wire: wire_hydra::CreditSettleResponseV1 =
        response.try_into().map_err(map_credit_contract_error)?;
    CreditSettleResponseV1::try_from(wire).map_err(map_credit_contract_error)
}

fn normalize_routing_score_request(
    request: RoutingScoreRequestV1,
) -> Result<RoutingScoreRequestV1, RuntimeClientError> {
    let wire: wire_hydra::RoutingScoreRequestV1 =
        request.try_into().map_err(map_routing_contract_error)?;
    RoutingScoreRequestV1::try_from(wire).map_err(map_routing_contract_error)
}

fn normalize_routing_score_response(
    response: RoutingScoreResponseV1,
) -> Result<RoutingScoreResponseV1, RuntimeClientError> {
    let wire: wire_hydra::RoutingScoreResponseV1 =
        response.try_into().map_err(map_routing_contract_error)?;
    RoutingScoreResponseV1::try_from(wire).map_err(map_routing_contract_error)
}
