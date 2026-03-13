use std::collections::BTreeMap;

use wgpui::components::hud::{DotShape, DotsGrid, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::state::nip90_payment_facts::{Nip90PaymentFact, Nip90PaymentFactLedgerState};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 68.0;

pub fn paint(
    content_bounds: Bounds,
    payment_facts: &Nip90PaymentFactLedgerState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "stream.nip90_payment_facts.v1", paint);
    let Some(view) = build_view(payment_facts) else {
        paint_empty_state(content_bounds, payment_facts, paint);
        return;
    };

    let header_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + PADDING,
        content_bounds.size.width - PADDING * 2.0,
        HEADER_HEIGHT,
    );
    let atlas_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width * 0.68,
        content_bounds.size.height - HEADER_HEIGHT - 28.0,
    );
    let detail_bounds = Bounds::new(
        atlas_bounds.max_x() + 10.0,
        atlas_bounds.origin.y,
        content_bounds.max_x() - atlas_bounds.max_x() - 22.0,
        atlas_bounds.size.height,
    );

    paint_header(header_bounds, &view, paint);
    paint_atlas(atlas_bounds, &view, paint);
    paint_detail(detail_bounds, &view, paint);
}

#[derive(Clone, Debug, PartialEq)]
struct SettlementAtlasView {
    buyers: Vec<AtlasNode>,
    providers: Vec<AtlasNode>,
    edges: Vec<AtlasEdge>,
    focus: AtlasEdge,
    degraded_fact_count: usize,
    total_volume_sats: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct AtlasNode {
    pubkey: String,
    label: String,
    total_sats: u64,
    edge_count: usize,
    is_local: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct AtlasEdge {
    buyer_pubkey: String,
    buyer_label: String,
    provider_pubkey: String,
    provider_label: String,
    latest_request_id: String,
    volume_sats: u64,
    fact_count: usize,
    latest_at: u64,
    avg_latency_seconds: Option<u64>,
    status_label: String,
    degraded: bool,
}

fn build_view(payment_facts: &Nip90PaymentFactLedgerState) -> Option<SettlementAtlasView> {
    build_view_from_facts(payment_facts.facts.as_slice())
}

fn build_view_from_facts(facts: &[Nip90PaymentFact]) -> Option<SettlementAtlasView> {
    let mut edges_by_pair = BTreeMap::<(String, String), AtlasEdge>::new();
    let mut buyer_totals = BTreeMap::<String, (u64, usize)>::new();
    let mut provider_totals = BTreeMap::<String, (u64, usize)>::new();
    let mut degraded_fact_count = 0usize;

    for fact in facts {
        let buyer = fact
            .buyer_nostr_pubkey
            .as_deref()
            .map(normalize_key)
            .filter(|value| !value.is_empty());
        let provider = fact
            .provider_nostr_pubkey
            .as_deref()
            .map(normalize_key)
            .filter(|value| !value.is_empty());
        let amount_sats = fact.amount_sats.unwrap_or_default();
        let latest_at = fact.latest_event_epoch_seconds().unwrap_or_default();
        let latency = fact
            .seller_wallet_confirmed_at
            .zip(fact.seller_settlement_feedback_at)
            .map(|(confirmed_at, settled_at)| confirmed_at.saturating_sub(settled_at));

        let (Some(buyer), Some(provider)) = (buyer, provider) else {
            degraded_fact_count = degraded_fact_count.saturating_add(1);
            continue;
        };

        buyer_totals
            .entry(buyer.clone())
            .and_modify(|entry| {
                entry.0 = entry.0.saturating_add(amount_sats);
                entry.1 = entry.1.saturating_add(1);
            })
            .or_insert((amount_sats, 1));
        provider_totals
            .entry(provider.clone())
            .and_modify(|entry| {
                entry.0 = entry.0.saturating_add(amount_sats);
                entry.1 = entry.1.saturating_add(1);
            })
            .or_insert((amount_sats, 1));

        let entry = edges_by_pair
            .entry((buyer.clone(), provider.clone()))
            .or_insert_with(|| AtlasEdge {
                buyer_pubkey: buyer.clone(),
                buyer_label: compact_identity(Some(buyer.as_str())),
                provider_pubkey: provider.clone(),
                provider_label: compact_identity(Some(provider.as_str())),
                latest_request_id: fact.request_id.clone(),
                volume_sats: 0,
                fact_count: 0,
                latest_at,
                avg_latency_seconds: latency,
                status_label: fact.status.label().to_string(),
                degraded: matches!(
                    fact.status,
                    crate::state::nip90_payment_facts::Nip90PaymentFactStatus::Failed
                ),
            });
        entry.volume_sats = entry.volume_sats.saturating_add(amount_sats);
        entry.fact_count = entry.fact_count.saturating_add(1);
        if latest_at >= entry.latest_at {
            entry.latest_at = latest_at;
            entry.latest_request_id = fact.request_id.clone();
            entry.status_label = fact.status.label().to_string();
        }
        entry.degraded |= matches!(
            fact.status,
            crate::state::nip90_payment_facts::Nip90PaymentFactStatus::Failed
        );
        entry.avg_latency_seconds = merge_avg_latency(entry.avg_latency_seconds, latency);
    }

    let mut edges = edges_by_pair.into_values().collect::<Vec<_>>();
    if edges.is_empty() {
        return None;
    }
    edges.sort_by(|left, right| {
        right
            .volume_sats
            .cmp(&left.volume_sats)
            .then_with(|| right.latest_at.cmp(&left.latest_at))
            .then_with(|| left.buyer_pubkey.cmp(&right.buyer_pubkey))
    });
    let focus = edges.first().cloned()?;

    let mut buyers = buyer_totals
        .into_iter()
        .map(|(pubkey, (total_sats, edge_count))| AtlasNode {
            label: compact_identity(Some(pubkey.as_str())),
            pubkey,
            total_sats,
            edge_count,
            is_local: false,
        })
        .collect::<Vec<_>>();
    buyers.sort_by(|left, right| {
        right
            .total_sats
            .cmp(&left.total_sats)
            .then_with(|| left.pubkey.cmp(&right.pubkey))
    });

    let mut providers = provider_totals
        .into_iter()
        .map(|(pubkey, (total_sats, edge_count))| AtlasNode {
            label: compact_identity(Some(pubkey.as_str())),
            pubkey,
            total_sats,
            edge_count,
            is_local: false,
        })
        .collect::<Vec<_>>();
    providers.sort_by(|left, right| {
        right
            .total_sats
            .cmp(&left.total_sats)
            .then_with(|| left.pubkey.cmp(&right.pubkey))
    });

    Some(SettlementAtlasView {
        total_volume_sats: edges.iter().map(|edge| edge.volume_sats).sum(),
        degraded_fact_count,
        buyers,
        providers,
        focus,
        edges,
    })
}

fn merge_avg_latency(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(left), Some(right)) => Some((left + right) / 2),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn paint_header(bounds: Bounds, view: &SettlementAtlasView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "SETTLEMENT ATLAS  //  BUYER TO PROVIDER FLOWS",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Edges are app-owned NIP-90 payment facts: volume controls thickness, recency controls glow, and degraded identity proof is counted explicitly.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "buyers={}  providers={}  edges={}  volume={}  degraded_hidden={}",
                view.buyers.len(),
                view.providers.len(),
                view.edges.len(),
                format_sats_amount(view.total_volume_sats),
                view.degraded_fact_count,
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 48.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );

    let density =
        (view.edges.len().min(7) as f32 / 7.0).max(view.degraded_fact_count.min(7) as f32 / 7.0);
    let mut meter = SignalMeter::new()
        .bars(7)
        .gap(2.0)
        .level(density)
        .min_bar_height(0.15)
        .active_color(Hsla::from_hex(0x6ed0ff).with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.48));
    meter.paint(
        Bounds::new(bounds.max_x() - 50.0, bounds.origin.y + 6.0, 38.0, 40.0),
        paint,
    );
}

