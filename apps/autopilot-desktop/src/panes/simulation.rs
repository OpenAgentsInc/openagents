use wgpui::{Bounds, PaintContext, Point, Quad, curve::CurvePrimitive, theme};

use crate::app_state::{
    AgentNetworkSimulationEvent, AgentNetworkSimulationPaneState, RelaySecuritySimulationPaneState,
    StableSatsSimulationMode, StableSatsSimulationPaneState, StableSatsTransferAsset,
    StableSatsWalletMode, TreasuryExchangeSimulationPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    agent_network_simulation_reset_button_bounds, agent_network_simulation_run_button_bounds,
    relay_security_simulation_reset_button_bounds, relay_security_simulation_run_button_bounds,
    stable_sats_simulation_mode_demo_button_bounds, stable_sats_simulation_mode_real_button_bounds,
    stable_sats_simulation_reset_button_bounds, stable_sats_simulation_run_button_bounds,
    treasury_exchange_simulation_reset_button_bounds,
    treasury_exchange_simulation_run_button_bounds,
};

pub fn paint_agent_network_simulation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &AgentNetworkSimulationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "sim+nip28", paint);

    let run_bounds = agent_network_simulation_run_button_bounds(content_bounds);
    let reset_bounds = agent_network_simulation_reset_button_bounds(content_bounds);
    let run_label = if pane_state.auto_run_enabled {
        "Pause Auto Run"
    } else {
        "Start Auto Run"
    };
    paint_action_button(run_bounds, run_label, paint);
    paint_action_button(reset_bounds, "Reset Simulation", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        run_bounds.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "NIP-28 channel",
        pane_state
            .channel_event_id
            .as_deref()
            .unwrap_or("not-created"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Channel name",
        &pane_state.channel_name,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rounds run",
        &pane_state.rounds_run.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Transferred sats",
        &pane_state.total_transferred_sats.to_string(),
    );

    let skills = if pane_state.learned_skills.is_empty() {
        "none".to_string()
    } else {
        pane_state.learned_skills.join(", ")
    };
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Learned skills",
        &skills,
    );

    paint_simulation_timeline(content_bounds, y, &pane_state.events, paint);
}

pub fn paint_treasury_exchange_simulation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &TreasuryExchangeSimulationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "sim+nip69", paint);

    let run_bounds = treasury_exchange_simulation_run_button_bounds(content_bounds);
    let reset_bounds = treasury_exchange_simulation_reset_button_bounds(content_bounds);
    let run_label = if pane_state.auto_run_enabled {
        "Pause Auto Run"
    } else {
        "Start Auto Run"
    };
    paint_action_button(run_bounds, run_label, paint);
    paint_action_button(reset_bounds, "Reset Simulation", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        run_bounds.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rounds run",
        &pane_state.rounds_run.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Order event",
        pane_state
            .order_event_id
            .as_deref()
            .unwrap_or("not-created"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Mint",
        pane_state.mint_reference.as_deref().unwrap_or("unknown"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Trade volume (sats)",
        &pane_state.trade_volume_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Total liquidity (sats)",
        &pane_state.total_liquidity_sats.to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Wallet connect",
        pane_state
            .wallet_connect_url
            .as_deref()
            .unwrap_or("not-initialized"),
    );

    paint_simulation_timeline(content_bounds, y, &pane_state.events, paint);
}

pub fn paint_relay_security_simulation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &RelaySecuritySimulationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "sim+nip42", paint);

    let run_bounds = relay_security_simulation_run_button_bounds(content_bounds);
    let reset_bounds = relay_security_simulation_reset_button_bounds(content_bounds);
    let run_label = if pane_state.auto_run_enabled {
        "Pause Auto Run"
    } else {
        "Start Auto Run"
    };
    paint_action_button(run_bounds, run_label, paint);
    paint_action_button(reset_bounds, "Reset Simulation", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        run_bounds.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Relay URL",
        &pane_state.relay_url,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Latest challenge",
        &pane_state.challenge,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Auth event",
        pane_state.auth_event_id.as_deref().unwrap_or("not-created"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "DM relays",
        &pane_state.dm_relay_count.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Negentropy ranges",
        &pane_state.sync_ranges.to_string(),
    );

    paint_simulation_timeline(content_bounds, y, &pane_state.events, paint);
}

