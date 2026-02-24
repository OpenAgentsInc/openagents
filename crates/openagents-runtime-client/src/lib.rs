use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_TIMEOUT_MS: u64 = 1_500;
pub const DEFAULT_REQUEST_ATTEMPTS: usize = 2;

#[derive(Debug, Clone)]
pub struct RuntimeInternalClientConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub request_attempts: usize,
}

impl RuntimeInternalClientConfig {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            request_attempts: DEFAULT_REQUEST_ATTEMPTS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeInternalClient {
    base_url: String,
    timeout: Duration,
    request_attempts: usize,
    http: reqwest::Client,
}

#[derive(Debug, Error)]
pub enum RuntimeClientError {
    #[error("runtime_client_base_url_missing")]
    BaseUrlMissing,
    #[error("runtime_client_invalid_path")]
    InvalidPath,
    #[error("runtime_request_failed:{message}")]
    Request { message: String },
    #[error("runtime_read_failed:{message}")]
    Read { message: String },
    #[error("runtime_http_{status}:{body}")]
    Http { status: StatusCode, body: String },
    #[error("runtime_json_decode_failed:{message}")]
    Decode { message: String },
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeWorkersListResponse {
    pub workers: Vec<ComputeRuntimeWorkerSnapshot>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeWorkerSnapshot {
    pub worker: ComputeRuntimeWorkerRecord,
    pub liveness: ComputeRuntimeWorkerLivenessRecord,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeWorkerRecord {
    pub worker_id: String,
    pub status: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeWorkerLivenessRecord {
    pub heartbeat_state: String,
    pub heartbeat_age_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeProviderCatalogResponse {
    pub providers: Vec<ComputeRuntimeProviderCatalogEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeProviderCatalogEntry {
    pub provider_id: String,
    pub worker_id: String,
    pub supply_class: String,
    #[serde(default)]
    pub reserve_pool: bool,
    pub status: String,
    pub heartbeat_state: String,
    pub heartbeat_age_ms: Option<i64>,
    pub min_price_msats: Option<u64>,
    #[serde(default)]
    pub quarantined: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeTelemetryResponse {
    pub provider_eligible_owned: usize,
    pub provider_eligible_reserve: usize,
    pub provider_eligible_total: usize,
    pub dispatch: ComputeRuntimeOwnerTelemetrySnapshot,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeOwnerTelemetrySnapshot {
    pub dispatch_total: u64,
    pub dispatch_not_found: u64,
    pub dispatch_errors: u64,
    pub dispatch_fallbacks: u64,
    pub latency_ms_avg: Option<u64>,
    pub latency_ms_p50: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeTreasurySummary {
    pub account: ComputeRuntimeBudgetAccount,
    pub released_msats_total: u64,
    pub released_count: u64,
    pub withheld_count: u64,
    #[serde(default)]
    pub provider_earnings: Vec<ComputeRuntimeProviderEarningsEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeBudgetAccount {
    pub limit_msats: u64,
    pub reserved_msats: u64,
    pub spent_msats: u64,
}

#[derive(Debug, Deserialize)]
pub struct ComputeRuntimeProviderEarningsEntry {
    pub provider_id: String,
    pub earned_msats: u64,
}

#[derive(Debug, Serialize)]
pub struct RuntimeWorkerRegisterRequest {
    pub worker_id: Option<String>,
    pub owner_user_id: u64,
    pub workspace_ref: Option<String>,
    pub codex_home_ref: Option<String>,
    pub adapter: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RuntimeWorkerHeartbeatRequest {
    pub owner_user_id: u64,
    pub metadata_patch: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RuntimeWorkerStatusTransitionRequest {
    pub owner_user_id: u64,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct RuntimeWorkerTransitionRequest {
    pub owner_user_id: u64,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimePoolRow {
    pub pool_kind: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct RuntimePoolStatusResponseV1 {
    pub pool: RuntimePoolRow,
    pub share_price_sats: i64,
    pub total_shares: i64,
    pub pending_withdrawals_sats_estimate: i64,
}

#[derive(Debug, Deserialize)]
pub struct RuntimePoolSnapshotRowV1 {
    pub snapshot_id: String,
    pub as_of: DateTime<Utc>,
    #[serde(default)]
    pub assets_json: serde_json::Value,
    pub canonical_json_sha256: String,
    #[serde(default)]
    pub signature_json: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimePoolSnapshotResponseV1 {
    pub snapshot: RuntimePoolSnapshotRowV1,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditCircuitBreakersV1 {
    pub halt_new_envelopes: bool,
    pub halt_large_settlements: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditPolicySnapshotV1 {
    pub max_sats_per_envelope: u64,
    pub max_outstanding_envelopes_per_agent: u64,
    pub max_offer_ttl_seconds: u64,
    pub underwriting_history_days: i64,
    pub underwriting_base_sats: u64,
    pub underwriting_k: f64,
    pub underwriting_default_penalty_multiplier: f64,
    pub min_fee_bps: u32,
    pub max_fee_bps: u32,
    pub fee_risk_scaler: f64,
    pub health_window_seconds: i64,
    pub health_settlement_sample_limit: u32,
    pub health_ln_pay_sample_limit: u32,
    pub circuit_breaker_min_sample: u64,
    pub loss_rate_halt_threshold: f64,
    pub ln_failure_rate_halt_threshold: f64,
    pub ln_failure_large_settlement_cap_sats: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditHealthResponseV1 {
    pub schema: String,
    pub generated_at: DateTime<Utc>,
    pub open_envelope_count: u64,
    pub open_reserved_commitments_sats: u64,
    pub settlement_sample: u64,
    pub loss_count: u64,
    pub loss_rate: f64,
    pub ln_pay_sample: u64,
    pub ln_fail_count: u64,
    pub ln_failure_rate: f64,
    pub breakers: RuntimeCreditCircuitBreakersV1,
    pub policy: RuntimeCreditPolicySnapshotV1,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCreditScopeTypeV1 {
    Nip90,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeCreditIntentRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub agent_id: String,
    pub scope_type: RuntimeCreditScopeTypeV1,
    pub scope_id: String,
    pub max_sats: u64,
    pub exp: DateTime<Utc>,
    #[serde(default)]
    pub policy_context: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditIntentResponseV1 {
    pub schema: String,
    pub intent: RuntimeCreditIntentRowV1,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditIntentRowV1 {
    pub intent_id: String,
    pub idempotency_key: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub exp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeCreditOfferRequestV1 {
    pub schema: String,
    pub agent_id: String,
    pub pool_id: String,
    #[serde(default)]
    pub intent_id: Option<String>,
    pub scope_type: RuntimeCreditScopeTypeV1,
    pub scope_id: String,
    pub max_sats: u64,
    pub fee_bps: u32,
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditOfferResponseV1 {
    pub schema: String,
    pub offer: RuntimeCreditOfferRowV1,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditOfferRowV1 {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeCreditEnvelopeRequestV1 {
    pub schema: String,
    pub offer_id: String,
    pub provider_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditEnvelopeResponseV1 {
    pub schema: String,
    pub envelope: RuntimeCreditEnvelopeRowV1,
    pub receipt: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditEnvelopeRowV1 {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeCreditSettleRequestV1 {
    pub schema: String,
    pub envelope_id: String,
    pub verification_passed: bool,
    pub verification_receipt_sha256: String,
    pub provider_invoice: String,
    pub provider_host: String,
    pub max_fee_msats: u64,
    #[serde(default)]
    pub policy_context: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditSettleResponseV1 {
    pub schema: String,
    pub envelope_id: String,
    pub settlement_id: String,
    pub outcome: String,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub verification_receipt_sha256: String,
    pub liquidity_receipt_sha256: Option<String>,
    pub receipt: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeCreditAgentExposureResponseV1 {
    pub schema: String,
    pub agent_id: String,
    pub open_envelope_count: u64,
    pub open_exposure_sats: u64,
    pub settled_count_30d: u64,
    pub success_volume_sats_30d: u64,
    pub pass_rate_30d: f64,
    pub loss_count_30d: u64,
    pub underwriting_limit_sats: u64,
    pub underwriting_fee_bps: u32,
    pub requires_verifier: bool,
    pub computed_at: DateTime<Utc>,
}

impl RuntimeInternalClient {
    pub fn new(config: RuntimeInternalClientConfig) -> Result<Self, RuntimeClientError> {
        let base_url = normalize_base_url(&config.base_url)?;
        Ok(Self {
            base_url,
            timeout: Duration::from_millis(config.timeout_ms.max(250)),
            request_attempts: config.request_attempts.max(1),
            http: reqwest::Client::new(),
        })
    }

    pub fn from_base_url(
        base_url: Option<&str>,
        timeout_ms: u64,
    ) -> Result<Self, RuntimeClientError> {
        let base_url = base_url.unwrap_or_default();
        let mut config = RuntimeInternalClientConfig::new(base_url);
        config.timeout_ms = timeout_ms;
        Self::new(config)
    }

    #[must_use]
    pub fn endpoint(&self, path: &str) -> Option<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.starts_with('/') {
            Some(format!("{}{}", self.base_url, trimmed))
        } else {
            Some(format!("{}/{}", self.base_url, trimmed))
        }
    }

    #[must_use]
    pub fn workers_path(owner_user_id: u64) -> String {
        format!("/internal/v1/workers?owner_user_id={owner_user_id}")
    }

    #[must_use]
    pub fn worker_path(worker_id: &str, owner_user_id: u64) -> String {
        format!(
            "/internal/v1/workers/{}?owner_user_id={owner_user_id}",
            worker_id.trim()
        )
    }

    #[must_use]
    pub fn worker_heartbeat_path(worker_id: &str) -> String {
        format!("/internal/v1/workers/{}/heartbeat", worker_id.trim())
    }

    #[must_use]
    pub fn worker_status_path(worker_id: &str) -> String {
        format!("/internal/v1/workers/{}/status", worker_id.trim())
    }

    #[must_use]
    pub fn provider_catalog_path(owner_user_id: u64) -> String {
        format!("/internal/v1/marketplace/catalog/providers?owner_user_id={owner_user_id}")
    }

    #[must_use]
    pub fn compute_telemetry_path(owner_user_id: u64, capability: &str) -> String {
        format!(
            "/internal/v1/marketplace/telemetry/compute?owner_user_id={owner_user_id}&capability={}",
            capability.trim()
        )
    }

    #[must_use]
    pub fn compute_treasury_summary_path(owner_user_id: u64) -> String {
        format!("/internal/v1/treasury/compute/summary?owner_user_id={owner_user_id}")
    }

    #[must_use]
    pub fn pool_status_path(pool_id: &str) -> String {
        format!("/internal/v1/pools/{}/status", pool_id.trim())
    }

    #[must_use]
    pub fn pool_snapshot_latest_path(pool_id: &str) -> String {
        format!("/internal/v1/pools/{}/snapshots/latest", pool_id.trim())
    }

    #[must_use]
    pub fn credit_health_path() -> &'static str {
        "/internal/v1/credit/health"
    }

    #[must_use]
    pub fn credit_intent_path() -> &'static str {
        "/internal/v1/credit/intent"
    }

    #[must_use]
    pub fn credit_offer_path() -> &'static str {
        "/internal/v1/credit/offer"
    }

    #[must_use]
    pub fn credit_envelope_path() -> &'static str {
        "/internal/v1/credit/envelope"
    }

    #[must_use]
    pub fn credit_settle_path() -> &'static str {
        "/internal/v1/credit/settle"
    }

    #[must_use]
    pub fn credit_agent_exposure_path(agent_id: &str) -> String {
        format!("/internal/v1/credit/agents/{}/exposure", agent_id.trim())
    }

    pub async fn list_workers(
        &self,
        owner_user_id: u64,
    ) -> Result<ComputeRuntimeWorkersListResponse, RuntimeClientError> {
        self.get_json(Self::workers_path(owner_user_id).as_str())
            .await
    }

    pub async fn list_workers_json(
        &self,
        owner_user_id: u64,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.get_json(Self::workers_path(owner_user_id).as_str())
            .await
    }

    pub async fn get_worker_json(
        &self,
        worker_id: &str,
        owner_user_id: u64,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.get_json(Self::worker_path(worker_id, owner_user_id).as_str())
            .await
    }

    pub async fn register_worker(
        &self,
        request: &RuntimeWorkerRegisterRequest,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.post_json("/internal/v1/workers", request).await
    }

    pub async fn heartbeat_worker(
        &self,
        worker_id: &str,
        request: &RuntimeWorkerHeartbeatRequest,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.post_json(Self::worker_heartbeat_path(worker_id).as_str(), request)
            .await
    }

    pub async fn transition_worker(
        &self,
        worker_id: &str,
        request: &RuntimeWorkerTransitionRequest,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.post_json(Self::worker_status_path(worker_id).as_str(), request)
            .await
    }

    pub async fn transition_worker_status(
        &self,
        worker_id: &str,
        request: &RuntimeWorkerStatusTransitionRequest,
    ) -> Result<serde_json::Value, RuntimeClientError> {
        self.post_json(Self::worker_status_path(worker_id).as_str(), request)
            .await
    }

    pub async fn provider_catalog(
        &self,
        owner_user_id: u64,
    ) -> Result<ComputeRuntimeProviderCatalogResponse, RuntimeClientError> {
        self.get_json(Self::provider_catalog_path(owner_user_id).as_str())
            .await
    }

    pub async fn compute_telemetry(
        &self,
        owner_user_id: u64,
        capability: &str,
    ) -> Result<ComputeRuntimeTelemetryResponse, RuntimeClientError> {
        self.get_json(Self::compute_telemetry_path(owner_user_id, capability).as_str())
            .await
    }

    pub async fn compute_treasury_summary(
        &self,
        owner_user_id: u64,
    ) -> Result<ComputeRuntimeTreasurySummary, RuntimeClientError> {
        self.get_json(Self::compute_treasury_summary_path(owner_user_id).as_str())
            .await
    }

    pub async fn pool_status(
        &self,
        pool_id: &str,
    ) -> Result<Option<RuntimePoolStatusResponseV1>, RuntimeClientError> {
        self.get_optional_json(Self::pool_status_path(pool_id).as_str())
            .await
    }

    pub async fn pool_snapshot_latest(
        &self,
        pool_id: &str,
    ) -> Result<Option<RuntimePoolSnapshotResponseV1>, RuntimeClientError> {
        self.get_optional_json(Self::pool_snapshot_latest_path(pool_id).as_str())
            .await
    }

    pub async fn credit_health(&self) -> Result<RuntimeCreditHealthResponseV1, RuntimeClientError> {
        self.get_json(Self::credit_health_path()).await
    }

    pub async fn credit_intent(
        &self,
        request: &RuntimeCreditIntentRequestV1,
    ) -> Result<RuntimeCreditIntentResponseV1, RuntimeClientError> {
        self.post_json(Self::credit_intent_path(), request).await
    }

    pub async fn credit_offer(
        &self,
        request: &RuntimeCreditOfferRequestV1,
    ) -> Result<RuntimeCreditOfferResponseV1, RuntimeClientError> {
        self.post_json(Self::credit_offer_path(), request).await
    }

    pub async fn credit_envelope(
        &self,
        request: &RuntimeCreditEnvelopeRequestV1,
    ) -> Result<RuntimeCreditEnvelopeResponseV1, RuntimeClientError> {
        self.post_json(Self::credit_envelope_path(), request).await
    }

    pub async fn credit_settle(
        &self,
        request: &RuntimeCreditSettleRequestV1,
    ) -> Result<RuntimeCreditSettleResponseV1, RuntimeClientError> {
        self.post_json(Self::credit_settle_path(), request).await
    }

    pub async fn credit_agent_exposure(
        &self,
        agent_id: &str,
    ) -> Result<RuntimeCreditAgentExposureResponseV1, RuntimeClientError> {
        self.get_json(Self::credit_agent_exposure_path(agent_id).as_str())
            .await
    }

    pub async fn get_json<T>(&self, path: &str) -> Result<T, RuntimeClientError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let response = self.send_get(path).await?;
        decode_json_response(response).await
    }

    pub async fn get_optional_json<T>(&self, path: &str) -> Result<Option<T>, RuntimeClientError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let response = self.send_get(path).await?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        decode_json_response(response).await.map(Some)
    }

    pub async fn post_json<Req, Res>(
        &self,
        path: &str,
        payload: &Req,
    ) -> Result<Res, RuntimeClientError>
    where
        Req: Serialize + ?Sized,
        Res: for<'de> serde::Deserialize<'de>,
    {
        let url = self.endpoint(path).ok_or(RuntimeClientError::InvalidPath)?;
        let mut last_error: Option<String> = None;

        for attempt in 0..self.request_attempts {
            let request = self
                .http
                .post(url.as_str())
                .header("x-request-id", format!("req_{}", Uuid::new_v4().simple()))
                .timeout(self.timeout)
                .json(payload);

            match request.send().await {
                Ok(response) => return decode_json_response(response).await,
                Err(error) => {
                    last_error = Some(error.to_string());
                    if attempt + 1 >= self.request_attempts {
                        break;
                    }
                }
            }
        }

        Err(RuntimeClientError::Request {
            message: last_error.unwrap_or_else(|| "unknown".to_string()),
        })
    }

    async fn send_get(&self, path: &str) -> Result<reqwest::Response, RuntimeClientError> {
        let url = self.endpoint(path).ok_or(RuntimeClientError::InvalidPath)?;
        let mut last_error: Option<String> = None;

        for attempt in 0..self.request_attempts {
            let request = self
                .http
                .get(url.as_str())
                .header("x-request-id", format!("req_{}", Uuid::new_v4().simple()))
                .timeout(self.timeout);

            match request.send().await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = Some(error.to_string());
                    if attempt + 1 >= self.request_attempts {
                        break;
                    }
                }
            }
        }

        Err(RuntimeClientError::Request {
            message: last_error.unwrap_or_else(|| "unknown".to_string()),
        })
    }
}

pub fn format_http_error(status: StatusCode, body: &[u8]) -> RuntimeClientError {
    let body = non_empty_string(String::from_utf8_lossy(body).to_string())
        .unwrap_or_else(|| "<empty>".to_string());
    RuntimeClientError::Http { status, body }
}

fn normalize_base_url(base_url: &str) -> Result<String, RuntimeClientError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(RuntimeClientError::BaseUrlMissing);
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

async fn decode_json_response<T>(response: reqwest::Response) -> Result<T, RuntimeClientError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| RuntimeClientError::Read {
            message: error.to_string(),
        })?;

    if !status.is_success() {
        return Err(format_http_error(status, &bytes));
    }

    serde_json::from_slice::<T>(&bytes).map_err(|error| RuntimeClientError::Decode {
        message: error.to_string(),
    })
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_builder_normalizes_paths() {
        let client = RuntimeInternalClient::new(RuntimeInternalClientConfig::new(
            "https://runtime.example.com/",
        ))
        .expect("runtime client");

        assert_eq!(
            client.endpoint("/internal/v1/workers"),
            Some("https://runtime.example.com/internal/v1/workers".to_string())
        );
        assert_eq!(
            client.endpoint("internal/v1/workers"),
            Some("https://runtime.example.com/internal/v1/workers".to_string())
        );
        assert_eq!(client.endpoint(""), None);
    }

    #[test]
    fn path_helpers_are_deterministic() {
        assert_eq!(
            RuntimeInternalClient::workers_path(42),
            "/internal/v1/workers?owner_user_id=42"
        );
        assert_eq!(
            RuntimeInternalClient::provider_catalog_path(7),
            "/internal/v1/marketplace/catalog/providers?owner_user_id=7"
        );
        assert_eq!(
            RuntimeInternalClient::compute_telemetry_path(5, "oa.sandbox_run.v1"),
            "/internal/v1/marketplace/telemetry/compute?owner_user_id=5&capability=oa.sandbox_run.v1"
        );
        assert_eq!(
            RuntimeInternalClient::worker_status_path("worker_abc"),
            "/internal/v1/workers/worker_abc/status"
        );
        assert_eq!(
            RuntimeInternalClient::credit_health_path(),
            "/internal/v1/credit/health"
        );
        assert_eq!(
            RuntimeInternalClient::credit_intent_path(),
            "/internal/v1/credit/intent"
        );
        assert_eq!(
            RuntimeInternalClient::credit_offer_path(),
            "/internal/v1/credit/offer"
        );
        assert_eq!(
            RuntimeInternalClient::credit_envelope_path(),
            "/internal/v1/credit/envelope"
        );
        assert_eq!(
            RuntimeInternalClient::credit_settle_path(),
            "/internal/v1/credit/settle"
        );
        assert_eq!(
            RuntimeInternalClient::credit_agent_exposure_path("abc"),
            "/internal/v1/credit/agents/abc/exposure"
        );
    }

    #[test]
    fn http_error_mapping_preserves_shape() {
        let error = format_http_error(StatusCode::BAD_GATEWAY, b" gateway failed ");
        assert_eq!(
            error.to_string(),
            "runtime_http_502 Bad Gateway:gateway failed"
        );

        let empty_body = format_http_error(StatusCode::SERVICE_UNAVAILABLE, b" ");
        assert_eq!(
            empty_body.to_string(),
            "runtime_http_503 Service Unavailable:<empty>"
        );
    }

    #[test]
    fn base_url_missing_is_rejected() {
        let result = RuntimeInternalClient::new(RuntimeInternalClientConfig::new("   "));
        assert!(matches!(result, Err(RuntimeClientError::BaseUrlMissing)));
    }
}
