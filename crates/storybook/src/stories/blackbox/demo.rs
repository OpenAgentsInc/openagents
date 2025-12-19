//! BlackBox full session demo.

use maud::{Markup, html};

use ui::blackbox::molecules::{ResultType, SessionMode, mode_indicator};
use ui::blackbox::organisms::{
    AgentLine, LifecycleEvent, McpLine, RecallLine, SubagentLine, ToolLine, UserLine, hour_divider,
    lifecycle_line, phase_line, time_marker,
};
use ui::blackbox::sections::{SessionHeader, SessionStats, ToolIndex, session_sidebar};

fn session_timeline() -> Markup {
    html! {
        div class="flex-1 p-4 overflow-y-auto bg-background" {
            (hour_divider("HOUR 0: ORIENTATION (00:00 - 01:00)"))

            (time_marker(0, 0, 0))

            (lifecycle_line(
                LifecycleEvent::Start { id: "sess_12h".to_string(), budget: 50.0, duration: "12h".to_string() },
                Some(1),
                Some((0, 0, 0)),
            ))

            (AgentLine::new("Starting 12-hour autonomous session. First, I'll check for relevant memories from past sessions and assess the current state of the codebase.")
                .step(2)
                .elapsed(0, 0, 2)
                .metrics(1200, 45, Some(800), 0.0008)
                .build())

            (time_marker(0, 0, 15))

            (RecallLine::new(vec!["platform", "priorities"])
                .step(4)
                .elapsed(0, 0, 15)
                .add_match("sess_20250617", "discussed Phase 1 roadmap - admin dashboard priority")
                .add_match("sess_20250615", "reviewed issue system design")
                .build())

            (AgentLine::new("Found relevant context. Phase 1 roadmap prioritizes admin dashboard. Let me check current issues and understand the codebase structure.")
                .step(5)
                .elapsed(0, 0, 30)
                .metrics(800, 62, Some(400), 0.0006)
                .build())

            (time_marker(0, 1, 0))

            (ToolLine::new("read", "docs/decisions/userstory.md", ResultType::Count { count: 19, unit: "lines".to_string() })
                .step(8)
                .elapsed(0, 1, 0)
                .call_id("call_1")
                .latency(120)
                .build())

            (ToolLine::new("read", "docs/decisions/agent-algorithms.md", ResultType::Blob {
                sha256: "a1b2c3d4".to_string(),
                bytes: 12847,
                mime: Some("text/markdown".to_string()),
            })
                .step(9)
                .elapsed(0, 1, 15)
                .call_id("call_2")
                .latency(85)
                .build())

            (time_marker(0, 2, 0))

            (McpLine::new("github.issues", "state=open", ResultType::Count { count: 8, unit: "issues".to_string() })
                .step(13)
                .call_id("call_4")
                .items(vec![
                    "#47 Admin dashboard monitoring (P0)",
                    "#48 Lease expiration background job",
                    "#49 Issue service unit tests",
                    "#50 Autopilot service refactoring",
                    "#51 Chat persistence verification",
                ])
                .build())

            (AgentLine::new("8 open issues. #47 Admin dashboard is P0 and aligns with Phase 1 roadmap. Let me explore the codebase structure to understand where to implement this.")
                .step(14)
                .elapsed(0, 2, 30)
                .metrics(2400, 89, Some(1800), 0.0015)
                .build())

            (time_marker(0, 4, 0))

            (ToolLine::new("grep", "TODO\\|FIXME type=rs", ResultType::Count { count: 12, unit: "matches".to_string() })
                .step(15)
                .elapsed(0, 4, 0)
                .call_id("call_5")
                .latency(340)
                .build())

            (SubagentLine::new("explore", "What are the main architectural components of this codebase?")
                .step(24)
                .elapsed(0, 4, 45)
                .tid(2)
                .session_id("sess_sub_1")
                .summary("6 crates: server, autopilot, chat_service, llm, daytona, storybook")
                .build())

            (AgentLine::new("Codebase has 6 crates. The server crate contains views and routes. I'll create a plan for the admin dashboard implementation.")
                .step(25)
                .elapsed(0, 7, 0)
                .metrics(3200, 124, Some(2400), 0.0022)
                .build())

            (hour_divider("HOUR 1-4: ADMIN DASHBOARD (01:00 - 05:00)"))

            (time_marker(1, 0, 0))

            (AgentLine::new("Beginning admin dashboard implementation. First, I need to understand the current aiur.rs structure and plan the monitoring panels.")
                .step(30)
                .elapsed(1, 0, 0)
                .metrics(1800, 95, Some(1200), 0.0012)
                .build())

            (ToolLine::new("read", "crates/server/src/views/pages/aiur.rs", ResultType::Count { count: 245, unit: "lines".to_string() })
                .step(31)
                .elapsed(1, 0, 30)
                .call_id("call_12")
                .latency(95)
                .preview("pub fn aiur_page(user: &User) -> Markup {\n    layout(\n        \"Admin Dashboard\",\n        ...\n    )\n}")
                .expanded(true)
                .build())

            (ToolLine::new("edit", "crates/server/src/views/pages/aiur.rs", ResultType::Ok)
                .step(35)
                .elapsed(1, 15, 0)
                .call_id("call_15")
                .latency(45)
                .build())

            (ToolLine::new("test", "cargo test -p platform aiur", ResultType::Count { count: 4, unit: "tests passed".to_string() })
                .step(40)
                .elapsed(1, 30, 0)
                .call_id("call_18")
                .latency(8400)
                .build())

            (time_marker(4, 0, 0))

            (lifecycle_line(
                LifecycleEvent::Checkpoint { hour: 4, tokens: 145000, cost: 12.30, budget_total: 50.0 },
                Some(72),
                Some((4, 0, 0)),
            ))

            (hour_divider("HOUR 4-5: AUTOPILOT REFACTORING (04:00 - 05:00)"))

            (AgentLine::new("Admin dashboard monitoring complete. Moving to autopilot service refactoring. This is a complex task - entering plan mode to design the approach.")
                .step(73)
                .elapsed(4, 0, 30)
                .metrics(2100, 78, Some(1600), 0.0014)
                .build())

            div class="border border-border bg-secondary/30 p-3 my-3" {
                div class="flex items-center gap-2 mb-2" {
                    (mode_indicator(SessionMode::Plan))
                }

                (phase_line("explore"))

                (SubagentLine::new("explore", "Understand autopilot_service.rs structure and identify extraction points")
                    .step(75)
                    .elapsed(4, 5, 0)
                    .tid(2)
                    .session_id("sess_sub_2")
                    .summary("3 extraction points identified: issue handling, sandbox management, LLM orchestration")
                    .build())

                (phase_line("design"))

                (SubagentLine::new("plan", "Design module extraction for autopilot service")
                    .step(78)
                    .elapsed(4, 12, 0)
                    .tid(3)
                    .session_id("sess_sub_3")
                    .summary("Proposed: Split into 4 files - mod.rs, issues.rs, sandbox.rs, orchestration.rs")
                    .build())

                (phase_line("review"))

                (ToolLine::new("read", "crates/server/src/services/autopilot_service.rs", ResultType::Count { count: 580, unit: "lines".to_string() })
                    .step(80)
                    .elapsed(4, 15, 0)
                    .call_id("call_39")
                    .latency(110)
                    .build())

                (phase_line("final"))

                (AgentLine::new("Plan finalized. Splitting autopilot_service.rs into 4 modules with clear boundaries. Exiting plan mode to begin implementation.")
                    .step(82)
                    .elapsed(4, 20, 0)
                    .metrics(1500, 92, Some(1100), 0.0010)
                    .build())

                (phase_line("exit"))
            }

            (time_marker(4, 25, 0))

            (ToolLine::new("edit", "crates/server/src/services/autopilot/mod.rs", ResultType::Ok)
                .step(85)
                .elapsed(4, 25, 0)
                .call_id("call_42")
                .latency(55)
                .build())

            (ToolLine::new("edit", "crates/server/src/services/autopilot/issues.rs", ResultType::Ok)
                .step(86)
                .elapsed(4, 28, 0)
                .call_id("call_43")
                .latency(48)
                .build())

            (ToolLine::new("test", "cargo test -p platform autopilot", ResultType::Count { count: 12, unit: "tests passed".to_string() })
                .step(90)
                .elapsed(4, 45, 0)
                .call_id("call_47")
                .latency(12400)
                .build())

            (hour_divider("HOUR 8: USER CHECK-IN (08:00 - 08:45)"))

            (time_marker(8, 0, 0))

            (lifecycle_line(
                LifecycleEvent::Pause { reason: "waiting for CI".to_string() },
                Some(95),
                Some((8, 0, 0)),
            ))

            (time_marker(8, 5, 0))

            (UserLine::new("Hey, looks good! I'm heading to bed. Quick thing - the Daytona API needs auth now, use DAYTONA_API_KEY env var.")
                .step(96)
                .elapsed(8, 5, 0)
                .build())

            (AgentLine::new("Acknowledged. I'll use the DAYTONA_API_KEY for Daytona API calls. Continuing with implementation once CI passes.")
                .step(97)
                .elapsed(8, 5, 30)
                .metrics(890, 45, Some(600), 0.0006)
                .build())

            (time_marker(8, 45, 0))

            (lifecycle_line(
                LifecycleEvent::Resume,
                Some(98),
                Some((8, 45, 0)),
            ))

            (hour_divider("HOUR 12: SESSION COMPLETE (12:00:00)"))

            (time_marker(12, 0, 0))

            (McpLine::new("github.pr_create", "title=\"Admin dashboard + autopilot refactor\" draft=false", ResultType::Ok)
                .step(120)
                .call_id("call_89")
                .items(vec![
                    "PR #201: Admin dashboard monitoring panels",
                    "PR #202: Autopilot service refactoring",
                    "PR #203: Issue service tests",
                    "PR #204: Service layer documentation",
                ])
                .build())

            (AgentLine::new("Session complete. Created 4 PRs addressing 8 issues. Admin dashboard monitoring is functional, autopilot service is now modular. Ready for review.")
                .step(121)
                .elapsed(12, 0, 0)
                .metrics(1200, 85, Some(900), 0.0008)
                .build())

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
    }
}

pub fn session_viewer_demo() -> Markup {
    html! {
        div class="flex flex-col h-[calc(100vh-4rem)] bg-background" {
            div class="flex items-center gap-4 px-4 py-3 border-b border-border bg-card" {
                div class="flex items-center gap-2 text-sm" {
                    a href="#" class="text-muted-foreground" { "Sessions" }
                    span class="text-muted-foreground" { ">" }
                    span class="text-foreground" { "sess_12h_20251218_001" }
                }
                span class="flex-1" {}
                div class="flex items-center gap-2" {
                    span class="text-xs text-muted-foreground" { "\u{25CF}" }
                    span class="text-xs text-muted-foreground" { "ENDED at 12:00:00" }
                }
                button class="bg-secondary border border-border text-muted-foreground px-3 py-1 text-xs" {
                    "\u{2699} Settings"
                }
                button class="bg-secondary border border-border text-muted-foreground px-3 py-1 text-xs" {
                    "\u{2193} Export"
                }
            }

            div class="flex flex-1 overflow-hidden" {
                (session_sidebar(
                    SessionHeader::new("sess_12h_20251218_001", "sonnet-4", "OpenAgentsInc/platform")
                        .mode(SessionMode::Auto)
                        .sha("215db51")
                        .branch("main")
                        .runner("daytona", "dtn_platform_12h_001")
                        .budget(50.0, "12h")
                        .mcp(vec!["github"]),
                    SessionMode::Auto,
                    None,
                    42.17,
                    50.0,
                    42.17,
                    None,
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
                        .add("test", 4)
                        .add("shell", 3)
                        .add("glob", 3),
                ))

                (session_timeline())
            }
        }
    }
}

pub fn blackbox_demo_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "BlackBox Demo" }
        p class="text-sm text-muted-foreground mb-6" {
            "Full session viewer demo showing a 12-hour autonomous autopilot session."
        }

        div class="border border-border overflow-hidden h-[800px]" {
            (session_viewer_demo())
        }

        div class="mt-8" {
            h3 class="text-sm font-medium text-muted-foreground mb-3" { "Components Used" }
            div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs text-muted-foreground" {
                div {
                    h4 class="text-xs uppercase tracking-widest mb-2" { "Sidebar" }
                    ul class="list-disc pl-4" {
                        li { "SessionHeader" }
                        li { "ModeIndicator" }
                        li { "PhaseIndicator" }
                        li { "BudgetMeter" }
                        li { "CostAccumulator" }
                        li { "SessionStats" }
                        li { "ToolIndex" }
                    }
                }
                div {
                    h4 class="text-xs uppercase tracking-widest mb-2" { "Timeline Lines" }
                    ul class="list-disc pl-4" {
                        li { "UserLine" }
                        li { "AgentLine" }
                        li { "ToolLine" }
                        li { "McpLine" }
                        li { "SubagentLine" }
                        li { "RecallLine" }
                        li { "LifecycleLine" }
                    }
                }
                div {
                    h4 class="text-xs uppercase tracking-widest mb-2" { "Markers" }
                    ul class="list-disc pl-4" {
                        li { "HourDivider" }
                        li { "TimeMarker" }
                        li { "PhaseLine" }
                    }
                }
            }
        }
    }
}
