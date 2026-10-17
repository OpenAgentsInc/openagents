use super::*;
use crate::projects::tests::{Browser, Options, world_with};
use axum::{
    body::{Body, to_bytes},
    http::{Request, header},
};
use futures_util::StreamExt;
use openagents_chat::basic_coder;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

#[derive(Default)]
struct Engine {
    calls: AtomicUsize,
    dropped: AtomicUsize,
    finish: tokio::sync::Notify,
}
struct Worker(Arc<Engine>);
impl crate::ask::Chat for Worker {
    fn door(&self, _: secp256k1::SecretKey) -> Result<Box<dyn basic_coder::Door>, String> {
        Ok(Box::new(Worker(self.0.clone())))
    }
}
impl basic_coder::Door for Worker {
    fn ask(
        &self,
        turns: Vec<basic_coder::Turn>,
        _: openagents_chat::router::Context,
        reply: Arc<Mutex<basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let engine = self.0.clone();
        Box::pin(async move {
            struct Dropped(Arc<Engine>);
            impl Drop for Dropped {
                fn drop(&mut self) {
                    self.0.dropped.fetch_add(1, Ordering::SeqCst);
                }
            }
            let _guard = Dropped(engine.clone());
            engine.calls.fetch_add(1, Ordering::SeqCst);
            let text = turns.last().unwrap().text.clone();
            if text == "quota" {
                basic_coder::lock(&reply).failure = Some(basic_coder::Failure::Refused {
                    code: "quota_exhausted".into(),
                    message: "synthetic private provider message".into(),
                    retry_after_ms: Some(500),
                });
                return;
            }
            basic_coder::lock(&reply).text = format!("Answer to {text}");
            engine.finish.notified().await;
            let mut r = basic_coder::lock(&reply);
            r.text.push_str(" complete");
            r.meta.judgment = Some("private-routing-canary".into());
            r.meta
                .cards
                .push(json!({"card":"video_lab","title":"Synthetic result"}));
            r.done = true;
        })
    }
}
async fn request(
    world: &crate::projects::tests::World,
    token: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Response {
    world
        .site
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, &world.host)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn value(response: Response, status: u16) -> Value {
    assert_eq!(response.status().as_u16(), status);
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn stream_values(response: Response) -> Vec<Value> {
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(response.into_body(), 1024 * 1024),
    )
    .await
    .unwrap()
    .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let doc = crate::app_openapi::document();
    let schema = json!({"$ref":"#/components/schemas/ChatEvent","components":doc["components"],"$schema":"https://json-schema.org/draft/2020-12/schema"});
    let validator = jsonschema::validator_for(&schema).unwrap();
    text.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(|data| {
            let event: Value = serde_json::from_str(data.trim()).unwrap();
            assert!(validator.is_valid(&event), "{event}");
            event
        })
        .collect()
}
#[tokio::test]
async fn existing_runtime_streams_same_chat_deduplicates_retries_and_stops_only_named_request() {
    let engine = Arc::new(Engine::default());
    let world = world_with(Options {
        chat: Some(Arc::new(Worker(engine.clone()))),
        ..Options::default()
    })
    .await;
    let mut alice = Browser::default();
    alice.sign_in(&world, "octo-local").await;
    let token = &alice.0["oa_cloud_session"];
    let mut bob = Browser::default();
    bob.sign_in(&world, "quiet-local").await;
    let other = &bob.0["oa_cloud_session"];
    let chat = "d75ef8a1-16ca-49b6-83be-000000000093";
    let next = "d75ef8a1-16ca-49b6-83be-000000000094";
    let quota = "d75ef8a1-16ca-49b6-83be-000000000095";
    let initial = json!({"request_id":chat,"text":"first"});
    value(
        request(&world, token, "POST", "/v1/threads", Some(initial.clone())).await,
        201,
    )
    .await;
    value(
        request(&world, token, "POST", "/v1/threads", Some(initial)).await,
        200,
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while engine.calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    engine.finish.notify_one();
    let events = stream_values(
        request(
            &world,
            token,
            "GET",
            &format!("/v1/threads/{chat}/events?request_id={chat}"),
            None,
        )
        .await,
    )
    .await;
    assert!(events.iter().any(|e|e["type"]=="text" && e["text"].as_str().unwrap().contains("Answer to first")));
    assert!(events.iter().any(|e| e["type"] == "cards"));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("private-routing-canary")
    );
    assert_eq!(events.last().unwrap()["outcome"], "answered");
    let replay = stream_values(
        request(
            &world,
            token,
            "GET",
            &format!("/v1/threads/{chat}/events?request_id={chat}"),
            None,
        )
        .await,
    )
    .await;
    assert!(
        replay
            .iter()
            .filter(|e| e["type"] == "text")
            .all(|e| e["replace"] == true)
    );
    value(
        request(
            &world,
            token,
            "POST",
            &format!("/v1/threads/{chat}/messages"),
            Some(json!({"request_id":next,"text":"second"})),
        )
        .await,
        202,
    )
    .await;
    let late = value(
        request(
            &world,
            token,
            "POST",
            &format!("/v1/threads/{chat}/stop"),
            Some(json!({"request_id":chat})),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(late["stopped"], false);
    // The website's original HTML SSE sees the same server-run answer.
    let mut browser_events = world
        .site
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/chat/{chat}/events"))
                .header(header::HOST, &world.host)
                .header(header::COOKIE, format!("oa_cloud_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    let frame = tokio::time::timeout(Duration::from_secs(5), browser_events.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(std::str::from_utf8(&frame).unwrap().contains("transcript"));
    drop(browser_events);
    for path in [
        format!("/v1/threads/{chat}/events?request_id={next}"),
        format!("/v1/threads/{chat}"),
    ] {
        assert_eq!(
            request(&world, other, "GET", &path, None).await.status(),
            404
        );
    }
    assert_eq!(
        request(
            &world,
            other,
            "POST",
            &format!("/v1/threads/{chat}/stop"),
            Some(json!({"request_id":next}))
        )
        .await
        .status(),
        404
    );
    let stopped = value(
        request(
            &world,
            token,
            "POST",
            &format!("/v1/threads/{chat}/stop"),
            Some(json!({"request_id":next})),
        )
        .await,
        200,
    )
    .await;
    assert_eq!(stopped["stopped"], true);
    let events = stream_values(
        request(
            &world,
            token,
            "GET",
            &format!("/v1/threads/{chat}/events?request_id={next}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(events.last().unwrap()["outcome"], "stopped");
    value(
        request(
            &world,
            token,
            "POST",
            &format!("/v1/threads/{chat}/messages"),
            Some(json!({"request_id":quota,"text":"quota"})),
        )
        .await,
        202,
    )
    .await;
    let events = stream_values(
        request(
            &world,
            token,
            "GET",
            &format!("/v1/threads/{chat}/events?request_id={quota}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(events.last().unwrap()["error"]["code"], "quota_exhausted");
    assert!(
        !events
            .last()
            .unwrap()
            .to_string()
            .contains("private provider")
    );
    // Separately owned work must never be reported as successfully stopped.
    let store = crate::chat_store::Store::local(world.root().join("chats"));
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let loaded = store.load(&owner, chat).await.unwrap().unwrap();
    let mut pending_elsewhere = loaded.conversation.clone();
    pending_elsewhere.revision += 1;
    pending_elsewhere.pending = None;
    pending_elsewhere.requests.last_mut().unwrap().outcome = Outcome::Pending;
    store
        .compare_and_swap(&loaded, &pending_elsewhere)
        .await
        .unwrap();
    let refused = value(
        request(
            &world,
            token,
            "POST",
            &format!("/v1/threads/{chat}/stop"),
            Some(json!({"request_id":quota})),
        )
        .await,
        409,
    )
    .await;
    assert_eq!(refused["error"]["code"], "unsupported_turn");
    let response = request(
        &world,
        token,
        "DELETE",
        &format!("/v1/threads/{chat}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);
}

#[test]
fn lab_scope_stays_saved_but_never_enters_public_card_events() {
    let meta = openagents_chat::router::Meta {
        cards: vec![json!({"schema":lab_kit::card::SCHEMA,"kind":"echo",
            "document":"example","version":1,"view":"report",
            "_lab":{"space":"me","owner":"private-owner"}})],
        ..Default::default()
    };
    let public = display_cards(&meta);
    assert!(public["cards"][0].get("_lab").is_none());
    assert_eq!(public["cards"][0]["document"], "example");
    assert_eq!(meta.cards[0]["_lab"]["owner"], "private-owner");
}
