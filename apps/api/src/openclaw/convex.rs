use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::form_urlencoded;
use worker::{Env, Method, Result};

use crate::ApiResponse;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenclawInstance {
    pub user_id: String,
    pub status: String,
    pub runtime_url: Option<String>,
    pub runtime_name: Option<String>,
    pub cf_account_id: Option<String>,
    pub cf_worker_name: Option<String>,
    pub cf_worker_id: Option<String>,
    pub cf_container_app_id: Option<String>,
    pub cf_container_app_name: Option<String>,
    pub r2_bucket_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_ready_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BillingSummary {
    pub user_id: String,
    pub balance_usd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct SecretResponse {
    secret: Option<String>,
}

fn encode_query(pairs: &[(String, String)]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

async fn convex_json<T: DeserializeOwned>(
    env: &Env,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<ApiResponse<T>> {
    let body_text = body
        .map(|value| serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string()));
    let mut response = crate::forward_convex_control(env, method, path, body_text, None, None).await?;
    let status = response.status_code();
    if !(200..=299).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(worker::Error::RustError(format!(
            "convex error {status}: {text}"
        )));
    }
    response.json().await
}

pub async fn get_instance(env: &Env, user_id: &str) -> Result<Option<OpenclawInstance>> {
    let query = encode_query(&[("user_id".to_string(), user_id.to_string())]);
    let path = format!("control/openclaw/instance?{query}");
    let response: ApiResponse<Option<OpenclawInstance>> = convex_json(env, Method::Get, &path, None).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    Ok(response.data.flatten())
}

pub async fn upsert_instance(
    env: &Env,
    payload: serde_json::Value,
) -> Result<OpenclawInstance> {
    let response: ApiResponse<OpenclawInstance> =
        convex_json(env, Method::Post, "control/openclaw/instance", Some(payload)).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    response
        .data
        .ok_or_else(|| worker::Error::RustError("missing instance".to_string()))
}

pub async fn set_status(env: &Env, user_id: &str, status: &str) -> Result<OpenclawInstance> {
    let payload = serde_json::json!({
        "user_id": user_id,
        "status": status,
    });
    let response: ApiResponse<OpenclawInstance> =
        convex_json(env, Method::Post, "control/openclaw/instance/status", Some(payload)).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    response
        .data
        .ok_or_else(|| worker::Error::RustError("missing instance".to_string()))
}

pub async fn store_secret(env: &Env, user_id: &str, key: &str, value: &str) -> Result<()> {
    let payload = serde_json::json!({
        "user_id": user_id,
        "key": key,
        "value": value,
    });
    let response: ApiResponse<serde_json::Value> =
        convex_json(env, Method::Post, "control/openclaw/instance/secret", Some(payload)).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    Ok(())
}

pub async fn get_secret(env: &Env, user_id: &str, key: &str) -> Result<Option<String>> {
    let query = encode_query(&[
        ("user_id".to_string(), user_id.to_string()),
        ("key".to_string(), key.to_string()),
    ]);
    let path = format!("control/openclaw/instance/secret?{query}");
    let response: ApiResponse<SecretResponse> = convex_json(env, Method::Get, &path, None).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    Ok(response.data.and_then(|value| value.secret))
}

pub async fn get_billing_summary(env: &Env, user_id: &str) -> Result<BillingSummary> {
    let query = encode_query(&[("user_id".to_string(), user_id.to_string())]);
    let path = format!("control/openclaw/billing/summary?{query}");
    let response: ApiResponse<BillingSummary> = convex_json(env, Method::Get, &path, None).await?;
    if !response.ok {
        return Err(worker::Error::RustError(
            response.error.unwrap_or_else(|| "convex error".to_string()),
        ));
    }
    response
        .data
        .ok_or_else(|| worker::Error::RustError("missing billing summary".to_string()))
}
