//! Recorder organism components story.

use maud::{Markup, html};
use ui::recorder::molecules::ResultType;
use ui::recorder::organisms::{
    AgentLine, LifecycleEvent, McpLine, QuestionLine, RecallLine, SubagentLine, ToolLine, UserLine,
    hour_divider, lifecycle_line, phase_line, time_marker,
};

use super::shared::section;

fn story_section(title: &str, content: Markup) -> Markup {
    html! {
        div class="mb-8" {
            h3 class="text-sm font-medium text-muted-foreground mb-3 border-b border-border pb-2" { (title) }
            (content)
        }
    }
}

pub fn recorder_organisms_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Recorder Organisms" }
        p class="text-sm text-muted-foreground mb-6" {
            "Complete line-level components for rendering Recorder log entries."
        }

        (section(story_section("UserLine", html! {
            (UserLine::new("Hey, looks good! I'm heading to bed. Quick thing - the Daytona API needs auth.")
                .step(43)
                .elapsed(0, 9, 5)
                .build())
        })))

        (section(story_section("AgentLine", html! {
            (AgentLine::new("Acknowledged. I'll use the token for Daytona API calls. Continuing with implementation.")
                .step(46)
                .elapsed(0, 9, 18)
                .metrics(2400, 62, Some(1800), 0.0018)
                .build())
        })))

        (section(story_section("ToolLine", html! {
            div class="flex flex-col gap-3" {
                (ToolLine::new("read", "docs/decisions/userstory.md", ResultType::Count { count: 19, unit: "lines".to_string() })
                    .step(8)
                    .elapsed(0, 1, 0)
                    .call_id("call_1")
                    .latency(120)
                    .preview("User logs in with GitHub, selects repo\nSingle button: full auto toggle (starts off)\nSingle text input to communicate with Auto...")
                    .expanded(true)
                    .build())

                (ToolLine::new("grep", "TODO type=rs", ResultType::Count { count: 7, unit: "matches".to_string() })
                    .step(15)
                    .elapsed(0, 2, 30)
                    .call_id("call_5")
                    .latency(45)
                    .build())

                (ToolLine::new("shell", "rm -rf /protected", ResultType::Error("permission denied".to_string()))
                    .step(56)
                    .elapsed(0, 35, 0)
                    .build())
            }
        })))

        (section(story_section("McpLine", html! {
            (McpLine::new("github.issues", "state=open", ResultType::Count { count: 8, unit: "issues".to_string() })
                .step(13)
                .call_id("call_4")
                .items(vec![
                    "#47 Admin dashboard (P0)",
                    "#48 Lease expiration job",
                    "#49 Issue service tests",
                ])
                .build())
        })))

        (section(story_section("SubagentLine", html! {
            (SubagentLine::new("explore", "What are the main architectural components?")
                .step(24)
                .elapsed(0, 4, 45)
                .tid(2)
                .session_id("sess_sub_1")
                .summary("6 crates identified")
                .build())
        })))

        (section(story_section("RecallLine", html! {
            (RecallLine::new(vec!["platform", "priorities"])
                .step(4)
                .elapsed(0, 0, 15)
                .add_match("sess_20251217", "discussed Phase 1 roadmap")
                .add_match("sess_20251215", "reviewed issue system design")
                .build())
        })))

        (section(story_section("QuestionLine", html! {
            div class="flex flex-col gap-3" {
                (QuestionLine::new("Which auth library should we use?")
                    .options(vec!["JWT", "OAuth", "Session"])
                    .selected("OAuth")
                    .step(30)
                    .elapsed(0, 4, 22)
                    .build())

                (QuestionLine::new("Which approach for caching?")
                    .options(vec!["Redis", "In-memory", "File-based"])
                    .auto_selected("Redis", "existing infrastructure")
                    .step(45)
                    .elapsed(0, 8, 0)
                    .build())

                (QuestionLine::new("Should I proceed with the refactor?")
                    .options(vec!["Yes", "No", "Need more info"])
                    .step(60)
                    .elapsed(0, 12, 0)
                    .build())
            }
        })))

        (section(story_section("LifecycleLine", html! {
            div class="flex flex-col gap-3" {
                (lifecycle_line(
                    LifecycleEvent::Start { id: "sess_12h".to_string(), budget: 50.0, duration: "12h".to_string() },
                    Some(1),
                    Some((0, 0, 0)),
                ))

                (lifecycle_line(
                    LifecycleEvent::Checkpoint { hour: 4, tokens: 145000, cost: 12.30, budget_total: 50.0 },
                    Some(72),
                    Some((4, 0, 0)),
                ))

                (lifecycle_line(
                    LifecycleEvent::Pause { reason: "waiting for CI".to_string() },
                    Some(85),
                    Some((8, 0, 0)),
                ))

                (lifecycle_line(
                    LifecycleEvent::Resume,
                    Some(86),
                    Some((8, 45, 0)),
                ))

                (lifecycle_line(
                    LifecycleEvent::End {
                        summary: "8 issues closed, 4 PRs merged".to_string(),
                        issues_completed: 8,
                        prs_merged: 4,
                        cost: 42.17,
                        duration: "12h 0m".to_string(),
                    },
                    Some(125),
                    Some((12, 0, 0)),
                ))
            }
        })))

        (section(story_section("PhaseLine", html! {
            (phase_line("explore"))
            (phase_line("design"))
            (phase_line("review"))
            (phase_line("final"))
            (phase_line("exit"))
        })))

        (section(story_section("TimeMarker", html! {
            (time_marker(0, 0, 0))
            (time_marker(0, 15, 0))
            (time_marker(4, 0, 0))
        })))

        (section(story_section("HourDivider", html! {
            (hour_divider("HOUR 0: ORIENTATION"))
            (hour_divider("HOUR 1-4: ADMIN DASHBOARD"))
            (hour_divider("HOUR 4-5: AUTOPILOT REFACTORING"))
        })))
    }
}
