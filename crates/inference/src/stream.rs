//! Working with an Open Responses event stream: stamping sequence
//! numbers, folding events into a response, and checking a stream against
//! the spec's order.

use std::collections::BTreeMap;

use crate::event::{Event, EventBody};
use crate::item::{ContentPart, Item, MessageContent, Reasoning, TextPart};
use crate::response::Response;
use crate::sse::StreamItem;

/// Hands out `sequence_number`s from zero.
#[derive(Debug, Default)]
pub struct Sequencer {
    next: u64,
}

impl Sequencer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next event.
    pub fn stamp(&mut self, body: EventBody) -> Event {
        let sequence_number = self.next;
        self.next += 1;
        Event {
            sequence_number,
            body,
        }
    }
}

/// Folds a stream's events into the response they describe.
///
/// Items are built from `output_item.added`, parts, and deltas, and
/// replaced by `output_item.done`'s copy; lifecycle events update the
/// rest. A stream cut off before its terminal event still yields what
/// arrived.
#[derive(Debug, Default)]
pub struct Accumulator {
    response: Option<Response>,
    output: Vec<Item>,
    terminal: bool,
}

impl Accumulator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event.
    pub fn push(&mut self, event: &Event) {
        let body = event.body.clone().normalized();
        if let Some(response) = body.response() {
            let mut response = response.clone();
            if body.is_terminal() {
                self.terminal = true;
                // A terminal response with no output (some upstreams send
                // it bare) keeps what the stream built.
                if response.output.is_empty() {
                    response.output = self.output.clone();
                }
            }
            self.response = Some(response);
            return;
        }
        match body {
            EventBody::OutputItemAdded(added) => {
                let index = to_index(added.output_index);
                if self.output.len() <= index {
                    self.output
                        .resize(index + 1, Item::Unknown(serde_json::Value::Null));
                }
                self.output[index] = added.item;
            }
            EventBody::OutputItemDone(done) => {
                let index = to_index(done.output_index);
                if self.output.len() <= index {
                    self.output
                        .resize(index + 1, Item::Unknown(serde_json::Value::Null));
                }
                self.output[index] = done.item;
            }
            EventBody::ContentPartAdded(added) => {
                if let Some(parts) = self.content_parts(added.output_index) {
                    set_at(parts, added.content_index, added.part);
                }
            }
            EventBody::ContentPartDone(done) => {
                if let Some(parts) = self.content_parts(done.output_index) {
                    set_at(parts, done.content_index, done.part);
                }
            }
            EventBody::ReasoningSummaryPartAdded(added) => {
                if let Some(Item::Reasoning(reasoning)) = self.item(added.output_index) {
                    set_at(&mut reasoning.summary, added.summary_index, added.part);
                }
            }
            EventBody::ReasoningSummaryPartDone(done) => {
                if let Some(Item::Reasoning(reasoning)) = self.item(done.output_index) {
                    set_at(&mut reasoning.summary, done.summary_index, done.part);
                }
            }
            EventBody::OutputTextDelta(delta)
            | EventBody::RefusalDelta(delta)
            | EventBody::ReasoningDelta(delta) => {
                let reasoning = matches!(self.item(delta.output_index), Some(Item::Reasoning(_)));
                if let Some(parts) = self.content_parts(delta.output_index) {
                    if parts.len() <= to_index(delta.content_index) && reasoning {
                        // Some upstreams stream raw reasoning without
                        // announcing its part.
                        set_at(
                            parts,
                            delta.content_index,
                            ContentPart::ReasoningText(TextPart::default()),
                        );
                    }
                    if let Some(text) = parts
                        .get_mut(to_index(delta.content_index))
                        .and_then(ContentPart::text_mut)
                    {
                        text.push_str(&delta.delta);
                    }
                }
            }
            EventBody::ReasoningSummaryTextDelta(delta) => {
                if let Some(Item::Reasoning(reasoning)) = self.item(delta.output_index) {
                    if reasoning.summary.len() <= to_index(delta.summary_index) {
                        set_at(
                            &mut reasoning.summary,
                            delta.summary_index,
                            ContentPart::SummaryText(TextPart::default()),
                        );
                    }
                    if let Some(text) = reasoning.summary[to_index(delta.summary_index)].text_mut()
                    {
                        text.push_str(&delta.delta);
                    }
                }
            }
            EventBody::FunctionCallArgumentsDelta(delta) => {
                if let Some(Item::FunctionCall(call)) = self.item(delta.output_index) {
                    call.arguments.push_str(&delta.delta);
                }
            }
            EventBody::FunctionCallArgumentsDone(done) => {
                if let Some(Item::FunctionCall(call)) = self.item(done.output_index) {
                    call.arguments = done.arguments;
                }
            }
            _ => {}
        }
    }

    /// The items built so far.
    #[must_use]
    pub fn output(&self) -> &[Item] {
        &self.output
    }

    /// Whether a terminal event arrived.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.terminal
    }

    /// The response: the last lifecycle event's copy, its output replaced
    /// by the built items when the stream ended early.
    #[must_use]
    pub fn finish(self) -> Option<Response> {
        let mut response = self.response?;
        if !self.terminal {
            response.output = self.output;
        }
        Some(response)
    }

    fn item(&mut self, output_index: u64) -> Option<&mut Item> {
        self.output.get_mut(to_index(output_index))
    }

    fn content_parts(&mut self, output_index: u64) -> Option<&mut Vec<ContentPart>> {
        match self.item(output_index)? {
            Item::Message(message) => {
                if let MessageContent::Text(text) = &message.content {
                    message.content = MessageContent::Parts(vec![ContentPart::OutputText(
                        crate::item::OutputText::new(text.clone()),
                    )]);
                }
                match &mut message.content {
                    MessageContent::Parts(parts) => Some(parts),
                    MessageContent::Text(_) => None,
                }
            }
            Item::Reasoning(Reasoning { content, .. }) => {
                Some(content.get_or_insert_with(Vec::new))
            }
            _ => None,
        }
    }
}

