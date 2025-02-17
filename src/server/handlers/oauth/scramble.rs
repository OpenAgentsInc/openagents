use crate::server::{config::AppState, handlers::oauth::session::create_session_and_redirect};
use axum::response::Redirect;
use axum::{
    extract::{Form, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    email: String,
    #[serde(rename = "password")]
    password: Option<String>,
    #[serde(rename = "password-confirm")]
    password_confirm: Option<String>,
    #[serde(rename = "terms")]
    terms: Option<bool>,
}

pub async fn scramble_login(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    form: Option<Form<LoginParams>>,
) -> Response {
    // Try to get email from either query params or form data
    let form_data = match (params, form) {
        (Some(query), _) => query.0,
        (_, Some(form)) => form.0,
        _ => return axum::response::Redirect::temporary("/login").into_response(),
    };

    info!(
        "Handling Scramble login request for email: {}",
        form_data.email
    );

    let (url, csrf_token, pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_login(&form_data.email);

    info!("Generated login URL with state: {}", csrf_token.secret());

    axum::response::Redirect::temporary(&url).into_response()
}

pub async fn scramble_signup(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    form: Option<Form<LoginParams>>,
) -> Response {
    info!(
        "Received signup request with params: {:?}, form: {:?}",
        params, form
    );

    // Try to get email from either query params or form data
    let form_data = match (params, form) {
        (Some(query), _) => query.0,
        (_, Some(form)) => form.0,
        _ => {
            info!("No email provided in signup request, redirecting to /signup");
            return axum::response::Redirect::temporary("/signup").into_response();
        }
    };

    info!("Form data: {:?}", form_data);

    // Validate form data
    if let Some(password) = &form_data.password {
        if password.len() < 8 {
            return axum::response::Redirect::temporary(
                "/signup?error=Password+must+be+at+least+8+characters",
            )
            .into_response();
        }

        if let Some(confirm) = &form_data.password_confirm {
            if password != confirm {
                return axum::response::Redirect::temporary("/signup?error=Passwords+do+not+match")
                    .into_response();
            }
        }
    }

    if form_data.terms != Some(true) {
        return axum::response::Redirect::temporary("/signup?error=You+must+accept+the+terms")
            .into_response();
    }

    info!(
        "Handling Scramble signup request for email: {}",
        form_data.email
    );

    // Generate the authorization URL
    let (url, csrf_token, pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_signup(&form_data.email);

    info!(
        "Generated signup authorization URL: {} with state: {}",
        url,
        csrf_token.secret()
    );

    // Try using a different redirect approach
    Redirect::to(&url).into_response()
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
        return axum::response::Redirect::temporary(&format!("/login?error={}", error))
            .into_response();
    }

    let state_param = params.state.clone().unwrap_or_default();
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
            create_session_and_redirect(&user, false).await
        }
        Err(error) => {
            error!("Authentication failed: {}", error);
            axum::response::Redirect::temporary(&format!(
                "/{}?error={}",
                if is_signup { "signup" } else { "login" },
                error
            ))
            .into_response()
        }
    }
}