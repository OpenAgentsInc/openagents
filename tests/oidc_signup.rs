use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};
use serde_json::json;
use base64::Engine;
use sqlx::PgPool;
use tracing::{debug, info};

use openagents::server::services::auth::{OIDCService, OIDCConfig};

async fn clean_test_db(pool: &PgPool) {
    info!("Cleaning up test database");
    
    // Terminate other connections
    sqlx::query!("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()")
        .fetch_all(pool)
        .await
        .unwrap();

    // Truncate the table
    sqlx::query!("TRUNCATE TABLE users RESTART IDENTITY CASCADE")
        .execute(pool)
        .await
        .unwrap();

    // Verify the table is empty
    let count = sqlx::query!("SELECT COUNT(*) as count FROM users")
        .fetch_one(pool)
        .await
        .unwrap()
        .count
        .unwrap_or(0);
    
    assert_eq!(count, 0, "Database should be empty after cleanup");
    info!("Database cleaned successfully");
}

async fn setup_test_db() -> PgPool {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    clean_test_db(&pool).await;
    pool
}

// Helper function to create test service
async fn create_test_service(base_url: String) -> OIDCService {
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/authorize", base_url),
        format!("{}/token", base_url),
    ).unwrap();
    let pool = setup_test_db().await;
    OIDCService::new(pool, config)
}

// Helper function to create a test JWT token
fn create_test_token(sub: &str) -> String {
    // Create a simple JWT token with just the sub claim
    // Header: {"alg":"HS256","typ":"JWT"}
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}"
    );
    
    // Claims: {"sub":"test_user_123"}
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        format!("{{\"sub\":\"{}\"}}", sub).as_bytes()
    );
    
    // Signature (not validated in tests)
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"signature");
    
    format!("{}.{}.{}", header, claims, signature)
}

#[tokio::test]
async fn test_signup_authorization_url() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    let url = service.authorization_url_for_signup().unwrap();
    
    assert!(url.contains("response_type=code"));
    assert!(url.contains("prompt=create"));
    assert!(url.contains("scope=openid"));
}

#[tokio::test]
async fn test_signup_flow() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": create_test_token("test_user_123"),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // Test successful signup
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "Signup failed: {:?}", result.err());
    
    let user = result.unwrap();
    assert_eq!(user.scramble_id, "test_user_123");

    // Verify user was created
    let pool = &service.pool;
    let db_user = sqlx::query!(
        "SELECT scramble_id FROM users WHERE scramble_id = $1",
        "test_user_123"
    )
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(db_user.scramble_id, "test_user_123");
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri()).await;

    // Verify database is empty
    let pool = &service.pool;
    let count = sqlx::query!("SELECT COUNT(*) as count FROM users")
        .fetch_one(pool)
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
            "id_token": create_test_token("test_user_123"),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .expect(2)  // Expect two calls
        .mount(&mock_server)
        .await;

    // First signup should succeed
    debug!("Attempting first signup");
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "First signup failed: {:?}", result.err());

    // Verify first user was created
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        "test_user_123"
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "First user should exist");

    // Second signup with same pseudonym should fail
    debug!("Attempting duplicate signup");
    let err = service.signup("test_code".to_string()).await.unwrap_err();
    assert!(matches!(err, openagents::server::services::auth::AuthError::UserAlreadyExists));

    // Verify still only one user exists
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        "test_user_123"
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "Should still be only one user");
}