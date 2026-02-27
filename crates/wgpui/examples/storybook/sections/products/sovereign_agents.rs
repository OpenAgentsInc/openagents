use super::*;

impl Storybook {
    pub(crate) fn paint_sovereign_agents(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Threshold Key Badges ==========
        let threshold_height = panel_height(160.0);
        let threshold_bounds = Bounds::new(bounds.origin.x, y, width, threshold_height);
        draw_panel("Threshold Key Badges", threshold_bounds, cx, |inner, cx| {
            let configs = [
                (2, 3, 2, "2-of-3 (ready)"),
                (2, 3, 1, "2-of-3 (partial)"),
                (3, 5, 3, "3-of-5 (ready)"),
                (3, 5, 2, "3-of-5 (partial)"),
                (2, 3, 0, "2-of-3 (unknown)"),
            ];

            let tile_w = 130.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (threshold, total, available, label)) in configs.iter().enumerate() {
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

                // Full badge
                let mut badge =
                    ThresholdKeyBadge::new(*threshold, *total).shares_available(*available);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 24.0), cx);
            }
        });
        y += threshold_height + SECTION_GAP;

        // ========== Panel 2: Agent Schedule Badges ==========
        let schedule_height = panel_height(180.0);
        let schedule_bounds = Bounds::new(bounds.origin.x, y, width, schedule_height);
        draw_panel("Agent Schedule Badges", schedule_bounds, cx, |inner, cx| {
            let schedules = [
                (
                    900,
                    vec![TriggerType::Mention, TriggerType::DirectMessage],
                    "15m + mentions/DMs",
                ),
                (
                    3600,
                    vec![TriggerType::Zap, TriggerType::Issue],
                    "1h + zaps/issues",
                ),
                (7200, vec![TriggerType::PullRequest], "2h + PRs"),
                (300, vec![], "5m heartbeat only"),
            ];

            let tile_w = 160.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (seconds, triggers, label)) in schedules.iter().enumerate() {
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

                // Full badge
                let mut badge = AgentScheduleBadge::new(*seconds).triggers(triggers.clone());
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 24.0), cx);
            }
        });
        y += schedule_height + SECTION_GAP;

        // ========== Panel 3: Goal Progress Badges ==========
        let goal_height = panel_height(160.0);
        let goal_bounds = Bounds::new(bounds.origin.x, y, width, goal_height);
        draw_panel("Goal Progress Badges", goal_bounds, cx, |inner, cx| {
            let goals = [
                (
                    0.0,
                    GoalStatus::NotStarted,
                    GoalPriority::Medium,
                    "Not started",
                ),
                (
                    0.35,
                    GoalStatus::InProgress,
                    GoalPriority::High,
                    "In progress",
                ),
                (
                    0.65,
                    GoalStatus::InProgress,
                    GoalPriority::Critical,
                    "Critical",
                ),
                (
                    1.0,
                    GoalStatus::Completed,
                    GoalPriority::Medium,
                    "Completed",
                ),
                (0.5, GoalStatus::Blocked, GoalPriority::High, "Blocked"),
                (0.8, GoalStatus::Failed, GoalPriority::Critical, "Failed"),
            ];

            let tile_w = 140.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (progress, status, priority, label)) in goals.iter().enumerate() {
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
                let mut badge = GoalProgressBadge::new(*progress)
                    .status(*status)
                    .priority(*priority);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 125.0, 22.0), cx);
            }
        });
        y += goal_height + SECTION_GAP;

        // ========== Panel 4: Tick Event Badges ==========
        let tick_height = panel_height(180.0);
        let tick_bounds = Bounds::new(bounds.origin.x, y, width, tick_height);
        draw_panel("Tick Event Badges", tick_bounds, cx, |inner, cx| {
            // Row 1: Tick outcomes
            let outcomes = [
                TickOutcome::Pending,
                TickOutcome::Success,
                TickOutcome::Failure,
                TickOutcome::Timeout,
                TickOutcome::Skipped,
            ];

            let mut x = inner.origin.x;
            for outcome in &outcomes {
                let mut badge = TickEventBadge::result(*outcome).duration_ms(1500);
                badge.paint(Bounds::new(x, inner.origin.y, 110.0, 22.0), cx);
                x += 120.0;
            }

            // Row 2: Request vs Result
            let row_y = inner.origin.y + 40.0;
            let req_label = cx.text.layout(
                "Request",
                Point::new(inner.origin.x, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(req_label);

            let mut request = TickEventBadge::request();
            request.paint(Bounds::new(inner.origin.x, row_y + 18.0, 80.0, 22.0), cx);

            let res_label = cx.text.layout(
                "Result (success, 2.3s)",
                Point::new(inner.origin.x + 120.0, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(res_label);

            let mut result = TickEventBadge::result(TickOutcome::Success).duration_ms(2300);
            result.paint(
                Bounds::new(inner.origin.x + 120.0, row_y + 18.0, 130.0, 22.0),
                cx,
            );

            // Compact versions
            let compact_y = row_y + 50.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, compact_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for outcome in &outcomes {
                let mut compact = TickEventBadge::result(*outcome).compact(true);
                compact.paint(Bounds::new(x, compact_y, 28.0, 22.0), cx);
                x += 36.0;
            }
        });
        y += tick_height + SECTION_GAP;

        // ========== Panel 5: Skill License Badges ==========
        let skill_height = panel_height(180.0);
        let skill_bounds = Bounds::new(bounds.origin.x, y, width, skill_height);
        draw_panel("Skill License Badges", skill_bounds, cx, |inner, cx| {
            let skills = [
                (SkillType::Code, LicenseStatus::Active, Some("git-rebase")),
                (SkillType::Data, LicenseStatus::Active, Some("market-data")),
                (SkillType::Model, LicenseStatus::Pending, Some("sonnet-4.5")),
                (SkillType::Tool, LicenseStatus::Expired, Some("browser-use")),
                (SkillType::Code, LicenseStatus::Revoked, None),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (skill_type, status, name)) in skills.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Type label
                let type_label = match skill_type {
                    SkillType::Code => "Code Skill",
                    SkillType::Data => "Data Skill",
                    SkillType::Model => "Model Skill",
                    SkillType::Tool => "Tool Skill",
                };
                let label_run = cx.text.layout(
                    type_label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = SkillLicenseBadge::new(*skill_type, *status);
                if let Some(n) = name {
                    badge = badge.name(*n);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += skill_height + SECTION_GAP;

        // ========== Panel 6: Complete Agent Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Sovereign Agent Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar with agent identity
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

                // Agent icon + name
                let agent_icon = cx.text.layout(
                    "🤖",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 14.0),
                    theme::font_size::LG,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_icon);

                let agent_name = cx.text.layout(
                    "code-monkey-42",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_name);

                let npub = cx.text.layout(
                    "npub1agent42xyz...",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 28.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(npub);

                // Status badges on right side of header
                let mut status =
                    AgentStatusBadge::new(AgentStatus::Online).agent_type(AgentType::Sovereign);
                status.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 200.0,
                        inner.origin.y + 13.0,
                        100.0,
                        24.0,
                    ),
                    cx,
                );

                let mut threshold = ThresholdKeyBadge::new(2, 3).shares_available(2);
                threshold.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 13.0,
                        80.0,
                        24.0,
                    ),
                    cx,
                );

                // Schedule row
                let sched_y = inner.origin.y + 60.0;
                let sched_label = cx.text.layout(
                    "Schedule:",
                    Point::new(inner.origin.x + 8.0, sched_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(sched_label);

                let mut schedule = AgentScheduleBadge::new(900).triggers(vec![
                    TriggerType::Mention,
                    TriggerType::Zap,
                    TriggerType::Issue,
                ]);
                schedule.paint(Bounds::new(inner.origin.x + 70.0, sched_y, 140.0, 24.0), cx);

                // Goals section
                let goals_y = sched_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, goals_y, inner.size.width, 90.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let goals_title = cx.text.layout(
                    "Current Goals",
                    Point::new(inner.origin.x + 8.0, goals_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goals_title);

                // Goal 1
                let mut goal1 = GoalProgressBadge::new(0.75)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::High);
                goal1.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 28.0, 125.0, 22.0),
                    cx,
                );
                let goal1_desc = cx.text.layout(
                    "Fix d-006 Phase 4 issues",
                    Point::new(inner.origin.x + 142.0, goals_y + 32.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal1_desc);

                // Goal 2
                let mut goal2 = GoalProgressBadge::new(0.3)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::Medium);
                goal2.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 56.0, 125.0, 22.0),
                    cx,
                );
                let goal2_desc = cx.text.layout(
                    "Publish trajectory events",
                    Point::new(inner.origin.x + 142.0, goals_y + 60.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal2_desc);

                // Skills section
                let skills_y = goals_y + 100.0;
                let skills_label = cx.text.layout(
                    "Licensed Skills:",
                    Point::new(inner.origin.x + 8.0, skills_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(skills_label);

                let mut skill1 =
                    SkillLicenseBadge::new(SkillType::Code, LicenseStatus::Active).name("git-ops");
                skill1.paint(
                    Bounds::new(inner.origin.x + 100.0, skills_y, 120.0, 22.0),
                    cx,
                );

                let mut skill2 = SkillLicenseBadge::new(SkillType::Model, LicenseStatus::Active)
                    .name("opus-4.5");
                skill2.paint(
                    Bounds::new(inner.origin.x + 230.0, skills_y, 130.0, 22.0),
                    cx,
                );

                // Recent ticks section
                let ticks_y = skills_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        ticks_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let ticks_title = cx.text.layout(
                    "Recent Ticks",
                    Point::new(inner.origin.x + 8.0, ticks_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(ticks_title);

                // Tick timeline
                let tick_row_y = ticks_y + 30.0;
                let times = ["2m ago", "17m ago", "32m ago", "47m ago"];
                let outcomes = [
                    TickOutcome::Success,
                    TickOutcome::Success,
                    TickOutcome::Failure,
                    TickOutcome::Success,
                ];
                let durations = [1200, 890, 0, 2300];

                for (i, ((time, outcome), dur)) in times
                    .iter()
                    .zip(outcomes.iter())
                    .zip(durations.iter())
                    .enumerate()
                {
                    let tick_x = inner.origin.x + 8.0 + i as f32 * 100.0;

                    let time_run = cx.text.layout(
                        *time,
                        Point::new(tick_x, tick_row_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(time_run);

                    let mut tick = if *dur > 0 {
                        TickEventBadge::result(*outcome).duration_ms(*dur as u64)
                    } else {
                        TickEventBadge::result(*outcome)
                    };
                    tick.paint(Bounds::new(tick_x, tick_row_y + 16.0, 90.0, 22.0), cx);
                }

                // Trajectory hash
                let traj_y = ticks_y + 72.0;
                let traj_label = cx.text.layout(
                    "Current trajectory:",
                    Point::new(inner.origin.x + 8.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(traj_label);

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(Bounds::new(inner.origin.x + 120.0, traj_y, 80.0, 22.0), cx);

                let hash = cx.text.layout(
                    "hash: 7c6267e85a...",
                    Point::new(inner.origin.x + 210.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(hash);
            },
        );
    }
}
