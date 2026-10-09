use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use super::chats::CHATS;

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
async fn demo_is_public_in_the_shared_shell_and_never_reaches_the_task_store() {
    let root = tempfile::tempdir().unwrap();
    assert!(crate::upstream::owned("/demo"));
    let (status, headers, html) =
        send(config(root.path()), get("/demo", "openagents.com", false)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!root.path().join("must-not-be-created").exists());
    for needle in [
        "class=\"oa-layout\"",
        "id=\"demo-chats\"",
        "href=\"/demo/environment\" hx-get=\"/demo/environment/thread\"",
        "aria-current=\"page\"",
        "class=\"oa-thread-view\"",
        "class=\"oa-tool-group\"",
        "class=\"oa-result-card\"",
        "class=\"oa-steps\"",
        "id=\"demo-composer\"",
        "action=\"/demo/environment/message\"",
        "src=\"/static/htmx.min.js\"",
        "Set up the repository environment",
    ] {
        assert!(html.contains(needle), "{needle}");
    }
    for chat in &CHATS {
        assert!(
            html.contains(&format!("href=\"/demo/{}\"", chat.slug)),
            "{}",
            chat.slug
        );
    }
    for gone in [
        "chat-start.js",
        "htmx-sse.js",
        "demo-html.css",
        "legacy-demo.css",
    ] {
        assert!(!html.contains(gone), "{gone}");
    }
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("script-src 'self';"), "{policy}");
    assert!(!policy.contains("unsafe"), "{policy}");
}

#[tokio::test]
async fn each_chat_has_its_own_page_and_thread_fragment() {
    let root = tempfile::tempdir().unwrap();
    for chat in &CHATS {
        let path = format!("/demo/{}", chat.slug);
        let (status, _, html) = send(config(root.path()), get(&path, HOST, false)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(html.contains(&format!("href=\"{path}\" hx-get")), "{path}");
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1, "{path}");
        let (status, headers, fragment) = send(
            config(root.path()),
            get(&format!("{path}/thread"), HOST, true),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(headers["HX-Push-Url"], path.as_str());
        assert!(!fragment.contains("<html"), "{path}");
        assert!(fragment.contains("id=\"demo-chats\" hx-swap-oob=\"outerHTML\""));
        assert!(fragment.contains("id=\"demo-dock\" hx-swap-oob=\"outerHTML\""));
        assert!(fragment.contains(chat.title));
    }
}

#[tokio::test]
async fn unknown_chats_are_missing_and_old_numbered_links_return_to_the_demo() {
    let root = tempfile::tempdir().unwrap();
    let (status, _, _) = send(config(root.path()), get("/demo/nope", HOST, false)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, headers, _) = send(config(root.path()), get("/demo/5", HOST, false)).await;
    assert!(status.is_redirection());
    assert_eq!(headers[header::LOCATION], "/demo");
}

#[tokio::test]
async fn a_sent_message_gets_a_scripted_reply_with_and_without_htmx() {
    let root = tempfile::tempdir().unwrap();
    let (status, _, fragment) = send(
        config(root.path()),
        post("/demo/lease-fix/message", "q=Hello+%3Cthere%3E", true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(fragment.contains("hx-swap-oob=\"beforeend:#demo-transcript\""));
    assert!(fragment.contains("Hello &lt;there&gt;"));
    assert!(fragment.contains("wasn't sent to an agent"));
    assert!(fragment.contains("id=\"demo-dock\" hx-swap-oob=\"outerHTML\""));
    let (status, _, page) = send(
        config(root.path()),
        post("/demo/lease-fix/message", "q=Hello", false),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("class=\"oa-layout\""));
    assert!(page.contains("wasn't sent to an agent"));
    let (status, _, _) = send(
        config(root.path()),
        post("/demo/lease-fix/message", "q=+", true),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
