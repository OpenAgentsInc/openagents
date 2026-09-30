//! The local Devin CLI as an ACP agent: `devin acp`.
//!
//! Devin's print mode (`devin -p`) writes nothing until the very end;
//! `devin acp` is the same agent as an ACP server over standard input and
//! output, and it streams: `agent_message_chunk`, `agent_thought_chunk`,
//! `tool_call` and `tool_call_update`, `plan`, and `usage_update` with token
//! counts under Devin's `cognition.ai/inputTokens` and
//! `cognition.ai/outputTokens` keys. A `session/prompt` reply carries the
//! turn's `usage` totals. Captured from Devin CLI 3000.11.3 on 2026-09-28
//! (`fixtures/devin-3000.11.3-turn.jsonl`).
//!
//! What this module adds to the generic client:
//!
//! - **The binary**: `DEVIN_BIN`, else `devin` on `PATH`, else
//!   `~/.local/bin/devin`, where Devin's installer puts it.
//! - **The login**: Devin keeps its CLI login in
//!   `$XDG_DATA_HOME/devin/credentials.toml` (by default
//!   `~/.local/share/devin/credentials.toml`). [`signed_in`] checks that the
//!   file exists and is not empty, and never reads it. `initialize`
//!   advertises only `devin-browser`, a login window; the client never sends
//!   `authenticate`, so the stored login is used, and an agent without one
//!   refuses `session/new`.
//! - **The model**: `devin acp --model NAME` sets the session's model; the
//!   route model `default` leaves Devin's own default. `session/new` reports
//!   the model in its `model` configuration option.
//! - **Permission modes**: Devin's ids are `accept-edits` (its default),
//!   `smart`, `ask`, `plan`, and `bypass`. Full access is `bypass`.
//! - **The engine marker**: `session/new`'s `_meta` is kept with the session
//!   and returned by `session/list`, so [`ENGINE_META_KEY`] marks a session
//!   Coder's engine started.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::process::{first_executable, on_path};

/// The variable that names the binary.
pub const BIN_VAR: &str = "DEVIN_BIN";
/// The route model that keeps Devin's own default model.
pub const DEFAULT_MODEL: &str = "default";
/// The `session/new` `_meta` key that marks a session Coder's engine
/// started. Its value is the engine mark.
pub const ENGINE_META_KEY: &str = "openagents.com/engine";

/// How much Devin may do without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Every tool runs without a question: Devin's `bypass`.
    Bypass,
    /// Workspace edits run; anything else asks: Devin's `accept-edits`.
    AcceptEdits,
}

impl Permission {
    /// Devin's mode id.
    #[must_use]
    pub const fn mode_id(self) -> &'static str {
        match self {
            Permission::Bypass => "bypass",
            Permission::AcceptEdits => "accept-edits",
        }
    }
}

/// The Devin binary: `DEVIN_BIN`, else `devin` on `PATH`, else
/// `~/.local/bin/devin`. `variable` reads the environment.
#[must_use]
pub fn binary(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(named) = variable(BIN_VAR).filter(|value| !value.is_empty()) {
        return first_executable([PathBuf::from(named)]);
    }
    let home = variable("HOME").map(PathBuf::from);
    first_executable(
        on_path("devin", variable("PATH").as_deref())
            .into_iter()
            .chain(home.map(|home| home.join(".local/bin/devin"))),
    )
}

/// Where Devin keeps its CLI login.
#[must_use]
pub fn credentials_path(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let data = variable("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| variable("HOME").map(|home| Path::new(&home).join(".local/share")))?;
    Some(data.join("devin/credentials.toml"))
}

/// Whether Devin has a stored CLI login. Only the file's size is read.
#[must_use]
pub fn signed_in(variable: &dyn Fn(&str) -> Option<OsString>) -> bool {
    credentials_path(variable)
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|meta| meta.is_file() && meta.len() > 0)
}

/// The arguments that start Devin as an ACP agent with `model`.
#[must_use]
pub fn arguments(model: &str) -> Vec<String> {
    let mut arguments = vec!["acp".to_string()];
    if model != DEFAULT_MODEL {
        arguments.push("--model".into());
        arguments.push(model.into());
    }
    arguments
}

/// The `_meta` a Coder engine session opens with.
#[must_use]
pub fn engine_meta(mark: &str) -> Value {
    json!({ ENGINE_META_KEY: mark })
}

/// Whether a session's `_meta`, as `session/list` returns it, carries the
/// engine mark.
#[must_use]
pub fn is_engine_session(meta: Option<&serde_json::Map<String, Value>>, mark: &str) -> bool {
    meta.and_then(|meta| meta.get(ENGINE_META_KEY))
        .and_then(Value::as_str)
        == Some(mark)
}

