//! Sample activity for the four agent rows beneath the composer.

use super::tools::{PluginCall, ToolCall, ToolKind, ToolState};

pub struct DemoAgent {
    pub name: &'static str,
    pub task: &'static str,
    pub tokens: &'static str,
    pub elapsed_seconds: u64,
    pub conversation: &'static [DemoMessage],
}

pub fn elapsed_time(seconds: u64) -> String {
    let minutes = seconds / 60 % 60;
    let hours = seconds / 3_600;
    if hours >= 24 {
        format!("{}d {}h {minutes}m", hours / 24, hours % 24)
    } else if hours > 0 {
        format!("{hours}h {minutes}m {}s", seconds % 60)
    } else if seconds >= 60 {
        format!("{minutes}m {}s", seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

pub enum DemoMessage {
    User(&'static str),
    Tool(ToolCall),
    Plugin(PluginCall),
    Assistant(&'static str),
}

pub const MAIN_TOOLS: [ToolCall; 4] = [
    ToolCall {
        kind: ToolKind::Read,
        input: "docs/coder-new/",
        output: "4 documents reviewed",
        state: ToolState::Complete,
    },
    ToolCall {
        kind: ToolKind::Search,
        input: "\"composer|agent_rail\" crates/coder-new/src/",
        output: "8 matches in 3 files",
        state: ToolState::Complete,
    },
    ToolCall {
        kind: ToolKind::Edit,
        input: "crates/coder-new/src/main.rs",
        output: "@@ -48 +48 @@\n-let cursor = (\"▏\", SetCursorStyle::SteadyBar);\n+let cursor = (\"█\", SetCursorStyle::BlinkingBlock);",
        state: ToolState::Complete,
    },
    ToolCall {
        kind: ToolKind::Run,
        input: "cargo fmt -p coder-new --check",
        output: "Exit 0 · formatting passed",
        state: ToolState::Complete,
    },
];

pub const MAIN_PLUGINS: [PluginCall; 2] = [
    PluginCall {
        plugin: "terminal-inspector",
        operation: "layout.inspect",
        input: "110×36 · composer and agent rail",
        output: "4 agent rows · aligned names, tasks, and tokens",
        state: ToolState::Complete,
    },
    PluginCall {
        plugin: "palette-audit",
        operation: "colors.check",
        input: "USGC · #0a0a0a",
        output: "Comparing tool accents and diff backgrounds",
        state: ToolState::Running,
    },
];

pub const DEMOS: [DemoAgent; 4] = [
    DemoAgent {
        name: "claude-code",
        task: "Reviewing keyboard navigation",
        tokens: "8.2k",
        elapsed_seconds: 4_358,
        conversation: &[
            DemoMessage::User("Review the composer keyboard navigation."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Read,
                input: "crates/coder-new/src/lib.rs",
                output: "Draft editing and per-conversation state",
                state: ToolState::Complete,
            }),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Search,
                input: "\"KeyCode|KeyEventKind\" crates/coder-new/src/lib.rs",
                output: "18 matches · editing, selection, and key releases",
                state: ToolState::Complete,
            }),
            DemoMessage::Plugin(PluginCall {
                plugin: "keyboard-audit",
                operation: "navigation.check",
                input: "",
                output: "Draft and cursor restored across conversation switches",
                state: ToolState::Complete,
            }),
            DemoMessage::Assistant(
                "The input keeps pasted text local and restores each conversation's draft. Left and Right move through whole graphemes.",
            ),
            DemoMessage::User("Check cursor restoration while switching agents."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo test -p coder-new switching_restores_each_conversations_draft_cursor_messages_and_scroll",
                output: "Checking draft, cursor, messages, and scroll restoration",
                state: ToolState::Running,
            }),
        ],
    },
    DemoAgent {
        name: "codex",
        task: "Checking the agent rail placement",
        tokens: "12.4k",
        elapsed_seconds: 967,
        conversation: &[
            DemoMessage::User("Check the agent rail at wide and narrow terminal sizes."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Read,
                input: "crates/coder-new/src/ui.rs",
                output: "Composer rules, agent columns, and token alignment",
                state: ToolState::Complete,
            }),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo test -p coder-new selected_agent_keeps_the_rail_visible_and_tokens_aligned_after_resize",
                output: "Exit 0 · 80- and 24-column views checked",
                state: ToolState::Complete,
            }),
            DemoMessage::Plugin(PluginCall {
                plugin: "terminal-inspector",
                operation: "layout.inspect",
                input: "80 and 24 columns · composer and agent rail",
                output: "4 consecutive agent rows · token counts aligned",
                state: ToolState::Complete,
            }),
            DemoMessage::Assistant(
                "Agent names and token counts stay visible. Long tasks end with an ellipsis, and narrow terminals shorten the token label.",
            ),
            DemoMessage::User("Verify there are no blank rows between the input and the rail."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo test -p coder-new header_and_rail_keep_compact_spacing_above_the_bottom_margin",
                output: "Checking consecutive rows and the bottom margin",
                state: ToolState::Running,
            }),
        ],
    },
    DemoAgent {
        name: "devin-cli",
        task: "Checking conversation switching",
        tokens: "4.7k",
        elapsed_seconds: 271,
        conversation: &[
            DemoMessage::User("Keep each agent conversation's draft independent."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Read,
                input: "crates/coder-new/src/lib.rs",
                output: "Conversation selection and saved draft state",
                state: ToolState::Complete,
            }),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Search,
                input: "\"saved_chats|select_agent\" crates/coder-new/src/lib.rs",
                output: "5 conversation slots · main and four agents",
                state: ToolState::Complete,
            }),
            DemoMessage::Plugin(PluginCall {
                plugin: "conversation-audit",
                operation: "state.check",
                input: "Main and four agents · multiline drafts",
                output: "5 independent drafts · cursor, messages, and scroll retained",
                state: ToolState::Complete,
            }),
            DemoMessage::Assistant(
                "Each conversation keeps its own draft, messages, and scroll position. Switching restores the cursor where you left it.",
            ),
            DemoMessage::User("Exercise a switch away from a multiline draft."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo test -p coder-new switching_restores_each_conversations_draft_cursor_messages_and_scroll",
                output: "Switching across all four agents and back to main",
                state: ToolState::Running,
            }),
        ],
    },
    DemoAgent {
        name: "grok-build",
        task: "Verifying the preview colors",
        tokens: "3.1k",
        elapsed_seconds: 45,
        conversation: &[
            DemoMessage::User("Verify the terminal preview colors."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Read,
                input: "crates/coder-new/src/theme.rs",
                output: "USGC accents and the shared Coder background",
                state: ToolState::Complete,
            }),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Edit,
                input: "crates/coder-new/src/theme.rs",
                output: "@@ -8,3 +8,4 @@\n fn diff_style() -> Style {\n-    Style::default().fg(Color::Green)\n+    let background = Color::Rgb(0, 41, 17);\n+    Style::default().bg(background)\n }",
                state: ToolState::Complete,
            }),
            DemoMessage::Plugin(PluginCall {
                plugin: "palette-audit",
                operation: "colors.check",
                input: "Terminal and SVG · USGC · #0a0a0a",
                output: "Shared background and tool accents match the exported cells",
                state: ToolState::Complete,
            }),
            DemoMessage::Assistant(
                "The terminal and SVG export use the same colors. The composer shares Coder's near-black background and uses quiet horizontal rules.",
            ),
            DemoMessage::User("Compare the terminal and exported preview."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo run -p coder-new -- --snapshot",
                output: "Rendering tool rows, delegation components, and the composer",
                state: ToolState::Running,
            }),
        ],
    },
];
