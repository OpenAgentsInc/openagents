use std::collections::BTreeMap;

use chrono::{Local, TimeZone};
use wgpui::components::hud::{DotShape, DotsGrid, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::state::nip90_payment_facts::{
    Nip90PaymentFact, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality, Nip90RelayHop,
    Nip90RelayHopKind,
};
use crate::state::operations::{RelayConnectionRow, RelayConnectionStatus, RelayConnectionsState};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 74.0;

pub fn paint(
    content_bounds: Bounds,
    relay_connections: &RelayConnectionsState,
    payment_facts: &Nip90PaymentFactLedgerState,
    paint: &mut PaintContext,
) {
    paint_source_badge(
        content_bounds,
        "runtime.relay_connections + stream.nip90_payment_facts.v1",
        paint,
    );
    let Some(view) = build_view(relay_connections, payment_facts) else {
        paint_empty_state(content_bounds, relay_connections, payment_facts, paint);
        return;
    };

    let header_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + PADDING,
        content_bounds.size.width - PADDING * 2.0,
        HEADER_HEIGHT,
    );
    let field_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width * 0.66,
        content_bounds.size.height - HEADER_HEIGHT - 28.0,
    );
    let detail_bounds = Bounds::new(
        field_bounds.max_x() + 10.0,
        field_bounds.origin.y,
        content_bounds.max_x() - field_bounds.max_x() - 22.0,
        field_bounds.size.height,
    );

    paint_header(header_bounds, &view, paint);
    paint_field(field_bounds, &view, paint);
    paint_detail(detail_bounds, &view, paint);
}

