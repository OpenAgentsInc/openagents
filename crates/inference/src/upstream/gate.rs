//! The rule every adapter's stream follows at the first token, and the
//! repairs that make an upstream's stream follow the spec's order.
//!
//! A [`Gate`] holds `response.created`, `response.queued`, and
//! `response.in_progress` until the first output item, so that a failure
//! before the first token reaches the router as one `Err` with nothing
//! sent ahead of it, and the router can fall back. After the first output
//! item everything passes, and a failure is the caller's: an upstream
//! `error` or `response.failed` passes through, and a stream that breaks
//! or ends without a terminal event gets its open parts and items closed
//! and a `response.failed` built from what arrived. A terminal event with
//! no output before it is an empty stream, which the router also falls
//! back on.
//!
//! Repairs, so what we serve passes [`crate::stream::StreamCheck`]: every
//! event is restamped with our own `sequence_number`; a delta for a part
//! the upstream never announced (Vercel streams raw reasoning that way)
//! gets its `content_part.added` first; and an item closed with parts
//! still open gets their `content_part.done` first.

use std::collections::{BTreeMap, BTreeSet};

use super::http::clean;
use super::{AttemptError, ErrorClass};
use crate::error::ResponseError;
use crate::event::{Event, EventBody, ItemEvent, Lifecycle, PartEvent};
use crate::item::{ContentPart, ItemStatus, OutputText, Refusal, TextPart};
use crate::request::CreateResponse;
use crate::response::{Response, ResponseStatus};
use crate::stream::{Accumulator, Sequencer};
use crate::wire::Extra;

/// The first-token gate for one stream.
pub struct Gate {
    model: String,
    scrub: &'static [&'static str],
    held: Vec<EventBody>,
    output_started: bool,
    closed: bool,
    last: Option<Response>,
    sequencer: Sequencer,
    accumulator: Accumulator,
    open_items: BTreeSet<u64>,
    /// Open parts: (output index, content index) to (item id, text so far,
    /// kind).
    open_parts: BTreeMap<(u64, u64), (String, String, PartKind)>,
    pending: std::collections::VecDeque<Result<Event, AttemptError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartKind {
    Text,
    Refusal,
    Reasoning,
    Other,
}

impl PartKind {
    fn of(part: &ContentPart) -> Self {
        match part {
            ContentPart::OutputText(_) => Self::Text,
            ContentPart::Refusal(_) => Self::Refusal,
            ContentPart::ReasoningText(_) => Self::Reasoning,
            _ => Self::Other,
        }
    }

    fn part(self, text: &str) -> ContentPart {
        match self {
            Self::Text => ContentPart::OutputText(OutputText::new(text)),
            Self::Refusal => ContentPart::Refusal(Refusal {
                refusal: text.to_owned(),
                extra: Extra::new(),
            }),
            Self::Reasoning | Self::Other => ContentPart::ReasoningText(TextPart::new(text)),
        }
    }
}

fn lifecycle_response(body: &mut EventBody) -> Option<&mut Response> {
    match body {
        EventBody::Created(event)
        | EventBody::Queued(event)
        | EventBody::InProgress(event)
        | EventBody::Completed(event)
        | EventBody::Incomplete(event)
        | EventBody::Failed(event) => Some(&mut event.response),
        _ => None,
    }
}

