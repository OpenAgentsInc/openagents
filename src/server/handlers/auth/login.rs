use crate::server::config::AppState;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    title: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
}

pub async fn login_page(State(_state): State<AppState>) -> Response {
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
) -> Response {
    info!("Handling login request for email: {}", request.email);

    // Generate Scramble login URL
    let (url, _csrf_token, _pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_login(&request.email);

    axum::response::Redirect::temporary(&url).into_response()
}