pub fn paint_stable_sats_simulation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &StableSatsSimulationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "sim+blink", paint);

    let run_bounds = stable_sats_simulation_run_button_bounds(content_bounds);
    let reset_bounds = stable_sats_simulation_reset_button_bounds(content_bounds);
    let demo_mode_bounds = stable_sats_simulation_mode_demo_button_bounds(content_bounds);
    let real_mode_bounds = stable_sats_simulation_mode_real_button_bounds(content_bounds);
    let run_label = if pane_state.mode == StableSatsSimulationMode::RealBlink {
        if pane_state.live_refresh_pending {
            "Refreshing..."
        } else {
            "Refresh Live"
        }
    } else if pane_state.auto_run_enabled {
        "Pause Auto Run"
    } else {
        "Start Auto Run"
    };
    paint_action_button(run_bounds, run_label, paint);
    paint_action_button(reset_bounds, "Reset Simulation", paint);
    paint_mode_radio_toggle(
        demo_mode_bounds,
        "Demo",
        pane_state.mode == StableSatsSimulationMode::Demo,
        paint,
    );
    paint_mode_radio_toggle(
        real_mode_bounds,
        "Real",
        pane_state.mode == StableSatsSimulationMode::RealBlink,
        paint,
    );

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        run_bounds
            .max_y()
            .max(reset_bounds.max_y())
            .max(demo_mode_bounds.max_y())
            .max(real_mode_bounds.max_y())
            + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Mode",
        pane_state.mode.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rounds run",
        &pane_state.rounds_run.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "BTC/USD quote",
        &format_usd_cents(pane_state.price_usd_cents_per_btc),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Converted sats",
        &pane_state.total_converted_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Converted USD",
        &format_usd_cents(pane_state.total_converted_usd_cents),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Settlement ref",
        pane_state
            .last_settlement_ref
            .as_deref()
            .unwrap_or("not-settled"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Aggregate BTC",
        &format!("{} sats", pane_state.total_btc_balance_sats()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Aggregate USD",
        &format_usd_cents(pane_state.total_usd_balance_cents()),
    );
    let pending_ops = pane_state
        .treasury_operations
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                crate::app_state::StableSatsTreasuryOperationStatus::Queued
                    | crate::app_state::StableSatsTreasuryOperationStatus::Running
            )
        })
        .count();
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Wallet nodes",
        &pane_state.agents.len().to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Treasury ops pending",
        &pending_ops.to_string(),
    );
    if pane_state.mode == StableSatsSimulationMode::RealBlink {
        let health = if pane_state.live_refresh_pending {
            format!(
                "refreshing request #{}",
                pane_state
                    .active_live_refresh_request_id
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "?".to_string())
            )
        } else if pane_state.last_error.is_some() {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        };
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Real mode health",
            &health,
        );
    }

    let node_graph_end_y =
        paint_stable_sats_agent_graph(content_bounds, y + 4.0, pane_state, paint);
    let transfer_strip_end_y =
        paint_stable_sats_transfer_strip(content_bounds, node_graph_end_y + 6.0, pane_state, paint);
    let graph_end_y = paint_stable_sats_graph(
        content_bounds,
        transfer_strip_end_y + 6.0,
        &pane_state.price_history_usd_cents_per_btc,
        &pane_state.converted_sats_history,
        paint,
    );

    paint_simulation_timeline(content_bounds, graph_end_y + 6.0, &pane_state.events, paint);
}

