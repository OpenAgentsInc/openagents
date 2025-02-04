use openagents::server::services::auth::{AuthError, OIDCConfig, OIDCService};
use sqlx::PgPool;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn create_test_service(mock_server: &MockServer) -> OIDCService {
    let config = OIDCConfig::new(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8000/auth/callback".to_string(),
        format!("{}/auth", mock_server.uri()),
        format!("{}/token", mock_server.uri()),
    )
    .expect("Failed to create OIDC config");

    let pool = PgPool::connect("postgres://postgres:postgres@localhost:5432/postgres")
        .await
        .expect("Failed to connect to database");

    OIDCService::new(pool, config)
}

fn create_test_token() -> String {
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"sub":"test_user"}"#);
    format!("{}.{}.signature", header, claims)
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

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "test_access_token",
                "id_token": create_test_token(),
                "token_type": "Bearer",
                "expires_in": 3600
            })),
        )
        .mount(&mock_server)
        .await;

    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok());

    let user = result.unwrap();
    assert_eq!(user.scramble_id, "test_user");
}

#[tokio::test]
async fn test_duplicate_signup() {
    let mock_server = MockServer::start().await;
    let service = create_test_service(&mock_server).await;

    // Mock token endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "test_access_token",
                "id_token": create_test_token(),
                "token_type": "Bearer",
                "expires_in": 3600
            })),
        )
        .mount(&mock_server)
        .await;

    // First signup should succeed
    let result = service.signup("test_code".to_string()).await;
    assert!(result.is_ok());

    // Second signup should return UserAlreadyExists
    let result = service.signup("test_code".to_string()).await;
    assert!(matches!(
        result,
        Ok(_) // Now returns Ok since we're handling existing users as a success case
    ));
}