use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::organisms::{
    AssistantMessage, CodexEventCard, CodexEventTone, CodexMcpToolCallCard, CodexPlanCard,
    CodexPlanStep, CodexPlanStepStatus, CodexRateLimitCard, CodexRateLimitWindow,
    CodexRawResponseCard, CodexReasoningCard, CodexTerminalInteractionCard, CodexTokenUsageCard,
    DiffLine, DiffLineKind, DiffToolCall, TerminalToolCall, ToolCallCard,
};
use wgpui::{Bounds, Component, PaintContext, Point, theme};

use crate::constants::SECTION_GAP;
use crate::helpers::{draw_panel, panel_height};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_codex_events(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let entry_gap = 12.0;

        // ========== Panel 1: Thread + Turn Events ==========
        let lifecycle_height = panel_height(240.0);
        let lifecycle_bounds = Bounds::new(bounds.origin.x, y, width, lifecycle_height);
        draw_panel("Thread + Turn Events", lifecycle_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let mut thread_started = CodexEventCard::new("Thread started")
                .tag("thread/started")
                .tone(CodexEventTone::Info)
                .line("thread", "thread-74d3")
                .line("model", "gpt-5.2-codex")
                .line("cwd", "/workspace");
            let h = thread_started.size_hint().1.unwrap_or(72.0);
            thread_started.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut turn_started = CodexEventCard::new("Turn started")
                .tag("turn/started")
                .tone(CodexEventTone::Info)
                .line("turn", "turn-9021")
                .line("status", "in_progress");
            let h = turn_started.size_hint().1.unwrap_or(72.0);
            turn_started.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut turn_completed = CodexEventCard::new("Turn completed")
                .tag("turn/completed")
                .tone(CodexEventTone::Success)
                .line("turn", "turn-9021")
                .line("status", "completed")
                .line("duration", "12.4s");
            let h = turn_completed.size_hint().1.unwrap_or(72.0);
            turn_completed.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
        });
        y += lifecycle_height + SECTION_GAP;

        // ========== Panel 2: Item + Error Events ==========
        let item_height = panel_height(300.0);
        let item_bounds = Bounds::new(bounds.origin.x, y, width, item_height);
        draw_panel("Item + Error Events", item_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let mut item_started = CodexEventCard::new("Item started")
                .tag("item/started")
                .tone(CodexEventTone::Info)
                .line("item", "item-31f0")
                .line("type", "commandExecution");
            let h = item_started.size_hint().1.unwrap_or(72.0);
            item_started.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut item_completed = CodexEventCard::new("Item completed")
                .tag("item/completed")
                .tone(CodexEventTone::Success)
                .line("item", "item-31f0")
                .line("status", "completed")
                .line("exit", "0");
            let h = item_completed.size_hint().1.unwrap_or(72.0);
            item_completed.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut compacted = CodexEventCard::new("Context compacted")
                .tag("thread/compacted")
                .tone(CodexEventTone::Neutral)
                .line("thread", "thread-74d3")
                .line("turn", "turn-9021");
            let h = compacted.size_hint().1.unwrap_or(72.0);
            compacted.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut error = CodexEventCard::new("Turn error")
                .tag("error")
                .tone(CodexEventTone::Error)
                .line("message", "Upstream timeout")
                .line("will_retry", "false");
            let h = error.size_hint().1.unwrap_or(72.0);
            error.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
        });
        y += item_height + SECTION_GAP;

        // ========== Panel 3: Streaming Deltas ==========
        let stream_height = panel_height(420.0);
        let stream_bounds = Bounds::new(bounds.origin.x, y, width, stream_height);
        draw_panel("Streaming Deltas", stream_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let label = cx.text.layout(
                "item/agentMessage/delta",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut agent_message = AssistantMessage::new(
                "Streaming response: indexing workspace and preparing plan...",
            )
            .streaming(true);
            let agent_h = 100.0;
            agent_message.paint(Bounds::new(inner.origin.x, entry_y, entry_w, agent_h), cx);
            entry_y += agent_h + entry_gap;

            let label = cx.text.layout(
                "item/reasoning/summaryTextDelta + summaryPartAdded + textDelta",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut reasoning = CodexReasoningCard::new(
                Some("Summary: locate failing tests, narrow to failing module.".to_string()),
                Some("Raw reasoning chunk 1...\nRaw reasoning chunk 2...".to_string()),
            )
            .summary_expanded(true);
            let reasoning_h = reasoning.size_hint().1.unwrap_or(160.0);
            reasoning.paint(
                Bounds::new(inner.origin.x, entry_y, entry_w, reasoning_h),
                cx,
            );
            entry_y += reasoning_h + entry_gap;

            let label = cx.text.layout(
                "item/commandExecution/outputDelta",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut terminal = TerminalToolCall::new("cargo test -p autopilot")
                .status(ToolStatus::Running)
                .output("running 18 tests...\n");
            let term_h = 100.0;
            terminal.paint(Bounds::new(inner.origin.x, entry_y, entry_w, term_h), cx);
        });
        y += stream_height + SECTION_GAP;

        // ========== Panel 4: Tool Interactions ==========
        let tools_height = panel_height(260.0);
        let tools_bounds = Bounds::new(bounds.origin.x, y, width, tools_height);
        draw_panel("Tool Interactions", tools_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let label = cx.text.layout(
                "item/commandExecution/terminalInteraction",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut terminal_input = CodexTerminalInteractionCard::new("pty-402a", "y\\n");
            let h = terminal_input.size_hint().1.unwrap_or(70.0);
            terminal_input.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let label = cx.text.layout(
                "item/fileChange/outputDelta",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut apply_patch = ToolCallCard::new(ToolType::Edit, "apply_patch")
                .status(ToolStatus::Success)
                .output("Updated 2 files");
            let patch_h = 70.0;
            apply_patch.paint(Bounds::new(inner.origin.x, entry_y, entry_w, patch_h), cx);
            entry_y += patch_h + entry_gap;

            let label = cx.text.layout(
                "item/mcpToolCall/progress",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut mcp = CodexMcpToolCallCard::new("github", "search_issues")
                .status(ToolStatus::Running)
                .message("Scanning 12 repos...");
            let h = mcp.size_hint().1.unwrap_or(70.0);
            mcp.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
        });
        y += tools_height + SECTION_GAP;

        // ========== Panel 5: Plan + Diff ==========
        let plan_height = panel_height(360.0);
        let plan_bounds = Bounds::new(bounds.origin.x, y, width, plan_height);
        draw_panel("Plan + Diff", plan_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let label = cx.text.layout(
                "turn/plan/updated",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut plan = CodexPlanCard::new()
                .explanation("Refactor the parser and update tests")
                .steps(vec![
                    CodexPlanStep::new("Map request schema", CodexPlanStepStatus::Completed),
                    CodexPlanStep::new("Update handler paths", CodexPlanStepStatus::InProgress),
                    CodexPlanStep::new("Run tests and fix", CodexPlanStepStatus::Pending),
                ]);
            let plan_h = plan.size_hint().1.unwrap_or(160.0);
            plan.paint(Bounds::new(inner.origin.x, entry_y, entry_w, plan_h), cx);
            entry_y += plan_h + entry_gap;

            let label = cx.text.layout(
                "turn/diff/updated",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut diff = DiffToolCall::new("Turn diff snapshot")
                .status(ToolStatus::Success)
                .lines(vec![
                    DiffLine {
                        kind: DiffLineKind::Header,
                        content: "@@ -12,6 +12,12 @@".into(),
                        old_line: None,
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Context,
                        content: "fn handle_request(req: Request) {".into(),
                        old_line: Some(12),
                        new_line: Some(12),
                    },
                    DiffLine {
                        kind: DiffLineKind::Deletion,
                        content: "    parse_request(req);".into(),
                        old_line: Some(13),
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    let parsed = parse_request(req)?;".into(),
                        old_line: None,
                        new_line: Some(13),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    validate_request(&parsed)?;".into(),
                        old_line: None,
                        new_line: Some(14),
                    },
                ]);
            let diff_h = 160.0;
            diff.paint(Bounds::new(inner.origin.x, entry_y, entry_w, diff_h), cx);
        });
        y += plan_height + SECTION_GAP;

        // ========== Panel 6: Usage ==========
        let usage_height = panel_height(300.0);
        let usage_bounds = Bounds::new(bounds.origin.x, y, width, usage_height);
        draw_panel("Usage", usage_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let label = cx.text.layout(
                "thread/tokenUsage/updated",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut tokens = CodexTokenUsageCard::new(1240, 320, 980);
            let tokens_h = tokens.size_hint().1.unwrap_or(120.0);
            tokens.paint(Bounds::new(inner.origin.x, entry_y, entry_w, tokens_h), cx);
            entry_y += tokens_h + entry_gap;

            let label = cx.text.layout(
                "account/rateLimits/updated",
                Point::new(inner.origin.x, entry_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label);
            entry_y += 18.0;

            let mut rate_limits = CodexRateLimitCard::new()
                .plan_label("Pro")
                .credits_label("Unlimited")
                .windows(vec![
                    CodexRateLimitWindow::new("Primary", 42).resets_at("Resets in 12m"),
                    CodexRateLimitWindow::new("Secondary", 78).resets_at("Resets in 2h"),
                ]);
            let rate_h = rate_limits.size_hint().1.unwrap_or(140.0);
            rate_limits.paint(Bounds::new(inner.origin.x, entry_y, entry_w, rate_h), cx);
        });
        y += usage_height + SECTION_GAP;

        // ========== Panel 7: Account + MCP ==========
        let account_height = panel_height(300.0);
        let account_bounds = Bounds::new(bounds.origin.x, y, width, account_height);
        draw_panel("Account + MCP", account_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let mut account_updated = CodexEventCard::new("Account updated")
                .tag("account/updated")
                .tone(CodexEventTone::Info)
                .line("auth_mode", "chatgpt")
                .line("plan", "pro");
            let h = account_updated.size_hint().1.unwrap_or(72.0);
            account_updated.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut login_completed = CodexEventCard::new("Login completed")
                .tag("account/login/completed")
                .tone(CodexEventTone::Success)
                .line("login_id", "login-11d7")
                .line("success", "true");
            let h = login_completed.size_hint().1.unwrap_or(72.0);
            login_completed.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut mcp_login = CodexEventCard::new("MCP OAuth completed")
                .tag("mcpServer/oauthLogin/completed")
                .tone(CodexEventTone::Success)
                .line("server", "github")
                .line("success", "true");
            let h = mcp_login.size_hint().1.unwrap_or(72.0);
            mcp_login.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut legacy = CodexEventCard::new("Legacy notifications")
                .tag("deprecated")
                .tone(CodexEventTone::Neutral)
                .line("authStatusChange", "deprecated")
                .line("loginChatGptComplete", "deprecated")
                .line("sessionConfigured", "deprecated");
            let h = legacy.size_hint().1.unwrap_or(72.0);
            legacy.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
        });
        y += account_height + SECTION_GAP;

        // ========== Panel 8: Notices + Raw Response ==========
        let notices_height = panel_height(220.0);
        let notices_bounds = Bounds::new(bounds.origin.x, y, width, notices_height);
        draw_panel("Notices + Raw Response", notices_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            let mut raw = CodexRawResponseCard::new("{\"type\":\"response\",\"status\":\"ok\"}");
            let raw_h = raw.size_hint().1.unwrap_or(70.0);
            raw.paint(Bounds::new(inner.origin.x, entry_y, entry_w, raw_h), cx);
            entry_y += raw_h + entry_gap;

            let mut deprecation = CodexEventCard::new("Deprecation notice")
                .tag("deprecationNotice")
                .tone(CodexEventTone::Warning)
                .line("summary", "Legacy turn/start will be removed")
                .line("details", "Use thread/start + turn/start");
            let h = deprecation.size_hint().1.unwrap_or(72.0);
            deprecation.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
            entry_y += h + entry_gap;

            let mut windows_warning = CodexEventCard::new("World-writable warning")
                .tag("windows/worldWritableWarning")
                .tone(CodexEventTone::Warning)
                .line("sample_path", "C:\\\\Users\\\\Public")
                .line("extra_count", "2")
                .line("failed_scan", "false");
            let h = windows_warning.size_hint().1.unwrap_or(72.0);
            windows_warning.paint(Bounds::new(inner.origin.x, entry_y, entry_w, h), cx);
        });
    }
}
