// GitHub OAuth integration

use actix_web::{web, HttpResponse, Result};
use oauth2::{
    AuthorizationCode, AuthUrl, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use tracing::{info, error};

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct OAuthState {
    csrf_token: String,
}

pub async fn start_oauth() -> Result<HttpResponse> {
    // Get GitHub OAuth credentials from environment
    let client_id = std::env::var("GITHUB_CLIENT_ID")
        .expect("GITHUB_CLIENT_ID must be set");
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET")
        .expect("GITHUB_CLIENT_SECRET must be set");
    let redirect_url = std::env::var("GITHUB_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:8080/auth/github/callback".to_string());

    // Create OAuth client
    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
            .expect("Invalid authorization endpoint URL"),
        Some(
            TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
                .expect("Invalid token endpoint URL"),
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url).expect("Invalid redirect URL"),
    );

    // Generate the authorization URL
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("repo".to_string()))
        .add_scope(Scope::new("user:email".to_string()))
        .url();

    info!("Generated OAuth URL, CSRF token: {}", csrf_state.secret());

    // In production, store CSRF token in session/cookie
    // For now, we'll redirect directly
    Ok(HttpResponse::Found()
        .append_header(("Location", authorize_url.to_string()))
        .finish())
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    code: String,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    pub email: Option<String>,
    pub name: Option<String>,
}

pub async fn oauth_callback(
    query: web::Query<OAuthCallback>,
) -> Result<HttpResponse> {
    info!("Received OAuth callback with code");

    let client_id = std::env::var("GITHUB_CLIENT_ID")
        .expect("GITHUB_CLIENT_ID must be set");
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET")
        .expect("GITHUB_CLIENT_SECRET must be set");
    let redirect_url = std::env::var("GITHUB_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:8080/auth/github/callback".to_string());

    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
            .expect("Invalid authorization endpoint URL"),
        Some(
            TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
                .expect("Invalid token endpoint URL"),
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url).expect("Invalid redirect URL"),
    );

    // Exchange code for token
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await;

    match token_result {
        Ok(token) => {
            let access_token = token.access_token().secret();
            info!("Successfully exchanged code for access token");

            // Fetch user info from GitHub
            let http_client = reqwest::Client::new();
            let user_result = http_client
                .get("https://api.github.com/user")
                .header("User-Agent", "OpenAgents")
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await;

            match user_result {
                Ok(response) => {
                    if response.status().is_success() {
                        let user: GitHubUser = response.json().await
                            .map_err(|e| {
                                error!("Failed to parse GitHub user response: {}", e);
                                actix_web::error::ErrorInternalServerError(e)
                            })?;

                        info!("Authenticated GitHub user: {}", user.login);

                        // In production:
                        // 1. Store user in database
                        // 2. Create session
                        // 3. Set session cookie
                        // 4. Redirect to dashboard

                        Ok(HttpResponse::Ok().json(serde_json::json!({
                            "success": true,
                            "user": user,
                            "message": "Authentication successful! In production, you'd be redirected to dashboard."
                        })))
                    } else {
                        error!("GitHub API returned error: {}", response.status());
                        Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": "Failed to fetch user info from GitHub"
                        })))
                    }
                }
                Err(e) => {
                    error!("Failed to call GitHub API: {}", e);
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to communicate with GitHub"
                    })))
                }
            }
        }
        Err(e) => {
            error!("Failed to exchange code for token: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "OAuth authentication failed"
            })))
        }
    }
}
