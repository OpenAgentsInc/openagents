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
    State(model_router): State<Arc<crate::server::services::model_router::ModelRouter>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let repo_url = payload["repo_url"]
        .as_str()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let prompt = format!(
        "Generate a repository map for {}. Include all files and their key functions.",
        repo_url
    );

    let (response, _) = model_router
        .chat(prompt)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "result": response
    })))
}