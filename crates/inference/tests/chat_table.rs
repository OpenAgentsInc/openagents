//! One test per row of the mapping table in `docs/inference/gateway.md`
//! section 3 (and the crate README): Chat Completions to Open Responses
//! and back, plus the reply direction.

mod common;

use inference::chat::{
    ChatRequest, ChatShape, Completion, completion_from_response, from_responses_request,
    response_from_completion, to_responses_request,
};
use inference::item::{ContentPart, Item, MessageContent, Role};
use inference::request::{
    CreateResponse, Include, Input, TextFormat, Tool, ToolChoice, ToolChoiceMode,
};
use inference::response::{IncompleteReason, ResponseStatus};
use serde_json::{Value, json};

/// Chat request JSON → internal request → Chat request JSON.
fn round_trip(chat: Value) -> (CreateResponse, ChatShape, Value) {
    let request: ChatRequest = serde_json::from_value(chat).expect("chat request decodes");
    let (internal, shape) = to_responses_request(request).expect("translates");
    let back = from_responses_request(&internal, &shape).expect("translates back");
    (internal, shape, serde_json::to_value(back).unwrap())
}

/// Asserts the round trip returns exactly what was sent.
fn exact(chat: Value) -> CreateResponse {
    let (internal, _, back) = round_trip(chat.clone());
    assert_eq!(back, chat);
    internal
}

fn items(request: &CreateResponse) -> Vec<Item> {
    match &request.input {
        Some(Input::Items(items)) => items.clone(),
        other => panic!("expected items, got {other:?}"),
    }
}

#[test]
fn system_and_developer_messages() {
    let internal = exact(json!({"model": "m", "messages": [
        {"role": "system", "content": "Be terse."},
        {"role": "developer", "content": "Answer in French."},
        {"role": "user", "content": "Hi"},
        {"role": "system", "content": "A later system message stays a message."},
    ]}));
    assert_eq!(internal.instructions.as_deref(), Some("Be terse."));
    let items = items(&internal);
    assert!(matches!(&items[0], Item::Message(m) if m.role == Role::Developer));
    assert!(matches!(&items[2], Item::Message(m) if m.role == Role::System));
}

#[test]
fn user_text_and_image_parts() {
    let internal = exact(json!({"model": "m", "messages": [
        {"role": "user", "content": [
            {"type": "text", "text": "What is this?"},
            {"type": "image_url", "image_url": {"url": "https://example.com/a.png", "detail": "high"}},
            {"type": "file", "file": {"file_data": "data:application/pdf;base64,AAAA", "filename": "a.pdf"}},
        ]},
    ]}));
    let Item::Message(message) = &items(&internal)[0] else {
        panic!()
    };
    let MessageContent::Parts(parts) = &message.content else {
        panic!()
    };
    assert!(matches!(&parts[0], ContentPart::InputText(text) if text.text == "What is this?"));
    assert!(matches!(&parts[1], ContentPart::InputImage(image)
        if image.image_url.as_deref() == Some("https://example.com/a.png")));
    assert!(matches!(&parts[2], ContentPart::InputFile(_)));
}

#[test]
fn assistant_content_and_tool_calls() {
    let internal = exact(json!({"model": "m", "messages": [
        {"role": "user", "content": "Weather in Paris and Rome?"},
        {"role": "assistant", "content": "Checking both.", "tool_calls": [
            {"id": "call_1", "type": "function", "function": {"name": "weather", "arguments": "{\"city\":\"Paris\"}"}},
            {"id": "call_2", "type": "function", "function": {"name": "weather", "arguments": "{\"city\":\"Rome\"}"}},
        ]},
        {"role": "tool", "tool_call_id": "call_1", "content": "18 C"},
        {"role": "assistant", "content": null, "tool_calls": [
            {"id": "call_3", "type": "function", "function": {"name": "time", "arguments": "{}"}},
        ]},
    ]}));
    let kinds: Vec<_> = items(&internal)
        .iter()
        .map(|item| item.type_name().to_owned())
        .collect();
    assert_eq!(
        kinds,
        [
            "message",
            "message",
            "function_call",
            "function_call",
            "function_call_output",
            "function_call"
        ]
    );
    // Items carry no turn boundary, so two assistant messages in a row come
    // back as one: the calls join the message before them.
    let (_, _, back) = round_trip(json!({"model": "m", "messages": [
        {"role": "assistant", "content": "A."},
        {"role": "assistant", "content": null, "tool_calls": [
            {"id": "call_3", "type": "function", "function": {"name": "time", "arguments": "{}"}}]},
    ]}));
    assert_eq!(
        back["messages"],
        json!([{"role": "assistant", "content": "A.", "tool_calls": [
        {"id": "call_3", "type": "function", "function": {"name": "time", "arguments": "{}"}}]}])
    );
}

