use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info};

use crate::server::{
    config::AppState,
    services::auth::{AuthError, OIDCConfig, OIDCService},
};

pub mod forms;
pub mod github;
pub mod login;
pub mod session;
pub mod signup;

pub use forms::*;
pub use github::*;
pub use login::*;
pub use session::*;
pub use signup::*;

pub const SESSION_COOKIE_NAME: &str = "session";
pub const SESSION_DURATION_DAYS: i64 = 7;

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub flow: Option<String>, // Optional flow parameter to distinguish login vs signup
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub url: String,
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

// Common error handling helper
pub fn handle_auth_error(error: AuthError) -> Response {
    error!("Authentication error: {}", &error);
    (
        StatusCode::from(error.clone()),
        Json(ErrorResponse {
            error: format!("Authentication error: {}", error),
        }),
    )
        .into_response()
}

// Callback handler
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
            create_session_and_redirect(user).await
        }
        Err(e) => match e {
            AuthError::UserAlreadyExists(user) => {
                info!("User already exists, creating session and redirecting");
                create_session_and_redirect(user).await
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