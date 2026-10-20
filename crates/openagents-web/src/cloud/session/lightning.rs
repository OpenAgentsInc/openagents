//! Bounded forwarding to the account service's Lightning address facade.
use super::device::Answer;
use super::{CloudSession, Result, SessionError};
impl CloudSession {
    pub async fn lightning_call(
        &self,
        method: reqwest::Method,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
        headers: &axum::http::HeaderMap,
    ) -> Result<Answer> {
        self.ready()?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| SessionError::Unavailable)?;
        let mut request = client.request(method, format!("{}{path}", self.account_service));
        if let Some(authorization) = authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization);
        }
        if !body.is_empty() {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        for key in ["x-breez-signature", "x-breez-timestamp"] {
            if let Some(value) = headers.get(key) {
                request = request.header(key, value);
            }
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| SessionError::Unavailable)?
        {
            if bytes.len() + chunk.len() > 256 * 1024 {
                return Err(SessionError::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = serde_json::from_slice(&bytes).map_err(|_| SessionError::Unavailable)?;
        Ok(Answer { status, body })
    }
}
