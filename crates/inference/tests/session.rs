//! The stateful layer with a stub upstream (#11071): stored responses and
//! `previous_response_id`, owner scope, zero retention, a WebSocket
//! connection's memory and its eviction rule, compaction, and the hosted
//! web search loop stitched into one stream.

use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use inference::event::{Event, EventBody};
use inference::hosted::{self, SearchResult, WebSearch};
use inference::item::{Item, MessageContent, Role};
use inference::meter::{self, Api, Meter};
use inference::request::{CreateResponse, Tool};
use inference::response::ResponseStatus;
use inference::run::{Caller, Gateway, collect};
use inference::seal::Sealer;
use inference::session::{CompactRequest, Local, Owner, Sessions};
use inference::sse::StreamItem;
use inference::store::MemoryStore;
use inference::stream::StreamCheck;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, EventStream, ModelRow,
    Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};

const MODEL: &str = "test/model";

/// Answers `items:<n>` (the input items it was sent), or calls the hosted
/// search function when it is offered and nothing has been searched yet.
struct Echo {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    seen: Mutex<Vec<CreateResponse>>,
}

fn echo() -> Arc<Echo> {
    Arc::new(Echo {
        account: Account {
            id: "echo-account".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![ModelRow {
            id: MODEL.into(),
            upstream_model: MODEL.into(),
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
        seen: Mutex::new(Vec::new()),
    })
}

impl Echo {
    fn seen(&self) -> Vec<CreateResponse> {
        self.seen.lock().unwrap().clone()
    }
}

fn event(value: Value) -> Event {
    serde_json::from_value(value).expect("event")
}

fn response(status: &str, output: Value, usage: Value) -> Value {
    json!({"id": "upstream_id", "object": "response", "created_at": 1, "status": status,
           "model": MODEL, "output": output, "usage": usage})
}

fn usage() -> Value {
    json!({"input_tokens": 1000, "output_tokens": 500,
           "input_tokens_details": {"cached_tokens": 0},
           "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 1500})
}

impl Upstream for Echo {
    fn name(&self) -> &'static str {
        "echo"
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
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(request.clone());
            let row = self.model(model).expect("model");
            let meter = AttemptMeter::start(self, row);
            let items = request.input_items();
            let offered = request
                .tools
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|tool| matches!(tool, Tool::Function(f) if f.name == hosted::FUNCTION));
            let searched = items
                .iter()
                .any(|item| matches!(item, Item::FunctionCallOutput(_)));
            let mut events = vec![event(
                json!({"type": "response.created", "sequence_number": 0,
                "response": response("in_progress", json!([]), Value::Null)}),
            )];
            if offered && !searched {
                let call = json!({"type": "function_call", "id": "fc_1", "status": "completed",
                    "call_id": "call_search", "name": hosted::FUNCTION,
                    "arguments": "{\"query\":\"rust news\"}"});
                let mut added = call.clone();
                added["arguments"] = "".into();
                added["status"] = "in_progress".into();
                events.extend([
                    event(
                        json!({"type": "response.output_item.added", "sequence_number": 1,
                        "output_index": 0, "item": added}),
                    ),
                    event(json!({"type": "response.function_call_arguments.delta",
                        "sequence_number": 2, "item_id": "fc_1", "output_index": 0,
                        "delta": "{\"query\":\"rust news\"}"})),
                    event(json!({"type": "response.function_call_arguments.done",
                        "sequence_number": 3, "item_id": "fc_1", "output_index": 0,
                        "arguments": "{\"query\":\"rust news\"}"})),
                    event(
                        json!({"type": "response.output_item.done", "sequence_number": 4,
                        "output_index": 0, "item": call}),
                    ),
                    event(json!({"type": "response.completed", "sequence_number": 5,
                        "response": response("completed", json!([call]), usage())})),
                ]);
            } else {
                let text = format!("items:{}", items.len());
                let message = json!({"type": "message", "id": "msg_1", "status": "completed",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": text, "annotations": []}]});
                events.extend([
                    event(
                        json!({"type": "response.output_item.added", "sequence_number": 1,
                        "output_index": 0, "item": {"type": "message", "id": "msg_1",
                        "status": "in_progress", "role": "assistant", "content": []}}),
                    ),
                    event(
                        json!({"type": "response.content_part.added", "sequence_number": 2,
                        "item_id": "msg_1", "output_index": 0, "content_index": 0,
                        "part": {"type": "output_text", "text": "", "annotations": []}}),
                    ),
                    event(
                        json!({"type": "response.output_text.delta", "sequence_number": 3,
                        "item_id": "msg_1", "output_index": 0, "content_index": 0,
                        "delta": text}),
                    ),
                    event(
                        json!({"type": "response.output_text.done", "sequence_number": 4,
                        "item_id": "msg_1", "output_index": 0, "content_index": 0,
                        "text": text}),
                    ),
                    event(
                        json!({"type": "response.content_part.done", "sequence_number": 5,
                        "item_id": "msg_1", "output_index": 0, "content_index": 0,
                        "part": {"type": "output_text", "text": text, "annotations": []}}),
                    ),
                    event(
                        json!({"type": "response.output_item.done", "sequence_number": 6,
                        "output_index": 0, "item": message}),
                    ),
                    event(json!({"type": "response.completed", "sequence_number": 7,
                        "response": response("completed", json!([message]), usage())})),
                ]);
            }
            let events: EventStream = Box::pin(futures_util::stream::iter(
                events.into_iter().map(Ok::<_, AttemptError>),
            ));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

struct FakeSearch {
    privacy: PrivacyTerms,
    queries: Mutex<Vec<String>>,
}

impl WebSearch for FakeSearch {
    fn name(&self) -> &str {
        "fake"
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn price_micros(&self) -> u64 {
        10_000
    }
    fn search<'a>(
        &'a self,
        query: &'a str,
        _results: u64,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, String>> {
        Box::pin(async move {
            self.queries.lock().unwrap().push(query.to_owned());
            Ok(vec![SearchResult {
                title: "Rust 2026".into(),
                url: "https://example.com/rust".into(),
                snippet: "News.".into(),
            }])
        })
    }
}

struct Setup {
    sessions: Sessions,
    echo: Arc<Echo>,
    store: Arc<MemoryStore>,
}

fn setup() -> Setup {
    let echo = echo();
    let meter = Arc::new(Meter::new(&meter::Config::default()));
    let gateway = Gateway::new(vec![echo.clone() as Arc<dyn Upstream>], meter);
    let store = Arc::new(MemoryStore::new(Sealer::generate().unwrap().0));
    let sessions = Sessions::new(Arc::new(gateway), Arc::new(Sealer::generate().unwrap().0))
        .with_store(store.clone(), 86_400_000);
    Setup {
        sessions,
        echo,
        store,
    }
}

fn owner(tenant: &str) -> Owner {
    Owner {
        tenant: tenant.into(),
        zero_retention: false,
    }
}

fn caller() -> Caller {
    Caller {
        request_id: "req_1".into(),
        api: Api::Responses,
        ..Caller::default()
    }
}

fn request(body: Value) -> CreateResponse {
    let mut body = body;
    body["model"] = MODEL.into();
    serde_json::from_value(body).expect("request")
}

async fn drain(events: inference::run::Events) -> Vec<Event> {
    events.collect().await
}

#[tokio::test]
async fn stored_responses_continue_and_stay_with_their_owner() {
    let s = setup();
    let acme = owner("acme");
    let first = s
        .sessions
        .create(
            request(json!({"input": "remember cobalt", "store": true})),
            &acme,
            &caller(),
            None,
        )
        .await
        .unwrap();
    let id = first.id.clone();
    assert!(inference::store::is_response_id(&id));
    let folded = collect(first.events).await.unwrap();
    assert_eq!(folded.id, id);
    assert!(folded.store);
    assert_eq!(folded.output_text(), "items:1");
    // Kept sealed: nothing readable in the store.
    assert!(s.store.sealed().iter().all(|blob| !blob.contains("cobalt")));
    assert_eq!(s.sessions.get(&acme, &id).unwrap().response.id, id);

    // Continuing sends the earlier input, its output, then the new input.
    let second = s
        .sessions
        .create(
            request(json!({"input": "what was it?", "previous_response_id": id})),
            &acme,
            &caller(),
            None,
        )
        .await
        .unwrap();
    let folded = collect(second.events).await.unwrap();
    assert_eq!(folded.output_text(), "items:3");
    assert_eq!(folded.previous_response_id.as_deref(), Some(id.as_str()));
    let sent = s.echo.seen().pop().unwrap();
    assert!(sent.previous_response_id.is_none());
    assert_eq!(sent.store, Some(false));
    let kinds: Vec<Role> = sent
        .input_items()
        .iter()
        .filter_map(|item| match item {
            Item::Message(message) => Some(message.role),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [Role::User, Role::Assistant, Role::User]);
    // A store:false response is not kept.
    assert!(s.sessions.get(&acme, &second.id).is_none());

    // Another tenant can neither read nor continue it.
    let other = owner("other");
    assert!(s.sessions.get(&other, &id).is_none());
    let refused = s
        .sessions
        .create(
            request(json!({"input": "x", "previous_response_id": id})),
            &other,
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.code.as_deref(), Some("previous_response_not_found"));
    assert_eq!(refused.param.as_deref(), Some("previous_response_id"));
    assert!(!s.sessions.delete(&other, &id));

    // Deleting removes it at once.
    assert!(s.sessions.delete(&acme, &id));
    assert!(s.sessions.get(&acme, &id).is_none());
    let gone = s
        .sessions
        .create(
            request(json!({"input": "x", "previous_response_id": id})),
            &acme,
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(gone.status(), 400);
}

#[tokio::test]
async fn zero_retention_tenants_and_stateless_gateways_refuse_store() {
    let s = setup();
    let quiet = Owner {
        tenant: "quiet".into(),
        zero_retention: true,
    };
    let refused = s
        .sessions
        .create(
            request(json!({"input": "x", "store": true})),
            &quiet,
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.code.as_deref(), Some("store_not_allowed"));
    assert!(s.store.sealed().is_empty());
    assert!(s.echo.seen().is_empty());

    let meter = Arc::new(Meter::new(&meter::Config::default()));
    let gateway = Gateway::new(vec![echo() as Arc<dyn Upstream>], meter);
    let stateless = Sessions::new(Arc::new(gateway), Arc::new(Sealer::generate().unwrap().0));
    let refused = stateless
        .create(
            request(json!({"input": "x", "store": true})),
            &owner("acme"),
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.param.as_deref(), Some("store"));
}

#[tokio::test]
async fn a_connection_remembers_its_last_response_and_forgets_it_on_failure() {
    let s = setup();
    let acme = owner("acme");
    let socket = Local::new();
    let first = s
        .sessions
        .create(
            request(json!({"input": "remember ember", "store": false})),
            &acme,
            &caller(),
            Some(&socket),
        )
        .await
        .unwrap();
    let id = first.id.clone();
    drain(first.events).await;
    assert_eq!(socket.held().as_deref(), Some(id.as_str()));
    // Nothing was written to the store.
    assert!(s.store.sealed().is_empty());

    // The same connection continues it.
    let next = s
        .sessions
        .create(
            request(json!({"input": "and?", "store": false, "previous_response_id": id})),
            &acme,
            &caller(),
            Some(&socket),
        )
        .await
        .unwrap();
    let next_id = next.id.clone();
    assert_eq!(collect(next.events).await.unwrap().output_text(), "items:3");

    // A new connection cannot.
    let fresh = Local::new();
    let missing = s
        .sessions
        .create(
            request(json!({"input": "and?", "store": false, "previous_response_id": next_id})),
            &acme,
            &caller(),
            Some(&fresh),
        )
        .await
        .unwrap_err();
    assert_eq!(missing.code.as_deref(), Some("previous_response_not_found"));

    // A failed continuation evicts the id it named.
    let failed = s
        .sessions
        .create(
            request(
                json!({"store": false, "previous_response_id": next_id, "input": [
                    {"type": "function_call_output", "call_id": "call_missing", "output": "x"}
                ]}),
            ),
            &acme,
            &caller(),
            Some(&socket),
        )
        .await
        .unwrap_err();
    assert_eq!(failed.status(), 400);
    assert!(socket.held().is_none());
    let stale = s
        .sessions
        .create(
            request(json!({"input": "stale", "store": false, "previous_response_id": next_id})),
            &acme,
            &caller(),
            Some(&socket),
        )
        .await
        .unwrap_err();
    assert_eq!(stale.code.as_deref(), Some("previous_response_not_found"));
}

#[tokio::test]
async fn compaction_seals_a_summary_only_its_owner_can_open() {
    let s = setup();
    let acme = owner("acme");
    let body: CompactRequest = serde_json::from_value(json!({
        "model": MODEL,
        "prompt_cache_key": "k",
        "input": [
            {"type": "message", "role": "user", "content": "We launch on Tuesday."},
            {"type": "message", "role": "assistant", "content": "Understood."}
        ]
    }))
    .unwrap();
    let compacted = s
        .sessions
        .compact(body, &acme, &caller(), None)
        .await
        .unwrap();
    assert_eq!(compacted.object, "response.compaction");
    let wire = serde_json::to_value(&compacted).unwrap();
    assert_eq!(wire["output"][0]["type"], "message");
    assert_eq!(wire["output"][0]["role"], "user");
    assert_eq!(wire["output"][0]["content"][0]["type"], "input_text");
    assert_eq!(wire["output"][1]["type"], "compaction");
    assert!(
        wire["output"][1]["id"]
            .as_str()
            .unwrap()
            .starts_with("cmp_")
    );
    let sealed = wire["output"][1]["encrypted_content"].as_str().unwrap();
    assert!(!sealed.contains("items"));
    assert_eq!(wire["usage"]["total_tokens"], 1500);
    // The summary pass saw the conversation plus the request to summarize.
    let summarize = s.echo.seen().pop().unwrap();
    assert_eq!(summarize.input_items().len(), 3);
    assert!(summarize.instructions.unwrap().contains("compact"));

    // A new chain on the compacted window: the compaction item opens into
    // a developer message carrying the summary.
    let mut input = wire["output"].as_array().unwrap().clone();
    input.push(json!({"type": "message", "role": "user", "content": "When do we launch?"}));
    let turn = s
        .sessions
        .create(
            request(json!({"input": input.clone(), "store": false})),
            &acme,
            &caller(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        collect(turn.events).await.unwrap().status,
        ResponseStatus::Completed
    );
    let sent = s.echo.seen().pop().unwrap();
    let developer = sent
        .input_items()
        .into_iter()
        .find_map(|item| match item {
            Item::Message(message) if message.role == Role::Developer => Some(message),
            _ => None,
        })
        .expect("summary message");
    match developer.content {
        MessageContent::Text(text) => assert!(text.contains("items:3"), "{text}"),
        MessageContent::Parts(_) => panic!("text"),
    }

    // Another tenant's request cannot open it.
    let refused = s
        .sessions
        .create(
            request(json!({"input": input, "store": false})),
            &owner("other"),
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.code.as_deref(), Some("invalid_compaction"));

    // `model` is required.
    let no_model: CompactRequest =
        serde_json::from_value(json!({"input": "Compact this conversation."})).unwrap();
    let refused = s
        .sessions
        .compact(no_model, &acme, &caller(), None)
        .await
        .unwrap_err();
    assert_eq!(refused.param.as_deref(), Some("model"));
}

#[tokio::test]
async fn web_search_runs_between_model_turns_as_one_stream() {
    let echo = echo();
    let meter = Arc::new(Meter::new(&meter::Config::default()));
    let gateway = Arc::new(Gateway::new(vec![echo.clone() as Arc<dyn Upstream>], meter));
    let sealer = Arc::new(Sealer::generate().unwrap().0);
    let unverified = Arc::new(FakeSearch {
        privacy: PrivacyTerms::unverified("test"),
        queries: Mutex::new(Vec::new()),
    });
    let sessions = Sessions::new(gateway.clone(), sealer.clone()).with_search(unverified.clone());
    let tools = json!([{"type": "openagents:web_search", "max_results": 3}]);

    // Under `strict` (the default) an unverified provider is refused.
    let refused = sessions
        .create(
            request(json!({"input": "news?", "tools": tools})),
            &owner("acme"),
            &caller(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(refused.param.as_deref(), Some("tools"));

    // Without a provider, search is refused.
    let none = Sessions::new(gateway.clone(), sealer.clone());
    assert!(
        none.create(
            request(json!({"input": "news?", "tools": tools})),
            &owner("acme"),
            &caller(),
            None
        )
        .await
        .is_err()
    );

    let turn = sessions
        .create(
            request(json!({"input": "news?", "tools": tools,
                           "openagents": {"privacy": "standard"}})),
            &owner("acme"),
            &caller(),
            None,
        )
        .await
        .unwrap();
    let events = drain(turn.events).await;
    // One stream in the spec's order, numbered from zero.
    let items: Vec<StreamItem> = events.iter().cloned().map(StreamItem::Event).collect();
    let violations = StreamCheck::run(items.iter().chain([&StreamItem::Done]));
    assert!(violations.is_empty(), "{violations:?}");
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.sequence_number, index as u64);
    }
    let types: Vec<&str> = events.iter().map(Event::type_name).collect();
    assert_eq!(
        types.iter().filter(|t| **t == "response.created").count(),
        1
    );
    assert!(!types.contains(&"response.function_call_arguments.delta"));
    // The search item, then the answer, each with its own index.
    let added: Vec<(u64, String)> = events
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::OutputItemAdded(added) => {
                Some((added.output_index, added.item.type_name().to_owned()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        added,
        [
            (0, hosted::WEB_SEARCH_CALL.to_owned()),
            (1, "message".to_owned())
        ]
    );
    let last = events.last().unwrap();
    let response = last.body.response().unwrap();
    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(response.output.len(), 2);
    let search_item = serde_json::to_value(&response.output[0]).unwrap();
    assert_eq!(search_item["action"]["query"], "rust news");
    assert_eq!(search_item["results"][0]["url"], "https://example.com/rust");
    // The answer turn saw the call and its output.
    assert_eq!(response.output_text(), "items:3");
    assert_eq!(*unverified.queries.lock().unwrap(), ["rust news"]);
    // Usage and cost sum both turns and the search: two turns of 1,000 in
    // at $1/M and 500 out at $2/M ($0.004) plus $0.01 for the search.
    let usage = response.usage.as_ref().unwrap();
    assert_eq!(usage.total_tokens, 3000);
    let cost = response.openagents.as_ref().unwrap().cost.as_ref().unwrap();
    assert_eq!(cost.upstream_usd, "0.014");
    assert_eq!(cost.margin_usd, "0.0007");
    // The caller's tools are echoed; the model saw the search function.
    assert_eq!(
        serde_json::to_value(&response.tools).unwrap()[0]["type"],
        "openagents:web_search"
    );
    let first = echo.seen().remove(0);
    assert!(matches!(
        &first.tools.unwrap()[0],
        Tool::Function(f) if f.name == hosted::FUNCTION
    ));
}
