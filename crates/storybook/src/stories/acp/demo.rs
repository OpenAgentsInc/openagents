//! Full ACP thread demo.

use maud::{Markup, html};
use ui::acp::atoms::{ToolKind, ToolStatus, AgentMode};
use ui::acp::organisms::*;
use ui::acp::sections::*;

pub fn acp_demo_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Thread Demo"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "A complete Claude Code conversation rendered with ACP components."
        }

        // Full thread container
        div class="border border-border bg-background max-w-3xl" {
            // Thread header
            (ThreadHeader::new("session-demo-001")
                .mode(AgentMode::Code)
                .model("claude-sonnet-4-20250514")
                .connection_status(ConnectionStatus::Connected)
                .build())

            // Thread controls
            (ThreadControls::new(AgentMode::Code, "claude-sonnet-4", "session-demo")
                .todos(vec![
                    PlanTodo { content: "Read current implementation".to_string(), completed: true },
                    PlanTodo { content: "Identify performance issues".to_string(), completed: true },
                    PlanTodo { content: "Implement optimizations".to_string(), completed: false },
                ])
                .build())

            // Messages area
            div class="p-4 space-y-4" {
                // User message
                (UserMessage::new(
                    "Can you help me optimize the sorting function in src/utils.rs? It seems slow for large arrays.",
                    "entry-1"
                )
                .timestamp("2:30 PM")
                .checkpoint("abc1234")
                .build())

                // Assistant thinking + response
                (AssistantMessage::new("entry-2")
                    .thinking("Let me first read the current implementation to understand what optimizations might be possible. I'll look for common performance issues like unnecessary allocations, suboptimal algorithms, or missing early exits.")
                    .text("I'll take a look at your sorting function. Let me read the file first to understand the current implementation.")
                    .timestamp("2:30 PM")
                    .build())

                // Read tool call
                (ToolCallCard::new(ToolKind::Read, "Read src/utils.rs", "entry-3")
                    .status(ToolStatus::Success)
                    .content(r#"pub fn sort_items(items: &mut Vec<Item>) {
    for i in 0..items.len() {
        for j in 0..items.len() - 1 {
            if items[j] > items[j + 1] {
                items.swap(j, j + 1);
            }
        }
    }
}"#)
                    .build())

                // Assistant analysis
                (AssistantMessage::new("entry-4")
                    .text("I see the issue! You're using a bubble sort algorithm which has O(n²) time complexity. For large arrays, this becomes very slow. Let me optimize it using a more efficient approach.")
                    .timestamp("2:31 PM")
                    .build())

                // Edit tool call
                (DiffToolCall::new("src/utils.rs", "entry-5")
                    .lines(vec![
                        DiffLine::Hunk("@@ -1,10 +1,5 @@".to_string()),
                        DiffLine::Del("pub fn sort_items(items: &mut Vec<Item>) {".to_string()),
                        DiffLine::Del("    for i in 0..items.len() {".to_string()),
                        DiffLine::Del("        for j in 0..items.len() - 1 {".to_string()),
                        DiffLine::Del("            if items[j] > items[j + 1] {".to_string()),
                        DiffLine::Del("                items.swap(j, j + 1);".to_string()),
                        DiffLine::Del("            }".to_string()),
                        DiffLine::Del("        }".to_string()),
                        DiffLine::Del("    }".to_string()),
                        DiffLine::Del("}".to_string()),
                        DiffLine::Add("pub fn sort_items(items: &mut Vec<Item>) {".to_string()),
                        DiffLine::Add("    items.sort();".to_string()),
                        DiffLine::Add("}".to_string()),
                    ])
                    .success()
                    .build())

                // Terminal tool call
                (TerminalToolCall::new("cargo test", "entry-6")
                    .output(r#"   Compiling project v0.1.0
    Finished test target in 0.5s
     Running unittests src/lib.rs

running 3 tests
test utils::test_sort_empty ... ok
test utils::test_sort_single ... ok
test utils::test_sort_many ... ok

test result: ok. 3 passed; 0 failed; 0 ignored"#)
                    .working_dir("/home/user/project")
                    .success()
                    .build())

                // Final response
                (AssistantMessage::new("entry-7")
                    .text("Done! I replaced the bubble sort with Rust's built-in `sort()` method which uses a highly optimized introsort algorithm. This reduces time complexity from O(n²) to O(n log n). All tests pass.")
                    .timestamp("2:32 PM")
                    .build())
            }

            // Feedback
            (ThreadFeedback::new("session-demo").build())

            // Message editor
            (MessageEditor::new("session-demo")
                .placeholder("Ask a follow-up question...")
                .build())
        }
    }
}
