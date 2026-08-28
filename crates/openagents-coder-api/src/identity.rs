//! Phoenix identity and credit reads. The rust API does not query Cloud SQL.

use axum::http::StatusCode;
use serde_json::Value;

/// Forward one GET to Phoenix with the caller's Bearer.
pub async fn phoenix_get(
    origin: &str,
    path: &str,
    bearer: &str,
) -> Result<(StatusCode, Value), (StatusCode, String)> {
    let url = format!("{}{}", origin.trim_end_matches('/'), path);
    let response = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .get(url)
        .bearer_auth(bearer)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let value = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| serde_json::json!({"code": "upstream", "message": "unreadable body"}));
    Ok((status, value))
}
