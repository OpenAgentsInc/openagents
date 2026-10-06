//! Sample activity for the four agent rows beneath the composer.

pub struct DemoAgent {
    pub name: &'static str,
    pub task: &'static str,
    pub tokens: &'static str,
    pub conversation: &'static [DemoMessage],
}

pub enum DemoMessage {
    User(&'static str),
    Tool(&'static str),
    Assistant(&'static str),
}

pub const DEMOS: [DemoAgent; 4] = [
    DemoAgent {
        name: "claude-code",
        task: "Reviewing keyboard navigation",
        tokens: "8.2k",
        conversation: &[
            DemoMessage::User("Review the composer keyboard navigation."),
            DemoMessage::Tool("Read the draft editor and checked paste handling"),
            DemoMessage::Assistant(
                "The composer keeps text editing local. Left and Right move through whole graphemes, and pasted text stays in the draft until you press Enter.",
            ),
            DemoMessage::User("Keep my draft when I switch conversations."),
            DemoMessage::Tool(
                "Checked draft and cursor restoration across four agent conversations",
            ),
            DemoMessage::Assistant(
                "Each conversation keeps its own draft and cursor. Switching agents restores the text exactly where you left it.",
            ),
        ],
    },
    DemoAgent {
        name: "codex",
        task: "Checking the footer placement",
        tokens: "12.4k",
        conversation: &[
            DemoMessage::User("Check the agent rail placement at different terminal sizes."),
            DemoMessage::Tool("Rendered the input and rail at 110, 80, and 24 columns"),
            DemoMessage::Assistant(
                "The rail stays below the input. Agent names remain visible, and every token count lines up on the right.",
            ),
            DemoMessage::User("What happens when the task description is too long?"),
            DemoMessage::Tool("Checked task truncation with a narrow viewport"),
            DemoMessage::Assistant(
                "Long task descriptions end with an ellipsis. Narrow terminals shorten the token label so the name and count still fit.",
            ),
        ],
    },
    DemoAgent {
        name: "devin-cli",
        task: "Inspecting the compact status row",
        tokens: "4.7k",
        conversation: &[
            DemoMessage::User("Inspect the compact plugin and wallet status row."),
            DemoMessage::Tool("Read the sample plugin labels and wallet balance"),
            DemoMessage::Assistant(
                "The status row shows six plugins and the sample balance of 24,000 sats. It fits beneath the four agent rows without crowding the input.",
            ),
            DemoMessage::User("Keep the status quiet while I read a conversation."),
            DemoMessage::Tool("Checked status colors against the conversation text"),
            DemoMessage::Assistant(
                "Plugin and wallet values use dim text. The selected conversation and its messages remain the focus.",
            ),
        ],
    },
    DemoAgent {
        name: "grok-build",
        task: "Verifying the preview colors",
        tokens: "3.1k",
        conversation: &[
            DemoMessage::User("Verify the terminal preview uses the Grok Night palette."),
            DemoMessage::Tool("Checked the RGB values used by the renderer"),
            DemoMessage::Assistant(
                "The preview uses the exact Grok Night RGB slots. The composer shares the base background, with quiet horizontal rules around the input.",
            ),
            DemoMessage::User("Do the exported previews use those same colors?"),
            DemoMessage::Tool("Compared the SVG export with the rendered terminal buffer"),
            DemoMessage::Assistant(
                "The SVG reads the same rendered cells as the terminal. Its foreground, background, and selected agent colors come from the same palette.",
            ),
        ],
    },
];
