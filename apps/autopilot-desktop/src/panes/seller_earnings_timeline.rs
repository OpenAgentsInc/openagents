use chrono::{Local, TimeZone};
use wgpui::components::hud::{DotShape, DotsGrid, Scanlines, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::state::nip90_payment_facts::{
    Nip90PaymentFact, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality,
    Nip90PaymentFactStatus,
};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 72.0;
const DETAILS_WIDTH_RATIO: f32 = 0.34;
const TIMELINE_HEIGHT_RATIO: f32 = 0.6;
const MAX_SETTLED_PULSES: usize = 18;
const MAX_PENDING_ROWS: usize = 6;

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
    let split_x = header_bounds.origin.x + header_bounds.size.width * (1.0 - DETAILS_WIDTH_RATIO);
    let timeline_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 10.0,
        split_x - header_bounds.origin.x - 10.0,
        (content_bounds.size.height * TIMELINE_HEIGHT_RATIO).max(220.0),
    );
    let detail_bounds = Bounds::new(
        split_x,
        header_bounds.max_y() + 10.0,
        content_bounds.max_x() - split_x - PADDING,
        timeline_bounds.size.height,
    );
    let pending_bounds = Bounds::new(
        header_bounds.origin.x,
        timeline_bounds.max_y() + 10.0,
        header_bounds.size.width,
        content_bounds.max_y() - timeline_bounds.max_y() - 22.0,
    );

    paint_header(header_bounds, &view, paint);
    paint_timeline_panel(timeline_bounds, &view, paint);
    paint_detail_panel(detail_bounds, &view.focus, &view, paint);
    paint_pending_panel(pending_bounds, &view, paint);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SellerEarningsTimelineView {
    settled: Vec<SellerEarningsPulse>,
    pending: Vec<SellerEarningsPulse>,
    focus: SellerEarningsPulse,
    settled_sats: u64,
    pending_sats: u64,
    known_payer_count: usize,
    avg_confirmation_latency_seconds: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SellerEarningsPulse {
    request_id: String,
    request_type: String,
    payer_identity: String,
    payer_label: String,
    provider_identity: String,
    provider_label: String,
    amount_sats: u64,
    status: Nip90PaymentFactStatus,
    source_quality: Nip90PaymentFactSourceQuality,
    settlement_authority: String,
    seller_payment_pointer: Option<String>,
    result_event_id: Option<String>,
    seller_feedback_event_id: Option<String>,
    settlement_observed_at: Option<u64>,
    wallet_confirmed_at: Option<u64>,
    effective_at: u64,
    confirmation_latency_seconds: Option<u64>,
    wallet_authoritative: bool,
}

fn build_view(payment_facts: &Nip90PaymentFactLedgerState) -> Option<SellerEarningsTimelineView> {
    build_view_from_facts(payment_facts.facts.as_slice())
}

fn build_view_from_facts(facts: &[Nip90PaymentFact]) -> Option<SellerEarningsTimelineView> {
    let mut pulses = facts
        .iter()
        .filter_map(seller_pulse_from_fact)
        .collect::<Vec<_>>();
    if pulses.is_empty() {
        return None;
    }

    pulses.sort_by(|left, right| {
        right
            .effective_at
            .cmp(&left.effective_at)
            .then_with(|| right.amount_sats.cmp(&left.amount_sats))
            .then_with(|| left.request_id.cmp(&right.request_id))
    });

    let mut settled = Vec::new();
    let mut pending = Vec::new();
    for pulse in pulses {
        if pulse.wallet_authoritative {
            settled.push(pulse);
        } else {
            pending.push(pulse);
        }
    }

    settled.truncate(MAX_SETTLED_PULSES);
    pending.truncate(MAX_PENDING_ROWS);

    let focus = settled
        .first()
        .cloned()
        .or_else(|| pending.first().cloned())?;
    let settled_sats = settled.iter().map(|pulse| pulse.amount_sats).sum();
    let pending_sats = pending.iter().map(|pulse| pulse.amount_sats).sum();
    let known_payer_count = settled
        .iter()
        .chain(pending.iter())
        .filter(|pulse| pulse.payer_identity != "unknown")
        .count();
    let latency_samples = settled
        .iter()
        .filter_map(|pulse| pulse.confirmation_latency_seconds)
        .collect::<Vec<_>>();
    let avg_confirmation_latency_seconds = if latency_samples.is_empty() {
        None
    } else {
        Some(latency_samples.iter().sum::<u64>() / latency_samples.len() as u64)
    };

    Some(SellerEarningsTimelineView {
        settled,
        pending,
        focus,
        settled_sats,
        pending_sats,
        known_payer_count,
        avg_confirmation_latency_seconds,
    })
}

fn seller_pulse_from_fact(fact: &Nip90PaymentFact) -> Option<SellerEarningsPulse> {
    let wallet_authoritative = fact.seller_wallet_confirmed_at.is_some()
        || fact.status == Nip90PaymentFactStatus::SellerWalletSettled;
    let has_seller_signal = wallet_authoritative
        || fact.seller_settlement_feedback_at.is_some()
        || fact.seller_payment_pointer.is_some()
        || matches!(
            fact.status,
            Nip90PaymentFactStatus::SellerSettlementObserved | Nip90PaymentFactStatus::Failed
        );
    if !has_seller_signal {
        return None;
    }

    let settlement_observed_at = fact
        .seller_settlement_feedback_at
        .or(fact.result_observed_at)
        .or(fact.request_published_at);
    let wallet_confirmed_at = fact.seller_wallet_confirmed_at;
    let effective_at = wallet_confirmed_at
        .or(settlement_observed_at)
        .or(fact.latest_event_epoch_seconds())
        .unwrap_or(0);
    let confirmation_latency_seconds = wallet_confirmed_at
        .zip(settlement_observed_at)
        .map(|(confirmed_at, observed_at)| confirmed_at.saturating_sub(observed_at));
    let payer_identity = normalized_identity(fact.buyer_nostr_pubkey.as_deref());
    let provider_identity = normalized_identity(fact.provider_nostr_pubkey.as_deref());

    Some(SellerEarningsPulse {
        request_id: fact.request_id.clone(),
        request_type: fact.request_type.clone(),
        payer_label: compact_identity(fact.buyer_nostr_pubkey.as_deref()),
        payer_identity,
        provider_label: compact_identity(fact.provider_nostr_pubkey.as_deref()),
        provider_identity,
        amount_sats: fact.amount_sats.unwrap_or_default(),
        status: fact.status,
        source_quality: fact.source_quality,
        settlement_authority: fact.settlement_authority.clone(),
        seller_payment_pointer: fact.seller_payment_pointer.clone(),
        result_event_id: fact.result_event_id.clone(),
        seller_feedback_event_id: fact.seller_feedback_event_id.clone(),
        settlement_observed_at,
        wallet_confirmed_at,
        effective_at,
        confirmation_latency_seconds,
        wallet_authoritative,
    })
}

fn paint_header(bounds: Bounds, view: &SellerEarningsTimelineView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "SELLER EARNINGS TIMELINE  //  WALLET-AUTHORITATIVE RECEIVES",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Confirmed Spark receives stay on the main rail; inferred settlement observations remain visibly degraded below.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "settled={}  pending={}  known_payers={}  avg_confirmation={}",
                format_sats_amount(view.settled_sats),
                format_sats_amount(view.pending_sats),
                view.known_payer_count,
                latency_label(view.avg_confirmation_latency_seconds),
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 48.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );

    let total_count = (view.settled.len() + view.pending.len()).max(1) as f32;
    let settled_ratio = view.settled.len() as f32 / total_count;
    let mut meter = SignalMeter::new()
        .bars(7)
        .gap(2.0)
        .level(settled_ratio)
        .min_bar_height(0.16)
        .active_color(Hsla::from_hex(0x6df0c5).with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.48));
    meter.paint(
        Bounds::new(bounds.max_x() - 50.0, bounds.origin.y + 4.0, 38.0, 44.0),
        paint,
    );
}

