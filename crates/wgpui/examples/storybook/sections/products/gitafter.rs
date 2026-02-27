use super::*;

impl Storybook {
    pub(crate) fn paint_gitafter(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Issue Status Badges ==========
        let issue_height = panel_height(160.0);
        let issue_bounds = Bounds::new(bounds.origin.x, y, width, issue_height);
        draw_panel("Issue Status Badges", issue_bounds, cx, |inner, cx| {
            let statuses = [
                IssueStatus::Open,
                IssueStatus::Claimed,
                IssueStatus::InProgress,
                IssueStatus::Closed,
                IssueStatus::Draft,
            ];

            let tile_w = 100.0;
            let gap = 12.0;

            for (idx, status) in statuses.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = IssueStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 80.0, 22.0), cx);

                // Compact version
                let mut compact = IssueStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 48.0, 24.0, 22.0), cx);
            }
        });
        y += issue_height + SECTION_GAP;

        // ========== Panel 2: PR Status Badges ==========
        let pr_height = panel_height(180.0);
        let pr_bounds = Bounds::new(bounds.origin.x, y, width, pr_height);
        draw_panel("PR Status Badges", pr_bounds, cx, |inner, cx| {
            let statuses = [
                PrStatus::Draft,
                PrStatus::Open,
                PrStatus::NeedsReview,
                PrStatus::Approved,
                PrStatus::ChangesRequested,
                PrStatus::Merged,
                PrStatus::Closed,
            ];

            let tile_w = 80.0;
            let tile_h = 60.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = PrStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 70.0, 22.0), cx);
            }
        });
        y += pr_height + SECTION_GAP;

        // ========== Panel 3: Bounty Badges ==========
        let bounty_height = panel_height(140.0);
        let bounty_bounds = Bounds::new(bounds.origin.x, y, width, bounty_height);
        draw_panel("Bounty Badges", bounty_bounds, cx, |inner, cx| {
            let bounties = [
                (500, BountyStatus::Active),
                (5000, BountyStatus::Active),
                (50000, BountyStatus::Claimed),
                (100000, BountyStatus::Paid),
                (25000, BountyStatus::Expired),
            ];

            let tile_w = 110.0;
            let gap = 12.0;

            for (idx, (amount, status)) in bounties.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Status label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Bounty badge
                let mut badge = BountyBadge::new(*amount).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 24.0), cx);
            }
        });
        y += bounty_height + SECTION_GAP;

        // ========== Panel 4: Stack Layer Indicators ==========
        let stack_height = panel_height(160.0);
        let stack_bounds = Bounds::new(bounds.origin.x, y, width, stack_height);
        draw_panel("Stack Layer Indicators", stack_bounds, cx, |inner, cx| {
            let stacks = [
                (1, 4, StackLayerStatus::Merged),
                (2, 4, StackLayerStatus::Ready),
                (3, 4, StackLayerStatus::Pending),
                (4, 4, StackLayerStatus::Blocked),
            ];

            let tile_w = 120.0;
            let gap = 12.0;

            for (idx, (layer, total, status)) in stacks.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Full badge
                let mut badge = StackLayerBadge::new(*layer, *total).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 80.0, 24.0), cx);

                // Compact badge
                let mut compact = StackLayerBadge::new(*layer, *total)
                    .status(*status)
                    .compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 32.0, 36.0, 22.0), cx);
            }
        });
        y += stack_height + SECTION_GAP;

        // ========== Panel 5: Agent Status Badges ==========
        let agent_height = panel_height(180.0);
        let agent_bounds = Bounds::new(bounds.origin.x, y, width, agent_height);
        draw_panel(
            "Agent Status & Type Badges",
            agent_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    AgentStatus::Online,
                    AgentStatus::Busy,
                    AgentStatus::Idle,
                    AgentStatus::Offline,
                    AgentStatus::Error,
                ];

                // Row 1: Agent statuses
                let mut x = inner.origin.x;
                for status in &statuses {
                    let mut badge = AgentStatusBadge::new(*status).show_dot(true);
                    badge.paint(Bounds::new(x, inner.origin.y, 80.0, 24.0), cx);
                    x += 90.0;
                }

                // Row 2: Agent types
                let types = [AgentType::Human, AgentType::Sovereign, AgentType::Custodial];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 40.0;
                for agent_type in &types {
                    let mut badge =
                        AgentStatusBadge::new(AgentStatus::Online).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);
                    x += 110.0;
                }

                // Row 3: Combined status + type
                let combined = [
                    (AgentType::Sovereign, AgentStatus::Busy, "Working on issue"),
                    (AgentType::Human, AgentStatus::Online, "Reviewing PRs"),
                    (AgentType::Sovereign, AgentStatus::Idle, "Waiting for work"),
                ];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 80.0;
                for (agent_type, status, desc) in &combined {
                    // Badge
                    let mut badge = AgentStatusBadge::new(*status).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);

                    // Description
                    let desc_run = cx.text.layout(
                        *desc,
                        Point::new(x, row_y + 28.0),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(desc_run);
                    x += 140.0;
                }
            },
        );
        y += agent_height + SECTION_GAP;

        // ========== Panel 6: Trajectory Status Badges ==========
        let traj_height = panel_height(160.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Status Badges", traj_bounds, cx, |inner, cx| {
            let statuses = [
                TrajectoryStatus::Verified,
                TrajectoryStatus::Partial,
                TrajectoryStatus::HasGaps,
                TrajectoryStatus::Suspicious,
                TrajectoryStatus::Mismatch,
                TrajectoryStatus::Unknown,
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Full badge
                let mut badge = TrajectoryStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 95.0, 22.0), cx);

                // Compact
                let mut compact = TrajectoryStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x + tile_w - 28.0, tile_y, 24.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 7: Complete GitAfter Dashboard ==========
        let dashboard_height = panel_height(360.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "GitAfter Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let title_run = cx.text.layout(
                    "openagents/openagents",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                // Issue row example
                let issue_y = inner.origin.y + 52.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, issue_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Issue status
                let mut issue_status = IssueStatusBadge::new(IssueStatus::Open);
                issue_status.paint(
                    Bounds::new(inner.origin.x + 8.0, issue_y + 17.0, 60.0, 22.0),
                    cx,
                );

                // Issue title
                let issue_title = cx.text.layout(
                    "#42: Add NIP-SA trajectory publishing",
                    Point::new(inner.origin.x + 76.0, issue_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(issue_title);

                // Bounty
                let mut bounty = BountyBadge::new(50000).status(BountyStatus::Active);
                bounty.paint(
                    Bounds::new(inner.origin.x + 76.0, issue_y + 28.0, 90.0, 22.0),
                    cx,
                );

                // Agent claimant
                let claimed_run = cx.text.layout(
                    "Claimed by npub1agent...",
                    Point::new(inner.origin.x + 180.0, issue_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(claimed_run);

                // PR row example
                let pr_y = issue_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, pr_y, inner.size.width, 72.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // PR status
                let mut pr_status = PrStatusBadge::new(PrStatus::Open);
                pr_status.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 8.0, 60.0, 22.0),
                    cx,
                );

                // Stack layer
                let mut stack_layer = StackLayerBadge::new(2, 4).status(StackLayerStatus::Ready);
                stack_layer.paint(
                    Bounds::new(inner.origin.x + 76.0, pr_y + 8.0, 80.0, 24.0),
                    cx,
                );

                // PR title
                let pr_title = cx.text.layout(
                    "Layer 2: Wire trajectory events to relay pool",
                    Point::new(inner.origin.x + 164.0, pr_y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(pr_title);

                // Agent author + trajectory
                let mut agent =
                    AgentStatusBadge::new(AgentStatus::Busy).agent_type(AgentType::Sovereign);
                agent.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 38.0, 100.0, 24.0),
                    cx,
                );

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(
                    Bounds::new(inner.origin.x + 116.0, pr_y + 40.0, 80.0, 22.0),
                    cx,
                );

                // "depends on layer 1" indicator
                let depends_run = cx.text.layout(
                    "Depends on: Layer 1 (merged)",
                    Point::new(inner.origin.x + 210.0, pr_y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(depends_run);
            },
        );
    }
}
