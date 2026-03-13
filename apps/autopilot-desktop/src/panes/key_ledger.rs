use wgpui::components::hud::{DotShape, DotsGrid, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::state::nip90_payment_facts::{
    Nip90Actor, Nip90ActorNamespace, Nip90PaymentFact, Nip90PaymentFactLedgerState,
    Nip90PaymentFactStatus,
};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 68.0;
const ROW_HEIGHT: f32 = 24.0;
const SPARK_POINTS: usize = 8;

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
    let nostr_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width * 0.64,
        content_bounds.size.height - HEADER_HEIGHT - 28.0,
    );
    let detail_bounds = Bounds::new(
        nostr_bounds.max_x() + 10.0,
        nostr_bounds.origin.y,
        content_bounds.max_x() - nostr_bounds.max_x() - 22.0,
        nostr_bounds.size.height * 0.56,
    );
    let lightning_bounds = Bounds::new(
        detail_bounds.origin.x,
        detail_bounds.max_y() + 10.0,
        detail_bounds.size.width,
        content_bounds.max_y() - detail_bounds.max_y() - 20.0,
    );

    paint_header(header_bounds, &view, paint);
    paint_actor_table(nostr_bounds, "Nostr Actors", &view.nostr_rows, paint);
    paint_detail(detail_bounds, &view.focus, &view, paint);
    paint_actor_table(
        lightning_bounds,
        "Lightning Destinations",
        &view.lightning_rows,
        paint,
    );
}