fn paint_timeline_panel(
    bounds: Bounds,
    view: &SellerEarningsTimelineView,
    paint: &mut PaintContext,
) {
    paint_panel_shell(
        bounds,
        "Spark Receive Rail",
        Hsla::from_hex(0x6df0c5).with_alpha(0.82),
        paint,
    );

    if view.settled.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No wallet-confirmed seller receives yet. Pending settlement evidence is still shown below.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    let field_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 30.0,
        bounds.size.width - 24.0,
        bounds.size.height - 44.0,
    );
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(1.0)
        .color(Hsla::from_hex(0x6df0c5).with_alpha(0.18))
        .animation_progress(1.0);
    dots.paint(field_bounds, paint);

    let mut scanlines = Scanlines::new()
        .spacing(15.0)
        .line_color(Hsla::from_hex(0x6df0c5).with_alpha(0.06))
        .scan_color(Hsla::from_hex(0x6df0c5).with_alpha(0.18))
        .scan_width(18.0)
        .scan_progress(0.72)
        .opacity(0.82);
    scanlines.paint(field_bounds, paint);

    let baseline_y = field_bounds.origin.y + field_bounds.size.height * 0.72;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            field_bounds.origin.x,
            baseline_y,
            field_bounds.size.width,
            2.0,
        ))
        .with_background(Hsla::from_hex(0x6df0c5).with_alpha(0.18)),
    );

    let ordered = view.settled.iter().rev().collect::<Vec<_>>();
    let min_epoch = ordered
        .iter()
        .map(|pulse| pulse.effective_at)
        .min()
        .unwrap_or_default();
    let max_epoch = ordered
        .iter()
        .map(|pulse| pulse.effective_at)
        .max()
        .unwrap_or(min_epoch);
    let max_amount = ordered
        .iter()
        .map(|pulse| pulse.amount_sats)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_latency = ordered
        .iter()
        .filter_map(|pulse| pulse.confirmation_latency_seconds)
        .max()
        .unwrap_or(1)
        .max(1);
    let usable_width = (field_bounds.size.width - 32.0).max(1.0);

    for (index, pulse) in ordered.iter().enumerate() {
        let x = field_bounds.origin.x
            + 16.0
            + position_share(
                index,
                ordered.len(),
                pulse.effective_at,
                min_epoch,
                max_epoch,
            ) * usable_width;
        let amount_ratio = (pulse.amount_sats.max(1) as f32 / max_amount as f32).clamp(0.08, 1.0);
        let capsule_width = 26.0 + amount_ratio * 84.0;
        let capsule_height = 14.0 + amount_ratio * 10.0;
        let trail_length = pulse
            .confirmation_latency_seconds
            .map(|latency| 18.0 + (latency as f32 / max_latency as f32) * 76.0)
            .unwrap_or(14.0);
        let y = baseline_y - 24.0 - amount_ratio * 56.0 - (index % 3) as f32 * 10.0;
        let accent = pulse_color(pulse);

        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                (x - trail_length).max(field_bounds.origin.x),
                y + capsule_height * 0.5,
                trail_length,
                3.0,
            ))
            .with_background(accent.with_alpha(0.18))
            .with_corner_radius(3.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                x - capsule_width * 0.5,
                y,
                capsule_width,
                capsule_height,
            ))
            .with_background(accent.with_alpha(0.86))
            .with_border(accent.with_alpha(0.98), 1.0)
            .with_corner_radius(capsule_height * 0.5),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(x - 1.0, baseline_y - 2.0, 2.0, 18.0))
                .with_background(accent.with_alpha(0.4)),
        );

        paint.scene.draw_text(paint.text.layout_mono(
            format_sats_amount(pulse.amount_sats).as_str(),
            Point::new(x - capsule_width * 0.5, y - 12.0),
            8.5,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            pulse.payer_label.as_str(),
            Point::new(x - capsule_width * 0.5, y + capsule_height + 4.0),
            8.5,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout_mono(
        timestamp_label(Some(min_epoch)).as_str(),
        Point::new(field_bounds.origin.x, bounds.max_y() - 16.0),
        8.5,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        timestamp_label(Some(max_epoch)).as_str(),
        Point::new(bounds.max_x() - 116.0, bounds.max_y() - 16.0),
        8.5,
        theme::text::MUTED,
    ));
}

