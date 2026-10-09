//! Streaming translation, both directions, plus folding a chunk stream
//! into a reply.
//!
//! - [`ChunkWriter`]: Open Responses events in, Chat Completions chunks
//!   out, for a caller on `/v1/chat/completions`.
//! - [`EventWriter`]: Chat Completions chunks in, Open Responses events
//!   out, for an upstream that speaks only Chat Completions.
//! - [`CompletionBuilder`]: chunks folded into the reply they describe.

use std::collections::BTreeMap;

use crate::chat::translate::{
    chat_usage, derived_id, finish_reason, finish_response, reasoning_text, responses_usage,
    status_from_finish,
};
use crate::chat::types::{
    AssistantMessage, ChatChunk, ChatFunctionCall, ChatToolCall, ChatUsage, Choice, ChunkChoice,
    Completion, Delta, FinishReason, FunctionCallDelta, ToolCallDelta, chunk_object,
};
use crate::error::{ApiError, ErrorType};
use crate::event::{
    ArgumentsDelta, ArgumentsDone, ContentDelta, ContentDone, ErrorEvent, Event, EventBody,
    ItemEvent, Lifecycle, PartEvent, RefusalDone, SummaryDelta, SummaryDone, SummaryPartEvent,
};
use crate::item::{
    ContentPart, FunctionCall, Item, ItemStatus, Message, MessageContent, OutputText, Reasoning,
    Refusal, Role, TextPart,
};
use crate::openagents::ResponseInfo;
use crate::request::CreateResponse;
use crate::response::{Response, ResponseStatus};
use crate::sse::encode_frame;
use crate::stream::Sequencer;
use crate::wire::Extra;

/// Encodes a chunk as a server-sent event (`data:` only, as OpenAI sends).
#[must_use]
pub fn encode_chunk(chunk: &ChatChunk) -> String {
    let data = serde_json::to_string(chunk).unwrap_or_else(|error| {
        serde_json::json!({"error": {"type": "server_error", "code": null, "param": null,
            "message": format!("could not encode chunk: {error}")}})
        .to_string()
    });
    encode_frame(None, &data)
}

/// Decodes one chunk's `data`. `[DONE]` gives `Ok(None)`.
pub fn decode_chunk(data: &str) -> Result<Option<ChatChunk>, serde_json::Error> {
    if data.trim() == "[DONE]" {
        return Ok(None);
    }
    serde_json::from_str(data).map(Some)
}

/// Open Responses events in, Chat Completions chunks out.
///
/// Text, refusal, and reasoning deltas become content, refusal, and
/// `reasoning` deltas; each `function_call` item becomes a tool call with
/// its own index; the terminal event becomes the finish chunk, then (with
/// `include_usage`) a usage chunk with no choices. Text an upstream only
/// put in a `*.done` event is sent then, so nothing is lost.
#[derive(Debug, Default)]
pub struct ChunkWriter {
    include_usage: bool,
    id: String,
    created: u64,
    model: String,
    started: bool,
    finished: bool,
    tool_index: BTreeMap<u64, u32>,
    sent_text: BTreeMap<(u64, u64), usize>,
    sent_arguments: BTreeMap<u64, usize>,
    error: Option<ApiError>,
    info: Option<ResponseInfo>,
}

impl ChunkWriter {
    /// A writer; `include_usage` is the caller's
    /// `stream_options.include_usage`.
    #[must_use]
    pub fn new(include_usage: bool) -> Self {
        Self {
            include_usage,
            ..Self::default()
        }
    }

