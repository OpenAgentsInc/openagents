use crate::server::services::{
    solver::ws::{ws_handler, SolverWsState},
    solver::SolverService,
};
use crate::{configuration, database};
use axum::{
    extract::Form,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub async fn admin_stats() -> impl IntoResponse {
    let config = match configuration::get_configuration() {
        Ok(config) => config,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                json!({
                    "error": format!("Config error: {}", e)
                })
                .to_string(),
            )
                .into_response()
        }
    };

    let pool = match database::get_connection_pool(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                json!({
                    "error": format!("Database error: {}", e)
                })
                .to_string(),
            )
                .into_response()
        }
    };

    // Get total events count
    let total_events: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
    {
        Ok(count) => count,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                json!({
                    "error": format!("Failed to get event count: {}", e)
                })
                .to_string(),
            )
                .into_response()
        }
    };

    // Get events by kind
    let kinds: Vec<(i32, i64)> = match sqlx::query_as(
        "SELECT kind, COUNT(*) as count 
         FROM events 
         GROUP BY kind 
         ORDER BY kind",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(kinds) => kinds,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                json!({
                    "error": format!("Failed to get event kinds: {}", e)
                })
                .to_string(),
            )
                .into_response()
        }
    };

    let events_by_kind: serde_json::Map<String, serde_json::Value> = kinds
        .into_iter()
        .map(|(kind, count)| (kind.to_string(), json!(count)))
        .collect();

    // Get database size
    let db_size: i64 = match sqlx::query_scalar("SELECT pg_database_size(current_database())")
        .fetch_one(&pool)
        .await
    {
        Ok(size) => size,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                json!({
                    "error": format!("Failed to get database size: {}", e)
                })
                .to_string(),
            )
                .into_response()
        }
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "total_events": total_events,
            "events_by_kind": events_by_kind,
            "storage_usage": format!("{:.1} MB", db_size as f64 / (1024.0 * 1024.0)),
            "index_usage": [],
            "status": "ok"
        })
        .to_string(),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    password: String,
}

pub async fn admin_dashboard() -> impl IntoResponse {
    Html(include_str!("../../../templates/admin/dashboard.html"))
}

pub async fn admin_login() -> impl IntoResponse {
    Html(include_str!("../../../templates/admin/login.html"))
}

pub async fn admin_login_post(Form(form): Form<LoginForm>) -> impl IntoResponse {
    let config = match configuration::get_configuration() {
        Ok(config) => config,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new()).into_response();
        }
    };

    let mut headers = HeaderMap::new();
    if form.password == config.application.admin_token {
        headers.insert(
            header::SET_COOKIE,
            format!(
                "admin_session={}; Path=/admin; Secure; HttpOnly",
                config.application.admin_token
            )
            .parse()
            .unwrap(),
        );
        headers.insert(header::LOCATION, "/admin".parse().unwrap());
        (StatusCode::FOUND, headers).into_response()
    } else {
        headers.insert(header::LOCATION, "/admin/login?error=1".parse().unwrap());
        (StatusCode::FOUND, headers).into_response()
    }
}

pub fn admin_routes() -> axum::Router {
    // Create shared solver service and WebSocket state
    let solver_service = Arc::new(SolverService::new());
    let ws_state = Arc::new(SolverWsState::new(solver_service.clone()));

    axum::Router::new()
        .route("/", axum::routing::get(admin_dashboard))
        .route("/login", axum::routing::get(admin_login))
        .route("/login", axum::routing::post(admin_login_post))
        .route("/stats", axum::routing::get(admin_stats))
        .route("/ws", axum::routing::get(ws_handler))
        .with_state(ws_state)
}
