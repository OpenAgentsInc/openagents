//! Shared app-server method aliasing and legacy compatibility metadata.

/// Canonical notification method with legacy aliases.
#[derive(Debug, Clone, Copy)]
pub struct NotificationMethodAliasGroup {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
}

const NOTIFICATION_METHOD_ALIAS_GROUPS: &[NotificationMethodAliasGroup] = &[
    NotificationMethodAliasGroup {
        canonical: "item/started",
        aliases: &["codex/event/item_started"],
    },
    NotificationMethodAliasGroup {
        canonical: "item/completed",
        aliases: &["codex/event/item_completed"],
    },
    NotificationMethodAliasGroup {
        canonical: "agent_message_delta",
        aliases: &[
            "item/agentMessage/delta",
            "item/assistantMessage/delta",
            "agent_message/delta",
            "agent_message_content_delta",
            "codex/event/agent_message_content_delta",
            "codex/event/agent_message_delta",
        ],
    },
    NotificationMethodAliasGroup {
        canonical: "agent_message",
        aliases: &["codex/event/agent_message"],
    },
    NotificationMethodAliasGroup {
        canonical: "item/agentMessage/completed",
        aliases: &["item/assistantMessage/completed"],
    },
    NotificationMethodAliasGroup {
        canonical: "agent_reasoning_delta",
        aliases: &[
            "item/reasoning/summaryTextDelta",
            "item/reasoning/textDelta",
            "codex/event/reasoning_content_delta",
            "codex/event/reasoning_raw_content_delta",
            "codex/event/agent_reasoning_content_delta",
            "codex/event/agent_reasoning_raw_content_delta",
            "reasoning_content_delta",
            "reasoning_raw_content_delta",
            "agent_reasoning_content_delta",
            "agent_reasoning_raw_content_delta",
            "codex/event/agent_reasoning_delta",
        ],
    },
    NotificationMethodAliasGroup {
        canonical: "agent_reasoning",
        aliases: &["codex/event/agent_reasoning"],
    },
    NotificationMethodAliasGroup {
        canonical: "task_started",
        aliases: &["codex/event/task_started"],
    },
    NotificationMethodAliasGroup {
        canonical: "task_complete",
        aliases: &["codex/event/task_complete"],
    },
    NotificationMethodAliasGroup {
        canonical: "task_failed",
        aliases: &[
            "codex/event/task_failed",
            "codex/event/task_error",
            "task_error",
        ],
    },
    NotificationMethodAliasGroup {
        canonical: "turn/error",
        aliases: &["error"],
    },
];

const LEGACY_CODEX_EVENT_OPT_OUT_NOTIFICATION_METHODS: &[&str] = &[
    // Legacy codex/event stream mirrors v2 server notifications and causes duplicate
    // transcript/status updates when both are consumed.
    "codex/event/agent_message_content_delta",
    "codex/event/agent_message_delta",
    "codex/event/agent_message",
    "codex/event/agent_reasoning_delta",
    "codex/event/agent_reasoning_content_delta",
    "codex/event/agent_reasoning_raw_content_delta",
    "codex/event/agent_reasoning_section_break",
    "codex/event/agent_reasoning",
    "codex/event/reasoning_content_delta",
    "codex/event/reasoning_raw_content_delta",
    "codex/event/item_started",
    "codex/event/item_completed",
    "codex/event/task_started",
    "codex/event/task_complete",
    "codex/event/task_failed",
    "codex/event/task_error",
    "codex/event/thread_status",
    "codex/event/thread_name_changed",
    "codex/event/turn_diff",
    "codex/event/turn_plan",
    "codex/event/token_count",
    "codex/event/user_message",
];

/// Returns the canonical notification method for known aliases.
///
/// Unknown methods are returned unchanged.
pub fn canonical_notification_method(method: &str) -> &str {
    for group in NOTIFICATION_METHOD_ALIAS_GROUPS {
        if group.canonical == method || group.aliases.contains(&method) {
            return group.canonical;
        }
    }
    method
}

/// Legacy `codex/event/*` methods that should usually be opted out to avoid duplicate updates.
pub fn legacy_codex_event_opt_out_notification_methods() -> &'static [&'static str] {
    LEGACY_CODEX_EVENT_OPT_OUT_NOTIFICATION_METHODS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn notification_aliases_are_unique_and_stable() {
        let mut seen = HashSet::new();
        for group in NOTIFICATION_METHOD_ALIAS_GROUPS {
            assert!(
                seen.insert(group.canonical),
                "duplicate canonical notification method: {}",
                group.canonical
            );
            assert!(
                canonical_notification_method(group.canonical) == group.canonical,
                "canonical method should map to itself: {}",
                group.canonical
            );
            for alias in group.aliases {
                assert!(
                    seen.insert(alias),
                    "duplicate alias notification method: {alias}"
                );
                assert_eq!(
                    canonical_notification_method(alias),
                    group.canonical,
                    "alias should map to canonical"
                );
            }
        }
    }

    #[test]
    fn legacy_opt_out_methods_are_non_empty() {
        assert!(!LEGACY_CODEX_EVENT_OPT_OUT_NOTIFICATION_METHODS.is_empty());
        for method in LEGACY_CODEX_EVENT_OPT_OUT_NOTIFICATION_METHODS {
            assert!(!method.trim().is_empty());
        }
    }
}
