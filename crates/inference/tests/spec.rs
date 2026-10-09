//! The spec's own examples decode into the typed models and re-encode
//! without losing anything.

mod common;

use common::{lossless, spec};
use inference::error::{ErrorBody, ErrorType};
use inference::event::{Event, EventBody};
use inference::item::{Item, ItemStatus, MessageContent, Phase, Role};
use inference::request::{CreateResponse, Input, ToolChoice, ToolChoiceMode};
use inference::response::{Response, ResponseStatus};
use serde_json::Value;

#[test]
fn response_resource_example() {
    let response: Response = lossless(&spec("response-resource.json"), "ResponseResource");
    assert_eq!(response.status, ResponseStatus::Completed);
    assert!(response.output_text().starts_with("Here is an example"));
    assert_eq!(
        response.usage.as_ref().map(|usage| usage.total_tokens),
        Some(67)
    );
    response.validate().expect("the spec's example is valid");
}

#[test]
fn response_phase_mock() {
    let response: Response = lossless(&spec("response-phase.json"), "phase mock");
    let phases: Vec<_> = response
        .output
        .iter()
        .filter_map(|item| match item {
            Item::Message(message) => message.phase,
            _ => None,
        })
        .collect();
    assert_eq!(phases, [Phase::Commentary, Phase::FinalAnswer]);
    response.validate().expect("valid");
}

#[test]
fn response_param_example_keeps_an_unknown_status() {
    let request: CreateResponse = lossless(&spec("response-param.json"), "ResponseParam");
    let items = request.input_items();
    assert_eq!(
        items[0].status(),
        Some(ItemStatus::Other("received".into()))
    );
}

#[test]
fn acceptance_suite_requests() {
    let Value::Object(requests) = spec("requests.json") else {
        panic!("requests.json is an object");
    };
    for (name, body) in &requests {
        let request: CreateResponse = lossless(body, name);
        assert!(request.input.is_some(), "{name}");
    }
    let allowed: CreateResponse =
        serde_json::from_value(requests["allowed-tools"].clone()).unwrap();
    assert_eq!(
        allowed.tool_choice,
        Some(ToolChoice::AllowedTools {
            mode: ToolChoiceMode::Auto,
            tools: vec!["get_latest_sales_report".into()],
        })
    );
    // A typeless input message is a message.
    let Some(Input::Items(items)) = &allowed.input else {
        panic!("items")
    };
    assert!(matches!(&items[0], Item::Message(message) if message.role == Role::User));
    assert!(allowed.call_allowed("get_latest_sales_report"));
    assert!(!allowed.call_allowed("send_email"));

    let continuation: CreateResponse =
        serde_json::from_value(requests["websocket-continuation"].clone()).unwrap();
    let refused = continuation.require_stateless().unwrap_err();
    assert_eq!(refused.status(), 400);
    assert_eq!(refused.param.as_deref(), Some("previous_response_id"));
}

#[test]
fn item_examples() {
    let Value::Array(items) = spec("items.json") else {
        panic!("array")
    };
    let decoded: Vec<Item> = items
        .iter()
        .enumerate()
        .map(|(index, item)| lossless(item, &format!("item {index}")))
        .collect();
    let kinds: Vec<_> = decoded.iter().map(Item::type_name).collect();
    assert_eq!(
        kinds,
        [
            "message",
            "function_call",
            "openai:web_search_call",
            "reasoning",
            "implementor_slug:custom_document_search",
            "function_call_output",
        ]
    );
    assert!(matches!(decoded[2], Item::Unknown(_)));
    assert_eq!(decoded[2].status(), Some(ItemStatus::Completed));
    let Item::Reasoning(reasoning) = &decoded[3] else {
        panic!("reasoning")
    };
    assert!(reasoning.summary_text().starts_with("Determined"));
}

#[test]
fn event_examples() {
    let Value::Array(events) = spec("events.json") else {
        panic!("array")
    };
    let decoded: Vec<Event> = events
        .iter()
        .enumerate()
        .map(|(index, event)| lossless(event, &format!("event {index}")))
        .collect();
    assert!(matches!(decoded[0].body, EventBody::OutputItemAdded(_)));
    assert!(matches!(decoded[2].body, EventBody::OutputTextDelta(_)));
    let EventBody::OutputTextDelta(delta) = &decoded[6].body else {
        panic!("delta")
    };
    assert_eq!(delta.obfuscation.as_deref(), Some("Wd6S45xQ7SyQLT"));
    // An extension event survives, its fields intact.
    assert_eq!(decoded[7].type_name(), "acme:trace_event");
    assert_eq!(decoded[7].sequence_number, 1);
    // Encoded events put `type` first, then `sequence_number`.
    let text = serde_json::to_string(&decoded[2]).unwrap();
    assert!(
        text.starts_with(r#"{"type":"response.output_text.delta","sequence_number":13,"#),
        "{text}"
    );
}

#[test]
fn error_example() {
    let body: ErrorBody = lossless(&spec("error.json"), "error");
    assert_eq!(
        body.error.kind,
        ErrorType::Other("invalid_request_error".into())
    );
    assert_eq!(body.error.code.as_deref(), Some("model_not_found"));
    assert_eq!(body.error.param.as_deref(), Some("model"));
}

#[test]
fn strict_where_the_spec_is_strict() {
    // A known item type missing a required field is an error, not Unknown.
    let missing = serde_json::json!({"type": "function_call", "name": "f", "arguments": "{}"});
    assert!(serde_json::from_value::<Item>(missing).is_err());
    // Roles are closed.
    let role = serde_json::json!({"type": "message", "role": "robot", "content": "hi"});
    assert!(serde_json::from_value::<Item>(role).is_err());
    // tool_choice modes are closed.
    assert!(serde_json::from_value::<ToolChoice>(serde_json::json!("sometimes")).is_err());
    // Every event has a sequence number.
    let unnumbered = serde_json::json!({"type": "response.output_text.delta", "item_id": "m",
        "output_index": 0, "content_index": 0, "delta": "x"});
    assert!(serde_json::from_value::<Event>(unnumbered).is_err());
}

#[test]
fn a_served_response_carries_every_required_field() {
    let request = CreateResponse {
        model: Some("openagents/fast".into()),
        input: Some(Input::Text("hi".into())),
        ..CreateResponse::default()
    };
    let response = Response::from_request("resp_1", 1, "google/gemini-3.8-flash", &request);
    let value = serde_json::to_value(&response).unwrap();
    for field in [
        "id",
        "object",
        "created_at",
        "completed_at",
        "status",
        "incomplete_details",
        "model",
        "previous_response_id",
        "instructions",
        "output",
        "error",
        "tools",
        "tool_choice",
        "truncation",
        "parallel_tool_calls",
        "text",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
        "top_logprobs",
        "temperature",
        "reasoning",
        "usage",
        "max_output_tokens",
        "max_tool_calls",
        "store",
        "background",
        "service_tier",
        "metadata",
        "safety_identifier",
        "prompt_cache_key",
    ] {
        assert!(value.get(field).is_some(), "missing {field}");
    }
    assert_eq!(value["text"]["format"]["type"], "text");
    let items = request.input_items();
    assert!(
        matches!(&items[0], Item::Message(m) if m.content == MessageContent::Text("hi".into()))
    );
}