#[derive(Clone, Debug, PartialEq)]
struct RelayChoreographyView {
    relays: Vec<RelayNode>,
    focus: Option<FocusRequest>,
    connected_count: usize,
    unhealthy_live_count: usize,
    historical_only_count: usize,
    publish_total: usize,
    total_hop_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct RelayNode {
    url: String,
    short_label: String,
    health: RelayHealth,
    live: bool,
    latency_ms: Option<u32>,
    last_seen_seconds_ago: Option<u64>,
    last_error: Option<String>,
    publish_accepted_count: usize,
    publish_rejected_count: usize,
    result_count: usize,
    invoice_count: usize,
    focus_weight: usize,
}

impl RelayNode {
    fn activity_total(&self) -> usize {
        self.publish_accepted_count
            + self.publish_rejected_count
            + self.result_count
            + self.invoice_count
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FocusRequest {
    request_id: String,
    buyer_pubkey: String,
    buyer_label: String,
    provider_pubkey: Option<String>,
    provider_label: String,
    status_label: String,
    amount_sats: Option<u64>,
    source_quality: Nip90PaymentFactSourceQuality,
    relay_threads: Vec<RelayThread>,
    buyer_wallet_confirmed: bool,
    seller_wallet_confirmed: bool,
}

impl FocusRequest {
    fn wallet_summary(&self) -> &'static str {
        match (self.buyer_wallet_confirmed, self.seller_wallet_confirmed) {
            (true, true) => "buyer+seller confirmed",
            (true, false) => "buyer confirmed",
            (false, true) => "seller confirmed",
            (false, false) => "request-level only",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RelayThread {
    relay_url: String,
    event_id: String,
    phase: RelayThreadPhase,
    observed_at: u64,
    accepted: bool,
    degraded: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelayHealth {
    Connected,
    Connecting,
    Disconnected,
    Error,
    HistoricalOnly,
}

impl RelayHealth {
    const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
            Self::HistoricalOnly => "historical-only",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Connected => 4,
            Self::Connecting => 3,
            Self::Disconnected => 2,
            Self::Error => 1,
            Self::HistoricalOnly => 0,
        }
    }

    fn accent(self) -> Hsla {
        match self {
            Self::Connected => theme::status::SUCCESS,
            Self::Connecting => Hsla::from_hex(0x6ed0ff),
            Self::Disconnected => theme::text::MUTED,
            Self::Error => theme::status::ERROR,
            Self::HistoricalOnly => Hsla::from_hex(0xffbf69),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelayThreadPhase {
    PublishAccepted,
    PublishRejected,
    ResultIngress,
    InvoiceIngress,
}

impl RelayThreadPhase {
    const fn label(self) -> &'static str {
        match self {
            Self::PublishAccepted => "publish-accepted",
            Self::PublishRejected => "publish-rejected",
            Self::ResultIngress => "result-ingress",
            Self::InvoiceIngress => "invoice-ingress",
        }
    }

    const fn short_label(self) -> &'static str {
        match self {
            Self::PublishAccepted => "PUB+",
            Self::PublishRejected => "PUB-",
            Self::ResultIngress => "RES",
            Self::InvoiceIngress => "INV",
        }
    }

    fn accent(self) -> Hsla {
        match self {
            Self::PublishAccepted => Hsla::from_hex(0x6ed0ff),
            Self::PublishRejected => Hsla::from_hex(0xff7f50),
            Self::ResultIngress => Hsla::from_hex(0x68f0b8),
            Self::InvoiceIngress => Hsla::from_hex(0xffbf69),
        }
    }

    const fn thickness(self) -> f32 {
        match self {
            Self::PublishAccepted => 4.0,
            Self::PublishRejected => 3.0,
            Self::ResultIngress => 4.4,
            Self::InvoiceIngress => 5.0,
        }
    }
}

fn build_view(
    relay_connections: &RelayConnectionsState,
    payment_facts: &Nip90PaymentFactLedgerState,
) -> Option<RelayChoreographyView> {
    build_view_from_state(
        relay_connections.relays.as_slice(),
        payment_facts.facts.as_slice(),
        payment_facts.relay_hops.as_slice(),
    )
}

fn build_view_from_state(
    relays: &[RelayConnectionRow],
    facts: &[Nip90PaymentFact],
    relay_hops: &[Nip90RelayHop],
) -> Option<RelayChoreographyView> {
    if relays.is_empty() && relay_hops.is_empty() {
        return None;
    }

    let mut relay_nodes = BTreeMap::<String, RelayNode>::new();
    for relay in relays {
        relay_nodes.insert(
            relay.url.clone(),
            RelayNode {
                url: relay.url.clone(),
                short_label: compact_relay(relay.url.as_str()),
                health: relay_health_from_status(relay.status),
                live: true,
                latency_ms: relay.latency_ms,
                last_seen_seconds_ago: relay.last_seen_seconds_ago,
                last_error: relay.last_error.clone(),
                publish_accepted_count: 0,
                publish_rejected_count: 0,
                result_count: 0,
                invoice_count: 0,
                focus_weight: 0,
            },
        );
    }

    for hop in relay_hops {
        let node = relay_nodes
            .entry(hop.relay_url.clone())
            .or_insert_with(|| RelayNode {
                url: hop.relay_url.clone(),
                short_label: compact_relay(hop.relay_url.as_str()),
                health: RelayHealth::HistoricalOnly,
                live: false,
                latency_ms: None,
                last_seen_seconds_ago: None,
                last_error: None,
                publish_accepted_count: 0,
                publish_rejected_count: 0,
                result_count: 0,
                invoice_count: 0,
                focus_weight: 0,
            });
        match hop.hop_kind {
            Nip90RelayHopKind::PublishAccepted => {
                node.publish_accepted_count = node.publish_accepted_count.saturating_add(1);
            }
            Nip90RelayHopKind::PublishRejected => {
                node.publish_rejected_count = node.publish_rejected_count.saturating_add(1);
            }
            Nip90RelayHopKind::ResultIngress => {
                node.result_count = node.result_count.saturating_add(1);
            }
            Nip90RelayHopKind::InvoiceIngress => {
                node.invoice_count = node.invoice_count.saturating_add(1);
            }
            Nip90RelayHopKind::RequestIngress => {}
        }
    }

    let focus = select_focus_request(facts, relay_hops);
    if let Some(focus_request) = focus.as_ref() {
        for thread in &focus_request.relay_threads {
            if let Some(node) = relay_nodes.get_mut(thread.relay_url.as_str()) {
                node.focus_weight = node.focus_weight.saturating_add(1);
            }
        }
    }

    let mut relays = relay_nodes.into_values().collect::<Vec<_>>();
    relays.sort_by(|left, right| {
        right
            .focus_weight
            .cmp(&left.focus_weight)
            .then_with(|| right.health.rank().cmp(&left.health.rank()))
            .then_with(|| right.activity_total().cmp(&left.activity_total()))
            .then_with(|| left.url.cmp(&right.url))
    });

    let connected_count = relays
        .iter()
        .filter(|relay| relay.health == RelayHealth::Connected)
        .count();
    let unhealthy_live_count = relays
        .iter()
        .filter(|relay| {
            relay.live && matches!(relay.health, RelayHealth::Disconnected | RelayHealth::Error)
        })
        .count();
    let historical_only_count = relays.iter().filter(|relay| !relay.live).count();
    let publish_total = relays
        .iter()
        .map(|relay| relay.publish_accepted_count + relay.publish_rejected_count)
        .sum();

    Some(RelayChoreographyView {
        connected_count,
        unhealthy_live_count,
        historical_only_count,
        publish_total,
        total_hop_count: relay_hops.len(),
        relays,
        focus,
    })
}

fn select_focus_request(
    facts: &[Nip90PaymentFact],
    relay_hops: &[Nip90RelayHop],
) -> Option<FocusRequest> {
    let focus_fact = facts
        .iter()
        .filter(|fact| relay_signal_count(fact, relay_hops) > 0)
        .max_by(|left, right| {
            left.latest_event_epoch_seconds()
                .unwrap_or(0)
                .cmp(&right.latest_event_epoch_seconds().unwrap_or(0))
                .then_with(|| {
                    relay_signal_count(left, relay_hops).cmp(&relay_signal_count(right, relay_hops))
                })
                .then_with(|| left.request_id.cmp(&right.request_id))
        })?;

    let mut relay_threads = relay_hops
        .iter()
        .filter(|hop| hop.request_id == focus_fact.request_id)
        .filter_map(|hop| relay_thread_from_hop(focus_fact, hop))
        .collect::<Vec<_>>();
    relay_threads.sort_by(|left, right| {
        left.observed_at
            .cmp(&right.observed_at)
            .then_with(|| left.phase.label().cmp(right.phase.label()))
            .then_with(|| left.relay_url.cmp(&right.relay_url))
    });

    Some(FocusRequest {
        request_id: focus_fact.request_id.clone(),
        buyer_pubkey: focus_fact
            .buyer_nostr_pubkey
            .as_deref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "buyer:unknown".to_string()),
        buyer_label: compact_identity(focus_fact.buyer_nostr_pubkey.as_deref()),
        provider_pubkey: focus_fact.provider_nostr_pubkey.clone(),
        provider_label: compact_identity(focus_fact.provider_nostr_pubkey.as_deref()),
        status_label: focus_fact.status.label().to_string(),
        amount_sats: focus_fact.amount_sats,
        source_quality: focus_fact.source_quality,
        relay_threads,
        buyer_wallet_confirmed: focus_fact.buyer_wallet_confirmed_at.is_some(),
        seller_wallet_confirmed: focus_fact.seller_wallet_confirmed_at.is_some(),
    })
}

fn relay_signal_count(fact: &Nip90PaymentFact, relay_hops: &[Nip90RelayHop]) -> usize {
    relay_hops
        .iter()
        .filter(|hop| hop.request_id == fact.request_id)
        .count()
        + fact.publish_accepted_relays.len()
        + fact.publish_rejected_relays.len()
        + fact.invoice_observed_relays.len()
        + fact.result_observed_relays.len()
}

fn relay_thread_from_hop(fact: &Nip90PaymentFact, hop: &Nip90RelayHop) -> Option<RelayThread> {
    let phase = match hop.hop_kind {
        Nip90RelayHopKind::PublishAccepted => RelayThreadPhase::PublishAccepted,
        Nip90RelayHopKind::PublishRejected => RelayThreadPhase::PublishRejected,
        Nip90RelayHopKind::ResultIngress => RelayThreadPhase::ResultIngress,
        Nip90RelayHopKind::InvoiceIngress => RelayThreadPhase::InvoiceIngress,
        Nip90RelayHopKind::RequestIngress => return None,
    };
    Some(RelayThread {
        relay_url: hop.relay_url.clone(),
        event_id: hop.event_id.clone(),
        phase,
        observed_at: hop.observed_at,
        accepted: hop.accepted,
        degraded: !hop.accepted
            || fact.source_quality == Nip90PaymentFactSourceQuality::LogBackfill,
    })
}

fn paint_header(bounds: Bounds, view: &RelayChoreographyView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "RELAY CHOREOGRAPHY  //  LIVE HEALTH VS PAYMENT EVIDENCE",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Relay node color is live runtime health. Threads are persisted payment-fact relay evidence only; wallet settlement stays request-scoped until relay-scoped proof exists.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));

    let focus_summary = if let Some(focus) = view.focus.as_ref() {
        format!(
            "focus={}  buyer={}  provider={}  threads={}  wallet={}",
            short_id(focus.request_id.as_str()),
            focus.buyer_label,
            focus.provider_label,
            focus.relay_threads.len(),
            focus.wallet_summary()
        )
    } else {
        format!(
            "focus=none  live_relays={}  historical_only={}  hops={}",
            view.relays.iter().filter(|relay| relay.live).count(),
            view.historical_only_count,
            view.total_hop_count
        )
    };
    paint.scene.draw_text(paint.text.layout_mono(
        focus_summary.as_str(),
        Point::new(bounds.origin.x, bounds.origin.y + 50.0),
        10.0,
        theme::text::SECONDARY,
    ));

    let relay_health = if view.relays.is_empty() {
        0.0
    } else {
        view.connected_count as f32
            / view.relays.iter().filter(|relay| relay.live).count().max(1) as f32
    };
    let mut meter = SignalMeter::new()
        .bars(7)
        .gap(2.0)
        .level(relay_health.clamp(0.0, 1.0))
        .min_bar_height(0.15)
        .active_color(Hsla::from_hex(0x6ed0ff).with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.48));
    meter.paint(
        Bounds::new(bounds.max_x() - 50.0, bounds.origin.y + 6.0, 38.0, 40.0),
        paint,
    );
}

fn paint_field(bounds: Bounds, view: &RelayChoreographyView, paint: &mut PaintContext) {
    paint_panel_shell(bounds, "Relay Field", Hsla::from_hex(0x6ed0ff), paint);

    let field_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 30.0,
        bounds.size.width - 24.0,
        bounds.size.height - 42.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(0.9)
        .color(Hsla::from_hex(0x6ed0ff).with_alpha(0.12))
        .animation_progress(1.0);
    dots.paint(field_bounds, paint);

    let buyer_pos = Point::new(
        field_bounds.origin.x + 62.0,
        field_bounds.origin.y + field_bounds.size.height * 0.5,
    );
    let provider_pos = Point::new(
        field_bounds.max_x() - 62.0,
        field_bounds.origin.y + field_bounds.size.height * 0.5,
    );

    let relay_positions = view
        .relays
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, relay)| {
            let column_x = field_bounds.origin.x
                + field_bounds.size.width * 0.5
                + if index % 2 == 0 { -48.0 } else { 48.0 };
            let y = vertical_slot(field_bounds, index, view.relays.len().min(8));
            (relay.url.clone(), Point::new(column_x, y))
        })
        .collect::<BTreeMap<_, _>>();

    let buyer_label = view
        .focus
        .as_ref()
        .map(|focus| focus.buyer_label.as_str())
        .unwrap_or("buyer");
    let provider_label = view
        .focus
        .as_ref()
        .map(|focus| focus.provider_label.as_str())
        .unwrap_or("provider");
    paint_actor_node(
        buyer_pos,
        buyer_label,
        Hsla::from_hex(0x6ed0ff),
        true,
        paint,
    );
    paint_actor_node(
        provider_pos,
        provider_label,
        Hsla::from_hex(0x68f0b8),
        false,
        paint,
    );

    for relay in view.relays.iter().take(8) {
        if let Some(position) = relay_positions.get(relay.url.as_str()) {
            paint_relay_node(*position, relay, paint);
        }
    }

    if let Some(focus) = view.focus.as_ref() {
        for thread in &focus.relay_threads {
            let Some(relay_pos) = relay_positions.get(thread.relay_url.as_str()) else {
                continue;
            };
            let accent = thread
                .phase
                .accent()
                .with_alpha(if thread.degraded { 0.58 } else { 0.9 });
            let broken = !thread.accepted || thread.degraded;
            match thread.phase {
                RelayThreadPhase::PublishAccepted | RelayThreadPhase::PublishRejected => {
                    paint_thread_path(
                        &[buyer_pos, *relay_pos],
                        accent,
                        thread.phase.thickness(),
                        broken,
                        paint,
                    );
                }
                RelayThreadPhase::ResultIngress | RelayThreadPhase::InvoiceIngress => {
                    paint_thread_path(
                        &[provider_pos, *relay_pos, buyer_pos],
                        accent,
                        thread.phase.thickness(),
                        broken,
                        paint,
                    );
                }
            }

            let label_pos = Point::new(
                relay_pos.x - 18.0,
                relay_pos.y - 34.0 - (thread.phase.thickness() * 0.6),
            );
            paint.scene.draw_text(paint.text.layout_mono(
                thread.phase.short_label(),
                label_pos,
                8.0,
                accent.with_alpha(0.94),
            ));
        }

        if focus.buyer_wallet_confirmed || focus.seller_wallet_confirmed {
            paint_settlement_halo(provider_pos, focus, paint);
        }
    }
}