#[test]
fn tool_messages() {
    let internal = exact(json!({"model": "m", "messages": [
        {"role": "assistant", "content": null, "tool_calls": [
            {"id": "call_1", "type": "function", "function": {"name": "weather", "arguments": "{}"}},
        ]},
        {"role": "tool", "tool_call_id": "call_1", "content": "18 C and clear"},
        {"role": "tool", "tool_call_id": "call_1", "content": [{"type": "text", "text": "parts work too"}]},
    ]}));
    assert!(
        matches!(&items(&internal)[1], Item::FunctionCallOutput(output) if output.call_id == "call_1")
    );
}

#[test]
fn tools_tool_choice_and_parallel_tool_calls() {
    let tool = json!({"type": "function", "function": {"name": "weather", "description": "Weather now",
        "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}, "strict": true}});
    for choice in [
        json!("auto"),
        json!("none"),
        json!("required"),
        json!({"type": "function", "function": {"name": "weather"}}),
        json!({"type": "allowed_tools", "allowed_tools": {"mode": "required",
            "tools": [{"type": "function", "function": {"name": "weather"}}]}}),
    ] {
        let internal = exact(
            json!({"model": "m", "messages": [{"role": "user", "content": "hi"}],
            "tools": [tool], "tool_choice": choice, "parallel_tool_calls": false}),
        );
        assert!(
            matches!(internal.tools.as_deref(), Some([Tool::Function(f)]) if f.name == "weather")
        );
        assert_eq!(internal.parallel_tool_calls, Some(false));
    }
    let (internal, _, _) = round_trip(json!({"model": "m", "messages": [], "tool_choice":
        {"type": "allowed_tools", "allowed_tools": {"mode": "auto", "tools": [{"type": "function", "function": {"name": "a"}}]}}}));
    assert_eq!(
        internal.tool_choice,
        Some(ToolChoice::AllowedTools {
            mode: ToolChoiceMode::Auto,
            tools: vec!["a".into()]
        })
    );
}

#[test]
fn response_format() {
    let internal =
        exact(json!({"model": "m", "messages": [], "response_format": {"type": "json_object"}}));
    assert!(matches!(
        internal.text.and_then(|text| text.format),
        Some(TextFormat::JsonObject(_))
    ));
    let internal = exact(
        json!({"model": "m", "messages": [], "response_format": {"type": "json_schema",
        "json_schema": {"name": "answer", "description": "One answer",
            "schema": {"type": "object", "properties": {"a": {"type": "string"}}}, "strict": true}}}),
    );
    assert!(matches!(internal.text.and_then(|text| text.format),
        Some(TextFormat::JsonSchema(schema)) if schema.name == "answer" && schema.strict == Some(true)));
    exact(
        json!({"model": "m", "messages": [], "response_format": {"type": "text"}, "verbosity": "low"}),
    );
}

#[test]
fn max_tokens_and_max_completion_tokens() {
    let internal = exact(json!({"model": "m", "messages": [], "max_completion_tokens": 300}));
    assert_eq!(internal.max_output_tokens, Some(300));
    let internal = exact(json!({"model": "m", "messages": [], "max_tokens": 200}));
    assert_eq!(internal.max_output_tokens, Some(200));
    // Both sent: max_completion_tokens wins, as OpenAI documents.
    let (internal, _, back) = round_trip(
        json!({"model": "m", "messages": [], "max_tokens": 1, "max_completion_tokens": 2}),
    );
    assert_eq!(internal.max_output_tokens, Some(2));
    assert_eq!(
        back,
        json!({"model": "m", "messages": [], "max_completion_tokens": 2})
    );
}

#[test]
fn reasoning_effort() {
    for effort in ["none", "minimal", "low", "medium", "high", "xhigh"] {
        let internal = exact(json!({"model": "m", "messages": [], "reasoning_effort": effort}));
        assert_eq!(internal.reasoning.unwrap().effort.unwrap().as_str(), effort);
    }
}

#[test]
fn sampling_and_identity_fields() {
    let internal = exact(
        json!({"model": "m", "messages": [], "temperature": 0.2, "top_p": 0.9,
        "presence_penalty": 0.5, "frequency_penalty": -0.5, "stop": ["\n\n", "END"], "seed": 7,
        "user": "user-1", "metadata": {"job": "a"}, "service_tier": "flex",
        "safety_identifier": "s", "prompt_cache_key": "k", "store": false}),
    );
    assert_eq!(internal.seed, Some(7));
    exact(json!({"model": "m", "messages": [], "stop": "END"}));
}

#[test]
fn stream_and_include_usage() {
    let (internal, shape, back) = round_trip(json!({"model": "m", "messages": [], "stream": true,
        "stream_options": {"include_usage": true}}));
    assert!(internal.is_stream());
    assert!(shape.include_usage);
    assert_eq!(back["stream_options"], json!({"include_usage": true}));
}

#[test]
fn logprobs() {
    let internal =
        exact(json!({"model": "m", "messages": [], "logprobs": true, "top_logprobs": 3}));
    assert_eq!(internal.include, [Include::OutputTextLogprobs]);
    assert_eq!(internal.top_logprobs, Some(3));
}

#[test]
fn n_above_one_is_refused() {
    let request: ChatRequest =
        serde_json::from_value(json!({"model": "m", "messages": [], "n": 2})).unwrap();
    let error = to_responses_request(request).unwrap_err();
    assert_eq!((error.status(), error.param.as_deref()), (400, Some("n")));
    // n: 1 is the default and is accepted (and not echoed back).
    let (_, _, back) = round_trip(json!({"model": "m", "messages": [], "n": 1}));
    assert_eq!(back, json!({"model": "m", "messages": []}));
}

#[test]
fn what_cannot_cross_is_refused() {
    let refused = |chat: Value| {
        let request: ChatRequest = serde_json::from_value(chat).unwrap();
        to_responses_request(request).unwrap_err().param.unwrap()
    };
    assert_eq!(
        refused(
            json!({"messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": "", "format": "wav"}}]}]})
        ),
        "messages[0].content[0]"
    );
    assert_eq!(
        refused(
            json!({"messages": [{"role": "user", "content": [{"type": "file", "file": {"file_id": "file-1"}}]}]})
        ),
        "messages[0].content[0].file.file_id"
    );
    // The legacy function role does not decode.
    assert!(
        serde_json::from_value::<ChatRequest>(
            json!({"messages": [{"role": "function", "name": "f", "content": "x"}]})
        )
        .is_err()
    );

    // Open Responses requests that cannot become Chat Completions.
    let stored = CreateResponse {
        previous_response_id: Some("resp_1".into()),
        ..CreateResponse::default()
    };
    assert!(from_responses_request(&stored, &ChatShape::default()).is_err());
    let hosted = CreateResponse {
        tools: Some(vec![Tool::Unknown(json!({"type": "openai:web_search"}))]),
        ..CreateResponse::default()
    };
    assert!(from_responses_request(&hosted, &ChatShape::default()).is_err());
}

