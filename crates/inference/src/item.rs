//! Items and content: the units a request's `input` and a response's
//! `output` are made of.
//!
//! One [`Item`] type serves both directions, as the spec's items are
//! bidirectional. Fields the spec requires only on output (`id`, `status`)
//! are optional here so an input item decodes too; a server emitting items
//! fills them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::wire::{Extra, open_enum, tagged_union};

open_enum! {
    /// An item's lifecycle state. The spec names three and lets
    /// implementers add more (an input item may say `received`).
    pub enum ItemStatus {
        InProgress = "in_progress",
        Completed = "completed",
        Incomplete = "incomplete",
        Failed = "failed",
    }
}

/// Who wrote a message. Closed: the spec allows exactly these four.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

/// An assistant message's phase label (added 2026-04-24).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Commentary,
    FinalAnswer,
}

/// Image detail level. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

/// A text-bearing part: `input_text`, `text`, `summary_text`,
/// `reasoning_text`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextPart {
    pub text: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl TextPart {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            extra: Extra::new(),
        }
    }
}

/// `output_text`: model text with annotations and optional logprobs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputText {
    pub text: String,
    #[serde(default)]
    pub annotations: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<LogProb>>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl OutputText {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

/// One token's log probability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogProb {
    pub token: String,
    pub logprob: f64,
    #[serde(default)]
    pub bytes: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<LogProb>>,
}

/// `refusal`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub refusal: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `input_image`: a URL (including a `data:` URL) and a detail level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputImage {
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `input_file`: inline data or a URL.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `input_video`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputVideo {
    pub video_url: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// One content part, user- or model-written.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentPart {
    InputText(TextPart),
    InputImage(InputImage),
    InputFile(InputFile),
    InputVideo(InputVideo),
    OutputText(OutputText),
    Text(TextPart),
    SummaryText(TextPart),
    ReasoningText(TextPart),
    Refusal(Refusal),
    /// A part type this crate does not name, kept verbatim.
    Unknown(Value),
}

tagged_union!(ContentPart {
    "input_text" => InputText,
    "input_image" => InputImage,
    "input_file" => InputFile,
    "input_video" => InputVideo,
    "output_text" => OutputText,
    "text" => Text,
    "summary_text" => SummaryText,
    "reasoning_text" => ReasoningText,
    "refusal" => Refusal,
});

impl ContentPart {
    /// The part's text, for the text-bearing kinds (a refusal's text
    /// included).
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::InputText(part)
            | Self::Text(part)
            | Self::SummaryText(part)
            | Self::ReasoningText(part) => Some(&part.text),
            Self::OutputText(part) => Some(&part.text),
            Self::Refusal(part) => Some(&part.refusal),
            _ => None,
        }
    }

    /// Mutable access to the same text, for appending deltas.
    pub fn text_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::InputText(part)
            | Self::Text(part)
            | Self::SummaryText(part)
            | Self::ReasoningText(part) => Some(&mut part.text),
            Self::OutputText(part) => Some(&mut part.text),
            Self::Refusal(part) => Some(&mut part.refusal),
            _ => None,
        }
    }
}

/// A message's content: a bare string or a list of parts. Both are legal
/// on input; a server's output uses parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl Default for MessageContent {
    fn default() -> Self {
        Self::Parts(Vec::new())
    }
}

/// A `message` item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,
    pub role: Role,
    pub content: MessageContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Message {
    /// A message with plain string content.
    #[must_use]
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            id: None,
            status: None,
            role,
            content: MessageContent::Text(text.into()),
            phase: None,
            extra: Extra::new(),
        }
    }
}

/// A `function_call` item: the model asking the caller to run a function.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A `function_call_output`'s output: a string or input parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolOutput {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// A `function_call_output` item: the caller's result for a call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCallOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,
    pub call_id: String,
    pub output: ToolOutput,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A `reasoning` item: a summary safe to show, opaque `encrypted_content`
/// for the next turn, and raw `content` only when the upstream allows it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,
    #[serde(default)]
    pub summary: Vec<ContentPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentPart>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Reasoning {
    /// The summary's text, parts joined by blank lines.
    #[must_use]
    pub fn summary_text(&self) -> String {
        join_text(&self.summary)
    }

    /// The raw reasoning text, parts joined by blank lines.
    #[must_use]
    pub fn content_text(&self) -> String {
        self.content.as_deref().map(join_text).unwrap_or_default()
    }
}

