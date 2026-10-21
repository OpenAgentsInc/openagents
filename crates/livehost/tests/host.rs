//! The host end to end without a relay: pointers go straight to
//! `Host::receive`.

use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use livehost::host::{Host, HostConfig, TenantConfig};
use livehost::manifest::{HOST_VERSION, Manifest, Route, SCHEMA};
use livehost::pointer;
use livehost::store::Store;
use nostr::domain::RelaySigner;
use tower::ServiceExt;

fn signer(byte: &str) -> RelaySigner {
    RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
}

async fn put(store: &Store, bytes: &[u8]) -> String {
    store.put(bytes).await.unwrap().0
}

async fn publish(
    store: &Store,
    tenant: &str,
    files: &[(&str, &str)],
    routes: Vec<Route>,
) -> String {
    let mut objects = BTreeMap::new();
    for (path, body) in files {
        objects.insert((*path).to_owned(), put(store, body.as_bytes()).await);
    }
    let manifest = Manifest {
        schema: SCHEMA.into(),
        tenant: tenant.into(),
        host_min: HOST_VERSION,
        parent: None,
        objects,
        routes,
        modules: BTreeMap::new(),
        handlers: BTreeMap::new(),
        encodings: BTreeMap::new(),
        meta: None,
    };
    put(store, &manifest.to_bytes()).await
}

