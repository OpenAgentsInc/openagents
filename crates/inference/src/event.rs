//! Streaming events.
//!
//! An [`Event`] is a `sequence_number` and a body. Every body kind the
//! spec names (2026-04-24) has a variant; so do our two extension events
//! and OpenAI's `response.reasoning_text.*` names, which upstreams send for
//! the spec's `response.reasoning.*` (see [`EventBody::normalized`]). Any
//! other `type` is kept whole as [`EventBody::Unknown`]: the spec requires
//! clients to ignore events they do not understand.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ApiError;
use crate::item::{ContentPart, Item, LogProb};
use crate::openagents::{CostEvent, RouteEvent};
use crate::response::Response;
use crate::wire::{Extra, tagged_union};

/// `response.created`, `.queued`, `.in_progress`, `.completed`,
/// `.incomplete`, `.failed`: the whole response at that point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseEvent {
    pub response: Box<Response>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.output_item.added` and `.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemEvent {
    pub output_index: u64,
    pub item: Item,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.content_part.added` and `.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartEvent {
    pub item_id: String,
    pub output_index: u64,
    pub content_index: u64,
    pub part: ContentPart,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.reasoning_summary_part.added` and `.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryPartEvent {
    pub item_id: String,
    pub output_index: u64,
    pub summary_index: u64,
    pub part: ContentPart,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A delta to a content part: output text, refusal, or raw reasoning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentDelta {
    pub item_id: String,
    pub output_index: u64,
    pub content_index: u64,
    pub delta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<LogProb>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfuscation: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A content part's final text: `response.output_text.done`,
/// `response.reasoning.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentDone {
    pub item_id: String,
    pub output_index: u64,
    pub content_index: u64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<LogProb>>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.refusal.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefusalDone {
    pub item_id: String,
    pub output_index: u64,
    pub content_index: u64,
    pub refusal: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.reasoning_summary_text.delta`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryDelta {
    pub item_id: String,
    pub output_index: u64,
    pub summary_index: u64,
    pub delta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfuscation: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.reasoning_summary_text.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryDone {
    pub item_id: String,
    pub output_index: u64,
    pub summary_index: u64,
    pub text: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.function_call_arguments.delta`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArgumentsDelta {
    pub item_id: String,
    pub output_index: u64,
    pub delta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfuscation: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.function_call_arguments.done`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArgumentsDone {
    pub item_id: String,
    pub output_index: u64,
    pub arguments: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `response.output_text.annotation.added`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationAdded {
    pub item_id: String,
    pub output_index: u64,
    pub content_index: u64,
    pub annotation_index: u64,
    pub annotation: Value,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `error`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub error: ApiError,
    #[serde(flatten)]
    pub extra: Extra,
}

/// An event's body, by `type`.
#[derive(Clone, Debug, PartialEq)]
pub enum EventBody {
    Created(ResponseEvent),
    Queued(ResponseEvent),
    InProgress(ResponseEvent),
    Completed(ResponseEvent),
    Incomplete(ResponseEvent),
    Failed(ResponseEvent),
    OutputItemAdded(ItemEvent),
    OutputItemDone(ItemEvent),
    ContentPartAdded(PartEvent),
    ContentPartDone(PartEvent),
    ReasoningSummaryPartAdded(SummaryPartEvent),
    ReasoningSummaryPartDone(SummaryPartEvent),
    OutputTextDelta(ContentDelta),
    OutputTextDone(ContentDone),
    OutputTextAnnotationAdded(AnnotationAdded),
    RefusalDelta(ContentDelta),
    RefusalDone(RefusalDone),
    ReasoningDelta(ContentDelta),
    ReasoningDone(ContentDone),
    /// OpenAI's name for [`EventBody::ReasoningDelta`]. We never emit it.
    ReasoningTextDelta(ContentDelta),
    /// OpenAI's name for [`EventBody::ReasoningDone`]. We never emit it.
    ReasoningTextDone(ContentDone),
    ReasoningSummaryTextDelta(SummaryDelta),
    ReasoningSummaryTextDone(SummaryDone),
    FunctionCallArgumentsDelta(ArgumentsDelta),
    FunctionCallArgumentsDone(ArgumentsDone),
    Error(ErrorEvent),
    /// `openagents:route`.
    Route(RouteEvent),
    /// `openagents:cost`.
    Cost(CostEvent),
    /// Any other event, kept whole (its `type` included).
    Unknown(Value),
}

tagged_union!(EventBody {
    "response.created" => Created,
    "response.queued" => Queued,
    "response.in_progress" => InProgress,
    "response.completed" => Completed,
    "response.incomplete" => Incomplete,
    "response.failed" => Failed,
    "response.output_item.added" => OutputItemAdded,
    "response.output_item.done" => OutputItemDone,
    "response.content_part.added" => ContentPartAdded,
    "response.content_part.done" => ContentPartDone,
    "response.reasoning_summary_part.added" => ReasoningSummaryPartAdded,
    "response.reasoning_summary_part.done" => ReasoningSummaryPartDone,
    "response.output_text.delta" => OutputTextDelta,
    "response.output_text.done" => OutputTextDone,
    "response.output_text.annotation.added" => OutputTextAnnotationAdded,
    "response.refusal.delta" => RefusalDelta,
    "response.refusal.done" => RefusalDone,
    "response.reasoning.delta" => ReasoningDelta,
    "response.reasoning.done" => ReasoningDone,
    "response.reasoning_text.delta" => ReasoningTextDelta,
    "response.reasoning_text.done" => ReasoningTextDone,
    "response.reasoning_summary_text.delta" => ReasoningSummaryTextDelta,
    "response.reasoning_summary_text.done" => ReasoningSummaryTextDone,
    "response.function_call_arguments.delta" => FunctionCallArgumentsDelta,
    "response.function_call_arguments.done" => FunctionCallArgumentsDone,
    "error" => Error,
    "openagents:route" => Route,
    "openagents:cost" => Cost,
});

impl EventBody {
    /// The same event under the spec's name: OpenAI's
    /// `response.reasoning_text.*` become `response.reasoning.*`.
    #[must_use]
    pub fn normalized(self) -> Self {
        match self {
            Self::ReasoningTextDelta(body) => Self::ReasoningDelta(body),
            Self::ReasoningTextDone(body) => Self::ReasoningDone(body),
            other => other,
        }
    }

    /// The response a lifecycle event carries.
    #[must_use]
    pub fn response(&self) -> Option<&Response> {
        match self {
            Self::Created(event)
            | Self::Queued(event)
            | Self::InProgress(event)
            | Self::Completed(event)
            | Self::Incomplete(event)
            | Self::Failed(event) => Some(&event.response),
            _ => None,
        }
    }

    /// Whether this is `response.completed`, `.incomplete`, or `.failed`.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed(_) | Self::Incomplete(_) | Self::Failed(_)
        )
    }

    /// A lifecycle event of this `type` carrying `response`.
    #[must_use]
    pub fn lifecycle(kind: Lifecycle, response: Response) -> Self {
        let event = ResponseEvent {
            response: Box::new(response),
            extra: Extra::new(),
        };
        match kind {
            Lifecycle::Created => Self::Created(event),
            Lifecycle::Queued => Self::Queued(event),
            Lifecycle::InProgress => Self::InProgress(event),
            Lifecycle::Completed => Self::Completed(event),
            Lifecycle::Incomplete => Self::Incomplete(event),
            Lifecycle::Failed => Self::Failed(event),
        }
    }
}

/// The lifecycle event kinds, for [`EventBody::lifecycle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Created,
    Queued,
    InProgress,
    Completed,
    Incomplete,
    Failed,
}

/// One streaming event.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub sequence_number: u64,
    pub body: EventBody,
}

impl Event {
    /// The event's `type`.
    #[must_use]
    pub fn type_name(&self) -> &str {
        self.body.type_name()
    }
}

impl Serialize for Event {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Value::Object(body) =
            serde_json::to_value(&self.body).map_err(serde::ser::Error::custom)?
        else {
            return Err(serde::ser::Error::custom("event body must be an object"));
        };
        let mut out = Map::with_capacity(body.len() + 1);
        let mut body = body.into_iter();
        // `type` first, then `sequence_number`, then the rest.
        if let Some((key, value)) = body.next() {
            out.insert(key, value);
        }
        out.insert(
            "sequence_number".to_owned(),
            Value::from(self.sequence_number),
        );
        out.extend(body);
        Value::Object(out).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut map) = value else {
            return Err(D::Error::custom("event must be a JSON object"));
        };
        let sequence_number = map
            .shift_remove("sequence_number")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| D::Error::custom("event has no integer `sequence_number`"))?;
        let body = EventBody::deserialize(Value::Object(map)).map_err(D::Error::custom)?;
        Ok(Self {
            sequence_number,
            body,
        })
    }
}
