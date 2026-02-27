use super::*;

impl Storybook {
    pub(crate) fn paint_marketplace(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Market Type Badges ==========
        let market_height = panel_height(140.0);
        let market_bounds = Bounds::new(bounds.origin.x, y, width, market_height);
        draw_panel("Market Type Badges", market_bounds, cx, |inner, cx| {
            let types = [
                MarketType::Compute,
                MarketType::Skills,
                MarketType::Data,
                MarketType::Trajectories,
            ];

            let mut x = inner.origin.x;
            for market_type in &types {
                // Full badge
                let mut badge = MarketTypeBadge::new(*market_type);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);

                // Compact badge
                let mut compact = MarketTypeBadge::new(*market_type).compact(true);
                compact.paint(Bounds::new(x, inner.origin.y + 30.0, 28.0, 22.0), cx);

                x += 100.0;
            }
        });
        y += market_height + SECTION_GAP;

        // ========== Panel 2: Job Status Badges ==========
        let job_height = panel_height(180.0);
        let job_bounds = Bounds::new(bounds.origin.x, y, width, job_height);
        draw_panel(
            "Job Status Badges (NIP-90 DVM)",
            job_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    (JobStatus::Pending, None, "Pending"),
                    (JobStatus::Processing, None, "Processing"),
                    (JobStatus::Streaming, None, "Streaming"),
                    (JobStatus::Completed, Some(150), "Completed"),
                    (JobStatus::Failed, None, "Failed"),
                    (JobStatus::Cancelled, None, "Cancelled"),
                ];

