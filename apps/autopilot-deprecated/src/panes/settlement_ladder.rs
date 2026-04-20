use chrono::{Local, TimeZone};
use wgpui::components::hud::{DotShape, DotsGrid, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::state::nip90_payment_facts::{Nip90PaymentFact, Nip90PaymentFactLedgerState};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 66.0;
const STRIP_HEIGHT: f32 = 54.0;
const RUNG_COUNT: usize = 6;

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
    let strip_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width,
        STRIP_HEIGHT,
    );
    let ladder_bounds = Bounds::new(
        header_bounds.origin.x,
        strip_bounds.max_y() + 8.0,
        header_bounds.size.width * 0.62,
        content_bounds.max_y() - strip_bounds.max_y() - 20.0,
    );
    let detail_bounds = Bounds::new(
        ladder_bounds.max_x() + 10.0,
        ladder_bounds.origin.y,
        content_bounds.max_x() - ladder_bounds.max_x() - 22.0,
        ladder_bounds.size.height,
    );

    paint_header(header_bounds, &view, paint);
    paint_focus_strip(strip_bounds, &view, paint);
    paint_ladder(ladder_bounds, &view.focus, paint);
    paint_detail_panel(detail_bounds, &view.focus, paint);
}

#[derive(Clone, Debug, PartialEq)]
struct SettlementLadderView {
    focus: SettlementFactView,
    recent_requests: Vec<SettlementRequestChip>,
    completion_ratio: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettlementRequestChip {
    request_id: String,
    status_label: String,
    lit_count: usize,
    missing_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettlementFactView {
    request_id: String,
    request_type: String,
    buyer_label: String,
    buyer_identity: String,
    provider_label: String,
    provider_identity: String,
    amount_sats: Option<u64>,
    settlement_authority: String,
    buyer_payment_pointer: Option<String>,
    seller_payment_pointer: Option<String>,
    buyer_payment_hash: Option<String>,
    request_event_id: Option<String>,
    result_event_id: Option<String>,
    invoice_event_id: Option<String>,
    seller_feedback_event_id: Option<String>,
    seller_wallet_confirmed_at: Option<u64>,
    buyer_wallet_confirmed_at: Option<u64>,
    source_quality: String,
    rungs: Vec<SettlementRung>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettlementRung {
    label: &'static str,
    lit: bool,
    evidence_label: String,
    proof_label: String,
    detail_label: Option<String>,
    observed_at: Option<u64>,
    role: SettlementRungRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettlementRungRole {
    Discovery,
    BuyerPayment,
    SellerSettlement,
    BuyerWallet,
}

fn build_view(payment_facts: &Nip90PaymentFactLedgerState) -> Option<SettlementLadderView> {
    build_view_from_facts(payment_facts.facts.as_slice())
}

fn build_view_from_facts(facts: &[Nip90PaymentFact]) -> Option<SettlementLadderView> {
    let mut views = facts
        .iter()
        .map(build_fact_view)
        .filter(|view| !view.rungs.is_empty())
        .collect::<Vec<_>>();
    if views.is_empty() {
        return None;
    }

    views.sort_by(|left, right| {
        focus_priority(left)
            .cmp(&focus_priority(right))
            .then_with(|| latest_rung_epoch(right).cmp(&latest_rung_epoch(left)))
            .then_with(|| left.request_id.cmp(&right.request_id))
    });

    let focus = views.first().cloned()?;
    let recent_requests = views
        .iter()
        .take(5)
        .map(|view| SettlementRequestChip {
            request_id: view.request_id.clone(),
            status_label: ladder_status_label(view).to_string(),
            lit_count: view.rungs.iter().filter(|rung| rung.lit).count(),
            missing_count: view.rungs.iter().filter(|rung| !rung.lit).count(),
        })
        .collect::<Vec<_>>();
    let completion_ratio =
        focus.rungs.iter().filter(|rung| rung.lit).count() as f32 / RUNG_COUNT as f32;

    Some(SettlementLadderView {
        focus,
        recent_requests,
        completion_ratio,
    })
}

fn build_fact_view(fact: &Nip90PaymentFact) -> SettlementFactView {
    let request_observed = fact.request_published_at.or_else(|| {
        fact.request_event_id
            .as_ref()
            .map(|_| fact.latest_event_epoch_seconds().unwrap_or_default())
    });
    let result_observed = fact.result_observed_at.or_else(|| {
        fact.result_event_id
            .as_ref()
            .map(|_| fact.latest_event_epoch_seconds().unwrap_or_default())
    });
    let invoice_observed = fact.invoice_observed_at.or_else(|| {
        fact.invoice_event_id
            .as_ref()
            .map(|_| fact.latest_event_epoch_seconds().unwrap_or_default())
    });
    let pointer_assigned = fact.buyer_payment_pointer_at.or_else(|| {
        fact.buyer_payment_pointer
            .as_ref()
            .map(|_| fact.latest_event_epoch_seconds().unwrap_or_default())
    });
    let seller_settled = fact.seller_settlement_feedback_at.or_else(|| {
        fact.seller_feedback_event_id
            .as_ref()
            .map(|_| fact.latest_event_epoch_seconds().unwrap_or_default())
    });
    let buyer_wallet_confirmed = fact.buyer_wallet_confirmed_at;

    let rungs = vec![
        SettlementRung {
            label: "request observed",
            lit: request_observed.is_some(),
            evidence_label: fact
                .request_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| missing_label(request_observed)),
            proof_label: rung_proof_label(request_observed.is_some(), fact.source_quality.label()),
            detail_label: Some(format!(
                "authority={} request_type={}",
                fact.settlement_authority, fact.request_type
            )),
            observed_at: request_observed,
            role: SettlementRungRole::Discovery,
        },
        SettlementRung {
            label: "result observed",
            lit: result_observed.is_some(),
            evidence_label: fact
                .result_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| missing_label(result_observed)),
            proof_label: rung_proof_label(result_observed.is_some(), fact.source_quality.label()),
            detail_label: Some(format!(
                "provider={}",
                compact_identity(fact.result_provider_pubkey.as_deref())
            )),
            observed_at: result_observed,
            role: SettlementRungRole::Discovery,
        },
        SettlementRung {
            label: "invoice observed",
            lit: invoice_observed.is_some(),
            evidence_label: fact
                .invoice_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| missing_label(invoice_observed)),
            proof_label: rung_proof_label(invoice_observed.is_some(), fact.source_quality.label()),
            detail_label: Some(format!(
                "invoice_provider={}",
                compact_identity(fact.invoice_provider_pubkey.as_deref())
            )),
            observed_at: invoice_observed,
            role: SettlementRungRole::BuyerPayment,
        },
        SettlementRung {
            label: "payment pointer assigned",
            lit: pointer_assigned.is_some(),
            evidence_label: fact
                .buyer_payment_pointer
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| missing_label(pointer_assigned)),
            proof_label: rung_proof_label(pointer_assigned.is_some(), fact.source_quality.label()),
            detail_label: fact
                .buyer_payment_hash
                .as_deref()
                .map(|payment_hash| format!("payment_hash={}", short_id(payment_hash))),
            observed_at: pointer_assigned,
            role: SettlementRungRole::BuyerPayment,
        },
        SettlementRung {
            label: "seller settled",
            lit: seller_settled.is_some() || fact.seller_wallet_confirmed_at.is_some(),
            evidence_label: fact
                .seller_feedback_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| missing_label(seller_settled)),
            proof_label: rung_proof_label(
                seller_settled.is_some() || fact.seller_wallet_confirmed_at.is_some(),
                fact.source_quality.label(),
            ),
            detail_label: Some(match fact.seller_wallet_confirmed_at {
                Some(epoch) => format!("seller_wallet_settled={}", timestamp_label(Some(epoch))),
                None => "seller wallet settlement missing".to_string(),
            }),
            observed_at: seller_settled.or(fact.seller_wallet_confirmed_at),
            role: SettlementRungRole::SellerSettlement,
        },
        SettlementRung {
            label: "buyer wallet confirmed",
            lit: buyer_wallet_confirmed.is_some(),
            evidence_label: if buyer_wallet_confirmed.is_some() {
                fact.buyer_payment_hash
                    .as_deref()
                    .map(short_id)
                    .or_else(|| fact.buyer_payment_pointer.as_deref().map(short_id))
                    .unwrap_or_else(|| "wallet-confirmed".to_string())
            } else {
                "missing".to_string()
            },
            proof_label: rung_proof_label(
                buyer_wallet_confirmed.is_some(),
                fact.source_quality.label(),
            ),
            detail_label: Some(format!(
                "buyer pointer={} seller pointer={}",
                compact_identity(fact.buyer_payment_pointer.as_deref()),
                compact_identity(fact.seller_payment_pointer.as_deref())
            )),
            observed_at: buyer_wallet_confirmed,
            role: SettlementRungRole::BuyerWallet,
        },
    ];

    SettlementFactView {
        request_id: fact.request_id.clone(),
        request_type: fact.request_type.clone(),
        buyer_label: compact_identity(fact.buyer_nostr_pubkey.as_deref()),
        buyer_identity: normalized_identity(fact.buyer_nostr_pubkey.as_deref()),
        provider_label: compact_identity(fact.provider_nostr_pubkey.as_deref()),
        provider_identity: normalized_identity(fact.provider_nostr_pubkey.as_deref()),
        amount_sats: fact.amount_sats,
        settlement_authority: fact.settlement_authority.clone(),
        buyer_payment_pointer: fact.buyer_payment_pointer.clone(),
        seller_payment_pointer: fact.seller_payment_pointer.clone(),
        buyer_payment_hash: fact.buyer_payment_hash.clone(),
        request_event_id: fact.request_event_id.clone(),
        result_event_id: fact.result_event_id.clone(),
        invoice_event_id: fact.invoice_event_id.clone(),
        seller_feedback_event_id: fact.seller_feedback_event_id.clone(),
        seller_wallet_confirmed_at: fact.seller_wallet_confirmed_at,
        buyer_wallet_confirmed_at: fact.buyer_wallet_confirmed_at,
        source_quality: fact.source_quality.label().to_string(),
        rungs,
    }
}

