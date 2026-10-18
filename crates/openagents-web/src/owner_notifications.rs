//! The public API forwards owner notes to the sync service that holds phone grants.
use crate::App;
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, OriginalUri, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::time::Duration;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/notify", get(forward).post(forward))
        .route("/v1/notify/grants/{agent}", put(forward))
        .route("/v1/notify/{id}/read", post(forward))
        .layer(DefaultBodyLimit::max(8192))
}
async fn forward(
    State(app): State<App>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(bearer) = headers
        .get("authorization")
        .filter(|h| h.to_str().is_ok_and(|h| h.starts_with("Bearer ")))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let base = &app.config.owner_notifications_url;
    let Ok(base_url) = reqwest::Url::parse(base) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
        || base_url.path() != "/"
        || (base_url.scheme() != "https"
            && !(base_url.scheme() == "http"
                && base_url.host_str().is_some_and(|h| {
                    h == "localhost"
                        || h.parse::<std::net::IpAddr>()
                            .is_ok_and(|ip| ip.is_loopback())
                })))
    {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Ok(client) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(45))
        .build()
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let mut request = client
        .request(method, format!("{}{}", base.trim_end_matches('/'), path))
        .header("authorization", bearer)
        .header("content-type", "application/json")
        .body(body);
    if headers
        .get("x-openagents-test")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.trim() == "1")
    {
        request = request.header("x-openagents-test", "1");
    }
    let Ok(mut upstream) = request.send().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let status = upstream.status();
    let after = upstream.headers().get("retry-after").cloned();
    let mut bytes = Vec::new();
    loop {
        match upstream.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 16384 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return StatusCode::BAD_GATEWAY.into_response(),
        }
    }
    let mut response = (status, [("content-type", "application/json")], bytes).into_response();
    if let Some(after) = after {
        response.headers_mut().insert("retry-after", after);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, body::Body, http::Request};
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn public_notify_routes_forward_only_account_auth_and_test_marker() {
        async fn upstream(
            headers: HeaderMap,
            method: Method,
            OriginalUri(uri): OriginalUri,
            body: Bytes,
        ) -> Response {
            assert_eq!(headers["authorization"], "Bearer fixture-only");
            assert_eq!(headers["x-openagents-test"], "1");
            assert!(!headers.contains_key("cookie"));
            assert!(!headers.contains_key("x-coder-verified-account"));
            (StatusCode::TOO_MANY_REQUESTS,[("retry-after","3600")],Json(json!({"method":method.as_str(),"path":uri.path(),"body":String::from_utf8_lossy(&body)}))).into_response()
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut config = crate::Config::development(root.path().join("tasks"));
        config.owner_notifications_url = format!("http://{}", listener.local_addr().unwrap());
        let upstream_task = tokio::spawn(async move {
            axum::serve(listener, Router::new().fallback(upstream))
                .await
                .unwrap()
        });
        let router = routes().with_state(App(std::sync::Arc::new(crate::Inner { config })));
        for (method, path) in [
            (Method::POST, "/v1/notify"),
            (Method::POST, "https://public.example/v1/notify"),
            (Method::PUT, "/v1/notify/grants/agent"),
            (Method::GET, "/v1/notify?device=phone"),
            (Method::POST, "/v1/notify/request/read?device=phone"),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("authorization", "Bearer fixture-only")
                        .header("x-openagents-test", "1")
                        .header("cookie", "private-cookie")
                        .header("x-coder-verified-account", "other")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()["retry-after"], "3600");
        }
        assert_eq!(
            router
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/v1/notify")
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        upstream_task.abort();
    }
}
