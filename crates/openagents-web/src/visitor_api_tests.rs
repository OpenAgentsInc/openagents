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
    call_surface(world, token, method, path, body, None).await
}

async fn call_surface(
    world: &crate::projects::tests::World,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
    surface: Option<&str>,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, &world.host)
        .header(header::CONTENT_TYPE, "application/json")
        // Each call its own address: the hourly visitor limit is shared by
        // every test in the process, and is checked by its own unit test.
        .header("x-forwarded-for", next_address());
    if let Some(surface) = surface {
        request = request.header("x-openagents-chat-surface", surface);
    }
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
/// [`counted`], once `name` has reached `count` or after about five
/// seconds: an answer's events are counted just after it is saved, so a
/// reader that saw the answer may not see its count yet.
async fn counted_soon(
    analytics: &crate::analytics::Analytics,
    name: &str,
    count: u64,
) -> (u64, Vec<String>) {
    for _ in 0..200 {
        if counted(analytics, name).0 >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    counted(analytics, name)
}

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
    // Its first answer is counted with how long it took (#11983). The
    // count follows the answer's save, so wait for it.
    assert_eq!(
        counted_soon(&analytics, "app_first_answer", 1).await,
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
        // The answer is readable a moment before its task lets go of the
        // chat; under load a send in that moment is refused as busy.
        let mut tries = 0;
        let (status, sent) = loop {
            let reply = call(
                &world,
                Some(&token),
                "POST",
                &format!("/v1/threads/{chat}/messages"),
                Some(json!({"request_id": id(100 + n), "text": format!("m{n}")})),
            )
            .await;
            tries += 1;
            if reply.0 != 409 || tries > 50 {
                break reply;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        };
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
    assert_eq!(counted_soon(&analytics, "app_first_answer", 2).await.0, 2);
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

fn next_address() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("203.0.{}.{}", (n >> 8) & 0xff, n & 0xff)
}

/// One request from the site's own page with the browser's cookies and,
/// for a write, the page's CSRF value (`/lander8`, [`crate::cookie_api`]).
async fn from_page(
    world: &crate::projects::tests::World,
    cookie: &str,
    csrf: Option<&str>,
    site: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, &world.host)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("sec-fetch-site", site)
        .header("x-forwarded-for", next_address());
    if let Some(csrf) = csrf {
        request = request.header(crate::cookie_api::CSRF_HEADER, csrf);
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
    let headers = response.headers().clone();
    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(response.into_body(), 1024 * 1024),
    )
    .await
    .unwrap()
    .unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, headers, value)
}