fn paint_header(bounds: Bounds, view: &SettlementLadderView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "SETTLEMENT LADDER  //  PER-REQUEST PROOF CHAIN",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Each rung only lights when the NIP-90 payment fact ledger has concrete evidence for that proof step.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "focus={}  status={}  amount={}  authority={}",
                short_id(view.focus.request_id.as_str()),
                ladder_status_label(&view.focus),
                view.focus
                    .amount_sats
                    .map(format_sats_amount)
                    .unwrap_or_else(|| "-".to_string()),
                view.focus.settlement_authority,
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 48.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );

    let mut meter = SignalMeter::new()
        .bars(6)
        .gap(2.0)
        .level(view.completion_ratio)
        .min_bar_height(0.16)
        .active_color(Hsla::from_hex(0x7ce3ff).with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.48));
    meter.paint(
        Bounds::new(bounds.max_x() - 44.0, bounds.origin.y + 6.0, 32.0, 40.0),
        paint,
    );
}

fn paint_focus_strip(bounds: Bounds, view: &SettlementLadderView, paint: &mut PaintContext) {
    paint_panel_shell(
        bounds,
        "Recent Requests",
        Hsla::from_hex(0x7ce3ff).with_alpha(0.82),
        paint,
    );

    let visible = view.recent_requests.len().max(1);
    let gap = 6.0;
    let chip_width = ((bounds.size.width - 20.0 - gap * (visible.saturating_sub(1) as f32))
        / visible as f32)
        .max(120.0);
    for (index, chip) in view.recent_requests.iter().enumerate() {
        let chip_bounds = Bounds::new(
            bounds.origin.x + 10.0 + index as f32 * (chip_width + gap),
            bounds.origin.y + 18.0,
            chip_width,
            24.0,
        );
        let focused = chip.request_id == view.focus.request_id;
        let accent = if focused {
            Hsla::from_hex(0x7ce3ff)
        } else {
            Hsla::from_hex(0x506c87)
        };
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(if focused {
                    Hsla::from_hex(0x0c1a24).with_alpha(0.96)
                } else {
                    Hsla::from_hex(0x0a1117).with_alpha(0.9)
                })
                .with_border(accent.with_alpha(0.45), 1.0)
                .with_corner_radius(7.0),
        );
        paint.scene.draw_text(
            paint.text.layout_mono(
                format!(
                    "{}  {}/{}",
                    short_id(chip.request_id.as_str()),
                    chip.lit_count,
                    RUNG_COUNT
                )
                .as_str(),
                Point::new(chip_bounds.origin.x + 8.0, chip_bounds.origin.y + 7.0),
                8.8,
                accent.with_alpha(0.96),
            ),
        );
        paint.scene.draw_text(paint.text.layout(
            chip.status_label.as_str(),
            Point::new(chip_bounds.origin.x + 84.0, chip_bounds.origin.y + 7.0),
            8.5,
            theme::text::MUTED,
        ));
    }
}

