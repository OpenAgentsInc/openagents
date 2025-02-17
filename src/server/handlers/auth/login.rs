use crate::server::config::AppState;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Extension,
};
use serde::Deserialize;
use tracing::info;
use askama_axum::IntoResponse as AskamaIntoResponse;

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    title: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
}

pub async fn login_page() -> impl IntoResponse {
    let template = LoginTemplate {
        title: "Login - OpenAgents".to_string(),
    };
    template.into_response()
}

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    error: Option<String>,
}

pub async fn handle_login(
    State(state): State<AppState>,
    Query(request): Query<LoginRequest>,
) -> impl IntoResponse {
    info!("Handling login request for email: {}", request.email);

    // Generate Scramble login URL
    let (url, _csrf_token) = state
        .oauth_state
        .scramble
        .authorization_url_for_login(&request.email);

    Redirect::temporary(&url)
}