#[derive(Clone, Debug, PartialEq)]
struct KeyLedgerView {
    focus: KeyLedgerRow,
    nostr_rows: Vec<KeyLedgerRow>,
    lightning_rows: Vec<KeyLedgerRow>,
    total_sent_sats: u64,
    total_received_sats: u64,
    settlement_failure_count: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct KeyLedgerRow {
    actor_id: String,
    namespace: Nip90ActorNamespace,
    display_label: String,
    identity: String,
    role_label: String,
    is_local: bool,
    sent_sats: u64,
    received_sats: u64,
    jobs_won: u64,
    invoices_emitted: u64,
    settlement_failures: u64,
    avg_latency_seconds: Option<u64>,
    sparkline_points: Vec<u64>,
}

fn build_view(payment_facts: &Nip90PaymentFactLedgerState) -> Option<KeyLedgerView> {
    build_view_from_parts(
        payment_facts.actors.as_slice(),
        payment_facts.facts.as_slice(),
    )
}

fn build_view_from_parts(
    actors: &[Nip90Actor],
    facts: &[Nip90PaymentFact],
) -> Option<KeyLedgerView> {
    let mut rows = actors
        .iter()
        .map(|actor| build_row(actor, facts))
        .filter(|row| {
            row.sent_sats > 0
                || row.received_sats > 0
                || row.jobs_won > 0
                || row.invoices_emitted > 0
                || row.settlement_failures > 0
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }

    rows.sort_by(|left, right| {
        right
            .received_sats
            .cmp(&left.received_sats)
            .then_with(|| right.sent_sats.cmp(&left.sent_sats))
            .then_with(|| right.jobs_won.cmp(&left.jobs_won))
            .then_with(|| left.display_label.cmp(&right.display_label))
    });
    let focus = rows.first().cloned()?;
    let total_sent_sats = rows.iter().map(|row| row.sent_sats).sum();
    let total_received_sats = rows.iter().map(|row| row.received_sats).sum();
    let settlement_failure_count = rows.iter().map(|row| row.settlement_failures).sum();

    let mut nostr_rows = rows
        .iter()
        .filter(|row| row.namespace == Nip90ActorNamespace::Nostr)
        .cloned()
        .collect::<Vec<_>>();
    let mut lightning_rows = rows
        .iter()
        .filter(|row| row.namespace == Nip90ActorNamespace::LightningDestination)
        .cloned()
        .collect::<Vec<_>>();
    nostr_rows.sort_by(row_sort_order);
    lightning_rows.sort_by(row_sort_order);

    Some(KeyLedgerView {
        focus,
        nostr_rows,
        lightning_rows,
        total_sent_sats,
        total_received_sats,
        settlement_failure_count,
    })
}

fn row_sort_order(left: &KeyLedgerRow, right: &KeyLedgerRow) -> std::cmp::Ordering {
    right
        .received_sats
        .cmp(&left.received_sats)
        .then_with(|| right.sent_sats.cmp(&left.sent_sats))
        .then_with(|| right.jobs_won.cmp(&left.jobs_won))
        .then_with(|| right.invoices_emitted.cmp(&left.invoices_emitted))
        .then_with(|| left.display_label.cmp(&right.display_label))
}

fn build_row(actor: &Nip90Actor, facts: &[Nip90PaymentFact]) -> KeyLedgerRow {
    let actor_key = normalize_key(actor.pubkey.as_str());
    let mut sent_sats = 0u64;
    let mut received_sats = 0u64;
    let mut jobs_won = 0u64;
    let mut invoices_emitted = 0u64;
    let mut settlement_failures = 0u64;
    let mut latencies = Vec::<u64>::new();
    let mut sparkline = Vec::<(u64, u64)>::new();

    for fact in facts {
        let amount_sats = fact.amount_sats.unwrap_or_default();
        let buyer_match = identity_matches(
            fact.buyer_nostr_pubkey.as_deref(),
            actor.namespace,
            actor_key.as_str(),
        );
        let provider_match = identity_matches(
            fact.provider_nostr_pubkey.as_deref(),
            actor.namespace,
            actor_key.as_str(),
        );
        let invoice_provider_match = identity_matches(
            fact.invoice_provider_pubkey.as_deref(),
            actor.namespace,
            actor_key.as_str(),
        );
        let lightning_match = identity_matches(
            fact.lightning_destination_pubkey.as_deref(),
            actor.namespace,
            actor_key.as_str(),
        );
        let involved = buyer_match || provider_match || invoice_provider_match || lightning_match;
        if !involved {
            continue;
        }

        if buyer_match {
            sent_sats = sent_sats.saturating_add(fact.total_debit_sats.unwrap_or(amount_sats));
            if let Some(latency) = fact
                .buyer_wallet_confirmed_at
                .zip(fact.buyer_payment_pointer_at)
                .map(|(confirmed_at, pointer_at)| confirmed_at.saturating_sub(pointer_at))
            {
                latencies.push(latency);
            }
        }
        if provider_match {
            received_sats = received_sats.saturating_add(amount_sats);
            if fact.result_event_id.is_some() {
                jobs_won = jobs_won.saturating_add(1);
            }
            if let Some(latency) = fact
                .seller_wallet_confirmed_at
                .zip(fact.seller_settlement_feedback_at)
                .map(|(confirmed_at, settled_at)| confirmed_at.saturating_sub(settled_at))
            {
                latencies.push(latency);
            }
        }
        if invoice_provider_match
            && (fact.invoice_event_id.is_some() || fact.invoice_observed_at.is_some())
        {
            invoices_emitted = invoices_emitted.saturating_add(1);
        }
        if lightning_match {
            received_sats = received_sats.saturating_add(amount_sats);
        }
        if fact.status == Nip90PaymentFactStatus::Failed {
            settlement_failures = settlement_failures.saturating_add(1);
        }

        if let Some(epoch) = fact.latest_event_epoch_seconds() {
            sparkline.push((epoch, amount_sats));
        }
    }

    sparkline.sort_by(|left, right| left.0.cmp(&right.0));
    let mut sparkline_points = sparkline
        .into_iter()
        .rev()
        .take(SPARK_POINTS)
        .collect::<Vec<_>>();
    sparkline_points.reverse();
    let sparkline_points = sparkline_points
        .into_iter()
        .map(|(_, amount)| amount)
        .collect::<Vec<_>>();

    KeyLedgerRow {
        actor_id: actor.actor_id.clone(),
        namespace: actor.namespace,
        display_label: actor.display_label.clone(),
        identity: actor.pubkey.clone(),
        role_label: derive_role_label(
            actor.namespace,
            sent_sats,
            received_sats,
            jobs_won,
            invoices_emitted,
        ),
        is_local: actor.is_local,
        sent_sats,
        received_sats,
        jobs_won,
        invoices_emitted,
        settlement_failures,
        avg_latency_seconds: if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<u64>() / latencies.len() as u64)
        },
        sparkline_points,
    }
}