fn paint_detail(bounds: Bounds, view: &RelayChoreographyView, paint: &mut PaintContext) {
    paint_panel_shell(bounds, "Evidence Detail", Hsla::from_hex(0xffbf69), paint);

    let mut y = bounds.origin.y + 32.0;
    if let Some(focus) = view.focus.as_ref() {
        paint.scene.draw_text(
            paint.text.layout_mono(
                format!(
                    "{}  {} -> {}",
                    short_id(focus.request_id.as_str()),
                    focus.buyer_label,
                    focus.provider_label
                )
                .as_str(),
                Point::new(bounds.origin.x + 12.0, y),
                12.0,
                theme::text::PRIMARY,
            ),
        );
        y += 18.0;
        paint.scene.draw_text(paint.text.layout(
            "Wallet confirmation stays request-scoped here. Relay rows below only summarize transport evidence and live runtime health.",
            Point::new(bounds.origin.x + 12.0, y),
            9.0,
            theme::text::MUTED,
        ));
        y += 28.0;
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "buyer",
            focus.buyer_pubkey.as_str(),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "provider",
            focus
                .provider_pubkey
                .as_deref()
                .unwrap_or("provider:unknown"),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "status",
            focus.status_label.as_str(),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "amount",
            focus
                .amount_sats
                .map(format_sats_amount)
                .unwrap_or_else(|| "-".to_string())
                .as_str(),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "source",
            focus.source_quality.label(),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "wallet",
            focus.wallet_summary(),
        );
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "relay_threads",
            focus.relay_threads.len().to_string().as_str(),
        );
    } else {
        paint.scene.draw_text(paint.text.layout_mono(
            "No request-level relay path in focus yet.",
            Point::new(bounds.origin.x + 12.0, y),
            12.0,
            theme::text::PRIMARY,
        ));
        y += 18.0;
        paint.scene.draw_text(paint.text.layout(
            "Live relay health is still visible below. Historical relay paths appear once request/result/invoice publish evidence lands in the payment ledger.",
            Point::new(bounds.origin.x + 12.0, y),
            9.0,
            theme::text::MUTED,
        ));
        y += 30.0;
    }

    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "connected",
        view.connected_count.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "unhealthy_live",
        view.unhealthy_live_count.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "historical_only",
        view.historical_only_count.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "publish_edges",
        view.publish_total.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "relay_hops",
        view.total_hop_count.to_string().as_str(),
    );

    y += 10.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "relay                  health        agg      focus",
        Point::new(bounds.origin.x + 12.0, y),
        8.6,
        theme::text::MUTED,
    ));
    y += 16.0;
    for relay in view.relays.iter().take(8) {
        let aggregate = format!(
            "P{}/{} R{} I{}",
            relay.publish_accepted_count,
            relay.publish_rejected_count,
            relay.result_count,
            relay.invoice_count
        );
        let focus_marker = if relay.focus_weight > 0 { "focus" } else { "-" };
        let line = format!(
            "{:<22} {:<12} {:<10} {}",
            relay.short_label,
            relay.health.label(),
            aggregate,
            focus_marker
        );
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(bounds.origin.x + 12.0, y),
            8.3,
            relay.health.accent().with_alpha(0.9),
        ));
        y += 14.0;
        if relay.focus_weight > 0 {
            paint.scene.draw_text(
                paint.text.layout_mono(
                    format!(
                        "  last_seen={}  latency={}  error={}",
                        relay
                            .last_seen_seconds_ago
                            .map(|seconds| format!("{seconds}s"))
                            .unwrap_or_else(|| "-".to_string()),
                        relay
                            .latency_ms
                            .map(|latency| format!("{latency}ms"))
                            .unwrap_or_else(|| "-".to_string()),
                        relay.last_error.as_deref().unwrap_or("-")
                    )
                    .as_str(),
                    Point::new(bounds.origin.x + 12.0, y),
                    7.6,
                    theme::text::SECONDARY,
                ),
            );
            y += 12.0;
        }
    }

    if let Some(focus) = view.focus.as_ref()
        && let Some(thread) = focus.relay_threads.last()
    {
        let _ = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            bounds.max_y() - 28.0,
            "latest_edge",
            format!(
                "{} {} @ {}",
                thread.phase.short_label(),
                compact_relay(thread.relay_url.as_str()),
                timestamp_label(Some(thread.observed_at))
            )
            .as_str(),
        );
    }
}