impl Gate {
    /// A gate for a stream answering as `model` (the public id). `scrub`
    /// lists words removed from upstream error messages.
    #[must_use]
    pub fn new(model: &str, scrub: &'static [&'static str]) -> Self {
        Self {
            model: model.to_owned(),
            scrub,
            held: Vec::new(),
            output_started: false,
            closed: false,
            last: None,
            sequencer: Sequencer::new(),
            accumulator: Accumulator::new(),
            open_items: BTreeSet::new(),
            open_parts: BTreeMap::new(),
            pending: std::collections::VecDeque::new(),
        }
    }

    /// Whether the first output item has passed.
    #[must_use]
    pub fn output_started(&self) -> bool {
        self.output_started
    }

    /// Whether the stream is over: nothing more will be accepted.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The next item to hand the reader.
    pub fn next(&mut self) -> Option<Result<Event, AttemptError>> {
        self.pending.pop_front()
    }

    fn emit(&mut self, body: EventBody) {
        let event = self.sequencer.stamp(body);
        self.accumulator.push(&event);
        self.pending.push_back(Ok(event));
    }

    /// Takes one upstream event.
    pub fn push(&mut self, event: Event) {
        if self.closed {
            return;
        }
        let mut body = event.body.normalized();
        if let Some(response) = lifecycle_response(&mut body) {
            response.model.clone_from(&self.model);
            self.last = Some(response.clone());
        }
        if !self.output_started {
            match &body {
                EventBody::Error(error) => {
                    let message = clean(&error.error.message, self.scrub);
                    return self.fail(AttemptError::new(ErrorClass::Upstream, message));
                }
                EventBody::Failed(failed) => {
                    let message = failed.response.error.as_ref().map_or_else(
                        || "the upstream failed".to_owned(),
                        |error| clean(&error.message, self.scrub),
                    );
                    return self.fail(AttemptError::new(ErrorClass::Upstream, message));
                }
                EventBody::Completed(_) | EventBody::Incomplete(_) => {
                    return self.fail(AttemptError::new(
                        ErrorClass::Empty,
                        "the upstream finished with no output",
                    ));
                }
                EventBody::OutputItemAdded(_) => {
                    self.output_started = true;
                    for held in std::mem::take(&mut self.held) {
                        self.emit(held);
                    }
                }
                _ => {
                    self.held.push(body);
                    return;
                }
            }
        }
        self.repair_and_emit(body);
    }

    fn repair_and_emit(&mut self, body: EventBody) {
        match &body {
            EventBody::OutputItemAdded(added) => {
                self.open_items.insert(added.output_index);
            }
            EventBody::ContentPartAdded(added) => {
                self.open_parts.insert(
                    (added.output_index, added.content_index),
                    (
                        added.item_id.clone(),
                        String::new(),
                        PartKind::of(&added.part),
                    ),
                );
            }
            EventBody::ContentPartDone(done) => {
                self.open_parts
                    .remove(&(done.output_index, done.content_index));
            }
            EventBody::OutputTextDelta(delta)
            | EventBody::RefusalDelta(delta)
            | EventBody::ReasoningDelta(delta) => {
                let key = (delta.output_index, delta.content_index);
                if !self.open_parts.contains_key(&key) {
                    let kind = match &body {
                        EventBody::OutputTextDelta(_) => PartKind::Text,
                        EventBody::RefusalDelta(_) => PartKind::Refusal,
                        _ => PartKind::Reasoning,
                    };
                    self.open_parts
                        .insert(key, (delta.item_id.clone(), String::new(), kind));
                    self.emit(EventBody::ContentPartAdded(PartEvent {
                        item_id: delta.item_id.clone(),
                        output_index: delta.output_index,
                        content_index: delta.content_index,
                        part: kind.part(""),
                        extra: Extra::new(),
                    }));
                }
                if let Some((_, text, _)) = self.open_parts.get_mut(&key) {
                    text.push_str(&delta.delta);
                }
            }
            EventBody::OutputItemDone(done) => {
                self.close_parts(done.output_index);
                self.open_items.remove(&done.output_index);
            }
            _ => {}
        }
        let terminal = body.is_terminal();
        self.emit(body);
        if terminal {
            self.closed = true;
        }
    }

    fn close_parts(&mut self, output_index: u64) {
        let keys: Vec<(u64, u64)> = self
            .open_parts
            .keys()
            .filter(|(item, _)| *item == output_index)
            .copied()
            .collect();
        for key in keys {
            if let Some((item_id, text, kind)) = self.open_parts.remove(&key) {
                self.emit(EventBody::ContentPartDone(PartEvent {
                    item_id,
                    output_index: key.0,
                    content_index: key.1,
                    part: kind.part(&text),
                    extra: Extra::new(),
                }));
            }
        }
    }

    /// A failure. Before the first output item it is the router's `Err`;
    /// after, the caller's `response.failed`, with open items closed.
    pub fn fail(&mut self, error: AttemptError) {
        if self.closed {
            return;
        }
        self.closed = true;
        if !self.output_started {
            self.held.clear();
            self.pending.push_back(Err(error));
            return;
        }
        for index in std::mem::take(&mut self.open_items) {
            self.close_parts(index);
            if let Some(mut item) = self.accumulator.output().get(index as usize).cloned() {
                item.set_status(ItemStatus::Incomplete);
                self.emit(EventBody::OutputItemDone(ItemEvent {
                    output_index: index,
                    item,
                    extra: Extra::new(),
                }));
            }
        }
        let mut response = self.last.clone().unwrap_or_else(|| {
            Response::from_request(
                super::emit::fresh_id("resp"),
                super::emit::unix_now(),
                self.model.clone(),
                &CreateResponse::default(),
            )
        });
        response.output = self.accumulator.output().to_vec();
        response.status = ResponseStatus::Failed;
        response.completed_at = Some(super::emit::unix_now());
        response.error = Some(ResponseError {
            code: "upstream_failed".into(),
            message: clean(&error.message, self.scrub),
            extra: Extra::new(),
        });
        self.emit(EventBody::lifecycle(Lifecycle::Failed, response));
    }

    /// The upstream's stream ended.
    pub fn end(&mut self) {
        if self.closed {
            return;
        }
        let message = if self.output_started {
            "the stream ended before the answer finished"
        } else {
            "the stream ended before any output"
        };
        self.fail(AttemptError::new(ErrorClass::Connection, message));
    }
}
