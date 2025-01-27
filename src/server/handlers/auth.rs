use axum::{
    extract::{Query, State},
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::Duration;
use tracing::{debug, error};

use crate::server::{
    models::user::User,
    services::auth::{AuthError, OIDCConfig},
};

const SESSION_COOKIE_NAME: &str = "session";
const SESSION_DURATION_DAYS: i64 = 7;

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    url: String,
}

#[derive(Clone)]
pub struct AppState {
    pub config: OIDCConfig,
    pub pool: PgPool,
}

impl AppState {
    pub fn new(config: OIDCConfig, pool: PgPool) -> Self {
        Self { config, pool }
    }
}

pub async fn login(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let auth_url = state.config.authorization_url();
    Redirect::temporary(&auth_url)
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // Exchange code for tokens and get/create user
    let user = state.config.authenticate(params.code, &state.pool)
        .await
        .map_err(|e| {
            error!("Authentication error: {}", e);
            (
                StatusCode::from(e),
                Json(ErrorResponse {
                    error: e.to_string(),
                })
            )
        })?;

    // Create session cookie
    let cookie = Cookie::build((SESSION_COOKIE_NAME, user.scramble_id.clone()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(SESSION_DURATION_DAYS))
        .build();

    // Set cookie and redirect to home
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok((headers, Redirect::to("/")))
}

pub async fn logout() -> impl IntoResponse {
    // Create cookie that will expire immediately
    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    (headers, Redirect::to("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::Request,
        Router,
        routing::{get, post},
    };
    use tower::ServiceExt;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_auth_flow() {
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

        // Setup mock responses
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "test_access_token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "id_token": "header.eyJzdWIiOiJ0ZXN0X3BzZXVkb255bSJ9.signature"
            })))
            .mount(&mock_server)
            .await;

        // Create test database connection
        let pool = sqlx::PgPool::connect("postgres://postgres:password@localhost/test_db")
            .await
            .unwrap();

        // Create app state
        let state = AppState::new(config, pool);

        // Create test app
        let app = Router::new()
            .route("/login", get(login))
            .route("/callback", get(callback))
            .route("/logout", post(logout))
            .with_state(state);

        // Test login endpoint
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/login").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert!(response.headers().contains_key("location"));

        // Test callback endpoint
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/callback?code=test_code")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert!(response.headers().contains_key(SET_COOKIE));
        assert!(response.headers().contains_key("location"));

        // Test logout endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/logout")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert!(response.headers().contains_key(SET_COOKIE));
        assert!(response.headers().contains_key("location"));
    }
}