async fn get(
    host: &std::sync::Arc<Host>,
    uri: &str,
    host_name: &str,
) -> (StatusCode, String, String) {
    let response = host
        .router()
        .oneshot(
            Request::get(uri)
                .header(header::HOST, host_name)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let manifest = response
        .headers()
        .get("x-livehost-manifest")
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        manifest,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[tokio::test]
async fn tenants_swap_independently_and_refuse_other_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::parse(dir.path().to_str().unwrap()).unwrap();
    let (site, game, stranger) = (signer("11"), signer("22"), signer("33"));
    let page = Route {
        path: "/".into(),
        template: Some("index.html".into()),
        data: Some("content/index.json".into()),
        handler: None,
    };
    let first = publish(
        &store,
        "site",
        &[
            (
                "templates/index.html",
                "<p>{{ text }}</p><link href=\"{{asset static/a.css}}\"></body>",
            ),
            ("content/index.json", r#"{"text": "one"}"#),
            ("static/a.css", "p{}"),
        ],
        vec![page.clone()],
    )
    .await;
    let game_manifest = publish(&store, "game", &[("index.html", "<b>game</b>")], vec![]).await;

    let config = HostConfig {
        tenants: vec![
            TenantConfig {
                name: "site".into(),
                pubkey: site.pubkey().into(),
                channel: "staging".into(),
                hosts: vec!["site.localhost".into()],
            },
            TenantConfig {
                name: "game".into(),
                pubkey: game.pubkey().into(),
                channel: "staging".into(),
                hosts: vec!["game.localhost".into()],
            },
        ],
    };
    let store_for_host = Store::parse(dir.path().to_str().unwrap()).unwrap();
    let host = Host::new(config, store_for_host).unwrap();
    assert_eq!(
        get(&host, "/", "site.localhost").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );

    host.receive(pointer::sign(&site, "site", "staging", &first, None, 10).unwrap())
        .await;
    host.receive(pointer::sign(&game, "game", "staging", &game_manifest, None, 10).unwrap())
        .await;
    let (status, manifest, body) = get(&host, "/", "site.localhost").await;
    assert_eq!(
        (status, manifest.as_str()),
        (StatusCode::OK, first.as_str())
    );
    assert!(body.starts_with("<p>one</p><link href=\"/_o/"));
    assert!(
        body.contains("data-tenant=\"site\""),
        "the live client is injected"
    );
    let (_, _, game_body) = get(&host, "/t/game/", "site.localhost").await;
    assert!(game_body.starts_with("<b>game</b>"), "{game_body}");

    // A pointer signed by another key changes nothing.
    let second = publish(
        &store,
        "site",
        &[
            ("templates/index.html", "<p>{{ text }}</p></body>"),
            ("content/index.json", r#"{"text": "two"}"#),
        ],
        vec![page],
    )
    .await;
    host.receive(pointer::sign(&stranger, "site", "staging", &second, None, 11).unwrap())
        .await;
    assert_eq!(get(&host, "/", "site.localhost").await.1, first);

    // The tenant's key moves it; the game is untouched.
    host.receive(pointer::sign(&site, "site", "staging", &second, Some(&first), 11).unwrap())
        .await;
    let (_, manifest, body) = get(&host, "/", "site.localhost").await;
    assert_eq!(manifest, second);
    assert!(body.starts_with("<p>two</p>"));
    assert_eq!(get(&host, "/", "game.localhost").await.1, game_manifest);

    // An older pointer doesn't win.
    host.receive(pointer::sign(&site, "site", "staging", &first, None, 9).unwrap())
        .await;
    assert_eq!(get(&host, "/", "site.localhost").await.1, second);

    // A manifest of another tenant can't be pointed to.
    host.receive(pointer::sign(&site, "site", "staging", &game_manifest, None, 12).unwrap())
        .await;
    assert_eq!(get(&host, "/", "site.localhost").await.1, second);

    let (status, _, manifest_doc) = get(&host, &format!("/t/site/_manifest/{second}"), "x").await;
    assert_eq!(status, StatusCode::OK);
    assert!(manifest_doc.contains("\"tenant\": \"site\""));
    assert_eq!(
        get(&host, &format!("/t/game/_manifest/{second}"), "x")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

fn host_with_game(dir: &std::path::Path) -> std::sync::Arc<Host> {
    let config = HostConfig {
        tenants: vec![TenantConfig {
            name: "game".into(),
            pubkey: signer("22").pubkey().into(),
            channel: "live".into(),
            hosts: vec!["game.localhost".into()],
        }],
    };
    Host::new(config, Store::parse(dir.to_str().unwrap()).unwrap()).unwrap()
}

#[tokio::test]
async fn a_page_that_updates_itself_is_never_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::parse(dir.path().to_str().unwrap()).unwrap();
    let manifest = publish(
        &store,
        "game",
        &[(
            "index.html",
            "<html data-livehost-self><body>game</body></html>",
        )],
        vec![],
    )
    .await;
    let host = host_with_game(dir.path());
    host.receive(pointer::sign(&signer("22"), "game", "live", &manifest, None, 10).unwrap())
        .await;
    let (status, _, body) = get(&host, "/t/game/", "127.0.0.1").await;
    assert_eq!(status, StatusCode::OK);
    // The page still gets the client, and the client checks the attribute
    // before it would reload.
    let opt_out = body.find("hasAttribute(\"data-livehost-self\")").unwrap();
    let reload = body.find("location.reload()").unwrap();
    assert!(opt_out < reload);
    let guard = &body[opt_out..reload];
    assert!(guard.contains("current = update.manifest;") && guard.contains("return;"));
}

#[tokio::test]
async fn acks_pass_their_fields_through_up_to_a_cap() {
    use futures_util::StreamExt;

    let dir = tempfile::tempdir().unwrap();
    let host = host_with_game(dir.path());
    let stream = host
        .router()
        .oneshot(Request::get("/_livehost/acks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let mut events = stream.into_body().into_data_stream();
    let post = |body: String| {
        host.router().oneshot(
            Request::post("/_livehost/ack")
                .header(header::HOST, "game.localhost")
                .body(Body::from(body))
                .unwrap(),
        )
    };
    let sent = r#"{"tenant":"game","manifest":"m1","kind":"swap","tick":42,"bytes":1234}"#;
    assert_eq!(
        post(sent.into()).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    let event = String::from_utf8(events.next().await.unwrap().unwrap().to_vec()).unwrap();
    let data = event
        .lines()
        .find_map(|line| line.strip_prefix("data:"))
        .unwrap();
    let ack: serde_json::Value = serde_json::from_str(data.trim()).unwrap();
    assert_eq!(
        (&ack["kind"], &ack["tick"], &ack["bytes"]),
        (&"swap".into(), &42.into(), &1234.into())
    );
    assert!(ack["at_ms"].is_number());

    let big = format!(
        r#"{{"tenant":"game","pad":"{}"}}"#,
        "x".repeat(livehost::host::ACK_MAX_BYTES)
    );
    assert_eq!(
        post(big).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
