use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::successor;
use crate::{Config, router, upstream};

const API: &str = "api.openagents.com";

/// A gateway that echoes what reached it and sets a cookie.
async fn gateway() -> String {
    let app = Router::new().fallback(|request: Request<Body>| async move {
        let headers = request.headers().clone();
        let value = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        let mut echo = json!({
            "method": request.method().as_str(),
            "uri": request.uri().to_string(),
            "host": value("host"),
            "authorization": value("authorization"),
            "cookie": value("cookie"),
        });
        let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
        echo["body"] = json!(String::from_utf8_lossy(&body));
        axum::response::Response::builder()
            .status(StatusCode::IM_A_TEAPOT)
            .header(header::SET_COOKIE, "gateway=1; Path=/")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(echo.to_string()))
            .unwrap()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

/// openagents.com as production runs it: the gateway behind, no previous
/// server, and the API host when `api` is set.
async fn site(root: &std::path::Path, api: bool) -> Router {
    let mut config = Config::development(root.join("tasks"));
    config.public_hosts.push("openagents.com".to_owned());
    config.inference = Some(Arc::new(upstream::Upstream::new(&gateway().await).unwrap()));
    if api {
        config.api_host = Some(API.to_owned());
    }
    router(config)
}

async fn call(
    site: &Router,
    method: &str,
    host: &str,
    uri: &str,
    extra: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, host);
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let response = site
        .clone()
        .oneshot(request.body(Body::from("{\"model\":\"coder\"}")).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[test]
fn the_successor_is_the_same_path_on_the_api_host() {
    assert_eq!(
        successor(API, "/v1/agents/challenge").as_deref(),
        Some("https://api.openagents.com/v1/agents/challenge")
    );
    assert_eq!(
        successor(API, "/api/v1/models/gpt").as_deref(),
        Some("https://api.openagents.com/v1/models/gpt")
    );
    for path in [
        "/",
        "/docs/api",
        "/api/traces",
        "/v1",
        "/api/v1",
        "/apiv1/x",
        "/v10/x",
    ] {
        assert_eq!(successor(API, path), None, "{path}");
    }
}

/// The API host answers the gateway's PUBLIC routes as openagents.com
/// does: the key passes, the body passes, cookies go neither way, and the
/// answer is not marked deprecated.
#[tokio::test]
async fn the_api_host_forwards_the_public_routes_with_keys_and_no_cookies() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), true).await;
    for (method, uri) in [
        ("GET", "/v1/models"),
        ("GET", "/v1/agents/challenge"),
        ("POST", "/v1/agents/register"),
        ("POST", "/v1/responses"),
        ("POST", "/v1/chat/completions"),
        ("POST", "/v1/decisions"),
        ("GET", "/v1/account"),
    ] {
        let (status, headers, body) = call(
            &site,
            method,
            API,
            uri,
            &[
                ("authorization", "Bearer oak_1.secret"),
                ("cookie", "oa_cloud_session=abc"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::IM_A_TEAPOT, "{method} {uri}");
        let echo: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(echo["method"], method);
        assert_eq!(echo["uri"], uri);
        assert_eq!(echo["host"], API);
        assert_eq!(echo["authorization"], "Bearer oak_1.secret");
        assert_eq!(echo["cookie"], "");
        assert_eq!(echo["body"], "{\"model\":\"coder\"}");
        assert!(headers.get(header::SET_COOKIE).is_none(), "{uri}");
        assert!(headers.get("deprecation").is_none(), "{uri}");
    }
}

/// The site's own `/v1` routes answer on the API host as on
/// openagents.com, and a cookie (a cross-site form post's only
/// credential) signs nothing in there.
#[tokio::test]
async fn the_api_host_answers_the_sites_own_v1_routes_the_same() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), true).await;
    for (method, uri) in [
        ("GET", "/v1/projects"),
        ("POST", "/v1/device/code"),
        ("POST", "/v1/device/token"),
        ("POST", "/v1/device/sign-out"),
        ("POST", "/v1/projects"),
        ("GET", "/v1/files"),
        ("GET", "/v1/threads"),
        ("GET", "/v1/computers"),
        ("GET", "/v1/traces"),
        ("GET", "/v1/token"),
    ] {
        let (old_status, _, old_body) = call(&site, method, "openagents.com", uri, &[]).await;
        let (status, _, body) = call(
            &site,
            method,
            API,
            uri,
            &[
                ("cookie", "oa_cloud_session=abc"),
                ("origin", "https://evil.example"),
            ],
        )
        .await;
        assert_eq!(status, old_status, "{method} {uri}");
        assert!(!status.is_success(), "{method} {uri}: {status}");
        assert_eq!(body, old_body, "{method} {uri}");
        if uri.starts_with("/v1/device/") {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let error: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(error["error"], "temporarily_unavailable");
        }
    }
}

