//! The FCM HTTP v1 sender with service-account OAuth.
//!
//! Every message is `{"message":{"token":…,"data":{"wake":"reconnect"},
//! "android":{"priority":"high","ttl":"<n>s"}}}`: the complete `data` member
//! is the constant [`nostr::push_lease::FCM_DATA`], and there is no
//! `notification` member. The token, priority, and TTL are routing
//! controls, not application bytes.

use std::time::Duration;

use ring::signature::RsaKeyPair;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{Outcome, config::FcmConfig, jwt};

/// The OAuth scope for sending messages.
pub const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const ASSERTION_SECONDS: u64 = 3_600;

#[derive(Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default)]
    private_key_id: Option<String>,
    token_uri: String,
    #[serde(default)]
    project_id: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Sends the FCM wake constant.
pub struct Fcm {
    http: reqwest::Client,
    send_url: String,
    token_uri: String,
    client_email: String,
    key_id: Option<String>,
    key: RsaKeyPair,
    token: Mutex<Option<(String, u64)>>,
    max_ttl_seconds: u64,
}

impl Fcm {
    /// A sender from configuration.
    ///
    /// # Errors
    ///
    /// Returns a reason when the service account, key, or URLs are not valid.
    pub fn new(config: &FcmConfig, max_ttl_seconds: u64) -> Result<Self, String> {
        let account: ServiceAccount = serde_json::from_str(config.service_account_json.expose())
            .map_err(|_| "the FCM service account file is not a service account key".to_owned())?;
        let project = config
            .project_id
            .clone()
            .or(account.project_id)
            .filter(|project| {
                !project.is_empty()
                    && project
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            .ok_or("PUSH_GATEWAY_FCM_PROJECT_ID is missing or not a project ID")?;
        let base = jwt::provider_url("PUSH_GATEWAY_FCM_URL", &config.base_url)?;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("cannot build the FCM client: {error}"))?;
        Ok(Self {
            http,
            send_url: format!("{base}/v1/projects/{project}/messages:send"),
            token_uri: jwt::provider_url("the service account token_uri", &account.token_uri)?,
            client_email: account.client_email,
            key_id: account.private_key_id,
            key: jwt::rs256_key(&account.private_key)?,
            token: Mutex::new(None),
            max_ttl_seconds,
        })
    }

    async fn access_token(&self, now: u64, force: bool) -> Result<String, Outcome> {
        let mut cached = self.token.lock().await;
        if !force
            && let Some((token, valid_until)) = cached.as_ref()
            && now < *valid_until
        {
            return Ok(token.clone());
        }
        let mut header = json!({"alg": "RS256", "typ": "JWT"});
        if let Some(key_id) = &self.key_id {
            header["kid"] = json!(key_id);
        }
        let assertion = jwt::rs256(
            &self.key,
            &header,
            &json!({
                "iss": self.client_email,
                "scope": SCOPE,
                "aud": self.token_uri,
                "iat": now,
                "exp": now + ASSERTION_SECONDS,
            }),
        )
        .map_err(|_| Outcome::ConfigurationFault)?;
        let response = self
            .http
            .post(&self.token_uri)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}"
            ))
            .send()
            .await
            .map_err(|_| Outcome::Retry { after: None })?;
        let status = response.status().as_u16();
        if status >= 500 || status == 429 {
            return Err(Outcome::Retry { after: None });
        }
        if status != 200 {
            return Err(Outcome::ConfigurationFault);
        }
        let token: TokenResponse = response
            .json()
            .await
            .map_err(|_| Outcome::ConfigurationFault)?;
        let lifetime = token.expires_in.unwrap_or(3_600).clamp(60, 3_600);
        *cached = Some((token.access_token.clone(), now + lifetime - 30));
        Ok(token.access_token)
    }

    /// Send one wake. A rejected access token permits one refresh and one
    /// retry; a connection failure permits one retry.
    pub async fn send(&self, device_token: &str, expires_at: u64, now: u64) -> Outcome {
        let ttl = expires_at
            .saturating_sub(now)
            .clamp(1, self.max_ttl_seconds);
        let body = message(device_token, ttl);
        let mut force = false;
        let mut connection_retried = false;
        let mut token_retried = false;
        loop {
            let token = match self.access_token(now, force).await {
                Ok(token) => token,
                Err(outcome) => return outcome,
            };
            let response = self
                .http
                .post(&self.send_url)
                .bearer_auth(token)
                .json(&body)
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(_) if !connection_retried => {
                    connection_retried = true;
                    continue;
                }
                Err(_) => return Outcome::Retry { after: None },
            };
            let status = response.status().as_u16();
            let after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok());
            let error: Value = response.json().await.unwrap_or_default();
            match classify(status, &error_code(&error)) {
                None if !token_retried => {
                    token_retried = true;
                    force = true;
                }
                None => return Outcome::ConfigurationFault,
                Some(Outcome::Retry { .. }) => return Outcome::Retry { after },
                Some(outcome) => return outcome,
            }
        }
    }
}

