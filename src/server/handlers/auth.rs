use axum::{
    extract::{Query, State},
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::Duration;
use tracing::{debug, error, info};

use crate::server::services::OIDCConfig;

const SESSION_COOKIE_NAME: &str = "session";
const SESSION_DURATION_DAYS: i64 = 7;

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
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
pub struct AppState {
    pub config: OIDCConfig,
    pub pool: PgPool,
}

impl AppState {
    pub fn new(config: OIDCConfig, pool: PgPool) -> Self {
        Self { config, pool }
    }
}

pub async fn login(State(state): State<AppState>) -> impl IntoResponse {
    let auth_url = state.config.authorization_url();
    debug!("Redirecting to auth URL: {}", auth_url);
    Redirect::temporary(&auth_url)
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    debug!("Received callback with code length: {}", params.code.len());

    // Exchange code for tokens and get/create user
    let user = state
        .config
        .authenticate(params.code, &state.pool)
        .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::Request,
        routing::{get, post},
        Router,
    };
    use std::sync::Once;
    use tower::ServiceExt;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static INIT: Once = Once::new();

    fn setup_logging() {
        INIT.call_once(|| {
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::new(
                    std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into()),
                ))
                .with(tracing_subscriber::fmt::layer())
                .init();
        });
    }

    #[tokio::test]
    async fn test_auth_flow() {
        setup_logging();
        info!("Starting auth flow test");

        // Setup mock OIDC server
        let mock_server = MockServer::start().await;
        info!("Mock server started at: {}", mock_server.uri());

        // Create test config
        let config = OIDCConfig::new(
            "client123".to_string(),
            "secret456".to_string(),
            "http://localhost:3000/callback".to_string(),
            format!("{}/authorize", mock_server.uri()),
            format!("{}/token", mock_server.uri()),
        )
        .unwrap();

        // Setup mock token endpoint
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "test_access_token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "id_token": "header.eyJzdWIiOiJ0ZXN0X3BzZXVkb255bSJ9.signature"
            })))
            .mount(&mock_server)
            .await;

        debug!("Mock token endpoint configured");

        // Create test database connection
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        info!("Database connected");

        // Clean up test data
        sqlx::query!("DELETE FROM users WHERE scramble_id = 'test_pseudonym'")
            .execute(&pool)
            .await
            .unwrap();

        debug!("Test data cleaned up");

        // Create app state
        let state = AppState::new(config, pool);

        // Create test app
        let app = Router::new()
            .route("/login", get(login))
            .route("/callback", get(callback))
            .route("/logout", post(logout))
            .with_state(state);

        debug!("Test app created");

        // Test login endpoint
        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("Login response status: {}", login_response.status());
        assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);

        let location = login_response.headers().get("location").unwrap();
        let location_str = location.to_str().unwrap();
        debug!("Login redirect location: {}", location_str);
        assert!(location_str.starts_with(&format!("{}/authorize", mock_server.uri())));

        // Test callback endpoint
        let callback_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/callback?code=test_code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("Callback response status: {}", callback_response.status());
        assert_eq!(callback_response.status(), StatusCode::TEMPORARY_REDIRECT);

        let cookie = callback_response
            .headers()
            .get("set-cookie")
            .expect("Cookie header missing");
        debug!("Callback set-cookie header: {}", cookie.to_str().unwrap());

        // Test logout endpoint
        let logout_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/logout")
                    .header("Cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("Logout response status: {}", logout_response.status());
        assert_eq!(logout_response.status(), StatusCode::TEMPORARY_REDIRECT);

        let logout_cookie = logout_response
            .headers()
            .get("set-cookie")
            .expect("Logout cookie missing");
        debug!(
            "Logout set-cookie header: {}",
            logout_cookie.to_str().unwrap()
        );
        assert!(logout_cookie.to_str().unwrap().contains("Max-Age=0"));

        info!("Auth flow test completed successfully");
    }

    #[tokio::test]
    async fn test_invalid_callback() {
        setup_logging();
        info!("Starting invalid callback test");

        // Setup mock OIDC server
        let mock_server = MockServer::start().await;

        // Create test config
        let config = OIDCConfig::new(
            "client123".to_string(),
            "secret456".to_string(),
            "http://localhost:3000/callback".to_string(),
            format!("{}/authorize", mock_server.uri()),
            format!("{}/token", mock_server.uri()),
        )
        .unwrap();

        // Setup mock token endpoint to return error
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "Invalid authorization code"
            })))
            .mount(&mock_server)
            .await;

        debug!("Mock token endpoint configured for error response");

        // Create test database connection
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        // Create app state and router
        let state = AppState::new(config, pool);
        let app = Router::new()
            .route("/callback", get(callback))
            .with_state(state);

        // Test callback with invalid code
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/callback?code=invalid_code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("Invalid callback response status: {}", response.status());
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        info!("Invalid callback test completed successfully");
    }

    #[tokio::test]
    async fn test_duplicate_login() {
        setup_logging();
        info!("Starting duplicate login test");

        // Setup mock OIDC server
        let mock_server = MockServer::start().await;

        // Create test config
        let config = OIDCConfig::new(
            "client123".to_string(),
            "secret456".to_string(),
            "http://localhost:3000/callback".to_string(),
            format!("{}/authorize", mock_server.uri()),
            format!("{}/token", mock_server.uri()),
        )
        .unwrap();

        // Setup mock token endpoint
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "test_access_token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "id_token": "header.eyJzdWIiOiJ0ZXN0X3BzZXVkb255bSJ9.signature"
            })))
            .mount(&mock_server)
            .await;

        debug!("Mock token endpoint configured");

        // Create test database connection
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        // Clean up test data
        sqlx::query!("DELETE FROM users WHERE scramble_id = 'test_pseudonym'")
            .execute(&pool)
            .await
            .unwrap();

        debug!("Test data cleaned up");

        // Create app state and router
        let state = AppState::new(config, pool.clone());
        let app = Router::new()
            .route("/callback", get(callback))
            .with_state(state);

        // First login - should create user
        let response1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/callback?code=test_code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("First login response status: {}", response1.status());
        assert_eq!(response1.status(), StatusCode::TEMPORARY_REDIRECT);

        // Second login with same pseudonym - should succeed and update last_login
        let response2 = app
            .oneshot(
                Request::builder()
                    .uri("/callback?code=test_code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        info!("Second login response status: {}", response2.status());
        assert_eq!(response2.status(), StatusCode::TEMPORARY_REDIRECT);

        // Verify only one user exists with updated last_login
        let users = sqlx::query!(
            "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
            "test_pseudonym"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        info!("Found {} users with test pseudonym", users.count.unwrap());
        assert_eq!(users.count, Some(1));

        info!("Duplicate login test completed successfully");
    }
}
