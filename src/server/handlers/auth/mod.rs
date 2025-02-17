use crate::server::config::AppState;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Extension,
};
use serde::Deserialize;
use tracing::info;

pub mod login;
pub mod session;
pub mod signup;

pub use login::{handle_login, login_page};
pub use signup::{handle_signup, signup_page};

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    info!("Processing OAuth callback");

    if let Some(error) = params.error {
        info!("OAuth error: {}", error);
        return Redirect::temporary(&format!("/login?error={}", error));
    }

    let is_signup = params.state.as_deref().map(|s| s.contains("signup")).unwrap_or(false);
    let is_mobile = params.state.as_deref().map(|s| s.contains("mobile")).unwrap_or(false);

    // Authenticate with Scramble
    let result = state
        .oauth_state
        .scramble
        .authenticate(params.code, is_signup)
        .await;

    match result {
        Ok(user) => {
            info!("Successfully authenticated user: {:?}", user);
            session::create_session_and_redirect(&user, is_mobile).await
        }
        Err(e) => {
            info!("Authentication failed: {}", e);
            Redirect::temporary(&format!(
                "/{}{}",
                if is_signup { "signup" } else { "login" },
                format!("?error={}", urlencoding::encode(&e.to_string()))
            ))
        }
    }
}

pub async fn clear_session_and_redirect() -> impl IntoResponse {
    Redirect::to("/")
}

pub async fn create_session_and_redirect(
    user: &User,
    is_mobile: Option<bool>,
) -> Result<impl IntoResponse, OAuthError> {
    let session = session::create_session(user).await?;
    let response = if is_mobile.unwrap_or(false) {
        Redirect::to(&format!("/mobile/auth?session={}", session.id))
    } else {
        Redirect::to("/")
    };
    Ok(response)
}
