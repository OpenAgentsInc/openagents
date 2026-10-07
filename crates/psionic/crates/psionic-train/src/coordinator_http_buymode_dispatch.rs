//! Default-off HTTP `BuyModeDispatch` for the OpenAgents Worker buy-mode eval
//! endpoint (Khala M6, issue #6014 / EPIC #6017).
//!
//! [`crate::coordinator_live_buymode_dispatch`] covers the signed TCP
//! worker-envelope protocol. This module covers the other live bridge: an HTTP
//! client that can call an owner-armed OpenAgents Worker endpoint which remains
//! the spend authority for buy-mode dispatch, settlement, and the shared daily
//! cap.
//!
//! The client is intentionally boring and fail-closed:
//!
//! - no environment config arms it unless `PSIONIC_BUY_MODE_HTTP_ARM=armed`;
//! - endpoint and bearer token are required only after that exact arm;
//! - endpoint parsing rejects non-HTTP(S), missing hosts, and URL credentials;
//! - errors never include the bearer token, response body, or endpoint URL;
//! - tests bind a local HTTP server and never move sats.

use std::collections::BTreeMap;
use std::fmt;
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator_eval_verdict_source::{BuyModeDispatch, BuyModeEvalJob, BuyModeEvalResult};
use crate::coordinator_live_training::CoordinatorLiveTrainingError;

/// Exact arming environment variable for the HTTP buy-mode dispatcher.
pub const PSIONIC_BUY_MODE_HTTP_ARM_ENV: &str = "PSIONIC_BUY_MODE_HTTP_ARM";
/// Absolute `http://` or `https://` OpenAgents Worker endpoint.
pub const PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV: &str = "PSIONIC_BUY_MODE_HTTP_ENDPOINT";
/// Bearer token for the owner-armed Worker endpoint.
pub const PSIONIC_BUY_MODE_HTTP_BEARER_TOKEN_ENV: &str = "PSIONIC_BUY_MODE_HTTP_BEARER_TOKEN";
/// Optional request timeout in milliseconds.
pub const PSIONIC_BUY_MODE_HTTP_TIMEOUT_MS_ENV: &str = "PSIONIC_BUY_MODE_HTTP_TIMEOUT_MS";
/// Default HTTP timeout. Kept short so a miswired live lane fails closed quickly.
pub const DEFAULT_BUY_MODE_HTTP_TIMEOUT_MS: u64 = 10_000;
const HTTP_DISPATCH_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_millis(15_000),
    Duration::from_millis(30_000),
    Duration::from_millis(60_000),
    Duration::from_millis(90_000),
];

/// Validated configuration for [`HttpBuyModeDispatch`].
#[derive(Clone, PartialEq, Eq)]
pub struct HttpBuyModeDispatchConfig {
    endpoint: String,
    bearer_token: String,
    timeout: Duration,
}

impl fmt::Debug for HttpBuyModeDispatchConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpBuyModeDispatchConfig")
            .field("endpoint", &"<redacted>")
            .field("bearer_token", &"<redacted>")
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl HttpBuyModeDispatchConfig {
    /// Validates an explicit HTTP dispatch config. This does not arm live spend
    /// by itself; callers still need to place it behind
    /// [`crate::DispatchBackedVerdictSource`] and its daily cap.
    pub fn new(
        endpoint: impl Into<String>,
        bearer_token: impl Into<String>,
    ) -> Result<Self, HttpBuyModeDispatchConfigError> {
        Self::with_timeout_ms(endpoint, bearer_token, DEFAULT_BUY_MODE_HTTP_TIMEOUT_MS)
    }

    /// Validates an explicit HTTP dispatch config with a custom timeout.
    pub fn with_timeout_ms(
        endpoint: impl Into<String>,
        bearer_token: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self, HttpBuyModeDispatchConfigError> {
        let endpoint = validate_endpoint(endpoint.into())?;
        let bearer_token = bearer_token.into();
        if bearer_token.trim().is_empty() {
            return Err(HttpBuyModeDispatchConfigError::MissingBearerToken);
        }
        if timeout_ms == 0 {
            return Err(HttpBuyModeDispatchConfigError::InvalidTimeoutMs);
        }
        Ok(Self {
            endpoint,
            bearer_token,
            timeout: Duration::from_millis(timeout_ms),
        })
    }

    /// Endpoint value for the HTTP POST. Do not log this in live runs.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    /// Configured request timeout.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Config parsing failures. Variants are deliberately secret-safe.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HttpBuyModeDispatchConfigError {
    /// The arming env var was set to something other than the accepted values.
    #[error(
        "{PSIONIC_BUY_MODE_HTTP_ARM_ENV} must be unset, empty, false, disarmed, or exactly armed"
    )]
    InvalidArmValue,
    /// The dispatch endpoint is required once the lane is armed.
    #[error(
        "{PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV} is required when HTTP buy-mode dispatch is armed"
    )]
    MissingEndpoint,
    /// The bearer token is required once the lane is armed.
    #[error(
        "{PSIONIC_BUY_MODE_HTTP_BEARER_TOKEN_ENV} is required when HTTP buy-mode dispatch is armed"
    )]
    MissingBearerToken,
    /// The endpoint must parse as a URL.
    #[error("{PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV} must be a valid absolute URL")]
    InvalidEndpoint,
    /// Only HTTP(S) endpoints are accepted.
    #[error("{PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV} must use http or https")]
    InvalidEndpointScheme,
    /// URL credentials are never accepted because they are too easy to leak.
    #[error("{PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV} must not contain URL credentials")]
    EndpointContainsCredentials,
    /// The timeout must be a positive integer number of milliseconds.
    #[error("{PSIONIC_BUY_MODE_HTTP_TIMEOUT_MS_ENV} must be a positive integer")]
    InvalidTimeoutMs,
}