    /// Translates one event.
    pub fn push(&mut self, event: &Event) -> Vec<ChatChunk> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        match event.body.clone().normalized() {
            EventBody::Created(event) | EventBody::Queued(event) | EventBody::InProgress(event) => {
                self.id.clone_from(&event.response.id);
                self.created = event.response.created_at;
                self.model.clone_from(&event.response.model);
                if !self.started {
                    self.started = true;
                    out.push(self.chunk(Delta {
                        role: Some("assistant".to_owned()),
                        content: Some(String::new()),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::OutputItemAdded(added) => {
                if let Item::FunctionCall(call) = &added.item {
                    let index = u32::try_from(self.tool_index.len()).unwrap_or(u32::MAX);
                    self.tool_index.insert(added.output_index, index);
                    self.sent_arguments
                        .insert(added.output_index, call.arguments.len());
                    out.push(self.chunk(Delta {
                        tool_calls: Some(vec![ToolCallDelta {
                            index,
                            id: Some(call.call_id.clone()),
                            kind: Some("function".to_owned()),
                            function: Some(FunctionCallDelta {
                                name: Some(call.name.clone()),
                                arguments: Some(call.arguments.clone()),
                            }),
                        }]),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::OutputTextDelta(delta) => {
                self.note_text(&delta);
                if !delta.delta.is_empty() {
                    out.push(self.chunk(Delta {
                        content: Some(delta.delta),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::RefusalDelta(delta) => {
                self.note_text(&delta);
                if !delta.delta.is_empty() {
                    out.push(self.chunk(Delta {
                        refusal: Some(delta.delta),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::ReasoningDelta(delta) => {
                if !delta.delta.is_empty() {
                    out.push(self.chunk(Delta {
                        reasoning: Some(delta.delta),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::ReasoningSummaryTextDelta(delta) => {
                if !delta.delta.is_empty() {
                    out.push(self.chunk(Delta {
                        reasoning: Some(delta.delta),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::OutputTextDone(done) => {
                if let Some(rest) =
                    self.unsent_text(done.output_index, done.content_index, &done.text)
                {
                    out.push(self.chunk(Delta {
                        content: Some(rest),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::RefusalDone(done) => {
                if let Some(rest) =
                    self.unsent_text(done.output_index, done.content_index, &done.refusal)
                {
                    out.push(self.chunk(Delta {
                        refusal: Some(rest),
                        ..Delta::default()
                    }));
                }
            }
            EventBody::FunctionCallArgumentsDelta(delta) => {
                *self.sent_arguments.entry(delta.output_index).or_default() += delta.delta.len();
                if let Some(chunk) = self.arguments_chunk(delta.output_index, delta.delta) {
                    out.push(chunk);
                }
            }
            EventBody::FunctionCallArgumentsDone(done) => {
                out.extend(self.unsent_arguments(done.output_index, &done.arguments));
            }
            EventBody::OutputItemDone(done) => {
                if let Item::FunctionCall(call) = &done.item {
                    out.extend(self.unsent_arguments(done.output_index, &call.arguments));
                }
            }
            EventBody::Route(route) => {
                let info = self.info.get_or_insert_with(ResponseInfo::default);
                info.model = route.model;
                info.upstream = route.upstream;
            }
            EventBody::Cost(cost) => {
                self.info.get_or_insert_with(ResponseInfo::default).cost = Some(cost.cost);
            }
            EventBody::Error(error) => self.error = Some(error.error),
            EventBody::Completed(terminal)
            | EventBody::Incomplete(terminal)
            | EventBody::Failed(terminal) => {
                out.extend(self.finish_with(&terminal.response));
            }
            _ => {}
        }
        out
    }

    /// Whether the finish chunk has been sent.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn finish_with(&mut self, response: &Response) -> Vec<ChatChunk> {
        self.finished = true;
        let mut out = Vec::new();
        let mut finish = self.chunk(Delta::default());
        finish.choices[0].finish_reason = finish_reason(response);
        if response.status == ResponseStatus::Failed {
            finish.error = Some(self.error.take().unwrap_or_else(|| {
                let (code, message) = response.error.as_ref().map_or_else(
                    || {
                        (
                            "model_error".to_owned(),
                            "The model failed while answering.".to_owned(),
                        )
                    },
                    |error| (error.code.clone(), error.message.clone()),
                );
                ApiError::new(ErrorType::ModelError, message).with_code(code)
            }));
        }
        let info = match (response.openagents.clone(), self.info.take()) {
            (Some(info), _) => Some(info),
            (None, info) => info,
        };
        if self.include_usage {
            out.push(finish);
            out.push(ChatChunk {
                choices: Vec::new(),
                usage: Some(response.usage.as_ref().map(chat_usage).unwrap_or_default()),
                openagents: info,
                ..self.chunk(Delta::default())
            });
        } else {
            finish.openagents = info;
            out.push(finish);
        }
        out
    }

    fn chunk(&self, delta: Delta) -> ChatChunk {
        ChatChunk {
            id: self.id.clone(),
            object: chunk_object(),
            created: self.created,
            model: self.model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta,
                ..ChunkChoice::default()
            }],
            usage: None,
            error: None,
            openagents: None,
            extra: Extra::new(),
        }
    }

    fn note_text(&mut self, delta: &ContentDelta) {
        *self
            .sent_text
            .entry((delta.output_index, delta.content_index))
            .or_default() += delta.delta.len();
    }

    fn unsent_text(&mut self, output_index: u64, content_index: u64, text: &str) -> Option<String> {
        let sent = self
            .sent_text
            .entry((output_index, content_index))
            .or_default();
        let rest = text
            .get(*sent..)
            .filter(|rest| !rest.is_empty())?
            .to_owned();
        *sent = text.len();
        Some(rest)
    }

    fn unsent_arguments(&mut self, output_index: u64, arguments: &str) -> Option<ChatChunk> {
        let sent = self.sent_arguments.entry(output_index).or_default();
        let rest = arguments
            .get(*sent..)
            .filter(|rest| !rest.is_empty())?
            .to_owned();
        *sent = arguments.len();
        self.arguments_chunk(output_index, rest)
    }

    fn arguments_chunk(&self, output_index: u64, arguments: String) -> Option<ChatChunk> {
        let index = *self.tool_index.get(&output_index)?;
        Some(self.chunk(Delta {
            tool_calls: Some(vec![ToolCallDelta {
                index,
                function: Some(FunctionCallDelta {
                    name: None,
                    arguments: Some(arguments),
                }),
                ..ToolCallDelta::default()
            }]),
            ..Delta::default()
        }))
    }
}

/// The item a Chat Completions stream is writing into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    None,
    Reasoning(u64),
    /// A message at this output index, writing this content index, which
    /// is a refusal part when `refusal`.
    Message {
        output_index: u64,
        content_index: u64,
        refusal: bool,
    },
}

/// Chat Completions chunks in, Open Responses events out, in the spec's
/// order. Chat reasoning text becomes a reasoning summary, content an
/// `output_text` part, a refusal a `refusal` part, and each tool call a
/// `function_call` item. The terminal event waits for [`EventWriter::finish`],
/// because usage arrives after the finish reason.
#[derive(Debug)]
pub struct EventWriter {
    seq: Sequencer,
    response: Response,
    started: bool,
    open: Open,
    tools: BTreeMap<u32, u64>,
    finish_reason: Option<FinishReason>,
    error: Option<ApiError>,
}

impl EventWriter {
    /// A writer for the reply to `request`.
    #[must_use]
    pub fn new(request: &CreateResponse) -> Self {
        Self {
            seq: Sequencer::new(),
            response: Response::from_request(
                String::new(),
                0,
                request.model.clone().unwrap_or_default(),
                request,
            ),
            started: false,
            open: Open::None,
            tools: BTreeMap::new(),
            finish_reason: None,
            error: None,
        }
    }

    /// Translates one chunk.
    pub fn push(&mut self, chunk: &ChatChunk) -> Vec<Event> {
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            self.response.id.clone_from(&chunk.id);
            self.response.created_at = chunk.created;
            if !chunk.model.is_empty() {
                self.response.model.clone_from(&chunk.model);
            }
            out.push(self.lifecycle(Lifecycle::Created));
            out.push(self.lifecycle(Lifecycle::InProgress));
        }
        if let Some(usage) = &chunk.usage {
            self.response.usage = Some(responses_usage(usage));
        }
        if chunk.openagents.is_some() {
            self.response.openagents.clone_from(&chunk.openagents);
        }
        if let Some(error) = &chunk.error {
            self.error = Some(error.clone());
        }
        let Some(choice) = chunk.choices.first() else {
            return out;
        };
        let delta = &choice.delta;
        if let Some(text) = delta.reasoning.as_ref().filter(|text| !text.is_empty()) {
            self.reasoning(text, &mut out);
        }
        if let Some(text) = delta.content.as_ref().filter(|text| !text.is_empty()) {
            self.text(text, false, &mut out);
        }
        if let Some(text) = delta.refusal.as_ref().filter(|text| !text.is_empty()) {
            self.text(text, true, &mut out);
        }
        for call in delta.tool_calls.iter().flatten() {
            self.tool_call(call, &mut out);
        }
        if choice.finish_reason.is_some() {
            self.finish_reason.clone_from(&choice.finish_reason);
        }
        out
    }

    /// Ends the stream: closes open items and sends the terminal event
    /// (after an `error` event when the stream failed).
    pub fn finish(&mut self, completed_at: u64) -> Vec<Event> {
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            out.push(self.lifecycle(Lifecycle::Created));
        }
        self.close_open(&mut out);
        for output_index in self.tools.values().copied().collect::<Vec<_>>() {
            self.close_item(output_index, &mut out);
        }
        self.tools.clear();
        let (mut status, incomplete) = status_from_finish(self.finish_reason.as_ref());
        if self.error.is_some() {
            status = ResponseStatus::Failed;
        }
        if status == ResponseStatus::Failed {
            let error = self.error.take().unwrap_or_else(|| {
                ApiError::new(ErrorType::ModelError, "The model failed while answering.")
            });
            self.response.error = Some(error.response_error());
            out.push(self.seq.stamp(EventBody::Error(ErrorEvent {
                error,
                extra: Extra::new(),
            })));
        }
        finish_response(&mut self.response, status.clone(), incomplete, completed_at);
        let kind = match status {
            ResponseStatus::Incomplete => Lifecycle::Incomplete,
            ResponseStatus::Failed => Lifecycle::Failed,
            _ => Lifecycle::Completed,
        };
        out.push(self.lifecycle(kind));
        out
    }

    fn lifecycle(&mut self, kind: Lifecycle) -> Event {
        self.seq
            .stamp(EventBody::lifecycle(kind, self.response.clone()))
    }

    fn add_item(&mut self, item: Item, out: &mut Vec<Event>) -> u64 {
        let output_index = self.response.output.len() as u64;
        self.response.output.push(item.clone());
        out.push(self.seq.stamp(EventBody::OutputItemAdded(ItemEvent {
            output_index,
            item,
            extra: Extra::new(),
        })));
        output_index
    }

    fn item_id(&self, output_index: u64) -> String {
        self.response.output[output_index as usize]
            .id()
            .unwrap_or_default()
            .to_owned()
    }

    fn reasoning(&mut self, text: &str, out: &mut Vec<Event>) {
        let output_index = match self.open {
            Open::Reasoning(index) => index,
            _ => {
                self.close_open(out);
                let id = derived_id("rs", &self.response.id, self.response.output.len());
                let index = self.add_item(
                    Item::Reasoning(Reasoning {
                        id: Some(id),
                        status: Some(ItemStatus::InProgress),
                        ..Reasoning::default()
                    }),
                    out,
                );
                let item_id = self.item_id(index);
                if let Item::Reasoning(reasoning) = &mut self.response.output[index as usize] {
                    reasoning
                        .summary
                        .push(ContentPart::SummaryText(TextPart::default()));
                }
                out.push(
                    self.seq
                        .stamp(EventBody::ReasoningSummaryPartAdded(SummaryPartEvent {
                            item_id,
                            output_index: index,
                            summary_index: 0,
                            part: ContentPart::SummaryText(TextPart::default()),
                            extra: Extra::new(),
                        })),
                );
                self.open = Open::Reasoning(index);
                index
            }
        };
        if let Item::Reasoning(reasoning) = &mut self.response.output[output_index as usize]
            && let Some(part) = reasoning
                .summary
                .first_mut()
                .and_then(ContentPart::text_mut)
        {
            part.push_str(text);
        }
        let item_id = self.item_id(output_index);
        out.push(
            self.seq
                .stamp(EventBody::ReasoningSummaryTextDelta(SummaryDelta {
                    item_id,
                    output_index,
                    summary_index: 0,
                    delta: text.to_owned(),
                    obfuscation: None,
                    extra: Extra::new(),
                })),
        );
    }

    fn text(&mut self, text: &str, refusal: bool, out: &mut Vec<Event>) {
        let (output_index, content_index) = match self.open {
            Open::Message {
                output_index,
                content_index,
                refusal: open_refusal,
            } if open_refusal == refusal => (output_index, content_index),
            Open::Message {
                output_index,
                content_index,
                ..
            } => {
                self.close_part(output_index, content_index, out);
                let next = content_index + 1;
                self.open_part(output_index, next, refusal, out);
                (output_index, next)
            }
            _ => {
                self.close_open(out);
                let id = derived_id("msg", &self.response.id, self.response.output.len());
                let index = self.add_item(
                    Item::Message(Message {
                        id: Some(id),
                        status: Some(ItemStatus::InProgress),
                        role: Role::Assistant,
                        content: MessageContent::Parts(Vec::new()),
                        phase: None,
                        extra: Extra::new(),
                    }),
                    out,
                );
                self.open_part(index, 0, refusal, out);
                (index, 0)
            }
        };
        if let Item::Message(message) = &mut self.response.output[output_index as usize]
            && let MessageContent::Parts(parts) = &mut message.content
            && let Some(part) = parts
                .get_mut(content_index as usize)
                .and_then(ContentPart::text_mut)
        {
            part.push_str(text);
        }
        let item_id = self.item_id(output_index);
        let delta = ContentDelta {
            item_id,
            output_index,
            content_index,
            delta: text.to_owned(),
            logprobs: (!refusal).then(Vec::new),
            obfuscation: None,
            extra: Extra::new(),
        };
        out.push(self.seq.stamp(if refusal {
            EventBody::RefusalDelta(delta)
        } else {
            EventBody::OutputTextDelta(delta)
        }));
    }

    fn open_part(
        &mut self,
        output_index: u64,
        content_index: u64,
        refusal: bool,
        out: &mut Vec<Event>,
    ) {
        let part = if refusal {
            ContentPart::Refusal(Refusal::default())
        } else {
            ContentPart::OutputText(OutputText::new(""))
        };
        if let Item::Message(message) = &mut self.response.output[output_index as usize]
            && let MessageContent::Parts(parts) = &mut message.content
        {
            parts.push(part.clone());
        }
        let item_id = self.item_id(output_index);
        out.push(self.seq.stamp(EventBody::ContentPartAdded(PartEvent {
            item_id,
            output_index,
            content_index,
            part,
            extra: Extra::new(),
        })));
        self.open = Open::Message {
            output_index,
            content_index,
            refusal,
        };
    }

    fn close_part(&mut self, output_index: u64, content_index: u64, out: &mut Vec<Event>) {
        let item_id = self.item_id(output_index);
        let part = match &self.response.output[output_index as usize] {
            Item::Message(Message {
                content: MessageContent::Parts(parts),
                ..
            }) => parts.get(content_index as usize).cloned(),
            _ => None,
        };
        let Some(part) = part else { return };
        let body = match &part {
            ContentPart::Refusal(refusal) => EventBody::RefusalDone(RefusalDone {
                item_id: item_id.clone(),
                output_index,
                content_index,
                refusal: refusal.refusal.clone(),
                extra: Extra::new(),
            }),
            other => EventBody::OutputTextDone(ContentDone {
                item_id: item_id.clone(),
                output_index,
                content_index,
                text: other.text().unwrap_or_default().to_owned(),
                logprobs: Some(Vec::new()),
                extra: Extra::new(),
            }),
        };
        out.push(self.seq.stamp(body));
        out.push(self.seq.stamp(EventBody::ContentPartDone(PartEvent {
            item_id,
            output_index,
            content_index,
            part,
            extra: Extra::new(),
        })));
    }

    fn close_open(&mut self, out: &mut Vec<Event>) {
        match std::mem::replace(&mut self.open, Open::None) {
            Open::None => {}
            Open::Reasoning(output_index) => {
                let item_id = self.item_id(output_index);
                if let Item::Reasoning(reasoning) = &self.response.output[output_index as usize] {
                    let part = reasoning
                        .summary
                        .first()
                        .cloned()
                        .unwrap_or_else(|| ContentPart::SummaryText(TextPart::default()));
                    let text = reasoning_text(reasoning);
                    out.push(
                        self.seq
                            .stamp(EventBody::ReasoningSummaryTextDone(SummaryDone {
                                item_id: item_id.clone(),
                                output_index,
                                summary_index: 0,
                                text,
                                extra: Extra::new(),
                            })),
                    );
                    out.push(self.seq.stamp(EventBody::ReasoningSummaryPartDone(
                        SummaryPartEvent {
                            item_id,
                            output_index,
                            summary_index: 0,
                            part,
                            extra: Extra::new(),
                        },
                    )));
                }
                self.close_item(output_index, out);
            }
            Open::Message {
                output_index,
                content_index,
                ..
            } => {
                self.close_part(output_index, content_index, out);
                self.close_item(output_index, out);
            }
        }
    }

    fn tool_call(&mut self, call: &ToolCallDelta, out: &mut Vec<Event>) {
        let output_index = match self.tools.get(&call.index) {
            Some(index) => *index,
            None => {
                self.close_open(out);
                let call_id = call.id.clone().unwrap_or_default();
                let id = derived_id("fc", &self.response.id, self.response.output.len());
                let index = self.add_item(
                    Item::FunctionCall(FunctionCall {
                        id: Some(id),
                        status: Some(ItemStatus::InProgress),
                        call_id,
                        name: call
                            .function
                            .as_ref()
                            .and_then(|function| function.name.clone())
                            .unwrap_or_default(),
                        arguments: String::new(),
                        extra: Extra::new(),
                    }),
                    out,
                );
                self.tools.insert(call.index, index);
                index
            }
        };
        let Some(arguments) = call
            .function
            .as_ref()
            .and_then(|function| function.arguments.clone())
            .filter(|arguments| !arguments.is_empty())
        else {
            return;
        };
        if let Item::FunctionCall(item) = &mut self.response.output[output_index as usize] {
            item.arguments.push_str(&arguments);
        }
        let item_id = self.item_id(output_index);
        out.push(
            self.seq
                .stamp(EventBody::FunctionCallArgumentsDelta(ArgumentsDelta {
                    item_id,
                    output_index,
                    delta: arguments,
                    obfuscation: None,
                    extra: Extra::new(),
                })),
        );
    }

    fn close_item(&mut self, output_index: u64, out: &mut Vec<Event>) {
        let item_id = self.item_id(output_index);
        if let Item::FunctionCall(call) = &self.response.output[output_index as usize] {
            let arguments = call.arguments.clone();
            out.push(
                self.seq
                    .stamp(EventBody::FunctionCallArgumentsDone(ArgumentsDone {
                        item_id,
                        output_index,
                        arguments,
                        extra: Extra::new(),
                    })),
            );
        }
        let incomplete = matches!(
            self.finish_reason,
            Some(FinishReason::Length | FinishReason::ContentFilter)
        ) && output_index as usize + 1 == self.response.output.len();
        let item = &mut self.response.output[output_index as usize];
        item.set_status(if incomplete {
            ItemStatus::Incomplete
        } else {
            ItemStatus::Completed
        });
        let item = item.clone();
        out.push(self.seq.stamp(EventBody::OutputItemDone(ItemEvent {
            output_index,
            item,
            extra: Extra::new(),
        })));
    }
}

/// Folds a chunk stream into the reply it describes.
#[derive(Debug, Default)]
pub struct CompletionBuilder {
    id: String,
    created: u64,
    model: String,
    message: AssistantMessage,
    tools: BTreeMap<u32, ChatToolCall>,
    finish_reason: Option<FinishReason>,
    usage: Option<ChatUsage>,
    openagents: Option<ResponseInfo>,
    error: Option<ApiError>,
}

impl CompletionBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one chunk.
    pub fn push(&mut self, chunk: &ChatChunk) {
        if self.id.is_empty() {
            self.id.clone_from(&chunk.id);
            self.created = chunk.created;
            self.model.clone_from(&chunk.model);
        }
        if chunk.usage.is_some() {
            self.usage.clone_from(&chunk.usage);
        }
        if chunk.openagents.is_some() {
            self.openagents.clone_from(&chunk.openagents);
        }
        if chunk.error.is_some() {
            self.error.clone_from(&chunk.error);
        }
        let Some(choice) = chunk.choices.first() else {
            return;
        };
        let delta = &choice.delta;
        if let Some(text) = &delta.content {
            self.message
                .content
                .get_or_insert_with(String::new)
                .push_str(text);
        }
        if let Some(text) = &delta.refusal {
            self.message
                .refusal
                .get_or_insert_with(String::new)
                .push_str(text);
        }
        if let Some(text) = &delta.reasoning {
            self.message
                .reasoning
                .get_or_insert_with(String::new)
                .push_str(text);
        }
        for call in delta.tool_calls.iter().flatten() {
            let entry = self
                .tools
                .entry(call.index)
                .or_insert_with(|| ChatToolCall {
                    id: String::new(),
                    kind: "function".to_owned(),
                    function: ChatFunctionCall {
                        name: String::new(),
                        arguments: String::new(),
                    },
                });
            if let Some(id) = &call.id {
                entry.id.clone_from(id);
            }
            if let Some(function) = &call.function {
                if let Some(name) = &function.name {
                    entry.function.name.push_str(name);
                }
                if let Some(arguments) = &function.arguments {
                    entry.function.arguments.push_str(arguments);
                }
            }
        }
        if choice.finish_reason.is_some() {
            self.finish_reason.clone_from(&choice.finish_reason);
        }
    }

    /// The error a failed stream carried.
    #[must_use]
    pub fn error(&self) -> Option<&ApiError> {
        self.error.as_ref()
    }

    /// The reply.
    #[must_use]
    pub fn finish(self) -> Completion {
        let mut message = self.message;
        if !self.tools.is_empty() {
            message.tool_calls = Some(self.tools.into_values().collect());
        }
        // The first chunk's empty content is a role marker, not text.
        if message.content.as_deref() == Some("") && message.tool_calls.is_some() {
            message.content = None;
        }
        Completion {
            id: self.id,
            object: "chat.completion".to_owned(),
            created: self.created,
            model: self.model,
            choices: vec![Choice {
                index: 0,
                message,
                finish_reason: self.finish_reason,
                logprobs: None,
                extra: Extra::new(),
            }],
            usage: self.usage,
            service_tier: None,
            system_fingerprint: None,
            openagents: self.openagents,
            extra: Extra::new(),
        }
    }
}