fn paint_detail_panel(
    bounds: Bounds,
    focus: &SellerEarningsPulse,
    view: &SellerEarningsTimelineView,
    paint: &mut PaintContext,
) {
    let accent = pulse_color(focus);
    paint_panel_shell(bounds, "Latest Payout Detail", accent, paint);

    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "{}  {}",
                format_sats_amount(focus.amount_sats),
                status_label(focus)
            )
            .as_str(),
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
            12.0,
            accent,
        ),
    );
    paint.scene.draw_text(paint.text.layout(
        focus_quality_line(focus).as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 50.0),
        9.5,
        theme::text::MUTED,
    ));

    let mut y = bounds.origin.y + 76.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "request",
        short_id(focus.request_id.as_str()).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "payer",
        focus.payer_identity.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "provider",
        focus.provider_identity.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "authority",
        focus.settlement_authority.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "seller_settle",
        timestamp_label(focus.settlement_observed_at).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "wallet_settle",
        timestamp_label(focus.wallet_confirmed_at).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "latency",
        latency_label(focus.confirmation_latency_seconds).as_str(),
    );
    if let Some(pointer) = focus.seller_payment_pointer.as_deref() {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "pointer",
            short_id(pointer).as_str(),
        );
    }
    if let Some(result_event_id) = focus.result_event_id.as_deref() {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "result",
            short_id(result_event_id).as_str(),
        );
    }
    if let Some(feedback_event_id) = focus.seller_feedback_event_id.as_deref() {
        let _ = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "seller_evt",
            short_id(feedback_event_id).as_str(),
        );
    }

    paint.scene.draw_text(paint.text.layout(
        &format!(
            "{} wallet-confirmed receives, {} degraded settlement rows still pending proof.",
            view.settled.len(),
            view.pending.len()
        ),
        Point::new(bounds.origin.x + 12.0, bounds.max_y() - 18.0),
        9.5,
        theme::text::MUTED,
    ));
}

