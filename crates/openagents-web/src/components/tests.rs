use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use super::*;

async fn request(
    config: crate::Config,
    path: &str,
    host: &str,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = crate::router(config)
        .oneshot(
            Request::builder()
                .uri(path)
                .header(header::HOST, host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn catalog_is_public_synthetic_and_never_reaches_the_task_store() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("must-not-be-created");
    let mut config = crate::Config::development(store.clone());
    config.public_hosts.push("openagents.com".into());
    let (status, headers, html) = request(config.clone(), "/components", "openagents.com").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("catalog-preview"));
    assert!(html.contains("synthetic fixtures"));
    assert!(!html.contains("src=\"/components/assets/start.js\""));
    assert!(
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
    assert!(!store.exists());
    assert_eq!(
        request(config, "/app", "openagents.com").await.0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn every_registered_variant_renders_as_html_and_has_owned_routes() {
    let root = tempfile::tempdir().unwrap();
    let config = crate::Config::development(root.path().join("tasks"));
    for entry in catalog::entries() {
        assert!(!entry.sources.is_empty(), "{} has no source", entry.id);
        for variant in &entry.variants {
            let path = format!("/components/{}?variant={}", entry.id, variant.id);
            assert!(crate::upstream::owned(&path.split('?').next().unwrap()));
            let (status, _, body) = request(config.clone(), &path, "127.0.0.1:4300").await;
            assert_eq!(status, StatusCode::OK, "{path}: {body}");
            assert!(body.contains("data-rn-instance"), "{path}");
        }
    }
}

#[tokio::test]
async fn unknown_variants_and_unbounded_viewports_refuse() {
    let root = tempfile::tempdir().unwrap();
    let config = crate::Config::development(root.path().join("tasks"));
    for path in [
        "/components/screen.main?variant=missing",
        "/components/screen.main?width=65535",
        "/components/screen.main?height=0",
    ] {
        assert_eq!(
            request(config.clone(), path, "127.0.0.1:4300").await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request(config, "/components/missing", "127.0.0.1:4300")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn only_named_build_assets_are_served_and_wasm_enables_the_loader() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(GLUE), "export default async function(){};").unwrap();
    std::fs::write(root.path().join(WASM), b"\0asm").unwrap();
    std::fs::write(root.path().join("private.json"), "never serve").unwrap();
    let mut config = crate::Config::development(root.path().join("tasks"));
    config.components_build = Some(root.path().to_owned());
    let (status, _, html) = request(config.clone(), "/components", "127.0.0.1:4300").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("src=\"/components/assets/start.js\""));
    let (status, headers, _) = request(
        config.clone(),
        "/components/assets/coder_components_web_bg.wasm",
        "127.0.0.1:4300",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/wasm");
    assert_eq!(
        request(config, "/components/assets/private.json", "127.0.0.1:4300")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(crate::upstream::owned("/components/assets/private.json"));
}
