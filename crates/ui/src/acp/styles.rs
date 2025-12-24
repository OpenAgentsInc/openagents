//! Shared CSS class constants for ACP components.
//!
//! These constants ensure consistent styling across all ACP components.

/// Card container for tool calls and messages
pub const ACP_CARD_CLASS: &str = "bg-card border border-border";

/// Header row within a card (icon + label + status)
pub const ACP_HEADER_CLASS: &str = "px-3 py-2 border-b border-border flex items-center gap-2";

/// Content area within a card
pub const ACP_CONTENT_CLASS: &str = "px-3 py-3 text-sm text-foreground";

/// Permission bar at bottom of tool call cards
pub const ACP_PERMISSION_BAR_CLASS: &str = "px-3 py-2 border-t border-border flex gap-2 items-center";

/// Footer area (actions, feedback)
pub const ACP_FOOTER_CLASS: &str = "px-3 py-2 border-t border-border";

/// Message container (user or assistant)
pub const ACP_MESSAGE_CLASS: &str = "mb-4";

/// User message specific styling
pub const ACP_USER_MESSAGE_CLASS: &str = "bg-secondary border border-border px-4 py-3";

/// Assistant message specific styling
pub const ACP_ASSISTANT_MESSAGE_CLASS: &str = "px-4 py-3";

/// Tool call container
pub const ACP_TOOL_CALL_CLASS: &str = "bg-card border border-border mb-2";

/// Collapsible details styling
pub const ACP_DETAILS_CLASS: &str = "group";

/// Details summary (clickable header)
pub const ACP_SUMMARY_CLASS: &str = "cursor-pointer list-none";

/// Diff container styling
pub const ACP_DIFF_CLASS: &str = "font-mono text-xs overflow-x-auto";

/// Diff addition line
pub const ACP_DIFF_ADD_CLASS: &str = "bg-green/10 text-green";

/// Diff deletion line
pub const ACP_DIFF_DEL_CLASS: &str = "bg-red/10 text-red";

/// Diff context line
pub const ACP_DIFF_CONTEXT_CLASS: &str = "text-muted-foreground";

/// Terminal output container
pub const ACP_TERMINAL_CLASS: &str = "bg-background font-mono text-xs p-3 overflow-x-auto";

/// Streaming indicator animation
pub const ACP_STREAMING_CLASS: &str = "animate-pulse";

/// Thread controls container
pub const ACP_THREAD_CONTROLS_CLASS: &str = "flex items-center gap-3 px-4 py-2 border-b border-border";

/// Message editor container
pub const ACP_EDITOR_CLASS: &str = "border-t border-border p-4";