/// The notification Devin sends when a turn ends, with the turn's
/// cumulative usage dimensions.
pub const TURN_STATS: &str = "_cognition.ai/turn_stats";

/// One usage dimension Devin reports for a turn: `input_tokens`,
/// `output_tokens`, `cached_input_tokens`, `agent_messages`, `model`, and,
/// on a billed plan, its credit and ACU dimensions.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Dimension {
    pub uid: String,
    #[serde(default)]
    pub label: Option<String>,
    pub kind: DimensionKind,
}

/// A dimension's value.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DimensionKind {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: Value,
}

/// The `_cognition.ai/turn_stats` parameters.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStats {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub response_dimensions: Vec<Dimension>,
}

impl TurnStats {
    /// Parse the notification's parameters.
    #[must_use]
    pub fn parse(params: &Value) -> Option<Self> {
        serde_json::from_value(params.clone()).ok()
    }

    /// A numeric dimension's value by its `uid`.
    #[must_use]
    pub fn number(&self, uid: &str) -> Option<f64> {
        self.response_dimensions
            .iter()
            .find(|dimension| dimension.uid == uid)
            .and_then(|dimension| dimension.kind.value.as_f64())
    }

    /// A text dimension's value by its `uid`, such as `model`.
    #[must_use]
    pub fn text(&self, uid: &str) -> Option<&str> {
        self.response_dimensions
            .iter()
            .find(|dimension| dimension.uid == uid)
            .and_then(|dimension| dimension.kind.value.as_str())
    }

    /// Every numeric dimension, by `uid`, for the record.
    #[must_use]
    pub fn numbers(&self) -> std::collections::BTreeMap<String, f64> {
        self.response_dimensions
            .iter()
            .filter_map(|dimension| {
                dimension
                    .kind
                    .value
                    .as_f64()
                    .map(|value| (dimension.uid.clone(), value))
            })
            .collect()
    }
}

/// Whether the model a session reports is the one a route admitted. The
/// route model `default` admits whatever Devin reports.
#[must_use]
pub fn admits(route_model: &str, reported: Option<&str>) -> bool {
    route_model == DEFAULT_MODEL || reported == Some(route_model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn the_binary_is_found_by_variable_path_or_installer_location() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let devin = bin.join("devin");
        std::fs::write(&devin, "#!/bin/sh\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&devin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert_eq!(binary(&env), Some(devin.clone()));
        let named = devin.as_os_str().to_owned();
        let env = |name: &str| (name == BIN_VAR).then(|| named.clone());
        assert_eq!(binary(&env), Some(devin));
        let missing = |name: &str| (name == BIN_VAR).then(|| OsString::from("/nonexistent/devin"));
        assert_eq!(binary(&missing), None);
    }

    #[test]
    fn the_login_is_checked_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert!(!signed_in(&env));
        let path = credentials_path(&env).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
        assert!(!signed_in(&env));
        std::fs::write(&path, "x").unwrap();
        assert!(signed_in(&env));
    }

    #[test]
    fn the_model_and_the_marker_are_explicit() {
        assert_eq!(arguments(DEFAULT_MODEL), vec!["acp"]);
        assert_eq!(
            arguments("swe-2-high"),
            vec!["acp", "--model", "swe-2-high"]
        );
        assert!(admits(DEFAULT_MODEL, Some("anything")));
        assert!(admits("swe-2-high", Some("swe-2-high")));
        assert!(!admits("swe-2-high", Some("adaptive")));
        assert!(!admits("swe-2-high", None));
        let meta = engine_meta("openagents-coder-engine");
        assert!(is_engine_session(
            meta.as_object(),
            "openagents-coder-engine"
        ));
        assert!(!is_engine_session(None, "openagents-coder-engine"));
        assert_eq!(Permission::Bypass.mode_id(), "bypass");
    }

    #[test]
    fn turn_stats_are_typed() {
        let params = json!({"sessionId":"s","responseDimensions":[
            {"uid":"model","label":"Model","kind":{"type":"metric","value":"SWE-2 High"}},
            {"uid":"input_tokens","label":"Input tokens","kind":{"type":"cumulativeMetric","value":11056.0}},
            {"uid":"output_tokens","kind":{"type":"cumulativeMetric","value":95.0}}]});
        let stats = TurnStats::parse(&params).unwrap();
        assert_eq!(stats.number("input_tokens"), Some(11056.0));
        assert_eq!(stats.text("model"), Some("SWE-2 High"));
        assert_eq!(stats.numbers().len(), 2);
        assert!(TurnStats::parse(&json!({"responseDimensions":[{"uid":1}]})).is_none());
    }
}
