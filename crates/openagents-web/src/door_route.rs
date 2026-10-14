//! `openagents.com/v1/responses` and `/v1/chat/completions` for a model id
//! go to the inference gateway, the same router and class table
//! `/api/v1/...` reaches (#11597).
//!
//! Without this, every `/v1` path went to `coder-serve`, whose generation
//! door is the Vercel AI Gateway: `google/gemini-4-argon` there answered
//! 402 (the account has no credit) while the gateway serves it from Vertex
//! first (docs/inference/providers.md). `coder-serve` keeps what is its
//! own: a body that names no model or one of its lane words (a run,
//! `chat`, `free`, `flash`, `pro`, `mesh`, `local`), and a turn a run's
//! agent posts with the run header, which is metered on that run. The
//! lane words are exact values of the `model` field, read only after the
//! path has picked this route.
//!
//! A caller whose credential the gateway does not know (`401`) is answered
//! by `coder-serve` as before, so a key only that server issued still
//! works. Any other gateway answer, a 402 on the caller's own balance
//! included, is final: the gateway has already fallen through its own
//! upstreams.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::upstream::Upstream;

/// The paths this route reads.
const PATHS: [&str; 2] = ["/v1/responses", "/v1/chat/completions"];

/// The `model` words `coder-serve` answers itself (its `LANES`, `coder`,
/// and `local`); `mesh/<row>` too.
const LANE_WORDS: [&str; 7] = ["coder", "chat", "free", "flash", "pro", "mesh", "local"];

/// The header a run's agent sends with its turns (`coder_runner::door`).
const RUN_HEADER: &str = "x-coder-run";

/// The largest body this route reads before choosing (Cloud Run's own
/// request limit).
const LIMIT: usize = 32 * 1024 * 1024;

/// Whether `method path` is one this route chooses for.
#[must_use]
pub(crate) fn applies(method: &Method, path: &str) -> bool {
    method == Method::POST && PATHS.contains(&path)
}

/// Whether a body's `model` is a model id for the gateway, rather than
/// nothing, a lane word, or a body that does not parse.
#[must_use]
pub(crate) fn names_gateway_model(body: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Ask {
        #[serde(default)]
        model: Option<String>,
    }
    let Ok(Ask { model: Some(model) }) = serde_json::from_slice::<Ask>(body) else {
        return false;
    };
    let model = model.trim();
    !model.is_empty() && !LANE_WORDS.contains(&model) && !model.starts_with("mesh/")
}

/// Sends the request to the gateway when it names a model id, else (or on
/// a credential the gateway does not know) to `coder`.
pub(crate) async fn route(request: Request, gateway: &Upstream, coder: &Upstream) -> Response {
    if request.headers().contains_key(RUN_HEADER) {
        return coder.forward(request).await;
    }
    let (parts, body) = request.into_parts();
    let body: Bytes = match axum::body::to_bytes(body, LIMIT).await {
        Ok(body) => body,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let rebuild = |cookies: bool| {
        let mut request = Request::new(Body::from(body.clone()));
        *request.method_mut() = parts.method.clone();
        *request.uri_mut() = parts.uri.clone();
        *request.version_mut() = parts.version;
        *request.headers_mut() = parts.headers.clone();
        *request.extensions_mut() = parts.extensions.clone();
        if !cookies {
            request.headers_mut().remove(header::COOKIE);
        }
        request
    };
    if !names_gateway_model(&body) {
        return coder.forward(rebuild(true)).await;
    }
    // As on `/api/v1`: an API call carries only its own key.
    let answer = gateway.forward(rebuild(false)).await;
    if answer.status() == StatusCode::UNAUTHORIZED {
        return coder.forward(rebuild(true)).await;
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_posts_to_the_two_inference_paths_apply() {
        assert!(applies(&Method::POST, "/v1/responses"));
        assert!(applies(&Method::POST, "/v1/chat/completions"));
        assert!(!applies(&Method::GET, "/v1/responses"));
        assert!(!applies(&Method::POST, "/v1/responses/compact"));
        assert!(!applies(&Method::POST, "/v1/models"));
    }

    #[test]
    fn model_ids_go_to_the_gateway_and_lane_words_stay() {
        for model in [
            "google/gemini-4-argon",
            "google/gemini-3.8-flash",
            "openagents/fast",
            "openai/gpt-6.1",
        ] {
            let body = serde_json::json!({ "model": model, "input": "hi" }).to_string();
            assert!(names_gateway_model(body.as_bytes()), "{model}");
        }
        for model in [
            "coder", "chat", "free", "flash", "pro", "mesh", "mesh/a", "local", "",
        ] {
            let body = serde_json::json!({ "model": model, "input": "hi" }).to_string();
            assert!(!names_gateway_model(body.as_bytes()), "{model}");
        }
        assert!(!names_gateway_model(br#"{"input":"open a run"}"#));
        assert!(!names_gateway_model(b"not json"));
    }
}