fn paint_pending_panel(
    bounds: Bounds,
    view: &SellerEarningsTimelineView,
    paint: &mut PaintContext,
) {
    paint_panel_shell(
        bounds,
        "Pending / Inferred Settlement",
        Hsla::from_hex(0xffbf52).with_alpha(0.82),
        paint,
    );

    if view.pending.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "All visible seller payouts are wallet-confirmed right now.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    let row_height = 30.0;
    for (index, pulse) in view.pending.iter().enumerate() {
        let row_bounds = Bounds::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + 28.0 + index as f32 * (row_height + 6.0),
            bounds.size.width - 20.0,
            row_height,
        );
        let accent = pulse_color(pulse);
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(Hsla::from_hex(0x1b1310).with_alpha(0.94))
                .with_border(accent.with_alpha(0.35), 1.0)
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(
            paint.text.layout_mono(
                format!(
                    "{}  {}  {}",
                    status_label(pulse),
                    format_sats_amount(pulse.amount_sats),
                    short_id(pulse.request_id.as_str())
                )
                .as_str(),
                Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
                9.0,
                accent,
            ),
        );
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "payer={}  authority={}  proof={}  at={}",
                pulse.payer_label,
                pulse.settlement_authority,
                pulse.source_quality.label(),
                timestamp_label(Some(pulse.effective_at))
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 18.0),
            8.5,
            theme::text::MUTED,
        ));
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
        "SELLER EARNINGS TIMELINE",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No seller-side payment facts exist yet. This pane wakes up once a receipt projection or wallet-confirmed Spark receive lands in the NIP-90 payment fact ledger.",
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

fn pulse_color(pulse: &SellerEarningsPulse) -> Hsla {
    if pulse.wallet_authoritative {
        Hsla::from_hex(0x6df0c5)
    } else if pulse.status == Nip90PaymentFactStatus::Failed {
        Hsla::from_hex(0xff6767)
    } else {
        Hsla::from_hex(0xffbf52)
    }
}

fn status_label(pulse: &SellerEarningsPulse) -> &'static str {
    if pulse.wallet_authoritative {
        "wallet settled"
    } else if pulse.status == Nip90PaymentFactStatus::Failed {
        "failed"
    } else {
        "settlement observed"
    }
}

fn focus_quality_line(pulse: &SellerEarningsPulse) -> String {
    format!(
        "{} proof via {}",
        if pulse.wallet_authoritative {
            "Wallet-authoritative"
        } else {
            "Degraded"
        },
        pulse.source_quality.label()
    )
}

fn position_share(index: usize, total: usize, epoch: u64, min_epoch: u64, max_epoch: u64) -> f32 {
    if total <= 1 || min_epoch == max_epoch {
        if total <= 1 {
            0.5
        } else {
            index as f32 / (total.saturating_sub(1)) as f32
        }
    } else {
        (epoch.saturating_sub(min_epoch) as f32 / max_epoch.saturating_sub(min_epoch) as f32)
            .clamp(0.0, 1.0)
    }
}

