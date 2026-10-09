//! Device sign-in and signed-in apps through the account service
//! (`oa_auth::device`): the app's start and poll, the approval page's
//! lookup and decision under the browser's own session, and Settings'
//! list and Remove. Answers carry no transport detail.

use super::{CloudSession, Result, SESSION_COOKIE, SessionError, session_token, value};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

/// One signed-in app, as Settings lists it.
#[derive(Clone, Debug, Deserialize)]
pub struct AppSession {
    pub id: String,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub computer: String,
    pub created_at: u64,
    pub expires_at: u64,
}

/// What the approval page shows about a waiting sign-in.
#[derive(Clone, Debug, Deserialize)]
pub struct DeviceRequest {
    pub app: String,
    pub computer: String,
    pub expires_at: u64,
}

/// A sign-in waiting under a pair code, with the code its app shows.
#[derive(Clone, Debug, Deserialize)]
pub struct PairedRequest {
    pub user_code: String,
    pub app: String,
    pub computer: String,
    pub expires_at: u64,
}

/// Whether a pair code has the account service's shape.
fn tenancy_pair(pair: &str) -> bool {
    (16..=64).contains(&pair.len()) && pair.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The account service's answer to a device call: status and body. A
/// device flow's refusals (`authorization_pending`, `slow_down`, ...)
/// are answers, not failures.
pub struct Answer {
    pub status: u16,
    pub body: Value,
}

impl Answer {
    /// The refusal code, when it is one.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.body["error"]["code"].as_str()
    }
}