fn paint_atlas(bounds: Bounds, view: &SettlementAtlasView, paint: &mut PaintContext) {
    paint_panel_shell(
        bounds,
        "Constellation",
        Hsla::from_hex(0x6ed0ff).with_alpha(0.82),
        paint,
    );

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

    let left_x = field_bounds.origin.x + 40.0;
    let right_x = field_bounds.max_x() - 40.0;
    let mid_x = (left_x + right_x) * 0.5;
    let max_edge_volume = view
        .edges
        .iter()
        .map(|edge| edge.volume_sats)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_edge_time = view
        .edges
        .iter()
        .map(|edge| edge.latest_at)
        .max()
        .unwrap_or(1)
        .max(1);

    let buyer_positions = view
        .buyers
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, buyer)| {
            let y = vertical_slot(field_bounds, index, view.buyers.len().min(8));
            (buyer.pubkey.clone(), Point::new(left_x, y))
        })
        .collect::<BTreeMap<_, _>>();
    let provider_positions = view
        .providers
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, provider)| {
            let y = vertical_slot(field_bounds, index, view.providers.len().min(8));
            (provider.pubkey.clone(), Point::new(right_x, y))
        })
        .collect::<BTreeMap<_, _>>();

    for edge in &view.edges {
        let (Some(from), Some(to)) = (
            buyer_positions.get(edge.buyer_pubkey.as_str()),
            provider_positions.get(edge.provider_pubkey.as_str()),
        ) else {
            continue;
        };
        let thickness = 2.0 + (edge.volume_sats as f32 / max_edge_volume as f32) * 7.0;
        let glow = 0.2 + (edge.latest_at as f32 / max_edge_time as f32) * 0.8;
        let accent = if edge.degraded {
            Hsla::from_hex(0xff8b6e)
        } else {
            Hsla::from_hex(0x6ed0ff)
        }
        .with_alpha(glow.clamp(0.18, 0.96));
        let control = Point::new(
            mid_x,
            (from.y + to.y) * 0.5 - ((to.y - from.y).abs() * 0.22 + 18.0),
        );
        paint_edge_curve(*from, control, *to, thickness, accent, paint);
    }

    for buyer in view.buyers.iter().take(8) {
        if let Some(position) = buyer_positions.get(buyer.pubkey.as_str()) {
            paint_node(position, buyer, true, view.total_volume_sats, paint);
        }
    }
    for provider in view.providers.iter().take(8) {
        if let Some(position) = provider_positions.get(provider.pubkey.as_str()) {
            paint_node(position, provider, false, view.total_volume_sats, paint);
        }
    }
}

