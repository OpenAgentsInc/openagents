//! An Open Responses stream from an answer that arrived whole: for
//! upstreams that do not stream (`psionic-serve`'s `/v1/responses`, a
//! Pylon job's result).
//!
//! The events follow the spec's order: `response.created`,
//! `response.in_progress`, then per item `output_item.added`, its parts and
//! deltas (the whole text as one delta), `*.done`, `output_item.done`, and
//! a terminal `response.completed` (or `response.incomplete`) carrying
//! every item and the usage.

use serde_json::{Value, json};

use crate::event::Event;
use crate::request::CreateResponse;
use crate::response::{Response, ResponseStatus, Usage};
use crate::seal::random_id;

/// A function call in a whole answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

/// An answer that arrived whole.
#[derive(Clone, Debug, PartialEq)]
pub struct Whole {
    /// The public model id.
    pub model: String,
    pub text: String,
    /// Raw reasoning text, when the upstream sent some.
    pub reasoning: Option<String>,
    pub calls: Vec<Call>,
    pub usage: Usage,
    /// `Completed`, or `Incomplete` when the upstream stopped at its
    /// output limit.
    pub status: ResponseStatus,
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// The events for `answer` to `request`.
#[must_use]
pub fn events(request: &CreateResponse, answer: &Whole) -> Vec<Event> {
    let id = random_id("resp_");
    let created_at = unix_secs();
    let mut response = Response::from_request(&id, created_at, &answer.model, request);
    let mut bodies: Vec<Value> = Vec::new();
    let lifecycle = |kind: &str, response: &Response| json!({"type": kind, "response": serde_json::to_value(response).unwrap_or(Value::Null)});
    bodies.push(lifecycle("response.created", &response));
    bodies.push(lifecycle("response.in_progress", &response));
    let mut output: Vec<Value> = Vec::new();
    if let Some(reasoning) = answer.reasoning.as_deref().filter(|text| !text.is_empty()) {
        let index = output.len();
        let item_id = random_id("rs_");
        let done = json!({"type": "reasoning", "id": item_id, "status": "completed",
            "summary": [], "content": [{"type": "reasoning_text", "text": reasoning}]});
        bodies.extend([
            json!({"type": "response.output_item.added", "output_index": index,
                "item": {"type": "reasoning", "id": item_id, "status": "in_progress",
                         "summary": []}}),
            json!({"type": "response.content_part.added", "item_id": item_id,
                "output_index": index, "content_index": 0,
                "part": {"type": "reasoning_text", "text": ""}}),
            json!({"type": "response.reasoning.delta", "item_id": item_id,
                "output_index": index, "content_index": 0, "delta": reasoning}),
            json!({"type": "response.reasoning.done", "item_id": item_id,
                "output_index": index, "content_index": 0, "text": reasoning}),
            json!({"type": "response.content_part.done", "item_id": item_id,
                "output_index": index, "content_index": 0,
                "part": {"type": "reasoning_text", "text": reasoning}}),
            json!({"type": "response.output_item.done", "output_index": index, "item": done}),
        ]);
        output.push(done);
    }
    if !answer.text.is_empty() {
        let index = output.len();
        let item_id = random_id("msg_");
        let part = json!({"type": "output_text", "text": answer.text, "annotations": []});
        let done = json!({"type": "message", "id": item_id, "status": "completed",
            "role": "assistant", "content": [part]});
        bodies.extend([
            json!({"type": "response.output_item.added", "output_index": index,
                "item": {"type": "message", "id": item_id, "status": "in_progress",
                         "role": "assistant", "content": []}}),
            json!({"type": "response.content_part.added", "item_id": item_id,
                "output_index": index, "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []}}),
            json!({"type": "response.output_text.delta", "item_id": item_id,
                "output_index": index, "content_index": 0, "delta": answer.text}),
            json!({"type": "response.output_text.done", "item_id": item_id,
                "output_index": index, "content_index": 0, "text": answer.text}),
            json!({"type": "response.content_part.done", "item_id": item_id,
                "output_index": index, "content_index": 0, "part": part}),
            json!({"type": "response.output_item.done", "output_index": index, "item": done}),
        ]);
        output.push(done);
    }
    for call in &answer.calls {
        let index = output.len();
        let item_id = random_id("fc_");
        let done = json!({"type": "function_call", "id": item_id, "status": "completed",
            "call_id": call.call_id, "name": call.name, "arguments": call.arguments});
        bodies.extend([
            json!({"type": "response.output_item.added", "output_index": index,
                "item": {"type": "function_call", "id": item_id, "status": "in_progress",
                         "call_id": call.call_id, "name": call.name, "arguments": ""}}),
            json!({"type": "response.function_call_arguments.delta", "item_id": item_id,
                "output_index": index, "delta": call.arguments}),
            json!({"type": "response.function_call_arguments.done", "item_id": item_id,
                "output_index": index, "arguments": call.arguments}),
            json!({"type": "response.output_item.done", "output_index": index, "item": done}),
        ]);
        output.push(done);
    }
    response.status = answer.status.clone();
    response.completed_at = Some(unix_secs());
    response.usage = Some(answer.usage.clone());
    response.output = output
        .into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .collect();
    if answer.status == ResponseStatus::Incomplete {
        response.incomplete_details = Some(crate::response::IncompleteDetails::new(
            crate::response::IncompleteReason::MaxOutputTokens,
        ));
        bodies.push(lifecycle("response.incomplete", &response));
    } else {
        bodies.push(lifecycle("response.completed", &response));
    }
    bodies
        .into_iter()
        .enumerate()
        .filter_map(|(sequence, mut body)| {
            body["sequence_number"] = Value::from(sequence as u64);
            serde_json::from_value(body).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::StreamItem;
    use crate::stream::{Accumulator, StreamCheck};

    #[test]
    fn a_whole_answer_streams_in_the_spec_order() {
        let request: CreateResponse =
            serde_json::from_value(json!({"model": "m", "input": "hi"})).unwrap();
        let answer = Whole {
            model: "local/qwen".into(),
            text: "hello".into(),
            reasoning: Some("thinking".into()),
            calls: vec![Call {
                call_id: "call_1".into(),
                name: "f".into(),
                arguments: "{}".into(),
            }],
            usage: Usage::new(3, 0, 2, 0),
            status: ResponseStatus::Completed,
        };
        let events = events(&request, &answer);
        assert_eq!(events.len(), 2 + 6 + 6 + 4 + 1);
        let items: Vec<StreamItem> = events.iter().cloned().map(StreamItem::Event).collect();
        let violations = StreamCheck::run(items.iter().chain([&StreamItem::Done]));
        assert!(violations.is_empty(), "{violations:?}");
        let mut folded = Accumulator::new();
        for event in &events {
            folded.push(event);
        }
        let response = folded.finish().unwrap();
        assert_eq!(response.output.len(), 3);
        assert_eq!(response.output_text(), "hello");
        assert_eq!(response.model, "local/qwen");
        assert_eq!(response.usage.unwrap().total_tokens, 5);
    }

    #[test]
    fn an_answer_cut_at_its_limit_is_incomplete() {
        let request = CreateResponse::default();
        let answer = Whole {
            model: "m".into(),
            text: "partial".into(),
            reasoning: None,
            calls: Vec::new(),
            usage: Usage::default(),
            status: ResponseStatus::Incomplete,
        };
        let events = events(&request, &answer);
        assert_eq!(events.last().unwrap().type_name(), "response.incomplete");
    }
}
