//! Read-only execution descriptions shared by the SDK and its clients.

pub mod control;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.session.v1";

/// The configured choice, without credentials or provider endpoints.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
}

/// Native provider operations. Missing fields never grant support.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NativeSupport {
    pub start: bool,
    pub observe: bool,
    pub stop: bool,
    pub resume: bool,
    pub steer: bool,
    pub fork: bool,
    pub images: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub image_followups: bool,
    pub questions: bool,
    pub approvals: bool,
    pub plans: bool,
    pub children: bool,
    pub context_usage: bool,
}

/// Task-owner operations, distinct from native provider controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    pub stop: bool,
    pub continue_task: bool,
    pub queue: bool,
    pub stop_and_send: bool,
    pub answer: bool,
}

/// A backend's support, not a claim that a particular hook is installed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookSupport {
    Native,
    Command,
    Emulated,
    #[default]
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Backend {
    pub selection: Selection,
    pub adapter: String,
    /// Absent until the executable reports its version.
    pub version: Option<String>,
    pub native: NativeSupport,
    pub hooks: BTreeMap<String, HookSupport>,
}

/// A snapshot of a task turn's choices and the controls its owner serves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Descriptor {
    pub schema: String,
    pub task: String,
    pub turn: usize,
    pub owner: String,
    pub checkout: String,
    /// None means automatic selection, not a request for the resolved model.
    pub requested: Option<Selection>,
    pub resolved: Backend,
    /// The native session ID, only when the backend has actually reported it.
    pub native_session: Option<String>,
    /// An explicit provider-session operation requested for this task turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<control::Mode>,
    pub controls: Controls,
    /// Configured alternatives; their presence does not assert readiness.
    pub alternatives: Vec<Backend>,
}

impl Descriptor {
    pub fn applies_to(&self, task: &str, turn: usize) -> bool {
        self.schema == SCHEMA
            && !task.is_empty()
            && turn > 0
            && self.task == task
            && self.turn == turn
    }

    /// Resolve a provider switch against this turn's frozen alternatives.
    pub fn switch_to(&mut self, route: Option<&str>) {
        self.native_session = None;
        let selected = route.and_then(|route| {
            std::iter::once(&self.resolved)
                .chain(&self.alternatives)
                .find(|backend| {
                    route == format!("{}:{}", backend.selection.provider, backend.selection.model)
                })
                .cloned()
        });
        self.resolved = selected.unwrap_or_else(|| Backend {
            selection: route
                .and_then(|route| route.split_once(':'))
                .map(|(provider, model)| Selection {
                    provider: provider.into(),
                    model: model.into(),
                    effort: None,
                })
                .unwrap_or_default(),
            ..Backend::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_future_capabilities_do_not_enable_operations() {
        let backend: Backend = serde_json::from_value(serde_json::json!({
            "selection": {"provider": "future", "model": "new"},
            "native": {"future_action": true},
            "hooks": {"PreToolUse": "future_tier"}
        }))
        .unwrap();
        assert_eq!(backend.native, NativeSupport::default());
        assert_eq!(backend.hooks["PreToolUse"], HookSupport::Unsupported);
        let descriptor: Descriptor = serde_json::from_str("{}").unwrap();
        assert_eq!(descriptor.controls, Controls::default());
        assert!(!descriptor.applies_to("task", 1));
    }

    #[test]
    fn switches_keep_requested_choice_and_do_not_invent_native_support() {
        let selection = |provider: &str| Selection {
            provider: provider.into(),
            model: "model:variant".into(),
            effort: None,
        };
        let mut descriptor = Descriptor {
            requested: Some(selection("codex")),
            alternatives: vec![Backend {
                selection: selection("claude"),
                adapter: "claude-session".into(),
                ..Backend::default()
            }],
            ..Descriptor::default()
        };
        descriptor.switch_to(Some("claude:model:variant"));
        assert_eq!(descriptor.resolved.adapter, "claude-session");
        assert_eq!(descriptor.requested.as_ref().unwrap().provider, "codex");
        descriptor.switch_to(Some("future:new"));
        assert_eq!(descriptor.resolved.selection.provider, "future");
        assert_eq!(descriptor.resolved.native, NativeSupport::default());
        assert!(descriptor.resolved.hooks.is_empty());
        descriptor.switch_to(None);
        assert!(descriptor.resolved.selection.provider.is_empty());
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
