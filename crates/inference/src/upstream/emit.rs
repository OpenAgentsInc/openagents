//! Builds an Open Responses event stream from Gemini's chunks. (Chat
//! Completions upstreams use the crate's [`crate::chat::EventWriter`];
//! Gemini needs what that writer has no field for: a thought signature
//! kept as `encrypted_content`.)
//!
//! An [`Emitter`] takes what an upstream chunk says — some reasoning, some
//! answer text, a function call's start or arguments, the usage, the
//! finish — and returns the events the spec orders for it, with
//! `sequence_number`s, opening and closing items as the kind of output
//! changes. It also keeps the finished items, so the terminal event
//! carries the whole response.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ResponseError;
use crate::event::{
    ArgumentsDelta, ArgumentsDone, ContentDelta, ContentDone, Event, EventBody, ItemEvent,
    Lifecycle, PartEvent, SummaryDelta, SummaryDone, SummaryPartEvent,
};
use crate::item::{
    ContentPart, FunctionCall, Item, ItemStatus, Message, MessageContent, OutputText, Reasoning,
    Role, TextPart,
};
use crate::request::CreateResponse;
use crate::response::{IncompleteDetails, IncompleteReason, Response, ResponseStatus, Usage};
use crate::wire::Extra;

/// How a stream finished.
#[derive(Clone, Debug, PartialEq)]
pub enum Finish {
    Completed,
    Incomplete(IncompleteReason),
    Failed(ResponseError),
}

struct ReasoningState {
    index: u64,
    id: String,
    content: Option<String>,
    summary: Option<String>,
    encrypted: Option<String>,
}

struct MessageState {
    index: u64,
    id: String,
    text: String,
}

struct CallState {
    key: String,
    index: u64,
    id: String,
    call_id: String,
    name: String,
    arguments: String,
}

/// The event builder for one response.
pub struct Emitter {
    sequence: u64,
    response: Response,
    reasoning: Option<ReasoningState>,
    message: Option<MessageState>,
    calls: Vec<CallState>,
    next_item: u64,
    started: bool,
    finished: bool,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh id with `prefix`: the time and a process counter, unique within
/// a process and sortable by time.
#[must_use]
pub fn fresh_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{nanos:x}{count:x}")
}

/// Seconds since the epoch.
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn text_part(text: &str) -> TextPart {
    TextPart::new(text)
}

impl Emitter {
    /// An emitter for a response to `request`, served as `model` (the
    /// public id).
    #[must_use]
    pub fn new(model: &str, request: &CreateResponse) -> Self {
        let response = Response::from_request(fresh_id("resp"), unix_now(), model, request);
        Self {
            sequence: 0,
            response,
            reasoning: None,
            message: None,
            calls: Vec::new(),
            next_item: 0,
            started: false,
            finished: false,
        }
    }

    /// Whether any output item has been opened: the first token has
    /// arrived, and a failure from here on is the caller's.
    #[must_use]
    pub fn has_output(&self) -> bool {
        self.next_item > 0
    }