fn paint_empty_state(
    content_bounds: Bounds,
    relay_connections: &RelayConnectionsState,
    payment_facts: &Nip90PaymentFactLedgerState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 40.0,
            content_bounds.size.width - 24.0,
            content_bounds.size.height - 52.0,
        ))
        .with_background(Hsla::from_hex(0x091118).with_alpha(0.96))
        .with_border(Hsla::from_hex(0x2c5767).with_alpha(0.32), 1.0)
        .with_corner_radius(12.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "RELAY CHOREOGRAPHY",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No relay telemetry or persisted relay-hop evidence is available yet. The pane turns on once either runtime relay rows or payment-fact relay provenance arrives.",
        Point::new(content_bounds.origin.x + 24.0, content_bounds.origin.y + 88.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "relay_rows={}  ledger_state={}  ledger_hops={}",
                relay_connections.relays.len(),
                payment_facts.load_state.label(),
                payment_facts.relay_hops.len()
            )
            .as_str(),
            Point::new(
                content_bounds.origin.x + 24.0,
                content_bounds.origin.y + 118.0,
            ),
            9.0,
            theme::text::SECONDARY,
        ),
    );
}

fn paint_panel_shell(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x071018).with_alpha(0.96))
            .with_border(accent.with_alpha(0.28), 1.0)
            .with_corner_radius(12.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        accent.with_alpha(0.92),
    ));
}

