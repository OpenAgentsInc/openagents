use std::io::Read;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use futures_util::StreamExt;
use tower::ServiceExt;

use super::*;

const LOCAL: &str = "127.0.0.1:4300";

fn accept(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT_ENCODING, value.parse().unwrap());
    headers
}

fn unbrotli(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    brotli::Decompressor::new(bytes, 4096)
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn ungzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .unwrap();
    out
}

async fn fetch(
    router: Router,
    uri: &str,
    encoding: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().uri(uri).header(header::HOST, LOCAL);
    if let Some(encoding) = encoding {
        request = request.header(header::ACCEPT_ENCODING, encoding);
    }
    let response = router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, body.to_vec())
}

fn page() -> String {
    "<!doctype html><p>OpenAgents</p>".repeat(200)
}

fn small_site() -> Router {
    Router::new()
        .route("/page", get(|| async { axum::response::Html(page()) }))
        .route(
            "/encoded",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                        (header::CONTENT_ENCODING, "br"),
                    ],
                    vec![1u8; 4096],
                )
                    .into_response()
            }),
        )
        .route(
            "/image",
            get(|| async { ([(header::CONTENT_TYPE, "image/png")], vec![0u8; 4096]) }),
        )
        .layer(axum::middleware::from_fn(layer))
}

#[test]
fn brotli_is_preferred_then_gzip() {
    assert_eq!(
        negotiate(&accept("gzip, deflate, br, zstd")),
        Some(Coding::Brotli)
    );
    assert_eq!(negotiate(&accept("gzip, deflate")), Some(Coding::Gzip));
    assert_eq!(negotiate(&accept("br;q=0, gzip")), Some(Coding::Gzip));
    assert_eq!(negotiate(&accept("br;q=0, gzip;q=0")), None);
    assert_eq!(negotiate(&accept("*")), Some(Coding::Brotli));
    assert_eq!(negotiate(&accept("identity")), None);
    assert_eq!(negotiate(&HeaderMap::new()), None);
    assert!(compressible("text/html; charset=utf-8"));
    assert!(compressible("application/wasm"));
    assert!(compressible("image/svg+xml"));
    assert!(compressible("application/manifest+json"));
    assert!(!compressible("text/event-stream"));
    assert!(!compressible("image/png"));
}

