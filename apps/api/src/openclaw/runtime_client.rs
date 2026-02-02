use serde::Deserialize;
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

    pub async fn backup(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/storage/backup", None).await
    }

    pub async fn restart(&self) -> Result<RuntimeResult<serde_json::Value>> {
        self.request_json(Method::Post, "/v1/gateway/restart", None).await
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
