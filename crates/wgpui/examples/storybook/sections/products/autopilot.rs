use super::*;

impl Storybook {
    pub(crate) fn paint_autopilot(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Session Status Badges ==========
        let session_height = panel_height(180.0);
        let session_bounds = Bounds::new(bounds.origin.x, y, width, session_height);
        draw_panel("Session Status Badges", session_bounds, cx, |inner, cx| {
            let statuses = [
                (SessionStatus::Pending, None, None, "Pending"),
                (SessionStatus::Running, Some(125), Some(8), "Running"),
                (SessionStatus::Paused, Some(340), Some(12), "Paused"),
                (SessionStatus::Completed, Some(1800), Some(45), "Completed"),
                (SessionStatus::Failed, Some(65), Some(3), "Failed"),
                (SessionStatus::Aborted, Some(200), Some(5), "Aborted"),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, duration, tasks, label)) in statuses.iter().enumerate() {
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
                let mut badge = SessionStatusBadge::new(*status);
                if let Some(secs) = duration {
                    badge = badge.duration(*secs);
                }
                if let Some(count) = tasks {
                    badge = badge.task_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += session_height + SECTION_GAP;

        // ========== Panel 2: APM Gauges ==========
        let apm_height = panel_height(160.0);
        let apm_bounds = Bounds::new(bounds.origin.x, y, width, apm_height);
        draw_panel(
            "APM (Actions Per Minute) Gauges",
            apm_bounds,
            cx,
            |inner, cx| {
                let apms = [
                    (0.0, "Idle"),
                    (5.0, "Low"),
                    (22.0, "Normal"),
                    (45.0, "High"),
                    (80.0, "Intense"),
                ];

                let tile_w = 160.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (apm, label)) in apms.iter().enumerate() {
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

                    // Gauge
                    let mut gauge = ApmGauge::new(*apm);
                    gauge.paint(Bounds::new(tile_x, tile_y + 18.0, 150.0, 22.0), cx);
                }
            },
        );
        y += apm_height + SECTION_GAP;

        // ========== Panel 3: Resource Usage Bars ==========
        let resource_height = panel_height(180.0);
        let resource_bounds = Bounds::new(bounds.origin.x, y, width, resource_height);
        draw_panel("Resource Usage Bars", resource_bounds, cx, |inner, cx| {
            let resources = [
                (ResourceType::Memory, 35.0, "Normal Memory (35%)"),
                (ResourceType::Memory, 65.0, "Warning Memory (65%)"),
                (ResourceType::Memory, 92.0, "Critical Memory (92%)"),
                (ResourceType::Cpu, 28.0, "Normal CPU (28%)"),
                (ResourceType::Cpu, 75.0, "Warning CPU (75%)"),
                (ResourceType::Cpu, 95.0, "Critical CPU (95%)"),
            ];

            let tile_w = 200.0;
            let tile_h = 50.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (rtype, pct, label)) in resources.iter().enumerate() {
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

                // Bar
                let mut bar = ResourceUsageBar::new(*rtype, *pct);
                bar.paint(Bounds::new(tile_x, tile_y + 18.0, 180.0, 22.0), cx);
            }
        });
        y += resource_height + SECTION_GAP;

        // ========== Panel 4: Daemon Status Badges ==========
        let daemon_height = panel_height(160.0);
        let daemon_bounds = Bounds::new(bounds.origin.x, y, width, daemon_height);
        draw_panel("Daemon Status Badges", daemon_bounds, cx, |inner, cx| {
            let statuses = [
                (DaemonStatus::Offline, None, None, "Offline"),
                (DaemonStatus::Starting, None, None, "Starting"),
                (
                    DaemonStatus::Online,
                    Some(86400),
                    Some(3),
                    "Online (1d, 3 workers)",
                ),
                (DaemonStatus::Restarting, None, None, "Restarting"),
                (DaemonStatus::Error, None, None, "Error"),
                (DaemonStatus::Stopping, None, None, "Stopping"),
            ];

            let tile_w = 170.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, uptime, workers, label)) in statuses.iter().enumerate() {
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
                let mut badge = DaemonStatusBadge::new(*status);
                if let Some(secs) = uptime {
                    badge = badge.uptime(*secs);
                }
                if let Some(count) = workers {
                    badge = badge.worker_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 160.0, 22.0), cx);
            }
        });
        y += daemon_height + SECTION_GAP;

        // ========== Panel 5: Parallel Agent Badges ==========
        let parallel_height = panel_height(180.0);
        let parallel_bounds = Bounds::new(bounds.origin.x, y, width, parallel_height);
        draw_panel("Parallel Agent Badges", parallel_bounds, cx, |inner, cx| {
            let agents = [
                (0, ParallelAgentStatus::Idle, None, "Agent 0: Idle"),
                (
                    1,
                    ParallelAgentStatus::Running,
                    Some("Building tests"),
                    "Agent 1: Running",
                ),
                (
                    2,
                    ParallelAgentStatus::Waiting,
                    Some("Awaiting input"),
                    "Agent 2: Waiting",
                ),
                (3, ParallelAgentStatus::Completed, None, "Agent 3: Done"),
                (
                    4,
                    ParallelAgentStatus::Failed,
                    Some("Build error"),
                    "Agent 4: Failed",
                ),
                (5, ParallelAgentStatus::Initializing, None, "Agent 5: Init"),
            ];

            let tile_w = 220.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (agent_idx, status, task, label)) in agents.iter().enumerate() {
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
                let mut badge = ParallelAgentBadge::new(*agent_idx, *status);
                if let Some(t) = task {
                    badge = badge.current_task(*t);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 200.0, 22.0), cx);
            }
        });
        y += parallel_height + SECTION_GAP;

        // ========== Panel 6: Complete Autopilot Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Autopilot Dashboard Preview",
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
                    "Autopilot Control",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Daemon status on right
                let mut daemon = DaemonStatusBadge::new(DaemonStatus::Online)
                    .uptime(86400)
                    .worker_count(3);
                daemon.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 180.0,
                        inner.origin.y + 10.0,
                        170.0,
                        22.0,
                    ),
                    cx,
                );

                // APM gauge
                let mut apm = ApmGauge::new(28.5);
                apm.paint(
                    Bounds::new(inner.origin.x + 12.0, inner.origin.y + 32.0, 140.0, 22.0),
                    cx,
                );

                // Active session row
                let session_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        session_y,
                        inner.size.width,
                        56.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                // Session info
                let session_title = cx.text.layout(
                    "Active Session #1234",
                    Point::new(inner.origin.x + 8.0, session_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(session_title);

                let mut session = SessionStatusBadge::new(SessionStatus::Running)
                    .duration(325)
                    .task_count(12);
                session.paint(
                    Bounds::new(inner.origin.x + 160.0, session_y + 6.0, 200.0, 22.0),
                    cx,
                );

                // Task info
                let task_info = cx.text.layout(
                    "Current: Building component tests",
                    Point::new(inner.origin.x + 8.0, session_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(task_info);

                // Parallel agents section
                let agents_y = session_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        agents_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let agents_label = cx.text.layout(
                    "Parallel Agents",
                    Point::new(inner.origin.x + 8.0, agents_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agents_label);

                // Agent badges in a row
                let mut x = inner.origin.x + 8.0;
                for (idx, status) in [
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Waiting,
                ]
                .iter()
                .enumerate()
                {
                    let mut agent = ParallelAgentBadge::new(idx as u8, *status).compact(true);
                    agent.paint(Bounds::new(x, agents_y + 32.0, 50.0, 22.0), cx);
                    x += 60.0;
                }

                // Resource bars
                let res_y = agents_y + 60.0;
                let mut mem = ResourceUsageBar::new(ResourceType::Memory, 45.0).bar_width(80.0);
                mem.paint(Bounds::new(inner.origin.x + 8.0, res_y, 160.0, 22.0), cx);

                let mut cpu = ResourceUsageBar::new(ResourceType::Cpu, 62.0).bar_width(80.0);
                cpu.paint(Bounds::new(inner.origin.x + 180.0, res_y, 160.0, 22.0), cx);

                // Session history section
                let history_y = agents_y + 112.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        history_y,
                        inner.size.width,
                        80.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let history_label = cx.text.layout(
                    "Recent Sessions",
                    Point::new(inner.origin.x + 8.0, history_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(history_label);

                // Completed sessions
                let mut completed1 = SessionStatusBadge::new(SessionStatus::Completed)
                    .duration(1800)
                    .task_count(45)
                    .compact(true);
                completed1.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 32.0, 28.0, 22.0),
                    cx,
                );
                let c1_label = cx.text.layout(
                    "#1233 - 45 tasks",
                    Point::new(inner.origin.x + 42.0, history_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c1_label);

                let mut completed2 = SessionStatusBadge::new(SessionStatus::Failed).compact(true);
                completed2.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 56.0, 28.0, 22.0),
                    cx,
                );
                let c2_label = cx.text.layout(
                    "#1232 - Build error",
                    Point::new(inner.origin.x + 42.0, history_y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c2_label);
            },
        );
    }
}
