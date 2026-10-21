#![allow(clippy::unwrap_used)]
use super::*;
use crate::projects::tests::{Options, World, world_with};
use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

/// The hosted chat: answers every message at once.
struct Hosted;
impl crate::ask::Chat for Hosted {
    fn door(&self, _: secp256k1::SecretKey) -> Result<Box<dyn Door>, String> {
        Ok(Box::new(Hosted))
    }
}
impl Door for Hosted {
    fn ask(
        &self,
        turns: Vec<Turn>,
        _: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let text = turns.last().unwrap().text.clone();
        Box::pin(async move {
            let mut r = basic_coder::lock(&reply);
            r.text = format!("Answer to {text}");
            r.done = true;
        })
    }
}

/// An in-process relay, with an open pylon `rig` echoing on it when
/// `with_pylon`. The pylon stops when what's returned with it drops.
async fn relay(
    with_pylon: bool,
) -> (
    String,
    Option<(tokio::sync::oneshot::Sender<()>, tempfile::TempDir)>,
) {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    if !with_pylon {
        return (url, None);
    }
    let home = tempfile::tempdir().unwrap();
    let mut config = pylon::provider::Config::new(&url, "rig", home.path().to_path_buf());
    config.label = "Rig".into();
    config.allow = None;
    config.rate_per_minute = 600;
    let provider =
        pylon::provider::Provider::new(config, Identity::generate(), Arc::new(pylon::engine::Echo))
            .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(provider.run(async {
        let _ = stopped.await;
    }));
    for _ in 0..200 {
        if hub.lock().await.stored.iter().any(|e| e.kind == 30_200) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    (url, Some((stop, home)))
}

async fn send(
    world: &World,
    cookie: &str,
    csrf: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
    home: bool,
) -> (u16, axum::http::HeaderMap, Value) {
    send_with(world, cookie, csrf, method, path, body, home, None).await
}

#[allow(clippy::too_many_arguments)]
async fn send_with(
    world: &World,
    cookie: &str,
    csrf: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
    home: bool,
    model: Option<&str>,
) -> (u16, axum::http::HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, &world.host)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("sec-fetch-site", "same-origin");
    if let Some(csrf) = csrf {
        request = request.header(crate::cookie_api::CSRF_HEADER, csrf);
    }
    if home {
        request = request.header("x-openagents-chat-surface", "home");
    }
    if let Some(model) = model {
        request = request.header("x-openagents-pylon-model", model);
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
        Duration::from_secs(30),
        to_bytes(response.into_body(), 1024 * 1024),
    )
    .await
    .unwrap()
    .unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, headers, value)
}

/// A signed-out visitor sends `text`, from the Grid terminal when `home`,
/// and reads the answer's events.
async fn ask(world: &World, n: usize, text: &str, home: bool) -> String {
    ask_for(world, n, text, home, None).await
}

/// As [`ask`], naming `model` in the turn's pylon model header.
async fn ask_for(world: &World, n: usize, text: &str, home: bool, model: Option<&str>) -> String {
    let (status, headers, session) =
        send(world, "", None, "GET", "/lander8/session", None, false).await;
    assert_eq!(status, 200, "{session}");
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let chat = format!("5e1f0c3a-7d2b-4c4e-9a1b-{n:012}");
    let (status, _, started) = send_with(
        world,
        &cookie,
        Some(&csrf),
        "POST",
        "/v1/threads",
        Some(json!({"request_id": chat, "text": text})),
        home,
        model,
    )
    .await;
    assert_eq!(status, 201, "{started}");
    let (status, _, events) = send_with(
        world,
        &cookie,
        None,
        "GET",
        &format!("/v1/threads/{chat}/events?request_id={chat}"),
        None,
        home,
        model,
    )
    .await;
    assert_eq!(status, 200);
    events.as_str().unwrap_or_default().to_owned()
}

