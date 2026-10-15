//! The Claude Code backend (#11496): the SDK's events and outcomes on
//! `claude_agent_sdk`'s hooks and its `can_use_tool` callback.
//!
//! Every event is native here: Claude Code calls the hook before it acts
//! and obeys the answer. A session's hooks run through one [`Runner`], so
//! the composition rules hold across them (squeeze rewrites, the floors
//! judge the rewritten command) instead of Claude Code running each hook
//! on the original input in parallel.
//!
//! `PreToolUse` reaches the hooks on one of two channels:
//!
//! - [`PreTool::Hook`]: a `PreToolUse` hook, for sessions under
//!   `bypassPermissions`, where Claude Code never sends `can_use_tool`;
//! - [`PreTool::Permission`]: the `can_use_tool` callback
//!   ([`Claude::permission_handler`]), for sessions that ask, such as the
//!   `claude/sdk:` seat. An allow there carries the rewritten input; a deny
//!   that also ends the session interrupts the turn.
//!
//! An `Ask` goes to the host's [`Asker`] (the approval card, a phone);
//! with none, it is held: denied with its step, so the agent hands that
//! step back to the person.
//!
//! [`SessionBox`] is the box the delegate recipe's Claude row describes:
//! system prompt, tools, setting sources, model and effort, applied at
//! session start from one declaration.

use std::collections::BTreeSet;
use std::sync::Arc;

use claude_agent_sdk::{
    HookEvent, HookJSONOutput, HookMatcher, PermissionResult, QueryOptions, SyncHookJSONOutput,
};
use coder_delegate::session::HookBackend;
use serde_json::{Value, json};

use super::compose::{Applied, Budget, Refused};
use super::events::{Call, Event};
use super::hooks::Hooks;
pub use super::hooks::{Asked, Asker};
use super::outcome::{Context, Permission, Stop};
use super::registry::Registry;
/// The Claude Agent SDK this backend speaks.
pub use claude_agent_sdk;

/// Where `PreToolUse` reaches the hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreTool {
    /// A `PreToolUse` hook: sessions under `bypassPermissions`.
    Hook,
    /// The `can_use_tool` callback: sessions that ask per call. The caller
    /// passes [`Claude::permission_handler`] to the query.
    Permission,
}

/// Claude Code's event for one of ours.
#[must_use]
pub fn hook_event(event: Event) -> HookEvent {
    match event {
        Event::SessionStart => HookEvent::SessionStart,
        Event::UserPromptSubmit => HookEvent::UserPromptSubmit,
        Event::PreToolUse => HookEvent::PreToolUse,
        Event::PostToolUse => HookEvent::PostToolUse,
        Event::PostToolBatch => HookEvent::PostToolBatch,
        Event::Stop => HookEvent::Stop,
        Event::PreCompact => HookEvent::PreCompact,
        Event::PostCompact => HookEvent::PostCompact,
        Event::SubagentStart => HookEvent::SubagentStart,
        Event::SubagentStop => HookEvent::SubagentStop,
        Event::FileChanged => HookEvent::CwdChanged,
        Event::SessionEnd => HookEvent::SessionEnd,
    }
}

/// The call a Claude Code hook input makes on `event`: a tool event's tool
/// and input (with the tool's response after it ran), a prompt, or the
/// whole input for the rest.
#[must_use]
pub fn call(event: Event, input: &Value) -> Call {
    match event {
        Event::PreToolUse => Call::tool(
            event,
            input["tool_name"].as_str().unwrap_or_default(),
            input["tool_input"].clone(),
        ),
        // A failed tool reaches Claude Code's `PostToolUseFailure` with its
        // error; both are our `PostToolUse`, with `failed` saying which.
        Event::PostToolUse => {
            let failed = input["hook_event_name"] == "PostToolUseFailure"
                || input.get("error").is_some_and(|error| !error.is_null());
            let response = if failed {
                json!({"error": input["error"]})
            } else {
                input["tool_response"].clone()
            };
            Call::tool(
                event,
                input["tool_name"].as_str().unwrap_or_default(),
                json!({"tool_input": input["tool_input"], "tool_response": response, "failed": failed}),
            )
        }
        Event::UserPromptSubmit => Call::new(event, json!({"prompt": input["prompt"]})),
        _ => Call::new(event, input.clone()),
    }
}

/// The kept context as one text, each part labeled with its source.
#[must_use]
pub fn context_text(context: &[Context]) -> Option<String> {
    (!context.is_empty()).then(|| {
        context
            .iter()
            .map(|c| format!("[{}] {}", c.source, c.text))
            .collect::<Vec<_>>()
            .join("\n\n")
    })
}

