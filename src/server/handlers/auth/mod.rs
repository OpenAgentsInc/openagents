use axum::{
    extract::State,
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
pub mod login;
pub mod session;
pub mod signup;

pub use forms::*;
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