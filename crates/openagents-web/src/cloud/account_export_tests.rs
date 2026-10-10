//! `coder export --account` (#11134) over the fake account service: the
//! app's own token downloads the same file Settings does, for its own
//! account only, with no token in it; a token that no longer signs in is
//! refused in the app API's words.

use super::*;
use crate::chat_store::{Conversation, Store, account_owner};
use serde_json::Value;

async fn export(site: &Router, bearer: Option<&str>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(crate::account_export::PATH)
        .header(header::HOST, HOST);
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    let response = site
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

fn chat(owner: &str, id: &str, text: &str) -> Conversation {
    serde_json::from_value(json!({
        "id": id,
        "owner": owner,
        "revision": 1,
        "title": "A chat",
        "messages": [{"role": "user", "text": text}],
        "pending": null,
        "requests": [],
        "updated_unix": now(),
    }))
    .unwrap()
}

async fn with_chats() -> (Fixture, Arc<Store>) {
    let mut store = None;
    let fixture = fixture_with(|config| store = Some(config.chat_store.clone())).await;
    let store = store.unwrap();
    store
        .create(&chat(
            &account_owner("alice"),
            "11111111-1111-4111-8111-111111111111",
            "alice asks about the build",
        ))
        .await
        .unwrap();
    store
        .create(&chat(
            &account_owner("bob"),
            "22222222-2222-4222-8222-222222222222",
            "bob's private plan",
        ))
        .await
        .unwrap();
    (fixture, store)
}

#[tokio::test]
async fn the_app_token_downloads_its_own_accounts_file() {
    let (fixture, _store) = with_chats().await;
    let alice = token("alice");
    let (status, headers, bytes) = export(&fixture.site, Some(&alice)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "application/json; charset=utf-8"
    );
    let disposition = headers[header::CONTENT_DISPOSITION].to_str().unwrap();
    assert!(
        disposition.starts_with("attachment; filename=\"openagents-export-"),
        "{disposition}"
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store, private");
    let text = String::from_utf8(bytes).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["schema"], "openagents.account-export.v1");
    assert_eq!(file["account"]["name"], "alice <account>");
    // The account's own workspace is the one the file was made in.
    assert_eq!(file["account"]["workspace"]["name"], "Alice personal");
    assert!(text.contains("alice asks about the build"), "{text}");
    assert!(!text.contains("bob's private plan"), "{text}");
    // No token or credential of any kind travels in the file.
    for secret in [token("alice"), token("bob"), credential("alice")] {
        assert!(!text.contains(&secret));
    }
    assert!(!text.contains(CANARY));
    assert!(!text.contains("native-session-alice"));

    // Bob's token gets Bob's file.
    let (status, _, bytes) = export(&fixture.site, Some(&token("bob"))).await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("bob's private plan"));
    assert!(!text.contains("alice asks about the build"));
}

#[tokio::test]
async fn a_token_that_no_longer_signs_in_is_refused_as_json() {
    let (fixture, _store) = with_chats().await;
    // Not a session the account service knows.
    let stranger = format!("sess_{}", "c".repeat(64));
    let (status, _, bytes) = export(&fixture.site, Some(&stranger)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "signed_out");
    assert_eq!(body["error"]["message"], "Sign in again with coder login.");
    // Not a session token at all.
    let (status, _, _) = export(&fixture.site, Some("oak_not-a-session")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Signed out on the account service.
    fixture.state.lock().unwrap().revoked.insert("alice".into());
    let (status, _, bytes) = export(&fixture.site, Some(&token("alice"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!String::from_utf8_lossy(&bytes).contains("alice asks"));
}

#[tokio::test]
async fn without_a_token_the_browser_is_sent_to_sign_in() {
    let (fixture, _store) = with_chats().await;
    let (status, headers, bytes) = export(&fixture.site, None).await;
    assert!(status.is_redirection(), "{status}");
    assert!(headers.contains_key(header::LOCATION));
    assert!(!String::from_utf8_lossy(&bytes).contains("alice asks"));
}
