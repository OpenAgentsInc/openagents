use base64::Engine;
use serde_json::json;
use tracing::debug;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use openagents::server::services::auth::{AuthError, OIDCConfig, OIDCService};
mod common;
use common::setup_test_db;

// Helper function to create test service with unique client ID
async fn create_test_service(base_url: String) -> OIDCService {
    let config = OIDCConfig::new(
        format!("test_client_{}", Uuid::new_v4()),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/authorize", base_url),
        format!("{}/token", base_url),
    )
    .unwrap();
    let pool = setup_test_db().await;
    OIDCService::new(pool, config)
}

// Helper function to create a test JWT token
fn create_test_token(sub: &str) -> String {
    // Create a simple JWT token with just the sub claim
    // Header: {"alg":"HS256","typ":"JWT"}
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");

    // Claims: {"sub":"test_user_123"}
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!("{{\"sub\":\"{}\"}}", sub).as_bytes());

    // Signature (not validated in tests)
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"signature");

    format!("{}.{}.{}", header, claims, signature)
}

#[tokio::test]
async fn test_signup_authorization_url() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    let url = service
        .authorization_url_for_signup("test@example.com")
        .unwrap();

    assert!(url.contains("response_type=code"));
    assert!(url.contains("prompt=create"));
    assert!(url.contains("scope=openid"));
    assert!(url.contains("email=test%40example.com"));
}

#[tokio::test]
async fn test_signup_flow() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    // Start a transaction for test isolation
    let mut tx = service.pool.begin().await.unwrap();

    // Generate unique test user ID
    let test_user_id = format!("test_user_{}", Uuid::new_v4());

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": create_test_token(&test_user_id),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // Test successful signup
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "Signup failed: {:?}", result.err());

    let user = result.unwrap();
    assert_eq!(user.scramble_id, Some(test_user_id.clone()));

    // Verify user was created
    let db_user = sqlx::query!(
        "SELECT scramble_id FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    assert_eq!(db_user.scramble_id, Some(test_user_id));

    // Rollback the transaction
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    // Start a transaction for test isolation
    let mut tx = service.pool.begin().await.unwrap();

    // Generate unique test user ID
    let test_user_id = format!("test_user_{}", Uuid::new_v4());

    // Verify database is empty
    let count = sqlx::query!("SELECT COUNT(*) as count FROM users")
        .fetch_one(&mut *tx)
        .await
        .unwrap()
        .count
        .unwrap_or(0);
    assert_eq!(count, 0, "Database should be empty at start");

    // Mock token endpoint for both requests
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": create_test_token(&test_user_id),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .expect(2) // Expect two calls
        .mount(&mock_server)
        .await;

    // First signup should succeed
    debug!("Attempting first signup");
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "First signup failed: {:?}", result.err());

    // Verify first user was created
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "First user should exist");

    // Second signup with same pseudonym should fail with UserAlreadyExists
    debug!("Attempting duplicate signup");
    let err = service.signup("test_code".to_string()).await.unwrap_err();
    assert!(matches!(err, AuthError::UserAlreadyExists(_)));

    // Verify still only one user exists
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "Should still be only one user");

    // Rollback the transaction
    tx.rollback().await.unwrap();
}