fn to_index(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn set_at(parts: &mut Vec<ContentPart>, index: u64, part: ContentPart) {
    let index = to_index(index);
    if index == usize::MAX {
        return;
    }
    while parts.len() < index {
        parts.push(ContentPart::Unknown(serde_json::Value::Null));
    }
    if parts.len() == index {
        parts.push(part);
    } else {
        parts[index] = part;
    }
}

/// One way a stream departed from the spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The event it was found at; `None` for the stream as a whole.
    pub sequence_number: Option<u64>,
    pub what: String,
}

/// Checks a stream against the spec's ordering rules:
///
/// - sequence numbers increase;
/// - the first event is `response.created`;
/// - items are added before their parts, parts before their deltas, and
///   each `*.done` text equals its deltas;
/// - `openagents:route` comes before the first item, `openagents:cost`
///   before the terminal event;
/// - exactly one terminal event, with nothing after it but `[DONE]`;
/// - the stream ends with `[DONE]`.
#[derive(Debug, Default)]
pub struct StreamCheck {
    violations: Vec<Violation>,
    last_sequence: Option<u64>,
    seen_any: bool,
    seen_item: bool,
    terminal: Option<u64>,
    done: bool,
    open_items: BTreeMap<u64, String>,
    open_parts: BTreeMap<(u64, u64), String>,
    summary_text: BTreeMap<(u64, u64), String>,
    arguments: BTreeMap<u64, String>,
}

