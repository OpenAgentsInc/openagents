use serde::Deserialize;
use url::form_urlencoded;
use worker::{Headers, Method, Request, RequestInit, Result};

use crate::openclaw::SERVICE_TOKEN_HEADER;

#[derive(Debug, Deserialize)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeEnvelope<T> {
    pub ok: bool,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<RuntimeError>,
}

#[derive(Debug)]
pub struct RuntimeResult<T> {
    pub status: u16,
    pub envelope: RuntimeEnvelope<T>,
}

pub struct RuntimeClient {
    base_url: String,
    service_token: String,
}

impl RuntimeClient {
    pub fn new(base_url: String, service_token: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            service_token,
        }
    }

    pub async fn status(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Get, "/v1/status", None).await
    }

    pub async fn devices(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Get, "/v1/devices", None).await
    }

    pub async fn approve_device(&self, request_id: &str) -> Result<RuntimeResult<serde_json::Value>> {
        let path = format!("/v1/devices/{request_id}/approve");
        self.request_json(Method::Post, &path, None).await
    }

    pub async fn pairing_list(
        &self,
        channel: &str,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let encoded = encode_path_segment(channel);
        let path = format!("/v1/pairing/{encoded}");
        self.request_json(Method::Get, &path, None).await
    }

    pub async fn pairing_approve(
        &self,
        channel: &str,
        code: &str,
        notify: Option<bool>,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let encoded = encode_path_segment(channel);
        let path = format!("/v1/pairing/{encoded}/approve");
        let mut body = serde_json::json!({ "code": code });
        if let Some(value) = notify {
            body["notify"] = serde_json::Value::Bool(value);
        }
        self.request_json(Method::Post, &path, Some(body)).await
    }

    pub async fn backup(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/storage/backup", None).await
    }

    pub async fn restart(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/gateway/restart", None).await
    }

    pub async fn stop(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/gateway/stop", None).await
    }

    pub async fn tools_invoke(
        &self,
        payload: serde_json::Value,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/tools/invoke", Some(payload))
            .await
    }

    pub async fn sessions_list(
        &self,
        query: Option<&str>,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let path = build_path_with_query("/v1/sessions", query);
        self.request_json(Method::Get, &path, None).await
    }

    pub async fn sessions_history(
        &self,
        session_key: &str,
        query: Option<&str>,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let encoded = encode_path_segment(session_key);
        let path = build_path_with_query(&format!("/v1/sessions/{encoded}/history"), query);
        self.request_json(Method::Get, &path, None).await
    }

    pub async fn sessions_send(
        &self,
        session_key: &str,
        body: serde_json::Value,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let encoded = encode_path_segment(session_key);
        let path = format!("/v1/sessions/{encoded}/send");
        self.request_json(Method::Post, &path, Some(body)).await
    }

    pub async fn responses_stream(
        &self,
        body: String,
        extra_headers: &[(String, String)],
    ) -> Result<worker::Response> {
        let url = crate::join_url(&self.base_url, "/v1/responses", "");
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let headers = Headers::new();
        headers.set("accept", "text/event-stream")?;
        headers.set("content-type", "application/json")?;
        headers.set(SERVICE_TOKEN_HEADER, &self.service_token)?;
        for (name, value) in extra_headers {
            headers.set(name, value)?;
        }
        init.with_headers(headers);
        if !body.is_empty() {
            init.with_body(Some(body.into()));
        }

        let outbound = Request::new_with_init(&url, &init)?;
        worker::Fetch::Request(outbound).send().await
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<RuntimeResult<serde_json::Value>> {
        let url = crate::join_url(&self.base_url, path, "");
        let mut init = RequestInit::new();
        init.with_method(method);
        let headers = Headers::new();
        headers.set("accept", "application/json")?;
        headers.set("content-type", "application/json")?;
        headers.set(SERVICE_TOKEN_HEADER, &self.service_token)?;
        init.with_headers(headers);
        if let Some(body) = body {
            let body_text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
            init.with_body(Some(body_text.into()));
        }

        let outbound = Request::new_with_init(&url, &init)?;
        let mut response = worker::Fetch::Request(outbound).send().await?;
        let status = response.status_code();
        let bytes = response.bytes().await.unwrap_or_default();
        let envelope = match serde_json::from_slice::<RuntimeEnvelope<serde_json::Value>>(&bytes) {
            Ok(parsed) => parsed,
            Err(_) => {
                let message = String::from_utf8_lossy(&bytes).trim().to_string();
                RuntimeEnvelope {
                    ok: false,
                    data: None,
                    error: Some(RuntimeError {
                        code: "invalid_response".to_string(),
                        message: if message.is_empty() {
                            format!("runtime http {status}")
                        } else {
                            message
                        },
                        details: None,
                    }),
                }
            }
        };

        Ok(RuntimeResult { status, envelope })
    }
}

fn encode_path_segment(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn build_path_with_query(path: &str, query: Option<&str>) -> String {
    match query {
        Some(q) if !q.trim().is_empty() => format!("{path}?{q}"),
        _ => path.to_string(),
    }
}
