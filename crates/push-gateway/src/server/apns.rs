//! The APNs sender: HTTP/2 with token-based authentication.
//!
//! Every attempt posts the PL constant [`nostr::push_lease::APNS_BODY`] to
//! `/3/device/<token>`. The topic, push type `alert`, and priority `10` come
//! from configuration; `apns-id` is the relay's job UUID, and
//! `apns-expiration` is the request's expiry capped by a gateway ceiling. No
//! request field enters the body.

use std::time::Duration;

use ring::signature::EcdsaKeyPair;
use serde_json::json;
use tokio::sync::Mutex;

use super::{Outcome, config::ApnsConfig, jwt};

/// Refresh the provider token after this many seconds. APNs accepts a
/// token for an hour and asks for no more than one new token per 20 minutes.
const TOKEN_REFRESH_SECONDS: u64 = 40 * 60;

/// Sends the APNs wake constant.
pub struct Apns {
    http: reqwest::Client,
    base: String,
    topic: String,
    key_id: String,
    team_id: String,
    key: EcdsaKeyPair,
    token: Mutex<Option<(String, u64)>>,
    max_expiration_seconds: u64,
}

impl Apns {
    /// A sender from configuration.
    ///
    /// # Errors
    ///
    /// Returns a reason when the key or URL is not valid.
    pub fn new(config: &ApnsConfig, max_expiration_seconds: u64) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .http2_prior_knowledge()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("cannot build the APNs client: {error}"))?;
        Ok(Self {
            http,
            base: jwt::provider_url("PUSH_GATEWAY_APNS_URL", &config.base_url)?,
            topic: config.topic.clone(),
            key_id: config.key_id.clone(),
            team_id: config.team_id.clone(),
            key: jwt::es256_key(config.key_pem.expose())?,
            token: Mutex::new(None),
            max_expiration_seconds,
        })
    }

    async fn provider_token(&self, now: u64, force: bool) -> Result<String, String> {
        let mut cached = self.token.lock().await;
        if !force
            && let Some((token, issued)) = cached.as_ref()
            && now.saturating_sub(*issued) < TOKEN_REFRESH_SECONDS
        {
            return Ok(token.clone());
        }
        let token = jwt::es256(
            &self.key,
            &json!({"alg": "ES256", "kid": self.key_id}),
            &json!({"iss": self.team_id, "iat": now}),
        )?;
        *cached = Some((token.clone(), now));
        Ok(token)
    }

    /// Send one wake. An expired provider token permits one refresh and one
    /// retry; a connection failure permits one retry.
    pub async fn send(
        &self,
        device_token: &str,
        request_id: &str,
        expires_at: u64,
        now: u64,
    ) -> Outcome {
        let expiration = expires_at.min(now.saturating_add(self.max_expiration_seconds));
        let url = format!("{}/3/device/{device_token}", self.base);
        let mut force = false;
        let mut connection_retried = false;
        let mut token_retried = false;
        loop {
            let Ok(token) = self.provider_token(now, force).await else {
                return Outcome::ConfigurationFault;
            };
            let response = self
                .http
                .post(&url)
                .header("authorization", format!("bearer {token}"))
                .header("apns-topic", &self.topic)
                .header("apns-push-type", "alert")
                .header("apns-priority", "10")
                .header("apns-id", request_id)
                .header("apns-expiration", expiration.to_string())
                .header("content-type", "application/json")
                .body(nostr::push_lease::APNS_BODY)
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
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            let reason = body
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            match classify(status, reason) {
                Classified::RefreshToken if !token_retried => {
                    token_retried = true;
                    force = true;
                }
                Classified::RefreshToken => return Outcome::ConfigurationFault,
                Classified::Done(Outcome::InvalidEndpoint { .. }) => {
                    let invalid_at = body
                        .get("timestamp")
                        .and_then(serde_json::Value::as_u64)
                        .map(|millis| millis / 1_000);
                    return Outcome::InvalidEndpoint { invalid_at };
                }
                Classified::Done(outcome) => return outcome,
            }
        }
    }
}

enum Classified {
    Done(Outcome),
    RefreshToken,
}

/// Map an APNs status and reason to an outcome.
fn classify(status: u16, reason: &str) -> Classified {
    Classified::Done(match (status, reason) {
        (200, _) => Outcome::Accepted,
        (410, _) | (400, "BadDeviceToken" | "DeviceTokenNotForTopic") => {
            Outcome::InvalidEndpoint { invalid_at: None }
        }
        (403, "ExpiredProviderToken") => return Classified::RefreshToken,
        (403, _) => Outcome::ConfigurationFault,
        (429 | 500 | 503, _) => Outcome::Retry { after: None },
        _ => Outcome::RequestFault,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(status: u16, reason: &str) -> Option<Outcome> {
        match classify(status, reason) {
            Classified::Done(outcome) => Some(outcome),
            Classified::RefreshToken => None,
        }
    }

    #[test]
    fn apns_statuses_map_to_bounded_outcomes() {
        assert_eq!(done(200, ""), Some(Outcome::Accepted));
        assert_eq!(
            done(410, "Unregistered"),
            Some(Outcome::InvalidEndpoint { invalid_at: None })
        );
        assert_eq!(
            done(400, "BadDeviceToken"),
            Some(Outcome::InvalidEndpoint { invalid_at: None })
        );
        assert_eq!(done(403, "ExpiredProviderToken"), None);
        assert_eq!(
            done(403, "InvalidProviderToken"),
            Some(Outcome::ConfigurationFault)
        );
        assert_eq!(
            done(429, "TooManyRequests"),
            Some(Outcome::Retry { after: None })
        );
        assert_eq!(done(503, ""), Some(Outcome::Retry { after: None }));
        assert_eq!(done(413, "PayloadTooLarge"), Some(Outcome::RequestFault));
    }
}