impl StreamCheck {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks a whole decoded stream.
    #[must_use]
    pub fn run<'a>(items: impl IntoIterator<Item = &'a StreamItem>) -> Vec<Violation> {
        let mut check = Self::new();
        for item in items {
            check.push(item);
        }
        check.finish()
    }

    /// Checks the next stream item.
    pub fn push(&mut self, item: &StreamItem) {
        let event = match item {
            StreamItem::Done => {
                if self.done {
                    self.stream_violation("more than one [DONE]");
                }
                if self.terminal.is_none() {
                    self.stream_violation("[DONE] before a terminal event");
                }
                self.done = true;
                return;
            }
            StreamItem::Event(event) => event,
        };
        let seq = event.sequence_number;
        if self.done {
            self.violation(seq, "event after [DONE]");
        }
        if let Some(last) = self.last_sequence
            && seq <= last
        {
            self.violation(seq, format!("sequence_number {seq} does not follow {last}"));
        }
        self.last_sequence = Some(seq);
        if !self.seen_any && !matches!(event.body, EventBody::Created(_)) {
            self.violation(
                seq,
                format!("first event is {}, not response.created", event.type_name()),
            );
        }
        self.seen_any = true;
        if let Some(terminal) = self.terminal {
            self.violation(
                seq,
                format!("{} after the terminal event {terminal}", event.type_name()),
            );
        }
        match &event.body.clone().normalized() {
            EventBody::Created(_) | EventBody::Queued(_) | EventBody::InProgress(_) => {}
            body if body.is_terminal() => {
                self.terminal = Some(seq);
                for (index, kind) in std::mem::take(&mut self.open_items) {
                    self.violation(
                        seq,
                        format!("{kind} item {index} never got output_item.done"),
                    );
                }
            }
            EventBody::Route(_) => {
                if self.seen_item {
                    self.violation(seq, "openagents:route after the first output item");
                }
            }
            EventBody::OutputItemAdded(added) => {
                self.seen_item = true;
                if self
                    .open_items
                    .insert(added.output_index, added.item.type_name().to_owned())
                    .is_some()
                {
                    self.violation(seq, format!("item {} added twice", added.output_index));
                }
            }
            EventBody::OutputItemDone(done) => {
                if self.open_items.remove(&done.output_index).is_none() {
                    self.violation(
                        seq,
                        format!("item {} done but never added", done.output_index),
                    );
                }
                let open: Vec<_> = self
                    .open_parts
                    .keys()
                    .filter(|(item, _)| *item == done.output_index)
                    .copied()
                    .collect();
                for key in open {
                    self.open_parts.remove(&key);
                    self.violation(seq, format!("part {key:?} never got content_part.done"));
                }
            }
            EventBody::ContentPartAdded(added) => {
                self.require_item(seq, added.output_index);
                self.open_parts
                    .insert((added.output_index, added.content_index), String::new());
            }
            EventBody::ContentPartDone(done) => {
                let key = (done.output_index, done.content_index);
                match self.open_parts.remove(&key) {
                    None => self.violation(seq, format!("part {key:?} done but never added")),
                    Some(text) => {
                        if let Some(final_text) = done.part.text()
                            && final_text != text
                        {
                            self.violation(
                                seq,
                                format!("part {key:?} text differs from its deltas"),
                            );
                        }
                    }
                }
            }
            EventBody::OutputTextDelta(delta)
            | EventBody::RefusalDelta(delta)
            | EventBody::ReasoningDelta(delta) => {
                let key = (delta.output_index, delta.content_index);
                match self.open_parts.get_mut(&key) {
                    Some(text) => text.push_str(&delta.delta),
                    None => self.violation(
                        seq,
                        format!(
                            "{} for part {key:?} without content_part.added",
                            event.type_name()
                        ),
                    ),
                }
            }
            EventBody::OutputTextDone(done) | EventBody::ReasoningDone(done) => {
                self.text_done(seq, (done.output_index, done.content_index), &done.text);
            }
            EventBody::RefusalDone(done) => {
                self.text_done(seq, (done.output_index, done.content_index), &done.refusal);
            }
            EventBody::ReasoningSummaryTextDelta(delta) => {
                self.require_item(seq, delta.output_index);
                self.summary_text
                    .entry((delta.output_index, delta.summary_index))
                    .or_default()
                    .push_str(&delta.delta);
            }
            EventBody::ReasoningSummaryTextDone(done) => {
                let built = self
                    .summary_text
                    .remove(&(done.output_index, done.summary_index))
                    .unwrap_or_default();
                if built != done.text {
                    self.violation(seq, "reasoning summary text differs from its deltas");
                }
            }
            EventBody::FunctionCallArgumentsDelta(delta) => {
                self.require_item(seq, delta.output_index);
                self.arguments
                    .entry(delta.output_index)
                    .or_default()
                    .push_str(&delta.delta);
            }
            EventBody::FunctionCallArgumentsDone(done) => {
                let built = self
                    .arguments
                    .remove(&done.output_index)
                    .unwrap_or_default();
                if built != done.arguments {
                    self.violation(seq, "function call arguments differ from their deltas");
                }
            }
            _ => {}
        }
    }

    /// Ends the stream and returns what was found.
    #[must_use]
    pub fn finish(mut self) -> Vec<Violation> {
        if self.terminal.is_none() {
            self.stream_violation("stream ended without a terminal event");
        }
        if !self.done {
            self.stream_violation("stream ended without [DONE]");
        }
        self.violations
    }

    fn text_done(&mut self, seq: u64, key: (u64, u64), text: &str) {
        match self.open_parts.get(&key) {
            Some(built) if built != text => {
                self.violation(
                    seq,
                    format!("part {key:?} done text differs from its deltas"),
                );
            }
            Some(_) => {}
            // Deltas without an announced part were already reported.
            None => {}
        }
    }

    fn require_item(&mut self, seq: u64, output_index: u64) {
        if !self.open_items.contains_key(&output_index) {
            self.violation(
                seq,
                format!("event for item {output_index}, which is not open"),
            );
        }
    }

    fn violation(&mut self, seq: u64, what: impl Into<String>) {
        self.violations.push(Violation {
            sequence_number: Some(seq),
            what: what.into(),
        });
    }

    fn stream_violation(&mut self, what: impl Into<String>) {
        self.violations.push(Violation {
            sequence_number: None,
            what: what.into(),
        });
    }
}
