//! What the site's OAuth authorization server (`crate::oauth`, #11084)
//! needs from the account service and the server's secret: purpose-bound
//! keys derived from the CSRF secret (never the secret itself), and an
//! approved sign-in for an app, made under the browser's own session.
//!
//! The sign-in is the device grant (`oa_auth::device`) started and
//! approved in one step: the access token the app later gets is the
//! ordinary app session the device grant issues, listed in Settings'
//! Computers section with Remove.

use super::{CloudSession, Result, SessionError};
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

impl CloudSession {
    /// A 256-bit key for one OAuth purpose (`label`), derived from the
    /// server's secret. Every instance with the same secret derives the
    /// same key, so a code minted on one instance redeems on another.
    #[must_use]
    pub(crate) fn oauth_key(&self, label: &str) -> [u8; 32] {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.csrf_key).expect("HMAC accepts this key");
        mac.update(b"openagents.oauth.key.v1\0");
        mac.update(label.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&mac.finalize().into_bytes());
        key
    }

    /// Start a sign-in for `app` on `computer` and approve it as the
    /// signed-in viewer. Answers the device code the token endpoint
    /// redeems, or the account service's refusal code.
    pub(crate) async fn oauth_approve(
        &self,
        headers: &HeaderMap,
        app: &str,
        computer: &str,
    ) -> Result<std::result::Result<String, String>> {
        let started = self.device_start(app, computer, None).await?;
        if started.status != 200 {
            return Ok(Err(started.code().unwrap_or("invalid_request").to_string()));
        }
        let device_code = started.body["device_code"]
            .as_str()
            .filter(|code| code.starts_with("dvc_") && code.len() <= 128)
            .ok_or(SessionError::Unavailable)?
            .to_string();
        let user_code = started.body["user_code"]
            .as_str()
            .and_then(oa_auth::device::normalize_user_code)
            .ok_or(SessionError::Unavailable)?;
        match self.device_decide(headers, &user_code, true).await? {
            Ok(_) => Ok(Ok(device_code)),
            Err(code) => Ok(Err(code)),
        }
    }
}
