use crate::receipts::{
    EvidenceRef, PolicyContext, Receipt, ReceiptBuilder, ReceiptHints, TraceContext,
};
use crate::snapshots::EconomySnapshot;
use anyhow::{Result, anyhow};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[allow(async_fn_in_trait)]
pub trait KernelAuthority: Send + Sync {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse>;
    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse>;
    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse>;
    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse>;
    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitRequest {
    pub work_unit_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitResponse {
    pub work_unit_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub verdict: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Clone)]
pub struct HttpKernelAuthorityClient {
    client: reqwest::Client,
    base_url: String,
    bearer_auth: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorityErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

impl HttpKernelAuthorityClient {
    pub fn new(base_url: impl Into<String>, bearer_auth: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|error| anyhow!("kernel authority client initialization failed: {error}"))?;
        Ok(Self::with_client(client, base_url, bearer_auth))
    }

    pub fn with_client(
        client: reqwest::Client,
        base_url: impl Into<String>,
        bearer_auth: Option<String>,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            bearer_auth: bearer_auth
                .map(|token| token.trim().to_string())
                .filter(|token| !token.is_empty()),
        }
    }

    pub fn canonical_endpoint(&self, path: &str) -> Result<Url> {
        canonical_kernel_endpoint(self.base_url.as_str(), path)
    }

    async fn post_json<Request, Response>(&self, path: &str, body: &Request) -> Result<Response>
    where
        Request: Serialize + ?Sized,
        Response: for<'de> Deserialize<'de>,
    {
        let endpoint = self.canonical_endpoint(path)?;
        let mut request = self.client.post(endpoint).json(body);
        if let Some(token) = self.bearer_auth.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!("kernel authority request failed: {error}"))?;
        decode_authority_response(response).await
    }

    async fn get_json<Response>(&self, path: &str) -> Result<Response>
    where
        Response: for<'de> Deserialize<'de>,
    {
        let endpoint = self.canonical_endpoint(path)?;
        let mut request = self.client.get(endpoint);
        if let Some(token) = self.bearer_auth.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!("kernel authority request failed: {error}"))?;
        decode_authority_response(response).await
    }
}

#[derive(Default)]
struct LocalKernelAuthorityState {
    work_units: BTreeMap<String, Value>,
    contracts: BTreeMap<String, Value>,
    submissions: BTreeMap<String, Value>,
    verdicts: BTreeMap<String, Value>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    receipts: Vec<Receipt>,
}

struct LocalReceiptSpec {
    receipt_id: String,
    receipt_type: String,
    created_at_ms: i64,
    idempotency_key: String,
    trace: TraceContext,
    policy: PolicyContext,
    inputs_payload: Value,
    outputs_payload: Value,
    evidence: Vec<EvidenceRef>,
    hints: ReceiptHints,
}

#[derive(Clone, Default)]
pub struct LocalKernelAuthority {
    state: Arc<RwLock<LocalKernelAuthorityState>>,
}

impl LocalKernelAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn receipts(&self) -> Result<Vec<Receipt>> {
        let state = self
            .state
            .read()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        Ok(state.receipts.clone())
    }

    pub fn record_snapshot(&self, minute_start_ms: i64, snapshot: EconomySnapshot) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state.snapshots.insert(minute_start_ms, snapshot);
        Ok(())
    }

    fn build_receipt(spec: LocalReceiptSpec) -> Result<Receipt> {
        ReceiptBuilder::new(
            spec.receipt_id,
            spec.receipt_type,
            spec.created_at_ms,
            spec.idempotency_key,
            spec.trace,
            spec.policy,
        )
        .with_inputs_payload(spec.inputs_payload)
        .with_outputs_payload(spec.outputs_payload)
        .with_evidence(spec.evidence)
        .with_hints(spec.hints)
        .build()
        .map_err(|error| anyhow!(error))
    }

    fn normalize_work_trace(mut trace: TraceContext, work_unit_id: &str) -> TraceContext {
        if trace.work_unit_id.is_none() {
            trace.work_unit_id = Some(work_unit_id.to_string());
        }
        trace
    }

    fn normalize_contract_trace(mut trace: TraceContext, contract_id: &str) -> TraceContext {
        if trace.contract_id.is_none() {
            trace.contract_id = Some(contract_id.to_string());
        }
        trace
    }
}