/// Only `/v1/...` is the API host's.
#[tokio::test]
async fn the_api_host_answers_nothing_but_v1() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), true).await;
    for uri in [
        "/",
        "/chat",
        "/docs/api",
        "/api/v1/models",
        "/login",
        "/app",
    ] {
        let (status, headers, body) = call(&site, "GET", API, uri, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json", "{uri}");
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["error"]["code"], "not_found");
    }
}

/// openagents.com/v1 and openagents.com/api/v1 answer exactly as before,
/// never a redirect, and name their successor on the API host.
#[tokio::test]
async fn the_older_addresses_still_answer_and_are_marked_deprecated() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), true).await;
    for (method, uri, forwarded) in [
        ("GET", "/v1/agents/challenge", "/v1/agents/challenge"),
        ("POST", "/v1/responses", "/v1/responses"),
        ("GET", "/api/v1/agents/challenge", "/v1/agents/challenge"),
        ("POST", "/api/v1/chat/completions", "/v1/chat/completions"),
        // Coder 1.0.0's Decisions call (jev::decisions::OPENAGENTS).
        ("POST", "/api/v1/decisions", "/v1/decisions"),
    ] {
        let (status, headers, body) = call(
            &site,
            method,
            "openagents.com",
            uri,
            &[("authorization", "Bearer oak_1.secret")],
        )
        .await;
        assert_eq!(status, StatusCode::IM_A_TEAPOT, "{method} {uri}");
        let echo: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(echo["uri"], forwarded);
        assert_eq!(echo["body"], "{\"model\":\"coder\"}");
        assert_eq!(headers["deprecation"], "true", "{uri}");
        assert_eq!(
            headers[header::LINK],
            format!("<https://api.openagents.com{forwarded}>; rel=\"successor-version\""),
            "{uri}"
        );
        assert!(headers.get("sunset").is_none());
    }
    // The site's own /v1 routes too, whatever they answer.
    let (_, headers, _) = call(&site, "GET", "openagents.com", "/v1/projects", &[]).await;
    assert_eq!(headers["deprecation"], "true");
    assert_eq!(
        headers[header::LINK],
        "<https://api.openagents.com/v1/projects>; rel=\"successor-version\""
    );
    // Pages are not API.
    let (_, headers, _) = call(&site, "GET", "openagents.com", "/docs/api", &[]).await;
    assert!(headers.get("deprecation").is_none());
}

/// Without an API host (staging, a local run) nothing changes.
#[tokio::test]
async fn without_an_api_host_nothing_is_marked() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), false).await;
    let (status, headers, _) = call(&site, "GET", "openagents.com", "/v1/models", &[]).await;
    assert_eq!(status, StatusCode::IM_A_TEAPOT);
    assert!(headers.get("deprecation").is_none());
    assert!(headers.get(header::LINK).is_none());
    let (status, _, _) = call(&site, "GET", API, "/v1/models", &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn public_openapi_includes_app_auth_and_keeps_gateway_operations() {
    let root = tempfile::tempdir().unwrap();
    let upstream = Router::new().route(
        "/openapi.json",
        axum::routing::get(|| async {
            axum::Json(json!({"openapi":"3.1.0", "paths":{"/v1/models":{"get":{}}}}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let mut config = Config::development(root.path().join("tasks"));
    config.public_hosts.push("openagents.com".into());
    config.api_host = Some(API.into());
    config.inference = Some(Arc::new(
        upstream::Upstream::new(&format!("http://{addr}")).unwrap(),
    ));
    let site = router(config);
    for (host, path) in [
        (API, "/v1/openapi.json"),
        ("openagents.com", "/openapi.json"),
        ("openagents.com", "/api/v1/openapi.json"),
    ] {
        let (status, _, body) = call(&site, "GET", host, path, &[]).await;
        assert_eq!(status, StatusCode::OK, "{host}{path}: {body}");
        let doc: Value = serde_json::from_str(&body).unwrap();
        assert!(doc["paths"]["/v1/models"]["get"].is_object());
        for path in [
            "/v1/device/code",
            "/v1/device/token",
            "/v1/device/sign-out",
            "/v1/account",
        ] {
            assert!(doc["paths"][path].is_object(), "{path}");
        }
        for path in [
            "/v1/session",
            "/v1/sessions/github",
            "/v1/sessions/device/decide",
        ] {
            assert!(doc["paths"].get(path).is_none());
        }
    }
}
