//! The phone's account API (#11107, #11165) over the fake account service:
//! the app token picks the account, and each account sees only its own.

use super::*;
use crate::chat_store::{Conversation, Store, account_owner};
use serde_json::Value;

async fn call(
    site: &Router,
    method: Method,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST);
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let response = site
        .clone()
        .oneshot(
            request
                .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn with_store() -> (Fixture, Arc<Store>) {
    let mut store = None;
    let fixture = fixture_with(|config| store = Some(config.chat_store.clone())).await;
    (fixture, store.unwrap())
}

fn web_chat(owner: &str, id: &str) -> Conversation {
    Conversation {
        links: Default::default(),
        id: id.into(),
        owner: owner.into(),
        revision: 1,
        title: "Web chat".into(),
        messages: Vec::new(),
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: now(),
        pinned_unix: None,
        archived_unix: None,
        project: None,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
        filed: false,
    }
}

async fn terminal_chat(store: &Store) -> String {
    let upload: crate::coder_sync::Upload = serde_json::from_value(json!({
        "computer": "Studio", "title": "Fix it",
        "messages": [
            {"role": "user", "text": "Fix it"},
            {"role": "thinking", "text": "I'll run the build first."},
        ]
    }))
    .unwrap();
    match crate::coder_sync::save(store, &account_owner("alice"), "s1", &upload)
        .await
        .unwrap()
    {
        crate::coder_sync::Saved::Saved { chat, .. } => chat,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn no_token_is_signed_out_and_another_account_sees_nothing() {
    let (fixture, store) = with_store().await;
    let site = &fixture.site;
    for path in ["/v1/threads", "/v1/agents", "/v1/computers"] {
        let (status, body) = call(site, Method::GET, path, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(body["error"]["code"], "signed_out");
    }
    let chat = terminal_chat(&store).await;
    let alice = token("alice");
    let bob = token("bob");
    let (status, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["threads"][0]["id"], chat.as_str());
    assert_eq!(body["threads"][0]["line"], "Terminal · Studio");
    // The list counts its messages and names its project (#11543).
    assert_eq!(body["threads"][0]["messages"], 2);
    assert_eq!(body["threads"][0]["project"], Value::Null);
    let (status, body) = call(site, Method::GET, "/v1/threads", Some(&bob), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["threads"], json!([]));
    let path = format!("/v1/threads/{chat}");
    let (status, _) = call(site, Method::GET, &path, Some(&bob), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(site, Method::GET, &path, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["messages"][0]["text"], "Fix it");
    // The model's thinking stays thinking (#11543).
    assert_eq!(body["messages"][1]["role"], "thinking");
    let reply = json!({"request_id": "00000000-0000-4000-8000-000000000001", "text": "Go on"});
    let (status, _) = call(
        site,
        Method::POST,
        &format!("{path}/messages"),
        Some(&bob),
        Some(reply),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_terminal_reply_waits_for_an_online_computer() {
    let (fixture, store) = with_store().await;
    let chat = terminal_chat(&store).await;
    let alice = token("alice");
    let path = format!("/v1/threads/{chat}/messages");
    let reply = json!({"request_id": "00000000-0000-4000-8000-000000000002", "text": "Go on"});
    let (status, body) = call(
        &fixture.site,
        Method::POST,
        &path,
        Some(&alice),
        Some(reply.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "offline");
    crate::coder_sync::check_in(&store, &account_owner("alice"), "Studio")
        .await
        .unwrap()
        .unwrap();
    let (status, body) = call(
        &fixture.site,
        Method::POST,
        &path,
        Some(&alice),
        Some(reply),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["queued"], true);
    let (_, body) = call(
        &fixture.site,
        Method::GET,
        &format!("/v1/threads/{chat}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(body["waiting"], 1);
}

#[tokio::test]
async fn a_web_reply_is_taken_once() {
    let (fixture, store) = with_store().await;
    let id = "1a2b3c4d-1111-4222-8333-444455556666";
    store
        .create(&web_chat(&account_owner("alice"), id))
        .await
        .unwrap();
    let alice = token("alice");
    let path = format!("/v1/threads/{id}/messages");
    let reply = json!({"request_id": "5a2b3c4d-1111-4222-8333-444455556666", "text": "Hello"});
    for _ in 0..2 {
        let (status, body) = call(
            &fixture.site,
            Method::POST,
            &path,
            Some(&alice),
            Some(reply.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        assert_eq!(body["answering"], true);
    }
    let chat = store
        .load(&account_owner("alice"), id)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    assert_eq!(chat.requests.len(), 1);
    assert_eq!(
        chat.messages.iter().filter(|m| m.text == "Hello").count(),
        1
    );
}

#[tokio::test]
async fn an_action_reaches_coder_once_and_an_unknown_item_is_refused() {
    let (fixture, _store) = with_store().await;
    let alice = token("alice");
    let site = &fixture.site;
    let items = json!({"items": [{
        "id": "s1", "kind": "chat", "title": "Fix it", "status": "asking",
        "started_unix": 1, "question": {"id": "7", "text": "Run cargo clean?"}
    }]});
    let (status, body) = call(
        site,
        Method::POST,
        "/v1/computers/Studio/activity",
        Some(&alice),
        Some(items.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["commands"], json!([]));
    let (_, body) = call(site, Method::GET, "/v1/agents", Some(&alice), None).await;
    assert_eq!(body["computers"][0]["name"], "Studio");
    assert_eq!(body["computers"][0]["online"], true);
    assert_eq!(body["computers"][0]["items"][0]["question"]["id"], "7");
    let (_, body) = call(site, Method::GET, "/v1/agents", Some(&token("bob")), None).await;
    assert_eq!(body["computers"], json!([]));
    let act = |item: &str| {
        json!({"request_id": "00000000-0000-4000-8000-0000000000a1", "computer": "Studio", "item": item,
               "action": "approve", "question": "7"})
    };
    let (status, _) = call(
        site,
        Method::POST,
        "/v1/agents/actions",
        Some(&alice),
        Some(act("gone")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        site,
        Method::POST,
        "/v1/agents/actions",
        Some(&token("bob")),
        Some(act("s1")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        site,
        Method::POST,
        "/v1/agents/actions",
        Some(&alice),
        Some(act("s1")),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (_, body) = call(
        site,
        Method::POST,
        "/v1/computers/Studio/activity",
        Some(&alice),
        Some(items.clone()),
    )
    .await;
    assert_eq!(body["commands"][0]["action"], "approve");
    assert_eq!(body["commands"][0]["question"], "7");
    let (_, body) = call(
        site,
        Method::POST,
        "/v1/computers/Studio/activity",
        Some(&alice),
        Some(items),
    )
    .await;
    assert_eq!(body["commands"], json!([]));
}

#[tokio::test]
async fn a_phone_keeps_its_sync_choice() {
    let (fixture, _store) = with_store().await;
    let alice = token("alice");
    let path = "/v1/computers/My%20iPhone/sync";
    let (_, body) = call(&fixture.site, Method::GET, path, Some(&alice), None).await;
    assert_eq!(body["choice"], Value::Null);
    let (status, _) = call(
        &fixture.site,
        Method::PUT,
        path,
        Some(&alice),
        Some(json!({"choice": "all"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = call(&fixture.site, Method::GET, path, Some(&alice), None).await;
    assert_eq!(body["choice"], "all");
    let (_, body) = call(
        &fixture.site,
        Method::GET,
        "/v1/computers",
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(body["computers"][0]["name"], "My iPhone");
}

/// A raw upload, as the apps send a file (#11174).
async fn upload(site: &Router, path: &str, bearer: &str, bytes: &[u8]) -> (StatusCode, Value) {
    let response = site
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(header::HOST, HOST)
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(bytes.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn an_app_sends_a_file_with_a_web_reply() {
    let (fixture, store) = with_store().await;
    let site = &fixture.site;
    let id = "2a2b3c4d-1111-4222-8333-444455556666";
    store
        .create(&web_chat(&account_owner("alice"), id))
        .await
        .unwrap();
    let alice = token("alice");
    let files = format!("/v1/threads/{id}/files?name=notes.md");
    let (status, file) = upload(site, &files, &alice, b"# Notes\n").await;
    assert_eq!(status, StatusCode::CREATED, "{file}");
    assert_eq!(file["kind"], "text");
    let file_id = file["id"].as_str().unwrap().to_owned();
    // Another account can't read it.
    let read = format!("/v1/threads/{id}/files/{file_id}");
    let (status, _) = call(site, Method::GET, &read, Some(&token("bob")), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A second file, removed before sending.
    let (_, other) = upload(site, &files, &alice, b"other").await;
    let other = format!("/v1/threads/{id}/files/{}", other["id"].as_str().unwrap());
    let (status, _) = call(site, Method::DELETE, &other, Some(&alice), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let reply = json!({
        "request_id": "6a2b3c4d-1111-4222-8333-444455556666",
        "text": "What do my notes say?",
        "files": [file_id],
    });
    let messages = format!("/v1/threads/{id}/messages");
    let (status, body) = call(site, Method::POST, &messages, Some(&alice), Some(reply)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let chat = store
        .load(&account_owner("alice"), id)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    assert_eq!(chat.requests[0].files.len(), 1);
    assert_eq!(chat.requests[0].files[0].name, "notes.md");
    // A sent file stays.
    let (status, _) = call(site, Method::DELETE, &read, Some(&alice), None).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // A Coder chat takes words only.
    let terminal = terminal_chat(&store).await;
    let (status, body) = upload(
        site,
        &format!("/v1/threads/{terminal}/files?name=a.txt"),
        &alice,
        b"a",
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

/// `call` as a test run (#11601): with `X-OpenAgents-Test: 1`.
async fn marked_call(
    site: &Router,
    method: Method,
    path: &str,
    bearer: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = site
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, HOST)
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(crate::test_runs::HEADER, "1")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// #11601: `DELETE /v1/threads/{id}` deletes the owner's chat for good;
/// another account can't, and a second delete finds nothing.
#[tokio::test]
async fn a_chat_is_deleted_by_its_owner_only() {
    let (fixture, store) = with_store().await;
    let site = &fixture.site;
    let id = "00000000-0000-4000-8000-0000000000d1";
    store
        .create(&web_chat(&account_owner("alice"), id))
        .await
        .unwrap();
    let path = format!("/v1/threads/{id}");
    let (status, _) = call(site, Method::DELETE, &path, Some(&token("bob")), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let alice = token("alice");
    let (status, _) = call(site, Method::DELETE, &path, Some(&alice), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(body["threads"], json!([]));
    assert!(
        store
            .load(&account_owner("alice"), id)
            .await
            .unwrap()
            .is_none()
    );
    let (status, _) = call(site, Method::DELETE, &path, Some(&alice), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A synced chat is deleted too (Coder deletes its copy when it checks).
    let chat = terminal_chat(&store).await;
    let path = format!("/v1/threads/{chat}");
    let (status, _) = call(site, Method::DELETE, &path, Some(&alice), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(body["threads"], json!([]));
}

/// #11601: `DELETE /v1/computers/{name}` removes the computer's check-ins,
/// sync choice, and running work; its synced chats stay.
#[tokio::test]
async fn a_computer_is_removed_and_its_chats_stay() {
    let (fixture, store) = with_store().await;
    let site = &fixture.site;
    let alice = token("alice");
    let owner = account_owner("alice");
    let chat = terminal_chat(&store).await;
    crate::coder_sync::check_in(&store, &owner, "Studio")
        .await
        .unwrap()
        .unwrap();
    crate::coder_sync::check_in(&store, &owner, "Laptop")
        .await
        .unwrap()
        .unwrap();
    crate::coder_sync::choose(&store, &owner, "Studio", crate::chat_store::SyncChoice::All)
        .await
        .unwrap()
        .unwrap();
    crate::phone_api::report(&store, &owner, "Studio", Vec::new())
        .await
        .unwrap();
    let (status, _) = call(
        site,
        Method::DELETE,
        "/v1/computers/Studio",
        Some(&token("bob")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        site,
        Method::DELETE,
        "/v1/computers/Studio",
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = call(site, Method::GET, "/v1/computers", Some(&alice), None).await;
    let names: Vec<&str> = body["computers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Laptop"]);
    let agents = crate::phone_api::read_agents(&store, &owner).await.unwrap();
    assert!(!agents.boards.contains_key("Studio"));
    let (_, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(body["threads"][0]["id"], chat.as_str());
    let (status, _) = call(
        site,
        Method::DELETE,
        "/v1/computers/Studio",
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A name with a space, as the apps send it.
    crate::coder_sync::check_in(&store, &owner, "OA Phone 59 Gate")
        .await
        .unwrap()
        .unwrap();
    let (status, _) = call(
        site,
        Method::DELETE,
        "/v1/computers/OA%20Phone%2059%20Gate",
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// #11601: a test run's chats never list and go within the hour; its
/// computer is never recorded.
#[tokio::test]
async fn a_test_run_leaves_nothing_listed() {
    let (fixture, store) = with_store().await;
    let site = &fixture.site;
    let alice = token("alice");
    let owner = account_owner("alice");
    let (status, _) = marked_call(
        site,
        Method::POST,
        "/v1/computers/check-in",
        &alice,
        json!({"computer": "harness-acceptance-gate"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = marked_call(
        site,
        Method::POST,
        "/v1/computers/harness-acceptance-gate/activity",
        &alice,
        json!({"items": []}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = marked_call(
        site,
        Method::PUT,
        "/v1/computers/harness-acceptance-gate/sync",
        &alice,
        json!({"choice": "all"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = marked_call(
        site,
        Method::PUT,
        "/coder/sessions/gate-s1",
        &alice,
        json!({"computer": "harness-acceptance-gate", "title": "who are you?",
               "messages": [{"role": "user", "text": "who are you?"}]}),
    )
    .await;
    assert!(status.is_success(), "{status} {body}");
    let (_, body) = call(site, Method::GET, "/v1/computers", Some(&alice), None).await;
    assert_eq!(body["computers"], json!([]));
    assert!(
        crate::phone_api::read_agents(&store, &owner)
            .await
            .unwrap()
            .boards
            .is_empty()
    );
    let (_, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(body["threads"], json!([]));
    // It is still there to read back, until the sweep.
    let made = store.list_with_deleted(&owner).await.unwrap();
    assert_eq!(made.len(), 1);
    let id = made[0].id.clone();
    let (status, _) = call(
        site,
        Method::GET,
        &format!("/v1/threads/{id}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // A person's own chat is never swept.
    let mine = "00000000-0000-4000-8000-0000000000d2";
    store.create(&web_chat(&owner, mine)).await.unwrap();
    assert_eq!(crate::test_runs::sweep(&store, now()).await.unwrap(), 0);
    assert_eq!(
        crate::test_runs::sweep(&store, now() + crate::test_runs::KEEP_SECONDS)
            .await
            .unwrap(),
        1
    );
    assert!(store.load(&owner, &id).await.unwrap().is_none());
    let (_, body) = call(site, Method::GET, "/v1/threads", Some(&alice), None).await;
    assert_eq!(body["threads"][0]["id"], mine);
    assert_eq!(body["threads"].as_array().unwrap().len(), 1);
}
