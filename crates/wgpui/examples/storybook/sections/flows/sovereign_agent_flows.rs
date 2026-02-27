use super::*;

impl Storybook {
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