#[test]
fn reasoning_degrades_to_summary_text() {
    // An assistant turn's reasoning crosses as summary text...
    let internal = exact(json!({"model": "m", "messages": [
        {"role": "assistant", "content": "Five.", "reasoning": "Two plus three."},
    ]}));
    let Item::Reasoning(reasoning) = &items(&internal)[0] else {
        panic!()
    };
    assert_eq!(reasoning.summary_text(), "Two plus three.");
    // ...and encrypted reasoning has no Chat Completions field, so it is dropped.
    let request: CreateResponse = serde_json::from_value(json!({"input": [
        {"type": "reasoning", "summary": [], "encrypted_content": "opaque"},
        {"type": "message", "role": "assistant", "content": "Five."},
    ]}))
    .unwrap();
    let chat =
        serde_json::to_value(from_responses_request(&request, &ChatShape::default()).unwrap())
            .unwrap();
    assert_eq!(
        chat["messages"],
        json!([{"role": "assistant", "content": "Five."}])
    );
}

#[test]
fn openagents_object_and_provider_fields_pass_through() {
    exact(
        json!({"model": "openagents/fast", "messages": [], "openagents": {
        "route": {"order": ["vertex"], "sort": "latency"}, "privacy": "strict", "pay": "ours",
        "max_price": {"input": "0.5", "output": "2"}, "fallbacks": ["zai/glm-5.3-flash"]},
        "provider": {"zdr": true}, "top_k": 40}),
    );
}

fn completion(value: Value) -> Completion {
    serde_json::from_value(value).expect("completion decodes")
}

/// Chat reply → Response → Chat reply.
fn reply_round_trip(value: Value) -> (inference::Response, Value) {
    let reply = completion(value);
    let response = response_from_completion(&reply, &CreateResponse::default());
    response.validate().expect("a valid response");
    let back = serde_json::to_value(completion_from_response(&response)).unwrap();
    (response, back)
}

