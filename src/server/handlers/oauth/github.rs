use super::{create_session_and_redirect, handle_oauth_error, OAuthCallback, OAuthState};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::PkceCodeVerifier;
use tracing::info;

pub async fn github_login(
    State(state): State<OAuthState>,
    Query(params): Query<GitHubLoginParams>,
) -> impl IntoResponse {
    info!("Starting GitHub login flow");
    
    // Generate authorization URL with PKCE
    let (auth_url, _csrf_token, pkce_verifier) = state.github.authorization_url(params.platform);
    
    // Store PKCE verifier in session or secure cookie
    // TODO: Implement secure storage of PKCE verifier
    
    Redirect::temporary(&auth_url)
}

pub async fn github_callback(
    State(state): State<OAuthState>,
    Query(params): Query<OAuthCallback>,
) -> impl IntoResponse {
    info!("Handling GitHub callback");

    match params.error {
        Some(error) => {
            info!("GitHub auth error: {}", error);
            Redirect::temporary("/auth/error")
        }
        None => {
            let code = params.code;
            let platform = params.state;

            // Get stored PKCE verifier
            // TODO: Implement secure retrieval of PKCE verifier
            let pkce_verifier = PkceCodeVerifier::new("temporary_verifier".to_string()); // This is just a placeholder

            // Authenticate with GitHub
            match state.github.authenticate(code, pkce_verifier).await {
                Ok(user) => {
                    match create_session_and_redirect(&user, platform).await {
                        Ok(response) => response,
                        Err(error) => handle_oauth_error(error),
                    }
                }
                Err(error) => handle_oauth_error(error),
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct GitHubLoginParams {
    platform: Option<String>,
}