//! Sample activity for the four agent rows beneath the composer.

pub struct DemoAgent {
    pub name: &'static str,
    pub task: &'static str,
    pub tokens: &'static str,
}

pub const DEMOS: [DemoAgent; 4] = [
    DemoAgent {
        name: "claude-code",
        task: "Reviewing keyboard navigation",
        tokens: "8.2k",
    },
    DemoAgent {
        name: "codex",
        task: "Checking the footer placement",
        tokens: "12.4k",
    },
    DemoAgent {
        name: "devin-cli",
        task: "Inspecting the compact status row",
        tokens: "4.7k",
    },
    DemoAgent {
        name: "grok-build",
        task: "Verifying the preview colors",
        tokens: "3.1k",
    },
];
