use crate::server::AppState;
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::info;

#[derive(Template)]
#[template(path = "pages/signup.html")]
struct SignupTemplate {
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    email: String,
}

pub async fn signup_page(Query(params): Query<SignupParams>) -> impl IntoResponse {
    let template = SignupTemplate {
        error: params.error,
    };
    template.into_response()
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
    let (url, _csrf_token) = state
        .oauth_state
        .scramble
        .authorization_url_for_signup(&request.email);

    Redirect::temporary(&url)
}