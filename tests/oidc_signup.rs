use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};
use serde_json::json;

use openagents::server::services::auth::OIDCService;

#[tokio::test]
async fn test_signup_authorization_url() {
    let mock_server = MockServer::start().await;
    let service = OIDCService::new_with_base_url(mock_server.uri());

    let url = service.authorization_url_for_signup().unwrap();
    
    assert!(url.contains("response_type=code"));
    assert!(url.contains("prompt=create"));
    assert!(url.contains("scope=openid"));
}

#[tokio::test]
async fn test_signup_flow() {
    let mock_server = MockServer::start().await;
    let service = OIDCService::new_with_base_url(mock_server.uri());

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0X3VzZXJfMTIzIn0.qsI_HfwqxJnI0eI-OjDj0DBAFDgxhpIjK9-0lmXp8Kk",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // Test successful signup
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok());
    
    let user = result.unwrap();
    assert_eq!(user.scramble_id, "test_user_123");
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = OIDCService::new_with_base_url(mock_server.uri());

    // Mock token endpoint for first signup
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test_access_token",
            "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0X3VzZXJfMTIzIn0.qsI_HfwqxJnI0eI-OjDj0DBAFDgxhpIjK9-0lmXp8Kk",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // First signup should succeed
    let _ = service.signup("test_code".to_string()).await.unwrap();

    // Second signup with same pseudonym should fail
    let err = service.signup("test_code".to_string()).await.unwrap_err();
    assert!(matches!(err, openagents::server::services::auth::AuthError::UserAlreadyExists));
}