fn paint_ladder(bounds: Bounds, view: &SettlementFactView, paint: &mut PaintContext) {
    paint_panel_shell(
        bounds,
        "Proof Rungs",
        Hsla::from_hex(0x7ce3ff).with_alpha(0.82),
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
        .size(1.0)
        .color(Hsla::from_hex(0x7ce3ff).with_alpha(0.14))
        .animation_progress(1.0);
    dots.paint(field_bounds, paint);

    let left_post_x = field_bounds.origin.x + 18.0;
    let right_post_x = field_bounds.max_x() - 18.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            left_post_x,
            field_bounds.origin.y + 12.0,
            3.0,
            field_bounds.size.height - 24.0,
        ))
        .with_background(Hsla::from_hex(0x284154).with_alpha(0.55))
        .with_corner_radius(2.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            right_post_x,
            field_bounds.origin.y + 12.0,
            3.0,
            field_bounds.size.height - 24.0,
        ))
        .with_background(Hsla::from_hex(0x284154).with_alpha(0.55))
        .with_corner_radius(2.0),
    );

    let rung_gap = ((field_bounds.size.height - 36.0) / RUNG_COUNT as f32).max(48.0);
    for (index, rung) in view.rungs.iter().enumerate() {
        let y = field_bounds.origin.y + 18.0 + index as f32 * rung_gap;
        let rung_bounds = Bounds::new(
            left_post_x + 18.0,
            y,
            right_post_x - left_post_x - 36.0,
            36.0,
        );
        let accent = rung_color(rung.role, rung.lit);
        paint.scene.draw_quad(
            Quad::new(rung_bounds)
                .with_background(if rung.lit {
                    accent.with_alpha(0.16)
                } else {
                    Hsla::from_hex(0x0a1015).with_alpha(0.9)
                })
                .with_border(accent.with_alpha(if rung.lit { 0.55 } else { 0.18 }), 1.0)
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            rung.label,
            Point::new(rung_bounds.origin.x + 10.0, rung_bounds.origin.y + 8.0),
            9.5,
            if rung.lit {
                accent.with_alpha(0.96)
            } else {
                theme::text::MUTED
            },
        ));
        paint.scene.draw_text(paint.text.layout(
            rung.evidence_label.as_str(),
            Point::new(rung_bounds.origin.x + 178.0, rung_bounds.origin.y + 7.0),
            9.0,
            if rung.lit {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
        paint.scene.draw_text(paint.text.layout(
            rung.proof_label.as_str(),
            Point::new(rung_bounds.origin.x + 178.0, rung_bounds.origin.y + 19.0),
            8.5,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            timestamp_label(rung.observed_at).as_str(),
            Point::new(rung_bounds.max_x() - 92.0, rung_bounds.origin.y + 8.0),
            8.5,
            theme::text::SECONDARY,
        ));
        if let Some(detail) = rung.detail_label.as_deref() {
            paint.scene.draw_text(paint.text.layout(
                detail,
                Point::new(rung_bounds.origin.x + 10.0, rung_bounds.origin.y + 20.0),
                8.3,
                theme::text::MUTED,
            ));
        }
    }
}

fn paint_detail_panel(bounds: Bounds, view: &SettlementFactView, paint: &mut PaintContext) {
    paint_panel_shell(
        bounds,
        "Focus Detail",
        Hsla::from_hex(0x68f0b8).with_alpha(0.82),
        paint,
    );

    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "{}  {}",
                short_id(view.request_id.as_str()),
                ladder_status_label(view)
            )
            .as_str(),
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
            12.0,
            theme::text::PRIMARY,
        ),
    );
    paint.scene.draw_text(paint.text.layout(
        "Buyer proof and seller proof stay split here: pointer/hash on the buyer side, seller feedback and seller-wallet settlement on the provider side.",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 50.0),
        9.0,
        theme::text::MUTED,
    ));

    let mut y = bounds.origin.y + 78.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "buyer",
        view.buyer_identity.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "provider",
        view.provider_identity.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "amount",
        view.amount_sats
            .map(format_sats_amount)
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "authority",
        view.settlement_authority.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "quality",
        view.source_quality.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "buyer_ptr",
        compact_identity(view.buyer_payment_pointer.as_deref()).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "seller_ptr",
        compact_identity(view.seller_payment_pointer.as_deref()).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "seller_wallet",
        timestamp_label(view.seller_wallet_confirmed_at).as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "buyer_wallet",
        timestamp_label(view.buyer_wallet_confirmed_at).as_str(),
    );
    if let Some(value) = view.request_event_id.as_deref() {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "request_evt",
            short_id(value).as_str(),
        );
    }
    if let Some(value) = view.result_event_id.as_deref() {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "result_evt",
            short_id(value).as_str(),
        );
    }
    if let Some(value) = view.invoice_event_id.as_deref() {
        y = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "invoice_evt",
            short_id(value).as_str(),
        );
    }
    if let Some(value) = view.seller_feedback_event_id.as_deref() {
        let _ = paint_label_line(
            paint,
            bounds.origin.x + 12.0,
            y,
            "seller_evt",
            short_id(value).as_str(),
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
        "SETTLEMENT LADDER",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No payment facts are available yet. The ladder needs at least one request-shaped fact row before it can show proof gaps.",
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

fn focus_priority(view: &SettlementFactView) -> (u8, u64) {
    let missing_count = view.rungs.iter().filter(|rung| !rung.lit).count();
    let incomplete_bias = if missing_count > 0 { 0 } else { 1 };
    (incomplete_bias, u64::MAX - latest_rung_epoch(view))
}

fn latest_rung_epoch(view: &SettlementFactView) -> u64 {
    view.rungs
        .iter()
        .filter_map(|rung| rung.observed_at)
        .max()
        .unwrap_or(0)
}

fn ladder_status_label(view: &SettlementFactView) -> &'static str {
    if view.rungs.iter().all(|rung| rung.lit) {
        "complete"
    } else if view.rungs[4].lit {
        "seller-settled"
    } else if view.rungs[3].lit {
        "payment-sent"
    } else if view.rungs[2].lit {
        "awaiting-payment"
    } else if view.rungs[1].lit {
        "result-observed"
    } else {
        "request-observed"
    }
}

fn rung_color(role: SettlementRungRole, lit: bool) -> Hsla {
    let base = match role {
        SettlementRungRole::Discovery => Hsla::from_hex(0x7ce3ff),
        SettlementRungRole::BuyerPayment => Hsla::from_hex(0x6d93ff),
        SettlementRungRole::SellerSettlement => Hsla::from_hex(0x68f0b8),
        SettlementRungRole::BuyerWallet => Hsla::from_hex(0xffd463),
    };
    if lit { base } else { base.with_alpha(0.32) }
}

fn rung_proof_label(lit: bool, source_quality: &str) -> String {
    if lit {
        format!("proof={source_quality}")
    } else {
        "proof=missing".to_string()
    }
}

fn missing_label(observed_at: Option<u64>) -> String {
    if observed_at.is_some() {
        "observed".to_string()
    } else {
        "missing".to_string()
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

#[cfg(test)]
mod tests {
    use super::build_view_from_facts;
    use crate::state::nip90_payment_facts::{
        Nip90PaymentFact, Nip90PaymentFactSourceQuality, Nip90PaymentFactStatus,
    };

    #[test]
    fn settlement_ladder_keeps_missing_rungs_unlit() {
        let view = build_view_from_facts(&[fact_with_rungs(
            "req-partial",
            Some(100),
            Some("req-event"),
            Some(110),
            Some("result-event"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )])
        .expect("ladder should build");

        assert!(view.focus.rungs[0].lit);
        assert!(view.focus.rungs[1].lit);
        assert!(!view.focus.rungs[2].lit);
        assert_eq!(view.focus.rungs[2].evidence_label, "missing");
        assert_eq!(view.focus.rungs[5].proof_label, "proof=missing");
    }

    #[test]
    fn settlement_ladder_distinguishes_seller_wallet_from_buyer_wallet_confirmation() {
        let view = build_view_from_facts(&[fact_with_rungs(
            "req-seller-wallet",
            Some(100),
            Some("req-event"),
            Some(110),
            Some("result-event"),
            Some(115),
            Some("invoice-event"),
            Some(120),
            Some("buyer-pointer"),
            Some(130),
            Some(140),
            None,
        )])
        .expect("ladder should build");

        assert!(view.focus.rungs[4].lit);
        assert!(
            view.focus.rungs[4]
                .detail_label
                .as_deref()
                .is_some_and(|detail| detail.contains("seller_wallet_settled"))
        );
        assert!(!view.focus.rungs[5].lit);
        assert_eq!(view.focus.rungs[5].evidence_label, "missing");
    }

    fn fact_with_rungs(
        request_id: &str,
        request_published_at: Option<u64>,
        request_event_id: Option<&str>,
        result_observed_at: Option<u64>,
        result_event_id: Option<&str>,
        invoice_observed_at: Option<u64>,
        invoice_event_id: Option<&str>,
        buyer_payment_pointer_at: Option<u64>,
        buyer_payment_pointer: Option<&str>,
        seller_settlement_feedback_at: Option<u64>,
        seller_wallet_confirmed_at: Option<u64>,
        buyer_wallet_confirmed_at: Option<u64>,
    ) -> Nip90PaymentFact {
        Nip90PaymentFact {
            fact_id: format!("fact-{request_id}"),
            request_id: request_id.to_string(),
            request_type: "kind5050".to_string(),
            request_event_id: request_event_id.map(ToString::to_string),
            result_event_id: result_event_id.map(ToString::to_string),
            invoice_event_id: invoice_event_id.map(ToString::to_string),
            seller_feedback_event_id: seller_settlement_feedback_at
                .map(|_| format!("seller-{request_id}")),
            buyer_nostr_pubkey: Some("npub1buyer".to_string()),
            provider_nostr_pubkey: Some("npub1provider".to_string()),
            invoice_provider_pubkey: Some("npub1provider".to_string()),
            result_provider_pubkey: Some("npub1provider".to_string()),
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: buyer_payment_pointer.map(ToString::to_string),
            seller_payment_pointer: seller_wallet_confirmed_at
                .map(|_| format!("seller-{request_id}")),
            buyer_payment_hash: buyer_payment_pointer.map(|_| format!("hash-{request_id}")),
            amount_sats: Some(21),
            fees_sats: Some(1),
            total_debit_sats: Some(22),
            wallet_method: Some("lightning".to_string()),
            status: if buyer_wallet_confirmed_at.is_some() {
                Nip90PaymentFactStatus::BuyerWalletSettled
            } else if seller_wallet_confirmed_at.is_some() {
                Nip90PaymentFactStatus::SellerWalletSettled
            } else if seller_settlement_feedback_at.is_some() {
                Nip90PaymentFactStatus::SellerSettlementObserved
            } else if buyer_payment_pointer.is_some() {
                Nip90PaymentFactStatus::BuyerPaymentPending
            } else if invoice_observed_at.is_some() {
                Nip90PaymentFactStatus::InvoiceObserved
            } else if result_observed_at.is_some() {
                Nip90PaymentFactStatus::ResultObserved
            } else {
                Nip90PaymentFactStatus::RequestPublished
            },
            settlement_authority: "wallet.reconciliation".to_string(),
            request_published_at,
            result_observed_at,
            invoice_observed_at,
            buyer_payment_pointer_at,
            seller_settlement_feedback_at,
            buyer_wallet_confirmed_at,
            seller_wallet_confirmed_at,
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality: Nip90PaymentFactSourceQuality::SellerWalletReconciled,
        }
    }
}
