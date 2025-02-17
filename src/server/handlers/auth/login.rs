use crate::server::config::AppState;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
}

pub async fn login_page(Query(params): Query<LoginParams>) -> impl IntoResponse {
    let template = LoginTemplate {
        error: params.error,
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