fn paint_mode_radio_toggle(bounds: Bounds, label: &str, selected: bool, paint: &mut PaintContext) {
    let border = if selected {
        theme::accent::PRIMARY
    } else {
        theme::border::DEFAULT
    };
    let background = if selected {
        theme::bg::HOVER
    } else {
        theme::bg::APP.with_alpha(0.78)
    };

    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(4.0),
    );

    let radio_diameter = 10.0;
    let radio_bounds = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + (bounds.size.height - radio_diameter) * 0.5,
        radio_diameter,
        radio_diameter,
    );
    paint.scene.draw_quad(
        Quad::new(radio_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(border, 1.0)
            .with_corner_radius(radio_diameter * 0.5),
    );
    if selected {
        let dot = Bounds::new(
            radio_bounds.origin.x + 2.5,
            radio_bounds.origin.y + 2.5,
            radio_diameter - 5.0,
            radio_diameter - 5.0,
        );
        paint.scene.draw_quad(
            Quad::new(dot)
                .with_background(theme::accent::PRIMARY)
                .with_corner_radius(dot.size.width * 0.5),
        );
    }

    let font_size = 10.0;
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(
            radio_bounds.max_x() + 6.0,
            bounds.origin.y + ((bounds.size.height - font_size) * 0.5) - 1.0,
        ),
        font_size,
        if selected {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
}

fn paint_stable_sats_agent_graph(
    content_bounds: Bounds,
    y: f32,
    pane_state: &StableSatsSimulationPaneState,
    paint: &mut PaintContext,
) -> f32 {
    let max_y = content_bounds.max_y() - 8.0;
    if y + 12.0 > max_y {
        return y;
    }

    paint.scene.draw_text(paint.text.layout(
        "Agent wallet topology",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));

    let graph_height = ((max_y - y) - 14.0).clamp(88.0, 170.0);
    if graph_height < 88.0 {
        return y + 12.0;
    }

    let graph_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        y + 14.0,
        (content_bounds.size.width - 24.0).clamp(200.0, 420.0),
        graph_height,
    );
    paint.scene.draw_quad(
        Quad::new(graph_bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(theme::border::DEFAULT.with_alpha(0.7), 1.0)
            .with_corner_radius(6.0),
    );

    if pane_state.agents.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No agents configured.",
            Point::new(graph_bounds.origin.x + 10.0, graph_bounds.origin.y + 22.0),
            10.0,
            theme::text::MUTED,
        ));
        return graph_bounds.max_y();
    }

    let agents: Vec<_> = pane_state.agents.iter().take(3).collect();
    let count = agents.len();
    let node_radius = if count <= 2 { 31.0 } else { 26.0 };
    let node_centers = stable_sats_node_centers(graph_bounds, count);

    let recent_transfers: Vec<_> = pane_state.transfer_ledger.iter().rev().take(10).collect();
    for (edge_rank, transfer) in recent_transfers.iter().rev().enumerate() {
        let from_anchor = stable_sats_wallet_anchor(transfer.from_wallet.as_str());
        let to_anchor = stable_sats_wallet_anchor(transfer.to_wallet.as_str());
        let Some(from_index) = agents
            .iter()
            .position(|agent| agent.agent_name == from_anchor)
        else {
            continue;
        };
        let Some(to_index) = agents
            .iter()
            .position(|agent| agent.agent_name == to_anchor)
        else {
            continue;
        };
        let from_center = node_centers[from_index];
        let to_center = node_centers[to_index];
        let stroke = match transfer.asset {
            StableSatsTransferAsset::BtcSats => theme::accent::PRIMARY.with_alpha(0.74),
            StableSatsTransferAsset::UsdCents => theme::status::SUCCESS.with_alpha(0.74),
        };
        if from_index == to_index {
            let loop_top = Point::new(from_center.x, from_center.y - node_radius - 3.0);
            let loop_end = Point::new(from_center.x + 0.1, from_center.y - node_radius - 1.0);
            let control_1 = Point::new(loop_top.x + 24.0, loop_top.y - 20.0);
            let control_2 = Point::new(loop_top.x - 24.0, loop_top.y - 20.0);
            paint.scene.draw_curve(
                CurvePrimitive::new(loop_top, control_1, control_2, loop_end)
                    .with_stroke_width(1.2)
                    .with_color(stroke),
            );
            continue;
        }

        let dx = to_center.x - from_center.x;
        let dy = to_center.y - from_center.y;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let ux = dx / len;
        let uy = dy / len;
        let start = Point::new(
            from_center.x + ux * (node_radius - 2.0),
            from_center.y + uy * (node_radius - 2.0),
        );
        let end = Point::new(
            to_center.x - ux * (node_radius - 2.0),
            to_center.y - uy * (node_radius - 2.0),
        );
        let perp_x = -uy;
        let perp_y = ux;
        let bend_sign = if edge_rank % 2 == 0 { 1.0 } else { -1.0 };
        let bend = (10.0 + (edge_rank as f32 * 1.4)).clamp(8.0, 22.0) * bend_sign;
        let control_1 = Point::new(
            start.x + dx * 0.34 + perp_x * bend,
            start.y + dy * 0.34 + perp_y * bend,
        );
        let control_2 = Point::new(
            start.x + dx * 0.72 + perp_x * bend,
            start.y + dy * 0.72 + perp_y * bend,
        );
        paint.scene.draw_curve(
            CurvePrimitive::new(start, control_1, control_2, end)
                .with_stroke_width(1.25)
                .with_color(stroke),
        );

        let marker_radius = 2.6;
        let marker = Bounds::new(
            end.x - marker_radius,
            end.y - marker_radius,
            marker_radius * 2.0,
            marker_radius * 2.0,
        );
        paint.scene.draw_quad(
            Quad::new(marker)
                .with_background(stroke)
                .with_corner_radius(marker_radius),
        );

        if edge_rank + 4 >= recent_transfers.len() {
            let label = format_transfer_edge_label(transfer);
            let label_x = (start.x + end.x) * 0.5 + perp_x * (bend * 0.28);
            let label_y = (start.y + end.y) * 0.5 + perp_y * (bend * 0.28);
            let label_width = paint.text.measure(label.as_str(), 8.0);
            paint.scene.draw_text(paint.text.layout_mono(
                label.as_str(),
                Point::new(label_x - (label_width * 0.5), label_y - 4.0),
                8.0,
                stroke.with_alpha(0.92),
            ));
        }
    }

    for (index, agent) in agents.iter().enumerate() {
        let node_center = node_centers[index];

        let node_fill = if agent.active_wallet == StableSatsWalletMode::Btc {
            theme::accent::PRIMARY.with_alpha(0.20)
        } else {
            theme::status::SUCCESS.with_alpha(0.20)
        };
        let node_border = if agent.active_wallet == StableSatsWalletMode::Btc {
            theme::accent::PRIMARY.with_alpha(0.8)
        } else {
            theme::status::SUCCESS.with_alpha(0.8)
        };
        let node_bounds = Bounds::new(
            node_center.x - node_radius,
            node_center.y - node_radius,
            node_radius * 2.0,
            node_radius * 2.0,
        );
        paint.scene.draw_quad(
            Quad::new(node_bounds)
                .with_background(node_fill)
                .with_border(node_border, 1.0)
                .with_corner_radius(node_radius),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                node_bounds.origin.x + 4.0,
                node_bounds.origin.y + 4.0,
                node_bounds.size.width - 8.0,
                node_bounds.size.height - 8.0,
            ))
            .with_background(theme::bg::SURFACE.with_alpha(0.28))
            .with_corner_radius((node_radius - 4.0).max(4.0)),
        );

        let agent_name = truncate_text(agent.agent_name.as_str(), 12);
        let btc_line = format!("{} sats", format_sats(agent.btc_balance_sats));
        let usd_line = format_usd_cents(agent.usd_balance_cents);

        let name_w = paint.text.measure(agent_name.as_str(), 9.0);
        let btc_w = paint.text.measure(btc_line.as_str(), 8.0);
        let usd_w = paint.text.measure(usd_line.as_str(), 8.0);

        paint.scene.draw_text(paint.text.layout(
            agent_name.as_str(),
            Point::new(node_center.x - (name_w * 0.5), node_center.y - 16.0),
            9.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            btc_line.as_str(),
            Point::new(node_center.x - (btc_w * 0.5), node_center.y - 2.0),
            8.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            usd_line.as_str(),
            Point::new(node_center.x - (usd_w * 0.5), node_center.y + 9.0),
            8.0,
            theme::text::MUTED,
        ));

        let badge = format!("{}  x{}", agent.active_wallet.label(), agent.switch_count);
        let badge_w = paint.text.measure(badge.as_str(), 8.0);
        let badge_y = (node_bounds.origin.y - 10.0).max(graph_bounds.origin.y + 2.0);
        paint.scene.draw_text(paint.text.layout_mono(
            badge.as_str(),
            Point::new(node_center.x - (badge_w * 0.5), badge_y),
            8.0,
            node_border.with_alpha(0.95),
        ));

        let owner_badge = match agent.owner_kind {
            crate::app_state::StableSatsWalletOwnerKind::Operator => "operator",
            crate::app_state::StableSatsWalletOwnerKind::SovereignAgent => "sovereign",
        };
        let owner_badge_w = paint.text.measure(owner_badge, 7.0);
        paint.scene.draw_text(paint.text.layout(
            owner_badge,
            Point::new(node_center.x - (owner_badge_w * 0.5), node_center.y + 18.0),
            7.0,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout(
        "edges: blue=BTC, green=USD | dot marks direction",
        Point::new(graph_bounds.origin.x + 8.0, graph_bounds.max_y() - 10.0),
        8.0,
        theme::text::MUTED,
    ));

    graph_bounds.max_y()
}

