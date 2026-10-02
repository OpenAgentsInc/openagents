//! The labeled evaluation split for route families
//! (`fixtures/route-families-v1.json`).
//!
//! Each row is a conversation and the route family it must reach, with the
//! detail a family needs to be right (a dispatch plan's run count and mode,
//! a command's effect). Rows seeded from the chat router's fixtures name
//! their source row. `tune` rows may be used to tune question wording and
//! policy; `test` rows are untouched evidence for promotion (plan section
//! 5).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::route::RouteFamily;
use crate::snapshot::Surface;

/// The checked-in split.
pub const ROUTE_FAMILIES_V1: &str = include_str!("../fixtures/route-families-v1.json");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Split {
    /// [`crate::EVAL_SCHEMA`].
    pub schema: String,
    pub set: String,
    pub about: String,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub split: Part,
    pub surface: Surface,
    pub messages: Vec<Message>,
    pub family: RouteFamily,
    /// Family-specific expectations, such as `{"runs": 3, "mode":
    /// "read_only"}`.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub expect: Value,
    /// The chat-router route the row was labeled with, where it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_route: Option<String>,
    /// `routes-v4:<id>`, `wallet-v1:<id>`, or `new`.
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Tune,
    Test,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// The checked-in split, parsed.
///
/// # Panics
///
/// If the checked-in file does not parse, which its tests rule out.
#[must_use]
pub fn route_families_v1() -> Split {
    serde_json::from_str(ROUTE_FAMILIES_V1).expect("route-families-v1.json parses")
}
