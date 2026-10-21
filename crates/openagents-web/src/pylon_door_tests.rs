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
        running.push(start_pylon(&url, &hub, slug, Arc::new(pylon::engine::Echo)).await);
    }
    (url, running)
}

/// Starts pylon `slug` on `engine` at the relay `url`, and waits for its
/// beacon. `v41-head` heads a ready V4.1 model group with one slot.
async fn start_pylon(
    url: &str,
    hub: &Arc<tokio::sync::Mutex<pylon::fixture::Hub>>,
    slug: &str,
    engine: Arc<dyn pylon::engine::Engine>,
) -> (tokio::sync::oneshot::Sender<()>, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let mut config = pylon::provider::Config::new(url, slug, home.path().to_path_buf());
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
    let provider = pylon::provider::Provider::new(config, key.clone(), engine).unwrap();
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
    (stop, home)
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
    // Named once, as the group, never as the head's own label.
    assert!(
        events.contains("**DeepSeek V4.1** · group v41-a")
            && events.contains("echo: olleh")
            && !events.contains("V41 group"),
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

/// A pylon whose small model answers with its last answer whenever the
/// conversation holds one, as the owner's 0.8B model did (#12421), and
/// otherwise names the message it answers. It records what it was sent.
struct Parrot(Arc<Mutex<Vec<Vec<pylon::engine::Turn>>>>);

impl pylon::engine::Engine for Parrot {
    fn model(&self) -> &str {
        "qwen3.5-0.8b-q8_0"
    }

    fn healthy(&self) -> pylon::engine::Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [pylon::engine::Turn],
        _: u32,
    ) -> pylon::engine::Pending<'a, Result<pylon::engine::Generation, String>> {
        self.0.lock().unwrap().push(turns.to_vec());
        let text = turns
            .iter()
            .rev()
            .find(|t| t.role == "assistant")
            .map_or_else(
                || format!("Answer to {}", turns.last().unwrap().content),
                |t| t.content.clone(),
            );
        Box::pin(async move {
            Ok(pylon::engine::Generation {
                text,
                input_tokens: Some(1),
                output_tokens: Some(1),
                model: "qwen3.5-0.8b-q8_0".into(),
                finish: pylon::engine::Finish::Stop,
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_turn_sends_the_latest_message_and_its_history() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut config = pylon::provider::Config::new(&url, "mac", home.path().to_path_buf());
    config.label = "Mac M5 Max".into();
    config.allow = None;
    config.rate_per_minute = 600;
    let sent = Arc::new(Mutex::new(Vec::new()));
    let provider = pylon::provider::Provider::new(
        config,
        Identity::generate(),
        Arc::new(Parrot(sent.clone())),
    )
    .unwrap();
    let (_stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(provider.run(async {
        let _ = stopped.await;
    }));
    for _ in 0..200 {
        if hub.lock().await.stored.iter().any(|e| e.kind == 30_200) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let world = world(url).await;
    let (_, headers, session) =
        send(&world, "", None, "GET", "/lander8/session", None, false).await;
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let chat = "5e1f0c3a-7d2b-4c4e-9a1b-000000000021";
    let messages = [
        "who are you",
        "ok i want to atlk to deepseek",
        "wtf u mean deepseek qwen goddammit",
    ];
    let mut answers = Vec::new();
    for (n, text) in messages.iter().enumerate() {
        let request = format!("5e1f0c3a-7d2b-4c4e-9a1b-00000000002{}", n + 1);
        let request = if n == 0 { chat.to_owned() } else { request };
        let path = if n == 0 {
            "/v1/threads".to_owned()
        } else {
            format!("/v1/threads/{chat}/messages")
        };
        let (status, _, started) = send(
            &world,
            &cookie,
            Some(&csrf),
            "POST",
            &path,
            Some(json!({"request_id": request, "text": text})),
            true,
        )
        .await;
        assert!(matches!(status, 200..=202), "{status} {started}");
        let (_, _, events) = send(
            &world,
            &cookie,
            None,
            "GET",
            &format!("/v1/threads/{chat}/events?request_id={request}"),
            None,
            true,
        )
        .await;
        let events = events.as_str().unwrap_or_default().to_owned();
        assert!(events.contains(&format!("Answer to {text}")), "{events}");
        answers.push(events);
    }
    // One source per answer, with the model's id as written.
    for events in &answers {
        assert_eq!(events.matches("** · ").count(), 1, "{events}");
        assert!(events.contains("qwen3.5-0.8b-q8\\\\_0"), "{events}");
    }
    let sent = sent.lock().unwrap().clone();
    let last = sent.last().unwrap();
    let words: Vec<(&str, &str)> = last
        .iter()
        .map(|t| (t.role.as_str(), t.content.as_str()))
        .collect();
    // The third message, last and whole, after the two before it; asked
    // again with only what the person wrote, since the first try repeated.
    assert_eq!(
        &words[words.len() - 3..],
        &[
            ("user", messages[0]),
            ("user", messages[1]),
            ("user", messages[2]),
        ]
    );
    let first_try = &sent[sent.len() - 2];
    let words: Vec<(&str, &str)> = first_try
        .iter()
        .map(|t| (t.role.as_str(), t.content.as_str()))
        .collect();
    assert_eq!(
        &words[words.len() - 5..],
        &[
            ("user", messages[0]),
            ("assistant", "Answer to who are you"),
            ("user", messages[1]),
            ("assistant", "Answer to ok i want to atlk to deepseek"),
            ("user", messages[2]),
        ],
        "earlier answers go without their source lines"
    );
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

#[test]
fn flint_is_a_separate_explicit_model_choice() {
    let mut headers = HeaderMap::new();
    headers.insert(MODEL_HEADER, "flint".parse().unwrap());
    assert!(asks_for_flint(&headers));
    assert!(!asks_for_group(&headers));
    headers.insert(MODEL_HEADER, pylon::flint::advertised().parse().unwrap());
    assert!(asks_for_flint(&headers));
    headers.insert(MODEL_HEADER, "flint@sha256:other".parse().unwrap());
    // An unavailable artifact stays an explicit request, with no fallback.
    assert!(asks_for_flint(&headers));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unavailable_flint_never_uses_hosted_chat_or_another_model() {
    let (relay, _pylon) = relay(true).await;
    let world = world(relay).await;
    for (number, model) in [(7, "flint"), (8, "flint@sha256:other")] {
        let events = ask_for(&world, number, "hello", true, Some(model)).await;
        assert!(
            !events.contains("Answer to hello") && !events.contains("echo:"),
            "{events}"
        );
        assert!(events.contains("unavailable"), "{events}");
    }
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
        shown("Rig *B*", "qwen3.5-0.8b", None, " Hi. \n"),
        "**Rig \\*B\\*** · qwen3.5-0.8b\n\nHi."
    );
    // The model's id as its beacon gives it: `q8_0`, never `q80`.
    assert_eq!(
        shown(
            "Mac M5 Max (network lane)",
            "qwen3.5-0.8b-q8_0",
            None,
            "Hi."
        ),
        "**Mac M5 Max (network lane)** · qwen3.5-0.8b-q8\\_0\n\nHi."
    );
    // A V4.1 group is named once, with its group and its parts.
    let group = router::AnsweredGroup {
        id: "w16-g5".into(),
        stages: 4,
    };
    assert_eq!(
        shown("W16 G5", GROUP_MODEL, Some(&group), "Hi."),
        "**DeepSeek V4.1** · group w16-g5 (4 parts)\n\nHi."
    );
    // A source line the model copied is dropped: one source per answer.
    assert_eq!(
        shown(
            "Mac",
            "qwen",
            None,
            "**DeepSeek (network lane)** · qwen3.5-0.8b-q80\n\nHi."
        ),
        "**Mac** · qwen\n\nHi."
    );
    for text in [
        shown("Rig", "echo", None, "Hi."),
        shown("W16 G5", GROUP_MODEL, Some(&group), "Hi."),
    ] {
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
}

#[test]
fn earlier_answers_lose_their_source_lines() {
    // The owner's chat (#12421): the second answer carried a copied line.
    let second = "**Mac M5 Max (network lane)** · qwen3.5-0.8b-q80\n\n**DeepSeek (network \
lane)** · qwen3.5-0.8b-q80\n\nI am a computer on the OpenAgents network.";
    assert_eq!(
        without_source(second),
        "I am a computer on the OpenAgents network."
    );
    assert_eq!(
        without_source("**Bold** start.\n\nMore."),
        "**Bold** start.\n\nMore."
    );
    assert_eq!(without_source("Plain."), "Plain.");
    let turn = |role: &str, content: &str| pylon::engine::Turn {
        role: role.into(),
        content: content.into(),
    };
    let turns = [turn("user", "who are you"), turn("assistant", second)];
    assert!(repeats(
        &turns,
        "I am a computer on the OpenAgents network."
    ));
    assert!(!repeats(&turns, "DeepSeek V4.1 is a model group."));
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
        group: None,
        sent_tokens: None,
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
        group: None,
        sent_tokens: None,
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

/// A V4.1 head's model: records what it was sent, and answers once the
/// test lets it through `open`.
struct Held {
    sent: Arc<Mutex<Vec<Vec<pylon::engine::Turn>>>>,
    open: Arc<tokio::sync::Semaphore>,
}

impl pylon::engine::Engine for Held {
    fn model(&self) -> &str {
        "held"
    }

    fn healthy(&self) -> pylon::engine::Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [pylon::engine::Turn],
        _: u32,
    ) -> pylon::engine::Pending<'a, Result<pylon::engine::Generation, String>> {
        self.sent.lock().unwrap().push(turns.to_vec());
        let text = format!("Answer to {}", turns.last().unwrap().content);
        Box::pin(async move {
            self.open.acquire().await.unwrap().forget();
            Ok(pylon::engine::Generation {
                text,
                input_tokens: Some(1),
                output_tokens: Some(1),
                model: "held".into(),
                finish: pylon::engine::Finish::Stop,
            })
        })
    }
}

/// Never asked: a V4.1 turn takes no decision.
struct NoDecision;
impl Decide for NoDecision {
    fn decide<'a>(&'a self, _: &'a Question) -> Deciding<'a> {
        Box::pin(async { Err("not asked".to_owned()) })
    }
}

/// The door a V4.1 terminal turn gets, on the relay `url`, waiting up to
/// `queue` while the group is busy.
fn group_door(url: &str, home: &std::path::Path, queue: Duration) -> PylonDoor {
    let mut settings = Settings::new(url, home.to_path_buf());
    settings.want = Some(pylon::group::Want::Model(GROUP_MODEL.into()));
    settings.deadline = Some(GROUP_DEADLINE);
    settings.prompt_tokens = Some(GROUP_PROMPT_TOKENS);
    settings.publish_receipt = false;
    PylonDoor {
        settings,
        buyer: Identity::generate(),
        decide: Arc::new(NoDecision),
        fallback: Some(Box::new(Hosted)),
        queue,
        analytics: None,
    }
}

#[test]
fn who_answered_is_counted_by_a_fixed_word() {
    assert_eq!(answer_source(true, true, false), "group");
    assert_eq!(answer_source(true, false, true), "pylon");
    assert_eq!(answer_source(false, false, true), "hosted");
    assert_eq!(answer_source(false, false, false), "unavailable");
    for word in ["group", "pylon", "hosted", "unavailable", "busy"] {
        assert!(crate::analytics::GRID_ANSWER.contains(&word));
    }
    assert!(crate::analytics::EVENTS.contains(&"grid_answer"));
}

fn said(role: Role, text: String) -> Turn {
    Turn {
        role,
        text,
        meta: None,
        request: None,
        stopped: false,
        at: None,
        model: None,
    }
}

/// A started V4.1 group with a held model, and what it was sent.
async fn held_group() -> (
    String,
    Arc<Mutex<Vec<Vec<pylon::engine::Turn>>>>,
    Arc<tokio::sync::Semaphore>,
    (tokio::sync::oneshot::Sender<()>, tempfile::TempDir),
) {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let open = Arc::new(tokio::sync::Semaphore::new(0));
    let head = start_pylon(
        &url,
        &hub,
        "v41-head",
        Arc::new(Held {
            sent: sent.clone(),
            open: open.clone(),
        }),
    )
    .await;
    (url, sent, open, head)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_v41_turn_sends_about_a_thousand_tokens_with_the_latest_message_whole() {
    let (url, sent, open, _head) = held_group().await;
    open.add_permits(8);
    let home = tempfile::tempdir().unwrap();
    let door = group_door(&url, home.path(), Duration::ZERO);
    // Twenty long earlier turns, about 8,000 tokens, then a new question.
    let mut turns: Vec<Turn> = (0..20)
        .map(|i| {
            said(
                if i % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                },
                format!("{i:02} {}", "w".repeat(1_600)),
            )
        })
        .collect();
    let latest = format!("What about this? {}", "q".repeat(1_200));
    turns.push(said(Role::User, latest.clone()));
    let reply = Arc::new(Mutex::new(Reply::default()));
    door.ask(turns, Context::default(), reply.clone()).await;
    let reply = basic_coder::lock(&reply).clone();
    assert!(reply.done, "{:?}", reply.failure);
    assert!(reply.text.starts_with("**DeepSeek V4.1** · group v41-a"));
    let sent = sent.lock().unwrap().clone();
    let job = sent.last().unwrap();
    assert_eq!(job.last().unwrap().content, latest, "the latest goes whole");
    let tokens: usize = job
        .iter()
        .map(|t| pylon::router::estimate_tokens(&t.content))
        .sum();
    assert!(tokens <= GROUP_PROMPT_TOKENS, "{tokens} tokens");
    // The newest earlier turn is kept, the oldest are dropped.
    let earlier: Vec<&str> = job
        .iter()
        .filter(|t| t.role != "system")
        .map(|t| t.content.as_str())
        .collect();
    assert!(
        earlier.len() >= 2 && earlier.len() < 21,
        "{}",
        earlier.len()
    );
    assert!(earlier[earlier.len() - 2].starts_with("19 "));
    assert!(!earlier.iter().any(|t| t.starts_with("00 ")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_busy_v41_group_is_waited_for_then_named_busy_never_answered_by_another_model() {
    let (url, _sent, open, _head) = held_group().await;
    let home = tempfile::tempdir().unwrap();
    // The first turn holds the group's one slot.
    let first = group_door(&url, home.path(), Duration::ZERO);
    let first_reply = Arc::new(Mutex::new(Reply::default()));
    let holding = tokio::spawn(first.ask(
        vec![said(Role::User, "first".into())],
        Context::default(),
        first_reply.clone(),
    ));
    // Wait for the group's beacon to say it is busy.
    let mut busy = false;
    for _ in 0..200 {
        let found = {
            let mut conn =
                pylon::relay::connect(&url, &Identity::generate(), pylon::relay::LIFETIME)
                    .await
                    .unwrap();
            let book = pylon::client::all_beacons(&mut conn, None).await.unwrap();
            let want = pylon::group::Want::Model(GROUP_MODEL.into());
            pylon::router::busy_for(&book, pylon::now(), &Default::default(), &want)
        };
        if found {
            busy = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(busy, "the group never read as busy");

    // A second turn waits, showing so, and then says DeepSeek is busy.
    let second = group_door(&url, home.path(), Duration::from_secs(5));
    let reply = Arc::new(Mutex::new(Reply::default()));
    let asking = tokio::spawn(second.ask(
        vec![said(Role::User, "second".into())],
        Context::default(),
        reply.clone(),
    ));
    let mut waited = false;
    for _ in 0..100 {
        if basic_coder::lock(&reply).text == WAITING {
            waited = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(waited, "no waiting line");
    asking.await.unwrap();
    let ended = basic_coder::lock(&reply).clone();
    assert!(!ended.done);
    assert!(ended.text.is_empty(), "{}", ended.text);
    let failure = ended.failure.unwrap();
    assert!(
        matches!(&failure, Failure::Refused { code, .. } if code == "deepseek_busy"),
        "{failure:?}"
    );
    let line = failure.describe();
    assert_eq!(line, basic_coder::DEEPSEEK_BUSY);
    assert!(line.contains("DeepSeek is busy") && line.contains("Any Pylon"));
    assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
    assert!(oa_copy::violations(WAITING, &[]).is_empty());

    // A turn that waits while the group frees up is answered by it.
    let third = group_door(&url, home.path(), Duration::from_secs(30));
    let reply = Arc::new(Mutex::new(Reply::default()));
    let asking = tokio::spawn(third.ask(
        vec![said(Role::User, "third".into())],
        Context::default(),
        reply.clone(),
    ));
    for _ in 0..100 {
        if basic_coder::lock(&reply).text == WAITING {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    open.add_permits(2);
    holding.await.unwrap();
    assert!(basic_coder::lock(&first_reply).done);
    asking.await.unwrap();
    let ended = basic_coder::lock(&reply).clone();
    assert!(ended.done, "{:?}", ended.failure);
    assert!(
        ended.text.starts_with("**DeepSeek V4.1** · group v41-a")
            && ended.text.contains("Answer to third"),
        "{}",
        ended.text
    );
}
