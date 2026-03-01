use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AgentNetworkSimulationEvent, AgentNetworkSimulationPaneState, RelaySecuritySimulationPaneState,
    StableSatsSimulationPaneState, TreasuryExchangeSimulationPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    agent_network_simulation_reset_button_bounds, agent_network_simulation_run_button_bounds,
    relay_security_simulation_reset_button_bounds, relay_security_simulation_run_button_bounds,
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

    paint.scene.draw_text(paint.text.layout(
        "Agent wallet states",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));
    let mut row_y = y + 16.0;
    for agent in pane_state.agents.iter().take(6) {
        let summary = format!(
            "{} wallet={} btc={} usd={} switches={} last={}",
            agent.agent_name,
            agent.active_wallet.label(),
            agent.btc_balance_sats,
            format_usd_cents(agent.usd_balance_cents),
            agent.switch_count,
            agent.last_switch_summary
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(content_bounds.origin.x + 12.0, row_y),
            10.0,
            theme::text::PRIMARY,
        ));
        row_y += 14.0;
    }

    row_y += 8.0;
    let graph_end_y = paint_stable_sats_graph(
        content_bounds,
        row_y,
        &pane_state.price_history_usd_cents_per_btc,
        &pane_state.converted_sats_history,
        paint,
    );

    paint_simulation_timeline(content_bounds, graph_end_y + 6.0, &pane_state.events, paint);
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
