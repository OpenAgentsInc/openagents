//! The attempt loop with stub adapters: fallback only before the first
//! token, the first-token deadline, every attempt recorded, the route and
//! cost events, benching, and `openagents/auto`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use inference::event::{Event, EventBody};
use inference::meter::{self, Api, Meter};
use inference::request::CreateResponse;
use inference::response::ResponseStatus;
use inference::router::{ClassEntry, ClassModel, ClassTable, TaskClass};
use inference::run::{Caller, Gateway, PickClass, collect};
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};

const MODEL: &str = "test/model";

/// What a stub does when sent a request.
#[derive(Clone, Copy, Debug)]
enum Script {
    /// Answers "hello" and completes.
    Answer,
    /// Refuses with this HTTP status before any stream.
    Status(u16),
    /// Opens a stream and never sends a token.
    Hang,
    /// Sends its first token, then breaks.
    BreakAfterFirst,
}

struct Stub {
    name: &'static str,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    script: Script,
    calls: AtomicUsize,
}

impl Stub {
    fn new(name: &'static str, script: Script) -> Arc<Self> {
        Arc::new(Self {
            name,
            account: Account {
                id: format!("{name}-account"),
                basis: CostBasis::PayAsYouGo,
            },
            privacy: PrivacyTerms::zero_retention("test"),
            models: vec![ModelRow {
                id: MODEL.to_owned(),
                upstream_model: MODEL.to_owned(),
                capabilities: Capabilities {
                    tools: true,
                    reasoning: false,
                    reasoning_always_on: false,
                    json_schema: true,
                    images: false,
                    context: 100_000,
                    max_output: 8_000,
                },
                // $1 in, $2 out per million tokens.
                price: Price::micro(1_000_000, 100_000, 2_000_000),
                price_source: "test",
            }],
            script,
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

fn event(value: Value) -> Event {
    serde_json::from_value(value).expect("event")
}

fn response(status: &str, output: Value, usage: Value) -> Value {
    json!({
        "id": "resp_1", "object": "response", "created_at": 1, "status": status,
        "model": MODEL, "output": output, "usage": usage,
    })
}

fn message(text: &str) -> Value {
    json!({"type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
           "content": [{"type": "output_text", "text": text, "annotations": []}]})
}

fn opening() -> Vec<Event> {
    vec![
        event(json!({"type": "response.created", "sequence_number": 0,
                     "response": response("in_progress", json!([]), Value::Null)})),
        event(
            json!({"type": "response.output_item.added", "sequence_number": 1, "output_index": 0,
                     "item": {"type": "message", "id": "msg_1", "status": "in_progress",
                              "role": "assistant", "content": []}}),
        ),
        event(
            json!({"type": "response.output_text.delta", "sequence_number": 2,
                     "item_id": "msg_1", "output_index": 0, "content_index": 0, "delta": "hello"}),
        ),
    ]
}

impl Upstream for Stub {
    fn name(&self) -> &'static str {
        self.name
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.models
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        _request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let row = self.model(model).expect("model");
            let meter = AttemptMeter::start(self, row);
            let events: EventStream = match self.script {
                Script::Status(status) => {
                    let error =
                        AttemptError::new(ErrorClass::of_status(status), "refused").status(status);
                    meter.fail(&error);
                    return Err(error);
                }
                Script::Answer => {
                    let mut events = opening();
                    events.push(event(json!({
                        "type": "response.completed", "sequence_number": 3,
                        "response": response("completed", json!([message("hello")]),
                            json!({"input_tokens": 1000, "output_tokens": 500,
                                   "input_tokens_details": {"cached_tokens": 0},
                                   "output_tokens_details": {"reasoning_tokens": 0},
                                   "total_tokens": 1500})),
                    })));
                    Box::pin(futures_util::stream::iter(events.into_iter().map(Ok)))
                }
                Script::Hang => Box::pin(
                    futures_util::stream::iter(vec![Ok(opening().remove(0))])
                        .chain(futures_util::stream::pending()),
                ),
                Script::BreakAfterFirst => Box::pin(
                    futures_util::stream::iter(opening().into_iter().map(Ok)).chain(
                        futures_util::stream::iter(vec![Err(AttemptError::new(
                            ErrorClass::Connection,
                            "reset",
                        ))]),
                    ),
                ),
            };
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

/// A chat class over `upstreams`, in order, with a 200 ms first-token
/// deadline.
fn classes(upstreams: &[&str]) -> ClassTable {
    let entry = ClassEntry {
        models: upstreams
            .iter()
            .map(|upstream| ClassModel {
                model: MODEL.to_owned(),
                upstream: Some((*upstream).to_owned()),
            })
            .collect(),
        first_token_ms: 200,
        floor: None,
    };
    let mut table = ClassTable::default();
    table.classes = BTreeMap::from([(TaskClass::Chat, entry.clone()), (TaskClass::Fast, entry)]);
    table
}

fn gateway(stubs: &[Arc<Stub>]) -> (Gateway, Arc<Meter>) {
    let meter = Arc::new(Meter::new(&meter::Config::default()));
    let upstreams = stubs
        .iter()
        .map(|stub| stub.clone() as Arc<dyn Upstream>)
        .collect();
    let names: Vec<&str> = stubs.iter().map(|stub| stub.name).collect();
    (
        Gateway::new(upstreams, meter.clone()).with_classes(classes(&names)),
        meter,
    )
}

fn request(model: &str) -> CreateResponse {
    serde_json::from_value(json!({"model": model, "input": "hi", "stream": true})).expect("request")
}

fn caller() -> Caller {
    Caller {
        request_id: "req_1".into(),
        api: Api::Responses,
        ..Caller::default()
    }
}

async fn drain(mut events: inference::run::Events) -> Vec<Event> {
    let mut out = Vec::new();
    while let Some(event) = events.next().await {
        out.push(event);
    }
    out
}

fn kept(meter: &Meter) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    meter.status(now).expect("status").records.kept
}

#[tokio::test]
async fn a_failure_before_the_first_token_falls_back_and_both_attempts_are_recorded() {
    let down = Stub::new("down", Script::Status(503));
    let up = Stub::new("up", Script::Answer);
    let (gateway, meter) = gateway(&[down.clone(), up.clone()]);
    let routed = gateway
        .run(&request("openagents/chat"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.upstream, "up");
    assert_eq!(routed.class, Some(TaskClass::Chat));
    assert_eq!(routed.attempts.len(), 2);
    assert_eq!(routed.attempts[0].reason.as_deref(), Some("server"));
    let events = drain(routed.events).await;
    let types: Vec<&str> = events.iter().map(Event::type_name).collect();
    // Route before the first output item; cost right before the terminal.
    let route = types.iter().position(|t| *t == "openagents:route").unwrap();
    let item = types
        .iter()
        .position(|t| *t == "response.output_item.added")
        .unwrap();
    assert!(route < item, "{types:?}");
    assert_eq!(
        &types[types.len() - 2..],
        ["openagents:cost", "response.completed"]
    );
    // Sequence numbers are ours, from zero, without gaps.
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.sequence_number, index as u64);
    }
    let EventBody::Cost(cost) = &events[events.len() - 2].body else {
        panic!("cost");
    };
    // 1,000 in at $1/M and 500 out at $2/M: $0.002, plus 5%.
    assert_eq!(cost.cost.upstream_usd, "0.002");
    assert_eq!(cost.cost.margin_usd, "0.0001");
    assert_eq!(cost.cost.price_usd, "0.0021");
    let EventBody::Completed(done) = &events[events.len() - 1].body else {
        panic!("completed");
    };
    let info = done.response.openagents.as_ref().expect("openagents");
    assert_eq!(info.upstream, "up");
    assert_eq!(info.attempts.len(), 2);
    assert_eq!(kept(&meter), 2);
}

#[tokio::test]
async fn no_first_token_within_the_deadline_falls_back() {
    let slow = Stub::new("slow", Script::Hang);
    let up = Stub::new("up", Script::Answer);
    let (gateway, meter) = gateway(&[slow.clone(), up.clone()]);
    let started = std::time::Instant::now();
    let routed = gateway
        .run(&request("openagents/chat"), &caller())
        .await
        .expect("routed");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(routed.upstream, "up");
    assert_eq!(
        routed.attempts[0].reason.as_deref(),
        Some("first_token_deadline")
    );
    let response = collect(routed.events).await.expect("response");
    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(response.output_text(), "hello");
    assert_eq!(kept(&meter), 2);
}

#[tokio::test]
async fn a_failure_after_the_first_token_is_the_callers_and_never_falls_back() {
    let breaks = Stub::new("breaks", Script::BreakAfterFirst);
    let up = Stub::new("up", Script::Answer);
    let (gateway, meter) = gateway(&[breaks.clone(), up.clone()]);
    let routed = gateway
        .run(&request("openagents/chat"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.upstream, "breaks");
    let events = drain(routed.events).await;
    let last = events.last().expect("terminal");
    assert_eq!(last.type_name(), "response.failed");
    let EventBody::Failed(failed) = &last.body else {
        panic!("failed");
    };
    let error = failed.response.error.as_ref().expect("error");
    assert_eq!(error.code, "upstream_failed");
    assert_eq!(up.calls(), 0, "no fallback after the first token");
    assert_eq!(kept(&meter), 1);
}

#[tokio::test]
async fn a_request_refused_as_malformed_does_not_fall_back() {
    let refuses = Stub::new("refuses", Script::Status(400));
    let up = Stub::new("up", Script::Answer);
    let (gateway, meter) = gateway(&[refuses.clone(), up.clone()]);
    let error = gateway
        .run(&request("openagents/chat"), &caller())
        .await
        .expect_err("refused");
    assert_eq!(error.kind.status(), 400);
    assert_eq!(up.calls(), 0);
    assert_eq!(kept(&meter), 1);
}

#[tokio::test]
async fn every_attempt_failing_is_upstream_failed() {
    let a = Stub::new("a", Script::Status(500));
    let b = Stub::new("b", Script::Status(429));
    let (gateway, meter) = gateway(&[a, b]);
    let error = gateway
        .run(&request("openagents/chat"), &caller())
        .await
        .expect_err("failed");
    assert_eq!(error.kind.status(), 502);
    assert!(error.message.contains("rate_limited"), "{}", error.message);
    assert_eq!(kept(&meter), 2);
}

#[tokio::test]
async fn an_auth_failure_benches_the_upstream_for_the_next_request() {
    let revoked = Stub::new("revoked", Script::Status(401));
    let up = Stub::new("up", Script::Answer);
    let (gateway, _meter) = gateway(&[revoked.clone(), up.clone()]);
    for _ in 0..2 {
        let routed = gateway
            .run(&request("openagents/chat"), &caller())
            .await
            .expect("routed");
        assert_eq!(routed.upstream, "up");
        drain(routed.events).await;
    }
    assert_eq!(revoked.calls(), 1, "benched after its 401");
}

struct Judge(Option<TaskClass>);

impl PickClass for Judge {
    fn pick<'a>(&'a self, _request: &'a CreateResponse) -> BoxFuture<'a, Option<TaskClass>> {
        Box::pin(async move { self.0 })
    }
}

#[tokio::test]
async fn auto_takes_the_judged_class_and_chat_without_a_judgment() {
    let up = Stub::new("up", Script::Answer);
    let (plain, _) = gateway(&[up.clone()]);
    let routed = plain
        .run(&request("openagents/auto"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.class, Some(TaskClass::Chat));
    let (judged, _) = gateway(&[up.clone()]);
    let judged = judged.with_judge(
        Arc::new(Judge(Some(TaskClass::Fast))),
        Duration::from_secs(1),
    );
    let routed = judged
        .run(&request("openagents/auto"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.class, Some(TaskClass::Fast));
    // A judged class with no route right now answers as chat.
    let (stranded, _) = gateway(&[up.clone()]);
    let stranded = stranded.with_judge(
        Arc::new(Judge(Some(TaskClass::Code))),
        Duration::from_secs(1),
    );
    let routed = stranded
        .run(&request("openagents/auto"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.class, Some(TaskClass::Chat));
    let (silent, _) = gateway(&[up]);
    let silent = silent.with_judge(Arc::new(Judge(None)), Duration::from_secs(1));
    let routed = silent
        .run(&request("openagents/auto"), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.class, Some(TaskClass::Chat));
}

#[tokio::test]
async fn the_committed_attempt_records_ok_with_its_cost() {
    let up = Stub::new("up", Script::Answer);
    let (gateway, meter) = gateway(&[up]);
    let routed = gateway
        .run(&request(MODEL), &caller())
        .await
        .expect("routed");
    assert_eq!(routed.class, None);
    drain(routed.events).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let rates = meter.rates(60_000, now);
    let rate = rates
        .iter()
        .find(|rate| rate.upstream == "up")
        .expect("rate");
    assert_eq!(rate.attempts, 1);
}