fn derive_role_label(
    namespace: Nip90ActorNamespace,
    sent_sats: u64,
    received_sats: u64,
    jobs_won: u64,
    invoices_emitted: u64,
) -> String {
    match namespace {
        Nip90ActorNamespace::LightningDestination => "lightning-destination".to_string(),
        Nip90ActorNamespace::Nostr => {
            if received_sats > 0 || jobs_won > 0 {
                "provider".to_string()
            } else if sent_sats > 0 {
                "buyer".to_string()
            } else if invoices_emitted > 0 {
                "invoice-emitter".to_string()
            } else {
                "participant".to_string()
            }
        }
    }
}

fn identity_matches(value: Option<&str>, namespace: Nip90ActorNamespace, actor_key: &str) -> bool {
    value
        .map(normalize_key)
        .is_some_and(|candidate| candidate == actor_key && namespace_matches(value, namespace))
}

fn namespace_matches(value: Option<&str>, namespace: Nip90ActorNamespace) -> bool {
    match namespace {
        Nip90ActorNamespace::Nostr => value.is_some(),
        Nip90ActorNamespace::LightningDestination => value.is_some(),
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn paint_header(bounds: Bounds, view: &KeyLedgerView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "KEY LEDGER  //  PER-ACTOR PAYMENT ACTIVITY",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Rows stay namespace-aware: Nostr actors and Lightning destinations do not share an ambiguous table.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "window=all_facts  sent={}  received={}  failures={}  focus={}",
                format_sats_amount(view.total_sent_sats),
                format_sats_amount(view.total_received_sats),
                view.settlement_failure_count,
                view.focus.display_label,
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 48.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );

    let total_rows = (view.nostr_rows.len() + view.lightning_rows.len()).max(1) as f32;
    let nostr_ratio = view.nostr_rows.len() as f32 / total_rows;
    let mut meter = SignalMeter::new()
        .bars(7)
        .gap(2.0)
        .level(nostr_ratio)
        .min_bar_height(0.15)
        .active_color(Hsla::from_hex(0x62d5ff).with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.48));
    meter.paint(
        Bounds::new(bounds.max_x() - 50.0, bounds.origin.y + 6.0, 38.0, 40.0),
        paint,
    );
}

fn paint_actor_table(bounds: Bounds, title: &str, rows: &[KeyLedgerRow], paint: &mut PaintContext) {
    let accent = if title.contains("Lightning") {
        Hsla::from_hex(0xffd463)
    } else {
        Hsla::from_hex(0x62d5ff)
    };
    paint_panel_shell(bounds, title, accent, paint);

    if rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No actors yet for this namespace.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    let field_bounds = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 28.0,
        bounds.size.width - 20.0,
        bounds.size.height - 38.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(0.9)
        .color(accent.with_alpha(0.12))
        .animation_progress(1.0);
    dots.paint(field_bounds, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        "actor                role         recv     sent     wins inv fail avg   spark",
        Point::new(field_bounds.origin.x + 6.0, field_bounds.origin.y + 8.0),
        8.8,
        accent.with_alpha(0.96),
    ));

    for (index, row) in rows
        .iter()
        .take(((field_bounds.size.height - 22.0) / ROW_HEIGHT) as usize)
        .enumerate()
    {
        let row_bounds = Bounds::new(
            field_bounds.origin.x,
            field_bounds.origin.y + 18.0 + index as f32 * ROW_HEIGHT,
            field_bounds.size.width,
            ROW_HEIGHT - 2.0,
        );
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if index % 2 == 0 {
                    Hsla::from_hex(0x0a1118).with_alpha(0.88)
                } else {
                    Hsla::from_hex(0x071018).with_alpha(0.84)
                })
                .with_corner_radius(6.0),
        );
        paint.scene.draw_text(
            paint.text.layout_mono(
                format!(
                    "{:<20} {:<12} {:>7} {:>8} {:>4} {:>3} {:>4} {:>5}",
                    short_label(row.display_label.as_str(), 20),
                    short_label(row.role_label.as_str(), 12),
                    short_amount(row.received_sats),
                    short_amount(row.sent_sats),
                    row.jobs_won,
                    row.invoices_emitted,
                    row.settlement_failures,
                    row.avg_latency_seconds
                        .map(|seconds| format!("{seconds}s"))
                        .unwrap_or_else(|| "-".to_string())
                )
                .as_str(),
                Point::new(row_bounds.origin.x + 6.0, row_bounds.origin.y + 7.0),
                8.6,
                if row.is_local {
                    accent.with_alpha(0.96)
                } else {
                    theme::text::PRIMARY
                },
            ),
        );
        paint_sparkline(
            Bounds::new(
                row_bounds.max_x() - 74.0,
                row_bounds.origin.y + 5.0,
                62.0,
                12.0,
            ),
            row.sparkline_points.as_slice(),
            accent,
            paint,
        );
    }
}

