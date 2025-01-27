use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use openagents::server::services::auth::{AuthError, OIDCConfig};

async fn setup_test_db() -> PgPool {
    let pool = PgPool::connect("postgres://postgres:postgres@localhost/test_db")
        .await
        .unwrap();

    // Clean up any existing test data
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[tokio::test]
async fn test_full_signup_flow() {
    // Setup
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;

    let config = OIDCConfig::new(
        "client123".to_string(),
        "secret456".to_string(),
        "http://localhost:3000/callback".to_string(),
        mock_server.uri(),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();

    // Test signup authorization URL
    let signup_url = config.authorization_url(true);
    assert!(signup_url.contains("prompt=create"));

    // Setup token endpoint mock
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": "header.eyJzdWIiOiJ0ZXN0X3NpZ251cF91c2VyIn0.signature"
        })))
        .mount(&mock_server)
        .await;

    // Test successful signup
    let user = config
        .signup("test_signup_code".to_string(), &pool)
        .await
        .unwrap();
    assert_eq!(user.scramble_id, "test_signup_user");

    // Test duplicate signup
    let result = config.signup("test_signup_code".to_string(), &pool).await;
    assert!(matches!(result, Err(AuthError::UserAlreadyExists)));

    // Verify user in database
    let db_user = sqlx::query!("SELECT * FROM users WHERE scramble_id = $1", "test_signup_user")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_user.scramble_id, "test_signup_user");

    // Cleanup
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", "test_signup_user")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_signup_error_handling() {
    // Setup
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;

    let config = OIDCConfig::new(
        "client123".to_string(),
        "secret456".to_string(),
        "http://localhost:3000/callback".to_string(),
        mock_server.uri(),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();

    // Test invalid token response
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_string("Invalid code"))
        .mount(&mock_server)
        .await;

    let result = config.signup("invalid_code".to_string(), &pool).await;
    assert!(matches!(result, Err(AuthError::TokenExchangeFailed(_))));

    // Test malformed ID token
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": "invalid_token"
        })))
        .mount(&mock_server)
        .await;

    let result = config.signup("test_code".to_string(), &pool).await;
    assert!(matches!(result, Err(AuthError::AuthenticationFailed)));
}