/// Builds an HTTP dispatch config from process environment.
///
/// Returns `Ok(None)` unless `PSIONIC_BUY_MODE_HTTP_ARM=armed` exactly (case
/// insensitive). Any malformed armed config returns an error so callers fail
/// closed instead of silently dropping into a partially configured live lane.
pub fn http_buy_mode_dispatch_config_from_env(
) -> Result<Option<HttpBuyModeDispatchConfig>, HttpBuyModeDispatchConfigError> {
    http_buy_mode_dispatch_config_from_iter(std::env::vars())
}

/// Builds an HTTP dispatch config from a key/value iterator. Exposed for tests
/// and for launchers that already parsed their environment into a map.
pub fn http_buy_mode_dispatch_config_from_iter<I, K, V>(
    iter: I,
) -> Result<Option<HttpBuyModeDispatchConfig>, HttpBuyModeDispatchConfigError>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let env: BTreeMap<String, String> = iter
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();

    let arm = env
        .get(PSIONIC_BUY_MODE_HTTP_ARM_ENV)
        .map_or("", String::as_str)
        .trim()
        .to_ascii_lowercase();

    match arm.as_str() {
        "" | "0" | "false" | "disarmed" => return Ok(None),
        "armed" => {}
        _ => return Err(HttpBuyModeDispatchConfigError::InvalidArmValue),
    }

    let endpoint = env
        .get(PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV)
        .map_or("", String::as_str)
        .trim();
    if endpoint.is_empty() {
        return Err(HttpBuyModeDispatchConfigError::MissingEndpoint);
    }

    let bearer_token = env
        .get(PSIONIC_BUY_MODE_HTTP_BEARER_TOKEN_ENV)
        .map_or("", String::as_str)
        .trim();
    if bearer_token.is_empty() {
        return Err(HttpBuyModeDispatchConfigError::MissingBearerToken);
    }

    let timeout_ms = match env
        .get(PSIONIC_BUY_MODE_HTTP_TIMEOUT_MS_ENV)
        .map_or("", String::as_str)
        .trim()
    {
        "" => DEFAULT_BUY_MODE_HTTP_TIMEOUT_MS,
        value => value
            .parse::<u64>()
            .map_err(|_| HttpBuyModeDispatchConfigError::InvalidTimeoutMs)?,
    };

    HttpBuyModeDispatchConfig::with_timeout_ms(endpoint, bearer_token, timeout_ms).map(Some)
}

/// HTTP implementation of [`BuyModeDispatch`] for an owner-armed OpenAgents
/// Worker endpoint.
pub struct HttpBuyModeDispatch {
    config: HttpBuyModeDispatchConfig,
    client: Client,
}

impl HttpBuyModeDispatch {
    /// Builds a dispatcher from validated config.
    pub fn new(config: HttpBuyModeDispatchConfig) -> Result<Self, HttpBuyModeDispatchBuildError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|_| HttpBuyModeDispatchBuildError::ClientBuildFailed)?;
        Ok(Self { config, client })
    }
}

/// Secret-safe build errors for [`HttpBuyModeDispatch`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HttpBuyModeDispatchBuildError {
    /// The HTTP client could not be constructed.
    #[error("failed to build buy-mode HTTP dispatch client")]
    ClientBuildFailed,
}

