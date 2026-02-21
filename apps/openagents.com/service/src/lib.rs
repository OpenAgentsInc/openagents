use std::sync::Arc;
use std::time::SystemTime;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub mod config;

use crate::config::Config;

const SERVICE_NAME: &str = "openagents-control-service";

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    started_at: SystemTime,
}

#[derive(Debug, Serialize)]
struct RootResponse {
    service: &'static str,
    version: &'static str,
    docs: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    static_dir: String,
}

#[derive(Debug, Serialize)]
struct NotImplementedResponse {
    error: &'static str,
    message: &'static str,
    next_issue: &'static str,
}

pub fn build_router(config: Config) -> Router {
    let state = AppState {
        config: Arc::new(config),
        started_at: SystemTime::now(),
    };

    let static_service = ServeDir::new(state.config.static_dir.clone());

    Router::new()
        .route("/", get(root))
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route("/api/v1/auth/session", get(auth_session_placeholder))
        .route("/api/v1/control/status", get(control_status_placeholder))
        .route("/api/v1/sync/token", post(sync_token_placeholder))
        .nest_service("/assets", static_service)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(TraceLayer::new_for_http()),
        )
}

async fn root() -> Json<RootResponse> {
    Json(RootResponse {
        service: SERVICE_NAME,
        version: env!("CARGO_PKG_VERSION"),
        docs: "apps/openagents.com/service/README.md",
    })
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_seconds = match state.started_at.elapsed() {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    };

    Json(HealthResponse {
        status: "ok",
        service: SERVICE_NAME,
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds,
    })
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let static_dir = state.config.static_dir.to_string_lossy().to_string();

    if state.config.static_dir.is_dir() {
        return (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "ready",
                static_dir,
            }),
        );
    }

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ReadinessResponse {
            status: "not_ready",
            static_dir,
        }),
    )
}

async fn auth_session_placeholder() -> impl IntoResponse {
    not_implemented(
        "Auth/session API wiring lands in the OA-RUST-016 milestone.",
        "OA-RUST-016",
    )
}

async fn control_status_placeholder() -> impl IntoResponse {
    not_implemented(
        "Control-policy API wiring lands in the OA-RUST-017 milestone.",
        "OA-RUST-017",
    )
}

async fn sync_token_placeholder() -> impl IntoResponse {
    not_implemented(
        "Sync-token minting API wiring lands in the OA-RUST-018 milestone.",
        "OA-RUST-018",
    )
}

fn not_implemented(
    message: &'static str,
    next_issue: &'static str,
) -> (StatusCode, Json<NotImplementedResponse>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(NotImplementedResponse {
            error: "not_implemented",
            message,
            next_issue,
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Result;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::build_router;
    use crate::config::Config;

    fn test_config(static_dir: PathBuf) -> Config {
        Config {
            bind_addr: "127.0.0.1:0"
                .parse()
                .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
            log_filter: "debug".to_string(),
            static_dir,
        }
    }

    async fn read_json(response: axum::response::Response) -> Result<Value> {
        let bytes = response.into_body().collect().await?.to_bytes();
        let value = serde_json::from_slice::<Value>(&bytes)?;
        Ok(value)
    }

    #[tokio::test]
    async fn healthz_route_returns_ok() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let request = Request::builder().uri("/healthz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "openagents-control-service");
        Ok(())
    }

    #[tokio::test]
    async fn readiness_route_is_not_ready_when_static_dir_missing() -> Result<()> {
        let base = tempdir()?;
        let missing_dir = base.path().join("missing-assets");
        let app = build_router(test_config(missing_dir));

        let request = Request::builder().uri("/readyz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "not_ready");
        Ok(())
    }

    #[tokio::test]
    async fn readiness_route_is_ready_when_static_dir_exists() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder().uri("/readyz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "ready");
        Ok(())
    }

    #[tokio::test]
    async fn placeholder_routes_return_not_implemented() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let auth_request = Request::builder()
            .uri("/api/v1/auth/session")
            .body(Body::empty())?;
        let auth_response = app.clone().oneshot(auth_request).await?;
        assert_eq!(auth_response.status(), StatusCode::NOT_IMPLEMENTED);

        let control_request = Request::builder()
            .uri("/api/v1/control/status")
            .body(Body::empty())?;
        let control_response = app.clone().oneshot(control_request).await?;
        assert_eq!(control_response.status(), StatusCode::NOT_IMPLEMENTED);

        let sync_request = Request::builder()
            .method("POST")
            .uri("/api/v1/sync/token")
            .body(Body::empty())?;
        let sync_response = app.oneshot(sync_request).await?;
        assert_eq!(sync_response.status(), StatusCode::NOT_IMPLEMENTED);

        Ok(())
    }
}
