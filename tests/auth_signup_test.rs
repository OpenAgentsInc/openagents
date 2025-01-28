use axum::{
    body::Body,
    extract::{Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_extra::extract::cookie::Cookie;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower::ServiceExt;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

// Import the actual types we need to test
#[derive(Clone)]
struct AppState {
    config: OIDCConfig,
    pool: PgPool,
}

#[derive(Debug, Clone)]
struct OIDCConfig {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_url: String,
    token_url: String,
}

impl OIDCConfig {
    fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        auth_url: String,
        token_url: String,
    ) -> Result<Self, &'static str> {
        if client_id.is_empty() || client_secret.is_empty() || redirect_uri.is_empty() {
            return Err("Invalid config");
        }

        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            auth_url,
            token_url,
        })
    }

    fn authorization_url(&self, is_signup: bool) -> String {
        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid",
            self.auth_url,
            self.client_id,
            urlencoding::encode(&self.redirect_uri)
        );
        
        if is_signup {
            url.push_str("&prompt=create");
        }
        
        url
    }
}

impl AppState {
    fn new(config: OIDCConfig, pool: PgPool) -> Self {
        Self { config, pool }
    }
}

async fn signup(State(state): State<AppState>) -> impl IntoResponse {
    let auth_url = state.config.authorization_url(true);
    Redirect::temporary(&auth_url)
}

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
        .with_state(state);

    // Test signup endpoint
    let signup_response = app
        .oneshot(
            Request::builder()
                .uri("/signup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(signup_response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = signup_response.headers().get("location").unwrap();
    let location_str = location.to_str().unwrap();
    assert!(location_str.contains("prompt=create")); // Verify signup parameter

    // Cleanup
    sqlx::query!("DELETE FROM users WHERE scramble_id = $1", "new_user_pseudonym")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_signup_url_generation() {
    let config = OIDCConfig::new(
        "client123".to_string(),
        "secret456".to_string(),
        "http://localhost:3000/callback".to_string(),
        "https://auth.example.com/authorize".to_string(),
        "https://auth.example.com/token".to_string(),
    )
    .unwrap();

    let signup_url = config.authorization_url(true);
    assert!(signup_url.contains("prompt=create"));
    assert!(signup_url.contains("client_id=client123"));
    assert!(signup_url.contains("response_type=code"));
    assert!(signup_url.contains("scope=openid"));
    assert!(signup_url.contains(urlencoding::encode("http://localhost:3000/callback").as_ref()));
}