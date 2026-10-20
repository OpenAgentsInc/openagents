//! Thinking-mode settings shared by the protocol adapters.

use deepseek_recipe_core::conversation::ReasoningEffort;
use serde::{Deserialize, Serialize};

/// The `type` of a request's `thinking` setting. `adaptive` is an alias of
/// `enabled`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingType {
    #[serde(alias = "adaptive")]
    Enabled,
    Disabled,
}

/// The reasoning effort declared in a protocol request.
///
/// Unless a `thinking` setting overrides it, `none` disables thinking and
/// other values enable it. Enabled thinking uses high effort by default.
/// Conversion maps `minimal` and `low` to [`ReasoningEffort::Low`], `medium`
/// and `high` to [`ReasoningEffort::High`], `xhigh` to
/// [`ReasoningEffort::Xhigh`], and `ultra` and `max` to
/// [`ReasoningEffort::Max`].
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Ultra,
    Max,
}

impl RequestReasoningEffort {
    /// Return the shared reasoning effort of the declared effort, or `None`
    /// for `none`.
    fn to_reasoning_effort(self) -> Option<ReasoningEffort> {
        match self {
            Self::None => None,
            Self::Minimal | Self::Low => Some(ReasoningEffort::Low),
            Self::Medium | Self::High => Some(ReasoningEffort::High),
            Self::Xhigh => Some(ReasoningEffort::Xhigh),
            Self::Ultra | Self::Max => Some(ReasoningEffort::Max),
        }
    }
}

/// Resolve the thinking mode and reasoning effort of a request.
///
/// An explicit `thinking` setting decides the mode. Otherwise an `effort` of
/// `none` disables thinking, any other effort enables it, and no effort falls
/// back to `default_thinking_mode`. In thinking mode, the reasoning effort is
/// the shared effort of `effort`, or [`ReasoningEffort::High`] without one.
/// Outside thinking mode, the reasoning effort is `None`.
pub fn resolve_thinking_mode(
    default_thinking_mode: bool,
    thinking_type: Option<ThinkingType>,
    effort: Option<RequestReasoningEffort>,
) -> (bool, Option<ReasoningEffort>) {
    let thinking_mode = match thinking_type {
        Some(ThinkingType::Enabled) => true,
        Some(ThinkingType::Disabled) => false,
        None => match effort {
            Some(RequestReasoningEffort::None) => false,
            Some(_) => true,
            None => default_thinking_mode,
        },
    };
    let reasoning_effort = thinking_mode.then(|| {
        effort
            .and_then(RequestReasoningEffort::to_reasoning_effort)
            .unwrap_or(ReasoningEffort::High)
    });
    (thinking_mode, reasoning_effort)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_thinking_decides_the_mode() {
        assert_eq!(
            resolve_thinking_mode(
                false,
                Some(ThinkingType::Enabled),
                Some(RequestReasoningEffort::None)
            ),
            (true, Some(ReasoningEffort::High))
        );
        assert_eq!(
            resolve_thinking_mode(
                true,
                Some(ThinkingType::Disabled),
                Some(RequestReasoningEffort::Max)
            ),
            (false, None)
        );
    }

    #[test]
    fn effort_decides_the_mode_without_explicit_thinking() {
        assert_eq!(
            resolve_thinking_mode(true, None, Some(RequestReasoningEffort::None)),
            (false, None)
        );
        assert_eq!(
            resolve_thinking_mode(false, None, Some(RequestReasoningEffort::Minimal)),
            (true, Some(ReasoningEffort::Low))
        );
        assert_eq!(
            resolve_thinking_mode(false, None, Some(RequestReasoningEffort::Ultra)),
            (true, Some(ReasoningEffort::Max))
        );
    }

    #[test]
    fn declared_efforts_map_to_shared_efforts() {
        let mapped = |effort: RequestReasoningEffort| effort.to_reasoning_effort();
        assert_eq!(mapped(RequestReasoningEffort::None), None);
        assert_eq!(
            mapped(RequestReasoningEffort::Low),
            Some(ReasoningEffort::Low)
        );
        assert_eq!(
            mapped(RequestReasoningEffort::Medium),
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            mapped(RequestReasoningEffort::Xhigh),
            Some(ReasoningEffort::Xhigh)
        );
        assert_eq!(
            mapped(RequestReasoningEffort::Max),
            Some(ReasoningEffort::Max)
        );
    }
}
