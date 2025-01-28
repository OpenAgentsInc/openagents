use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use openagents::server::{
    handlers::{callback, login, logout, signup, AppState},
    services::OIDCConfig,
};

async fn setup_test_db() -> PgPool {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    // Clean up any existing test data
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

fn create_test_token(sub: &str) -> String {
    // Create a simple JWT token for testing
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(&format!(r#"{{"sub":"{}","iat":1516239022}}"#, sub));
    let signature = URL_SAFE_NO_PAD.encode("test_signature");
    format!("{}.{}.{}", header, claims, signature)
}

#[tokio::test]
async fn test_full_auth_flow() {
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
    let test_token = create_test_token("test_pseudonym");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": test_token
        })))
        .mount(&mock_server)
        .await;

    // Setup test database
    let pool = setup_test_db().await;

    // Create app state and router
    let state = AppState::new(config.clone(), pool.clone());
    let app = Router::new()
        .route("/login", get(login))
        .route("/signup", get(signup))
        .route("/callback", get(callback))
        .route("/logout", post(logout))
        .with_state(state);

    // Test login endpoint - should redirect to auth server
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

    assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = login_response.headers().get("location").unwrap();
    assert!(location
        .to_str()
        .unwrap()
        .starts_with(&format!("{}/authorize", mock_server.uri())));

    // Test callback endpoint - should create user and set session
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

    assert_eq!(callback_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert!(callback_response.headers().contains_key("set-cookie"));

    // Verify user was created
    let user = sqlx::query!(
        "SELECT * FROM users WHERE scramble_id = $1",
        "test_pseudonym"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(user.scramble_id, "test_pseudonym");

    // Extract session cookie
    let cookie = callback_response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Test logout endpoint - should clear session
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

    assert_eq!(logout_response.status(), StatusCode::TEMPORARY_REDIRECT);

    let logout_cookie = logout_response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(logout_cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn test_invalid_callback() {
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
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "Invalid authorization code"
        })))
        .mount(&mock_server)
        .await;

    // Setup test database
    let pool = setup_test_db().await;

    // Create app state and router
    let state = AppState::new(config.clone(), pool.clone());
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

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_duplicate_login() {
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
    let test_token = create_test_token("test_pseudonym");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": test_token
        })))
        .expect(2)
        .mount(&mock_server)
        .await;

    // Setup test database
    let pool = setup_test_db().await;

    // Create app state and router
    let state = AppState::new(config.clone(), pool.clone());
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

    assert_eq!(response2.status(), StatusCode::TEMPORARY_REDIRECT);

    // Verify only one user exists with updated last_login
    let users = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        "test_pseudonym"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(users.count, Some(1));
}