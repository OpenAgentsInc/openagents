//! Sample activity for the four agent rows beneath the composer.

use crate::tools::{ToolCall, ToolKind, ToolState};

pub struct DemoAgent {
    pub name: &'static str,
    pub task: &'static str,
    pub tokens: &'static str,
    pub conversation: &'static [DemoMessage],
}

pub enum DemoMessage {
    User(&'static str),
    Tool(ToolCall),
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
        output: "- SetCursorStyle::SteadyBar\n+ SetCursorStyle::BlinkingBlock",
        state: ToolState::Complete,
    },
    ToolCall {
        kind: ToolKind::Run,
        input: "cargo fmt -p coder-new --check",
        output: "Exit 0 · formatting passed",
        state: ToolState::Complete,
    },
];

pub const DEMOS: [DemoAgent; 4] = [
    DemoAgent {
        name: "claude-code",
        task: "Reviewing keyboard navigation",
        tokens: "8.2k",
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
            DemoMessage::Assistant(
                "Agent names and token counts stay visible. Long tasks end with an ellipsis, and narrow terminals shorten the token label.",
            ),
            DemoMessage::User("Verify there are no blank rows between the input and the rail."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Run,
                input: "cargo test -p coder-new header_and_rail_use_compact_spacing_at_the_terminal_bottom",
                output: "Checking consecutive rows and the final terminal row",
                state: ToolState::Running,
            }),
        ],
    },
    DemoAgent {
        name: "devin-cli",
        task: "Checking conversation switching",
        tokens: "4.7k",
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
        conversation: &[
            DemoMessage::User("Verify the terminal preview colors."),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Read,
                input: "crates/coder-new/src/theme.rs",
                output: "Grok Night colors and the shared Coder background",
                state: ToolState::Complete,
            }),
            DemoMessage::Tool(ToolCall {
                kind: ToolKind::Search,
                input: "\"NEAR_BLACK\" crates/coder-ui/src/theme.rs",
                output: "#0a0a0a · shared near-black background",
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