fn paint_actor_node(
    position: Point,
    label: &str,
    accent: Hsla,
    buyer_side: bool,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            position.x - 22.0,
            position.y - 22.0,
            44.0,
            44.0,
        ))
        .with_background(accent.with_alpha(0.18))
        .with_border(accent.with_alpha(0.72), 1.0)
        .with_corner_radius(22.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(position.x - 5.0, position.y - 5.0, 10.0, 10.0))
            .with_background(accent.with_alpha(0.96))
            .with_corner_radius(5.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(
            if buyer_side {
                position.x - 34.0
            } else {
                position.x - 16.0
            },
            position.y + 28.0,
        ),
        9.0,
        theme::text::PRIMARY,
    ));
}

fn paint_relay_node(position: Point, relay: &RelayNode, paint: &mut PaintContext) {
    let accent = relay.health.accent();
    let bounds = Bounds::new(position.x - 58.0, position.y - 18.0, 116.0, 42.0);
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(if relay.focus_weight > 0 { 0.18 } else { 0.08 }))
            .with_border(accent.with_alpha(0.54), 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        relay.short_label.as_str(),
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 8.0),
        8.8,
        accent.with_alpha(0.94),
    ));
    let status_line = if relay.live {
        format!(
            "{}  {}",
            relay.health.label(),
            relay
                .latency_ms
                .map(|latency| format!("{latency}ms"))
                .unwrap_or_else(|| "-".to_string())
        )
    } else {
        "historical evidence".to_string()
    };
    paint.scene.draw_text(paint.text.layout(
        status_line.as_str(),
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 20.0),
        7.6,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "P{}/{} R{} I{}",
                relay.publish_accepted_count,
                relay.publish_rejected_count,
                relay.result_count,
                relay.invoice_count
            )
            .as_str(),
            Point::new(bounds.origin.x + 8.0, bounds.origin.y + 30.0),
            7.4,
            theme::text::MUTED,
        ),
    );
}