fn normalized_identity(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
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

fn short_id(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 18 {
        value.to_string()
    } else {
        format!("{}..{}", &value[..10], &value[value.len() - 6..])
    }
}

fn timestamp_label(epoch_seconds: Option<u64>) -> String {
    epoch_seconds
        .and_then(|value| Local.timestamp_opt(value as i64, 0).single())
        .map(|value| value.format("%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn latency_label(value: Option<u64>) -> String {
    value
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::build_view_from_facts;
    use crate::state::nip90_payment_facts::{
        Nip90PaymentFact, Nip90PaymentFactSourceQuality, Nip90PaymentFactStatus,
    };

    #[test]
    fn seller_earnings_timeline_distinguishes_wallet_settled_from_inferred_receipts() {
        let view = build_view_from_facts(&[
            seller_fact(
                "req-wallet",
                21,
                Nip90PaymentFactStatus::SellerWalletSettled,
                Nip90PaymentFactSourceQuality::SellerWalletReconciled,
                Some(220),
                Some(250),
                Some("pointer-wallet"),
                Some("npub1buyerwallet"),
            ),
            seller_fact(
                "req-pending",
                13,
                Nip90PaymentFactStatus::SellerSettlementObserved,
                Nip90PaymentFactSourceQuality::SellerReceiptProjection,
                Some(320),
                None,
                Some("pointer-pending"),
                None,
            ),
        ])
        .expect("seller timeline view should exist");

        assert_eq!(view.settled.len(), 1);
        assert_eq!(view.pending.len(), 1);
        assert!(view.settled[0].wallet_authoritative);
        assert!(!view.pending[0].wallet_authoritative);
        assert_eq!(view.focus.request_id, "req-wallet");
        assert_eq!(view.pending[0].payer_identity, "unknown");
    }

    #[test]
    fn seller_earnings_timeline_surfaces_requester_identity_and_confirmation_latency() {
        let view = build_view_from_facts(&[seller_fact(
            "req-latency",
            55,
            Nip90PaymentFactStatus::SellerWalletSettled,
            Nip90PaymentFactSourceQuality::SellerWalletReconciled,
            Some(500),
            Some(545),
            Some("pointer-latency"),
            Some("npub1buyerlatency000111222333"),
        )])
        .expect("seller timeline view should exist");

        assert_eq!(
            view.focus.payer_identity,
            "npub1buyerlatency000111222333".to_string()
        );
        assert_eq!(view.focus.confirmation_latency_seconds, Some(45));
        assert_eq!(view.avg_confirmation_latency_seconds, Some(45));
        assert_eq!(
            view.focus.seller_payment_pointer.as_deref(),
            Some("pointer-latency")
        );
    }

    fn seller_fact(
        request_id: &str,
        amount_sats: u64,
        status: Nip90PaymentFactStatus,
        source_quality: Nip90PaymentFactSourceQuality,
        seller_settlement_feedback_at: Option<u64>,
        seller_wallet_confirmed_at: Option<u64>,
        seller_payment_pointer: Option<&str>,
        buyer_nostr_pubkey: Option<&str>,
    ) -> Nip90PaymentFact {
        Nip90PaymentFact {
            fact_id: format!("fact-{request_id}"),
            request_id: request_id.to_string(),
            request_type: "kind5050".to_string(),
            request_event_id: Some(format!("event-{request_id}")),
            result_event_id: Some(format!("result-{request_id}")),
            invoice_event_id: None,
            seller_feedback_event_id: Some(format!("seller-{request_id}")),
            buyer_nostr_pubkey: buyer_nostr_pubkey.map(ToString::to_string),
            provider_nostr_pubkey: Some("npub1providerlocal".to_string()),
            invoice_provider_pubkey: None,
            result_provider_pubkey: None,
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: None,
            seller_payment_pointer: seller_payment_pointer.map(ToString::to_string),
            buyer_payment_hash: None,
            amount_sats: Some(amount_sats),
            fees_sats: Some(1),
            total_debit_sats: Some(amount_sats),
            wallet_method: Some("lightning".to_string()),
            status,
            settlement_authority: if seller_wallet_confirmed_at.is_some() {
                "wallet.reconciliation".to_string()
            } else {
                "earn.receipts".to_string()
            },
            request_published_at: Some(100),
            result_observed_at: Some(150),
            invoice_observed_at: None,
            buyer_payment_pointer_at: None,
            seller_settlement_feedback_at,
            buyer_wallet_confirmed_at: None,
            seller_wallet_confirmed_at,
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality,
        }
    }
}
