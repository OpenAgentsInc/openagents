use crate::server::config::AppState;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use askama_axum::IntoResponse as AskamaIntoResponse;
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "auth/signup.html")]
struct SignupTemplate {
    title: String,
}

pub async fn signup_page(
    State(_state): State<AppState>,
) -> impl AskamaIntoResponse {
    let template = SignupTemplate {
        title: "Sign Up - OpenAgents".to_string(),
    };
    template
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupParams {
    error: Option<String>,
}

pub async fn handle_signup(
    State(state): State<AppState>,
    Query(request): Query<SignupRequest>,
) -> impl IntoResponse {
    info!("Handling signup request for email: {}", request.email);

    // Generate Scramble signup URL
    let (url, _csrf_token, _pkce_verifier) = state
        .oauth_state
        .scramble
        .authorization_url_for_signup(&request.email);

    Redirect::temporary(&url)
}