#[tokio::test]
async fn text_goes_out_in_the_coding_the_request_accepts() {
    let html = page().into_bytes();
    let (status, headers, body) = fetch(small_site(), "/page", Some("gzip, br")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert_eq!(headers[header::VARY], "accept-encoding");
    assert!(body.len() < html.len() / 4, "{} bytes", body.len());
    assert_eq!(unbrotli(&body), html);

    let (_, headers, body) = fetch(small_site(), "/page", Some("gzip")).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    assert_eq!(ungzip(&body), html);

    let (_, headers, body) = fetch(small_site(), "/page", None).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(headers[header::VARY], "accept-encoding");
    assert_eq!(body, html);
}

#[tokio::test]
async fn encoded_and_binary_bodies_pass_through() {
    let (_, headers, body) = fetch(small_site(), "/encoded", Some("br, gzip")).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert_eq!(body, vec![1u8; 4096]);
    let (_, headers, body) = fetch(small_site(), "/image", Some("br, gzip")).await;
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert!(headers.get(header::VARY).is_none());
    assert_eq!(body, vec![0u8; 4096]);
}

/// An event stream is never held for compression: its first event arrives
/// while the stream is still open, as sent.
#[tokio::test]
async fn an_event_stream_flows_uncompressed_event_by_event() {
    let (send, receive) = tokio::sync::mpsc::unbounded_channel::<String>();
    let receive = std::sync::Arc::new(std::sync::Mutex::new(Some(receive)));
    let site = Router::new()
        .route(
            "/events",
            get(move || {
                let receive = receive.lock().unwrap().take().unwrap();
                async move {
                    let stream = tokio_stream_from(receive)
                        .map(|text| Ok::<_, std::convert::Infallible>(Event::default().data(text)));
                    Sse::new(stream)
                }
            }),
        )
        .layer(axum::middleware::from_fn(layer));
    let response = site
        .oneshot(
            Request::builder()
                .uri("/events")
                .header(header::ACCEPT_ENCODING, "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let mut body = response.into_body().into_data_stream();
    // A long event, well over the compression floor.
    let long = "x".repeat(4096);
    send.send(long.clone()).unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), body.next())
        .await
        .expect("the first event arrives while the stream is open")
        .unwrap()
        .unwrap();
    assert_eq!(
        String::from_utf8(first.to_vec()).unwrap(),
        format!("data: {long}\n\n")
    );
    send.send("second".into()).unwrap();
    let second = tokio::time::timeout(Duration::from_secs(5), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(&second[..], b"data: second\n\n");
}

fn tokio_stream_from(
    mut receive: tokio::sync::mpsc::UnboundedReceiver<String>,
) -> impl futures_util::Stream<Item = String> {
    futures_util::stream::poll_fn(move |cx| receive.poll_recv(cx))
}

/// The site's stylesheet arrives brotli-compressed, and its best-level
/// form is made once and kept.
#[tokio::test]
async fn the_stylesheet_is_compressed_once_and_kept() {
    let root = tempfile::tempdir().unwrap();
    let config = crate::Config::development(root.path().join("tasks"));
    let css = openagents_ui::stylesheet().as_bytes();
    let (status, headers, first) = fetch(
        crate::router(config.clone()),
        crate::theme::STYLESHEET_PATH,
        Some("gzip, deflate, br, zstd"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert!(
        headers
            .get_all(header::VARY)
            .iter()
            .any(|v| v == "accept-encoding")
    );
    assert!(
        first.len() * 4 < css.len(),
        "{} of {}",
        first.len(),
        css.len()
    );
    assert_eq!(unbrotli(&first), css);
    // From its second time the best-level form is made, kept, and sent.
    let mut kept = Vec::new();
    for _ in 0..2 {
        let (_, _, body) = fetch(
            crate::router(config.clone()),
            crate::theme::STYLESHEET_PATH,
            Some("br"),
        )
        .await;
        assert_eq!(unbrotli(&body), css);
        assert!(body.len() <= first.len());
        kept.push(body);
    }
    assert_eq!(kept[0], kept[1]);
    let key: [u8; 32] = Sha256::digest(css).into();
    let entry = cache().lock().unwrap().entries.get(&key).cloned().unwrap();
    assert_eq!(
        entry.brotli.get().map(|b| b.to_vec()).as_ref(),
        Some(&kept[0])
    );
}

/// A page that compresses its own document keeps its single coding.
#[tokio::test]
async fn self_compressed_pages_are_not_compressed_twice() {
    let root = tempfile::tempdir().unwrap();
    let config = crate::Config::development(root.path().join("tasks"));
    for path in ["/lander10", "/agent/foreman"] {
        let (status, headers, body) = fetch(crate::router(config.clone()), path, Some("br")).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let codings: Vec<_> = headers.get_all(header::CONTENT_ENCODING).iter().collect();
        assert_eq!(codings, ["br"], "{path}");
        let html = String::from_utf8(unbrotli(&body)).unwrap();
        assert!(html.contains("<html"), "{path}");
    }
    // The homepage compresses through the layer.
    let (_, headers, body) = fetch(crate::router(config), "/", Some("br")).await;
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    assert!(
        String::from_utf8(unbrotli(&body))
            .unwrap()
            .contains("<html")
    );
}

#[test]
fn a_strong_validator_becomes_weak() {
    let mut headers = HeaderMap::new();
    headers.insert(header::ETAG, "\"abc\"".parse().unwrap());
    weaken_etag(&mut headers);
    assert_eq!(headers[header::ETAG], "W/\"abc\"");
    weaken_etag(&mut headers);
    assert_eq!(headers[header::ETAG], "W/\"abc\"");
}
