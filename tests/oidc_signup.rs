use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};
use serde_json::json;
use base64::Engine;

use openagents::server::services::auth::{OIDCService, OIDCConfig};

// Helper function to create test service
fn create_test_service(base_url: String) -> OIDCService {
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/authorize", base_url),
        format!("{}/token", base_url),
    ).unwrap();
    let pool = sqlx::Pool::connect_lazy("postgres://postgres:postgres@localhost/test").unwrap();
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
    let service = create_test_service(mock_server.uri());

    let url = service.authorization_url_for_signup().unwrap();
    
    assert!(url.contains("response_type=code"));
    assert!(url.contains("prompt=create"));
    assert!(url.contains("scope=openid"));
}

#[tokio::test]
async fn test_signup_flow() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri());

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
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(mock_server.uri());

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
    let _ = service.signup("test_code".to_string()).await.unwrap();

    // Second signup with same pseudonym should fail
    let err = service.signup("test_code".to_string()).await.unwrap_err();
    assert!(matches!(err, openagents::server::services::auth::AuthError::UserAlreadyExists));
}