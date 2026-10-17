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

/// One sign-in key (#11305), as Settings and `GET /v1/keys` list it:
/// never its secret.
#[derive(Clone, Debug, Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct SignInKey {
    pub id: String,
    pub name: String,
    pub created_at: u64,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    /// `None` for a key that works until revoked.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Whether the request was made with this key.
    #[serde(default)]
    pub current: bool,
}

/// A new key and its secret, shown once.
pub struct MadeKey {
    pub key: SignInKey,
    pub token: String,
}

impl std::fmt::Debug for MadeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MadeKey")
            .field("key", &self.key)
            .field("token", &"[redacted]")
            .finish()
    }
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
    /// Fresh account authority for computer operations. The account service
    /// validates sessions, sign-in keys, and account-scoped API keys; prefixes
    /// alone never grant access, and a decision-only API key is refused there.
    /// A transport failure or a 5xx is tried once more. A refused session is
    /// not tried again, and a result is never remembered for a later call.
    pub(crate) async fn computer_account(&self, token: &str) -> Result<(String, bool)> {
        if !(session_token(token) || token.starts_with("oak_"))
            || token.len() > 512
            || token
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(SessionError::Unauthenticated);
        }
        for attempt in 0..2 {
            match self
                .device_call(reqwest::Method::GET, "/v1/account", Some(token), None)
                .await
            {
                Ok(answer) => {
                    return match answer.status {
                        200 => {
                            let id = answer.body["account"]["id"]
                                .as_str()
                                .filter(|id| super::identifier(id))
                                .ok_or(SessionError::Unavailable)?;
                            Ok((
                                id.to_owned(),
                                answer.body["account"]["admin"].as_bool() == Some(true),
                            ))
                        }
                        401 => Err(SessionError::Unauthenticated),
                        403 | 404 => Err(SessionError::Forbidden),
                        _ => Err(SessionError::Unavailable),
                    };
                }
                Err(SessionError::Unavailable) if attempt == 0 => {}
                Err(error) => return Err(error),
            }
        }
        Err(SessionError::Unavailable)
    }

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

    /// One account-service call as the browser's session. It is not tried
    /// again: a create or a link issue must not run twice.
    pub(crate) async fn account_json(
        &self,
        method: reqwest::Method,
        path: &str,
        headers: &HeaderMap,
        body: Option<Value>,
    ) -> Result<Value> {
        let token = self.browser_token(headers)?;
        let answer = self.device_call(method, path, Some(&token), body).await?;
        match answer.status {
            200 | 201 => Ok(answer.body),
            400 => Err(SessionError::InvalidRequest),
            401 => Err(SessionError::Unauthenticated),
            403 => Err(SessionError::Forbidden),
            409 => Err(SessionError::Conflict),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// Whether the account service still has this short referral name.
    /// A failure is a no, so an unconfirmed name sets nothing.
    pub(crate) async fn referral_handle_active(&self, handle: &str) -> bool {
        if !tenancy::accounts::referrals::handle_word(handle) {
            return false;
        }
        let url = format!("{}/join", self.account_service.trim_end_matches('/'));
        match self
            .http
            .get(url)
            .query(&[("ref", handle)])
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// The browser's own session token, for calls made as the viewer.
    pub(crate) fn browser_token(&self, headers: &HeaderMap) -> Result<String> {
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
        self.app_sessions_as(&token).await
    }

    /// The same, under a signed-in app's own token (#11134).
    pub async fn app_sessions_as(&self, token: &str) -> Result<Vec<AppSession>> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        let answer = self
            .device_call(
                reqwest::Method::GET,
                "/v1/account/sessions",
                Some(token),
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

    /// The account a live user session or sign-in key acts for.
    pub async fn app_account(&self, token: &str) -> Result<String> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        if token.starts_with(super::SIGN_IN_KEY_PREFIX) {
            let account = self
                .client(token)?
                .account()
                .details()
                .await
                .map_err(|error| match super::native_error(error) {
                    SessionError::Forbidden => SessionError::Unauthenticated,
                    error => error,
                })?
                .account
                .id;
            return if super::identifier(&account) {
                Ok(account)
            } else {
                Err(SessionError::Unauthenticated)
            };
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

    /// The whole signed-in view (account, workspaces) for an app's own
    /// token, as a browser session reads it, for a request an app makes
    /// for the account itself (`coder export --account`, #11134). The
    /// account's own workspace is selected, as Settings selects it for a
    /// browser that has none; the token must be a live user session.
    pub async fn app_viewer(&self, token: &str) -> Result<super::Viewer> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        let viewer = self.read_view(token, None).await?;
        match crate::cloud::default_workspace(&viewer).map(|w| w.id.clone()) {
            Some(id) => self.read_view(token, Some(&id)).await,
            None => Ok(viewer),
        }
    }

    /// The sign-in keys of the account `token` (a sign-in or a sign-in
    /// key) acts for, newest first (`oa_auth::sign_in_keys::list`).
    pub async fn sign_in_keys(&self, token: &str) -> Result<Vec<SignInKey>> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        let answer = self
            .device_call(
                reqwest::Method::GET,
                "/v1/account/sign-in-keys",
                Some(token),
                None,
            )
            .await?;
        match answer.status {
            200 => serde_json::from_value(answer.body["keys"].clone())
                .map_err(|_| SessionError::Unavailable),
            401 | 403 => Err(SessionError::Unauthenticated),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// Make a sign-in key named `name`, working `days` days or until
    /// revoked. The inner error is the account service's words for a
    /// name or expiry it won't take, or for too many keys.
    pub async fn create_sign_in_key(
        &self,
        token: &str,
        name: &str,
        days: Option<u64>,
    ) -> Result<std::result::Result<MadeKey, String>> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        let answer = self
            .device_call(
                reqwest::Method::POST,
                "/v1/account/sign-in-keys",
                Some(token),
                Some(json!({"name": name, "expires_in_days": days})),
            )
            .await?;
        match answer.status {
            200 => {
                let key: SignInKey = serde_json::from_value(answer.body["key"].clone())
                    .map_err(|_| SessionError::Unavailable)?;
                let token = answer.body["token"]
                    .as_str()
                    .filter(|t| t.starts_with(super::SIGN_IN_KEY_PREFIX) && session_token(t))
                    .ok_or(SessionError::Unavailable)?
                    .to_owned();
                Ok(Ok(MadeKey { key, token }))
            }
            401 | 403 => Err(SessionError::Unauthenticated),
            400 | 409 => Ok(Err(answer.body["error"]["message"]
                .as_str()
                .unwrap_or("That key couldn't be made.")
                .to_owned())),
            _ => Err(SessionError::Unavailable),
        }
    }

    /// Revoke one of the account's sign-in keys. `false`: no working key
    /// of the account's has that id.
    pub async fn revoke_sign_in_key(&self, token: &str, id: &str) -> Result<bool> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated);
        }
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let answer = self
            .device_call(
                reqwest::Method::DELETE,
                &format!("/v1/account/sign-in-keys/{id}"),
                Some(token),
                None,
            )
            .await?;
        match answer.status {
            200 => Ok(true),
            404 => Ok(false),
            401 | 403 => Err(SessionError::Unauthenticated),
            _ => Err(SessionError::Unavailable),
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