fn stable_sats_node_centers(graph_bounds: Bounds, count: usize) -> Vec<Point> {
    let center = Point::new(
        graph_bounds.origin.x + graph_bounds.size.width * 0.5,
        graph_bounds.origin.y + graph_bounds.size.height * 0.5,
    );
    match count {
        0 => Vec::new(),
        1 => vec![center],
        2 => vec![
            Point::new(
                graph_bounds.origin.x + graph_bounds.size.width * 0.30,
                center.y - 4.0,
            ),
            Point::new(
                graph_bounds.origin.x + graph_bounds.size.width * 0.70,
                center.y - 4.0,
            ),
        ],
        _ => vec![
            Point::new(
                center.x,
                graph_bounds.origin.y + graph_bounds.size.height * 0.24,
            ),
            Point::new(
                graph_bounds.origin.x + graph_bounds.size.width * 0.26,
                graph_bounds.origin.y + graph_bounds.size.height * 0.73,
            ),
            Point::new(
                graph_bounds.origin.x + graph_bounds.size.width * 0.74,
                graph_bounds.origin.y + graph_bounds.size.height * 0.73,
            ),
        ],
    }
}

fn stable_sats_wallet_anchor(wallet_ref: &str) -> &str {
    wallet_ref.split(':').next().unwrap_or(wallet_ref)
}