impl CloudSession {
    async fn device_call(
        &self,
        method: reqwest::Method,
        path: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> Result<Answer> {
        self.ready()?;
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.account_service))
            .timeout(Duration::from_secs(10));
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        if status >= 500 || bytes.len() > 64 * 1024 {
            return Err(SessionError::Unavailable);
        }
        let body = serde_json::from_slice(&bytes).map_err(|_| SessionError::Unavailable)?;
        Ok(Answer { status, body })
    }

    /// The browser's own session token, for calls made as the viewer.
    fn browser_token(&self, headers: &HeaderMap) -> Result<String> {
        self.request_host(headers)?;
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        if !session_token(&token) {
            return Err(SessionError::Unauthenticated);
        }
        Ok(token)
    }

    /// Start a device sign-in for an app. The answer adds the
    /// verification addresses on this site's origin (RFC 8628 3.2).
    pub async fn device_start(
        &self,
        app: &str,
        computer: &str,
        pair: Option<&str>,
    ) -> Result<Answer> {
        let mut body = json!({"app": app, "computer": computer});
        if let Some(pair) = pair.filter(|pair| tenancy_pair(pair)) {
            body["pair"] = json!(pair);
        }
        let mut answer = self
            .device_call(
                reqwest::Method::POST,
                "/v1/sessions/device",
                None,
                Some(body),
            )
            .await?;
        if answer.status == 200 {
            let user_code = answer.body["user_code"]
                .as_str()
                .ok_or(SessionError::Unavailable)?
                .to_string();
            answer.body["verification_uri"] = json!(format!("{}/device", self.origin));
            answer.body["verification_uri_complete"] =
                json!(format!("{}/device?code={user_code}", self.origin));
        }
        Ok(answer)
    }

    /// The app's poll.
    pub async fn device_poll(&self, device_code: &str) -> Result<Answer> {
        if !device_code.starts_with("dvc_") || device_code.len() > 128 {
            return Err(SessionError::InvalidRequest);
        }
        self.device_call(
            reqwest::Method::POST,
            "/v1/sessions/device/poll",
            None,
            Some(json!({"device_code": device_code})),
        )
        .await
    }

    /// The waiting sign-in a typed code names, as the signed-in viewer.
    pub async fn device_lookup(
        &self,
        headers: &HeaderMap,
        user_code: &str,
    ) -> Result<std::result::Result<DeviceRequest, String>> {
        let token = self.browser_token(headers)?;
        let answer = self
            .device_call(
                reqwest::Method::POST,
                "/v1/sessions/device/lookup",
                Some(&token),
                Some(json!({"user_code": user_code})),
            )
            .await?;
        match answer.status {
            200 => serde_json::from_value(answer.body["device"].clone())
                .map(Ok)
                .map_err(|_| SessionError::Unavailable),
            401 => Err(SessionError::Unauthenticated),
            _ => Ok(Err(answer.code().unwrap_or("invalid_grant").to_string())),
        }
    }

    /// The sign-ins waiting under the viewer's pair code (the Connect
    /// page's `coder login --pair`), newest first.
    pub async fn device_paired(
        &self,
        headers: &HeaderMap,
        pair: &str,
    ) -> Result<Vec<PairedRequest>> {
        let token = self.browser_token(headers)?;
        let answer = self
            .device_call(
                reqwest::Method::POST,
                "/v1/sessions/device/paired",
                Some(&token),
                Some(json!({"pair": pair})),
            )
            .await?;
        match answer.status {
            200 => serde_json::from_value(answer.body["devices"].clone())
                .map_err(|_| SessionError::Unavailable),
            401 => Err(SessionError::Unauthenticated),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// The viewer's pair code for `coder login --pair`: the same for the
    /// account on this server, unguessable without the server's key.
    #[must_use]
    pub fn pair_code(&self, viewer: &super::Viewer) -> String {
        use hmac::{Hmac, Mac};
        let mut mac =
            Hmac::<sha2::Sha256>::new_from_slice(&self.csrf_key).expect("HMAC accepts this key");
        mac.update(b"openagents.terminal.pair.v1\0");
        mac.update(viewer.account_id.as_bytes());
        const LETTERS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
        mac.finalize()
            .into_bytes()
            .iter()
            .take(20)
            .map(|byte| LETTERS[usize::from(*byte) % LETTERS.len()] as char)
            .collect()
    }

    /// The viewer's Approve or Deny.
    pub async fn device_decide(
        &self,
        headers: &HeaderMap,
        user_code: &str,
        approve: bool,
    ) -> Result<std::result::Result<DeviceRequest, String>> {
        let token = self.browser_token(headers)?;
        let answer = self
            .device_call(
                reqwest::Method::POST,
                "/v1/sessions/device/decide",
                Some(&token),
                Some(json!({"user_code": user_code, "approve": approve})),
            )
            .await?;
        match answer.status {
            200 => serde_json::from_value(answer.body["device"].clone())
                .map(Ok)
                .map_err(|_| SessionError::Unavailable),
            401 => Err(SessionError::Unauthenticated),
            _ => Ok(Err(answer.code().unwrap_or("invalid_grant").to_string())),
        }
    }

    /// The viewer's signed-in apps and computers.
    pub async fn app_sessions(&self, headers: &HeaderMap) -> Result<Vec<AppSession>> {
        let token = self.browser_token(headers)?;
        let answer = self
            .device_call(
                reqwest::Method::GET,
                "/v1/account/sessions",
                Some(&token),
                None,
            )
            .await?;
        match answer.status {
            200 => serde_json::from_value(answer.body["sessions"].clone())
                .map_err(|_| SessionError::Unavailable),
            401 => Err(SessionError::Unauthenticated),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// Sign one of the viewer's apps out.
    pub async fn revoke_app_session(&self, headers: &HeaderMap, id: &str) -> Result<()> {
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SessionError::InvalidRequest);
        }
        let token = self.browser_token(headers)?;
        let answer = self
            .device_call(
                reqwest::Method::DELETE,
                &format!("/v1/account/sessions/{id}"),
                Some(&token),
                None,
            )
            .await?;
        match answer.status {
            200 | 404 => Ok(()),
            401 => Err(SessionError::Unauthenticated),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// The account an app's own token (a signed-in Coder, #11046) belongs
    /// to, when the token is a live user session.
    pub async fn app_account(&self, token: &str) -> Result<String> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        let answer = self
            .device_call(reqwest::Method::GET, "/v1/session", Some(token), None)
            .await?;
        if answer.status == 401 || answer.status == 403 {
            return Err(SessionError::Unauthenticated);
        }
        if answer.status != 200 {
            return Err(SessionError::Unavailable);
        }
        let session = &answer.body["session"];
        let expires = session["expires_at"]
            .as_u64()
            .or_else(|| session["expires_at"].as_str().and_then(|v| v.parse().ok()))
            .unwrap_or(0);
        match session["account"].as_str() {
            Some(account)
                if session["kind"] == "user"
                    && session["state"] == "active"
                    && expires > super::now()
                    && super::identifier(account) =>
            {
                Ok(account.to_owned())
            }
            _ => Err(SessionError::Unauthenticated),
        }
    }

    /// An app signing itself out with its own token.
    pub async fn app_sign_out(&self, token: &str) -> Result<()> {
        if !session_token(token) {
            return Err(SessionError::InvalidRequest);
        }
        let answer = self
            .device_call(reqwest::Method::DELETE, "/v1/session", Some(token), None)
            .await?;
        match answer.status {
            200 | 401 => Ok(()),
            _ => Err(SessionError::Unavailable),
        }
    }
}
