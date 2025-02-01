use axum::{
    extract::{Form, Query, State},
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use time::Duration;
use tracing::{debug, error, info};

use crate::server::services::auth::{OIDCConfig, OIDCService};

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
    #[serde(rename = "terms", deserialize_with = "deserialize_checkbox")]
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

pub async fn login(State(state): State<AuthState>) -> impl IntoResponse {
    let auth_url = state.service.authorization_url_for_login().unwrap();
    debug!("Redirecting to auth URL: {}", auth_url);
    Redirect::temporary(&auth_url)
}

pub async fn signup(State(state): State<AuthState>) -> impl IntoResponse {
    let auth_url = state.service.authorization_url_for_signup().unwrap();
    debug!("Redirecting to signup URL: {}", auth_url);
    Redirect::temporary(&auth_url)
}

// New handler for signup form submission
pub async fn handle_signup(
    State(state): State<AuthState>,
    Form(form): Form<SignupForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // Basic validation
    if !form.terms_accepted {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Terms must be accepted".to_string(),
            }),
        ));
    }
    if form.password != form.password_confirmation {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Passwords do not match".to_string(),
            }),
        ));
    }

    // Generate signup URL with prompt=create
    let auth_url = state
        .service
        .authorization_url_for_signup()
        .map_err(|e| {
            error!("Failed to generate auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                }),
            )
        })?;

    debug!("Redirecting to signup URL: {}", auth_url);
    Ok(Redirect::temporary(&auth_url))
}

pub async fn callback(
    State(state): State<AuthState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    debug!("Received callback with code length: {}", params.code.len());

    // Determine if this is a signup flow
    let is_signup = params.flow.as_deref() == Some("signup");
    debug!(
        "Callback flow: {}",
        if is_signup { "signup" } else { "login" }
    );

    // Use appropriate service method based on flow
    let user = if is_signup {
        state.service.signup(params.code).await
    } else {
        state.service.login(params.code).await
    }
    .map_err(|e| {
        error!("Authentication error: {}", e.to_string());
        (
            StatusCode::from(e.clone()),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    info!("User authenticated with scramble_id: {}", user.scramble_id);

    // Create session cookie
    let cookie = Cookie::build((SESSION_COOKIE_NAME, user.scramble_id.clone()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(SESSION_DURATION_DAYS))
        .build();

    debug!("Created session cookie: {}", cookie.to_string());

    // Set cookie and redirect to home
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok((headers, Redirect::temporary("/")))
}

pub async fn logout() -> impl IntoResponse {
    debug!("Processing logout request");

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

    debug!("Created logout cookie: {}", cookie.to_string());

    (headers, Redirect::temporary("/"))
}