                let tile_w = 110.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (status, cost, label)) in statuses.iter().enumerate() {
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

                    // Badge
                    let mut badge = JobStatusBadge::new(*status);
                    if let Some(sats) = cost {
                        badge = badge.cost_sats(*sats);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 22.0), cx);
                }
            },
        );
        y += job_height + SECTION_GAP;

        // ========== Panel 3: Reputation Badges ==========
        let rep_height = panel_height(160.0);
        let rep_bounds = Bounds::new(bounds.origin.x, y, width, rep_height);
        draw_panel(
            "Reputation & Trust Tier Badges",
            rep_bounds,
            cx,
            |inner, cx| {
                let tiers = [
                    (TrustTier::New, None, "New provider"),
                    (TrustTier::Established, Some(0.85), "Established"),
                    (TrustTier::Trusted, Some(0.95), "Trusted"),
                    (TrustTier::Expert, Some(0.99), "Expert"),
                ];

                let tile_w = 130.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (tier, rate, label)) in tiers.iter().enumerate() {
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

                    // Badge
                    let mut badge = ReputationBadge::new(*tier);
                    if let Some(r) = rate {
                        badge = badge.success_rate(*r);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 22.0), cx);
                }
            },
        );
        y += rep_height + SECTION_GAP;

        // ========== Panel 4: Trajectory Source Badges ==========
        let traj_height = panel_height(180.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Source Badges", traj_bounds, cx, |inner, cx| {
            let sources = [
                (
                    TrajectorySource::Codex,
                    Some(ContributionStatus::Accepted),
                    Some(42),
                ),
                (
                    TrajectorySource::Cursor,
                    Some(ContributionStatus::Pending),
                    Some(15),
                ),
                (
                    TrajectorySource::Windsurf,
                    Some(ContributionStatus::Redacted),
                    Some(23),
                ),
                (TrajectorySource::Custom, None, None),
            ];

            let tile_w = 180.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (source, status, count)) in sources.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Source label
                let label_run = cx.text.layout(
                    source.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = TrajectorySourceBadge::new(*source);
                if let Some(s) = status {
                    badge = badge.status(*s);
                }
                if let Some(c) = count {
                    badge = badge.session_count(*c);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 170.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 5: Earnings Badges ==========
        let earn_height = panel_height(180.0);
        let earn_bounds = Bounds::new(bounds.origin.x, y, width, earn_height);
        draw_panel("Earnings Badges", earn_bounds, cx, |inner, cx| {
            let earnings = [
                (EarningsType::Total, 1_250_000),
                (EarningsType::Compute, 500_000),
                (EarningsType::Skills, 350_000),
                (EarningsType::Data, 250_000),
                (EarningsType::Trajectories, 150_000),
            ];

            // Row 1: Full earnings badges
            let mut x = inner.origin.x;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount);
                badge.paint(Bounds::new(x, inner.origin.y, 160.0, 22.0), cx);
                x += 170.0;
                if x > inner.origin.x + inner.size.width - 100.0 {
                    break;
                }
            }

            // Row 2: Compact versions
            let row_y = inner.origin.y + 40.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, row_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount).compact(true);
                badge.paint(Bounds::new(x, row_y, 70.0, 22.0), cx);
                x += 80.0;
            }
        });
        y += earn_height + SECTION_GAP;

        // ========== Panel 6: Complete Marketplace Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Marketplace Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Title
                let title = cx.text.layout(
                    "Unified Marketplace",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Market type tabs
                let mut x = inner.origin.x + 12.0;
                let tab_y = inner.origin.y + 28.0;
                for market_type in &[
                    MarketType::Compute,
                    MarketType::Skills,
                    MarketType::Data,
                    MarketType::Trajectories,
                ] {
                    let mut badge = MarketTypeBadge::new(*market_type);
                    badge.paint(Bounds::new(x, tab_y, 80.0, 20.0), cx);
                    x += 90.0;
                }

                // Earnings summary on right
                let mut total_earn =
                    EarningsBadge::new(EarningsType::Total, 1_250_000).compact(true);
                total_earn.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 14.0,
                        80.0,
                        22.0,
                    ),
                    cx,
                );

                // Provider row
                let prov_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, prov_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Provider name + reputation
                let prov_name = cx.text.layout(
                    "compute-provider-1",
                    Point::new(inner.origin.x + 8.0, prov_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(prov_name);

                let mut rep = ReputationBadge::new(TrustTier::Trusted).success_rate(0.97);
                rep.paint(
                    Bounds::new(inner.origin.x + 140.0, prov_y + 6.0, 100.0, 22.0),
                    cx,
                );

                // Job in progress
                let mut job = JobStatusBadge::new(JobStatus::Processing);
                job.paint(
                    Bounds::new(inner.origin.x + 8.0, prov_y + 32.0, 90.0, 22.0),
                    cx,
                );

                let job_info = cx.text.layout(
                    "llama3 • 1.2K tokens",
                    Point::new(inner.origin.x + 106.0, prov_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(job_info);

                // Trajectory contribution section
                let traj_y = prov_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, traj_y, inner.size.width, 100.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let traj_title = cx.text.layout(
                    "Trajectory Contributions",
                    Point::new(inner.origin.x + 8.0, traj_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(traj_title);

                // Source badges row
                let source_y = traj_y + 28.0;
                let mut x = inner.origin.x + 8.0;
                let sources = [
                    (TrajectorySource::Codex, ContributionStatus::Accepted, 42),
                    (TrajectorySource::Cursor, ContributionStatus::Pending, 15),
                ];
                for (source, status, count) in &sources {
                    let mut badge = TrajectorySourceBadge::new(*source)
                        .status(*status)
                        .session_count(*count);
                    badge.paint(Bounds::new(x, source_y, 170.0, 22.0), cx);
                    x += 180.0;
                }

                // Earnings row
                let earn_y = traj_y + 56.0;
                let earn_label = cx.text.layout(
                    "Trajectory earnings:",
                    Point::new(inner.origin.x + 8.0, earn_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(earn_label);

                let mut traj_earn = EarningsBadge::new(EarningsType::Trajectories, 150_000);
                traj_earn.paint(Bounds::new(inner.origin.x + 120.0, earn_y, 160.0, 22.0), cx);

                // Total earnings bar at bottom
                let total_y = traj_y + 80.0;
                let total_label = cx.text.layout(
                    "Total today:",
                    Point::new(inner.origin.x + 8.0, total_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(total_label);

                let mut today_earn = EarningsBadge::new(EarningsType::Total, 25_000);
                today_earn.paint(Bounds::new(inner.origin.x + 80.0, total_y, 150.0, 22.0), cx);
            },
        );
    }
}
