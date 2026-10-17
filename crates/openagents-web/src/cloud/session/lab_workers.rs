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
        } else {
            request = request.header(reqwest::header::CONTENT_LENGTH, 0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, body::Body, http::Request, routing::post};
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn bodyless_worker_mutations_send_zero_length_framing() {
        async fn accept(request: Request<Body>) -> Json<Value> {
            assert_eq!(request.headers()[reqwest::header::CONTENT_LENGTH], "0");
            assert_eq!(
                request.headers()["Authorization"],
                "Bearer synthetic-worker-key"
            );
            assert!(
                axum::body::to_bytes(request.into_body(), 1)
                    .await
                    .unwrap()
                    .is_empty()
            );
            Json(json!({"accepted": true}))
        }
        let router = Router::new().route("/worker", post(accept).delete(accept));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let root = tempfile::tempdir().unwrap();
        let private = root.path().canonicalize().unwrap().join("private");
        std::fs::create_dir(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let secret = private.join("csrf.key");
        std::fs::write(&secret, [11; 32]).unwrap();
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config = private.join("cloud.json");
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({
                "schema":"openagents.cloud.web-config.v1", "public_origin":origin,
                "account_service":origin, "csrf_secret":secret
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
        let session = CloudSession::load(&config).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            "Bearer synthetic-worker-key".parse().unwrap(),
        );
        for method in [Method::POST, Method::DELETE] {
            let result: Value = session
                .lab_worker_call(&headers, method, "/worker", None)
                .await
                .unwrap();
            assert_eq!(result["accepted"], true);
        }
        server.abort();
    }
}