fn paint_thread_path(
    points: &[Point],
    accent: Hsla,
    thickness: f32,
    broken: bool,
    paint: &mut PaintContext,
) {
    if points.len() < 2 {
        return;
    }

    for segment in points.windows(2) {
        let from = segment[0];
        let to = segment[1];
        let steps = 16usize;
        for index in 0..=steps {
            if broken && index % 2 == 1 {
                continue;
            }
            let t = index as f32 / steps as f32;
            let current = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            let dot = thickness + (1.0 - (0.5 - t).abs()) * 1.2;
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    current.x - dot * 0.5,
                    current.y - dot * 0.5,
                    dot,
                    dot,
                ))
                .with_background(accent.with_alpha(0.14 + 0.72 * t))
                .with_corner_radius(dot * 0.5),
            );
        }
    }
}

fn paint_settlement_halo(position: Point, focus: &FocusRequest, paint: &mut PaintContext) {
    let accent = if focus.seller_wallet_confirmed {
        Hsla::from_hex(0x68f0b8)
    } else {
        Hsla::from_hex(0xffbf69)
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            position.x - 34.0,
            position.y - 34.0,
            68.0,
            68.0,
        ))
        .with_border(accent.with_alpha(0.42), 2.0)
        .with_corner_radius(34.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "wallet settle is request-scoped",
        Point::new(position.x - 52.0, position.y - 52.0),
        8.2,
        accent.with_alpha(0.92),
    ));
}

