//! Public suggestion-box configuration, fetched from its dedicated inbox.
use axum::{Json, http::StatusCode};
use playtest::suggestion::InboxConfig;

pub(crate) async fn configuration() -> Result<Json<InboxConfig>, StatusCode> {
    let base = std::env::var("OPENAGENTS_SUGGESTION_INBOX")
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let config = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .get(format!("{}/config.json", base.trim_end_matches('/')))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .json::<InboxConfig>()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    config
        .check()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(config))
}