fn paint_detail(
    bounds: Bounds,
    focus: &KeyLedgerRow,
    view: &KeyLedgerView,
    paint: &mut PaintContext,
) {
    paint_panel_shell(
        bounds,
        "Focus Actor",
        Hsla::from_hex(0x68f0b8).with_alpha(0.82),
        paint,
    );

    paint.scene.draw_text(paint.text.layout_mono(
        format!("{}  {}", focus.display_label, focus.namespace.label()).as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Drill path: Settlement Atlas, Spark Replay, and Relay Choreography can all pivot on this actor once those panes are open.",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 50.0),
        9.0,
        theme::text::MUTED,
    ));

    let mut y = bounds.origin.y + 78.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "identity",
        focus.identity.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "role",
        focus.role_label.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "received",
        format_sats_amount(focus.received_sats).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "sent",
        format_sats_amount(focus.sent_sats).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "jobs_won",
        focus.jobs_won.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "invoices",
        focus.invoices_emitted.to_string().as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "failures",
        focus.settlement_failures.to_string().as_str(),
    );
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "avg_latency",
        focus
            .avg_latency_seconds
            .map(|seconds| format!("{seconds}s"))
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );

    paint.scene.draw_text(paint.text.layout(
        &format!(
            "row_count nostr={} lightning={}  time_window=all_facts",
            view.nostr_rows.len(),
            view.lightning_rows.len()
        ),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 18.0),
        9.5,
        theme::text::MUTED,
    ));
}

fn paint_sparkline(bounds: Bounds, points: &[u64], accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x050a0f).with_alpha(0.9))
            .with_corner_radius(4.0),
    );
    if points.is_empty() {
        return;
    }
    let max_value = points.iter().copied().max().unwrap_or(1).max(1) as f32;
    let bar_width = (bounds.size.width / points.len() as f32).max(3.0);
    for (index, point) in points.iter().enumerate() {
        let height_ratio = (*point as f32 / max_value).clamp(0.1, 1.0);
        let bar_height = height_ratio * (bounds.size.height - 2.0);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + index as f32 * bar_width + 1.0,
                bounds.max_y() - bar_height - 1.0,
                (bar_width - 2.0).max(1.0),
                bar_height,
            ))
            .with_background(accent.with_alpha(0.84))
            .with_corner_radius(2.0),
        );
    }
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
        "KEY LEDGER",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No actor metrics yet. This pane activates once payment facts produce actor rows in the app-owned ledger.",
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

fn short_amount(value: u64) -> String {
    if value >= 1_000 {
        format!("{}k", value / 1_000)
    } else {
        value.to_string()
    }
}

fn short_label(value: &str, width: usize) -> String {
    if value.len() <= width {
        value.to_string()
    } else {
        format!("{}..{}", &value[..width - 4], &value[value.len() - 2..])
    }
}

#[cfg(test)]
mod tests {
    use super::build_view_from_parts;
    use crate::state::nip90_payment_facts::{
        Nip90Actor, Nip90ActorNamespace, Nip90PaymentFact, Nip90PaymentFactSourceQuality,
        Nip90PaymentFactStatus,
    };

    #[test]
    fn key_ledger_aggregates_actor_totals_by_namespace() {
        let view = build_view_from_parts(
            &[
                actor(
                    "actor:nostr:buyer",
                    Nip90ActorNamespace::Nostr,
                    "npub1buyer",
                ),
                actor(
                    "actor:nostr:provider",
                    Nip90ActorNamespace::Nostr,
                    "npub1provider",
                ),
                actor(
                    "actor:lightning:dest",
                    Nip90ActorNamespace::LightningDestination,
                    "ln-dest-1",
                ),
            ],
            &[fact(
                "req-1",
                Some("npub1buyer"),
                Some("npub1provider"),
                Some("npub1provider"),
                Some("ln-dest-1"),
                21,
                Some(30),
                Some(45),
            )],
        )
        .expect("view should exist");

        let buyer = view
            .nostr_rows
            .iter()
            .find(|row| row.identity == "npub1buyer")
            .expect("buyer row");
        let provider = view
            .nostr_rows
            .iter()
            .find(|row| row.identity == "npub1provider")
            .expect("provider row");
        let lightning = view
            .lightning_rows
            .iter()
            .find(|row| row.identity == "ln-dest-1")
            .expect("lightning row");

        assert_eq!(buyer.sent_sats, 21);
        assert_eq!(provider.received_sats, 21);
        assert_eq!(provider.jobs_won, 1);
        assert_eq!(provider.invoices_emitted, 1);
        assert_eq!(provider.avg_latency_seconds, Some(15));
        assert_eq!(lightning.received_sats, 21);
    }