/// `/lander8` signed out: the page's session read gives the browser a
/// visitor cookie and a CSRF value, and the chat API then answers that
/// browser as an app's signed-out visitor, with the same limits; its chats
/// are the browser's own, as the home page's are.
#[tokio::test]
async fn a_browser_visitor_chats_through_the_api_with_its_cookie() {
    let world = world_with(Options {
        chat: Some(Arc::new(Worker)),
        ..Options::default()
    })
    .await;
    let (status, headers, session) = from_page(
        &world,
        "",
        None,
        "same-origin",
        "GET",
        "/lander8/session",
        None,
    )
    .await;
    assert_eq!(status, 200, "{session}");
    assert_eq!(session["signed_in"], false);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let set = headers[header::SET_COOKIE].to_str().unwrap();
    let cookie = set.split(';').next().unwrap().to_owned();
    let visitor = cookie.strip_prefix("oa_visitor=").unwrap().to_owned();
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    // The same browser reads the same value and keeps its cookie.
    let (_, headers, again) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "GET",
        "/lander8/session",
        None,
    )
    .await;
    assert_eq!(again["csrf"], csrf.as_str());
    assert!(!headers.contains_key(header::SET_COOKIE));

    // A write needs the page's CSRF value, from this site.
    let chat = id(500);
    let start = json!({"request_id": chat, "text": "hello"});
    let (status, _, _) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "POST",
        "/v1/threads",
        Some(start.clone()),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _, _) = from_page(
        &world,
        &cookie,
        Some(&csrf),
        "cross-site",
        "POST",
        "/v1/threads",
        Some(start.clone()),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _, started) = from_page(
        &world,
        &cookie,
        Some(&csrf),
        "same-origin",
        "POST",
        "/v1/threads",
        Some(start),
    )
    .await;
    assert_eq!(status, 201, "{started}");
    assert_eq!(started["left"], MESSAGES - 1);

    // Its answer streams to this browser, which reads the chat back.
    let (status, _, events) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "GET",
        &format!("/v1/threads/{chat}/events?request_id={chat}"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let events = events.as_str().unwrap_or_default();
    assert!(
        events.contains("Answer to hello") && events.contains("\"done\""),
        "{events}"
    );
    let (status, _, thread) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "GET",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{thread}");
    assert_eq!(thread["messages"][0]["text"], "hello");
    // The chat is the browser's own, where the home page finds it.
    let stored = world.store.load(&visitor, &chat).await.unwrap();
    assert!(stored.is_some());

    // Another browser reads nothing of it.
    let other = format!("oa_visitor={}", crate::ask::new_visitor());
    let (status, _, _) = from_page(
        &world,
        &other,
        None,
        "same-origin",
        "GET",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
}

/// The GPUI web chat's sidebar, signed out (#12222): a visitor lists its
/// chats, reads every message, renames, pins, and archives one, can't file
/// it in a project, and deletes it; another visitor touches none of it.
#[tokio::test]
async fn a_visitor_lists_changes_and_deletes_its_own_chats() {
    let world = world_with(Options {
        chat: Some(Arc::new(Worker)),
        ..Options::default()
    })
    .await;
    let (_, minted) = call(&world, None, "POST", "/v1/visitors", None).await;
    let token = minted["token"].as_str().unwrap().to_owned();
    let (_, other) = call(&world, None, "POST", "/v1/visitors", None).await;
    let other = other["token"].as_str().unwrap().to_owned();
    let chat = id(41);
    let (status, _) = call(
        &world,
        Some(&token),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": chat, "text": "first\nsecond"})),
    )
    .await;
    assert_eq!(status, 201);
    answered(&world, &token, &chat).await;

    let (status, list) = call(&world, Some(&token), "GET", "/v1/threads", None).await;
    assert_eq!(status, 200, "{list}");
    assert_eq!(list["threads"][0]["id"], json!(chat));
    let (_, list) = call(&world, Some(&other), "GET", "/v1/threads", None).await;
    assert_eq!(list["threads"], json!([]));

    let path = format!("/v1/threads/{chat}/messages");
    let (status, rows) = call(&world, Some(&token), "GET", &path, None).await;
    assert_eq!(status, 200, "{rows}");
    assert_eq!(rows["data"][0]["content"][0]["text"], "first\nsecond");
    let (status, _) = call(&world, Some(&other), "GET", &path, None).await;
    assert_eq!(status, 404);

    let path = format!("/v1/threads/{chat}");
    for change in [
        json!({"title": "Renamed"}),
        json!({"pinned": true}),
        json!({"pinned": false}),
    ] {
        let (status, body) = call(&world, Some(&token), "PATCH", &path, Some(change)).await;
        assert_eq!(status, 200, "{body}");
    }
    let (_, list) = call(&world, Some(&token), "GET", "/v1/threads", None).await;
    assert_eq!(list["threads"][0]["title"], "Renamed");
    let (status, refusal) = call(
        &world,
        Some(&token),
        "PATCH",
        &path,
        Some(json!({"project": "name:p"})),
    )
    .await;
    assert_eq!(
        (status, &refusal["error"]["code"]),
        (403, &json!("sign_in_required"))
    );
    let (status, _) = call(
        &world,
        Some(&other),
        "PATCH",
        &path,
        Some(json!({"title": "Theirs"})),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(&world, Some(&other), "DELETE", &path, None).await;
    assert_eq!(status, 404);

    let (status, body) = call(
        &world,
        Some(&token),
        "PATCH",
        &path,
        Some(json!({"archived": true})),
    )
    .await;
    assert_eq!((status, &body["archived"]), (200, &json!(true)), "{body}");
    let (_, list) = call(&world, Some(&token), "GET", "/v1/threads", None).await;
    assert_eq!(list["threads"], json!([]));
    let (status, _) = call(&world, Some(&token), "DELETE", &path, None).await;
    assert_eq!(status, 204);
    let (status, _) = call(&world, Some(&token), "GET", &path, None).await;
    assert_eq!(status, 404);
}

/// A signed-in page reaches the rest of the account API with its cookie
/// (#12262): each route answers a same-origin read exactly as it answers
/// the same session as a Bearer, a cross-site read is refused, and a write
/// needs the page's CSRF value.
#[tokio::test]
async fn a_signed_in_page_reads_the_account_api_with_its_cookie() {
    let world = world_with(Options::default()).await;
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let session = alice.0["oa_cloud_session"].clone();
    let cookie = format!("oa_cloud_session={session}");
    let (status, _, page) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "GET",
        "/lander8/session",
        None,
    )
    .await;
    assert_eq!((status, &page["signed_in"]), (200, &json!(true)), "{page}");
    let csrf = page["csrf"].as_str().unwrap().to_owned();
    for path in [
        "/v1/history",
        "/v1/computers",
        "/v1/computer-usage",
        "/v1/agent-definitions",
        "/v1/agent-assignments",
        "/v1/schedules",
        "/v1/labs/agent/kinds",
        "/v1/labs/agent/documents",
        "/v1/voice-agent",
        "/v1/keys",
        "/v1/device/keys",
        "/v1/account",
        "/v1/notify",
        "/v1/land",
        "/v1/traces",
        "/v1/work",
    ] {
        let (bearer, _) = call(&world, Some(&session), "GET", path, None).await;
        let (status, _, body) =
            from_page(&world, &cookie, None, "same-origin", "GET", path, None).await;
        assert_eq!(status, bearer, "{path}: {body}");
        // `/v1/notify` forwards to a service this world doesn't run.
        if path != "/v1/notify" {
            assert_ne!(status, 401, "{path}: {body}");
        }
        let (status, _, body) =
            from_page(&world, &cookie, None, "cross-site", "GET", path, None).await;
        assert_eq!(
            (status, &body["error"]["code"]),
            (403, &json!("cross_site")),
            "{path}"
        );
    }
    // A write: refused without the page's value, taken with it as the
    // Bearer's is.
    let spec = openagents_sdk::agents::AgentSpec {
        name: "Reviewer".into(),
        description: "Reviews changes".into(),
        instructions: "Review the change.".into(),
        runtime: "claude-code".into(),
        model: None,
        access: openagents_sdk::agents::Access::ReadOnly,
        fallbacks: vec![],
        autonomy: None,
    };
    let agent = json!({"request_id": format!("{:064x}", 12262), "spec": spec});
    let (status, _, body) = from_page(
        &world,
        &cookie,
        None,
        "same-origin",
        "POST",
        "/v1/agent-definitions",
        Some(agent.clone()),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    let (status, _, body) = from_page(
        &world,
        &cookie,
        Some("not-the-value"),
        "same-origin",
        "POST",
        "/v1/agent-definitions",
        Some(agent.clone()),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    let (status, _, made) = from_page(
        &world,
        &cookie,
        Some(&csrf),
        "same-origin",
        "POST",
        "/v1/agent-definitions",
        Some(agent.clone()),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let (status, listed) = call(&world, Some(&session), "GET", "/v1/agent-definitions", None).await;
    assert_eq!(status, 200);
    assert!(listed.to_string().contains("Reviewer"), "{listed}");
    // A visitor's cookie reaches only the chat routes.
    let (_, headers, _) = from_page(
        &world,
        "",
        None,
        "same-origin",
        "GET",
        "/lander8/session",
        None,
    )
    .await;
    let visitor = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let (status, _, _) = from_page(
        &world,
        &visitor,
        None,
        "same-origin",
        "GET",
        "/v1/threads",
        None,
    )
    .await;
    assert_eq!(status, 200);
    let (status, _, body) = from_page(
        &world,
        &visitor,
        None,
        "same-origin",
        "GET",
        "/v1/computers",
        None,
    )
    .await;
    assert_eq!(status, 401, "{body}");
}

// The saved answer can become visible just before the worker releases its slot.
async fn home_post(
    world: &crate::projects::tests::World,
    token: &str,
    path: &str,
    body: Value,
) -> (u16, Value) {
    for _ in 0..400 {
        let result = call_surface(
            world,
            Some(token),
            "POST",
            path,
            Some(body.clone()),
            Some("home"),
        )
        .await;
        if result.1["error"]["code"] != "busy" {
            return result;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Previous home answer stayed busy");
}

#[tokio::test]
async fn home_guests_keep_getting_answers_past_message_and_chat_allowances() {
    let world = world_with(Options {
        chat: Some(Arc::new(Worker)),
        ..Options::default()
    })
    .await;
    let (_, minted) = call(&world, None, "POST", "/v1/visitors", None).await;
    let token = minted["token"].as_str().unwrap();
    // More conversations than the old account-conversion allowance.
    for n in 0..=CHATS {
        let chat = id(800 + n);
        let (status, result) = home_post(
            &world,
            token,
            "/v1/threads",
            json!({"request_id":chat,"text":format!("home chat {n}")}),
        )
        .await;
        assert_eq!(status, 201, "{result}");
        assert!(
            result.get("left").is_none(),
            "A free chat must not report an account-conversion allowance"
        );
        answered(&world, token, &chat).await;
    }
    let chat = id(800 + CHATS);
    // Follow-ups keep reaching the worker after the total message allowance.
    for n in 0..=MESSAGES {
        let (status, result) = home_post(
            &world,
            token,
            &format!("/v1/threads/{chat}/messages"),
            json!({"request_id":id(900+n),"text":format!("free follow-up {n}")}),
        )
        .await;
        assert_eq!(status, 202, "{result}");
        answered(&world, token, &chat).await;
    }
    let last = answered(&world, token, &chat).await;
    assert!(
        last["messages"].as_array().unwrap().last().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("Answer to free follow-up")
    );
    let (status, refusal) = call(
        &world,
        Some(token),
        "POST",
        &format!("/v1/threads/{chat}/messages"),
        Some(json!({"request_id":id(999),"text":"app policy"})),
    )
    .await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"]["code"], "sign_in_required");
    let (status, result) = home_post(
        &world,
        token,
        &format!("/v1/threads/{chat}/messages"),
        json!({"request_id":id(1000),"text":"already exhausted, still free at home"}),
    )
    .await;
    assert_eq!(status, 202, "{result}");
    answered(&world, token, &chat).await;
}

#[tokio::test]
async fn signed_in_home_chat_uses_the_existing_free_account_chat() {
    let world = world_with(Options {
        chat: Some(Arc::new(Worker)),
        ..Options::default()
    })
    .await;
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let session = alice.0["oa_cloud_session"].clone();
    let chat = id(1001);
    let (status, result) = call_surface(
        &world,
        Some(&session),
        "POST",
        "/v1/threads",
        Some(json!({"request_id":chat,"text":"signed in at home"})),
        Some("home"),
    )
    .await;
    assert_eq!(status, 201, "{result}");
    answered(&world, &session, &chat).await;
    let (status, result) = home_post(
        &world,
        &session,
        &format!("/v1/threads/{chat}/messages"),
        json!({"request_id":id(1002),"text":"free follow-up"}),
    )
    .await;
    assert_eq!(status, 202, "{result}");
    answered(&world, &session, &chat).await;
}
