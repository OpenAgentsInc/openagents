use crate::server::services::repomap::RepomapService;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde_json::Value;
use std::sync::Arc;

pub async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn home() -> Html<&'static str> {
    Html(include_str!("../templates/pages/home.html"))
}

pub async fn login() -> Html<&'static str> {
    Html(include_str!("../templates/pages/login.html"))
}

pub async fn signup() -> Html<&'static str> {
    Html(include_str!("../templates/pages/signup.html"))
}

pub async fn chat() -> Html<&'static str> {
    Html(include_str!("../templates/pages/chat.html"))
}

pub async fn mobile_app() -> Html<&'static str> {
    Html(include_str!("../templates/pages/coming-soon.html"))
}

pub async fn business() -> Html<&'static str> {
    Html(include_str!("../templates/pages/services.html"))
}

pub async fn video_series() -> Html<&'static str> {
    Html(include_str!("../templates/pages/video-series.html"))
}

pub async fn company() -> Html<&'static str> {
    Html(include_str!("../templates/pages/company.html"))
}

pub async fn coming_soon() -> Html<&'static str> {
    Html(include_str!("../templates/pages/coming-soon.html"))
}

pub async fn repomap() -> Html<&'static str> {
    Html(include_str!("../templates/pages/repomap.html"))
}

#[axum::debug_handler]
pub async fn generate_repomap(
    State(repomap_service): State<Arc<RepomapService>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let repo_url = payload["repo_url"]
        .as_str()
        .ok_or(StatusCode::BAD_REQUEST)?;

    repomap_service
        .generate_repomap(repo_url)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}