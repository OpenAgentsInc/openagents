use crate::configuration::get_configuration;
use axum::{
    body::Body,
    extract::Request,
    http::{header, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::json;

pub async fn admin_auth(req: Request<Body>, next: Next) -> impl IntoResponse {
    let config = match get_configuration() {
        Ok(config) => config,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "error": format!("Config error: {}", e)
                    })
                    .to_string(),
                ))
                .unwrap()
                .into_response();
        }
    };

    // Skip auth in local development
    use crate::configuration::AppEnvironment;
    let env_str = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "local".into());
    println!("Current environment: {}", env_str);

    if let Ok(env) = env_str.try_into() {
        if matches!(env, AppEnvironment::Local) {
            println!("Bypassing auth in local environment");
            return next.run(req).await;
        }
    }

    // Allow access to login routes without authentication
    let path = req.uri().path();
    if path == "/admin/login" || path == "/admin/login/" || path.ends_with("/admin/login") {
        return next.run(req).await;
    }

    // Check Authorization header
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        let expected = format!("Bearer {}", config.application.admin_token);
        if auth_header.as_bytes() == expected.as_bytes() {
            return next.run(req).await;
        }
    }

    // Check URL query parameter
    if let Some(query) = req.uri().query() {
        if let Some(token) = query
            .split('&')
            .find(|p| p.starts_with("token="))
            .map(|p| p.trim_start_matches("token="))
        {
            if token == config.application.admin_token {
                return next.run(req).await;
            }
        }
    }

    // Check session cookie
    if let Some(cookie) = req
        .headers()
        .get(header::COOKIE)
        .and_then(|c| c.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .find(|s| s.trim().starts_with("admin_session="))
                .map(|s| s.trim().trim_start_matches("admin_session="))
        })
    {
        if cookie == config.application.admin_token {
            return next.run(req).await;
        }
    }

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "error": "Unauthorized"
            })
            .to_string(),
        ))
        .unwrap()
        .into_response()
}