#[test]
fn finish_reason_and_status() {
    for (finish, status, reason) in [
        ("stop", ResponseStatus::Completed, None),
        ("tool_calls", ResponseStatus::Completed, None),
        (
            "length",
            ResponseStatus::Incomplete,
            Some(IncompleteReason::MaxOutputTokens),
        ),
        (
            "content_filter",
            ResponseStatus::Incomplete,
            Some(IncompleteReason::ContentFilter),
        ),
        ("error", ResponseStatus::Failed, None),
    ] {
        let tool_calls = (finish == "tool_calls").then(|| {
            json!([
            {"id": "call_1", "type": "function", "function": {"name": "f", "arguments": "{}"}}])
        });
        let mut message = json!({"role": "assistant", "content": "x", "refusal": null});
        if let Some(calls) = tool_calls {
            message["tool_calls"] = calls;
        }
        let (response, back) = reply_round_trip(json!({"id": "c1", "object": "chat.completion",
            "created": 5, "model": "m", "choices": [{"index": 0, "message": message,
            "finish_reason": finish, "logprobs": null}]}));
        assert_eq!(response.status, status, "{finish}");
        assert_eq!(
            response.incomplete_details.map(|details| details.reason),
            reason,
            "{finish}"
        );
        assert_eq!(back["choices"][0]["finish_reason"], finish);
        assert_eq!(back["choices"][0]["message"], message, "{finish}");
    }
}

#[test]
fn usage() {
    let usage = json!({"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30,
        "prompt_tokens_details": {"cached_tokens": 4}, "completion_tokens_details": {"reasoning_tokens": 6},
        "cost": 0.0001});
    let (response, back) = reply_round_trip(json!({"id": "c1", "created": 5, "model": "m",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "x"}, "finish_reason": "stop"}],
        "usage": usage}));
    let internal = response.usage.unwrap();
    assert_eq!(
        (
            internal.input_tokens,
            internal.input_tokens_details.cached_tokens
        ),
        (10, 4)
    );
    assert_eq!(back["usage"], usage);
}

#[test]
fn several_output_items_flatten_in_order() {
    let response: inference::Response = serde_json::from_value(json!({
        "id": "resp_1", "created_at": 1, "status": "completed", "model": "m",
        "output": [
            {"type": "reasoning", "id": "rs_1", "status": "completed", "summary": [{"type": "summary_text", "text": "Think."}], "encrypted_content": "opaque"},
            {"type": "message", "id": "m1", "status": "completed", "role": "assistant", "content": [{"type": "output_text", "text": "One, ", "annotations": [{"type": "url_citation", "url": "https://a", "start_index": 0, "end_index": 3, "title": "A"}]}]},
            {"type": "function_call", "id": "fc_1", "status": "completed", "call_id": "call_a", "name": "a", "arguments": "{}"},
            {"type": "message", "id": "m2", "status": "completed", "role": "assistant", "content": [{"type": "output_text", "text": "two.", "annotations": []}]},
            {"type": "function_call", "id": "fc_2", "status": "completed", "call_id": "call_b", "name": "b", "arguments": "{}"},
            {"type": "openai:web_search_call", "id": "ws_1", "status": "completed"},
        ],
    })).unwrap();
    let chat = serde_json::to_value(completion_from_response(&response)).unwrap();
    let message = &chat["choices"][0]["message"];
    assert_eq!(message["content"], "One, two.");
    assert_eq!(message["reasoning"], "Think.");
    assert_eq!(message["tool_calls"][0]["id"], "call_a");
    assert_eq!(message["tool_calls"][1]["id"], "call_b");
    assert_eq!(chat["choices"][0]["finish_reason"], "tool_calls");
    // Citations and the hosted tool item have no Chat Completions field.
    assert!(message.get("annotations").is_none());
}

#[test]
fn spec_requests_cross_to_chat_and_back() {
    let Value::Object(requests) = common::spec("requests.json") else {
        panic!()
    };
    for (name, body) in requests {
        let request: CreateResponse = serde_json::from_value(body).unwrap();
        let Ok(chat) = from_responses_request(&request, &ChatShape::default()) else {
            assert_eq!(
                name, "websocket-continuation",
                "only the stored continuation is refused"
            );
            continue;
        };
        let (back, _) = to_responses_request(chat).unwrap();
        assert_eq!(back.model, request.model, "{name}");
        assert_eq!(back.tools, request.tools, "{name}");
        assert_eq!(back.tool_choice, request.tool_choice, "{name}");
        let texts = |request: &CreateResponse| -> Vec<String> {
            let mut out: Vec<String> = request.instructions.iter().cloned().collect();
            for item in request.input_items() {
                if let Item::Message(message) = item {
                    match message.content {
                        MessageContent::Text(text) => out.push(text),
                        MessageContent::Parts(parts) => {
                            out.extend(parts.iter().filter_map(|p| p.text().map(str::to_owned)))
                        }
                    }
                }
            }
            out
        };
        assert_eq!(texts(&back), texts(&request), "{name}");
    }
}
