//! HTTP values shared by the Labs API and portable SDK clients. No transport or storage.
use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) fn me() -> String {
    "me".into()
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default = "me")]
    pub space: String,
    #[serde(default)]
    pub after: u64,
    #[serde(default = "page_size")]
    pub limit: u64,
    pub kind: Option<String>,
    pub since: Option<u64>,
    pub document: Option<String>,
}
fn page_size() -> u64 {
    50
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDocument {
    #[serde(default = "me")]
    pub space: String,
    pub id: String,
    pub kind: String,
    pub schema: String,
    pub title: String,
    pub content: Value,
    pub summary: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EditDocument {
    #[serde(default = "me")]
    pub space: String,
    pub base: u64,
    pub change: Change,
    pub summary: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateJob {
    #[serde(default = "me")]
    pub space: String,
    pub version: Reference,
    pub want: Want,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    #[serde(default = "me")]
    pub space: String,
    pub version: Reference,
    pub media_type: String,
    pub class: AssetClass,
    #[serde(default)]
    pub no_training: bool,
    pub content_base64: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddNote {
    #[serde(default = "me")]
    pub space: String,
    pub version: u64,
    pub locator: Locator,
    pub text: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    #[serde(default = "me")]
    pub space: String,
    pub chat: Option<String>,
    pub base: Option<u64>,
    pub arguments: Value,
}

/// The same envelopes are serialized by handlers and described by OpenAPI.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct List<T> {
    pub data: Vec<T>,
    pub next: Option<u64>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Data<T> {
    pub data: T,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct ArtifactLink {
    pub artifact: Artifact,
    pub url: String,
    pub expires_seconds: Option<u32>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Deleted {
    pub id: String,
    pub deleted: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Saved {
    pub saved: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct ToolResult {
    pub result: Value,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Event {
    pub cursor: u64,
    pub document: String,
    pub at_unix_ms: u64,
    pub change: Value,
}