fn join_text(parts: &[ContentPart]) -> String {
    parts
        .iter()
        .filter_map(ContentPart::text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// An `item_reference`: an earlier item named by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemReference {
    pub id: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A `compaction` item from `/responses/compact`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compaction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub encrypted_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// One item.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Message(Message),
    FunctionCall(FunctionCall),
    FunctionCallOutput(FunctionCallOutput),
    Reasoning(Reasoning),
    ItemReference(ItemReference),
    Compaction(Compaction),
    /// An item type this crate does not name (a prefixed extension such
    /// as `openai:web_search_call`), kept verbatim.
    Unknown(Value),
}

impl Item {
    /// The `type` string this item carries on the wire.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Message(_) => "message",
            Self::FunctionCall(_) => "function_call",
            Self::FunctionCallOutput(_) => "function_call_output",
            Self::Reasoning(_) => "reasoning",
            Self::ItemReference(_) => "item_reference",
            Self::Compaction(_) => "compaction",
            Self::Unknown(value) => value.get("type").and_then(Value::as_str).unwrap_or(""),
        }
    }

    /// The item's id, if it has one.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Message(item) => item.id.as_deref(),
            Self::FunctionCall(item) => item.id.as_deref(),
            Self::FunctionCallOutput(item) => item.id.as_deref(),
            Self::Reasoning(item) => item.id.as_deref(),
            Self::ItemReference(item) => Some(&item.id),
            Self::Compaction(item) => item.id.as_deref(),
            Self::Unknown(value) => value.get("id").and_then(Value::as_str),
        }
    }

    /// The item's status, if it carries one this crate can read.
    #[must_use]
    pub fn status(&self) -> Option<ItemStatus> {
        match self {
            Self::Message(item) => item.status.clone(),
            Self::FunctionCall(item) => item.status.clone(),
            Self::FunctionCallOutput(item) => item.status.clone(),
            Self::Reasoning(item) => item.status.clone(),
            Self::ItemReference(_) | Self::Compaction(_) => None,
            Self::Unknown(value) => value
                .get("status")
                .and_then(Value::as_str)
                .map(ItemStatus::from),
        }
    }

    /// Sets the status on the kinds that carry one.
    pub fn set_status(&mut self, status: ItemStatus) {
        match self {
            Self::Message(item) => item.status = Some(status),
            Self::FunctionCall(item) => item.status = Some(status),
            Self::FunctionCallOutput(item) => item.status = Some(status),
            Self::Reasoning(item) => item.status = Some(status),
            Self::ItemReference(_) | Self::Compaction(_) => {}
            Self::Unknown(value) => {
                if let Some(map) = value.as_object_mut() {
                    map.insert(
                        "status".to_owned(),
                        Value::String(status.as_str().to_owned()),
                    );
                }
            }
        }
    }
}

impl Serialize for Item {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use crate::wire::tagged;
        let value = match self {
            Self::Message(body) => tagged("message", body),
            Self::FunctionCall(body) => tagged("function_call", body),
            Self::FunctionCallOutput(body) => tagged("function_call_output", body),
            Self::Reasoning(body) => tagged("reasoning", body),
            Self::ItemReference(body) => tagged("item_reference", body),
            Self::Compaction(body) => tagged("compaction", body),
            Self::Unknown(value) => return value.serialize(serializer),
        };
        value
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Item {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use crate::wire::{from_map, rejoin, split_tag};
        use serde::de::Error;
        let value = Value::deserialize(deserializer)?;
        let (tag, map) = split_tag(value, "item").map_err(D::Error::custom)?;
        // The spec's examples send input messages without `type`
        // (`{"role": "user", "content": ...}`), and an item reference's
        // `type` is nullable.
        let tag = tag.or_else(|| {
            if map.contains_key("role") {
                Some("message".to_owned())
            } else if map.len() == 1 && map.contains_key("id") {
                Some("item_reference".to_owned())
            } else {
                None
            }
        });
        let wrap = |kind: &str, error: serde_json::Error| {
            D::Error::custom(format!("{kind} item: {error}"))
        };
        match tag.as_deref() {
            Some("message") => from_map(map)
                .map(Self::Message)
                .map_err(|e| wrap("message", e)),
            Some("function_call") => from_map(map)
                .map(Self::FunctionCall)
                .map_err(|e| wrap("function_call", e)),
            Some("function_call_output") => from_map(map)
                .map(Self::FunctionCallOutput)
                .map_err(|e| wrap("function_call_output", e)),
            Some("reasoning") => from_map(map)
                .map(Self::Reasoning)
                .map_err(|e| wrap("reasoning", e)),
            Some("item_reference") => from_map(map)
                .map(Self::ItemReference)
                .map_err(|e| wrap("item_reference", e)),
            Some("compaction") => from_map(map)
                .map(Self::Compaction)
                .map_err(|e| wrap("compaction", e)),
            Some(_) => Ok(Self::Unknown(rejoin(tag, map))),
            None => Err(D::Error::custom("item has no `type`")),
        }
    }
}
