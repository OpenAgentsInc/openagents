//! Streaming translation: Chat Completions chunks become an Open Responses
//! stream that follows the spec's order, and that stream becomes the same
//! chunks again.

mod common;

use inference::chat::{ChatChunk, ChunkWriter, CompletionBuilder, EventWriter};
use inference::event::{Event, EventBody};
use inference::openagents::{Cost, CostEvent, RouteEvent};
use inference::request::CreateResponse;
use inference::response::{Response, ResponseStatus};
use inference::sse::{DONE_FRAME, ResponsesDecoder, StreamItem, encode_event};
use inference::stream::{Accumulator, Sequencer, StreamCheck};
use serde_json::{Value, json};

fn chunk(choice: Value) -> ChatChunk {
    serde_json::from_value(
        json!({"id": "chatcmpl-1", "object": "chat.completion.chunk",
        "created": 9, "model": "m", "choices": [choice]}),
    )
    .unwrap()
}

fn delta(delta: Value) -> ChatChunk {
    chunk(json!({"index": 0, "delta": delta, "finish_reason": null}))
}

fn finish(reason: &str) -> ChatChunk {
    chunk(json!({"index": 0, "delta": {}, "finish_reason": reason}))
}

fn usage() -> ChatChunk {
    serde_json::from_value(json!({"id": "chatcmpl-1", "created": 9, "model": "m", "choices": [],
        "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7,
            "prompt_tokens_details": {"cached_tokens": 0}, "completion_tokens_details": {"reasoning_tokens": 1}}}))
    .unwrap()
}

/// Chat chunks → events (checked against the spec) → chunks → reply.
fn through(chunks: &[ChatChunk]) -> (Vec<Event>, inference::chat::Completion) {
    let mut writer = EventWriter::new(&CreateResponse {
        model: Some("m".into()),
        ..CreateResponse::default()
    });
    let mut events = Vec::new();
    for chunk in chunks {
        events.extend(writer.push(chunk));
    }
    events.extend(writer.finish(10));

    // On the wire and back, then the spec's ordering rules.
    let mut wire = String::new();
    for event in &events {
        wire.push_str(&encode_event(event));
    }
    wire.push_str(DONE_FRAME);
    let items: Vec<StreamItem> = ResponsesDecoder::decode_all(wire.as_bytes())
        .into_iter()
        .map(Result::unwrap)
        .collect();
    let violations = StreamCheck::run(&items);
    assert!(violations.is_empty(), "{violations:#?}");

    let mut back = ChunkWriter::new(true);
    let mut builder = CompletionBuilder::new();
    for event in &events {
        for chunk in back.push(event) {
            builder.push(&chunk);
        }
    }
    (events, builder.finish())
}

fn fold(chunks: &[ChatChunk]) -> inference::chat::Completion {
    let mut builder = CompletionBuilder::new();
    for chunk in chunks {
        builder.push(chunk);
    }
    builder.finish()
}

fn terminal(events: &[Event]) -> Response {
    events
        .last()
        .and_then(|event| event.body.response().cloned())
        .expect("ends with a lifecycle event")
}

#[test]
fn text_reasoning_and_refusal() {
    let chunks = [
        delta(json!({"role": "assistant", "content": ""})),
        delta(json!({"reasoning": "Count "})),
        delta(json!({"reasoning": "up."})),
        delta(json!({"content": "one "})),
        delta(json!({"content": "two"})),
        delta(json!({"refusal": "No more."})),
        finish("stop"),
        usage(),
    ];
    let (events, reply) = through(&chunks);
    let response = terminal(&events);
    response.validate().unwrap();
    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(response.output_text(), "one two");
    assert_eq!(response.usage.as_ref().unwrap().total_tokens, 7);
    assert_eq!(reply, fold(&chunks));
}

