//! Recorder section components story.

use maud::{Markup, html};
use ui::recorder::molecules::{PlanPhase, SessionMode};
use ui::recorder::sections::{SessionHeader, SessionStats, ToolIndex, session_sidebar};

use super::shared::section;

fn story_section(title: &str, content: Markup) -> Markup {
    html! {
        div class="mb-8" {
            h3 class="text-sm font-medium text-muted-foreground mb-3 border-b border-border pb-2" { (title) }
            (content)
        }
    }
}

pub fn recorder_sections_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Recorder Sections" }
        p class="text-sm text-muted-foreground mb-6" {
            "Sidebar and header section components for the session viewer."
        }

        (section(story_section("SessionHeader", html! {
            div class="max-w-sm" {
                (SessionHeader::new("sess_12h_20251218_001", "sonnet-4", "OpenAgentsInc/platform")
                    .mode(SessionMode::Auto)
                    .sha("215db51")
                    .branch("main")
                    .runner("daytona", "dtn_platform_12h_001")
                    .budget(50.0, "12h")
                    .skills(vec![])
                    .mcp(vec!["github"])
                    .build())
            }
        })))

        (section(story_section("SessionStats", html! {
            div class="max-w-sm" {
                (SessionStats {
                    lines: 728,
                    duration: "12h 0m".to_string(),
                    cost: 42.17,
                    user_msgs: 3,
                    agent_msgs: 14,
                    tool_calls: 55,
                    mcp_calls: 21,
                    subagents: 4,
                    questions: 0,
                    phases: 5,
                    blobs: 4,
                    redacted: 1,
                }.build())
            }
        })))

        (section(story_section("ToolIndex", html! {
            div class="max-w-sm" {
                (ToolIndex::new()
                    .add("read", 18)
                    .add("grep", 12)
                    .add("edit", 8)
                    .add("git", 6)
                    .add("test", 4)
                    .add("shell", 3)
                    .add("glob", 3)
                    .build())
            }
        })))

        (section(story_section("Full Sidebar", html! {
            div class="h-[600px] border border-border overflow-hidden" {
                (session_sidebar(
                    SessionHeader::new("sess_12h_20251218_001", "sonnet-4", "OpenAgentsInc/platform")
                        .mode(SessionMode::Auto)
                        .sha("215db51")
                        .runner("daytona", "dtn_platform_12h_001")
                        .budget(50.0, "12h")
                        .mcp(vec!["github"]),
                    SessionMode::Plan,
                    Some(PlanPhase::Design),
                    12.30,
                    50.0,
                    12.47,
                    Some(0.02),
                    SessionStats {
                        lines: 728,
                        duration: "12h 0m".to_string(),
                        cost: 42.17,
                        user_msgs: 3,
                        agent_msgs: 14,
                        tool_calls: 55,
                        mcp_calls: 21,
                        subagents: 4,
                        questions: 0,
                        phases: 5,
                        blobs: 4,
                        redacted: 1,
                    },
                    ToolIndex::new()
                        .add("read", 18)
                        .add("grep", 12)
                        .add("edit", 8)
                        .add("git", 6)
                        .add("test", 4),
                ))
            }
        })))
    }
}
