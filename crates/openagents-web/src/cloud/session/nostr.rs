//! Sign in with a Nostr key (#11993) through the account service
//! (`oa_auth::nostr`). The signed request's body is forwarded byte for
//! byte, because the event signs its hash.

use std::time::Duration;

use super::device::Answer;
use super::{CloudSession, Result, SessionError};

impl CloudSession {
    /// One call to the account service's Nostr routes, with `authorization`
    /// (`Nostr …` or `Bearer sess_…`) and the exact `body`. Not tried again:
    /// each signed request works once.
    pub async fn nostr_call(
        &self,
        method: reqwest::Method,
        path: &str,
        authorization: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<Answer> {
        self.ready()?;
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.account_service))
            .timeout(Duration::from_secs(10));
        if let Some(value) = authorization {
            request = request.header(reqwest::header::AUTHORIZATION, value);
        }
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
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
}
