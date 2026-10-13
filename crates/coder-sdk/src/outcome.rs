//! What a hook answers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A hook's answer on whether the action may go ahead.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "permission", rename_all = "snake_case")]
pub enum Permission {
    Allow,
    Deny {
        reason: String,
    },
    /// Ask the person, at this step.
    Ask {
        step: String,
    },
    #[default]
    NoOpinion,
}

impl Permission {
    /// How restrictive the answer is: deny beats ask beats allow beats no
    /// opinion.
    #[must_use]
    pub fn rank(&self) -> u8 {
        match self {
            Permission::NoOpinion => 0,
            Permission::Allow => 1,
            Permission::Ask { .. } => 2,
            Permission::Deny { .. } => 3,
        }
    }

    #[must_use]
    pub fn deny(reason: &str) -> Self {
        Permission::Deny {
            reason: reason.to_string(),
        }
    }

    #[must_use]
    pub fn ask(step: &str) -> Self {
        Permission::Ask {
            step: step.to_string(),
        }
    }
}

/// A hook's answer on whether the session goes on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stop", rename_all = "snake_case")]
pub enum Stop {
    #[default]
    Continue,
    /// Keep the agent working, for this reason.
    Block { reason: String },
    /// End the session.
    End,
}

impl Stop {
    /// End beats block beats continue.
    #[must_use]
    pub fn rank(&self) -> u8 {
        match self {
            Stop::Continue => 0,
            Stop::Block { .. } => 1,
            Stop::End => 2,
        }
    }
}

/// Text a hook adds to the session, labeled with where it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// Where the text came from, such as `squeeze` or a plugin's name.
    pub source: String,
    pub text: String,
    /// Higher is kept first when the budget is short.
    #[serde(default)]
    pub priority: i32,
}

impl Context {
    #[must_use]
    pub fn new(source: &str, text: &str, priority: i32) -> Self {
        Context {
            source: source.to_string(),
            text: text.to_string(),
            priority,
        }
    }
}

/// Everything one hook answers on one event.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookOutcome {
    pub permission: Permission,
    /// The tool input to use instead; `PreToolUse` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,
    #[serde(default)]
    pub stop: Stop,
}

impl HookOutcome {
    /// No opinion on anything.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn allow() -> Self {
        Self::permit(Permission::Allow)
    }

    #[must_use]
    pub fn deny(reason: &str) -> Self {
        Self::permit(Permission::deny(reason))
    }

    #[must_use]
    pub fn ask(step: &str) -> Self {
        Self::permit(Permission::ask(step))
    }

    #[must_use]
    pub fn permit(permission: Permission) -> Self {
        HookOutcome {
            permission,
            ..Self::default()
        }
    }

    /// Rewrites the tool input.
    #[must_use]
    pub fn rewrite(input: Value) -> Self {
        HookOutcome {
            rewrite: Some(input),
            ..Self::default()
        }
    }

    /// Adds context.
    #[must_use]
    pub fn context(context: Context) -> Self {
        HookOutcome {
            context: Some(context),
            ..Self::default()
        }
    }

    /// Answers on the session going on.
    #[must_use]
    pub fn stopping(stop: Stop) -> Self {
        HookOutcome {
            stop,
            ..Self::default()
        }
    }
}
