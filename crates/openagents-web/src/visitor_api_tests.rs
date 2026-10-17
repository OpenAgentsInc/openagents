use super::*;
use crate::projects::tests::{Browser, CHAT_SALT, Options, world_with};
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use openagents_chat::basic_coder;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

/// A chat worker that answers every message at once.
struct Worker;
impl crate::ask::Chat for Worker {
    fn door(&self, _: secp256k1::SecretKey) -> Result<Box<dyn basic_coder::Door>, String> {
        Ok(Box::new(Worker))
    }
}
impl basic_coder::Door for Worker {
    fn ask(
        &self,
        turns: Vec<basic_coder::Turn>,
        _: openagents_chat::router::Context,
        reply: Arc<std::sync::Mutex<basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let text = turns.last().unwrap().text.clone();
        Box::pin(async move {
            let mut r = basic_coder::lock(&reply);
            r.text = format!("Answer to {text}");
            r.done = true;
        })
    }
}

async fn call(
    world: &crate::projects::tests::World,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, &world.host)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = world
        .site
        .clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(response.into_body(), 1024 * 1024),
    )
    .await
    .unwrap()
    .unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, value)
}

/// A chat worker that ends every answer without a word.
struct Silent;
impl crate::ask::Chat for Silent {
    fn door(&self, _: secp256k1::SecretKey) -> Result<Box<dyn basic_coder::Door>, String> {
        Ok(Box::new(Silent))
    }
}
impl basic_coder::Door for Silent {
    fn ask(
        &self,
        _: Vec<basic_coder::Turn>,
        _: openagents_chat::router::Context,
        _: Arc<std::sync::Mutex<basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}

/// How many times `name` was counted, and with which details.
fn counted(analytics: &crate::analytics::Analytics, name: &str) -> (u64, Vec<String>) {
    let rows: Vec<_> = analytics
        .snapshot()
        .into_iter()
        .filter(|r| r.series == crate::analytics::Series::Event && r.name == name)
        .collect();
    (
        rows.iter().map(|r| r.count).sum(),
        rows.into_iter().map(|r| r.detail).collect(),
    )
}

/// No count carries anything from the requests: every app event's detail
/// is empty or a fixed time bucket, and no row holds the chats' ids, the
/// tokens, or the words sent.
fn nothing_identifying(analytics: &crate::analytics::Analytics, secrets: &[&str]) {
    let buckets = ["", "under_5s", "under_15s", "under_60s", "over_60s"];
    for row in analytics.snapshot() {
        if row.series == crate::analytics::Series::Event && row.name.starts_with("app_") {
            assert!(buckets.contains(&row.detail.as_str()), "{row:?}");
        }
        for secret in secrets {
            assert!(
                !row.name.contains(secret) && !row.detail.contains(secret),
                "{row:?} holds {secret}"
            );
        }
    }
}

fn id(n: usize) -> String {
    format!("5e1f0c3a-7d2b-4c4e-9a1b-{n:012}")
}

/// The chat's messages once its answer is written.
async fn answered(world: &crate::projects::tests::World, token: &str, chat: &str) -> Value {
    for _ in 0..200 {
        let (status, thread) = call(
            world,
            Some(token),
            "GET",
            &format!("/v1/threads/{chat}"),
            None,
        )
        .await;
        assert_eq!(status, 200, "{thread}");
        if thread["messages"]
            .as_array()
            .is_some_and(|m| m.last().is_some_and(|m| m["role"] == "assistant"))
        {
            return thread;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no answer in {chat}");
}

#[tokio::test]
async fn a_visitor_chats_until_the_limit_and_its_chats_move_to_the_account() {
    let analytics = Arc::new(crate::analytics::Analytics::default());
    let world = world_with(Options {
        chat: Some(Arc::new(Worker)),
        analytics: Some(analytics.clone()),
        ..Options::default()
    })
    .await;
    let (status, minted) = call(&world, None, "POST", "/v1/visitors", None).await;
    assert_eq!(status, 201, "{minted}");
    assert_eq!(minted["messages"], MESSAGES);
    let token = minted["token"].as_str().unwrap().to_owned();
    assert!(visitor_of(&CHAT_SALT, &token).is_some());

    // Start a chat and read its answer.
    let chat = id(1);
    let (status, started) = call(
        &world,
        Some(&token),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": chat, "text": "hello"})),
    )
    .await;
    assert_eq!(status, 201, "{started}");
    assert_eq!(started["left"], MESSAGES - 1);
    let thread = answered(&world, &token, &chat).await;
    assert!(
        thread["messages"][1]["text"]
            .as_str()
            .unwrap()
            .contains("Answer to hello")
    );
    // Its first answer is counted with how long it took (#11983).
    assert_eq!(
        counted(&analytics, "app_first_answer"),
        (1, vec!["under_5s".to_owned()])
    );

    // Its answer streams to the visitor, and to no one else.
    let (status, events) = call(
        &world,
        Some(&token),
        "GET",
        &format!("/v1/threads/{chat}/events?request_id={chat}"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let events = events.as_str().unwrap_or_default().to_owned();
    assert!(
        events.contains("Answer to hello") && events.contains("\"done\""),
        "{events}"
    );

    // Words only.
    let (status, refusal) = call(
        &world,
        Some(&token),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": id(2), "text": "x", "project": "name:p"})),
    )
    .await;
    assert_eq!(
        (status, &refusal["error"]["code"]),
        (409, &json!("sign_in_required"))
    );

    // Another visitor, a forged token, and no token read nothing of it.
    let (_, other) = call(&world, None, "POST", "/v1/visitors", None).await;
    let other = other["token"].as_str().unwrap();
    let (status, _) = call(
        &world,
        Some(other),
        "GET",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &world,
        Some(other),
        "GET",
        &format!("/v1/threads/{chat}/events?request_id={chat}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let forged = format!("{PREFIX}{}{}", "0".repeat(32), "0".repeat(64));
    let (status, _) = call(
        &world,
        Some(&forged),
        "GET",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(status, 401);

    // Replies count toward the limit, then sending is refused.
    for n in 2..=MESSAGES {
        let (status, sent) = call(
            &world,
            Some(&token),
            "POST",
            &format!("/v1/threads/{chat}/messages"),
            Some(json!({"request_id": id(100 + n), "text": format!("m{n}")})),
        )
        .await;
        assert_eq!(status, 202, "{sent}");
        assert_eq!(sent["left"], MESSAGES - n);
        answered_after(&world, &token, &chat, n).await;
    }
    let (status, refusal) = call(
        &world,
        Some(&token),
        "POST",
        &format!("/v1/threads/{chat}/messages"),
        Some(json!({"request_id": id(999), "text": "one more"})),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"]["code"], "sign_in_required");
    let (status, _) = call(
        &world,
        Some(&token),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": id(998), "text": "new chat"})),
    )
    .await;
    assert_eq!(status, 409);

    // Signed in, the visitor's chats move to the account.
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let session = alice.0["oa_cloud_session"].clone();
    let (status, claimed) = call(
        &world,
        Some(&session),
        "POST",
        "/v1/visitors/claim",
        Some(json!({"token": token})),
    )
    .await;
    assert_eq!(status, 200, "{claimed}");
    assert_eq!(claimed["moved"], 1);
    // Replies are not first answers; the visitor had reached the limit.
    assert_eq!(counted(&analytics, "app_first_answer").0, 1);
    assert_eq!(counted(&analytics, "app_first_answer_failed").0, 0);
    assert_eq!(counted(&analytics, "app_visitor_claimed").0, 1);
    assert_eq!(
        counted(&analytics, "app_visitor_signed_in_after_limit"),
        (1, vec![String::new()])
    );
    let (status, _) = call(
        &world,
        Some(&token),
        "GET",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (status, list) = call(&world, Some(&session), "GET", "/v1/threads", None).await;
    assert_eq!(status, 200);
    assert!(
        list["threads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == chat.as_str()),
        "{list}"
    );
    // Claiming needs a sign-in and a real token.
    let (status, _) = call(
        &world,
        None,
        "POST",
        "/v1/visitors/claim",
        Some(json!({"token": token})),
    )
    .await;
    assert_eq!(status, 401);
    let (status, _) = call(
        &world,
        Some(&session),
        "POST",
        "/v1/visitors/claim",
        Some(json!({"token": forged})),
    )
    .await;
    assert_eq!(status, 400);

    // A signed-in chat started from the app counts its first answer too.
    let (status, started) = call(
        &world,
        Some(&session),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": id(500), "text": "signed in"})),
    )
    .await;
    assert!(status == 201 || status == 202, "{status} {started}");
    answered(&world, &session, &id(500)).await;
    for _ in 0..200 {
        if counted(&analytics, "app_first_answer").0 == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(counted(&analytics, "app_first_answer").0, 2);
    nothing_identifying(
        &analytics,
        &[
            &chat,
            &id(500),
            &token,
            &session,
            "hello",
            "signed in",
            "octo",
        ],
    );
}

#[tokio::test]
async fn a_failed_first_answer_and_a_claim_before_the_limit_count_as_such() {
    let analytics = Arc::new(crate::analytics::Analytics::default());
    let world = world_with(Options {
        chat: Some(Arc::new(Silent)),
        analytics: Some(analytics.clone()),
        ..Options::default()
    })
    .await;
    let (_, minted) = call(&world, None, "POST", "/v1/visitors", None).await;
    let token = minted["token"].as_str().unwrap().to_owned();
    let chat = id(7);
    let (status, started) = call(
        &world,
        Some(&token),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": chat, "text": "anyone there"})),
    )
    .await;
    assert_eq!(status, 201, "{started}");
    for _ in 0..200 {
        if counted(&analytics, "app_first_answer_failed").0 == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        counted(&analytics, "app_first_answer_failed"),
        (1, vec![String::new()])
    );
    assert_eq!(counted(&analytics, "app_first_answer").0, 0);

    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let session = alice.0["oa_cloud_session"].clone();
    let (status, claimed) = call(
        &world,
        Some(&session),
        "POST",
        "/v1/visitors/claim",
        Some(json!({"token": token})),
    )
    .await;
    assert_eq!((status, &claimed["moved"]), (200, &json!(1)), "{claimed}");
    assert_eq!(counted(&analytics, "app_visitor_claimed").0, 1);
    assert_eq!(
        counted(&analytics, "app_visitor_signed_in_after_limit").0,
        0
    );
    nothing_identifying(&analytics, &[&chat, &token, &session, "anyone"]);
}

/// Wait until the chat holds `n` answers.
async fn answered_after(world: &crate::projects::tests::World, token: &str, chat: &str, n: usize) {
    for _ in 0..200 {
        let (_, thread) = call(
            world,
            Some(token),
            "GET",
            &format!("/v1/threads/{chat}"),
            None,
        )
        .await;
        let answers = thread["messages"]
            .as_array()
            .map_or(0, |m| m.iter().filter(|m| m["role"] == "assistant").count());
        if answers >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no answer {n} in {chat}");
}
