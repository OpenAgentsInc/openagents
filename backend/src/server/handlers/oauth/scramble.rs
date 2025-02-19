use crate::server::{config::AppState, handlers::oauth::session::create_session_and_redirect};
use axum::{
    extract::{Form, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    email: String,
    #[serde(rename = "password")]
    password: Option<String>,
    #[serde(rename = "password-confirm")]
    password_confirm: Option<String>,
}

pub async fn scramble_login(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    Form(form_data): Form<LoginParams>,
) -> Response {
    let form_data = match params {
        Some(query) => query.0,
        None => form_data,
    };

    // Require password for login
    if form_data.password.is_none() {
        return axum::response::Json(json!({
            "error": "Password is required"
        }))
        .into_response();
    }

    info!(
        "Handling Scramble login request for email: {}",
        form_data.email
    );

    let (url, csrf_token, _pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_login(&form_data.email);

    info!("Generated login URL with state: {}", csrf_token.secret());

    // Return JSON response with URL instead of redirecting
    axum::response::Json(json!({
        "url": url
    }))
    .into_response()
}

pub async fn scramble_signup(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    Form(form_data): Form<LoginParams>,
) -> Response {
    info!(
        "Received signup request with params: {:?}, form: {:?}",
        params, form_data
    );

    let form_data = match params {
        Some(query) => query.0,
        None => form_data,
    };

    info!("Form data: {:?}", form_data);

    // Require password for signup
    if form_data.password.is_none() {
        info!("Password required but not provided");
        return axum::response::Json(json!({
            "error": "Password is required"
        }))
        .into_response();
    }

    // Validate password
    if let Some(password) = &form_data.password {
        if password.len() < 8 {
            info!("Password too short");
            return axum::response::Json(json!({
                "error": "Password must be at least 8 characters"
            }))
            .into_response();
        }

        if let Some(confirm) = &form_data.password_confirm {
            if password != confirm {
                info!("Passwords do not match");
                return axum::response::Json(json!({
                    "error": "Passwords do not match"
                }))
                .into_response();
            }
        }
    }

    info!(
        "Handling Scramble signup request for email: {}",
        form_data.email
    );

    let (url, csrf_token, _pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_signup(&form_data.email);

    info!(
        "Generated signup authorization URL: {} with state: {}",
        url,
        csrf_token.secret()
    );

    // Return JSON response with URL instead of redirecting
    let response = axum::response::Json(json!({
        "url": url
    }))
    .into_response();

    info!("Sending response with URL: {}", url);
    response
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
    info!("Processing Scramble callback with params: {:?}", params);

    if let Some(error) = params.error {
        info!("Scramble OAuth error: {}", error);
        return axum::response::Redirect::temporary(&format!(
            "{}/login?error={}",
            state.frontend_url, error
        ))
        .into_response();
    }

    let state_param = params.state.clone().unwrap_or_default();
    // Check if this is a signup flow by looking at the state suffix
    let is_signup = state_param.ends_with("_signup");

    info!(
        "Callback is for {} with state: {}",
        if is_signup { "signup" } else { "login" },
        state_param
    );

    match state
        .scramble_oauth
        .authenticate(params.code.clone(), state_param, is_signup)
        .await
    {
        Ok(user) => {
            info!("Successfully authenticated Scramble user: {:?}", user);
            create_session_and_redirect(&state, &user, false).await
        }
        Err(error) => {
            error!("Authentication failed: {}", error);
            axum::response::Redirect::temporary(&format!(
                "{}/{}?error={}",
                state.frontend_url,
                if is_signup { "signup" } else { "login" },
                error
            ))
            .into_response()
        }
    }
}