fn format_transfer_edge_label(
    transfer: &crate::app_state::StableSatsTransferLedgerEntry,
) -> String {
    let amount = match transfer.asset {
        StableSatsTransferAsset::BtcSats => format!("{} sats", transfer.amount),
        StableSatsTransferAsset::UsdCents => format_usd_cents(transfer.amount),
    };
    let fee = match transfer.asset {
        StableSatsTransferAsset::BtcSats => format!("{} sats", transfer.effective_fee),
        StableSatsTransferAsset::UsdCents => format_usd_cents(transfer.effective_fee),
    };
    format!("{amount} fee={fee}")
}

fn paint_stable_sats_transfer_strip(
    content_bounds: Bounds,
    y: f32,
    pane_state: &StableSatsSimulationPaneState,
    paint: &mut PaintContext,
) -> f32 {
    let max_y = content_bounds.max_y() - 8.0;
    if y + 12.0 > max_y {
        return y;
    }

    paint.scene.draw_text(paint.text.layout(
        "Latest transfer edges",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));
    let row_start = y + 14.0;
    let row_height = 12.0;
    let available_rows = ((max_y - row_start) / row_height).floor().max(0.0) as usize;
    if available_rows == 0 {
        return y + 12.0;
    }

    let entries = pane_state
        .transfer_ledger
        .iter()
        .rev()
        .take(available_rows.min(4))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No transfers yet.",
            Point::new(content_bounds.origin.x + 12.0, row_start),
            10.0,
            theme::text::MUTED,
        ));
        return row_start + row_height;
    }

    let mut row_y = row_start;
    for entry in entries.into_iter().rev() {
        let edge_color = match entry.asset {
            StableSatsTransferAsset::BtcSats => theme::accent::PRIMARY,
            StableSatsTransferAsset::UsdCents => theme::status::SUCCESS,
        };
        let summary = format!(
            "#{} {} -> {} {} fee={} status={}",
            entry.seq,
            stable_sats_wallet_anchor(entry.from_wallet.as_str()),
            stable_sats_wallet_anchor(entry.to_wallet.as_str()),
            match entry.asset {
                StableSatsTransferAsset::BtcSats => format!("{} sats", entry.amount),
                StableSatsTransferAsset::UsdCents => format_usd_cents(entry.amount),
            },
            match entry.asset {
                StableSatsTransferAsset::BtcSats => format!("{} sats", entry.effective_fee),
                StableSatsTransferAsset::UsdCents => format_usd_cents(entry.effective_fee),
            },
            entry.status.label(),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            summary.as_str(),
            Point::new(content_bounds.origin.x + 12.0, row_y),
            9.0,
            edge_color.with_alpha(0.96),
        ));
        row_y += row_height;
    }

    row_y
}