/// What Claude Code reads back for `applied`. An outcome with nothing to
/// say lets the call through unchanged.
#[must_use]
pub fn output(applied: &Applied) -> SyncHookJSONOutput {
    let mut out = SyncHookJSONOutput::default();
    let name = hook_event_name(applied.event);
    let context = context_text(&applied.context);
    let mut specific = serde_json::Map::new();
    match applied.event {
        Event::PreToolUse => {
            match &applied.permission {
                Permission::Deny { reason } => {
                    specific.insert("permissionDecision".into(), json!("deny"));
                    specific.insert("permissionDecisionReason".into(), json!(reason));
                }
                // Unresolved: Claude Code asks the person itself.
                Permission::Ask { step } => {
                    specific.insert("permissionDecision".into(), json!("ask"));
                    specific.insert("permissionDecisionReason".into(), json!(step));
                }
                Permission::Allow => {
                    specific.insert("permissionDecision".into(), json!("allow"));
                }
                Permission::NoOpinion => {}
            }
            if !applied.rewritten_by.is_empty()
                && !matches!(applied.permission, Permission::Deny { .. })
            {
                specific.insert("updatedInput".into(), applied.input.clone());
            }
        }
        Event::UserPromptSubmit => {
            if let Permission::Deny { reason } = &applied.permission {
                out.decision = Some("block".into());
                out.reason = Some(reason.clone());
            }
        }
        _ => {}
    }
    if let Stop::Block { reason } = &applied.stop {
        out.decision = Some("block".into());
        out.reason = Some(reason.clone());
    }
    if let Some(text) = context
        && takes_context(applied.event)
    {
        specific.insert("additionalContext".into(), json!(text));
    }
    if applied.stop == Stop::End {
        out.continue_execution = Some(false);
        out.stop_reason = Some(match &applied.permission {
            Permission::Deny { reason } => reason.clone(),
            _ => "A Coder hook ended the session.".to_owned(),
        });
    }
    if !specific.is_empty() {
        specific.insert("hookEventName".into(), json!(name));
        out.hook_specific_output = Some(Value::Object(specific));
    }
    if out == SyncHookJSONOutput::default() {
        return SyncHookJSONOutput::continue_execution();
    }
    out
}

/// The `can_use_tool` answer for a `PreToolUse` that came to `applied`.
#[must_use]
pub fn permission_result(applied: &Applied) -> PermissionResult {
    match &applied.permission {
        Permission::Deny { reason } if applied.stop == Stop::End => {
            PermissionResult::deny_and_interrupt(reason.clone())
        }
        Permission::Deny { reason } | Permission::Ask { step: reason } => {
            PermissionResult::deny(reason.clone())
        }
        Permission::Allow | Permission::NoOpinion => PermissionResult::allow(applied.input.clone()),
    }
}

fn hook_event_name(event: Event) -> &'static str {
    match event {
        Event::FileChanged => "CwdChanged",
        other => other.name(),
    }
}

/// The events whose `hookSpecificOutput` carries `additionalContext`.
fn takes_context(event: Event) -> bool {
    matches!(
        event,
        Event::SessionStart
            | Event::UserPromptSubmit
            | Event::PreToolUse
            | Event::PostToolUse
            | Event::PostToolBatch
            | Event::SubagentStart
            | Event::PostCompact
    )
}

/// One session's hooks on Claude Code.
#[derive(Clone, Debug)]
pub struct Claude {
    hooks: Hooks,
}

/// Native call identity kept separate from the hook's final tool input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionIdentity {
    pub native_call: String,
    pub child: Option<String>,
}

/// Bind each waiting callback to its own native call before resolving Ask.
pub type AskerFactory = Arc<dyn Fn(PermissionIdentity) -> Asker + Send + Sync>;

impl Claude {
    /// The backend for one session's `registry`.
    ///
    /// # Errors
    ///
    /// The hooks Claude Code can't honor (none today: every event is
    /// native).
    pub fn new(registry: Registry, budget: Budget) -> Result<Self, Vec<Refused>> {
        Ok(Claude {
            hooks: Hooks::new(registry, budget, HookBackend::ClaudeCode)?,
        })
    }

    /// Answers each `Ask` through `asker`.
    #[must_use]
    pub fn asking(mut self, asker: Asker) -> Self {
        self.hooks = self.hooks.asking(asker);
        self
    }

    /// The session's registry.
    #[must_use]
    pub fn registry(&self) -> Registry {
        self.hooks.registry()
    }

    /// Runs the hooks on `call` and resolves an `Ask` ([`Hooks::fire`]).
    pub async fn fire(&self, call: Call) -> Applied {
        self.hooks.fire(call).await
    }

