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
