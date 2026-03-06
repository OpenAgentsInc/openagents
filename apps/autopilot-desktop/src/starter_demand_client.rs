use reqwest::Url;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const STARTER_DEMAND_POLL_PATH: &str = "/api/starter-demand/poll";
const STARTER_DEMAND_OFFER_PREFIX: &str = "/api/starter-demand/offers";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StarterDemandPollRequest {
    pub provider_nostr_pubkey: Option<String>,
    pub primary_relay_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct StarterDemandOffer {
    pub request_id: String,
    pub requester: String,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct StarterDemandPollResponse {
    pub authority: String,
    pub hosted_nexus_relay_url: String,
    pub eligible: bool,
    pub reason: Option<String>,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
    pub dispatch_interval_seconds: u64,
    pub request_ttl_seconds: u64,
    pub max_active_offers_per_session: usize,
    pub offers: Vec<StarterDemandOffer>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StarterDemandCompleteRequest {
    pub payment_pointer: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct StarterDemandCompleteResponse {
    pub request_id: String,
    pub status: String,
    pub payment_pointer: String,
    pub completed_at_unix_ms: u64,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
}

pub fn canonical_starter_demand_poll_endpoint(control_base_url: &str) -> Result<Url, String> {
    canonical_control_endpoint(control_base_url, STARTER_DEMAND_POLL_PATH)
}

pub fn canonical_starter_demand_complete_endpoint(
    control_base_url: &str,
    request_id: &str,
) -> Result<Url, String> {
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("starter demand request_id must not be empty".to_string());
    }
    canonical_control_endpoint(
        control_base_url,
        format!("{STARTER_DEMAND_OFFER_PREFIX}/{request_id}/complete").as_str(),
    )
}

pub fn poll_starter_demand_blocking(
    client: &Client,
    control_base_url: &str,
    bearer_auth: &str,
    request: &StarterDemandPollRequest,
) -> Result<StarterDemandPollResponse, String> {
    let endpoint = canonical_starter_demand_poll_endpoint(control_base_url)?;
    let response = client
        .post(endpoint)
        .bearer_auth(bearer_auth.trim())
        .json(request)
        .send()
        .map_err(|error| format!("starter demand poll request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|_| "<unreadable-body>".to_string());
    if !status.is_success() {
        return Err(format!(
            "starter demand poll failed status={} body={}",
            status.as_u16(),
            truncate_body(body.as_str())
        ));
    }
    serde_json::from_str(body.as_str())
        .map_err(|error| format!("invalid starter demand poll payload: {error}"))
}

pub fn complete_starter_demand_offer_blocking(
    client: &Client,
    control_base_url: &str,
    bearer_auth: &str,
    request_id: &str,
    payment_pointer: &str,
) -> Result<StarterDemandCompleteResponse, String> {
    let endpoint = canonical_starter_demand_complete_endpoint(control_base_url, request_id)?;
    let response = client
        .post(endpoint)
        .bearer_auth(bearer_auth.trim())
        .json(&StarterDemandCompleteRequest {
            payment_pointer: payment_pointer.trim().to_string(),
        })
        .send()
        .map_err(|error| format!("starter demand completion request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|_| "<unreadable-body>".to_string());
    if !status.is_success() {
        return Err(format!(
            "starter demand completion failed status={} body={}",
            status.as_u16(),
            truncate_body(body.as_str())
        ));
    }
    serde_json::from_str(body.as_str())
        .map_err(|error| format!("invalid starter demand completion payload: {error}"))
}

fn canonical_control_endpoint(control_base_url: &str, path: &str) -> Result<Url, String> {
    let normalized = normalize_http_base_url(control_base_url)?;
    let mut url = Url::parse(normalized.as_str())
        .map_err(|error| format!("invalid control base url: {error}"))?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn normalize_http_base_url(control_base_url: &str) -> Result<String, String> {
    let trimmed = control_base_url.trim();
    if trimmed.is_empty() {
        return Err("control base url must not be empty".to_string());
    }
    let mut url =
        Url::parse(trimmed).map_err(|error| format!("invalid control base url: {error}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(format!(
            "unsupported control base url scheme '{}'; expected http or https",
            url.scheme()
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    if url.path().ends_with('/') {
        let trimmed_path = url.path().trim_end_matches('/').to_string();
        url.set_path(trimmed_path.as_str());
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn truncate_body(body: &str) -> String {
    const LIMIT: usize = 180;
    if body.chars().count() <= LIMIT {
        return body.to_string();
    }
    let truncated = body.chars().take(LIMIT).collect::<String>();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_starter_demand_complete_endpoint, canonical_starter_demand_poll_endpoint,
    };

    #[test]
    fn canonical_starter_demand_poll_endpoint_uses_canonical_path() {
        let url = canonical_starter_demand_poll_endpoint("https://control.example.com/base")
            .expect("starter poll endpoint should resolve");
        assert_eq!(
            url.as_str(),
            "https://control.example.com/api/starter-demand/poll"
        );
    }

    #[test]
    fn canonical_starter_demand_complete_endpoint_uses_canonical_path() {
        let url = canonical_starter_demand_complete_endpoint(
            "https://control.example.com/base",
            "starter-hosted-000001",
        )
        .expect("starter complete endpoint should resolve");
        assert_eq!(
            url.as_str(),
            "https://control.example.com/api/starter-demand/offers/starter-hosted-000001/complete"
        );
    }
}
