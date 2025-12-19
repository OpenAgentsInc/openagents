//! BlackBox atom components story.

use maud::{Markup, html};
use ui::{
    CallType, LineType, StatusState, attempt_badge, blob_ref, call_id_badge, cost_badge,
    latency_badge, line_type_label, redacted_value, result_arrow, status_dot, step_badge,
    tid_badge, timestamp_badge_elapsed, timestamp_badge_wall, token_badge,
};

fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" {
            (title)
        }
    }
}

fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" {
            (content)
        }
    }
}

fn row(content: Markup) -> Markup {
    html! {
        div class="flex gap-6 items-center flex-wrap" {
            (content)
        }
    }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn atoms_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Atoms"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Atomic UI primitives for rendering BlackBox session logs."
        }

        (section_title("Status Dot"))
        (section(row(html! {
            (item("Success", status_dot(StatusState::Success)))
            (item("Running", status_dot(StatusState::Running)))
            (item("Pending", status_dot(StatusState::Pending)))
            (item("Error", status_dot(StatusState::Error)))
            (item("Skipped", status_dot(StatusState::Skipped)))
        })))

        (section_title("Line Type Label"))
        (section(html! {
            (row(html! {
                (item("User", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::User)) }))
                (item("Agent", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Agent)) }))
                (item("Tool", html! { (status_dot(StatusState::Running)) " " (line_type_label(LineType::Tool)) }))
                (item("Observation", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Observation)) }))
            }))
            (row(html! {
                (item("Plan", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Plan)) }))
                (item("Mode", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Mode)) }))
                (item("Recall", html! { (status_dot(StatusState::Success)) " " (line_type_label(LineType::Recall)) }))
                (item("Subagent", html! { (status_dot(StatusState::Running)) " " (line_type_label(LineType::Subagent)) }))
            }))
            (row(html! {
                (item("MCP", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Mcp)) }))
                (item("Question", html! { (status_dot(StatusState::Pending)) " " (line_type_label(LineType::Question)) }))
                (item("Comment", line_type_label(LineType::Comment)))
                (item("Lifecycle", line_type_label(LineType::Lifecycle)))
                (item("Phase", line_type_label(LineType::Phase)))
                (item("Skill", line_type_label(LineType::Skill)))
            }))
        }))

        (section_title("Step Badge"))
        (section(row(html! {
            (item("Step 1", step_badge(1)))
            (item("Step 42", step_badge(42)))
            (item("Step 125", step_badge(125)))
        })))

        (section_title("Timestamp Badge"))
        (section(row(html! {
            (item("Elapsed", timestamp_badge_elapsed(0, 15, 23)))
            (item("Elapsed (4h)", timestamp_badge_elapsed(4, 0, 0)))
            (item("Wall clock", timestamp_badge_wall("03:21:08Z")))
        })))

        (section_title("Call ID Badge"))
        (section(row(html! {
            (item("Tool call", call_id_badge("call_47", CallType::Tool)))
            (item("MCP call", call_id_badge("call_13", CallType::Mcp)))
            (item("Subagent", call_id_badge("sub_1", CallType::Subagent)))
        })))

        (section_title("Cost Badge"))
        (section(row(html! {
            (item("Low (<$0.01)", cost_badge(0.0018)))
            (item("Medium ($0.01-0.10)", cost_badge(0.0456)))
            (item("High (>$0.10)", cost_badge(0.2345)))
        })))

        (section_title("Token Badge"))
        (section(row(html! {
            (item("Small", token_badge(520, 80, None)))
            (item("Large", token_badge(2400, 62, None)))
            (item("With cached", token_badge(12400, 890, Some(8000))))
        })))

        (section_title("Latency Badge"))
        (section(row(html! {
            (item("Fast (<1s)", latency_badge(340)))
            (item("Medium (1-5s)", latency_badge(2500)))
            (item("Slow (>5s)", latency_badge(8400)))
        })))

        (section_title("Attempt Badge"))
        (section(row(html! {
            (item("Retry 2/3", attempt_badge(2, 3)))
            (item("Retry 3/3", attempt_badge(3, 3)))
        })))

        (section_title("TID Badge"))
        (section(row(html! {
            (item("Main (tid:1)", tid_badge(1)))
            (item("Thread 2", tid_badge(2)))
            (item("Thread 3", tid_badge(3)))
            (item("Thread 4", tid_badge(4)))
        })))

        (section_title("Blob Ref"))
        (section(row(html! {
            (item("Small", blob_ref("a1b2c3d4e5f6", 1024, Some("text/plain"))))
            (item("Medium", blob_ref("f1a2b3c4d5e6", 12847, Some("text/markdown"))))
            (item("Large", blob_ref("deadbeef1234", 1048576, Some("application/octet-stream"))))
        })))

        (section_title("Redacted Value"))
        (section(row(html! {
            (item("API key", redacted_value("api_key")))
            (item("Token", redacted_value("github_token")))
            (item("Env var", redacted_value("env_var")))
        })))

        (section_title("Result Arrow"))
        (section(row(html! {
            (item("Arrow", result_arrow()))
            (item("With result", html! { (result_arrow()) "[ok]" }))
            (item("With count", html! { (result_arrow()) "[186 lines]" }))
        })))

        (section_title("Combined Examples"))
        (section(html! {
            div class="flex flex-col gap-3" {
                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Tool))
                    span class="text-foreground ml-1" { "read" }
                    span class="flex-1" {}
                    (step_badge(42))
                    (timestamp_badge_elapsed(0, 15, 23))
                    (call_id_badge("call_42", CallType::Tool))
                    (latency_badge(340))
                }

                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Agent))
                    span class="flex-1" {}
                    (step_badge(46))
                    (timestamp_badge_elapsed(0, 9, 18))
                    (token_badge(2400, 62, Some(1800)))
                    (cost_badge(0.0018))
                }

                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Running))
                    (line_type_label(LineType::Subagent))
                    span class="text-foreground ml-1" { "explore" }
                    span class="flex-1" {}
                    (step_badge(24))
                    (timestamp_badge_elapsed(0, 4, 45))
                    (tid_badge(2))
                    (call_id_badge("sub_1", CallType::Subagent))
                }

                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Mcp))
                    span class="text-foreground ml-1" { "github.issues" }
                    (result_arrow())
                    span class="text-muted-foreground" { "[8 issues]" }
                    span class="flex-1" {}
                    (step_badge(13))
                    (call_id_badge("call_4", CallType::Mcp))
                }

                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Tool))
                    span class="text-foreground ml-1" { "read" }
                    (result_arrow())
                    (blob_ref("a1b2c3d4", 12847, Some("text/markdown")))
                }

                div class="flex items-center gap-2 bg-card border border-red p-2 border-l-2" {
                    (status_dot(StatusState::Error))
                    (line_type_label(LineType::Tool))
                    span class="text-foreground ml-1" { "shell" }
                    (result_arrow())
                    span class="text-red" { "[err: permission denied]" }
                    span class="flex-1" {}
                    (attempt_badge(2, 3))
                }

                div class="flex items-center gap-2 bg-card border border-border p-2" {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Tool))
                    span class="text-foreground ml-1" { "shell" }
                    span class="text-muted-foreground ml-1 opacity-60" { "export DAYTONA_API_KEY=" }
                    (redacted_value("api_key"))
                    (result_arrow())
                    span class="text-green" { "[ok]" }
                }
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::{
    CallType, LineType, StatusState, attempt_badge, blob_ref, call_id_badge, cost_badge,
    latency_badge, line_type_label, redacted_value, result_arrow, status_dot, step_badge,
    tid_badge, timestamp_badge_elapsed, timestamp_badge_wall, token_badge,
};

// Status indicators
status_dot(StatusState::Success)
status_dot(StatusState::Running)

// Line type labels (combined with status dot)
html! {
    (status_dot(StatusState::Success))
    " "
    (line_type_label(LineType::Tool))
}

// Metadata badges
step_badge(42)
timestamp_badge_elapsed(0, 15, 23)
call_id_badge("call_47", CallType::Tool)
cost_badge(0.0018)
token_badge(2400, 62, Some(1800))
latency_badge(340)
tid_badge(2)

// Special content
blob_ref("a1b2c3d4", 12847, Some("text/markdown"))
redacted_value("api_key")
result_arrow()"#))
    }
}
