use wgpui::components::atoms::{
    AgentScheduleBadge, AgentStatus, AgentStatusBadge, AgentType, AmountDirection, ApmGauge,
    Bech32Entity, Bech32Type, BitcoinAmount, BitcoinNetwork, BitcoinUnit, BountyBadge,
    BountyStatus, CheckpointBadge, ContentType, ContentTypeIcon, DaemonStatus, DaemonStatusBadge,
    EarningsBadge, EarningsType, EntryMarker, EntryType, EventKind, EventKindBadge, GoalPriority,
    GoalProgressBadge, GoalStatus, IssueStatus, IssueStatusBadge, JobStatus, JobStatusBadge,
    KeybindingHint, LicenseStatus, MarketType, MarketTypeBadge, Mode, ModeBadge, Model, ModelBadge,
    NetworkBadge, ParallelAgentBadge, ParallelAgentStatus, PaymentMethod, PaymentMethodIcon,
    PaymentStatus, PaymentStatusBadge, PermissionAction, PermissionButton, PrStatus, PrStatusBadge,
    RelayStatus, RelayStatusBadge, RelayStatusDot, ReputationBadge, ResourceType, ResourceUsageBar,
    SessionStatus, SessionStatusBadge, SkillLicenseBadge, SkillType, StackLayerBadge,
    StackLayerStatus, Status, StatusDot, ThinkingToggle, ThresholdKeyBadge, TickEventBadge,
    TickOutcome, ToolIcon, ToolStatus, ToolStatusBadge, ToolType, TrajectorySource,
    TrajectorySourceBadge, TrajectoryStatus, TrajectoryStatusBadge, TrustTier,
};
use wgpui::components::molecules::{DiffHeader, DiffType, MessageHeader, ToolHeader};
use wgpui::components::organisms::{
    SearchToolCall, TerminalToolCall, ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
};
use wgpui::{Bounds, Component, InputEvent, Key, MouseButton, PaintContext, Point, Text, theme};

