use axum::{
    body::Body,
    http::Request,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tower::ServiceExt;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

use crate::server::{
    handlers::{auth::AppState, auth::{login, signup, callback}},
    services::OIDCConfig,
};

#[tokio::test]
async fn test_signup_flow() {
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
            "id_token": "header.eyJzdWIiOiJuZXdfdXNlcl9wc2V1ZG9ueW0ifQ.signature"
        })))
        .mount(&mock_server)
        .await;

    // Create test database connection
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    // Clean up test data
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", "new_user_pseudonym")
        .execute(&pool)
        .await
        .unwrap();

    // Create app state and router
    let state = AppState::new(config, pool.clone());
    let app = Router::new()
        .route("/signup", get(signup))
        .route("/callback", get(callback))
        .with_state(state);

    // Test signup endpoint
    let signup_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/signup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(signup_response.status(), 307); // Temporary redirect
    let location = signup_response.headers().get("location").unwrap();
    let location_str = location.to_str().unwrap();
    assert!(location_str.contains("prompt=create")); // Verify signup parameter

    // Test callback with signup code
    let callback_response = app
        .oneshot(
            Request::builder()
                .uri("/callback?code=test_signup_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(callback_response.status(), 307); // Temporary redirect
    assert!(callback_response.headers().contains_key("set-cookie")); // Verify session cookie

    // Verify user was created
    let user = sqlx::query!("SELECT * FROM users WHERE scramble_id = $1", "new_user_pseudonym")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(user.scramble_id, "new_user_pseudonym");

    // Cleanup
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", "new_user_pseudonym")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_signup_existing_user() {
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
            "id_token": "header.eyJzdWIiOiJleGlzdGluZ191c2VyIn0.signature"
        })))
        .mount(&mock_server)
        .await;

    // Create test database connection
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    // Create existing user
    sqlx::query!(
        "INSERT INTO users (scramble_id, metadata) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        "existing_user",
        serde_json::json!({})
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create app state and router
    let state = AppState::new(config, pool.clone());
    let app = Router::new()
        .route("/signup", get(signup))
        .route("/callback", get(callback))
        .with_state(state);

    // Test signup with existing user
    let response = app
        .oneshot(
            Request::builder()
                .uri("/callback?code=test_signup_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 409); // Conflict status code

    // Cleanup
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", "existing_user")
        .execute(&pool)
        .await
        .unwrap();
}