#[test]
fn interleaved_tool_calls() {
    let chunks = [
        delta(json!({"role": "assistant", "content": null})),
        delta(json!({"content": "Checking."})),
        delta(
            json!({"tool_calls": [{"index": 0, "id": "call_a", "type": "function", "function": {"name": "weather", "arguments": ""}}]}),
        ),
        delta(
            json!({"tool_calls": [{"index": 1, "id": "call_b", "type": "function", "function": {"name": "time", "arguments": "{}"}}]}),
        ),
        delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "{\"city\":"}}]})),
        delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"Rome\"}"}}]})),
        finish("tool_calls"),
        usage(),
    ];
    let (events, reply) = through(&chunks);
    let response = terminal(&events);
    let calls: Vec<_> = response
        .output
        .iter()
        .filter_map(|item| match item {
            inference::Item::FunctionCall(call) => {
                Some((call.call_id.as_str(), call.arguments.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(calls, [("call_a", "{\"city\":\"Rome\"}"), ("call_b", "{}")]);
    let expected = fold(&chunks);
    assert_eq!(
        reply.choices[0].message.tool_calls,
        expected.choices[0].message.tool_calls
    );
    assert_eq!(
        reply.choices[0].finish_reason,
        expected.choices[0].finish_reason
    );
    assert_eq!(
        reply.choices[0].message.content.as_deref(),
        Some("Checking.")
    );
}

#[test]
fn cut_short_by_length() {
    let chunks = [delta(json!({"content": "Once upon"})), finish("length")];
    let (events, reply) = through(&chunks);
    let response = terminal(&events);
    assert!(matches!(
        events.last().unwrap().body,
        EventBody::Incomplete(_)
    ));
    response.validate().unwrap();
    assert_eq!(
        reply.choices[0].finish_reason,
        fold(&chunks).choices[0].finish_reason
    );
}

#[test]
fn a_failure_mid_stream() {
    let failed: ChatChunk = serde_json::from_value(json!({"id": "chatcmpl-1", "created": 9, "model": "m",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "error"}],
        "error": {"type": "upstream_failed", "code": "provider_error", "param": null, "message": "The provider stopped answering."}}))
    .unwrap();
    let chunks = [delta(json!({"content": "Half"})), failed];
    let (events, reply) = through(&chunks);
    let n = events.len();
    assert!(matches!(events[n - 2].body, EventBody::Error(_)));
    assert!(matches!(events[n - 1].body, EventBody::Failed(_)));
    let response = terminal(&events);
    assert_eq!(response.error.as_ref().unwrap().code, "provider_error");
    assert_eq!(
        reply.choices[0].finish_reason.as_ref().unwrap().as_str(),
        "error"
    );
    assert_eq!(reply.choices[0].message.content.as_deref(), Some("Half"));
}

#[test]
fn route_and_cost_events_reach_chat_clients() {
    let request = CreateResponse::default();
    let mut seq = Sequencer::new();
    let mut response = Response::from_request("resp_1", 1, "google/gemini-3.8-flash", &request);
    let mut events = vec![
        seq.stamp(EventBody::lifecycle(
            inference::event::Lifecycle::Created,
            response.clone(),
        )),
        seq.stamp(EventBody::Route(RouteEvent {
            model: "google/gemini-3.8-flash".into(),
            upstream: "vertex".into(),
            ..RouteEvent::default()
        })),
        seq.stamp(EventBody::Cost(CostEvent {
            cost: Cost {
                upstream_usd: "0.00010".into(),
                margin_usd: "0.000005".into(),
                price_usd: "0.000105".into(),
                price_sats: Some(1),
                ..Cost::default()
            },
            ..CostEvent::default()
        })),
    ];
    response.status = ResponseStatus::Completed;
    events.push(seq.stamp(EventBody::lifecycle(
        inference::event::Lifecycle::Completed,
        response,
    )));

    let mut writer = ChunkWriter::new(false);
    let chunks: Vec<_> = events.iter().flat_map(|event| writer.push(event)).collect();
    let last = chunks.last().unwrap();
    let info = last.openagents.as_ref().unwrap();
    assert_eq!(info.upstream, "vertex");
    assert_eq!(info.cost.as_ref().unwrap().price_usd, "0.000105");
    assert_eq!(
        last.choices[0].finish_reason.as_ref().unwrap().as_str(),
        "stop"
    );

    // The accumulator ignores extension events.
    let mut accumulator = Accumulator::new();
    for event in &events {
        accumulator.push(event);
    }
    assert_eq!(
        accumulator.finish().unwrap().status,
        ResponseStatus::Completed
    );
    // And they keep their names on the wire.
    assert!(encode_event(&events[1]).starts_with("event: openagents:route\n"));
}
