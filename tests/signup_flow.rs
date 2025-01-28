mod common;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, Level};
use tracing_subscriber::fmt::format::FmtSpan;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use openagents::server::services::auth::{AuthError, OIDCConfig, OIDCService};
use openagents::server::models::user::User;

use crate::common::setup_test_db;

fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_test_writer()
        .with_span_events(FmtSpan::NONE)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .try_init();
}

fn create_test_service(mock_server: &MockServer, pool: PgPool) -> OIDCService {
    let config = OIDCConfig::new(
        "client123".to_string(),
        "test_secret".to_string(),
        "http://localhost:3000/callback".to_string(),
        format!("{}/authorize", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .unwrap();

    OIDCService::new(pool, config)
}

fn create_test_token(sub: &str) -> String {
    // Create a simple JWT token for testing
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(&format!(r#"{{"sub":"{}","iat":1516239022}}"#, sub));
    let signature = URL_SAFE_NO_PAD.encode("test_signature");
    format!("{}.{}.{}", header, claims, signature)
}

#[tokio::test]
async fn test_signup_authorization_url() {
    init_test_logging();
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;
    let service = create_test_service(&mock_server, pool);

    let url = service.authorization_url_for_signup().unwrap();
    assert!(url.contains("prompt=create"));
    assert!(url.contains("client_id=client123"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("scope=openid"));
}

#[tokio::test]
async fn test_signup_flow() {
    init_test_logging();
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;
    let service = create_test_service(&mock_server, pool.clone());

    // Setup mock token endpoint
    let test_token = create_test_token("test_user_123");
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

    // Test successful signup
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "Signup failed: {:?}", result.err());
    
    let user = result.unwrap();
    assert_eq!(user.scramble_id, "test_user_123");

    // Verify user was created in database
    let db_user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE scramble_id = $1",
        "test_user_123"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(db_user.scramble_id, "test_user_123");
}

#[tokio::test]
async fn test_duplicate_signup() {
    init_test_logging();
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;
    let service = create_test_service(&mock_server, pool.clone());

    // Setup mock token endpoint for both calls
    let test_token = create_test_token("test_user_456");
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

    // First signup should succeed
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok(), "First signup failed: {:?}", result.err());

    // Second signup with same pseudonym should fail
    let result = service.signup("test_code".to_string()).await;
    assert!(matches!(result, Err(AuthError::UserAlreadyExists)));

    // Verify only one user exists
    let count = sqlx::query!("SELECT COUNT(*) as count FROM users WHERE scramble_id = $1", "test_user_456")
        .fetch_one(&pool)
        .await
        .unwrap()
        .count
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_signup_error_handling() {
    init_test_logging();
    info!("Starting error handling test");
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;
    let service = create_test_service(&mock_server, pool);

    // Test malformed token response
    info!("Testing malformed token response");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock_server)
        .await;

    let result = service.signup("test_code".to_string()).await;
    info!("Got result: {:?}", result);
    assert!(matches!(result, Err(AuthError::TokenExchangeFailed(_))), 
        "Expected TokenExchangeFailed, got {:?}", result);

    // Test invalid token response
    info!("Testing invalid token response");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_string("Invalid code"))
        .mount(&mock_server)
        .await;

    let result = service.signup("invalid_code".to_string()).await;
    info!("Got result: {:?}", result);
    assert!(matches!(result, Err(AuthError::TokenExchangeFailed(_))), 
        "Expected TokenExchangeFailed, got {:?}", result);

    // Test invalid JWT token
    info!("Testing invalid JWT token");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": "not.a.jwt"
        })))
        .mount(&mock_server)
        .await;

    let result = service.signup("test_code".to_string()).await;
    info!("Got result: {:?}", result);
    assert!(matches!(result, Err(AuthError::AuthenticationFailed)), 
        "Expected AuthenticationFailed, got {:?}", result);
}