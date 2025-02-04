use axum::{
    extract::{Form, Query, State},
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use time::Duration;
use tracing::{error, info};

use crate::server::{
    config::AppState,
    services::auth::{AuthError, OIDCConfig, OIDCService},
};

const SESSION_COOKIE_NAME: &str = "session";
const SESSION_DURATION_DAYS: i64 = 7;

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    flow: Option<String>, // Optional flow parameter to distinguish login vs signup
}

// Custom deserializer for HTML checkbox
fn deserialize_checkbox<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // HTML checkboxes only send a value when checked
    // If the field is missing, it means unchecked
    Option::<String>::deserialize(deserializer).map(|x| x.is_some())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SignupForm {
    email: String,
    password: String,
    #[serde(rename = "password-confirm")]
    password_confirmation: String,
    #[serde(rename = "terms", deserialize_with = "deserialize_checkbox", default)]
    terms_accepted: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    url: String,
}

#[derive(Clone)]
pub struct AuthState {
    pub service: Arc<OIDCService>,
}

impl AuthState {
    pub fn new(config: OIDCConfig, pool: PgPool) -> Self {
        info!("Creating new AuthState");
        Self {
            service: Arc::new(OIDCService::new(pool, config)),
        }
    }
}

pub async fn login(State(state): State<AppState>) -> Response {
    info!("Handling login request");
    match state.auth_state.service.authorization_url_for_login() {
        Ok(auth_url) => {
            info!("Generated login auth URL: {}", auth_url);
            Redirect::temporary(&auth_url).into_response()
        }
        Err(e) => {
            error!("Failed to generate login auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to generate authorization URL: {}", e),
                }),
            )
                .into_response()
        }
    }
}

pub async fn signup(State(state): State<AppState>) -> Response {
    info!("Handling signup request");
    match state.auth_state.service.authorization_url_for_signup("") {
        Ok(auth_url) => {
            info!("Generated signup auth URL: {}", auth_url);
            Redirect::temporary(&auth_url).into_response()
        }
        Err(e) => {
            error!("Failed to generate signup auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to generate authorization URL: {}", e),
                }),
            )
                .into_response()
        }
    }
}

pub async fn handle_signup(
    State(state): State<AppState>,
    Form(form): Form<SignupForm>,
) -> Response {
    info!("Received signup form: {:?}", form);

    // Basic validation
    if !form.terms_accepted {
        error!("Terms not accepted");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Terms must be accepted".to_string(),
            }),
        )
            .into_response();
    }
    if form.password != form.password_confirmation {
        error!("Passwords do not match");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Passwords do not match".to_string(),
            }),
        )
            .into_response();
    }

    // Generate signup URL with prompt=create and email
    match state
        .auth_state
        .service
        .authorization_url_for_signup(&form.email)
    {
        Ok(auth_url) => {
            info!("Redirecting to signup URL: {}", auth_url);
            Redirect::temporary(&auth_url).into_response()
        }
        Err(e) => {
            error!("Failed to generate auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Internal server error: {}", e),
                }),
            )
                .into_response()
        }
    }
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Response {
    info!("Received callback with code length: {}", params.code.len());
    info!("Flow parameter: {:?}", params.flow);

    // Determine if this is a signup flow
    let is_signup = params.flow.as_deref() == Some("signup");
    info!(
        "Callback flow: {}",
        if is_signup { "signup" } else { "login" }
    );

    // Use appropriate service method based on flow
    let result = if is_signup {
        info!("Processing as signup flow");
        state.auth_state.service.signup(params.code).await
    } else {
        info!("Processing as login flow");
        state.auth_state.service.login(params.code).await
    };

    match result {
        Ok(user) => {
            info!("Successfully processed user: {:?}", user);
            create_session_and_redirect(user)
        }
        Err(e) => match e {
            AuthError::UserAlreadyExists(user) => {
                info!("User already exists, creating session and redirecting");
                create_session_and_redirect(user)
            }
            other_error => {
                error!("Authentication error: {}", &other_error);
                (
                    StatusCode::from(other_error.clone()),
                    Json(ErrorResponse {
                        error: format!("Authentication error: {}", other_error),
                    }),
                )
                    .into_response()
            }
        },
    }
}

fn create_session_and_redirect(user: crate::server::models::user::User) -> Response {
    let cookie = Cookie::build((SESSION_COOKIE_NAME, user.scramble_id))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(SESSION_DURATION_DAYS))
        .build();

    info!("Created session cookie: {}", cookie.to_string());

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("Redirecting to home with session cookie");
    (headers, Redirect::temporary("/")).into_response()
}

pub async fn logout() -> Response {
    info!("Processing logout request");

    // Create cookie that will expire immediately
    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("Created logout cookie: {}", cookie.to_string());
    info!("Redirecting to home after logout");

    (headers, Redirect::temporary("/")).into_response()
}