fn relay_health_from_status(status: RelayConnectionStatus) -> RelayHealth {
    match status {
        RelayConnectionStatus::Connected => RelayHealth::Connected,
        RelayConnectionStatus::Connecting => RelayHealth::Connecting,
        RelayConnectionStatus::Disconnected => RelayHealth::Disconnected,
        RelayConnectionStatus::Error => RelayHealth::Error,
    }
}

fn vertical_slot(bounds: Bounds, index: usize, total: usize) -> f32 {
    let total = total.max(1);
    let step = bounds.size.height / (total as f32 + 1.0);
    bounds.origin.y + step * (index as f32 + 1.0)
}

fn compact_identity(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "unknown".to_string();
    };
    if value.chars().count() <= 12 {
        return value.to_string();
    }
    let prefix = value.chars().take(6).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}..{suffix}")
}

fn compact_relay(value: &str) -> String {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix("wss://")
        .or_else(|| trimmed.strip_prefix("ws://"))
        .unwrap_or(trimmed);
    if trimmed.chars().count() <= 18 {
        trimmed.to_string()
    } else {
        let prefix = trimmed.chars().take(12).collect::<String>();
        let suffix = trimmed
            .chars()
            .rev()
            .take(5)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("{prefix}..{suffix}")
    }
}

fn short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 12 {
        trimmed.to_string()
    } else {
        let prefix = trimmed.chars().take(8).collect::<String>();
        let suffix = trimmed
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("{prefix}..{suffix}")
    }
}