impl KernelAuthority for LocalKernelAuthority {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse> {
        let trace = Self::normalize_work_trace(req.trace, req.work_unit_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.work_unit:{}", req.work_unit_id),
            receipt_type: "kernel.work_unit.created.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "work_unit_id": req.work_unit_id,
                "status": "created",
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let work_unit_id = trace
            .work_unit_id
            .clone()
            .unwrap_or_else(|| "work_unit.unknown".to_string());
        state.work_units.insert(work_unit_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(CreateWorkUnitResponse {
            work_unit_id,
            receipt,
        })
    }

    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.contract:{}", req.contract_id),
            receipt_type: "kernel.contract.created.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "created",
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        state.contracts.insert(contract_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(CreateContractResponse {
            contract_id,
            receipt,
        })
    }

    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.submission:{}", req.contract_id),
            receipt_type: "kernel.submission.received.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "submitted",
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        state.submissions.insert(contract_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(SubmitOutputResponse {
            contract_id,
            receipt,
        })
    }

    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.verdict:{}", req.contract_id),
            receipt_type: "kernel.verdict.finalized.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.verdict.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "finalized",
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        state.verdicts.insert(contract_id.clone(), req.verdict);
        state.receipts.push(receipt.clone());
        Ok(FinalizeVerdictResponse {
            contract_id,
            receipt,
        })
    }

    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot> {
        let state = self
            .state
            .read()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state
            .snapshots
            .get(&minute_start_ms)
            .cloned()
            .ok_or_else(|| anyhow!("snapshot for minute {minute_start_ms} not found"))
    }
}

impl KernelAuthority for HttpKernelAuthorityClient {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse> {
        self.post_json("/v1/kernel/work_units", &req).await
    }

    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse> {
        self.post_json("/v1/kernel/contracts", &req).await
    }

    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse> {
        let path = format!("/v1/kernel/contracts/{}/submit", req.contract_id.trim());
        self.post_json(path.as_str(), &req).await
    }

    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse> {
        let path = format!(
            "/v1/kernel/contracts/{}/verdict/finalize",
            req.contract_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot> {
        let path = format!("/v1/kernel/snapshots/{minute_start_ms}");
        self.get_json(path.as_str()).await
    }
}

pub fn canonical_kernel_endpoint(base_url: &str, path: &str) -> Result<Url> {
    let normalized_base = normalize_http_base_url(base_url)?;
    let normalized_path = path.trim();
    if !normalized_path.starts_with('/') {
        return Err(anyhow!(
            "kernel authority path must start with '/': {normalized_path}"
        ));
    }
    let mut url = Url::parse(normalized_base.as_str())
        .map_err(|error| anyhow!("invalid kernel authority base url: {error}"))?;
    url.set_path(normalized_path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn normalize_http_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("kernel authority base url cannot be empty"));
    }
    let mut url =
        Url::parse(trimmed).map_err(|error| anyhow!("invalid kernel authority base url: {error}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(anyhow!(
            "kernel authority base url must use http or https"
        ));
    }
    let normalized_path = url.path().trim_end_matches('/').to_string();
    if normalized_path.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(normalized_path.as_str());
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

async fn decode_authority_response<Response>(response: reqwest::Response) -> Result<Response>
where
    Response: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable-body>".to_string());
    if !status.is_success() {
        return Err(anyhow!(
            "kernel authority request failed: {}",
            format_authority_error(status, body.as_str())
        ));
    }
    serde_json::from_str(body.as_str())
        .map_err(|error| anyhow!("invalid kernel authority response: {error}"))
}

fn format_authority_error(status: reqwest::StatusCode, body: &str) -> String {
    if let Ok(payload) = serde_json::from_str::<AuthorityErrorResponse>(body) {
        if let Some(reason) = payload.reason.as_deref().filter(|value| !value.is_empty()) {
            if let Some(error) = payload.error.as_deref().filter(|value| !value.is_empty()) {
                return format!("status={} error={} reason={reason}", status.as_u16(), error);
            }
            return format!("status={} reason={reason}", status.as_u16());
        }
        if let Some(error) = payload.error.as_deref().filter(|value| !value.is_empty()) {
            return format!("status={} error={error}", status.as_u16());
        }
    }
    format!(
        "status={} body={}",
        status.as_u16(),
        truncate_response_body(body)
    )
}

fn truncate_response_body(body: &str) -> String {
    const LIMIT: usize = 256;
    let trimmed = body.trim();
    if trimmed.len() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..LIMIT])
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_kernel_endpoint;

    #[test]
    fn canonical_endpoint_strips_query_and_joins_path() {
        let endpoint = canonical_kernel_endpoint(
            "https://control.example.com/base?foo=bar",
            "/v1/kernel/work_units",
        )
        .expect("endpoint");
        assert_eq!(
            endpoint.as_str(),
            "https://control.example.com/v1/kernel/work_units"
        );
    }

    #[test]
    fn canonical_endpoint_rejects_relative_path() {
        let error = canonical_kernel_endpoint("https://control.example.com", "v1/kernel/work_units")
            .expect_err("relative paths should fail");
        assert!(
            error
                .to_string()
                .contains("path must start with '/'"),
            "unexpected error: {error}"
        );
    }
}
