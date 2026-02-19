use wgpui::components::atoms::{
    AgentStatus, AgentType, ApmGauge, ApmLevel, BreadcrumbItem, IssueStatus, Mode, RelayStatus,
    SessionBreadcrumb, SessionStatus, SessionStatusBadge, ToolStatus, ToolType, TrajectoryStatus,
};
use wgpui::components::molecules::{
    AddressCard, AddressType, AgentProfileCard, AgentProfileInfo, ApmComparisonCard,
    ApmSessionData, ApmSessionRow, ComparisonSession, ContactCard, ContactInfo,
    ContactVerification, DataFormat, DataLicense, DatasetCard, DatasetInfo, DmBubble, DmDirection,
    DmMessage, EncryptionStatus, EntryActions, IssueInfo, IssueLabel, IssueRow, MnemonicDisplay,
    PermissionBar, PermissionDecision, PermissionHistory, PermissionHistoryItem, PermissionRule,
    PermissionRuleRow, PermissionScope, PrEvent, PrEventType, PrTimelineItem, ProviderCard,
    ProviderInfo, ProviderSpecs, ProviderStatus, RelayInfo, RepoCard, RepoInfo, RepoVisibility,
    ReviewState, SessionCard, SessionInfo, SessionSearchBar, SigningRequestCard,
    SigningRequestInfo, SigningType, SigningUrgency, SkillCard, SkillCategory, SkillInfo,
    SkillInstallStatus, TerminalHeader, TransactionDirection, TransactionInfo, TransactionRow,
    ZapCard, ZapInfo,
};
use wgpui::components::organisms::{
    AgentAction, AgentGoal, AgentGoalStatus, AgentStateInspector, ApmLeaderboard, DmThread,
    EventData, EventInspector, IntervalUnit, KeyShare, LeaderboardEntry, PeerStatus, ReceiveFlow,
    ReceiveStep, ReceiveType, RelayManager, ResourceUsage, ScheduleConfig, ScheduleData,
    ScheduleType, SendFlow, SendStep, SigningRequest, TagData, ThresholdKeyManager, ThresholdPeer,
    ZapFlow,
};
use wgpui::components::sections::{
    MessageEditor, ThreadFeedback, ThreadHeader, TrajectoryEntry, TrajectoryView,
};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::helpers::{draw_panel, panel_height, panel_stack};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_thread_components(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let header_height = panel_height(160.0);
        let editor_height = panel_height(180.0);
        let feedback_height = panel_height(200.0);
        let actions_height = panel_height(140.0);
        let terminal_height = panel_height(140.0);
        let layout_height = panel_height(400.0);
        let trajectory_height = panel_height(220.0);

        let panels = panel_stack(
            bounds,
            &[
                header_height,
                editor_height,
                feedback_height,
                actions_height,
                terminal_height,
                layout_height,
                trajectory_height,
            ],
        );

        // ========== Panel 1: Thread Headers ==========
        let header_bounds = panels[0];
        draw_panel("Thread Headers", header_bounds, cx, |inner, cx| {
            let variants = [
                ("Full header", true, true, Some("3 messages")),
                ("No back button", false, true, None),
                ("No menu button", true, false, None),
                ("Minimal", false, false, Some("subtitle only")),
            ];

            let tile_w = 280.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, show_back, show_menu, subtitle)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // ThreadHeader
                let mut header = ThreadHeader::new("Conversation")
                    .show_back_button(*show_back)
                    .show_menu_button(*show_menu);
                if let Some(sub) = subtitle {
                    header = header.subtitle(*sub);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 48.0), cx);
            }
        });

        // ========== Panel 2: Message Editor States ==========
        let editor_bounds = panels[1];
        draw_panel("Message Editor States", editor_bounds, cx, |inner, cx| {
            let states = [
                ("Normal mode", Mode::Normal, false, "Type a message..."),
                ("Plan mode", Mode::Plan, false, "Describe your plan..."),
                ("Streaming", Mode::Normal, true, ""),
            ];

            let tile_w = 320.0;
            let tile_h = 70.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, mode, streaming, placeholder)) in states.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // MessageEditor
                let mut editor = MessageEditor::new().mode(*mode).streaming(*streaming);
                if !placeholder.is_empty() {
                    editor = editor.placeholder(*placeholder);
                }
                editor.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 64.0), cx);
            }
        });

        // ========== Panel 3: Thread Feedback ==========
        let feedback_bounds = panels[2];
        draw_panel("Thread Feedback", feedback_bounds, cx, |inner, cx| {
            let tile_w = 280.0;
            let gap = 16.0;

            // Default state
            let label_run = cx.text.layout(
                "Default (no rating)",
                Point::new(inner.origin.x, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            let mut feedback1 = ThreadFeedback::new();
            feedback1.paint(
                Bounds::new(inner.origin.x, inner.origin.y + 14.0, tile_w, 80.0),
                cx,
            );

            // Second column - with comment shown (simulated by larger height)
            let label_run2 = cx.text.layout(
                "Rating selected",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run2);

            // Show a description of what would happen
            let info = cx.text.layout(
                "Click thumbs up/down to rate",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y + 50.0),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(info);
        });

        // ========== Panel 4: Entry Actions ==========
        let actions_bounds = panels[3];
        draw_panel("Entry Actions", actions_bounds, cx, |inner, cx| {
            let variants = [
                ("Default (copy)", true, false, false, false),
                ("With retry", true, true, false, false),
                ("With edit/delete", true, false, true, true),
                ("All actions", true, true, true, true),
                ("Copy only", true, false, false, false),
            ];

            let tile_w = 200.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, copy, retry, edit, delete)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // EntryActions
                let mut actions = EntryActions::new()
                    .show_copy(*copy)
                    .show_retry(*retry)
                    .show_edit(*edit)
                    .show_delete(*delete);
                actions.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w, 24.0), cx);
            }
        });

        // ========== Panel 5: Terminal Headers ==========
        let terminal_bounds = panels[4];
        draw_panel("Terminal Headers", terminal_bounds, cx, |inner, cx| {
            let variants = [
                ("Pending", "cargo build", ToolStatus::Pending, None),
                ("Running", "npm install", ToolStatus::Running, None),
                ("Success", "cargo test", ToolStatus::Success, Some(0)),
                ("Error", "rm -rf /", ToolStatus::Error, Some(1)),
            ];

            let tile_w = 280.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, cmd, status, exit_code)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // TerminalHeader
                let mut header = TerminalHeader::new(*cmd).status(*status);
                if let Some(code) = exit_code {
                    header = header.exit_code(*code);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 32.0), cx);
            }
        });

        // ========== Panel 6: Complete Thread Layout ==========
        let layout_bounds = panels[5];
        draw_panel("Complete Thread Layout", layout_bounds, cx, |inner, cx| {
            // ThreadHeader at top
            let mut header = ThreadHeader::new("Code Review Session")
                .subtitle("5 messages")
                .show_back_button(true)
                .show_menu_button(true);
            header.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // Thread content area
            let content_y = inner.origin.y + 56.0;
            let content_h = inner.size.height - 56.0 - 72.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    inner.origin.x,
                    content_y,
                    inner.size.width,
                    content_h,
                ))
                .with_background(theme::bg::APP)
                .with_border(theme::border::DEFAULT, 1.0),
            );

            // Sample messages
            let msg1 = cx.text.layout(
                "User: Can you review this code?",
                Point::new(inner.origin.x + 12.0, content_y + 12.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(msg1);

            let msg2 = cx.text.layout(
                "Assistant: I'll analyze the code structure...",
                Point::new(inner.origin.x + 12.0, content_y + 36.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(msg2);

            // Entry actions for a message
            let mut actions = EntryActions::new().show_copy(true).show_retry(true);
            actions.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 60.0, 180.0, 24.0),
                cx,
            );

            // Terminal header in content
            let mut terminal = TerminalHeader::new("cargo clippy")
                .status(ToolStatus::Success)
                .exit_code(0);
            terminal.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 92.0, 300.0, 32.0),
                cx,
            );

            // MessageEditor at bottom
            let editor_y = inner.origin.y + inner.size.height - 64.0;
            let mut editor = MessageEditor::new()
                .mode(Mode::Normal)
                .placeholder("Continue the conversation...");
            editor.paint(
                Bounds::new(inner.origin.x, editor_y, inner.size.width, 64.0),
                cx,
            );
        });

        // ========== Panel 7: Trajectory View ==========
        let trajectory_bounds = panels[6];
        draw_panel("Trajectory View", trajectory_bounds, cx, |inner, cx| {
            let entries = vec![
                TrajectoryEntry::new("Load workspace")
                    .detail("Open repository state")
                    .timestamp("00:12")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Analyze failing tests")
                    .detail("Unit tests: 3 failed")
                    .timestamp("00:32")
                    .status(TrajectoryStatus::Partial),
                TrajectoryEntry::new("Apply fix")
                    .detail("Update parser edge cases")
                    .timestamp("01:05")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Re-run suite")
                    .detail("All green")
                    .timestamp("01:42")
                    .status(TrajectoryStatus::Verified),
            ];

            let mut view = TrajectoryView::new().entries(entries);
            view.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width,
                    inner.size.height,
                ),
                cx,
            );
        });
    }

    pub(crate) fn paint_sessions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let cards_height = panel_height(280.0);
        let breadcrumb_height = panel_height(120.0);
        let search_height = panel_height(180.0);
        let actions_height = panel_height(160.0);
        let list_height = panel_height(320.0);

        let panels = panel_stack(
            bounds,
            &[
                cards_height,
                breadcrumb_height,
                search_height,
                actions_height,
                list_height,
            ],
        );

        // ========== Panel 1: Session Cards ==========
        let cards_bounds = panels[0];
        draw_panel("Session Cards", cards_bounds, cx, |inner, cx| {
            let card_w = (inner.size.width - 24.0) / 3.0;

            // Running session
            let running_info = SessionInfo::new("sess-001", "Implement auth flow")
                .status(SessionStatus::Running)
                .timestamp("10:30 AM")
                .duration(1847)
                .task_count(12)
                .model("sonnet");
            let mut running = SessionCard::new(running_info);
            running.paint(
                Bounds::new(inner.origin.x, inner.origin.y, card_w, 160.0),
                cx,
            );

            // Completed session
            let completed_info = SessionInfo::new("sess-002", "Fix CI pipeline")
                .status(SessionStatus::Completed)
                .timestamp("Yesterday")
                .duration(3621)
                .task_count(8)
                .model("opus");
            let mut completed = SessionCard::new(completed_info);
            completed.paint(
                Bounds::new(
                    inner.origin.x + card_w + 12.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Failed session
            let failed_info = SessionInfo::new("sess-003", "Migrate database")
                .status(SessionStatus::Failed)
                .timestamp("2 days ago")
                .duration(892)
                .task_count(5)
                .model("sonnet");
            let mut failed = SessionCard::new(failed_info);
            failed.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Second row - more states
            let row2_y = inner.origin.y + 172.0;

            let paused_info = SessionInfo::new("sess-004", "Refactor components")
                .status(SessionStatus::Paused)
                .timestamp("1 hour ago")
                .duration(1200)
                .task_count(15)
                .model("sonnet");
            let mut paused = SessionCard::new(paused_info);
            paused.paint(Bounds::new(inner.origin.x, row2_y, card_w, 160.0), cx);

            let aborted_info = SessionInfo::new("sess-005", "Update dependencies")
                .status(SessionStatus::Aborted)
                .timestamp("3 hours ago")
                .duration(456)
                .task_count(3)
                .model("haiku");
            let mut aborted = SessionCard::new(aborted_info);
            aborted.paint(
                Bounds::new(inner.origin.x + card_w + 12.0, row2_y, card_w, 160.0),
                cx,
            );

            let pending_info = SessionInfo::new("sess-006", "Write tests")
                .status(SessionStatus::Pending)
                .timestamp("Queued")
                .model("sonnet");
            let mut pending = SessionCard::new(pending_info);
            pending.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    row2_y,
                    card_w,
                    160.0,
                ),
                cx,
            );
        });

        // ========== Panel 2: Session Breadcrumbs ==========
        let breadcrumb_bounds = panels[1];
        draw_panel("Session Breadcrumbs", breadcrumb_bounds, cx, |inner, cx| {
            // Simple breadcrumb
            let mut bc1 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("sess-001", "Main Session"),
                BreadcrumbItem::new("sess-002", "Fork: Auth").current(true),
            ]);
            bc1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 28.0),
                cx,
            );

            // Deep breadcrumb
            let mut bc2 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("root", "Root Session"),
                BreadcrumbItem::new("fork-1", "API Changes"),
                BreadcrumbItem::new("fork-2", "Error Handling"),
                BreadcrumbItem::new("current", "Final Polish").current(true),
            ]);
            bc2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 36.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Single item
            let mut bc3 = SessionBreadcrumb::new()
                .push_item(BreadcrumbItem::new("standalone", "Standalone Session").current(true));
            bc3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 72.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );
        });

        // ========== Panel 3: Session Search ==========
        let search_bounds = panels[2];
        draw_panel(
            "Session Search & Filters",
            search_bounds,
            cx,
            |inner, cx| {
                // Empty search bar
                let mut search1 = SessionSearchBar::new();
                search1.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                    cx,
                );

                // Search bar with placeholder
                let mut search2 = SessionSearchBar::new().placeholder("Search auth sessions...");
                search2.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 52.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );

                // With active filter
                let mut search3 = SessionSearchBar::new();
                search3.set_filter(SessionStatus::Running, true);
                search3.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 104.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 4: Session Actions ==========
        let actions_bounds = panels[3];
        draw_panel("Session Actions", actions_bounds, cx, |inner, cx| {
            let label_x = inner.origin.x;
            let badge_x = inner.origin.x + 200.0;
            let mut row_y = inner.origin.y;

            // Resumable session (paused)
            let paused_label = cx.text.layout(
                "Paused → Can Resume:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(paused_label);
            let mut paused_badge = SessionStatusBadge::new(SessionStatus::Paused);
            paused_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Forkable sessions
            let completed_label = cx.text.layout(
                "Completed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(completed_label);
            let mut completed_badge = SessionStatusBadge::new(SessionStatus::Completed);
            completed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            let failed_label = cx.text.layout(
                "Failed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(failed_label);
            let mut failed_badge = SessionStatusBadge::new(SessionStatus::Failed);
            failed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Active session
            let running_label = cx.text.layout(
                "Running → Active:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(running_label);
            let mut running_badge = SessionStatusBadge::new(SessionStatus::Running)
                .duration(3621)
                .task_count(8);
            running_badge.paint(Bounds::new(badge_x, row_y, 200.0, 24.0), cx);
        });

        // ========== Panel 5: Complete Session List ==========
        let list_bounds = panels[4];
        draw_panel("Complete Session List", list_bounds, cx, |inner, cx| {
            // Search bar at top
            let mut search = SessionSearchBar::new();
            search.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                cx,
            );

            // Breadcrumb showing current path
            let mut breadcrumb = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("all", "All Sessions"),
                BreadcrumbItem::new("today", "Today").current(true),
            ]);
            breadcrumb.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 52.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Session cards in a grid
            let cards_y = inner.origin.y + 80.0;
            let card_w = (inner.size.width - 12.0) / 2.0;

            let sessions = [
                ("Current Task", SessionStatus::Running, 1847u64, 12u32),
                ("Yesterday's Work", SessionStatus::Completed, 7200, 15),
                ("Blocked Task", SessionStatus::Paused, 2400, 10),
                ("Failed Migration", SessionStatus::Failed, 600, 8),
            ];

            for (i, (title, status, dur, total)) in sessions.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let x = inner.origin.x + col as f32 * (card_w + 12.0);
                let y = cards_y + row as f32 * 112.0;

                let info = SessionInfo::new(format!("sess-{}", i), *title)
                    .status(*status)
                    .timestamp("Today")
                    .duration(*dur)
                    .task_count(*total)
                    .model("sonnet");
                let mut card = SessionCard::new(info);
                card.paint(Bounds::new(x, y, card_w, 100.0), cx);
            }
        });
    }

    pub(crate) fn paint_permissions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let decisions_height = panel_height(160.0);
        let rules_height = panel_height(240.0);
        let history_height = panel_height(280.0);
        let bar_height = panel_height(200.0);
        let stats_height = panel_height(140.0);

        let panels = panel_stack(
            bounds,
            &[
                decisions_height,
                rules_height,
                history_height,
                bar_height,
                stats_height,
            ],
        );

        // ========== Panel 1: Permission Decisions ==========
        let decisions_bounds = panels[0];
        draw_panel("Permission Decisions", decisions_bounds, cx, |inner, cx| {
            let decisions = [
                (PermissionDecision::Ask, "Ask every time"),
                (PermissionDecision::AllowOnce, "Allow once"),
                (PermissionDecision::AllowAlways, "Allow always"),
                (PermissionDecision::Deny, "Deny"),
            ];

            for (i, (decision, desc)) in decisions.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * 140.0);

                // Decision badge
                let color = decision.color();
                let badge_bounds = Bounds::new(x, inner.origin.y, 120.0, 28.0);
                cx.scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(color.with_alpha(0.2))
                        .with_border(color, 1.0),
                );
                let label = cx.text.layout(
                    decision.label(),
                    Point::new(x + 8.0, inner.origin.y + 6.0),
                    theme::font_size::SM,
                    color,
                );
                cx.scene.draw_text(label);

                // Description
                let desc_text = cx.text.layout(
                    desc,
                    Point::new(x, inner.origin.y + 40.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_text);
            }

            // Short labels row
            let short_y = inner.origin.y + 72.0;
            let short_label = cx.text.layout(
                "Short labels:",
                Point::new(inner.origin.x, short_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(short_label);

            for (i, (decision, _)) in decisions.iter().enumerate() {
                let x = inner.origin.x + 80.0 + (i as f32 * 60.0);
                let short = cx.text.layout(
                    decision.short_label(),
                    Point::new(x, short_y),
                    theme::font_size::SM,
                    decision.color(),
                );
                cx.scene.draw_text(short);
            }
        });

        // ========== Panel 2: Permission Rules ==========
        let rules_bounds = panels[1];
        draw_panel("Permission Rules", rules_bounds, cx, |inner, cx| {
            let rules = [
                PermissionRule::new("rule-1", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Session)
                    .pattern("cargo *")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-2", ToolType::Write, "Write")
                    .scope(PermissionScope::Project)
                    .pattern("src/*")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-3", ToolType::Read, "Read")
                    .scope(PermissionScope::Global)
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-4", ToolType::Edit, "Edit")
                    .scope(PermissionScope::Session)
                    .decision(PermissionDecision::Ask),
                PermissionRule::new("rule-5", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Global)
                    .pattern("sudo *")
                    .decision(PermissionDecision::Deny),
            ];

            for (i, rule) in rules.iter().enumerate() {
                let mut row = PermissionRuleRow::new(rule.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 44.0,
                        inner.size.width,
                        40.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Permission History ==========
        let history_bounds = panels[2];
        draw_panel("Permission History", history_bounds, cx, |inner, cx| {
            let histories = [
                PermissionHistory::new("h-1", ToolType::Bash, "Bash", "cargo build --release")
                    .decision(PermissionDecision::AllowOnce)
                    .timestamp("2 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-2", ToolType::Write, "Write", "src/lib.rs")
                    .decision(PermissionDecision::AllowAlways)
                    .timestamp("5 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-3", ToolType::Bash, "Bash", "rm -rf node_modules/")
                    .decision(PermissionDecision::Deny)
                    .timestamp("10 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-4", ToolType::Read, "Read", "/etc/passwd")
                    .decision(PermissionDecision::Deny)
                    .timestamp("15 min ago")
                    .session("sess-002"),
            ];

            for (i, history) in histories.iter().enumerate() {
                let mut item = PermissionHistoryItem::new(history.clone());
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 64.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Permission Bar Variants ==========
        let bar_bounds = panels[3];
        draw_panel("Permission Bar Variants", bar_bounds, cx, |inner, cx| {
            // Standard permission bar
            let mut bar1 = PermissionBar::new("Bash wants to execute: cargo test");
            bar1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // File write permission
            let mut bar2 = PermissionBar::new("Write wants to create: src/new_module.rs");
            bar2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 56.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );

            // Dangerous operation
            let mut bar3 = PermissionBar::new("Bash wants to execute: git push --force");
            bar3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 112.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: Permission Statistics ==========
        let stats_bounds = panels[4];
        draw_panel("Permission Statistics", stats_bounds, cx, |inner, cx| {
            let stats = [
                ("Total Requests", "247", theme::text::PRIMARY),
                ("Allowed", "189", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                ("Denied", "42", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                ("Pending", "16", Hsla::new(45.0, 0.7, 0.5, 1.0)),
            ];

            let stat_w = inner.size.width / 4.0;
            for (i, (label, value, color)) in stats.iter().enumerate() {
                let x = inner.origin.x + i as f32 * stat_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    *color,
                );
                cx.scene.draw_text(value_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Rule counts
            let rule_y = inner.origin.y + 80.0;
            let rule_label = cx.text.layout(
                "Active Rules:",
                Point::new(inner.origin.x + 12.0, rule_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(rule_label);

            let rule_counts = [("Global", 5), ("Project", 12), ("Session", 8)];

            let mut rx = inner.origin.x + 120.0;
            for (scope, count) in rule_counts {
                let scope_text = format!("{}: {}", scope, count);
                let scope_run = cx.text.layout(
                    &scope_text,
                    Point::new(rx, rule_y),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(scope_run);
                rx += 100.0;
            }
        });
    }

    pub(crate) fn paint_apm_metrics(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let gauge_height = panel_height(200.0);
        let rows_height = panel_height(220.0);
        let comparison_height = panel_height(280.0);
        let leaderboard_height = panel_height(320.0);
        let trends_height = panel_height(200.0);

        let panels = panel_stack(
            bounds,
            &[
                gauge_height,
                rows_height,
                comparison_height,
                leaderboard_height,
                trends_height,
            ],
        );

        // ========== Panel 1: APM Gauge Variations ==========
        let gauge_bounds = panels[0];
        draw_panel("APM Gauge Variations", gauge_bounds, cx, |inner, cx| {
            let apms = [
                (0.0, "Idle"),
                (25.0, "Slow"),
                (50.0, "Moderate"),
                (75.0, "Fast"),
                (95.0, "Expert"),
                (120.0, "Elite"),
            ];

            let gauge_w = 100.0;
            let gauge_h = 60.0;
            let gap = 20.0;

            for (i, (apm, label)) in apms.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * (gauge_w + gap));

                // APM Gauge
                let mut gauge = ApmGauge::new(*apm);
                gauge.paint(Bounds::new(x, inner.origin.y, gauge_w, gauge_h), cx);

                // Label
                let label_text = cx.text.layout(
                    *label,
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 8.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);

                // APM Value
                let level = ApmLevel::from_apm(*apm);
                let apm_text = cx.text.layout(
                    &format!("{:.0} APM", apm),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 24.0),
                    theme::font_size::SM,
                    level.color(),
                );
                cx.scene.draw_text(apm_text);

                // Tier label
                let tier_text = cx.text.layout(
                    level.label(),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 42.0),
                    theme::font_size::XS,
                    level.color(),
                );
                cx.scene.draw_text(tier_text);
            }
        });

        // ========== Panel 2: APM Session Rows ==========
        let rows_bounds = panels[1];
        draw_panel("APM Session Rows", rows_bounds, cx, |inner, cx| {
            let sessions = [
                ApmSessionData::new("sess-1", "Build feature authentication", 92.0)
                    .status(SessionStatus::Completed)
                    .duration(1800)
                    .rank(1),
                ApmSessionData::new("sess-2", "Fix database query bug", 78.5)
                    .status(SessionStatus::Completed)
                    .duration(2400)
                    .rank(2),
                ApmSessionData::new("sess-3", "Refactor API endpoints", 65.0)
                    .status(SessionStatus::Running)
                    .duration(900)
                    .rank(3),
                ApmSessionData::new("sess-4", "Add unit tests", 45.2)
                    .status(SessionStatus::Paused)
                    .duration(600)
                    .rank(4),
            ];

            for (i, session) in sessions.iter().enumerate() {
                let mut row = ApmSessionRow::new(session.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 48.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Session Comparison ==========
        let comparison_bounds = panels[2];
        draw_panel("Session Comparison", comparison_bounds, cx, |inner, cx| {
            let session_a = ComparisonSession::new("sess-a", "Monday Session", 68.5)
                .messages(120)
                .tool_calls(85)
                .duration(3600);

            let session_b = ComparisonSession::new("sess-b", "Tuesday Session", 82.3)
                .messages(95)
                .tool_calls(110)
                .duration(2800);

            let mut card = ApmComparisonCard::new(session_a, session_b);
            card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    220.0,
                ),
                cx,
            );
        });

        // ========== Panel 4: APM Leaderboard ==========
        let leaderboard_bounds = panels[3];
        draw_panel("APM Leaderboard", leaderboard_bounds, cx, |inner, cx| {
            let entries = vec![
                LeaderboardEntry::new("1", "Implement OAuth2 flow", 98.5)
                    .status(SessionStatus::Completed)
                    .messages(150)
                    .tool_calls(120),
                LeaderboardEntry::new("2", "Build payment integration", 92.0)
                    .status(SessionStatus::Completed)
                    .messages(180)
                    .tool_calls(95),
                LeaderboardEntry::new("3", "Create dashboard UI", 85.5)
                    .status(SessionStatus::Completed)
                    .messages(200)
                    .tool_calls(75),
                LeaderboardEntry::new("4", "Add real-time sync", 78.0)
                    .status(SessionStatus::Completed)
                    .messages(90)
                    .tool_calls(60),
                LeaderboardEntry::new("5", "Fix memory leak", 65.0)
                    .status(SessionStatus::Completed)
                    .messages(50)
                    .tool_calls(35),
                LeaderboardEntry::new("6", "Write documentation", 45.0)
                    .status(SessionStatus::Completed)
                    .messages(80)
                    .tool_calls(10),
            ];

            let mut leaderboard = ApmLeaderboard::new()
                .title("Top Sessions This Week")
                .entries(entries);
            leaderboard.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(600.0),
                    260.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: APM Trends Summary ==========
        let trends_bounds = panels[4];
        draw_panel("APM Trends Summary", trends_bounds, cx, |inner, cx| {
            let metrics = [
                ("Avg APM", "72.4", "+5.2%", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                (
                    "Peak APM",
                    "98.5",
                    "+12.1%",
                    Hsla::new(120.0, 0.7, 0.45, 1.0),
                ),
                ("Sessions", "24", "+3", Hsla::new(200.0, 0.7, 0.5, 1.0)),
                (
                    "Tool Calls",
                    "1,847",
                    "-2.3%",
                    Hsla::new(0.0, 0.7, 0.5, 1.0),
                ),
            ];

            let metric_w = inner.size.width / 4.0;
            for (i, (label, value, change, change_color)) in metrics.iter().enumerate() {
                let x = inner.origin.x + i as f32 * metric_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(value_text);

                // Change indicator
                let change_text = cx.text.layout(
                    change,
                    Point::new(x + 12.0, inner.origin.y + 40.0),
                    theme::font_size::SM,
                    *change_color,
                );
                cx.scene.draw_text(change_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Period selector hint
            let period_y = inner.origin.y + 100.0;
            let periods = ["1h", "24h", "7d", "30d"];
            let period_label = cx.text.layout(
                "Time Period:",
                Point::new(inner.origin.x + 12.0, period_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(period_label);

            for (i, period) in periods.iter().enumerate() {
                let x = inner.origin.x + 100.0 + i as f32 * 60.0;
                let is_selected = i == 2; // 7d selected

                let bg = if is_selected {
                    theme::accent::PRIMARY.with_alpha(0.3)
                } else {
                    theme::bg::HOVER
                };

                let btn_bounds = Bounds::new(x, period_y - 4.0, 48.0, 24.0);
                cx.scene
                    .draw_quad(Quad::new(btn_bounds).with_background(bg).with_border(
                        if is_selected {
                            theme::accent::PRIMARY
                        } else {
                            theme::border::DEFAULT
                        },
                        1.0,
                    ));

                let period_text = cx.text.layout(
                    period,
                    Point::new(x + 14.0, period_y),
                    theme::font_size::SM,
                    if is_selected {
                        theme::accent::PRIMARY
                    } else {
                        theme::text::MUTED
                    },
                );
                cx.scene.draw_text(period_text);
            }
        });
    }

    pub(crate) fn paint_wallet_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mnemonic_height = panel_height(260.0);
        let address_height = panel_height(180.0);
        let tx_height = panel_height(280.0);
        let send_height = panel_height(360.0);
        let receive_height = panel_height(420.0);

        let panels = panel_stack(
            bounds,
            &[
                mnemonic_height,
                address_height,
                tx_height,
                send_height,
                receive_height,
            ],
        );

        // ========== Panel 1: Mnemonic Display ==========
        let mnemonic_bounds = panels[0];
        draw_panel("Mnemonic Display", mnemonic_bounds, cx, |inner, cx| {
            // Sample 12-word mnemonic
            let words = vec![
                "abandon".to_string(),
                "ability".to_string(),
                "able".to_string(),
                "about".to_string(),
                "above".to_string(),
                "absent".to_string(),
                "absorb".to_string(),
                "abstract".to_string(),
                "absurd".to_string(),
                "abuse".to_string(),
                "access".to_string(),
                "accident".to_string(),
            ];

            let mut mnemonic = MnemonicDisplay::new(words).revealed(true);
            mnemonic.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    200.0,
                ),
                cx,
            );
        });

        // ========== Panel 2: Address Cards ==========
        let address_bounds = panels[1];
        draw_panel("Address Cards", address_bounds, cx, |inner, cx| {
            // Bitcoin address
            let mut btc_card = AddressCard::new(
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                AddressType::Bitcoin,
            )
            .label("Primary Wallet");
            btc_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );

            // Lightning address
            let mut ln_card =
                AddressCard::new("lnbc1500n1pj9nr6mpp5argz38...", AddressType::Lightning)
                    .label("Lightning Invoice");
            ln_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 80.0,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );
        });

        // ========== Panel 3: Transaction History ==========
        let tx_bounds = panels[2];
        draw_panel("Transaction History", tx_bounds, cx, |inner, cx| {
            let transactions = [
                TransactionInfo::new("tx-1", 150000, TransactionDirection::Incoming)
                    .timestamp("2 hours ago")
                    .description("Payment from Alice"),
                TransactionInfo::new("tx-2", 50000, TransactionDirection::Outgoing)
                    .timestamp("Yesterday")
                    .description("Coffee shop")
                    .fee(500),
                TransactionInfo::new("tx-3", 1000000, TransactionDirection::Incoming)
                    .timestamp("3 days ago")
                    .description("Freelance payment"),
                TransactionInfo::new("tx-4", 25000, TransactionDirection::Outgoing)
                    .timestamp("1 week ago")
                    .description("Subscription")
                    .fee(250),
            ];

            for (i, tx) in transactions.iter().enumerate() {
                let mut row = TransactionRow::new(tx.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Send Flow ==========
        let send_bounds = panels[3];
        draw_panel("Send Flow Wizard", send_bounds, cx, |inner, cx| {
            let mut send_flow = SendFlow::new()
                .step(SendStep::Review)
                .address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq")
                .amount(50000)
                .fee(500);
            send_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    320.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: Receive Flow ==========
        let receive_bounds = panels[4];
        draw_panel("Receive Flow Wizard", receive_bounds, cx, |inner, cx| {
            let mut receive_flow = ReceiveFlow::new()
                .step(ReceiveStep::ShowInvoice)
                .receive_type(ReceiveType::Lightning)
                .amount(25000)
                .invoice("lnbc250u1pjxxx...")
                .expires_in(3600);
            receive_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    380.0,
                ),
                cx,
            );
        });
    }

    pub(crate) fn paint_gitafter_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let repo_height = panel_height(240.0);
        let issue_height = panel_height(320.0);
        let pr_height = panel_height(280.0);
        let labels_height = panel_height(200.0);

        let panels = panel_stack(
            bounds,
            &[repo_height, issue_height, pr_height, labels_height],
        );

        // ========== Panel 1: Repository Cards ==========
        let repo_bounds = panels[0];
        draw_panel("Repository Cards", repo_bounds, cx, |inner, cx| {
            let repos = [
                RepoInfo::new("repo-1", "openagents")
                    .description("An open source AI agent framework for autonomous workflows")
                    .visibility(RepoVisibility::Public)
                    .stars(1250)
                    .forks(180)
                    .issues(42)
                    .language("Rust")
                    .updated_at("2 hours ago"),
                RepoInfo::new("repo-2", "wgpui")
                    .description("GPU-accelerated native UI framework")
                    .visibility(RepoVisibility::Public)
                    .stars(340)
                    .forks(28)
                    .issues(15)
                    .language("Rust")
                    .updated_at("Yesterday"),
            ];

            for (i, repo) in repos.iter().enumerate() {
                let mut card = RepoCard::new(repo.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 110.0,
                        inner.size.width.min(500.0),
                        100.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: Issue List ==========
        let issue_bounds = panels[1];
        draw_panel("Issue List with Bounties", issue_bounds, cx, |inner, cx| {
            let issues = [
                IssueInfo::new("issue-1", 42, "Memory leak in event processing loop")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::bug())
                    .label(IssueLabel::help_wanted())
                    .author("alice")
                    .bounty(50000)
                    .comments(12)
                    .created_at("3 days ago"),
                IssueInfo::new("issue-2", 43, "Add dark mode toggle to settings")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::enhancement())
                    .label(IssueLabel::good_first_issue())
                    .author("bob")
                    .bounty(25000)
                    .comments(5)
                    .created_at("1 week ago"),
                IssueInfo::new("issue-3", 44, "Update documentation for v2.0 release")
                    .status(IssueStatus::Closed)
                    .label(IssueLabel::new("docs", Hsla::new(190.0, 0.6, 0.5, 1.0)))
                    .author("charlie")
                    .comments(8)
                    .created_at("2 weeks ago"),
            ];

            for (i, issue) in issues.iter().enumerate() {
                let mut row = IssueRow::new(issue.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 90.0,
                        inner.size.width,
                        80.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: PR Timeline ==========
        let pr_bounds = panels[2];
        draw_panel("PR Timeline", pr_bounds, cx, |inner, cx| {
            let events = [
                PrEvent::new("ev-1", PrEventType::Commit, "alice")
                    .message("Initial implementation of feature X")
                    .commit_sha("abc1234def")
                    .timestamp("3 hours ago"),
                PrEvent::new("ev-2", PrEventType::Review, "bob")
                    .review_state(ReviewState::Approved)
                    .timestamp("2 hours ago"),
                PrEvent::new("ev-3", PrEventType::Comment, "charlie")
                    .message("Looks good! Just one minor suggestion...")
                    .timestamp("1 hour ago"),
                PrEvent::new("ev-4", PrEventType::Merge, "alice")
                    .message("Merged into main")
                    .timestamp("30 minutes ago"),
            ];

            for (i, event) in events.iter().enumerate() {
                let is_last = i == events.len() - 1;
                let mut item = PrTimelineItem::new(event.clone()).is_last(is_last);
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width.min(500.0),
                        60.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Issue Labels & Status Variants ==========
        let labels_bounds = panels[3];
        draw_panel(
            "Issue Labels & PR Events",
            labels_bounds,
            cx,
            |inner, cx| {
                // Draw predefined labels
                let labels = [
                    IssueLabel::bug(),
                    IssueLabel::enhancement(),
                    IssueLabel::good_first_issue(),
                    IssueLabel::help_wanted(),
                    IssueLabel::new("security", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                    IssueLabel::new("performance", Hsla::new(280.0, 0.6, 0.5, 1.0)),
                ];

                let mut label_x = inner.origin.x;
                for label in &labels {
                    let label_w = (label.name.len() as f32 * 7.0) + 16.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(label_x, inner.origin.y, label_w, 20.0))
                            .with_background(label.color.with_alpha(0.2))
                            .with_border(label.color, 1.0),
                    );
                    let text = cx.text.layout(
                        &label.name,
                        Point::new(label_x + 6.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        label.color,
                    );
                    cx.scene.draw_text(text);
                    label_x += label_w + 8.0;
                }

                // Draw PR event types
                let mut event_y = inner.origin.y + 40.0;
                let events = [
                    PrEventType::Commit,
                    PrEventType::Review,
                    PrEventType::Comment,
                    PrEventType::StatusChange,
                    PrEventType::Merge,
                    PrEventType::Close,
                    PrEventType::Reopen,
                ];

                let mut event_x = inner.origin.x;
                for event in &events {
                    let icon_text = format!("{} {}", event.icon(), event.label());
                    let text = cx.text.layout(
                        &icon_text,
                        Point::new(event_x, event_y),
                        theme::font_size::SM,
                        event.color(),
                    );
                    cx.scene.draw_text(text);
                    event_x += 120.0;
                    if event_x > inner.origin.x + inner.size.width - 120.0 {
                        event_x = inner.origin.x;
                        event_y += 24.0;
                    }
                }

                // Draw review states
                let review_y = event_y + 40.0;
                let states = [
                    ReviewState::Approved,
                    ReviewState::RequestChanges,
                    ReviewState::Commented,
                    ReviewState::Pending,
                ];

                let mut state_x = inner.origin.x;
                for state in &states {
                    let state_w = 120.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(state_x, review_y, state_w, 24.0))
                            .with_background(state.color().with_alpha(0.2))
                            .with_border(state.color(), 1.0),
                    );
                    let text = cx.text.layout(
                        state.label(),
                        Point::new(state_x + 8.0, review_y + 5.0),
                        theme::font_size::XS,
                        state.color(),
                    );
                    cx.scene.draw_text(text);
                    state_x += state_w + 12.0;
                }
            },
        );
    }

    pub(crate) fn paint_marketplace_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let provider_height = panel_height(260.0);
        let skills_height = panel_height(280.0);
        let data_height = panel_height(280.0);
        let ref_height = panel_height(180.0);

        let panels = panel_stack(
            bounds,
            &[provider_height, skills_height, data_height, ref_height],
        );

        // ========== Panel 1: Compute Providers ==========
        let provider_bounds = panels[0];
        draw_panel("Compute Providers", provider_bounds, cx, |inner, cx| {
            let providers = [
                ProviderInfo::new(
                    "p1",
                    "FastCompute Pro",
                    ProviderSpecs::new(32, 128, 2000).gpu("NVIDIA A100"),
                )
                .status(ProviderStatus::Online)
                .price(15000)
                .rating(4.9)
                .jobs(1250)
                .location("US-East"),
                ProviderInfo::new("p2", "Budget Runner", ProviderSpecs::new(8, 32, 500))
                    .status(ProviderStatus::Busy)
                    .price(2000)
                    .rating(4.5)
                    .jobs(340)
                    .location("EU-West"),
            ];

            for (i, provider) in providers.iter().enumerate() {
                let mut card = ProviderCard::new(provider.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: Skills Marketplace ==========
        let skills_bounds = panels[1];
        draw_panel("Skills Marketplace", skills_bounds, cx, |inner, cx| {
            let skills = [
                SkillInfo::new(
                    "s1",
                    "Code Review Pro",
                    "AI-powered code review with security analysis",
                )
                .category(SkillCategory::CodeGeneration)
                .author("openagents")
                .version("2.1.0")
                .status(SkillInstallStatus::Installed)
                .downloads(45000)
                .rating(4.8),
                SkillInfo::new(
                    "s2",
                    "Data Transformer",
                    "Transform and clean datasets automatically",
                )
                .category(SkillCategory::DataAnalysis)
                .author("datacraft")
                .version("1.5.2")
                .status(SkillInstallStatus::Available)
                .price(5000)
                .downloads(12000)
                .rating(4.6),
            ];

            for (i, skill) in skills.iter().enumerate() {
                let mut card = SkillCard::new(skill.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 120.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Data Marketplace ==========
        let data_bounds = panels[2];
        draw_panel("Data Marketplace", data_bounds, cx, |inner, cx| {
            let datasets = [
                DatasetInfo::new(
                    "d1",
                    "LLM Training Corpus",
                    "High-quality text corpus for language model training",
                )
                .format(DataFormat::Parquet)
                .license(DataLicense::OpenSource)
                .size(10_737_418_240) // 10 GB
                .rows(50_000_000)
                .author("opendata")
                .downloads(2500)
                .updated_at("2 days ago"),
                DatasetInfo::new(
                    "d2",
                    "Code Embeddings",
                    "Pre-computed embeddings for 100+ programming languages",
                )
                .format(DataFormat::Arrow)
                .license(DataLicense::Commercial)
                .size(5_368_709_120) // 5 GB
                .rows(25_000_000)
                .author("codebase")
                .price(25000)
                .downloads(850)
                .updated_at("1 week ago"),
            ];

            for (i, dataset) in datasets.iter().enumerate() {
                let mut card = DatasetCard::new(dataset.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(550.0),
                        105.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Categories & Formats Reference ==========
        let ref_bounds = panels[3];
        draw_panel("Categories & Formats", ref_bounds, cx, |inner, cx| {
            // Skill categories
            let mut cat_x = inner.origin.x;
            let categories = [
                SkillCategory::CodeGeneration,
                SkillCategory::DataAnalysis,
                SkillCategory::WebAutomation,
                SkillCategory::FileProcessing,
                SkillCategory::ApiIntegration,
            ];

            for cat in &categories {
                let cat_w = (cat.label().len() as f32 * 6.0) + 12.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cat_x, inner.origin.y, cat_w, 18.0))
                        .with_background(cat.color().with_alpha(0.2))
                        .with_border(cat.color(), 1.0),
                );
                let text = cx.text.layout(
                    cat.label(),
                    Point::new(cat_x + 4.0, inner.origin.y + 3.0),
                    theme::font_size::XS,
                    cat.color(),
                );
                cx.scene.draw_text(text);
                cat_x += cat_w + 8.0;
            }

            // Data formats
            let mut fmt_x = inner.origin.x;
            let formats = [
                DataFormat::Json,
                DataFormat::Csv,
                DataFormat::Parquet,
                DataFormat::Arrow,
                DataFormat::Sqlite,
            ];

            for fmt in &formats {
                let fmt_w = 60.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(fmt_x, inner.origin.y + 30.0, fmt_w, 18.0))
                        .with_background(fmt.color().with_alpha(0.2))
                        .with_border(fmt.color(), 1.0),
                );
                let text = cx.text.layout(
                    fmt.label(),
                    Point::new(fmt_x + 6.0, inner.origin.y + 33.0),
                    theme::font_size::XS,
                    fmt.color(),
                );
                cx.scene.draw_text(text);
                fmt_x += fmt_w + 8.0;
            }

            // Provider statuses
            let mut status_x = inner.origin.x;
            let statuses = [
                ProviderStatus::Online,
                ProviderStatus::Busy,
                ProviderStatus::Offline,
                ProviderStatus::Maintenance,
            ];

            for status in &statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y + 60.0, status_w, 18.0))
                        .with_background(status.color().with_alpha(0.2))
                        .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 6.0, inner.origin.y + 63.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                status_x += status_w + 8.0;
            }

            // Install statuses
            let mut install_x = inner.origin.x;
            let install_statuses = [
                SkillInstallStatus::Available,
                SkillInstallStatus::Installed,
                SkillInstallStatus::UpdateAvailable,
                SkillInstallStatus::Installing,
            ];

            for status in &install_statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        install_x,
                        inner.origin.y + 90.0,
                        status_w,
                        18.0,
                    ))
                    .with_background(status.color().with_alpha(0.2))
                    .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(install_x + 6.0, inner.origin.y + 93.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                install_x += status_w + 8.0;
            }
        });
    }

    pub(crate) fn paint_nostr_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let contacts_height = panel_height(320.0);
        let dm_height = panel_height(380.0);
        let zaps_height = panel_height(280.0);
        let relay_mgr_height = panel_height(420.0);
        let dm_thread_height = panel_height(450.0);
        let zap_flow_height = panel_height(420.0);
        let event_inspector_height = panel_height(400.0);
        let ref_height = panel_height(180.0);

        let panels = panel_stack(
            bounds,
            &[
                contacts_height,
                dm_height,
                zaps_height,
                relay_mgr_height,
                dm_thread_height,
                zap_flow_height,
                event_inspector_height,
                ref_height,
            ],
        );

        // ========== Panel 1: Contact Cards ==========
        let contacts_bounds = panels[0];
        draw_panel("Contact Management", contacts_bounds, cx, |inner, cx| {
            let contacts = [
                ContactInfo::new("npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq")
                    .display_name("Alice Developer")
                    .nip05("alice@openagents.com")
                    .about("Building the future of decentralized AI")
                    .verification(ContactVerification::Verified)
                    .following(true)
                    .mutual(true),
                ContactInfo::new("npub1zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
                    .display_name("Bob Builder")
                    .nip05("bob@nostr.dev")
                    .about("Open source contributor")
                    .verification(ContactVerification::WebOfTrust)
                    .following(true)
                    .mutual(false),
                ContactInfo::new("npub1yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy")
                    .display_name("Anonymous")
                    .verification(ContactVerification::Unknown)
                    .following(false)
                    .mutual(false),
            ];

            for (i, contact) in contacts.iter().enumerate() {
                let mut card = ContactCard::new(contact.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 95.0,
                        inner.size.width.min(500.0),
                        90.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: DM Conversations ==========
        let dm_bounds = panels[1];
        draw_panel("Direct Messages", dm_bounds, cx, |inner, cx| {
            let messages = [
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates. I'll review it tonight.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
                DmMessage::new(
                    "m4",
                    "[Encrypted message - decryption failed]",
                    DmDirection::Incoming,
                )
                .sender("Unknown")
                .timestamp("5 min ago")
                .encryption(EncryptionStatus::Failed)
                .read(false),
            ];

            for (i, msg) in messages.iter().enumerate() {
                let mut bubble = DmBubble::new(msg.clone());
                bubble.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 80.0,
                        inner.size.width.min(500.0),
                        75.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Zaps & Lightning ==========
        let zaps_bounds = panels[2];
        draw_panel("Zaps & Lightning", zaps_bounds, cx, |inner, cx| {
            let zaps = [
                ZapInfo::new("z1", 21000, "npub1alice...")
                    .sender_name("Alice")
                    .message("Great thread!")
                    .timestamp("5 min ago"),
                ZapInfo::new("z2", 1000000, "npub1bob...")
                    .sender_name("Bob")
                    .message("Thanks for the amazing tutorial!")
                    .timestamp("1 hour ago"),
                ZapInfo::new("z3", 500, "npub1anon...").timestamp("2 hours ago"),
            ];

            for (i, zap) in zaps.iter().enumerate() {
                let mut card = ZapCard::new(zap.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 85.0,
                        inner.size.width.min(450.0),
                        80.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Relay Manager Organism ==========
        let relay_mgr_bounds = panels[3];
        draw_panel(
            "Relay Manager (Organism)",
            relay_mgr_bounds,
            cx,
            |inner, cx| {
                let relays = vec![
                    RelayInfo::new("wss://relay.damus.io").status(RelayStatus::Connected),
                    RelayInfo::new("wss://nos.lol").status(RelayStatus::Connecting),
                    RelayInfo::new("wss://relay.nostr.band").status(RelayStatus::Connected),
                    RelayInfo::new("wss://relay.snort.social").status(RelayStatus::Disconnected),
                ];
                let mut manager = RelayManager::new(relays);
                manager.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 5: DM Thread Organism ==========
        let dm_thread_bounds = panels[4];
        draw_panel("DM Thread (Organism)", dm_thread_bounds, cx, |inner, cx| {
            let messages = vec![
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
            ];
            let mut thread =
                DmThread::new("Alice Developer", "npub1abc123xyz789").messages(messages);
            thread.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    400.0,
                ),
                cx,
            );
        });

        // ========== Panel 6: Zap Flow Organism ==========
        let zap_flow_bounds = panels[5];
        draw_panel(
            "Zap Flow Wizard (Organism)",
            zap_flow_bounds,
            cx,
            |inner, cx| {
                let mut flow = ZapFlow::new("Alice Developer", "npub1abc123xyz789...");
                flow.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(400.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 7: Event Inspector Organism ==========
        let event_inspector_bounds = panels[6];
        draw_panel(
            "Event Inspector (Organism)",
            event_inspector_bounds,
            cx,
            |inner, cx| {
                let event_data = EventData::new(
                    "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd",
                    "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                    1,
                )
                .content("GM! Building the future of decentralized AI. #OpenAgents #Nostr")
                .created_at(1700000000)
                .tags(vec![
                    TagData::new("t", vec!["OpenAgents".to_string()]),
                    TagData::new("t", vec!["Nostr".to_string()]),
                    TagData::new("p", vec!["npub1alice...".to_string()]),
                ])
                .sig("abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234")
                .verified(true);

                let mut inspector = EventInspector::new(event_data);
                inspector.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(450.0),
                        350.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 8: Status Reference ==========
        let nostr_ref_bounds = panels[7];
        draw_panel(
            "Nostr Status Reference",
            nostr_ref_bounds,
            cx,
            |inner, cx| {
                // Verification statuses
                let mut ver_x = inner.origin.x;
                let verifications = [
                    ContactVerification::Verified,
                    ContactVerification::WebOfTrust,
                    ContactVerification::Unknown,
                ];

                for ver in &verifications {
                    let ver_w = (ver.label().len() as f32 * 7.0) + 16.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(ver_x, inner.origin.y, ver_w, 20.0))
                            .with_background(ver.color().with_alpha(0.2))
                            .with_border(ver.color(), 1.0),
                    );
                    let text = cx.text.layout(
                        ver.label(),
                        Point::new(ver_x + 6.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        ver.color(),
                    );
                    cx.scene.draw_text(text);
                    ver_x += ver_w + 12.0;
                }

                // Encryption statuses
                let mut enc_x = inner.origin.x;
                let encryptions = [
                    EncryptionStatus::Encrypted,
                    EncryptionStatus::Decrypted,
                    EncryptionStatus::Failed,
                ];

                for enc in &encryptions {
                    let enc_w = 80.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(enc_x, inner.origin.y + 35.0, enc_w, 20.0))
                            .with_background(enc.color().with_alpha(0.2))
                            .with_border(enc.color(), 1.0),
                    );
                    let label = format!(
                        "{} {}",
                        enc.icon(),
                        match enc {
                            EncryptionStatus::Encrypted => "Encrypted",
                            EncryptionStatus::Decrypted => "Decrypted",
                            EncryptionStatus::Failed => "Failed",
                        }
                    );
                    let text = cx.text.layout(
                        &label,
                        Point::new(enc_x + 6.0, inner.origin.y + 39.0),
                        theme::font_size::XS,
                        enc.color(),
                    );
                    cx.scene.draw_text(text);
                    enc_x += enc_w + 12.0;
                }

                // DM directions
                let mut dir_x = inner.origin.x;
                let directions = [
                    ("Incoming", Hsla::new(200.0, 0.6, 0.5, 1.0)),
                    ("Outgoing", theme::accent::PRIMARY),
                ];

                for (label, color) in &directions {
                    let dir_w = 80.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(dir_x, inner.origin.y + 70.0, dir_w, 20.0))
                            .with_background(color.with_alpha(0.2))
                            .with_border(*color, 1.0),
                    );
                    let text = cx.text.layout(
                        label,
                        Point::new(dir_x + 6.0, inner.origin.y + 74.0),
                        theme::font_size::XS,
                        *color,
                    );
                    cx.scene.draw_text(text);
                    dir_x += dir_w + 12.0;
                }
            },
        );
    }

    pub(crate) fn paint_sovereign_agent_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let profiles_height = panel_height(340.0);
        let signing_height = panel_height(400.0);
        let matrix_height = panel_height(280.0);
        let inspector_height = panel_height(450.0);
        let key_mgr_height = panel_height(450.0);
        let schedule_height = panel_height(400.0);
        let agent_ref_height = panel_height(180.0);

        let panels = panel_stack(
            bounds,
            &[
                profiles_height,
                signing_height,
                matrix_height,
                inspector_height,
                key_mgr_height,
                schedule_height,
                agent_ref_height,
            ],
        );

        // ========== Panel 1: Agent Profiles ==========
        let profiles_bounds = panels[0];
        draw_panel(
            "Sovereign Agent Profiles",
            profiles_bounds,
            cx,
            |inner, cx| {
                let agents = [
                    AgentProfileInfo::new("agent-1", "CodeReviewer", AgentType::Sovereign)
                        .status(AgentStatus::Busy)
                        .npub("npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq")
                        .description("AI-powered code review with security analysis")
                        .capabilities(vec![
                            "code_review".to_string(),
                            "testing".to_string(),
                            "security".to_string(),
                        ])
                        .created_at("2 weeks ago")
                        .last_active("Just now"),
                    AgentProfileInfo::new("agent-2", "DataProcessor", AgentType::Custodial)
                        .status(AgentStatus::Idle)
                        .description("Processes and transforms data pipelines")
                        .capabilities(vec!["data_transform".to_string(), "etl".to_string()])
                        .created_at("1 month ago")
                        .last_active("5 min ago"),
                    AgentProfileInfo::new("agent-3", "MarketWatch", AgentType::Sovereign)
                        .status(AgentStatus::Online)
                        .description("Monitors market conditions and sends alerts")
                        .capabilities(vec!["monitoring".to_string(), "alerts".to_string()])
                        .created_at("3 days ago"),
                ];

                for (i, agent) in agents.iter().enumerate() {
                    let mut card = AgentProfileCard::new(agent.clone());
                    card.paint(
                        Bounds::new(
                            inner.origin.x,
                            inner.origin.y + i as f32 * 105.0,
                            inner.size.width.min(520.0),
                            100.0,
                        ),
                        cx,
                    );
                }
            },
        );

        // ========== Panel 2: Signing Requests (FROSTR) ==========
        let signing_bounds = panels[1];
        draw_panel(
            "Threshold Signing Requests",
            signing_bounds,
            cx,
            |inner, cx| {
                let requests = [
                    SigningRequestInfo::new(
                        "sr1",
                        SigningType::Transaction,
                        "Send 0.05 BTC to bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
                        "Agent-CodeReviewer",
                    )
                    .urgency(SigningUrgency::Urgent)
                    .threshold(1, 2)
                    .expires_in("5 minutes")
                    .created_at("2 min ago"),
                    SigningRequestInfo::new(
                        "sr2",
                        SigningType::Event,
                        "Publish NIP-90 job result event to nostr relays",
                        "Agent-DataProcessor",
                    )
                    .urgency(SigningUrgency::Normal)
                    .threshold(0, 3)
                    .expires_in("1 hour")
                    .created_at("10 min ago"),
                    SigningRequestInfo::new(
                        "sr3",
                        SigningType::Message,
                        "Sign DM reply to npub1alice...",
                        "Agent-MarketWatch",
                    )
                    .urgency(SigningUrgency::Normal)
                    .threshold(2, 2)
                    .created_at("1 hour ago"),
                    SigningRequestInfo::new(
                        "sr4",
                        SigningType::KeyRotation,
                        "Rotate threshold key shares - quarterly rotation",
                        "System",
                    )
                    .urgency(SigningUrgency::Expired)
                    .threshold(1, 3)
                    .expires_in("expired")
                    .created_at("2 days ago"),
                ];

                for (i, req) in requests.iter().enumerate() {
                    let mut card = SigningRequestCard::new(req.clone());
                    card.paint(
                        Bounds::new(
                            inner.origin.x,
                            inner.origin.y + i as f32 * 100.0,
                            inner.size.width.min(550.0),
                            95.0,
                        ),
                        cx,
                    );
                }
            },
        );

        // ========== Panel 3: Agent Status Matrix ==========
        let matrix_bounds = panels[2];
        draw_panel("Agent Status Overview", matrix_bounds, cx, |inner, cx| {
            // Status summary header
            let statuses = [
                (AgentStatus::Online, 3),
                (AgentStatus::Busy, 2),
                (AgentStatus::Idle, 5),
                (AgentStatus::Error, 1),
                (AgentStatus::Offline, 0),
            ];

            let mut status_x = inner.origin.x;
            for (status, count) in &statuses {
                let status_w = 100.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y, status_w, 50.0))
                        .with_background(status.color().with_alpha(0.1))
                        .with_border(status.color().with_alpha(0.5), 1.0),
                );
                let count_run = cx.text.layout(
                    &count.to_string(),
                    Point::new(status_x + 40.0, inner.origin.y + 8.0),
                    theme::font_size::LG,
                    status.color(),
                );
                cx.scene.draw_text(count_run);
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 10.0, inner.origin.y + 32.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(label_run);
                status_x += status_w + 8.0;
            }

            // Threshold key status
            let key_y = inner.origin.y + 70.0;
            let key_text = cx.text.layout(
                "Threshold Keys: 2-of-3 active",
                Point::new(inner.origin.x, key_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(key_text);

            // Key share indicators
            let shares = [
                ("Share 1", true, "Local"),
                ("Share 2", true, "Hardware Key"),
                ("Share 3", false, "Cloud Backup"),
            ];

            let mut share_x = inner.origin.x;
            for (label, active, location) in &shares {
                let share_w = 140.0;
                let color = if *active {
                    Hsla::new(120.0, 0.6, 0.45, 1.0)
                } else {
                    theme::text::MUTED
                };
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(share_x, key_y + 25.0, share_w, 40.0))
                        .with_background(color.with_alpha(0.1))
                        .with_border(color, 1.0),
                );
                let share_run = cx.text.layout(
                    label,
                    Point::new(share_x + 8.0, key_y + 30.0),
                    theme::font_size::XS,
                    color,
                );
                cx.scene.draw_text(share_run);
                let loc_run = cx.text.layout(
                    location,
                    Point::new(share_x + 8.0, key_y + 46.0),
                    10.0,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(loc_run);
                share_x += share_w + 10.0;
            }

            // Pending signatures counter
            let pending_y = key_y + 85.0;
            let pending_run = cx.text.layout(
                "Pending Signatures: 4",
                Point::new(inner.origin.x, pending_y),
                theme::font_size::SM,
                Hsla::new(30.0, 0.8, 0.5, 1.0),
            );
            cx.scene.draw_text(pending_run);
        });

        // ========== Panel 4: Agent State Inspector Organism ==========
        let inspector_bounds = panels[3];
        draw_panel(
            "Agent State Inspector (Organism)",
            inspector_bounds,
            cx,
            |inner, cx| {
                let goals = vec![
                    AgentGoal::new("g1", "Complete code review for PR #123")
                        .progress(0.75)
                        .status(AgentGoalStatus::Active),
                    AgentGoal::new("g2", "Run security scan on dependencies")
                        .progress(1.0)
                        .status(AgentGoalStatus::Completed),
                    AgentGoal::new("g3", "Waiting for API rate limit reset")
                        .progress(0.3)
                        .status(AgentGoalStatus::Blocked),
                ];
                let actions = vec![
                    AgentAction::new("Read", "Reading src/main.rs").timestamp("12:34"),
                    AgentAction::new("Edit", "Modified config.toml").timestamp("12:35"),
                    AgentAction::new("Bash", "Running tests...")
                        .timestamp("12:36")
                        .success(false),
                ];
                let resources = ResourceUsage {
                    tokens_used: 45000,
                    tokens_limit: 100000,
                    actions_count: 47,
                    runtime_seconds: 384,
                };
                let mut inspector = AgentStateInspector::new("CodeReviewer", "agent-123")
                    .goals(goals)
                    .actions(actions)
                    .memory(vec![
                        ("current_file".to_string(), "src/main.rs".to_string()),
                        ("branch".to_string(), "feature/auth".to_string()),
                    ])
                    .resources(resources);
                inspector.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        400.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 5: Threshold Key Manager Organism ==========
        let key_mgr_bounds = panels[4];
        draw_panel(
            "FROSTR Key Manager (Organism)",
            key_mgr_bounds,
            cx,
            |inner, cx| {
                let key_share = KeyShare::new("key-001", 1, 2, 3)
                    .created_at("2024-01-15")
                    .backed_up(true);
                let peers = vec![
                    ThresholdPeer::new("npub1alice...", "Alice (Local)", 1)
                        .status(PeerStatus::Online)
                        .last_seen("Now"),
                    ThresholdPeer::new("npub1bob...", "Bob (Hardware)", 2)
                        .status(PeerStatus::Signing)
                        .last_seen("Just now"),
                    ThresholdPeer::new("npub1carol...", "Carol (Cloud)", 3)
                        .status(PeerStatus::Offline)
                        .last_seen("5 min ago"),
                ];
                let requests = vec![
                    SigningRequest::new("req-1", "Sign Bitcoin transaction: 0.05 BTC")
                        .requester("CodeReviewer Agent")
                        .timestamp("2 min ago")
                        .progress(1, 2),
                ];
                let mut key_manager = ThresholdKeyManager::new()
                    .key_share(key_share)
                    .peers(peers)
                    .requests(requests);
                key_manager.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        400.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 6: Schedule Configuration Organism ==========
        let schedule_bounds = panels[5];
        draw_panel(
            "Schedule Configuration (Organism)",
            schedule_bounds,
            cx,
            |inner, cx| {
                let config = ScheduleData::new(ScheduleType::Continuous)
                    .heartbeat(30, IntervalUnit::Seconds)
                    .tick(5, IntervalUnit::Minutes)
                    .enabled(true)
                    .next_run(1700050000)
                    .last_run(1700000000);

                let mut schedule = ScheduleConfig::new(config);
                schedule.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(450.0),
                        360.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 7: Type & Status Reference ==========
        let agent_ref_bounds = panels[6];
        draw_panel(
            "Agent Types & Statuses",
            agent_ref_bounds,
            cx,
            |inner, cx| {
                // Agent types
                let mut type_x = inner.origin.x;
                let types = [AgentType::Human, AgentType::Sovereign, AgentType::Custodial];

                for agent_type in &types {
                    let type_w = (agent_type.label().len() as f32 * 7.0) + 24.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(type_x, inner.origin.y, type_w, 22.0))
                            .with_background(agent_type.color().with_alpha(0.2))
                            .with_border(agent_type.color(), 1.0),
                    );
                    let icon = agent_type.icon();
                    let icon_run = cx.text.layout(
                        icon,
                        Point::new(type_x + 4.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        agent_type.color(),
                    );
                    cx.scene.draw_text(icon_run);
                    let label_run = cx.text.layout(
                        agent_type.label(),
                        Point::new(type_x + 18.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        agent_type.color(),
                    );
                    cx.scene.draw_text(label_run);
                    type_x += type_w + 10.0;
                }

                // Agent statuses
                let mut status_x = inner.origin.x;
                let statuses = [
                    AgentStatus::Online,
                    AgentStatus::Busy,
                    AgentStatus::Idle,
                    AgentStatus::Error,
                    AgentStatus::Offline,
                ];

                for status in &statuses {
                    let status_w = (status.label().len() as f32 * 6.0) + 14.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(status_x, inner.origin.y + 35.0, status_w, 20.0))
                            .with_background(status.color().with_alpha(0.2))
                            .with_border(status.color(), 1.0),
                    );
                    let status_run = cx.text.layout(
                        status.label(),
                        Point::new(status_x + 6.0, inner.origin.y + 39.0),
                        theme::font_size::XS,
                        status.color(),
                    );
                    cx.scene.draw_text(status_run);
                    status_x += status_w + 8.0;
                }

                // Signing types
                let mut sig_x = inner.origin.x;
                let sig_types = [
                    SigningType::Transaction,
                    SigningType::Message,
                    SigningType::Event,
                    SigningType::KeyRotation,
                ];

                for sig_type in &sig_types {
                    let sig_w = (sig_type.label().len() as f32 * 6.0) + 22.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(sig_x, inner.origin.y + 70.0, sig_w, 20.0))
                            .with_background(sig_type.color().with_alpha(0.2))
                            .with_border(sig_type.color(), 1.0),
                    );
                    let icon_run = cx.text.layout(
                        sig_type.icon(),
                        Point::new(sig_x + 4.0, inner.origin.y + 74.0),
                        theme::font_size::XS,
                        sig_type.color(),
                    );
                    cx.scene.draw_text(icon_run);
                    let sig_run = cx.text.layout(
                        sig_type.label(),
                        Point::new(sig_x + 16.0, inner.origin.y + 74.0),
                        theme::font_size::XS,
                        sig_type.color(),
                    );
                    cx.scene.draw_text(sig_run);
                    sig_x += sig_w + 8.0;
                }

                // Urgency levels
                let mut urg_x = inner.origin.x;
                let urgencies = [
                    SigningUrgency::Normal,
                    SigningUrgency::Urgent,
                    SigningUrgency::Expired,
                ];

                for urgency in &urgencies {
                    let urg_w = (urgency.label().len() as f32 * 6.0) + 12.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(urg_x, inner.origin.y + 105.0, urg_w, 20.0))
                            .with_background(urgency.color().with_alpha(0.2))
                            .with_border(urgency.color(), 1.0),
                    );
                    let urg_run = cx.text.layout(
                        urgency.label(),
                        Point::new(urg_x + 5.0, inner.origin.y + 109.0),
                        theme::font_size::XS,
                        urgency.color(),
                    );
                    cx.scene.draw_text(urg_run);
                    urg_x += urg_w + 8.0;
                }
            },
        );
    }
}
