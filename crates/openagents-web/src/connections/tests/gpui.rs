//! Browser-cookie connections and the GPUI Settings addresses.

use super::*;
use axum::body::Body;
use axum::http::{Request, header};

fn csrf(html: &str) -> String {
    html.match_indices(r#"name="csrf" value=""#)
        .map(|(at, _)| &html[at + 19..])
        .find_map(|rest| {
            let end = rest.find('"')?;
            rest[end..]
                .starts_with(r#"" form="chat-form""#)
                .then(|| rest[..end].to_owned())
        })
        .expect("chat CSRF")
}

#[tokio::test]
async fn gpui_settings_routes_and_cookie_google_connect_remove() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY]).await;
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let build = dir.path().join("gpui");
    std::fs::create_dir(&build).unwrap();
    for file in [
        "openagents_web_chat.js",
        "openagents_web_chat_bg.wasm",
        "start.js",
    ] {
        std::fs::write(build.join(file), b"fixture").unwrap();
    }
    let world = world_with(Options {
        gpui_chat: Some(build),
        connections: Some((
            store.clone(),
            Arc::new(Google::new(
                Client::new("cid.apps".into(), "client-secret".into()),
                google.endpoints(),
            )),
        )),
        ..Options::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    store
        .update_access(|a| {
            a.add(AccessEntry {
                account: Some(account_id_of(&world, "octo-local")),
                ..Default::default()
            })
        })
        .unwrap()
        .unwrap();
    for path in [
        "/settings/connections",
        "/settings/vault",
        "/settings/terminal",
    ] {
        let page = browser.get(&world, &format!("{path}?ui=gpui")).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        assert!(page.body.contains("/gpui/build/start.js"), "{path}");
        assert!(
            page.body.contains(&format!(
                "name=\"openagents-settings\" content=\"{}\"",
                path.rsplit('/').next().unwrap()
            )),
            "standalone module for {path}"
        );
        assert!(
            page.body.contains(&format!("{path}?ui=html")),
            "fallback for {path}"
        );
        let fallback = browser.get(&world, &format!("{path}?ui=html")).await;
        assert_eq!(fallback.status, StatusCode::OK);
        assert!(!fallback.body.contains("/gpui/build/start.js"));
    }
    let terminal = browser
        .get(
            &world,
            "/settings/terminal?computer=Scratch&since=123&ui=gpui",
        )
        .await;
    assert!(
        terminal
            .body
            .contains("/settings/terminal?computer=Scratch&amp;since=123&amp;ui=html")
    );
    let connections = browser
        .get(&world, "/settings/connections?problem=denied&ui=gpui")
        .await;
    assert!(
        connections
            .body
            .contains("/settings/connections?problem=denied&amp;ui=html")
    );
    let page = browser.get(&world, "/new?ui=html").await;
    let csrf = csrf(&page.body);
    let request = |method, path| {
        Request::builder()
            .method(method)
            .uri(path)
            .header("sec-fetch-site", "same-origin")
            .header(header::ORIGIN, world.origin())
            .header(crate::cookie_api::CSRF_HEADER, &csrf)
            .header(header::CONTENT_TYPE, "application/json")
    };
    let start = browser
        .send(
            &world,
            request("POST", "/v1/connections/google/connect"),
            Body::from(r#"{"return_to":"/settings/connections?ui=gpui"}"#),
        )
        .await;
    assert_eq!(start.status, StatusCode::OK, "{}", start.body);
    let start: Value = serde_json::from_str(&start.body).unwrap();
    assert!(start["url"].as_str().unwrap().contains("return_to="));
    connect_google(&mut browser, &world).await;
    let listed = browser
        .send(&world, request("GET", "/v1/connections"), Body::empty())
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    assert_eq!(
        serde_json::from_str::<Value>(&listed.body).unwrap()["google"]["status"],
        "connected"
    );
    assert!(!listed.body.contains(fake::REFRESH));
    let removed = browser
        .send(
            &world,
            request("DELETE", "/v1/connections/google"),
            Body::empty(),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    let listed = browser
        .send(&world, request("GET", "/v1/connections"), Body::empty())
        .await;
    assert_eq!(
        serde_json::from_str::<Value>(&listed.body).unwrap()["google"]["status"],
        "not_connected"
    );
}
