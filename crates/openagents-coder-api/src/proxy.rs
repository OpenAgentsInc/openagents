//! Provider fan-in. Pinned grants never receive the Vercel fallback list.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde_json::{json, Value};

use crate::catalog::Model;
use crate::config::Config;

const VERCEL: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";
const OPENROUTER: &str = "https://openrouter.ai/api/v1/chat/completions";

pub async fn stream_completion(
    config: &Config,
    model: &Model,
    mut body: Value,
    pinned: bool,
) -> Result<Response, (StatusCode, String)> {
    let (url, key) = match model.provider.as_str() {
        "vercel_gateway" => (
            VERCEL,
            config.ai_gateway_api_key.clone().ok_or((
                StatusCode::SERVICE_UNAVAILABLE,
                "vercel gateway is not configured".into(),
            ))?,
        ),
        "openrouter" => (
            OPENROUTER,
            config.openrouter_api_key.clone().ok_or((
                StatusCode::SERVICE_UNAVAILABLE,
                "openrouter is not configured".into(),
            ))?,
        ),
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("provider `{other}` is not served"),
            ))
        }
    };

    prepare_upstream_body(&mut body, model, pinned);

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let upstream = client
        .post(url)
        .bearer_auth(key)
        .header("accept", "text/event-stream")
        .json(&body)
        .send()
        .await
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;

    if !upstream.status().is_success() {
        let status =
            StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let text = upstream.text().await.unwrap_or_default();
        return Err((status, text));
    }

    let label = model.id.clone();
    let byte_stream = upstream
        .bytes_stream()
        .map(|chunk| chunk.map_err(|error| std::io::Error::other(error.to_string())));
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("x-openagents-model"),
        HeaderValue::from_str(&label).unwrap_or(HeaderValue::from_static("unresolved")),
    );

    Ok((headers, Body::from_stream(byte_stream)).into_response())
}

pub fn prepare_upstream_body(body: &mut Value, model: &Model, pinned: bool) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    obj.insert("model".into(), json!(model.provider_model));
    obj.insert("stream".into(), json!(true));
    obj.entry("stream_options")
        .or_insert_with(|| json!({"include_usage": true}));
    obj.entry("max_tokens")
        .or_insert_with(|| json!(model.max_output));
    if model.provider == "vercel_gateway" {
        let mut gateway = serde_json::Map::new();
        gateway.insert("order".into(), json!(["vertex"]));
        // Pin integrity: a grant that named a model is not eligible for
        // gateway.models fallback. Attribution-only is not a pin.
        if !pinned {
            gateway.insert(
                "models".into(),
                json!(["zai/glm-5.3-flash", "zai/glm-5.3", "zai/glm-5.2"]),
            );
        }
        obj.insert("providerOptions".into(), json!({ "gateway": gateway }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Model;

    fn gemini() -> Model {
        Model {
            id: "gemini-3.7-flash".into(),
            provider: "vercel_gateway".into(),
            provider_model: "google/gemini-3.7-flash".into(),
            context_window: 1,
            max_output: 64,
            pricing_basis: "placeholder".into(),
            pricing_id: "p".into(),
            input_per_million: 0,
            output_per_million: 0,
            cached_input_per_million: None,
        }
    }

    #[test]
    fn a_pinned_grant_does_not_send_gateway_fallback_models() {
        let mut body = json!({"model": "gemini-3.7-flash", "messages": []});
        prepare_upstream_body(&mut body, &gemini(), true);
        assert_eq!(body["model"], "google/gemini-3.7-flash");
        assert!(body["providerOptions"]["gateway"]["models"].is_null());
        assert_eq!(
            body["providerOptions"]["gateway"]["order"],
            json!(["vertex"])
        );
    }

    #[test]
    fn an_unpinned_call_may_include_fallback_models() {
        let mut body = json!({"model": "gemini-3.7-flash"});
        prepare_upstream_body(&mut body, &gemini(), false);
        assert!(body["providerOptions"]["gateway"]["models"].is_array());
    }
}
