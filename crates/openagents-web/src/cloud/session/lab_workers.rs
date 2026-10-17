//! Job-only worker credentials are issued and rechecked by the account service.
use super::{CloudSession, Result, SessionError};
use axum::http::HeaderMap;
use lab_jobs::{
    authority::Enrolled,
    protocol::{Access, Enroll},
};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

impl CloudSession {
    async fn lab_worker_call<T: DeserializeOwned>(
        &self,
        headers: &HeaderMap,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        self.ready()?;
        if headers.get_all("Authorization").iter().count() != 1 {
            return Err(SessionError::Unauthenticated);
        }
        let token = headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .filter(|s| !s.is_empty() && s.len() <= 512)
            .ok_or(SessionError::Unauthenticated)?;
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.account_service))
            .bearer_auth(token)
            .timeout(std::time::Duration::from_secs(10));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        match response.status().as_u16() {
            200 | 201 => (),
            401 => return Err(SessionError::Unauthenticated),
            403 | 404 => return Err(SessionError::Forbidden),
            400 | 422 => return Err(SessionError::InvalidRequest),
            _ => return Err(SessionError::Unavailable),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| SessionError::Unavailable)?
        {
            if bytes.len().saturating_add(chunk.len()) > 64 * 1024 {
                return Err(SessionError::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| SessionError::Unavailable)
    }
    pub(crate) async fn enroll_lab_worker(
        &self,
        headers: &HeaderMap,
        request: &Enroll,
    ) -> Result<Enrolled> {
        self.lab_worker_call(
            headers,
            Method::POST,
            "/v1/account/lab-workers",
            Some(serde_json::to_value(request).map_err(|_| SessionError::InvalidRequest)?),
        )
        .await
    }
    pub(crate) async fn lab_worker_access(&self, headers: &HeaderMap) -> Result<Access> {
        self.lab_worker_call(headers, Method::POST, "/v1/account/lab-worker-access", None)
            .await
    }
    pub(crate) async fn revoke_lab_worker(&self, headers: &HeaderMap, id: &str) -> Result<()> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SessionError::InvalidRequest);
        }
        let _: Value = self
            .lab_worker_call(
                headers,
                Method::DELETE,
                &format!("/v1/account/lab-workers/{id}"),
                None,
            )
            .await?;
        Ok(())
    }
}