fn paint_detail(bounds: Bounds, view: &SettlementAtlasView, paint: &mut PaintContext) {
    paint_panel_shell(
        bounds,
        "Focus Edge",
        Hsla::from_hex(0x68f0b8).with_alpha(0.82),
        paint,
    );
    let focus = &view.focus;

    paint.scene.draw_text(paint.text.layout_mono(
        format!("{}  ->  {}", focus.buyer_label, focus.provider_label).as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "This card is the drill anchor for the underlying request/payment evidence behind the brightest current edge.",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 50.0),
        9.0,
        theme::text::MUTED,
    ));

    let mut y = bounds.origin.y + 80.0;
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
        focus.provider_pubkey.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "volume",
        format_sats_amount(focus.volume_sats).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "facts",
        focus.fact_count.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "latency",
        focus
            .avg_latency_seconds
            .map(|seconds| format!("{seconds}s"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
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
        "request",
        focus.latest_request_id.as_str(),
    );
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "drill",
        "spark_replay / settlement_ladder / key_ledger",
    );

    paint.scene.draw_text(paint.text.layout(
        &format!(
            "Hidden degraded facts without both buyer/provider Nostr keys: {}",
            view.degraded_fact_count
        ),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 18.0),
        9.5,
        theme::text::MUTED,
    ));
}

fn paint_node(
    position: &Point,
    node: &AtlasNode,
    buyer_side: bool,
    total_volume_sats: u64,
    paint: &mut PaintContext,
) {
    let share = (node.total_sats as f32 / total_volume_sats.max(1) as f32).clamp(0.08, 1.0);
    let radius = 8.0 + share * 16.0;
    let accent = if buyer_side {
        Hsla::from_hex(0x6ed0ff)
    } else {
        Hsla::from_hex(0x68f0b8)
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            position.x - radius,
            position.y - radius,
            radius * 2.0,
            radius * 2.0,
        ))
        .with_background(accent.with_alpha(0.24))
        .with_border(accent.with_alpha(0.66), 1.0)
        .with_corner_radius(radius),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(position.x - 4.0, position.y - 4.0, 8.0, 8.0))
            .with_background(accent.with_alpha(0.96))
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        node.label.as_str(),
        Point::new(
            if buyer_side {
                position.x - radius - 72.0
            } else {
                position.x + radius + 6.0
            },
            position.y - 6.0,
        ),
        9.0,
        theme::text::PRIMARY,
    ));
}

fn paint_edge_curve(
    from: Point,
    control: Point,
    to: Point,
    thickness: f32,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let segments = 18usize;
    let dot_size = thickness.max(2.0) + 1.5;
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        let current = quadratic_point(from, control, to, t);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                current.x - dot_size * 0.5,
                current.y - dot_size * 0.5,
                dot_size,
                dot_size,
            ))
            .with_background(accent.with_alpha(0.18 + 0.82 * t))
            .with_corner_radius(dot_size * 0.5),
        );
    }
}

