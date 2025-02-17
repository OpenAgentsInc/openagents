use crate::server::{config::AppState, handlers::oauth::session::create_session_and_redirect};
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

    let (url, _csrf_token, _pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_login(&form_data.email);

    axum::response::Redirect::temporary(&url).into_response()
}

pub async fn scramble_signup(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
    form: Option<Form<LoginParams>>,
) -> Response {
    // Try to get email from either query params or form data
    let form_data = match (params, form) {
        (Some(query), _) => query.0,
        (_, Some(form)) => form.0,
        _ => {
            info!("No email provided in signup request, redirecting to /signup");
            return axum::response::Redirect::temporary("/signup").into_response();
        }
    };

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
    let (url, csrf_token, _pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_signup(&form_data.email);

    info!(
        "Generated signup authorization URL: {} with state: {}",
        url,
        csrf_token.secret()
    );

    // Add signup-specific state to help identify this flow in the callback
    let final_url = format!("{}&signup=true", url);
    info!("Final signup URL with state: {}", final_url);

    // Redirect to the Scramble auth URL
    axum::response::Redirect::temporary(&final_url).into_response()
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

    let is_signup = params
        .state
        .as_deref()
        .map(|s| s.contains("signup"))
        .unwrap_or(false);
    info!(
        "Callback is for {}",
        if is_signup { "signup" } else { "login" }
    );

    let state_param = params.state.clone().unwrap_or_default();
    info!("Using state token: {}", state_param);

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