//! Minimal `LayerZero` Scan API client.
//!
//! Used to confirm that an OFT (USDT0) cross-chain message reached the
//! destination chain, so a bridged swap can advance from `Settling` to
//! `Completed`. This is the OFT analog of Circle Iris for CCTP: a single
//! status lookup by message GUID, needing no destination-chain RPC.
//!
//! Unlike CCTP, the OFT delivered amount is already known at claim time (from
//! the source `OFTSent` log's `amountReceivedLD`), so this client only confirms
//! *delivery*; it does not carry an amount.

use std::collections::HashMap;

use serde::Deserialize;

use platform_utils::http::HttpClient;

use crate::error::BoltzError;

/// Client for the `LayerZero` Scan REST API (`/v1/messages/...`).
pub struct LzScanClient {
    http: Box<dyn HttpClient>,
    api_url: String,
}

/// Delivery status of a `LayerZero` message from the Scan API.
///
/// Response shape verified against the live mainnet `OpenAPI` spec
/// (`GET https://scan.layerzero-api.com/v1/openapi`, 2026-06): the body is
/// `{ "data": [ { "status": { "name": <enum>, "message": string }, ... } ] }`,
/// and `status.name` is one of `INFLIGHT`, `CONFIRMING`, `FAILED`, `DELIVERED`,
/// `BLOCKED`, `PAYLOAD_STORED`, `APPLICATION_BURNED`, `APPLICATION_SKIPPED`,
/// `UNRESOLVABLE_COMMAND`, `MALFORMED_COMMAND`. A missing message is HTTP 404
/// (`{"code":4040}`), treated as not-yet-indexed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LzMessageStatus {
    /// `false` when Scan hasn't indexed the source tx yet (HTTP 404 or empty
    /// `data`); the message is still in flight.
    pub found: bool,
    /// High-level lifecycle name, e.g. `"INFLIGHT"` / `"DELIVERED"` / `"FAILED"`.
    pub status: Option<String>,
}

impl LzMessageStatus {
    /// Whether the message was successfully executed on the destination chain.
    #[must_use]
    pub fn is_delivered(&self) -> bool {
        self.status.as_deref() == Some(LZ_STATUS_DELIVERED)
    }
}

/// `status.name` value Scan reports once a message is executed on the
/// destination chain.
///
/// Note: only `DELIVERED` finalizes a swap. Every other value (including the
/// terminal failures `FAILED` / `BLOCKED` / `MALFORMED_COMMAND` /
/// `UNRESOLVABLE_COMMAND`) keeps the swap polling in `Settling`. That is a
/// deliberate choice — a stuck-but-honest `Settling` over a false `Completed` —
/// but a future change could short-circuit the known-terminal failures to a
/// distinct surfaced status.
const LZ_STATUS_DELIVERED: &str = "DELIVERED";

impl LzScanClient {
    pub fn new(http: Box<dyn HttpClient>, api_url: String) -> Self {
        // Normalize: drop a single trailing slash so path joins are clean.
        let api_url = match api_url.strip_suffix('/') {
            Some(trimmed) => trimmed.to_string(),
            None => api_url,
        };
        Self { http, api_url }
    }

    /// Look up a message's delivery status by its `LayerZero` GUID
    /// (`0x`-prefixed hex). A 404 means Scan hasn't indexed it yet (treated as
    /// not-yet-found, still in flight).
    pub async fn get_message_status(&self, guid: &str) -> Result<LzMessageStatus, BoltzError> {
        let url = format!("{}/v1/messages/guid/{guid}", self.api_url);
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/json".to_string());

        let response = self
            .http
            .get(url, Some(headers))
            .await
            .map_err(|e| BoltzError::Generic(format!("LayerZero Scan request failed: {e}")))?;

        // Not indexed yet — the message is still in flight, not an error.
        if response.status == 404 {
            return Ok(LzMessageStatus::default());
        }
        if !response.is_success() {
            return Err(BoltzError::Generic(format!(
                "LayerZero Scan HTTP error {}: {}",
                response.status, response.body
            )));
        }

        Self::parse_message_status(&response.body)
    }

    /// Parse the Scan `/v1/messages` response. Split out for testing against
    /// recorded JSON.
    fn parse_message_status(body: &str) -> Result<LzMessageStatus, BoltzError> {
        let parsed: LzMessagesResponse = serde_json::from_str(body).map_err(|e| {
            BoltzError::Generic(format!("failed to parse LayerZero Scan response: {e}"))
        })?;
        let Some(msg) = parsed.data.into_iter().next() else {
            return Ok(LzMessageStatus::default());
        };
        Ok(LzMessageStatus {
            found: true,
            status: msg.status.and_then(|s| s.name),
        })
    }
}

#[derive(Deserialize)]
struct LzMessagesResponse {
    #[serde(default)]
    data: Vec<LzMessageSnapshot>,
}

#[derive(Deserialize)]
struct LzMessageSnapshot {
    #[serde(default)]
    status: Option<LzStatus>,
}

#[derive(Deserialize)]
struct LzStatus {
    #[serde(default)]
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn parse_delivered() {
        let body = r#"{"data":[{"status":{"name":"DELIVERED"}}]}"#;
        let s = LzScanClient::parse_message_status(body).unwrap();
        assert!(s.found);
        assert!(s.is_delivered());
    }

    #[macros::test_all]
    fn parse_inflight_not_delivered() {
        let body = r#"{"data":[{"status":{"name":"INFLIGHT"}}]}"#;
        let s = LzScanClient::parse_message_status(body).unwrap();
        assert!(s.found);
        assert!(!s.is_delivered());
    }

    #[macros::test_all]
    fn parse_empty_is_not_found() {
        let s = LzScanClient::parse_message_status(r#"{"data":[]}"#).unwrap();
        assert!(!s.found);
        assert!(!s.is_delivered());
    }

    #[macros::test_all]
    fn parse_found_but_missing_status_is_not_delivered() {
        // A message present but without a status object: found, not delivered.
        let s = LzScanClient::parse_message_status(r#"{"data":[{}]}"#).unwrap();
        assert!(s.found);
        assert_eq!(s.status, None);
        assert!(!s.is_delivered());
    }

    #[macros::test_all]
    fn parse_missing_data_field_is_not_found() {
        // Unexpected-but-valid JSON without `data` defaults to empty.
        let s = LzScanClient::parse_message_status("{}").unwrap();
        assert!(!s.found);
    }

    #[macros::test_all]
    fn parse_malformed_json_errors() {
        assert!(LzScanClient::parse_message_status("not json").is_err());
    }
}
