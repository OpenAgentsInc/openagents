use base64::Engine;
use openagents::server::services::auth::{AuthError, OIDCConfig, OIDCService};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn create_test_service(mock_server: &MockServer) -> OIDCService {
    let config = OIDCConfig::new(
        format!("test_client_{}", Uuid::new_v4()),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/auth", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .expect("Failed to create OIDC config");

    // Use DATABASE_URL from environment, fall back to default for local dev
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/postgres".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Clean up any existing test data
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await
        .expect("Failed to clean up test data");

    OIDCService::new(pool, config)
}

fn create_test_token(sub: &str) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{}"}}"#, sub));
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"signature");
    format!("{}.{}.{}", header, claims, signature)
}

#[tokio::test]
async fn test_signup_authorization_url() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(&mock_server).await;

    let url = service
        .authorization_url_for_signup("test@example.com")
        .unwrap();

    assert!(url.contains("prompt=create"));
    assert!(url.contains("email=test%40example.com"));
    assert!(url.contains("flow=signup"));
}

#[tokio::test]
async fn test_signup_flow() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(&mock_server).await;

    // Generate unique test user ID
    let test_user_id = format!("test_user_{}", Uuid::new_v4());

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test_access_token",
            "id_token": create_test_token(&test_user_id),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "Signup failed: {:?}", result.err());

    let user = result.unwrap();
    assert_eq!(user.scramble_id, Some(test_user_id.clone()));

    // Verify user was created
    let db_user = sqlx::query!(
        "SELECT scramble_id FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&service.pool)
    .await
    .unwrap();

    assert_eq!(db_user.scramble_id, Some(test_user_id.clone()));

    // Clean up test data
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", test_user_id)
        .execute(&service.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(&mock_server).await;

    // Generate unique test user ID
    let test_user_id = format!("test_user_{}", Uuid::new_v4());

    // Mock token endpoint for both requests
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test_access_token",
            "id_token": create_test_token(&test_user_id),
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .expect(2) // Expect two calls
        .mount(&mock_server)
        .await;

    // First signup should succeed
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "First signup failed: {:?}", result.err());

    // Verify first user was created
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&service.pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "First user should exist");

    // Second signup should return UserAlreadyExists
    let result = service.signup("test_code".to_string()).await;
    assert!(
        matches!(result, Err(AuthError::UserAlreadyExists(_))),
        "Expected UserAlreadyExists error, got: {:?}",
        result
    );

    // Verify still only one user exists
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM users WHERE scramble_id = $1",
        test_user_id
    )
    .fetch_one(&service.pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);
    assert_eq!(count, 1, "Should still be only one user");

    // Clean up test data
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", test_user_id)
        .execute(&service.pool)
        .await
        .unwrap();
}