fn quadratic_point(from: Point, control: Point, to: Point, t: f32) -> Point {
    let inv = 1.0 - t;
    Point::new(
        inv * inv * from.x + 2.0 * inv * t * control.x + t * t * to.x,
        inv * inv * from.y + 2.0 * inv * t * control.y + t * t * to.y,
    )
}

fn vertical_slot(bounds: Bounds, index: usize, count: usize) -> f32 {
    if count <= 1 {
        return bounds.origin.y + bounds.size.height * 0.5;
    }
    let gap = bounds.size.height / count as f32;
    bounds.origin.y + gap * (index as f32 + 0.5)
}

fn paint_empty_state(
    content_bounds: Bounds,
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
        "SETTLEMENT ATLAS",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No buyer/provider payment edges are available yet. This pane needs canonical Nostr identities on both sides of a fact before it will draw a graph edge.",
        Point::new(content_bounds.origin.x + 24.0, content_bounds.origin.y + 88.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "ledger_state={}  last_action={}",
                payment_facts.load_state.label(),
                payment_facts.last_action.as_deref().unwrap_or("none")
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

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn compact_identity(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "unknown".to_string();
    };
    if value.len() <= 18 {
        value.to_string()
    } else {
        format!("{}..{}", &value[..10], &value[value.len() - 6..])
    }
}

#[cfg(test)]
mod tests {
    use super::build_view_from_facts;
    use crate::state::nip90_payment_facts::{
        Nip90PaymentFact, Nip90PaymentFactSourceQuality, Nip90PaymentFactStatus,
    };

    #[test]
    fn settlement_atlas_aggregates_edges_by_buyer_provider_pair() {
        let view = build_view_from_facts(&[
            fact("req-1", Some("npub1buyer"), Some("npub1provider"), 21),
            fact("req-2", Some("npub1buyer"), Some("npub1provider"), 34),
        ])
        .expect("atlas should exist");

        assert_eq!(view.edges.len(), 1);
        assert_eq!(view.focus.volume_sats, 55);
        assert_eq!(view.focus.fact_count, 2);
        assert_eq!(view.buyers.len(), 1);
        assert_eq!(view.providers.len(), 1);
    }

    #[test]
    fn settlement_atlas_counts_degraded_facts_without_both_nostr_actors() {
        let view = build_view_from_facts(&[
            fact("req-good", Some("npub1buyer"), Some("npub1provider"), 21),
            fact("req-orphan", Some("npub1buyer"), None, 9),
        ])
        .expect("atlas should exist");

        assert_eq!(view.edges.len(), 1);
        assert_eq!(view.degraded_fact_count, 1);
    }

    fn fact(
        request_id: &str,
        buyer_nostr_pubkey: Option<&str>,
        provider_nostr_pubkey: Option<&str>,
        amount_sats: u64,
    ) -> Nip90PaymentFact {
        Nip90PaymentFact {
            fact_id: format!("fact-{request_id}"),
            request_id: request_id.to_string(),
            request_type: "kind5050".to_string(),
            request_event_id: Some(format!("request-{request_id}")),
            result_event_id: Some(format!("result-{request_id}")),
            invoice_event_id: Some(format!("invoice-{request_id}")),
            seller_feedback_event_id: Some(format!("seller-{request_id}")),
            buyer_nostr_pubkey: buyer_nostr_pubkey.map(ToString::to_string),
            provider_nostr_pubkey: provider_nostr_pubkey.map(ToString::to_string),
            invoice_provider_pubkey: provider_nostr_pubkey.map(ToString::to_string),
            result_provider_pubkey: provider_nostr_pubkey.map(ToString::to_string),
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: Some(format!("pointer-{request_id}")),
            seller_payment_pointer: Some(format!("seller-{request_id}")),
            buyer_payment_hash: Some(format!("hash-{request_id}")),
            amount_sats: Some(amount_sats),
            fees_sats: Some(1),
            total_debit_sats: Some(amount_sats),
            wallet_method: Some("lightning".to_string()),
            status: Nip90PaymentFactStatus::SellerWalletSettled,
            settlement_authority: "wallet.reconciliation".to_string(),
            request_published_at: Some(10),
            result_observed_at: Some(20),
            invoice_observed_at: Some(24),
            buyer_payment_pointer_at: Some(26),
            seller_settlement_feedback_at: Some(30),
            buyer_wallet_confirmed_at: Some(32),
            seller_wallet_confirmed_at: Some(40),
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality: Nip90PaymentFactSourceQuality::SellerWalletReconciled,
        }
    }
}