use crate::constants::GAP;
use crate::helpers::{
    center_bounds, component_event, draw_panel, panel_height, panel_inner, panel_stack,
};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_atoms(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let panel_heights = [
            panel_height(140.0),
            panel_height(160.0),
            panel_height(180.0),
            panel_height(180.0),
            panel_height(180.0),
            panel_height(180.0),
            panel_height(180.0),
            panel_height(180.0),
            panel_height(160.0),
        ];
        let panels = panel_stack(bounds, &panel_heights);

        // ========== Panel 1: Tool & Status Atoms ==========
        let tool_bounds = panels[0];
        draw_panel("Tool & Status Atoms", tool_bounds, cx, |inner, cx| {
            let mut x = inner.origin.x;
            let row_y = inner.origin.y;

            // Tool icons
            for tool_type in &[
                ToolType::Bash,
                ToolType::Read,
                ToolType::Edit,
                ToolType::Search,
            ] {
                let mut icon = ToolIcon::new(*tool_type);
                icon.paint(Bounds::new(x, row_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Tool status badges
            x = inner.origin.x;
            let status_y = row_y + 35.0;
            for status in &[ToolStatus::Running, ToolStatus::Success, ToolStatus::Error] {
                let mut badge = ToolStatusBadge::new(*status);
                badge.paint(Bounds::new(x, status_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Status dots
            x = inner.origin.x;
            let dots_y = status_y + 35.0;
            for status in &[Status::Online, Status::Busy, Status::Away, Status::Error] {
                let mut dot = StatusDot::new(*status).size(10.0);
                dot.paint(Bounds::new(x, dots_y, 12.0, 12.0), cx);
                let label = match status {
                    Status::Online => "Online",
                    Status::Busy => "Busy",
                    Status::Away => "Away",
                    Status::Error => "Error",
                    _ => "",
                };
                let label_run = cx.text.layout(
                    label,
                    Point::new(x + 16.0, dots_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);
                x += 70.0;
            }
        });

        // ========== Panel 2: Mode & Model Atoms ==========
        let mode_bounds = panels[1];
        draw_panel("Mode & Model Atoms", mode_bounds, cx, |inner, cx| {
            // Mode badges
            let mut x = inner.origin.x;
            for mode in &[Mode::Normal, Mode::Act, Mode::Plan] {
                let mut badge = ModeBadge::new(*mode);
                badge.paint(Bounds::new(x, inner.origin.y, 70.0, 22.0), cx);
                x += 80.0;
            }

            // Model badges
            x = inner.origin.x;
            let model_y = inner.origin.y + 35.0;
            for model in &[Model::CodexSonnet, Model::CodexOpus, Model::CodexHaiku] {
                let mut badge = ModelBadge::new(*model);
                badge.paint(Bounds::new(x, model_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Content types
            x = inner.origin.x;
            let content_y = model_y + 35.0;
            for content in &[
                ContentType::Markdown,
                ContentType::Code,
                ContentType::Image,
                ContentType::Text,
            ] {
                let mut icon = ContentTypeIcon::new(*content);
                icon.paint(Bounds::new(x, content_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Entry markers
            x = inner.origin.x + 180.0;
            for entry in &[
                EntryType::User,
                EntryType::Assistant,
                EntryType::Tool,
                EntryType::System,
            ] {
                let mut marker = EntryMarker::new(*entry);
                marker.paint(Bounds::new(x, content_y, 28.0, 22.0), cx);
                x += 36.0;
            }
        });

        // ========== Panel 3: Agent Status Badges ==========
        let agent_bounds = panels[2];
        draw_panel("Agent Status Badges", agent_bounds, cx, |inner, cx| {
            // Agent status badges
            let mut x = inner.origin.x;
            for (status, atype) in &[
                (AgentStatus::Idle, AgentType::Human),
                (AgentStatus::Online, AgentType::Sovereign),
                (AgentStatus::Busy, AgentType::Sovereign),
                (AgentStatus::Error, AgentType::Custodial),
            ] {
                let mut badge = AgentStatusBadge::new(*status).agent_type(*atype);
                badge.paint(Bounds::new(x, inner.origin.y, 120.0, 22.0), cx);
                x += 130.0;
            }

            // Agent schedule badges (heartbeat intervals)
            x = inner.origin.x;
            let sched_y = inner.origin.y + 35.0;
            for seconds in &[60, 300, 900, 3600] {
                let mut badge = AgentScheduleBadge::new(*seconds);
                badge.paint(Bounds::new(x, sched_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Goal progress badges
            x = inner.origin.x;
            let goal_y = sched_y + 35.0;
            for (progress, status, priority) in &[
                (0.0, GoalStatus::NotStarted, GoalPriority::Low),
                (0.5, GoalStatus::InProgress, GoalPriority::Medium),
                (1.0, GoalStatus::Completed, GoalPriority::High),
                (0.3, GoalStatus::Blocked, GoalPriority::Critical),
            ] {
                let mut badge = GoalProgressBadge::new(*progress)
                    .status(*status)
                    .priority(*priority);
                badge.paint(Bounds::new(x, goal_y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Stack layer badges
            x = inner.origin.x;
            let stack_y = goal_y + 35.0;
            for (layer, status) in &[
                (1, StackLayerStatus::Pending),
                (2, StackLayerStatus::Ready),
                (3, StackLayerStatus::Merged),
            ] {
                let mut badge = StackLayerBadge::new(*layer, 3).status(status.clone());
                badge.paint(Bounds::new(x, stack_y, 100.0, 22.0), cx);
                x += 110.0;
            }
        });

        // ========== Panel 4: Bitcoin & Payment Atoms ==========
        let btc_bounds = panels[3];
        draw_panel("Bitcoin & Payment Atoms", btc_bounds, cx, |inner, cx| {
            // Bitcoin amounts
            let mut x = inner.origin.x;
            for (sats, unit, dir) in &[
                (100_000u64, BitcoinUnit::Sats, AmountDirection::Incoming),
                (100_000u64, BitcoinUnit::Btc, AmountDirection::Outgoing),
                (50_000u64, BitcoinUnit::Sats, AmountDirection::Neutral),
            ] {
                let mut badge = BitcoinAmount::new(*sats).unit(*unit).direction(*dir);
                badge.paint(Bounds::new(x, inner.origin.y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Network badges
            x = inner.origin.x;
            let net_y = inner.origin.y + 35.0;
            for network in &[
                BitcoinNetwork::Mainnet,
                BitcoinNetwork::Testnet,
                BitcoinNetwork::Signet,
                BitcoinNetwork::Regtest,
            ] {
                let mut badge = NetworkBadge::new(*network);
                badge.paint(Bounds::new(x, net_y, 80.0, 22.0), cx);
                x += 90.0;
            }

            // Payment method icons
            x = inner.origin.x;
            let method_y = net_y + 35.0;
            for method in &[
                PaymentMethod::Lightning,
                PaymentMethod::OnChain,
                PaymentMethod::Spark,
            ] {
                let mut icon = PaymentMethodIcon::new(*method);
                icon.paint(Bounds::new(x, method_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Payment status badges
            x = inner.origin.x + 120.0;
            for status in &[
                PaymentStatus::Pending,
                PaymentStatus::Completed,
                PaymentStatus::Failed,
            ] {
                let mut badge = PaymentStatusBadge::new(*status);
                badge.paint(Bounds::new(x, method_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Threshold key badges
            x = inner.origin.x;
            let key_y = method_y + 35.0;
            let mut key1 = ThresholdKeyBadge::new(1, 3);
            key1.paint(Bounds::new(x, key_y, 80.0, 22.0), cx);
            let mut key2 = ThresholdKeyBadge::new(2, 3);
            key2.paint(Bounds::new(x + 90.0, key_y, 80.0, 22.0), cx);
        });

        // ========== Panel 5: Nostr Protocol Atoms ==========
        let nostr_bounds = panels[4];
        draw_panel("Nostr Protocol Atoms", nostr_bounds, cx, |inner, cx| {
            // Relay status badges
            let mut x = inner.origin.x;
            for status in &[
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
            ] {
                let mut badge = RelayStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 160.0, 22.0), cx);
                x += 170.0;
            }

            // Relay status dots
            x = inner.origin.x;
            let dot_y = inner.origin.y + 35.0;
            for status in &[
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
            ] {
                let mut dot = RelayStatusDot::new(*status);
                dot.paint(Bounds::new(x, dot_y, 12.0, 12.0), cx);
                x += 24.0;
            }

            // Event kind badges
            x = inner.origin.x;
            let event_y = dot_y + 30.0;
            for kind in &[
                EventKind::TextNote,
                EventKind::EncryptedDm,
                EventKind::Reaction,
                EventKind::Repost,
            ] {
                let mut badge = EventKindBadge::new(*kind);
                badge.paint(Bounds::new(x, event_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Bech32 entities
            x = inner.origin.x;
            let bech_y = event_y + 35.0;
            for btype in &[Bech32Type::Npub, Bech32Type::Note, Bech32Type::Nevent] {
                let mut entity = Bech32Entity::new(*btype, "abc123def456");
                entity.paint(Bounds::new(x, bech_y, 140.0, 22.0), cx);
                x += 150.0;
            }
        });

        // ========== Panel 6: GitAfter Atoms ==========
        let git_bounds = panels[5];
        draw_panel("GitAfter Atoms", git_bounds, cx, |inner, cx| {
            // Issue status badges
            let mut x = inner.origin.x;
            for status in &[
                IssueStatus::Open,
                IssueStatus::InProgress,
                IssueStatus::Closed,
                IssueStatus::Claimed,
            ] {
                let mut badge = IssueStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // PR status badges
            x = inner.origin.x;
            let pr_y = inner.origin.y + 35.0;
            for status in &[
                PrStatus::Open,
                PrStatus::Merged,
                PrStatus::Closed,
                PrStatus::Draft,
            ] {
                let mut badge = PrStatusBadge::new(*status);
                badge.paint(Bounds::new(x, pr_y, 80.0, 22.0), cx);
                x += 90.0;
            }

            // Bounty badges
            x = inner.origin.x;
            let bounty_y = pr_y + 35.0;
            for (status, sats) in &[
                (BountyStatus::Active, 50000u64),
                (BountyStatus::Claimed, 100000u64),
                (BountyStatus::Paid, 250000u64),
                (BountyStatus::Expired, 10000u64),
            ] {
                let mut badge = BountyBadge::new(*sats).status(*status);
                badge.paint(Bounds::new(x, bounty_y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Tick event badges
            x = inner.origin.x;
            let tick_y = bounty_y + 35.0;
            let mut request = TickEventBadge::request();
            request.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
            x += 110.0;
            let mut result_success = TickEventBadge::result(TickOutcome::Success);
            result_success.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
            x += 110.0;
            let mut result_fail = TickEventBadge::result(TickOutcome::Failure);
            result_fail.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
        });

        // ========== Panel 7: Marketplace Atoms ==========
        let market_bounds = panels[6];
        draw_panel("Marketplace Atoms", market_bounds, cx, |inner, cx| {
            // Market type badges
            let mut x = inner.origin.x;
            for mtype in &[
                MarketType::Compute,
                MarketType::Skills,
                MarketType::Data,
                MarketType::Trajectories,
            ] {
                let mut badge = MarketTypeBadge::new(*mtype);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Job status badges
            x = inner.origin.x;
            let job_y = inner.origin.y + 35.0;
            for status in &[
                JobStatus::Pending,
                JobStatus::Processing,
                JobStatus::Completed,
                JobStatus::Failed,
            ] {
                let mut badge = JobStatusBadge::new(*status);
                badge.paint(Bounds::new(x, job_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Reputation badges
            x = inner.origin.x;
            let rep_y = job_y + 35.0;
            for tier in &[
                TrustTier::New,
                TrustTier::Established,
                TrustTier::Trusted,
                TrustTier::Expert,
            ] {
                let mut badge = ReputationBadge::new(*tier);
                badge.paint(Bounds::new(x, rep_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Trajectory source badges
            x = inner.origin.x;
            let traj_y = rep_y + 35.0;
            for source in &[
                TrajectorySource::Codex,
                TrajectorySource::Cursor,
                TrajectorySource::Windsurf,
            ] {
                let mut badge = TrajectorySourceBadge::new(*source);
                badge.paint(Bounds::new(x, traj_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Trajectory status badges
            for status in &[
                TrajectoryStatus::Verified,
                TrajectoryStatus::Partial,
                TrajectoryStatus::Suspicious,
            ] {
                let mut badge = TrajectoryStatusBadge::new(*status);
                badge.paint(Bounds::new(x, traj_y, 100.0, 22.0), cx);
                x += 110.0;
            }
        });

        // ========== Panel 8: Autopilot Atoms ==========
        let auto_bounds = panels[7];
        draw_panel("Autopilot Atoms", auto_bounds, cx, |inner, cx| {
            // Session status badges
            let mut x = inner.origin.x;
            for status in &[
                SessionStatus::Pending,
                SessionStatus::Running,
                SessionStatus::Completed,
                SessionStatus::Failed,
            ] {
                let mut badge = SessionStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // APM gauges
            x = inner.origin.x;
            let apm_y = inner.origin.y + 35.0;
            for apm in &[0.0, 15.0, 45.0, 80.0] {
                let mut gauge = ApmGauge::new(*apm).compact(true);
                gauge.paint(Bounds::new(x, apm_y, 70.0, 22.0), cx);
                x += 80.0;
            }

            // Resource usage bars
            x = inner.origin.x;
            let res_y = apm_y + 35.0;
            for (rtype, pct) in &[
                (ResourceType::Memory, 35.0),
                (ResourceType::Memory, 75.0),
                (ResourceType::Cpu, 50.0),
            ] {
                let mut bar = ResourceUsageBar::new(*rtype, *pct).bar_width(50.0);
                bar.paint(Bounds::new(x, res_y, 140.0, 22.0), cx);
                x += 150.0;
            }

            // Daemon status badges
            x = inner.origin.x;
            let daemon_y = res_y + 35.0;
            for status in &[
                DaemonStatus::Offline,
                DaemonStatus::Online,
                DaemonStatus::Error,
            ] {
                let mut badge = DaemonStatusBadge::new(*status).compact(true);
                badge.paint(Bounds::new(x, daemon_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Parallel agent badges
            x = inner.origin.x + 120.0;
            for (idx, status) in &[
                (0, ParallelAgentStatus::Idle),
                (1, ParallelAgentStatus::Running),
                (2, ParallelAgentStatus::Completed),
            ] {
                let mut badge = ParallelAgentBadge::new(*idx, *status).compact(true);
                badge.paint(Bounds::new(x, daemon_y, 50.0, 22.0), cx);
                x += 60.0;
            }
        });

        // ========== Panel 9: Interactive Atoms ==========
        let interact_bounds = panels[8];
        draw_panel("Interactive Atoms", interact_bounds, cx, |inner, cx| {
            // Permission buttons
            let mut x = inner.origin.x;
            for action in &[
                PermissionAction::AllowOnce,
                PermissionAction::AllowAlways,
                PermissionAction::Deny,
            ] {
                let mut btn = PermissionButton::new(*action);
                btn.paint(Bounds::new(x, inner.origin.y, 100.0, 26.0), cx);
                x += 110.0;
            }

            // Thinking toggle
            x = inner.origin.x;
            let control_y = inner.origin.y + 38.0;
            let mut toggle = ThinkingToggle::new().expanded(true);
            toggle.paint(Bounds::new(x, control_y, 120.0, 26.0), cx);

            // Keybinding hints
            x = inner.origin.x;
            let key_y = control_y + 38.0;
            let mut hint1 = KeybindingHint::single("K");
            hint1.paint(Bounds::new(x, key_y, 24.0, 22.0), cx);
            let mut hint2 = KeybindingHint::combo(&["Ctrl", "K"]);
            hint2.paint(Bounds::new(x + 32.0, key_y, 60.0, 22.0), cx);
            let mut hint3 = KeybindingHint::combo(&["Cmd", "Shift", "P"]);
            hint3.paint(Bounds::new(x + 100.0, key_y, 100.0, 22.0), cx);

            // Checkpoint badges
            let mut cp1 = CheckpointBadge::new("v1.0").active(false);
            cp1.paint(Bounds::new(x + 220.0, key_y, 60.0, 22.0), cx);
            let mut cp2 = CheckpointBadge::new("v1.2").active(true);
            cp2.paint(Bounds::new(x + 290.0, key_y, 60.0, 22.0), cx);

            // Streaming indicator
            self.streaming_indicator
                .paint(Bounds::new(x + 370.0, key_y, 80.0, 22.0), cx);

            // Skill license badges
            x = inner.origin.x;
            let skill_y = key_y + 32.0;
            for (stype, lstatus) in &[
                (SkillType::Code, LicenseStatus::Active),
                (SkillType::Data, LicenseStatus::Expired),
                (SkillType::Model, LicenseStatus::Pending),
            ] {
                let mut badge = SkillLicenseBadge::new(*stype, *lstatus);
                badge.paint(Bounds::new(x, skill_y, 110.0, 22.0), cx);
                x += 120.0;
            }

            // Earnings badges (compact)
            for etype in &[
                EarningsType::Compute,
                EarningsType::Skills,
                EarningsType::Data,
            ] {
                let mut badge = EarningsBadge::new(*etype, 25000).compact(true);
                badge.paint(Bounds::new(x, skill_y, 70.0, 22.0), cx);
                x += 80.0;
            }
        });
    }

    pub(crate) fn paint_molecules(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let selectors = Bounds::new(left_x, bounds.origin.y, col_width, 90.0);
        let permission = Bounds::new(left_x, bounds.origin.y + 110.0, col_width, 70.0);
        let checkpoints = Bounds::new(left_x, bounds.origin.y + 200.0, col_width, 90.0);

        let thinking = Bounds::new(right_x, bounds.origin.y, col_width, 170.0);
        let headers = Bounds::new(right_x, bounds.origin.y + 190.0, col_width, 120.0);

        draw_panel("Selectors", selectors, cx, |inner, cx| {
            let selector_h = 28.0;
            self.mode_selector.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, selector_h),
                cx,
            );
            self.model_selector.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + selector_h + 12.0,
                    inner.size.width,
                    selector_h,
                ),
                cx,
            );
        });

        draw_panel("Permission bar", permission, cx, |inner, cx| {
            self.permission_bar.paint(inner, cx);
        });

        draw_panel("Checkpoint restore", checkpoints, cx, |inner, cx| {
            self.checkpoint_restore.paint(inner, cx);
        });

        draw_panel("Thinking block", thinking, cx, |inner, cx| {
            self.thinking_block.paint(inner, cx);
        });

        draw_panel("Headers", headers, cx, |inner, cx| {
            let row_height = 28.0;
            let mut header = MessageHeader::assistant(Model::CodexHaiku).timestamp("12:42");
            header.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, row_height),
                cx,
            );
            let mut tool_header =
                ToolHeader::new(ToolType::Read, "read_file").status(ToolStatus::Success);
            tool_header.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + row_height + 8.0,
                    inner.size.width,
                    row_height,
                ),
                cx,
            );
            let mut diff_header = DiffHeader::new("src/main.rs")
                .additions(3)
                .deletions(1)
                .diff_type(DiffType::Unified);
            diff_header.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + (row_height + 8.0) * 2.0,
                    inner.size.width,
                    row_height,
                ),
                cx,
            );
        });
    }

    pub(crate) fn handle_molecules_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let selectors = Bounds::new(left_x, bounds.origin.y, col_width, 90.0);
        let permission = Bounds::new(left_x, bounds.origin.y + 110.0, col_width, 70.0);
        let checkpoints = Bounds::new(left_x, bounds.origin.y + 200.0, col_width, 90.0);
        let thinking = Bounds::new(right_x, bounds.origin.y, col_width, 170.0);

        let selectors_inner = panel_inner(selectors);
        let permission_inner = panel_inner(permission);
        let checkpoints_inner = panel_inner(checkpoints);
        let thinking_inner = panel_inner(thinking);

        let mut handled = false;
        handled |= component_event(
            &mut self.mode_selector,
            event,
            Bounds::new(
                selectors_inner.origin.x,
                selectors_inner.origin.y,
                selectors_inner.size.width,
                28.0,
            ),
            &mut self.event_context,
        );
        handled |= component_event(
            &mut self.model_selector,
            event,
            Bounds::new(
                selectors_inner.origin.x,
                selectors_inner.origin.y + 40.0,
                selectors_inner.size.width,
                28.0,
            ),
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.permission_bar,
            event,
            permission_inner,
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.checkpoint_restore,
            event,
            checkpoints_inner,
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.thinking_block,
            event,
            thinking_inner,
            &mut self.event_context,
        );

        handled
    }

    pub(crate) fn paint_organisms(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let user_msg = Bounds::new(left_x, bounds.origin.y, col_width, 140.0);
        let asst_msg = Bounds::new(left_x, bounds.origin.y + 160.0, col_width, 180.0);
        let thread_entry = Bounds::new(left_x, bounds.origin.y + 360.0, col_width, 120.0);

        let controls = Bounds::new(right_x, bounds.origin.y, col_width, 80.0);
        let tool_card = Bounds::new(right_x, bounds.origin.y + 100.0, col_width, 160.0);
        let terminal = Bounds::new(right_x, bounds.origin.y + 280.0, col_width, 150.0);
        let permission = Bounds::new(right_x, bounds.origin.y + 450.0, col_width, 160.0);

        draw_panel("User message", user_msg, cx, |inner, cx| {
            let mut msg = UserMessage::new("User says hello from ACP.");
            msg.paint(inner, cx);
        });

        draw_panel("Assistant message", asst_msg, cx, |inner, cx| {
            self.assistant_message.paint(inner, cx);
        });

        draw_panel("Thread entry", thread_entry, cx, |inner, cx| {
            let mut entry = ThreadEntry::new(ThreadEntryType::System, Text::new("System note"));
            entry.paint(inner, cx);
        });

        draw_panel("Thread controls", controls, cx, |inner, cx| {
            self.thread_controls.paint(inner, cx);
        });

        draw_panel("Tool call card", tool_card, cx, |inner, cx| {
            let mut card = ToolCallCard::new(ToolType::Read, "read_file")
                .status(ToolStatus::Success)
                .input("path: /etc/hosts")
                .output("read 12 lines");
            card.paint(inner, cx);
        });

        draw_panel("Terminal tool", terminal, cx, |inner, cx| {
            let mut tool = TerminalToolCall::new("ls -la")
                .output("src\nCargo.toml\ntarget")
                .status(ToolStatus::Running);
            tool.paint(inner, cx);
        });

        draw_panel("Permission dialog", permission, cx, |inner, cx| {
            if self.show_permission_dialog {
                self.permission_dialog.show();
            } else {
                self.permission_dialog.hide();
            }
            self.permission_dialog.paint(inner, cx);
        });
    }

    pub(crate) fn handle_organisms_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let right_x = bounds.origin.x + col_width + col_gap;

        let controls = Bounds::new(right_x, bounds.origin.y, col_width, 80.0);
        let permission = Bounds::new(right_x, bounds.origin.y + 450.0, col_width, 160.0);
        let controls_inner = panel_inner(controls);
        let permission_inner = panel_inner(permission);

        let mut handled = false;
        handled |= component_event(
            &mut self.thread_controls,
            event,
            controls_inner,
            &mut self.event_context,
        );

        if let InputEvent::KeyDown { key, .. } = event {
            if let Key::Character(ch) = key {
                if ch.eq_ignore_ascii_case("p") {
                    self.show_permission_dialog = !self.show_permission_dialog;
                    handled = true;
                }
            }
        }

        handled |= component_event(
            &mut self.permission_dialog,
            event,
            permission_inner,
            &mut self.event_context,
        );

        if let InputEvent::MouseDown { button, x, y, .. } = event {
            if *button == MouseButton::Left
                && permission.contains(Point::new(*x, *y))
                && !self.permission_dialog.is_open()
            {
                self.permission_dialog.show();
                handled = true;
            }
        }

        handled
    }

    pub(crate) fn paint_interactions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let focus_panel = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 150.0);
        let tool_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 170.0,
            bounds.size.width,
            260.0,
        );
        let stream_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 450.0,
            bounds.size.width,
            140.0,
        );

        draw_panel("Focus and keyboard", focus_panel, cx, |inner, cx| {
            self.focus_demo.paint(inner, cx);
        });

        draw_panel("Tool cards", tool_panel, cx, |inner, cx| {
            let col_gap = 16.0;
            let col_width = ((inner.size.width - col_gap) / 2.0).max(0.0);
            let left = Bounds::new(inner.origin.x, inner.origin.y, col_width, inner.size.height);
            let right = Bounds::new(
                inner.origin.x + col_width + col_gap,
                inner.origin.y,
                col_width,
                inner.size.height,
            );

            let mut tool = ToolCallCard::new(ToolType::Search, "grep")
                .status(ToolStatus::Success)
                .input("query: todo")
                .output("6 matches");
            tool.paint(center_bounds(left, left.size.width, 140.0), cx);

            let mut search = SearchToolCall::new("todo").status(ToolStatus::Success);
            search.paint(center_bounds(right, right.size.width, 180.0), cx);
        });

        draw_panel("Streaming indicator", stream_panel, cx, |inner, cx| {
            let mut title = Text::new("Press S to toggle streaming")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            title.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 16.0),
                cx,
            );
            self.streaming_indicator
                .paint(center_bounds(inner, 120.0, 24.0), cx);
        });
    }

    pub(crate) fn handle_interactions_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let focus_panel = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 150.0);
        let stream_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 450.0,
            bounds.size.width,
            140.0,
        );
        let focus_inner = panel_inner(focus_panel);
        let stream_inner = panel_inner(stream_panel);

        let mut handled = self.focus_demo.handle_event(event, focus_inner);

        if let InputEvent::KeyDown { key, .. } = event {
            if let Key::Character(ch) = key {
                if ch.eq_ignore_ascii_case("s") {
                    let active = self.streaming_indicator.is_active();
                    self.streaming_indicator.set_active(!active);
                    handled = true;
                }
            }
        }

        if let InputEvent::MouseDown { button, x, y, .. } = event {
            if *button == MouseButton::Left && stream_inner.contains(Point::new(*x, *y)) {
                let active = self.streaming_indicator.is_active();
                self.streaming_indicator.set_active(!active);
                handled = true;
            }
        }

        handled
    }
}