#[derive(Debug, Deserialize, Serialize)]
struct HttpBuyModeDispatchResponse {
    verdict: crate::VerificationClassVerdict,
    settled_msats: u64,
}

impl BuyModeDispatch for HttpBuyModeDispatch {
    fn dispatch_eval(
        &self,
        job: &BuyModeEvalJob,
    ) -> Result<BuyModeEvalResult, CoordinatorLiveTrainingError> {
        let mut attempt = 0_usize;

        loop {
            let response =
                match self
                    .client
                    .post(self.config.endpoint())
                    .bearer_auth(self.config.bearer_token.as_str())
                    .json(job)
                    .send()
                {
                    Ok(response) => response,
                    Err(_) if attempt < HTTP_DISPATCH_RETRY_DELAYS.len() => {
                        let delay = HTTP_DISPATCH_RETRY_DELAYS[attempt];
                        attempt += 1;
                        thread::sleep(delay);
                        continue;
                    }
                    Err(_) => return Err(CoordinatorLiveTrainingError::VerdictSource {
                        detail: String::from(
                            "buy-mode HTTP dispatch request failed before a verdict was received",
                        ),
                    }),
                };

            let status = response.status();
            if status.is_success() {
                let decoded =
                    response.json::<HttpBuyModeDispatchResponse>().map_err(|_| {
                        CoordinatorLiveTrainingError::VerdictSource {
                            detail: String::from(
                                "buy-mode HTTP dispatch response did not match the expected verdict schema",
                            ),
                        }
                    })?;

                return Ok(BuyModeEvalResult {
                    verdict: decoded.verdict,
                    settled_msats: decoded.settled_msats,
                });
            }

            let body = response.text().unwrap_or_default();
            if is_retryable_relay_publish_blocker(&body)
                && attempt < HTTP_DISPATCH_RETRY_DELAYS.len()
            {
                let delay = HTTP_DISPATCH_RETRY_DELAYS[attempt];
                attempt += 1;
                thread::sleep(delay);
                continue;
            }

            return Err(CoordinatorLiveTrainingError::VerdictSource {
                detail: format!("buy-mode HTTP dispatch rejected the job with status {status}"),
            });
        }
    }
}

fn is_retryable_relay_publish_blocker(body: &str) -> bool {
    body.len() <= 4_096
        && body.contains("blocker.buy_mode.relay")
        && !body.contains("daily_cap")
        && !body.contains("per_job_cap")
        && !body.contains("operator_spend")
}