fn paint_simulation_timeline(
    content_bounds: wgpui::Bounds,
    y: f32,
    events: &[AgentNetworkSimulationEvent],
    paint: &mut PaintContext,
) {
    let max_y = content_bounds.max_y() - 8.0;
    if y + 10.0 > max_y {
        return;
    }

    if events.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No simulation events yet.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    paint.scene.draw_text(paint.text.layout(
        "Latest protocol timeline",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));
    let row_start_y = y + 16.0;
    if row_start_y + 8.0 > max_y {
        return;
    }

    let row_height = 14.0;
    let available_height = (max_y - row_start_y).max(0.0);
    let max_rows = (available_height / row_height).floor() as usize;
    if max_rows == 0 {
        return;
    }

    let mut row_y = row_start_y;
    for event in events.iter().rev().take(max_rows.min(8)) {
        let color = match event.protocol.as_str() {
            "NIP-28" => theme::accent::PRIMARY,
            "NIP-SKL" => theme::status::SUCCESS,
            "NIP-AC" => theme::status::ERROR,
            "NIP-SA" => theme::text::PRIMARY,
            "NIP-69" => theme::status::SUCCESS,
            "NIP-60" => theme::accent::PRIMARY,
            "NIP-61" => theme::status::ERROR,
            "NIP-42" => theme::status::SUCCESS,
            "NIP-59" => theme::accent::PRIMARY,
            "NIP-77" => theme::status::ERROR,
            "BLINK-PRICE" => theme::accent::PRIMARY,
            "BLINK-SWAP" => theme::status::SUCCESS,
            "BLINK-LEDGER" => theme::status::ERROR,
            _ => theme::text::PRIMARY,
        };
        let summary = format!(
            "#{} [{}] {} {}",
            event.seq, event.protocol, event.event_ref, event.summary
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(content_bounds.origin.x + 12.0, row_y),
            10.0,
            color,
        ));
        row_y += 14.0;
    }
}

fn format_usd_cents(usd_cents: u64) -> String {
    format!("${}.{:02}", usd_cents / 100, usd_cents % 100)
}

fn format_sats(sats: u64) -> String {
    let raw = sats.to_string();
    let mut grouped = String::with_capacity(raw.len() + (raw.len() / 3));
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

fn truncate_text(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }
    let truncated: String = label.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}...")
}

fn paint_stable_sats_graph(
    content_bounds: Bounds,
    y: f32,
    price_history: &[u64],
    converted_sats_history: &[u64],
    paint: &mut PaintContext,
) -> f32 {
    let max_y = content_bounds.max_y() - 8.0;
    if y + 12.0 > max_y {
        return y;
    }

    paint.scene.draw_text(paint.text.layout(
        "Graph: price + switched sats (last rounds)",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));

    let available = (max_y - y).max(0.0);
    let include_legend = available >= 52.0;
    let legend_space = if include_legend { 19.0 } else { 0.0 };
    let graph_height_max = available - 14.0 - legend_space;
    if graph_height_max < 12.0 {
        return y + 12.0;
    }

    let graph_height = graph_height_max.min(68.0);
    let graph_width = (content_bounds.size.width - 24.0).clamp(140.0, 320.0);
    let graph_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        y + 14.0,
        graph_width,
        graph_height,
    );
    paint.scene.draw_quad(
        Quad::new(graph_bounds)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(4.0),
    );

    if price_history.is_empty() || converted_sats_history.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Run rounds to populate graph data.",
            Point::new(graph_bounds.origin.x + 8.0, graph_bounds.origin.y + 22.0),
            10.0,
            theme::text::MUTED,
        ));
        if include_legend {
            paint.scene.draw_text(paint.text.layout(
                "blue=BTC/USD quote, green=switched sats",
                Point::new(graph_bounds.origin.x + 8.0, graph_bounds.max_y() + 10.0),
                9.0,
                theme::text::MUTED,
            ));
            return graph_bounds.max_y() + 19.0;
        }
        return graph_bounds.max_y();
    }

    let count = price_history
        .len()
        .min(converted_sats_history.len())
        .min(18);
    let start_price = price_history.len().saturating_sub(count);
    let start_sats = converted_sats_history.len().saturating_sub(count);
    let prices = &price_history[start_price..];
    let sats = &converted_sats_history[start_sats..];
    let max_price = prices.iter().copied().max().unwrap_or(1).max(1);
    let max_sats = sats.iter().copied().max().unwrap_or(1).max(1);

    let inner_pad = 6.0;
    let graph_inner = Bounds::new(
        graph_bounds.origin.x + inner_pad,
        graph_bounds.origin.y + inner_pad,
        graph_bounds.size.width - (inner_pad * 2.0),
        graph_bounds.size.height - (inner_pad * 2.0),
    );
    let group_w = graph_inner.size.width / count as f32;
    let price_bar_w = (group_w * 0.38).max(2.0);
    let sats_bar_w = (group_w * 0.38).max(2.0);

    for idx in 0..count {
        let price_ratio = prices[idx] as f32 / max_price as f32;
        let sats_ratio = sats[idx] as f32 / max_sats as f32;

        let price_h = (graph_inner.size.height * price_ratio).max(1.0);
        let sats_h = (graph_inner.size.height * sats_ratio).max(1.0);
        let base_x = graph_inner.origin.x + group_w * idx as f32;

        let price_bounds = Bounds::new(
            base_x + 0.5,
            graph_inner.max_y() - price_h,
            price_bar_w,
            price_h,
        );
        paint.scene.draw_quad(
            Quad::new(price_bounds)
                .with_background(theme::accent::PRIMARY)
                .with_corner_radius(1.0),
        );

        let sats_bounds = Bounds::new(
            base_x + price_bar_w + 1.5,
            graph_inner.max_y() - sats_h,
            sats_bar_w,
            sats_h,
        );
        paint.scene.draw_quad(
            Quad::new(sats_bounds)
                .with_background(theme::status::SUCCESS)
                .with_corner_radius(1.0),
        );
    }

    if include_legend {
        paint.scene.draw_text(paint.text.layout(
            "blue=BTC/USD quote, green=switched sats",
            Point::new(graph_bounds.origin.x + 8.0, graph_bounds.max_y() + 10.0),
            9.0,
            theme::text::MUTED,
        ));
        return graph_bounds.max_y() + 19.0;
    }

    graph_bounds.max_y()
}

