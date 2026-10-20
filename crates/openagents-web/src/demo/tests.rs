use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:4300";

async fn send(
    config: crate::Config,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = crate::router(config).oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn get(path: &str, host: &str, htmx: bool) -> Request<Body> {
    let mut request = Request::builder().uri(path).header(header::HOST, host);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    request.body(Body::empty()).unwrap()
}

fn post(path: &str, form: &str, htmx: bool) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if htmx {
        request = request.header("HX-Request", "true");
    }
    request.body(Body::from(form.to_owned())).unwrap()
}

fn config(root: &std::path::Path) -> crate::Config {
    let mut config = crate::Config::development(root.join("must-not-be-created"));
    config.public_hosts.push("openagents.com".into());
    config
}

#[tokio::test]
async fn every_html_demo_address_permanently_redirects_without_a_build() {
    let root = tempfile::tempdir().unwrap();
    for path in [
        "/demo",
        "/demo/environment",
        "/demo/5",
        "/demo/nope",
        "/demo/lease-fix/thread",
        "/demo/lease-fix/message",
    ] {
        for htmx in [false, true] {
            let (status, headers, _) =
                send(config(root.path()), get(path, "openagents.com", htmx)).await;
            assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{path}");
            assert_eq!(headers[header::LOCATION], "/demo/gpui", "{path}");
        }
    }
    let (status, headers, _) = send(
        config(root.path()),
        post("/demo/lease-fix/message", "q=Hello", true),
    )
    .await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/demo/gpui");
    assert!(!root.path().join("must-not-be-created").exists());
}

#[tokio::test]
async fn the_gpui_demo_still_loads() {
    let root = tempfile::tempdir().unwrap();
    let build = root.path().join("gpui-demo");
    std::fs::create_dir(&build).unwrap();
    for file in [
        crate::pages::gpui_demo::GPUI_DEMO_GLUE,
        crate::pages::gpui_demo::GPUI_DEMO_WASM,
        crate::pages::gpui_demo::GPUI_DEMO_START,
    ] {
        std::fs::write(build.join(file), "").unwrap();
    }
    let mut config = config(root.path());
    config.gpui_demo = Some(build);
    let (status, _, _) = send(config, get("/demo/gpui", "openagents.com", false)).await;
    assert_eq!(status, StatusCode::OK);
}
