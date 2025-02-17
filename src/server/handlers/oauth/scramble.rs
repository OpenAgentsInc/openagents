use crate::server::{config::AppState, handlers::oauth::session::create_session_and_redirect};
use axum::{
    extract::{Form, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    email: String,
}

pub async fn scramble_login(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    form: Option<Form<LoginParams>>,
) -> Response {
    // Try to get email from either query params or form data
    let email = match (params, form) {
        (Some(query), _) => query.email.clone(),
        (_, Some(form)) => form.email.clone(),
        _ => return axum::response::Redirect::temporary("/login").into_response(),
    };

    info!("Handling Scramble login request for email: {}", email);

    let (url, _csrf_token, _pkce_verifier) =
        state.scramble_oauth.authorization_url_for_login(&email);

    axum::response::Redirect::temporary(&url).into_response()
}

pub async fn scramble_signup(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    form: Option<Form<LoginParams>>,
) -> Response {
    // Try to get email from either query params or form data
    let email = match (params, form) {
        (Some(query), _) => query.email.clone(),
        (_, Some(form)) => form.email.clone(),
        _ => return axum::response::Redirect::temporary("/signup").into_response(),
    };

    info!("Handling Scramble signup request for email: {}", email);

    let (url, _csrf_token, _pkce_verifier) =
        state.scramble_oauth.authorization_url_for_signup(&email);

    axum::response::Redirect::temporary(&url).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

pub async fn scramble_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Response {
    info!("Processing Scramble callback");

    if let Some(error) = params.error {
        info!("Scramble OAuth error: {}", error);
        return axum::response::Redirect::temporary(&format!("/login?error={}", error))
            .into_response();
    }

    let is_signup = params
        .state
        .as_deref()
        .map(|s| s.contains("signup"))
        .unwrap_or(false);

    let state_param = params.state.unwrap_or_default();

    match state
        .scramble_oauth
        .authenticate(params.code, state_param, is_signup)
        .await
    {
        Ok(user) => {
            info!("Successfully authenticated Scramble user: {:?}", user);
            create_session_and_redirect(&user, false).await
        }
        Err(error) => {
            info!("Authentication failed: {}", error);
            axum::response::Redirect::temporary(&format!(
                "/{}?error={}",
                if is_signup { "signup" } else { "login" },
                error
            ))
            .into_response()
        }
    }
}