/// The complete FCM request body for one wake.
#[must_use]
pub fn message(device_token: &str, ttl_seconds: u64) -> Value {
    let data: Value = serde_json::from_str(nostr::push_lease::FCM_DATA)
        .expect("the registered FCM constant is JSON");
    json!({
        "message": {
            "token": device_token,
            "data": data,
            "android": {"priority": "high", "ttl": format!("{ttl_seconds}s")},
        }
    })
}

/// The FCM error code from `error.details[].errorCode`, else `error.status`.
fn error_code(error: &Value) -> String {
    let error = &error["error"];
    error["details"]
        .as_array()
        .and_then(|details| {
            details
                .iter()
                .find_map(|detail| detail["errorCode"].as_str())
        })
        .or_else(|| error["status"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Map an FCM status and error code to an outcome; `None` asks for one
/// access-token refresh.
fn classify(status: u16, code: &str) -> Option<Outcome> {
    Some(match (status, code) {
        (200, _) => Outcome::Accepted,
        (_, "UNREGISTERED" | "SENDER_ID_MISMATCH") | (404, _) => {
            Outcome::InvalidEndpoint { invalid_at: None }
        }
        // The message is a constant, so an invalid argument is the token.
        (400, "INVALID_ARGUMENT") => Outcome::InvalidEndpoint { invalid_at: None },
        (401, _) => return None,
        (403, _) | (_, "THIRD_PARTY_AUTH_ERROR") => Outcome::ConfigurationFault,
        (429 | 500 | 502 | 503 | 504, _) => Outcome::Retry { after: None },
        _ => Outcome::RequestFault,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_data_is_exactly_the_registered_constant() {
        let body = message("token-1", 60);
        let message = body["message"].as_object().unwrap();
        let mut keys = message.keys().map(String::as_str).collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, ["android", "data", "token"]);
        assert_eq!(message["data"].to_string(), nostr::push_lease::FCM_DATA);
        assert_eq!(message["android"]["ttl"], "60s");
    }

    #[test]
    fn fcm_statuses_map_to_bounded_outcomes() {
        let unregistered = json!({"error":{"code":404,"status":"NOT_FOUND","details":[{"@type":"type.googleapis.com/google.firebase.fcm.v1.FcmError","errorCode":"UNREGISTERED"}]}});
        assert_eq!(error_code(&unregistered), "UNREGISTERED");
        assert_eq!(
            classify(404, "UNREGISTERED"),
            Some(Outcome::InvalidEndpoint { invalid_at: None })
        );
        assert_eq!(
            classify(400, "INVALID_ARGUMENT"),
            Some(Outcome::InvalidEndpoint { invalid_at: None })
        );
        assert_eq!(classify(401, "UNAUTHENTICATED"), None);
        assert_eq!(
            classify(403, "PERMISSION_DENIED"),
            Some(Outcome::ConfigurationFault)
        );
        assert_eq!(
            classify(429, "QUOTA_EXCEEDED"),
            Some(Outcome::Retry { after: None })
        );
        assert_eq!(classify(400, "OTHER"), Some(Outcome::RequestFault));
    }
}
