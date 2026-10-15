//! HTTP transport over the same fixed-schema in-process service.

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State, rejection::StringRejection},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tokio::sync::Semaphore;

use crate::{
    Error, Result,
    service::{self, MAX_BODY_BYTES, Refusal, RouterService},
};

struct Shared {
    model: Mutex<RouterService>,
    entry: serde_json::Value,
    capacity: Arc<Semaphore>,
}

pub fn router(mut model: RouterService, max_queue: usize) -> Result<Router> {
    if !(1..=256).contains(&max_queue) {
        return Err(Error("Choose a queue limit between 1 and 256".into()));
    }
    // Readiness is exposed only after the selected engine answers successfully.
    model.warm_up()?;
    let entry = model.model_entry();
    let shared = Arc::new(Shared {
        model: Mutex::new(model),
        entry,
        capacity: Arc::new(Semaphore::new(max_queue)),
    });
    Ok(Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status":"ok"})) }),
        )
        .route("/v1/models", get(models))
        .route("/v1/systemone", post(decide))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(shared))
}

fn refusal(error: Refusal) -> Response {
    let status = StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = (status, Json(error.body())).into_response();
    if status == StatusCode::SERVICE_UNAVAILABLE {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, header::HeaderValue::from_static("1"));
    }
    response
}

async fn models(State(shared): State<Arc<Shared>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"object":"list","data":[shared.entry]}))
}

async fn decide(
    State(shared): State<Arc<Shared>>,
    body: std::result::Result<String, StringRejection>,
) -> Response {
    let text = match body {
        Ok(body) => body,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return refusal(Refusal::new(
                413,
                "not_admitted",
                "This request is too large",
            ));
        }
        Err(_) => {
            return refusal(Refusal::new(
                400,
                "invalid_request",
                "Provide a UTF-8 JSON request",
            ));
        }
    };
    let value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            return refusal(Refusal::new(
                400,
                "invalid_request",
                "Provide a valid JSON request",
            ));
        }
    };
    let request = match service::validate(value) {
        Ok(request) => request,
        Err(error) => return refusal(error),
    };
    let permit = match shared.capacity.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return refusal(Refusal::new(
                503,
                "busy",
                "The model is busy; try again shortly",
            ));
        }
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        shared
            .model
            .lock()
            .map_err(|_| Refusal::new(500, "internal", "The model is unavailable"))?
            .decide_validated(request)
    })
    .await
    {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => refusal(error),
        Err(_) => refusal(Refusal::new(
            500,
            "internal",
            "The model could not finish this request",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    #[tokio::test]
    async fn validates_before_inference_and_warms_before_health() {
        let counter = Arc::new(AtomicUsize::new(0));
        assert!(router(service::fixture_service(counter.clone(), true), 1).is_err());
        let counter = Arc::new(AtomicUsize::new(0));
        let app = router(service::fixture_service(counter.clone(), false), 1).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
        let bad = serde_json::json!({"state":"Hello","questions":{"other":{"type":"noul","instructions":"Anything else?"}}});
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/systemone")
                    .body(Body::from(bad.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        let oversized = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/systemone")
                    .body(Body::from("x".repeat(MAX_BODY_BYTES + 1)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn full_queue_returns_retryable_busy_without_running_the_model() {
        let counter = Arc::new(AtomicUsize::new(0));
        let shared = Arc::new(Shared {
            model: Mutex::new(service::fixture_service(counter.clone(), false)),
            entry: serde_json::Value::Null,
            capacity: Arc::new(Semaphore::new(1)),
        });
        let _hold = shared.capacity.clone().acquire_owned().await.unwrap();
        let request = serde_json::json!({"state":"Hello","questions":service::questions()});
        let response = decide(State(shared), Ok(request.to_string())).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
