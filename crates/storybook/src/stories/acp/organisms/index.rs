//! ACP organisms index story.

use maud::{Markup, html};
use ui::acp::atoms::{ToolKind, ToolStatus, AgentMode};
use ui::acp::organisms::*;

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

pub fn organisms_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Organisms"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Complex UI components for ACP thread rendering."
        }

        // User Message
        (section_title("User Message"))
        (section(html! {
            (UserMessage::new("Can you help me refactor this function to be more efficient?", "user-1")
                .timestamp("10:30 AM")
                .checkpoint("abc1234")
                .build())
        }))

        // Assistant Message
        (section_title("Assistant Message"))
        (section(html! {
            (AssistantMessage::new("assistant-1")
                .text("I'd be happy to help! Let me analyze the function and suggest improvements.")
                .thinking("First, I'll read the file to understand the current implementation...")
                .timestamp("10:31 AM")
                .build())
        }))

        // Tool Call Card
        (section_title("Tool Call Card"))
        (section(html! {
            (ToolCallCard::new(ToolKind::Read, "Read file", "tool-1")
                .status(ToolStatus::Success)
                .content("Contents of the file...")
                .build())
        }))

        // Terminal Tool Call
        (section_title("Terminal Tool Call"))
        (section(html! {
            (TerminalToolCall::new("cargo test", "terminal-1")
                .output("running 5 tests\ntest test_one ... ok\ntest test_two ... ok\n\ntest result: ok. 5 passed; 0 failed")
                .working_dir("/home/user/project")
                .success()
                .build())
        }))

        // Diff Tool Call
        (section_title("Diff Tool Call"))
        (section(html! {
            (DiffToolCall::new("src/lib.rs", "diff-1")
                .lines(vec![
                    DiffLine::Hunk("@@ -10,6 +10,8 @@".to_string()),
                    DiffLine::Context("fn main() {".to_string()),
                    DiffLine::Del("    println!(\"Hello\");".to_string()),
                    DiffLine::Add("    println!(\"Hello, World!\");".to_string()),
                    DiffLine::Add("    println!(\"Welcome!\");".to_string()),
                    DiffLine::Context("}".to_string()),
                ])
                .success()
                .build())
        }))

        // Search Tool Call
        (section_title("Search Tool Call"))
        (section(html! {
            (SearchToolCall::new("fn main", "search-1")
                .results(vec![
                    SearchResult { path: "src/main.rs".to_string(), preview: Some("fn main() {".to_string()), line_number: Some(1) },
                    SearchResult { path: "examples/demo.rs".to_string(), preview: Some("fn main() {".to_string()), line_number: Some(5) },
                ])
                .success()
                .build())
        }))

        // Thread Controls
        (section_title("Thread Controls"))
        (section(html! {
            (ThreadControls::new(AgentMode::Plan, "claude-sonnet-4", "session-1")
                .todos(vec![
                    PlanTodo { content: "Read the file".to_string(), completed: true },
                    PlanTodo { content: "Analyze the code".to_string(), completed: true },
                    PlanTodo { content: "Implement changes".to_string(), completed: false },
                    PlanTodo { content: "Run tests".to_string(), completed: false },
                ])
                .build())
        }))

        // Permission Dialog
        (section_title("Permission Dialog"))
        (section(html! {
            div class="max-w-md" {
                (PermissionDialog::new(
                    ToolKind::Execute,
                    "Bash",
                    "Claude wants to run a command in your terminal.",
                    "perm-1"
                )
                .details("rm -rf node_modules && npm install")
                .build())
            }
        }))
    }
}