#[cfg(test)]
mod tests {
    use super::{format_transfer_edge_label, stable_sats_node_centers, stable_sats_wallet_anchor};
    use crate::app_state::{
        StableSatsTransferAsset, StableSatsTransferLedgerEntry, StableSatsTransferStatus,
    };
    use wgpui::Bounds;

    #[test]
    fn stablesats_topology_centers_render_three_wallet_layout() {
        let bounds = Bounds::new(10.0, 20.0, 360.0, 160.0);
        let centers = stable_sats_node_centers(bounds, 3);
        assert_eq!(centers.len(), 3);
        assert!(centers[0].y < centers[1].y);
        assert!(centers[0].y < centers[2].y);
    }

    #[test]
    fn stablesats_wallet_anchor_extracts_agent_name() {
        assert_eq!(stable_sats_wallet_anchor("sa-wallet-1:BTC"), "sa-wallet-1");
        assert_eq!(
            stable_sats_wallet_anchor("autopilot-user:usd_cents"),
            "autopilot-user"
        );
        assert_eq!(
            stable_sats_wallet_anchor("raw-wallet-name"),
            "raw-wallet-name"
        );
    }

    #[test]
    fn transfer_edge_label_includes_amount_and_fee() {
        let btc = StableSatsTransferLedgerEntry {
            seq: 1,
            transfer_ref: "blink:live:transfer:1".to_string(),
            from_wallet: "autopilot-user:BTC".to_string(),
            to_wallet: "sa-wallet-1:BTC".to_string(),
            asset: StableSatsTransferAsset::BtcSats,
            amount: 2_500,
            effective_fee: 10,
            status: StableSatsTransferStatus::Settled,
            summary: "btc transfer".to_string(),
            occurred_at_epoch_seconds: 1_761_000_000,
        };
        assert_eq!(format_transfer_edge_label(&btc), "2500 sats fee=10 sats");

        let usd = StableSatsTransferLedgerEntry {
            seq: 2,
            transfer_ref: "blink:live:transfer:2".to_string(),
            from_wallet: "sa-wallet-1:USD".to_string(),
            to_wallet: "sa-wallet-2:USD".to_string(),
            asset: StableSatsTransferAsset::UsdCents,
            amount: 311,
            effective_fee: 5,
            status: StableSatsTransferStatus::Settled,
            summary: "usd transfer".to_string(),
            occurred_at_epoch_seconds: 1_761_000_001,
        };
        assert_eq!(format_transfer_edge_label(&usd), "$3.11 fee=$0.05");
    }
}
