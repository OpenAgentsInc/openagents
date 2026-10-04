//! Host callbacks the CLI invokes over the control protocol: hooks,
//! MCP elicitation, and user dialogs.
//!
//! Hooks follow the TS SDK: each callback gets an ID (`hook_0`,
//! `hook_1`, ...), the IDs go to the CLI in the `initialize` request, and
//! the CLI sends `hook_callback` with that ID when the hook fires. The
//! reply is the callback's [`HookJSONOutput`].

use crate::error::{Error, Result};
use crate::protocol::{
    ElicitationRequest, ElicitationResult, HookCallbackRequest, HookEvent, HookJSONOutput,
    RequestUserDialogRequest, SdkHookCallbackMatcher,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

/// A host hook callback (TS `HookCallback`).
#[async_trait]
pub trait HookCallback: Send + Sync {
    /// Run the hook. `input` is the CLI's hook input, with
    /// `hook_event_name` and the event's fields. An error is sent to the
    /// CLI as a control error response.
    async fn call(&self, input: Value, tool_use_id: Option<String>) -> Result<HookJSONOutput>;
}

impl fmt::Debug for dyn HookCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HookCallback")
    }
}

struct FnHook<F>(F);

#[async_trait]
impl<F, Fut> HookCallback for FnHook<F>
where
    F: Fn(Value, Option<String>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<HookJSONOutput>> + Send,
{
    async fn call(&self, input: Value, tool_use_id: Option<String>) -> Result<HookJSONOutput> {
        (self.0)(input, tool_use_id).await
    }
}

/// Wrap an async closure as a [`HookCallback`].
pub fn hook_fn<F, Fut>(f: F) -> Arc<dyn HookCallback>
where
    F: Fn(Value, Option<String>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<HookJSONOutput>> + Send + 'static,
{
    Arc::new(FnHook(f))
}

/// Hooks for one event that share a matcher (TS `HookCallbackMatcher`).
#[derive(Clone, Default)]
pub struct HookMatcher {
    /// Tool-name pattern for tool events; `None` matches everything.
    pub matcher: Option<String>,
    /// Callbacks run when the matcher matches.
    pub hooks: Vec<Arc<dyn HookCallback>>,
    /// Seconds the CLI waits for these callbacks.
    pub timeout: Option<f64>,
}

impl HookMatcher {
    /// A matcher with one callback.
    pub fn new(matcher: Option<&str>, hook: Arc<dyn HookCallback>) -> Self {
        Self {
            matcher: matcher.map(str::to_string),
            hooks: vec![hook],
            timeout: None,
        }
    }
}

impl fmt::Debug for HookMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookMatcher")
            .field("matcher", &self.matcher)
            .field("hooks", &self.hooks.len())
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Answers MCP elicitation requests (TS `onElicitation`).
#[async_trait]
pub trait ElicitationHandler: Send + Sync {
    /// Return `Ok(None)` to send no response, as the TS SDK does when the
    /// handler returns `null`.
    async fn elicit(&self, request: &ElicitationRequest) -> Result<Option<ElicitationResult>>;
}

impl fmt::Debug for dyn ElicitationHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ElicitationHandler")
    }
}

/// Answers `request_user_dialog` (TS `onUserDialog`).
#[async_trait]
pub trait UserDialogHandler: Send + Sync {
    /// Return `Ok(None)` to send no response.
    async fn dialog(&self, request: &RequestUserDialogRequest) -> Result<Option<Value>>;
}

impl fmt::Debug for dyn UserDialogHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UserDialogHandler")
    }
}

/// The `initialize` request's `hooks` field.
pub(crate) type HookPayload = HashMap<HookEvent, Vec<SdkHookCallbackMatcher>>;

/// Hook callbacks registered for one query, by callback ID.
#[derive(Clone, Default)]
pub(crate) struct HookRegistry {
    callbacks: HashMap<String, Arc<dyn HookCallback>>,
}

impl HookRegistry {
    /// Assign `hook_N` IDs, as the TS SDK does, and build the `initialize`
    /// hooks payload.
    pub(crate) fn register(
        hooks: &HashMap<HookEvent, Vec<HookMatcher>>,
    ) -> (Self, Option<HookPayload>) {
        let mut registry = Self::default();
        if hooks.is_empty() {
            return (registry, None);
        }
        let mut next = 0usize;
        let mut payload = HashMap::new();
        // Sort events so IDs are stable across runs.
        let mut events: Vec<_> = hooks.iter().filter(|(_, m)| !m.is_empty()).collect();
        events.sort_by_key(|(event, _)| format!("{event:?}"));
        for (event, matchers) in events {
            let wire = matchers
                .iter()
                .map(|matcher| {
                    let ids = matcher
                        .hooks
                        .iter()
                        .map(|hook| {
                            let id = format!("hook_{next}");
                            next += 1;
                            registry.callbacks.insert(id.clone(), hook.clone());
                            id
                        })
                        .collect();
                    SdkHookCallbackMatcher {
                        matcher: matcher.matcher.clone(),
                        hook_callback_ids: ids,
                        timeout: matcher.timeout,
                    }
                })
                .collect();
            payload.insert(*event, wire);
        }
        (registry, Some(payload))
    }

    /// Run the callback the request names.
    pub(crate) async fn run(&self, request: &HookCallbackRequest) -> Result<HookJSONOutput> {
        let callback = self.callbacks.get(&request.callback_id).ok_or_else(|| {
            Error::HookCallbackFailed(format!(
                "no hook callback found for ID: {}",
                request.callback_id
            ))
        })?;
        callback
            .call(request.input.clone(), request.tool_use_id.clone())
            .await
    }
}