fn validate_endpoint(endpoint: String) -> Result<String, HttpBuyModeDispatchConfigError> {
    let trimmed = endpoint.trim();
    let url = reqwest::Url::parse(trimmed)
        .map_err(|_| HttpBuyModeDispatchConfigError::InvalidEndpoint)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(HttpBuyModeDispatchConfigError::InvalidEndpointScheme);
    }
    if url.host_str().is_none() {
        return Err(HttpBuyModeDispatchConfigError::InvalidEndpoint);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(HttpBuyModeDispatchConfigError::EndpointContainsCredentials);
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuyModeEvalJob, VerificationClass, VerificationClassVerdict};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn job() -> BuyModeEvalJob {
        BuyModeEvalJob {
            worker_id: String::from("open-pylon-a"),
            role_index: 2,
            sample_id: String::from("sample.http"),
            amount_msats: 1_250,
        }
    }

    #[test]
    fn env_config_is_default_off_until_exactly_armed() {
        assert_eq!(
            http_buy_mode_dispatch_config_from_iter(Vec::<(String, String)>::new()).unwrap(),
            None
        );
        assert_eq!(
            http_buy_mode_dispatch_config_from_iter([(PSIONIC_BUY_MODE_HTTP_ARM_ENV, "disarmed",)])
                .unwrap(),
            None
        );
        assert_eq!(
            http_buy_mode_dispatch_config_from_iter([(PSIONIC_BUY_MODE_HTTP_ARM_ENV, "true")])
                .unwrap_err(),
            HttpBuyModeDispatchConfigError::InvalidArmValue
        );
    }

    #[test]
    fn armed_env_requires_endpoint_and_token() {
        assert_eq!(
            http_buy_mode_dispatch_config_from_iter([(PSIONIC_BUY_MODE_HTTP_ARM_ENV, "armed")])
                .unwrap_err(),
            HttpBuyModeDispatchConfigError::MissingEndpoint
        );
        assert_eq!(
            http_buy_mode_dispatch_config_from_iter([
                (PSIONIC_BUY_MODE_HTTP_ARM_ENV, "armed"),
                (
                    PSIONIC_BUY_MODE_HTTP_ENDPOINT_ENV,
                    "http://127.0.0.1:1/eval"
                ),
            ])
            .unwrap_err(),
            HttpBuyModeDispatchConfigError::MissingBearerToken
        );
    }

    #[test]
    fn config_rejects_non_http_urls_credentials_and_zero_timeout() {
        assert_eq!(
            HttpBuyModeDispatchConfig::new("tcp://127.0.0.1:1", "token").unwrap_err(),
            HttpBuyModeDispatchConfigError::InvalidEndpointScheme
        );
        assert_eq!(
            HttpBuyModeDispatchConfig::new("https://user:pass@example.com/eval", "token")
                .unwrap_err(),
            HttpBuyModeDispatchConfigError::EndpointContainsCredentials
        );
        assert_eq!(
            HttpBuyModeDispatchConfig::with_timeout_ms("https://example.com/eval", "token", 0)
                .unwrap_err(),
            HttpBuyModeDispatchConfigError::InvalidTimeoutMs
        );
    }

    #[test]
    fn config_debug_redacts_endpoint_and_token() {
        let config =
            HttpBuyModeDispatchConfig::new("https://example.com/secret-path", "secret-token")
                .unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret-path"));
        assert!(!debug.contains("secret-token"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn http_dispatch_posts_job_and_decodes_verdict() {
        let expected_token = "test-token";
        let endpoint = spawn_one_shot_http_server(expected_token, 200, verdict_body(), true);
        let config =
            HttpBuyModeDispatchConfig::with_timeout_ms(endpoint, expected_token, 2_000).unwrap();
        let dispatch = HttpBuyModeDispatch::new(config).unwrap();

        let result = dispatch.dispatch_eval(&job()).expect("dispatch");
        assert_eq!(
            result.verdict,
            VerificationClassVerdict {
                class: VerificationClass::ExactTraceReplay,
                passed: true,
            }
        );
        assert_eq!(result.settled_msats, 1_250);
    }

    #[test]
    fn http_dispatch_status_errors_are_redacted() {
        let endpoint =
            spawn_one_shot_http_server("secret-token", 503, "sensitive failure body", false);
        let config =
            HttpBuyModeDispatchConfig::with_timeout_ms(endpoint.clone(), "secret-token", 2_000)
                .unwrap();
        let dispatch = HttpBuyModeDispatch::new(config).unwrap();

        let error = dispatch.dispatch_eval(&job()).unwrap_err();
        let detail = format!("{error}");
        assert!(detail.contains("503"));
        assert!(!detail.contains("secret-token"));
        assert!(!detail.contains("sensitive failure body"));
        assert!(!detail.contains(endpoint.as_str()));
    }

    #[test]
    fn http_dispatch_schema_errors_are_redacted() {
        let endpoint = spawn_one_shot_http_server("secret-token", 200, "{not-json", false);
        let config =
            HttpBuyModeDispatchConfig::with_timeout_ms(endpoint.clone(), "secret-token", 2_000)
                .unwrap();
        let dispatch = HttpBuyModeDispatch::new(config).unwrap();

        let error = dispatch.dispatch_eval(&job()).unwrap_err();
        let detail = format!("{error}");
        assert!(detail.contains("expected verdict schema"));
        assert!(!detail.contains("secret-token"));
        assert!(!detail.contains("{not-json"));
        assert!(!detail.contains(endpoint.as_str()));
    }

    fn verdict_body() -> &'static str {
        r#"{"verdict":{"class":"exact_trace_replay","passed":true},"settled_msats":1250}"#
    }

    fn spawn_one_shot_http_server(
        expected_token: &'static str,
        status: u16,
        body: &'static str,
        assert_request_body: bool,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let endpoint = format!("http://{}/eval", listener.local_addr().expect("local addr"));

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request);
                    let content_length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    let header_end = request
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .map(|index| index + 4)
                        .unwrap_or(request.len());
                    if request.len() >= header_end + content_length {
                        break;
                    }
                }
            }

            let request_text = String::from_utf8_lossy(&request);
            assert!(request_text.starts_with("POST /eval HTTP/1.1"));
            assert!(
                request_text.contains(format!("authorization: Bearer {expected_token}").as_str())
            );
            if assert_request_body {
                assert!(request_text.contains(r#""worker_id":"open-pylon-a""#));
                assert!(request_text.contains(r#""sample_id":"sample.http""#));
                assert!(request_text.contains(r#""amount_msats":1250"#));
            }

            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        });

        endpoint
    }
}