    /// Whether the terminal event has been sent.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn event(&mut self, body: EventBody) -> Event {
        let event = Event {
            sequence_number: self.sequence,
            body,
        };
        self.sequence += 1;
        event
    }

    /// `response.created` and `response.in_progress`, once.
    pub fn start(&mut self) -> Vec<Event> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let snapshot = self.response.clone();
        vec![
            self.event(EventBody::lifecycle(Lifecycle::Created, snapshot.clone())),
            self.event(EventBody::lifecycle(Lifecycle::InProgress, snapshot)),
        ]
    }

    fn item_id(&mut self, prefix: &str) -> (u64, String) {
        let index = self.next_item;
        self.next_item += 1;
        (index, format!("{prefix}_{}_{index}", self.response.id))
    }

    fn open_reasoning(&mut self, out: &mut Vec<Event>) {
        if self.reasoning.is_some() {
            return;
        }
        self.close_message(out);
        self.close_calls(out, ItemStatus::Completed);
        let (index, id) = self.item_id("rs");
        let item = Item::Reasoning(Reasoning {
            id: Some(id.clone()),
            status: Some(ItemStatus::InProgress),
            ..Reasoning::default()
        });
        out.push(self.event(EventBody::OutputItemAdded(ItemEvent {
            output_index: index,
            item,
            extra: Extra::new(),
        })));
        self.reasoning = Some(ReasoningState {
            index,
            id,
            content: None,
            summary: None,
            encrypted: None,
        });
    }

    /// Raw reasoning text (Z.ai's `reasoning_content`).
    pub fn reasoning(&mut self, delta: &str) -> Vec<Event> {
        let mut out = self.start();
        if delta.is_empty() {
            return out;
        }
        self.open_reasoning(&mut out);
        let Some(state) = self.reasoning.as_mut() else {
            return out;
        };
        let (index, id) = (state.index, state.id.clone());
        let first = state.content.is_none();
        state
            .content
            .get_or_insert_with(String::new)
            .push_str(delta);
        if first {
            out.push(self.event(EventBody::ContentPartAdded(PartEvent {
                item_id: id.clone(),
                output_index: index,
                content_index: 0,
                part: ContentPart::ReasoningText(text_part("")),
                extra: Extra::new(),
            })));
        }
        out.push(self.event(EventBody::ReasoningDelta(ContentDelta {
            item_id: id,
            output_index: index,
            content_index: 0,
            delta: delta.to_owned(),
            logprobs: None,
            obfuscation: None,
            extra: Extra::new(),
        })));
        out
    }

    /// Reasoning summary text (Gemini's thought summaries).
    pub fn reasoning_summary(&mut self, delta: &str) -> Vec<Event> {
        let mut out = self.start();
        if delta.is_empty() {
            return out;
        }
        self.open_reasoning(&mut out);
        let Some(state) = self.reasoning.as_mut() else {
            return out;
        };
        let (index, id) = (state.index, state.id.clone());
        let first = state.summary.is_none();
        state
            .summary
            .get_or_insert_with(String::new)
            .push_str(delta);
        if first {
            out.push(
                self.event(EventBody::ReasoningSummaryPartAdded(SummaryPartEvent {
                    item_id: id.clone(),
                    output_index: index,
                    summary_index: 0,
                    part: ContentPart::SummaryText(text_part("")),
                    extra: Extra::new(),
                })),
            );
        }
        out.push(
            self.event(EventBody::ReasoningSummaryTextDelta(SummaryDelta {
                item_id: id,
                output_index: index,
                summary_index: 0,
                delta: delta.to_owned(),
                obfuscation: None,
                extra: Extra::new(),
            })),
        );
        out
    }

    /// An opaque reasoning signature (Gemini's `thoughtSignature`), kept as
    /// the reasoning item's `encrypted_content` so a stateless caller can
    /// send it back.
    pub fn reasoning_signature(&mut self, signature: &str) -> Vec<Event> {
        let mut out = self.start();
        if signature.is_empty() {
            return out;
        }
        self.open_reasoning(&mut out);
        if let Some(state) = self.reasoning.as_mut() {
            state.encrypted = Some(signature.to_owned());
        }
        out
    }

    /// Answer text.
    pub fn text(&mut self, delta: &str) -> Vec<Event> {
        let mut out = self.start();
        if delta.is_empty() {
            return out;
        }
        if self.message.is_none() {
            self.close_reasoning(&mut out);
            self.close_calls(&mut out, ItemStatus::Completed);
            let (index, id) = self.item_id("msg");
            let item = Item::Message(Message {
                id: Some(id.clone()),
                status: Some(ItemStatus::InProgress),
                role: Role::Assistant,
                content: MessageContent::Parts(Vec::new()),
                phase: None,
                extra: Extra::new(),
            });
            out.push(self.event(EventBody::OutputItemAdded(ItemEvent {
                output_index: index,
                item,
                extra: Extra::new(),
            })));
            out.push(self.event(EventBody::ContentPartAdded(PartEvent {
                item_id: id.clone(),
                output_index: index,
                content_index: 0,
                part: ContentPart::OutputText(OutputText::new("")),
                extra: Extra::new(),
            })));
            self.message = Some(MessageState {
                index,
                id,
                text: String::new(),
            });
        }
        let Some(state) = self.message.as_mut() else {
            return out;
        };
        state.text.push_str(delta);
        let (index, id) = (state.index, state.id.clone());
        out.push(self.event(EventBody::OutputTextDelta(ContentDelta {
            item_id: id,
            output_index: index,
            content_index: 0,
            delta: delta.to_owned(),
            logprobs: None,
            obfuscation: None,
            extra: Extra::new(),
        })));
        out
    }

    /// A function call's start. `key` is the upstream's handle for it (a
    /// Chat Completions `index`); a repeated key is ignored.
    pub fn call_start(&mut self, key: &str, call_id: &str, name: &str) -> Vec<Event> {
        let mut out = self.start();
        if self.calls.iter().any(|call| call.key == key) {
            return out;
        }
        self.close_reasoning(&mut out);
        self.close_message(&mut out);
        let (index, id) = self.item_id("fc");
        let call_id = if call_id.is_empty() {
            format!("call_{}_{index}", self.response.id)
        } else {
            call_id.to_owned()
        };
        let item = Item::FunctionCall(FunctionCall {
            id: Some(id.clone()),
            status: Some(ItemStatus::InProgress),
            call_id: call_id.clone(),
            name: name.to_owned(),
            arguments: String::new(),
            extra: Extra::new(),
        });
        out.push(self.event(EventBody::OutputItemAdded(ItemEvent {
            output_index: index,
            item,
            extra: Extra::new(),
        })));
        self.calls.push(CallState {
            key: key.to_owned(),
            index,
            id,
            call_id,
            name: name.to_owned(),
            arguments: String::new(),
        });
        out
    }

    /// A piece of a function call's arguments.
    pub fn call_arguments(&mut self, key: &str, delta: &str) -> Vec<Event> {
        let mut out = self.start();
        if delta.is_empty() {
            return out;
        }
        let Some(call) = self.calls.iter_mut().find(|call| call.key == key) else {
            return out;
        };
        call.arguments.push_str(delta);
        let (index, id) = (call.index, call.id.clone());
        out.push(
            self.event(EventBody::FunctionCallArgumentsDelta(ArgumentsDelta {
                item_id: id,
                output_index: index,
                delta: delta.to_owned(),
                obfuscation: None,
                extra: Extra::new(),
            })),
        );
        out
    }

    /// Closes the current output at a native content-block boundary.
    pub fn end_block(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        self.close_reasoning(&mut out);
        self.close_message(&mut out);
        self.close_calls(&mut out, ItemStatus::Completed);
        out
    }

    /// The usage the terminal event will carry.
    pub fn usage(&mut self, usage: Usage) {
        self.response.usage = Some(usage);
    }

    /// The usage so far.
    #[must_use]
    pub fn current_usage(&self) -> Option<&Usage> {
        self.response.usage.as_ref()
    }

    fn close_reasoning(&mut self, out: &mut Vec<Event>) {
        self.close_reasoning_as(out, ItemStatus::Completed);
    }

    fn close_reasoning_as(&mut self, out: &mut Vec<Event>, status: ItemStatus) {
        let Some(state) = self.reasoning.take() else {
            return;
        };
        if let Some(summary) = &state.summary {
            out.push(self.event(EventBody::ReasoningSummaryTextDone(SummaryDone {
                item_id: state.id.clone(),
                output_index: state.index,
                summary_index: 0,
                text: summary.clone(),
                extra: Extra::new(),
            })));
            out.push(
                self.event(EventBody::ReasoningSummaryPartDone(SummaryPartEvent {
                    item_id: state.id.clone(),
                    output_index: state.index,
                    summary_index: 0,
                    part: ContentPart::SummaryText(text_part(summary)),
                    extra: Extra::new(),
                })),
            );
        }
        if let Some(content) = &state.content {
            out.push(self.event(EventBody::ReasoningDone(ContentDone {
                item_id: state.id.clone(),
                output_index: state.index,
                content_index: 0,
                text: content.clone(),
                logprobs: None,
                extra: Extra::new(),
            })));
            out.push(self.event(EventBody::ContentPartDone(PartEvent {
                item_id: state.id.clone(),
                output_index: state.index,
                content_index: 0,
                part: ContentPart::ReasoningText(text_part(content)),
                extra: Extra::new(),
            })));
        }
        let item = Item::Reasoning(Reasoning {
            id: Some(state.id),
            status: Some(status),
            summary: state
                .summary
                .map(|text| vec![ContentPart::SummaryText(text_part(&text))])
                .unwrap_or_default(),
            content: state
                .content
                .map(|text| vec![ContentPart::ReasoningText(text_part(&text))]),
            encrypted_content: state.encrypted,
            extra: Extra::new(),
        });
        self.done(out, state.index, item);
    }

    fn close_message(&mut self, out: &mut Vec<Event>) {
        self.close_message_as(out, ItemStatus::Completed);
    }

    fn close_message_as(&mut self, out: &mut Vec<Event>, status: ItemStatus) {
        let Some(state) = self.message.take() else {
            return;
        };
        out.push(self.event(EventBody::OutputTextDone(ContentDone {
            item_id: state.id.clone(),
            output_index: state.index,
            content_index: 0,
            text: state.text.clone(),
            logprobs: None,
            extra: Extra::new(),
        })));
        let part = ContentPart::OutputText(OutputText::new(state.text.clone()));
        out.push(self.event(EventBody::ContentPartDone(PartEvent {
            item_id: state.id.clone(),
            output_index: state.index,
            content_index: 0,
            part: part.clone(),
            extra: Extra::new(),
        })));
        let item = Item::Message(Message {
            id: Some(state.id),
            status: Some(status),
            role: Role::Assistant,
            content: MessageContent::Parts(vec![part]),
            phase: None,
            extra: Extra::new(),
        });
        self.done(out, state.index, item);
    }

    fn close_calls(&mut self, out: &mut Vec<Event>, status: ItemStatus) {
        for call in std::mem::take(&mut self.calls) {
            out.push(
                self.event(EventBody::FunctionCallArgumentsDone(ArgumentsDone {
                    item_id: call.id.clone(),
                    output_index: call.index,
                    arguments: call.arguments.clone(),
                    extra: Extra::new(),
                })),
            );
            let item = Item::FunctionCall(FunctionCall {
                id: Some(call.id),
                status: Some(status.clone()),
                call_id: call.call_id,
                name: call.name,
                arguments: call.arguments,
                extra: Extra::new(),
            });
            self.done(out, call.index, item);
        }
    }

    fn done(&mut self, out: &mut Vec<Event>, index: u64, item: Item) {
        out.push(self.event(EventBody::OutputItemDone(ItemEvent {
            output_index: index,
            item: item.clone(),
            extra: Extra::new(),
        })));
        self.response.output.push(item);
    }

    /// Closes every open item and sends the terminal event. A second call
    /// sends nothing.
    pub fn finish(&mut self, finish: Finish) -> Vec<Event> {
        let mut out = self.start();
        if self.finished {
            return out;
        }
        self.finished = true;
        let (item_status, lifecycle, status) = match &finish {
            Finish::Completed => (
                ItemStatus::Completed,
                Lifecycle::Completed,
                ResponseStatus::Completed,
            ),
            Finish::Incomplete(_) => (
                ItemStatus::Incomplete,
                Lifecycle::Incomplete,
                ResponseStatus::Incomplete,
            ),
            Finish::Failed(_) => (
                ItemStatus::Incomplete,
                Lifecycle::Failed,
                ResponseStatus::Failed,
            ),
        };
        // Open items close in output order; only the last may be
        // incomplete (the spec's rule), so earlier ones complete.
        let mut open: Vec<u64> = Vec::new();
        if let Some(state) = &self.reasoning {
            open.push(state.index);
        }
        if let Some(state) = &self.message {
            open.push(state.index);
        }
        open.extend(self.calls.iter().map(|call| call.index));
        let last = open.iter().copied().max();
        let status_for = |index: u64| {
            if Some(index) == last {
                item_status.clone()
            } else {
                ItemStatus::Completed
            }
        };
        if let Some(index) = self.reasoning.as_ref().map(|state| state.index) {
            self.close_reasoning_as(&mut out, status_for(index));
        }
        if let Some(index) = self.message.as_ref().map(|state| state.index) {
            self.close_message_as(&mut out, status_for(index));
        }
        if let Some(index) = self.calls.iter().map(|call| call.index).max() {
            let status = status_for(index);
            self.close_calls(&mut out, status);
        }
        self.response.output.sort_by_key(|item| {
            item.id()
                .and_then(|id| id.rsplit('_').next())
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        self.response.status = status;
        self.response.completed_at = Some(unix_now());
        match finish {
            Finish::Completed => {}
            Finish::Incomplete(reason) => {
                self.response.incomplete_details = Some(IncompleteDetails::new(reason));
            }
            Finish::Failed(error) => self.response.error = Some(error),
        }
        let snapshot = self.response.clone();
        out.push(self.event(EventBody::lifecycle(lifecycle, snapshot)));
        out
    }

    /// The response as it stands (the finished one after [`Emitter::finish`]).
    #[must_use]
    pub fn response(&self) -> &Response {
        &self.response
    }
}