    /// `options` with the hooks installed: one Claude Code hook per event
    /// that has any, matching the union of their tools. With
    /// [`PreTool::Permission`], `PreToolUse` is left to
    /// [`Claude::permission_handler`].
    #[must_use]
    pub fn install(&self, mut options: QueryOptions, pre_tool: PreTool) -> QueryOptions {
        let registry = self.registry();
        for event in Event::ALL {
            if event == Event::PreToolUse && pre_tool == PreTool::Permission {
                continue;
            }
            let hooks: Vec<_> = registry.matching_any(event).collect();
            if hooks.is_empty() {
                continue;
            }
            let matcher = union_matcher(hooks.iter().map(|hook| hook.matcher.as_deref()));
            let backend = self.clone();
            let callback = claude_agent_sdk::hook_fn(move |input: Value, _tool_use_id| {
                let backend = backend.clone();
                async move {
                    let applied = backend.fire(call(event, &input)).await;
                    let mut out = output(&applied);
                    // A failed tool's answer names the event it came on.
                    if input["hook_event_name"] == "PostToolUseFailure"
                        && let Some(Value::Object(specific)) = out.hook_specific_output.as_mut()
                    {
                        specific.insert("hookEventName".into(), json!("PostToolUseFailure"));
                    }
                    Ok(HookJSONOutput::from(out))
                }
            });
            if event == Event::PostToolUse {
                options = options.hook(
                    HookEvent::PostToolUseFailure,
                    HookMatcher::new(matcher.as_deref(), callback.clone()),
                );
            }
            options = options.hook(
                hook_event(event),
                HookMatcher::new(matcher.as_deref(), callback),
            );
        }
        options
    }

    /// The `can_use_tool` callback that runs the `PreToolUse` hooks.
    #[must_use]
    pub fn permission_handler(&self) -> Arc<dyn claude_agent_sdk::PermissionHandler> {
        self.handler(None)
    }

    /// The same runner, with a separately bound asker for each native call.
    pub fn permission_handler_with_asker(
        &self,
        factory: AskerFactory,
    ) -> Arc<dyn claude_agent_sdk::PermissionHandler> {
        self.handler(Some(factory))
    }

    fn handler(
        &self,
        factory: Option<AskerFactory>,
    ) -> Arc<dyn claude_agent_sdk::PermissionHandler> {
        let backend = self.clone();
        claude_agent_sdk::permission_handler(move |request| {
            let mut backend = backend.clone();
            if let Some(factory) = &factory {
                backend.hooks = backend.hooks.asking(factory(PermissionIdentity {
                    native_call: request.tool_use_id.clone(),
                    child: request.agent_id.clone(),
                }));
            }
            async move {
                let applied = backend
                    .fire(Call::tool(
                        Event::PreToolUse,
                        &request.tool_name,
                        request.input.clone(),
                    ))
                    .await;
                Ok::<_, claude_agent_sdk::Error>(permission_result(&applied))
            }
        })
    }
}

/// One matcher for several hooks: none when any fires on every tool,
/// else their tool names joined with `|`.
fn union_matcher<'a>(matchers: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let mut names = BTreeSet::new();
    for matcher in matchers {
        match matcher {
            None | Some("" | "*") => return None,
            Some(matcher) => names.extend(matcher.split('|').map(|name| name.trim().to_owned())),
        }
    }
    Some(names.into_iter().collect::<Vec<_>>().join("|"))
}

/// The box a Claude Code session runs in, from the delegate recipe's row:
/// system prompt, tools, setting sources, model and effort.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionBox {
    /// Replaces Claude Code's system prompt.
    pub system_prompt: Option<String>,
    /// The tools offered: `None` keeps Claude Code's default set.
    pub tools: Option<Vec<String>>,
    /// The settings Claude Code loads; `None` keeps its own default.
    pub setting_sources: Option<Vec<claude_agent_sdk::SettingSource>>,
    pub model: Option<String>,
    /// `low`, `medium`, `high`, `xhigh` or `max`.
    pub effort: Option<String>,
}

impl SessionBox {
    /// The box for a run on the recipe's `engine`: the effort its row gives
    /// a task of `class`, else the admitted one.
    #[must_use]
    pub fn for_recipe(
        engine: &str,
        class: Option<route_contract::recipe::TaskClass>,
        admitted: Option<&str>,
    ) -> Self {
        SessionBox {
            effort: route_contract::recipe::effort(engine, class, admitted),
            ..SessionBox::default()
        }
    }

    /// `options` inside the box. What the box leaves unset stays as it was.
    #[must_use]
    pub fn apply(&self, mut options: QueryOptions) -> QueryOptions {
        if let Some(prompt) = &self.system_prompt {
            options.system_prompt =
                Some(claude_agent_sdk::SystemPromptConfig::Custom(prompt.clone()));
        }
        if let Some(tools) = &self.tools {
            options.tools = Some(claude_agent_sdk::ToolsConfig::Names(tools.clone()));
        }
        if let Some(sources) = &self.setting_sources {
            options.setting_sources = Some(sources.clone());
        }
        if let Some(model) = &self.model {
            options = options.model(model);
        }
        if let Some(effort) = self.effort.as_deref().and_then(effort_level) {
            options.effort = Some(effort);
        }
        options
    }
}

/// Claude Code's effort level for a recipe word.
#[must_use]
pub fn effort_level(effort: &str) -> Option<claude_agent_sdk::EffortLevel> {
    serde_json::from_value(json!(effort.to_ascii_lowercase())).ok()
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