    #[test]
    fn key_ledger_counts_settlement_failures_and_splits_namespaces() {
        let view = build_view_from_parts(
            &[
                actor(
                    "actor:nostr:buyer",
                    Nip90ActorNamespace::Nostr,
                    "npub1buyer",
                ),
                actor(
                    "actor:nostr:provider",
                    Nip90ActorNamespace::Nostr,
                    "npub1provider",
                ),
            ],
            &[failed_fact("req-fail", "npub1buyer", "npub1provider", 9)],
        )
        .expect("view should exist");

        assert_eq!(view.nostr_rows.len(), 2);
        assert_eq!(view.lightning_rows.len(), 0);
        assert_eq!(view.settlement_failure_count, 2);
        assert!(
            view.nostr_rows
                .iter()
                .all(|row| row.namespace == Nip90ActorNamespace::Nostr)
        );
    }

    fn actor(actor_id: &str, namespace: Nip90ActorNamespace, pubkey: &str) -> Nip90Actor {
        Nip90Actor {
            actor_id: actor_id.to_string(),
            namespace,
            pubkey: pubkey.to_string(),
            display_label: pubkey.to_string(),
            is_local: false,
            role_mask: 0,
        }
    }

    fn fact(
        request_id: &str,
        buyer_nostr_pubkey: Option<&str>,
        provider_nostr_pubkey: Option<&str>,
        invoice_provider_pubkey: Option<&str>,
        lightning_destination_pubkey: Option<&str>,
        amount_sats: u64,
        seller_settlement_feedback_at: Option<u64>,
        seller_wallet_confirmed_at: Option<u64>,
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
            invoice_provider_pubkey: invoice_provider_pubkey.map(ToString::to_string),
            result_provider_pubkey: provider_nostr_pubkey.map(ToString::to_string),
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: lightning_destination_pubkey.map(ToString::to_string),
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
            invoice_observed_at: Some(25),
            buyer_payment_pointer_at: Some(28),
            seller_settlement_feedback_at,
            buyer_wallet_confirmed_at: Some(32),
            seller_wallet_confirmed_at,
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality: Nip90PaymentFactSourceQuality::SellerWalletReconciled,
        }
    }

    fn failed_fact(
        request_id: &str,
        buyer_nostr_pubkey: &str,
        provider_nostr_pubkey: &str,
        amount_sats: u64,
    ) -> Nip90PaymentFact {
        Nip90PaymentFact {
            status: Nip90PaymentFactStatus::Failed,
            seller_feedback_event_id: None,
            buyer_wallet_confirmed_at: None,
            seller_wallet_confirmed_at: None,
            seller_settlement_feedback_at: None,
            buyer_payment_pointer_at: None,
            invoice_event_id: None,
            invoice_observed_at: None,
            result_event_id: None,
            result_observed_at: None,
            request_event_id: Some(format!("request-{request_id}")),
            request_published_at: Some(10),
            buyer_nostr_pubkey: Some(buyer_nostr_pubkey.to_string()),
            provider_nostr_pubkey: Some(provider_nostr_pubkey.to_string()),
            invoice_provider_pubkey: None,
            result_provider_pubkey: None,
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: None,
            seller_payment_pointer: None,
            buyer_payment_hash: None,
            amount_sats: Some(amount_sats),
            fees_sats: None,
            total_debit_sats: Some(amount_sats),
            wallet_method: None,
            settlement_authority: "unknown".to_string(),
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality: Nip90PaymentFactSourceQuality::RequestProjection,
            fact_id: format!("fact-{request_id}"),
            request_id: request_id.to_string(),
            request_type: "kind5050".to_string(),
        }
    }
}
