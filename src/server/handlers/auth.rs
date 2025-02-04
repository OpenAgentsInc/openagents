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
    services::auth::{OIDCConfig, OIDCService},
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
                    error: "Failed to generate authorization URL".to_string(),
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
                    error: "Failed to generate authorization URL".to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn handle_signup(
    State(state): State<AppState>,
    Form(form): Form<SignupForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    info!("Received signup form: {:?}", form);

    // Basic validation
    if !form.terms_accepted {
        error!("Terms not accepted");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Terms must be accepted".to_string(),
            }),
        ));
    }
    if form.password != form.password_confirmation {
        error!("Passwords do not match");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Passwords do not match".to_string(),
            }),
        ));
    }

    // Generate signup URL with prompt=create and email
    let auth_url = state
        .auth_state
        .service
        .authorization_url_for_signup(&form.email)
        .map_err(|e| {
            error!("Failed to generate auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                }),
            )
        })?;

    info!("Redirecting to signup URL: {}", auth_url);
    Ok(Redirect::temporary(&auth_url))
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    let user = match result {
        Ok(user) => {
            info!("Successfully processed user: {:?}", user);
            user
        }
        Err(e) => {
            error!("Authentication error: {}", e);
            return Err((
                StatusCode::from(e.clone()),
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    info!("User authenticated with scramble_id: {}", user.scramble_id);

    // Create session cookie
    let cookie = Cookie::build((SESSION_COOKIE_NAME, user.scramble_id.clone()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(SESSION_DURATION_DAYS))
        .build();

    info!("Created session cookie: {}", cookie.to_string());

    // Set cookie and redirect to home
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("Redirecting to home with session cookie");
    Ok((headers, Redirect::temporary("/")))
}

pub async fn logout() -> impl IntoResponse {
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

    (headers, Redirect::temporary("/"))
}