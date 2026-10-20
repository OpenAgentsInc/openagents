use crate::projects::tests::{Browser, Options, world_with};
use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use coder_access::room_token::{self, Room};
use serde_json::{Value, json};
use tower::ServiceExt;

const KEY: &str = "0505050505050505050505050505050505050505050505050505050505050505";

async fn ask(
    world: &crate::projects::tests::World,
    cookie: &str,
    csrf: Option<&str>,
    site: &str,
    body: Value,
) -> (u16, Value) {
    let mut request = Request::post(super::PATH)
        .header(header::HOST, &world.host)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("sec-fetch-site", site);
    if let Some(csrf) = csrf {
        request = request.header(crate::cookie_api::CSRF_HEADER, csrf);
    }
    let response = world
        .site
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn csrf(world: &crate::projects::tests::World, cookie: &str) -> String {
    let response = world
        .site
        .clone()
        .oneshot(
            Request::get("/lander8/session")
                .header(header::HOST, &world.host)
                .header(header::COOKIE, cookie)
                .header("sec-fetch-site", "same-origin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    value["csrf"].as_str().unwrap().to_owned()
}

/// A signed-in page gets a token for its own registry room and for a
/// device or chat room, each scoped to that room and a few minutes, that
/// `coder-sync` accepts with this site's public key.
#[tokio::test]
async fn a_signed_in_page_gets_a_token_for_one_room() {
    let world = world_with(Options {
        room_token_key: Some(KEY.into()),
        ..Options::default()
    })
    .await;
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let cookie = format!("oa_cloud_session={}", alice.0["oa_cloud_session"]);
    let csrf = csrf(&world, &cookie).await;
    let keys = vec![room_token::public_key(KEY).unwrap()];

    let (status, registry) = ask(
        &world,
        &cookie,
        Some(&csrf),
        "same-origin",
        json!({"room": "registry"}),
    )
    .await;
    assert_eq!(status, 200, "{registry}");
    let token = registry["token"].as_str().unwrap();
    let now = crate::chat_store::now_unix();
    let admitted = room_token::verify(token, &keys, now).unwrap();
    assert_eq!(admitted.room, Room::Registry);
    assert_eq!(admitted.id, admitted.account);
    assert_eq!(registry["id"], admitted.account.as_str());
    assert!(admitted.expires <= now + room_token::LIFETIME);
    assert_eq!(
        registry["url"],
        format!(
            "wss://sync.openagents.com/registry/{}/ws?token={token}",
            admitted.account
        )
    );
    // It is checked as a token for that room only.
    assert!(
        room_token::verify(
            token,
            &[room_token::public_key(&"06".repeat(32)).unwrap()],
            now
        )
        .is_err()
    );

    let (status, device) = ask(
        &world,
        &cookie,
        Some(&csrf),
        "same-origin",
        json!({"room": "device", "id": "mac-studio"}),
    )
    .await;
    assert_eq!(status, 200, "{device}");
    let admitted = room_token::verify(device["token"].as_str().unwrap(), &keys, now).unwrap();
    assert_eq!(
        (admitted.room, admitted.id.as_str()),
        (Room::Device, "mac-studio")
    );

    // Another account's registry, a missing id, a cross-site page, and a
    // write without the page's value are refused.
    for (body, want) in [
        (json!({"room": "registry", "id": "someone-else"}), 403),
        (json!({"room": "device"}), 400),
        (json!({"room": "terminal", "id": "x"}), 400),
        (json!({"room": "device", "id": "a/b"}), 400),
    ] {
        let (status, answer) = ask(&world, &cookie, Some(&csrf), "same-origin", body).await;
        assert_eq!(status, want, "{answer}");
    }
    let (status, _) = ask(
        &world,
        &cookie,
        Some(&csrf),
        "cross-site",
        json!({"room": "registry"}),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = ask(
        &world,
        &cookie,
        None,
        "same-origin",
        json!({"room": "registry"}),
    )
    .await;
    assert_eq!(status, 403);
    // Signed out, nothing.
    let (status, _) = ask(&world, "", None, "same-origin", json!({"room": "registry"})).await;
    assert_eq!(status, 401);
}

/// Without a key, the site makes no room tokens and its pages don't open
/// the sync server.
#[tokio::test]
async fn without_a_key_there_are_no_room_tokens() {
    let world = world_with(Options::default()).await;
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let cookie = format!("oa_cloud_session={}", alice.0["oa_cloud_session"]);
    let csrf = csrf(&world, &cookie).await;
    let (status, _) = ask(
        &world,
        &cookie,
        Some(&csrf),
        "same-origin",
        json!({"room": "registry"}),
    )
    .await;
    assert_eq!(status, 404);
}

#[test]
fn checkpoint_paths_name_one_chat() {
    assert_eq!(
        super::checkpoint_chat("/v1/sync/chat2/0b3f6a52-8f0c/checkpoint"),
        Some("0b3f6a52-8f0c")
    );
    assert_eq!(super::checkpoint_chat("/v1/sync/chat2//checkpoint"), None);
    assert_eq!(
        super::checkpoint_chat("/v1/sync/chat2/a/b/checkpoint"),
        None
    );
    assert_eq!(
        super::checkpoint_chat("/v1/sync/chat2/a%2f/checkpoint"),
        None
    );
    assert!(super::owns("/v1/sync/chat2/abc/checkpoint"));
    assert!(!super::owns("/v1/sync/chat2/abc/rows"));
}