fn timestamp_label(value: Option<u64>) -> String {
    let Some(epoch_seconds) = value else {
        return "-".to_string();
    };
    Local
        .timestamp_opt(epoch_seconds as i64, 0)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| epoch_seconds.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::nip90_payment_facts::{Nip90PaymentFactStatus, Nip90RelayHopDirection};

    #[test]
    fn build_view_merges_live_and_historical_relays_for_focus_request() {
        let relays = vec![
            RelayConnectionRow {
                url: "wss://relay.alpha".to_string(),
                status: RelayConnectionStatus::Connected,
                latency_ms: Some(34),
                last_seen_seconds_ago: Some(3),
                last_error: None,
            },
            RelayConnectionRow {
                url: "wss://relay.beta".to_string(),
                status: RelayConnectionStatus::Error,
                latency_ms: None,
                last_seen_seconds_ago: Some(12),
                last_error: Some("timeout".to_string()),
            },
        ];
        let facts = vec![test_fact(
            "req-live-001",
            Nip90PaymentFactSourceQuality::SellerWalletReconciled,
        )];
        let relay_hops = vec![
            test_hop(
                "req-live-001",
                "event-request-001",
                Nip90RelayHopKind::PublishAccepted,
                "wss://relay.alpha",
                true,
                100,
            ),
            test_hop(
                "req-live-001",
                "event-request-001",
                Nip90RelayHopKind::PublishRejected,
                "wss://relay.beta",
                false,
                101,
            ),
            test_hop(
                "req-live-001",
                "event-result-001",
                Nip90RelayHopKind::ResultIngress,
                "wss://relay.alpha",
                true,
                110,
            ),
            test_hop(
                "req-live-001",
                "event-feedback-001",
                Nip90RelayHopKind::InvoiceIngress,
                "wss://relay.gamma",
                true,
                111,
            ),
        ];

        let view = build_view_from_state(&relays, &facts, &relay_hops).expect("view");
        assert_eq!(view.connected_count, 1);
        assert_eq!(view.unhealthy_live_count, 1);
        assert_eq!(view.historical_only_count, 1);
        assert_eq!(view.publish_total, 2);
        assert_eq!(view.total_hop_count, 4);

        let focus = view.focus.expect("focus request");
        assert_eq!(focus.request_id, "req-live-001");
        assert_eq!(focus.relay_threads.len(), 4);
        assert_eq!(focus.wallet_summary(), "buyer+seller confirmed");

        assert!(view.relays.iter().any(|relay| {
            relay.url == "wss://relay.gamma"
                && !relay.live
                && relay.health == RelayHealth::HistoricalOnly
                && relay.invoice_count == 1
        }));
    }

    #[test]
    fn build_view_marks_backfilled_and_rejected_threads_as_degraded() {
        let facts = vec![test_fact(
            "req-backfill-001",
            Nip90PaymentFactSourceQuality::LogBackfill,
        )];
        let relay_hops = vec![
            test_hop(
                "req-backfill-001",
                "event-request-002",
                Nip90RelayHopKind::PublishRejected,
                "wss://relay.fail",
                false,
                210,
            ),
            test_hop(
                "req-backfill-001",
                "event-feedback-002",
                Nip90RelayHopKind::InvoiceIngress,
                "wss://relay.fail",
                true,
                220,
            ),
        ];

        let view = build_view_from_state(&[], &facts, &relay_hops).expect("view");
        let focus = view.focus.expect("focus request");

        assert!(focus.relay_threads.iter().any(|thread| {
            thread.phase == RelayThreadPhase::PublishRejected && thread.degraded && !thread.accepted
        }));
        assert!(focus.relay_threads.iter().any(|thread| {
            thread.phase == RelayThreadPhase::InvoiceIngress && thread.degraded && thread.accepted
        }));
    }

    #[test]
    fn build_view_can_render_live_health_without_historical_focus() {
        let relays = vec![RelayConnectionRow {
            url: "wss://relay.health.only".to_string(),
            status: RelayConnectionStatus::Connecting,
            latency_ms: None,
            last_seen_seconds_ago: None,
            last_error: None,
        }];

        let view = build_view_from_state(&relays, &[], &[]).expect("view");
        assert!(view.focus.is_none());
        assert_eq!(view.relays.len(), 1);
        assert_eq!(view.relays[0].health, RelayHealth::Connecting);
    }

    fn test_fact(
        request_id: &str,
        source_quality: Nip90PaymentFactSourceQuality,
    ) -> Nip90PaymentFact {
        Nip90PaymentFact {
            fact_id: format!("fact:{request_id}"),
            request_id: request_id.to_string(),
            request_type: "text-generation".to_string(),
            request_event_id: Some(format!("event-request:{request_id}")),
            result_event_id: Some(format!("event-result:{request_id}")),
            invoice_event_id: Some(format!("event-invoice:{request_id}")),
            seller_feedback_event_id: Some(format!("event-feedback:{request_id}")),
            buyer_nostr_pubkey: Some("buyerpubkey001".to_string()),
            provider_nostr_pubkey: Some("providerpubkey001".to_string()),
            invoice_provider_pubkey: Some("providerpubkey001".to_string()),
            result_provider_pubkey: Some("providerpubkey001".to_string()),
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: Some("wallet-send-001".to_string()),
            seller_payment_pointer: Some("wallet-recv-001".to_string()),
            buyer_payment_hash: Some("hash-001".to_string()),
            amount_sats: Some(2),
            fees_sats: Some(0),
            total_debit_sats: Some(2),
            wallet_method: Some("spark".to_string()),
            status: Nip90PaymentFactStatus::SellerWalletSettled,
            settlement_authority: "spark".to_string(),
            request_published_at: Some(100),
            result_observed_at: Some(110),
            invoice_observed_at: Some(111),
            buyer_payment_pointer_at: Some(120),
            seller_settlement_feedback_at: Some(130),
            buyer_wallet_confirmed_at: Some(140),
            seller_wallet_confirmed_at: Some(150),
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality,
        }
    }

    fn test_hop(
        request_id: &str,
        event_id: &str,
        hop_kind: Nip90RelayHopKind,
        relay_url: &str,
        accepted: bool,
        observed_at: u64,
    ) -> Nip90RelayHop {
        Nip90RelayHop {
            request_id: request_id.to_string(),
            event_id: event_id.to_string(),
            hop_kind,
            relay_url: relay_url.to_string(),
            direction: match hop_kind {
                Nip90RelayHopKind::PublishAccepted | Nip90RelayHopKind::PublishRejected => {
                    Nip90RelayHopDirection::Outbound
                }
                Nip90RelayHopKind::RequestIngress
                | Nip90RelayHopKind::ResultIngress
                | Nip90RelayHopKind::InvoiceIngress => Nip90RelayHopDirection::Inbound,
            },
            accepted,
            observed_at,
        }
    }
}