/// An in-process relay with an open pylon `rig` and a ready V4.1 model
/// group head `v41-head`, both echoing. They stop when what's returned
/// drops.
async fn relay_with_group() -> (
    String,
    Vec<(tokio::sync::oneshot::Sender<()>, tempfile::TempDir)>,
) {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let mut running = Vec::new();
    for slug in ["rig", "v41-head"] {
        let home = tempfile::tempdir().unwrap();
        let mut config = pylon::provider::Config::new(&url, slug, home.path().to_path_buf());
        config.label = if slug == "rig" { "Rig" } else { "V41 group" }.into();
        config.allow = None;
        config.rate_per_minute = 600;
        if slug == "v41-head" {
            config.slots = 1;
            config.job_timeout = GROUP_DEADLINE;
            let readiness = pylon::group::SharedReadiness::default();
            readiness.set(pylon::group::GroupStatus {
                readiness: nostr::pylon::Readiness::Ready,
                generation: 1,
            });
            config.group = Some(pylon::provider::HeadGroup {
                spec: pylon::group::GroupSpec {
                    id: "v41-a".into(),
                    model: GROUP_MODEL.into(),
                    artifact: format!("sha256:{}", "4c".repeat(32)),
                    deadline_secs: GROUP_DEADLINE.as_secs(),
                    max_output_tokens: 4_096,
                },
                readiness: Arc::new(readiness),
                stages: vec![("s0".into(), nostr::pylon::Layers { from: 0, to: 43 })],
                answers: None,
            });
        }
        let key = Identity::generate();
        let provider =
            pylon::provider::Provider::new(config, key.clone(), Arc::new(pylon::engine::Echo))
                .unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(provider.run(async {
            let _ = stopped.await;
        }));
        for _ in 0..400 {
            let up = hub.lock().await.stored.iter().any(|e| {
                e.kind == 30_200
                    && e.pubkey == key.pubkey()
                    && nostr::pylon::parse_beacon(e).is_ok_and(|b| {
                        b.slots.free > 0
                            && b.group_head()
                                .is_none_or(|g| g.readiness == Some(nostr::pylon::Readiness::Ready))
                    })
            });
            if up {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        running.push((stop, home));
    }
    (url, running)
}

async fn world(relay: String) -> World {
    world_with(Options {
        chat: Some(Arc::new(Hosted)),
        pylons_relay: Some(relay),
        ..Options::default()
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_home_turn_is_answered_by_a_live_pylon() {
    let (relay, _pylon) = relay(true).await;
    let world = world(relay).await;
    let events = ask(&world, 1, "hello", true).await;
    assert!(
        events.contains("**Rig** · echo") && events.contains("echo: olleh"),
        "{events}"
    );
    assert!(!events.contains("Answer to hello"), "{events}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_home_turn_with_no_live_pylon_gets_the_hosted_chat() {
    let (relay, _) = relay(false).await;
    let world = world(relay).await;
    let events = ask(&world, 2, "hello", true).await;
    assert!(events.contains("Answer to hello"), "{events}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_home_turn_that_asks_for_v41_goes_only_to_the_group() {
    let (relay, _running) = relay_with_group().await;
    let world = world(relay).await;
    let events = ask_for(&world, 4, "hello", true, Some(GROUP_MODEL)).await;
    assert!(
        events.contains("**V41 group**") && events.contains("echo: olleh"),
        "{events}"
    );
    // A default turn never reaches the group: the ordinary pylon answers.
    let events = ask(&world, 5, "hello", true).await;
    assert!(
        events.contains("**Rig** · echo") && !events.contains("V41 group"),
        "{events}"
    );
    // Another model name is ignored, and the turn routes as a default one.
    let events = ask_for(&world, 6, "hello", true, Some("gpt-x")).await;
    assert!(events.contains("**Rig** · echo"), "{events}");
}

#[test]
fn only_the_v41_model_header_asks_for_the_group() {
    let mut headers = HeaderMap::new();
    assert!(!asks_for_group(&headers));
    headers.insert(MODEL_HEADER, GROUP_MODEL.parse().unwrap());
    assert!(asks_for_group(&headers));
    headers.insert(MODEL_HEADER, "qwen3.5-0.8b-q8_0".parse().unwrap());
    assert!(!asks_for_group(&headers));
    assert!(GROUP_TURN > GROUP_DEADLINE + pylon::client::GRACE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn other_pages_never_ask_the_pylons() {
    let (relay, _pylon) = relay(true).await;
    let world = world(relay).await;
    let events = ask(&world, 3, "hello", false).await;
    assert!(events.contains("Answer to hello"), "{events}");
    assert!(!events.contains("echo:"), "{events}");
}

#[test]
fn a_pylon_answer_is_shown_under_its_name() {
    assert_eq!(
        shown("Rig *B*", "qwen3.5-0.8b", " Hi. \n"),
        "**Rig B** · qwen3.5-0.8b\n\nHi."
    );
    assert!(oa_copy::violations(&shown("Rig", "echo", "Hi."), &[]).is_empty());
}

#[test]
fn the_log_line_never_holds_the_message() {
    let routed = router::Routed {
        outcome: Outcome::Declined,
        how: How::Decided {
            choice: "default".into(),
            confidence: 0.9,
            door: Some("pylon:clef".into()),
            model: Some("clef-flash".into()),
            ms: 40,
        },
        candidates: 2,
        deciders: 1,
        failed: vec![],
    };
    assert_eq!(
        log_line(&routed, 45),
        "openagents-web: pylon router: handed to the hosted chat; decided default at 0.90 \
by pylon:clef in 40 ms; 2 candidates, 1 deciders, 0 failed, 45 ms"
    );
}

#[test]
fn the_log_line_names_each_failed_pylon_and_why() {
    let routed = router::Routed {
        outcome: Outcome::Unavailable("no pylon answered".into()),
        how: How::Ranked {
            why: "no decision model online".into(),
        },
        candidates: 2,
        deciders: 0,
        failed: vec![
            (
                "30200:aa:mac-net".into(),
                "not_admitted: this pylon serves listed keys only".into(),
            ),
            ("30200:bb:rig".into(), "no answer within\n30 s".into()),
        ],
    };
    assert_eq!(
        log_line(&routed, 900),
        "openagents-web: pylon router: no pylon answered; ranked (no decision model online); \
2 candidates, 0 deciders, 2 failed, 900 ms; mac-net: not_admitted: this pylon serves listed \
keys only; rig: no answer within 30 s"
    );
}

#[test]
fn each_person_sends_as_their_own_buyer_key() {
    let salt = [7; 32];
    assert_eq!(buyer(&salt, "a"), buyer(&salt, "a"));
    assert_ne!(buyer(&salt, "a"), buyer(&salt, "b"));
    assert_ne!(buyer(&salt, "a"), crate::ask::key(&salt, "a"));
}
