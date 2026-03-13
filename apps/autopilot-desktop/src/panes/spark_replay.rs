use std::collections::{BTreeMap, BTreeSet};

use chrono::{Local, TimeZone};
use wgpui::components::hud::{DotShape, DotsGrid, Scanlines};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::SparkReplayPaneState;
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_secondary_button, paint_source_badge,
};
use crate::pane_system::{
    spark_replay_auto_button_bounds, spark_replay_next_button_bounds,
    spark_replay_prev_button_bounds,
};
use crate::state::nip90_payment_facts::{
    Nip90PaymentFact, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality,
};
use crate::state::operations::{
    NetworkRequestProviderObservationHistoryEvent, NetworkRequestProviderObservationHistoryKind,
};

const PADDING: f32 = 12.0;
const HEADER_HEIGHT: f32 = 72.0;
const SCRUBBER_HEIGHT: f32 = 72.0;

pub fn replay_step_count(payment_facts: &Nip90PaymentFactLedgerState) -> usize {
    build_view(payment_facts)
        .map(|view| view.steps.len())
        .unwrap_or(0)
}

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut SparkReplayPaneState,
    payment_facts: &Nip90PaymentFactLedgerState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "stream.nip90_payment_facts.v1", paint);
    let Some(view) = build_view(payment_facts) else {
        paint_empty_state(content_bounds, pane_state, payment_facts, paint);
        return;
    };

    pane_state.sync_focus(view.request_id.as_str(), view.steps.len());
    let current_index = pane_state
        .cursor_step
        .min(view.steps.len().saturating_sub(1));
    let current_step = &view.steps[current_index];

    let controls = [
        (
            spark_replay_prev_button_bounds(content_bounds),
            "< PREV",
            false,
        ),
        (
            spark_replay_auto_button_bounds(content_bounds),
            if pane_state.auto_follow {
                "AUTO ON"
            } else {
                "AUTO OFF"
            },
            pane_state.auto_follow,
        ),
        (
            spark_replay_next_button_bounds(content_bounds),
            "NEXT >",
            false,
        ),
    ];
    for (bounds, label, active) in controls {
        if active {
            paint_secondary_button(bounds, label, paint);
        } else {
            paint_action_button(bounds, label, paint);
        }
    }

    let header_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + 40.0,
        content_bounds.size.width - PADDING * 2.0,
        HEADER_HEIGHT,
    );
    let graph_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width * 0.66,
        content_bounds.size.height - HEADER_HEIGHT - SCRUBBER_HEIGHT - 36.0,
    );
    let detail_bounds = Bounds::new(
        graph_bounds.max_x() + 10.0,
        graph_bounds.origin.y,
        content_bounds.max_x() - graph_bounds.max_x() - 22.0,
        graph_bounds.size.height,
    );
    let scrubber_bounds = Bounds::new(
        graph_bounds.origin.x,
        graph_bounds.max_y() + 10.0,
        graph_bounds.size.width,
        SCRUBBER_HEIGHT,
    );

    paint_header(header_bounds, &view, current_index, pane_state, paint);
    paint_replay_field(graph_bounds, &view, current_index, paint);
    paint_detail(
        detail_bounds,
        &view,
        current_step,
        current_index,
        pane_state,
        paint,
    );
    paint_scrubber(scrubber_bounds, &view, current_index, paint);
}

#[derive(Clone, Debug, PartialEq)]
struct SparkReplayView {
    request_id: String,
    buyer_identity: String,
    buyer_label: String,
    provider_order: Vec<String>,
    provider_labels: BTreeMap<String, String>,
    source_quality: Nip90PaymentFactSourceQuality,
    steps: Vec<SparkReplayStep>,
}

#[derive(Clone, Debug, PartialEq)]
struct SparkReplayStep {
    phase: SparkReplayPhase,
    label: String,
    provider_pubkey: Option<String>,
    provider_label: Option<String>,
    observed_at: Option<u64>,
    evidence_label: String,
    degraded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SparkReplayPhase {
    RequestPublished,
    ResultRace,
    WinnerSelected,
    InvoiceObserved,
    BuyerPaymentPointer,
    SellerSettled,
    SellerWalletSettled,
    BuyerWalletConfirmed,
}

fn build_view(payment_facts: &Nip90PaymentFactLedgerState) -> Option<SparkReplayView> {
    build_view_from_facts(payment_facts.facts.as_slice())
}

fn build_view_from_facts(facts: &[Nip90PaymentFact]) -> Option<SparkReplayView> {
    let fact = facts
        .iter()
        .filter(|fact| {
            fact.request_published_at.is_some()
                && (!fact.provider_observation_history.is_empty()
                    || fact.buyer_payment_pointer.is_some()
                    || fact.seller_wallet_confirmed_at.is_some())
        })
        .max_by(|left, right| {
            left.latest_event_epoch_seconds()
                .cmp(&right.latest_event_epoch_seconds())
        })?;

    let buyer_identity = fact
        .buyer_nostr_pubkey
        .as_deref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "buyer:unknown".to_string());
    let provider_order = ordered_provider_pubkeys(fact);
    let provider_labels = provider_order
        .iter()
        .map(|provider| (provider.clone(), compact_identity(Some(provider.as_str()))))
        .collect::<BTreeMap<_, _>>();
    let mut steps = build_steps(fact, &provider_labels);
    if steps.is_empty() {
        return None;
    }
    steps.sort_by(|left, right| {
        left.observed_at
            .unwrap_or(0)
            .cmp(&right.observed_at.unwrap_or(0))
            .then_with(|| left.label.cmp(&right.label))
    });

    Some(SparkReplayView {
        request_id: fact.request_id.clone(),
        buyer_label: compact_identity(fact.buyer_nostr_pubkey.as_deref()),
        buyer_identity,
        provider_order,
        provider_labels,
        source_quality: fact.source_quality,
        steps,
    })
}

fn build_steps(
    fact: &Nip90PaymentFact,
    provider_labels: &BTreeMap<String, String>,
) -> Vec<SparkReplayStep> {
    let mut steps = Vec::<SparkReplayStep>::new();
    let log_backfilled = fact.source_quality == Nip90PaymentFactSourceQuality::LogBackfill;

    if let Some(request_published_at) = fact.request_published_at {
        steps.push(SparkReplayStep {
            phase: SparkReplayPhase::RequestPublished,
            label: "request published".to_string(),
            provider_pubkey: None,
            provider_label: None,
            observed_at: Some(request_published_at),
            evidence_label: fact
                .request_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "request timestamp".to_string()),
            degraded: log_backfilled,
        });
    }

    let mut saw_seller_settlement = false;
    for event in &fact.provider_observation_history {
        if let Some(step) = step_from_history_event(event, provider_labels, log_backfilled) {
            if step.phase == SparkReplayPhase::SellerSettled {
                saw_seller_settlement = true;
            }
            steps.push(step);
        }
    }

    if let Some(pointer_at) = fact.buyer_payment_pointer_at {
        steps.push(SparkReplayStep {
            phase: SparkReplayPhase::BuyerPaymentPointer,
            label: "buyer payment pointer".to_string(),
            provider_pubkey: fact.provider_nostr_pubkey.clone(),
            provider_label: fact
                .provider_nostr_pubkey
                .as_deref()
                .map(|provider| compact_identity(Some(provider))),
            observed_at: Some(pointer_at),
            evidence_label: fact
                .buyer_payment_pointer
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "payment pointer".to_string()),
            degraded: log_backfilled,
        });
    }

    if !saw_seller_settlement
        && let Some(seller_settlement_feedback_at) = fact.seller_settlement_feedback_at
    {
        steps.push(SparkReplayStep {
            phase: SparkReplayPhase::SellerSettled,
            label: "seller settlement feedback".to_string(),
            provider_pubkey: fact.provider_nostr_pubkey.clone(),
            provider_label: fact
                .provider_nostr_pubkey
                .as_deref()
                .map(|provider| compact_identity(Some(provider))),
            observed_at: Some(seller_settlement_feedback_at),
            evidence_label: fact
                .seller_feedback_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "seller settlement".to_string()),
            degraded: true,
        });
    }

    if let Some(seller_wallet_confirmed_at) = fact.seller_wallet_confirmed_at {
        steps.push(SparkReplayStep {
            phase: SparkReplayPhase::SellerWalletSettled,
            label: "seller wallet settled".to_string(),
            provider_pubkey: fact.provider_nostr_pubkey.clone(),
            provider_label: fact
                .provider_nostr_pubkey
                .as_deref()
                .map(|provider| compact_identity(Some(provider))),
            observed_at: Some(seller_wallet_confirmed_at),
            evidence_label: fact
                .seller_payment_pointer
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "seller wallet settle".to_string()),
            degraded: log_backfilled,
        });
    }

    if let Some(buyer_wallet_confirmed_at) = fact.buyer_wallet_confirmed_at {
        steps.push(SparkReplayStep {
            phase: SparkReplayPhase::BuyerWalletConfirmed,
            label: "buyer wallet confirmed".to_string(),
            provider_pubkey: fact.provider_nostr_pubkey.clone(),
            provider_label: fact
                .provider_nostr_pubkey
                .as_deref()
                .map(|provider| compact_identity(Some(provider))),
            observed_at: Some(buyer_wallet_confirmed_at),
            evidence_label: fact
                .buyer_payment_hash
                .as_deref()
                .map(short_id)
                .or_else(|| fact.buyer_payment_pointer.as_deref().map(short_id))
                .unwrap_or_else(|| "wallet confirmation".to_string()),
            degraded: log_backfilled,
        });
    }

    steps
}

fn step_from_history_event(
    event: &NetworkRequestProviderObservationHistoryEvent,
    provider_labels: &BTreeMap<String, String>,
    log_backfilled: bool,
) -> Option<SparkReplayStep> {
    let provider_pubkey = event.provider_pubkey.clone();
    let provider_label = provider_pubkey
        .as_deref()
        .and_then(|provider| provider_labels.get(provider).cloned())
        .or_else(|| {
            provider_pubkey
                .as_deref()
                .map(|provider| compact_identity(Some(provider)))
        });
    let observed_at = Some(event.observed_at_epoch_ms / 1_000);
    match event.kind {
        NetworkRequestProviderObservationHistoryKind::ResultObserved => Some(SparkReplayStep {
            phase: SparkReplayPhase::ResultRace,
            label: "provider result".to_string(),
            provider_pubkey,
            provider_label,
            observed_at,
            evidence_label: event
                .observed_event_id
                .as_deref()
                .map(short_id)
                .unwrap_or_else(|| "result observed".to_string()),
            degraded: log_backfilled,
        }),
        NetworkRequestProviderObservationHistoryKind::FeedbackObserved => {
            if event
                .status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("payment-required"))
                || event.bolt11_present
            {
                Some(SparkReplayStep {
                    phase: SparkReplayPhase::InvoiceObserved,
                    label: "invoice observed".to_string(),
                    provider_pubkey,
                    provider_label,
                    observed_at,
                    evidence_label: event
                        .observed_event_id
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "payment-required".to_string()),
                    degraded: log_backfilled,
                })
            } else if event
                .status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("success"))
            {
                Some(SparkReplayStep {
                    phase: SparkReplayPhase::SellerSettled,
                    label: "seller settlement feedback".to_string(),
                    provider_pubkey,
                    provider_label,
                    observed_at,
                    evidence_label: event
                        .observed_event_id
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "seller success".to_string()),
                    degraded: log_backfilled,
                })
            } else {
                None
            }
        }
        NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected => {
            Some(SparkReplayStep {
                phase: SparkReplayPhase::WinnerSelected,
                label: "winner selected".to_string(),
                provider_pubkey,
                provider_label,
                observed_at,
                evidence_label: event
                    .winner_result_event_id
                    .as_deref()
                    .map(short_id)
                    .unwrap_or_else(|| "payable winner".to_string()),
                degraded: log_backfilled,
            })
        }
        NetworkRequestProviderObservationHistoryKind::PayableWinnerCleared => None,
    }
}

fn ordered_provider_pubkeys(fact: &Nip90PaymentFact) -> Vec<String> {
    let mut providers = BTreeSet::<String>::new();
    if let Some(provider) = fact.provider_nostr_pubkey.as_deref() {
        let normalized = provider.trim().to_string();
        if !normalized.is_empty() {
            providers.insert(normalized);
        }
    }
    for event in &fact.provider_observation_history {
        for candidate in [
            event.provider_pubkey.as_deref(),
            event.previous_provider_pubkey.as_deref(),
        ] {
            if let Some(candidate) = candidate {
                let normalized = candidate.trim().to_string();
                if !normalized.is_empty() {
                    providers.insert(normalized);
                }
            }
        }
    }
    providers.into_iter().collect()
}

fn paint_header(
    bounds: Bounds,
    view: &SparkReplayView,
    current_index: usize,
    pane_state: &SparkReplayPaneState,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "SPARK REPLAY  //  REQUEST RACE TO SETTLEMENT",
        Point::new(bounds.origin.x, bounds.origin.y + 10.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Replay is built from persisted request facts and provider-observation history, not live mutable request state alone.",
        Point::new(bounds.origin.x, bounds.origin.y + 30.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "request={}  buyer={}  providers={}  step={}/{}  source={}  mode={}",
                short_id(view.request_id.as_str()),
                view.buyer_label,
                view.provider_order.len(),
                current_index + 1,
                view.steps.len(),
                view.source_quality.label(),
                if pane_state.auto_follow {
                    "auto"
                } else {
                    "manual"
                },
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 50.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );
}

fn paint_replay_field(
    bounds: Bounds,
    view: &SparkReplayView,
    current_index: usize,
    paint: &mut PaintContext,
) {
    paint_panel_shell(bounds, "Replay Field", Hsla::from_hex(0x6ed0ff), paint);

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

    let mut scanlines = Scanlines::new()
        .spacing(18.0)
        .line_color(Hsla::from_hex(0x6ed0ff).with_alpha(0.04))
        .scan_color(Hsla::from_hex(0x6ed0ff).with_alpha(0.12))
        .scan_width(16.0)
        .scan_progress(0.62)
        .opacity(0.8);
    scanlines.paint(field_bounds, paint);

    let buyer_pos = Point::new(
        field_bounds.origin.x + 56.0,
        field_bounds.origin.y + field_bounds.size.height * 0.5,
    );
    let provider_positions = view
        .provider_order
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            (
                provider.clone(),
                Point::new(
                    field_bounds.max_x() - 72.0,
                    vertical_slot(field_bounds, index, view.provider_order.len()),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let replay_state = state_at_step(view, current_index);

    paint_node(
        buyer_pos,
        view.buyer_label.as_str(),
        Hsla::from_hex(0x6ed0ff),
        paint,
    );
    for provider in &view.provider_order {
        if let Some(position) = provider_positions.get(provider) {
            let provider_state = replay_state.providers.get(provider);
            let accent = if provider_state.is_some_and(|state| state.wallet_settled) {
                Hsla::from_hex(0x68f0b8)
            } else if provider_state.is_some_and(|state| state.payment_pointer) {
                Hsla::from_hex(0xffd463)
            } else if provider_state.is_some_and(|state| state.invoice_seen || state.result_seen) {
                Hsla::from_hex(0x6ed0ff)
            } else {
                theme::text::MUTED
            };
            paint_node(
                *position,
                view.provider_labels
                    .get(provider)
                    .map(|label| label.as_str())
                    .unwrap_or("provider"),
                accent,
                paint,
            );

            let state = provider_state.copied().unwrap_or_default();
            if state.result_seen {
                let edge_color = if state.wallet_settled {
                    Hsla::from_hex(0x68f0b8)
                } else if state.payment_pointer {
                    Hsla::from_hex(0xffd463)
                } else if state.invoice_seen {
                    Hsla::from_hex(0x8ed4ff)
                } else {
                    Hsla::from_hex(0x6ed0ff)
                };
                paint_edge(
                    buyer_pos,
                    *position,
                    edge_color.with_alpha(if state.degraded { 0.42 } else { 0.86 }),
                    if state.wallet_settled {
                        7.0
                    } else if state.payment_pointer {
                        5.0
                    } else {
                        3.5
                    },
                    paint,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ReplayProviderState {
    result_seen: bool,
    invoice_seen: bool,
    payment_pointer: bool,
    seller_settled: bool,
    wallet_settled: bool,
    degraded: bool,
}

struct ReplayAccumulatedState {
    providers: BTreeMap<String, ReplayProviderState>,
}

fn state_at_step(view: &SparkReplayView, current_index: usize) -> ReplayAccumulatedState {
    let mut providers = BTreeMap::<String, ReplayProviderState>::new();
    for step in view.steps.iter().take(current_index + 1) {
        if let Some(provider) = step.provider_pubkey.as_deref() {
            let state = providers.entry(provider.to_string()).or_default();
            state.degraded |= step.degraded;
            match step.phase {
                SparkReplayPhase::ResultRace => state.result_seen = true,
                SparkReplayPhase::WinnerSelected => state.result_seen = true,
                SparkReplayPhase::InvoiceObserved => {
                    state.result_seen = true;
                    state.invoice_seen = true;
                }
                SparkReplayPhase::BuyerPaymentPointer => {
                    state.result_seen = true;
                    state.invoice_seen = true;
                    state.payment_pointer = true;
                }
                SparkReplayPhase::SellerSettled => {
                    state.result_seen = true;
                    state.invoice_seen = true;
                    state.payment_pointer = true;
                    state.seller_settled = true;
                }
                SparkReplayPhase::SellerWalletSettled => {
                    state.result_seen = true;
                    state.invoice_seen = true;
                    state.payment_pointer = true;
                    state.seller_settled = true;
                    state.wallet_settled = true;
                }
                SparkReplayPhase::BuyerWalletConfirmed | SparkReplayPhase::RequestPublished => {}
            }
        }
    }
    ReplayAccumulatedState { providers }
}

fn paint_detail(
    bounds: Bounds,
    view: &SparkReplayView,
    step: &SparkReplayStep,
    current_index: usize,
    pane_state: &SparkReplayPaneState,
    paint: &mut PaintContext,
) {
    let accent = phase_color(step.phase);
    paint_panel_shell(bounds, "Step Detail", accent, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        format!("{}  {}", current_index + 1, step.label).as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
        12.0,
        accent.with_alpha(0.96),
    ));
    paint.scene.draw_text(paint.text.layout(
        if step.degraded {
            "Lower-confidence segment: derived or backfilled evidence is being used for this replay step."
        } else {
            "Persisted request/history evidence is available for this step."
        },
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 50.0),
        9.0,
        theme::text::MUTED,
    ));

    let mut y = bounds.origin.y + 80.0;
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "phase",
        step.phase.label(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "request",
        view.request_id.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "buyer",
        view.buyer_identity.as_str(),
    );
    if let Some(provider) = step.provider_pubkey.as_deref() {
        y = paint_label_line(paint, bounds.origin.x + 12.0, y, "provider", provider);
    }
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "evidence",
        step.evidence_label.as_str(),
    );
    y = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "observed",
        timestamp_label(step.observed_at).as_str(),
    );
    let _ = paint_label_line(
        paint,
        bounds.origin.x + 12.0,
        y,
        "cursor",
        if pane_state.auto_follow {
            "auto-follow"
        } else {
            "manual scrub"
        },
    );
}

fn paint_scrubber(
    bounds: Bounds,
    view: &SparkReplayView,
    current_index: usize,
    paint: &mut PaintContext,
) {
    paint_panel_shell(bounds, "Scrubber", Hsla::from_hex(0x6ed0ff), paint);

    let chip_count = view.steps.len().max(1);
    let gap = 6.0;
    let chip_width = ((bounds.size.width - 20.0 - gap * (chip_count.saturating_sub(1) as f32))
        / chip_count as f32)
        .max(56.0);
    for (index, step) in view.steps.iter().enumerate() {
        let chip_bounds = Bounds::new(
            bounds.origin.x + 10.0 + index as f32 * (chip_width + gap),
            bounds.origin.y + 24.0,
            chip_width.min(120.0),
            28.0,
        );
        let accent = phase_color(step.phase);
        let active = index == current_index;
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(if active {
                    accent.with_alpha(0.18)
                } else {
                    Hsla::from_hex(0x091018).with_alpha(0.88)
                })
                .with_border(
                    accent.with_alpha(if step.degraded { 0.4 } else { 0.7 }),
                    1.0,
                )
                .with_corner_radius(7.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            format!("{} {}", index + 1, step.phase.short_label()).as_str(),
            Point::new(chip_bounds.origin.x + 6.0, chip_bounds.origin.y + 7.0),
            8.4,
            if active {
                accent.with_alpha(0.96)
            } else {
                theme::text::PRIMARY
            },
        ));
        if step.degraded {
            paint.scene.draw_text(paint.text.layout(
                "degraded",
                Point::new(chip_bounds.origin.x + 6.0, chip_bounds.origin.y + 18.0),
                7.6,
                theme::status::WARNING,
            ));
        }
    }
}

fn paint_empty_state(
    content_bounds: Bounds,
    pane_state: &SparkReplayPaneState,
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
        "SPARK REPLAY",
        Point::new(
            content_bounds.origin.x + 24.0,
            content_bounds.origin.y + 62.0,
        ),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No replayable payment facts yet. The pane needs a request plus either provider-history events or settlement timestamps to build a step sequence.",
        Point::new(content_bounds.origin.x + 24.0, content_bounds.origin.y + 88.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "last_action={}  ledger_state={}",
                pane_state.last_action.as_deref().unwrap_or("none"),
                payment_facts.load_state.label()
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

fn paint_node(position: Point, label: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            position.x - 18.0,
            position.y - 18.0,
            36.0,
            36.0,
        ))
        .with_background(accent.with_alpha(0.18))
        .with_border(accent.with_alpha(0.72), 1.0)
        .with_corner_radius(18.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(position.x - 5.0, position.y - 5.0, 10.0, 10.0))
            .with_background(accent.with_alpha(0.96))
            .with_corner_radius(5.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(position.x - 34.0, position.y + 24.0),
        9.0,
        theme::text::PRIMARY,
    ));
}

fn paint_edge(from: Point, to: Point, accent: Hsla, thickness: f32, paint: &mut PaintContext) {
    let segments = 14usize;
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        let point = Point::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t + ((0.5 - t).abs() * -24.0 + 12.0),
        );
        let dot = thickness + (1.0 - (0.5 - t).abs()) * 1.8;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                point.x - dot * 0.5,
                point.y - dot * 0.5,
                dot,
                dot,
            ))
            .with_background(accent.with_alpha(0.12 + 0.88 * t))
            .with_corner_radius(dot * 0.5),
        );
    }
}

fn vertical_slot(bounds: Bounds, index: usize, count: usize) -> f32 {
    if count <= 1 {
        return bounds.origin.y + bounds.size.height * 0.5;
    }
    let gap = bounds.size.height / count as f32;
    bounds.origin.y + gap * (index as f32 + 0.5)
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

fn phase_color(phase: SparkReplayPhase) -> Hsla {
    match phase {
        SparkReplayPhase::RequestPublished => Hsla::from_hex(0x6ed0ff),
        SparkReplayPhase::ResultRace => Hsla::from_hex(0x7cbcff),
        SparkReplayPhase::WinnerSelected => Hsla::from_hex(0xffd463),
        SparkReplayPhase::InvoiceObserved => Hsla::from_hex(0xffd463),
        SparkReplayPhase::BuyerPaymentPointer => Hsla::from_hex(0xffd463),
        SparkReplayPhase::SellerSettled => Hsla::from_hex(0x68f0b8),
        SparkReplayPhase::SellerWalletSettled => Hsla::from_hex(0x68f0b8),
        SparkReplayPhase::BuyerWalletConfirmed => Hsla::from_hex(0xfff2a0),
    }
}

fn timestamp_label(epoch_seconds: Option<u64>) -> String {
    epoch_seconds
        .and_then(|value| Local.timestamp_opt(value as i64, 0).single())
        .map(|value| value.format("%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}

impl SparkReplayPhase {
    fn label(self) -> &'static str {
        match self {
            Self::RequestPublished => "request_published",
            Self::ResultRace => "result_race",
            Self::WinnerSelected => "winner_selected",
            Self::InvoiceObserved => "invoice_observed",
            Self::BuyerPaymentPointer => "buyer_payment_pointer",
            Self::SellerSettled => "seller_settled",
            Self::SellerWalletSettled => "seller_wallet_settled",
            Self::BuyerWalletConfirmed => "buyer_wallet_confirmed",
        }
    }

    fn short_label(self) -> &'static str {
        match self {
            Self::RequestPublished => "REQ",
            Self::ResultRace => "RACE",
            Self::WinnerSelected => "WIN",
            Self::InvoiceObserved => "INV",
            Self::BuyerPaymentPointer => "PAY",
            Self::SellerSettled => "SET",
            Self::SellerWalletSettled => "WAL",
            Self::BuyerWalletConfirmed => "CONF",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SparkReplayPhase, build_view_from_facts};
    use crate::state::nip90_payment_facts::{
        Nip90PaymentFact, Nip90PaymentFactSourceQuality, Nip90PaymentFactStatus,
    };
    use crate::state::operations::{
        NetworkRequestProviderObservationHistoryEvent, NetworkRequestProviderObservationHistoryKind,
    };

    #[test]
    fn spark_replay_builds_steps_from_persisted_fact_history() {
        let view = build_view_from_facts(&[fact_with_history(
            Nip90PaymentFactSourceQuality::SellerWalletReconciled,
        )])
        .expect("replay view should exist");

        assert_eq!(
            view.steps.first().map(|step| step.phase),
            Some(SparkReplayPhase::RequestPublished)
        );
        assert!(
            view.steps
                .iter()
                .any(|step| step.phase == SparkReplayPhase::ResultRace)
        );
        assert!(
            view.steps
                .iter()
                .any(|step| step.phase == SparkReplayPhase::InvoiceObserved)
        );
        assert!(
            view.steps
                .iter()
                .any(|step| step.phase == SparkReplayPhase::BuyerPaymentPointer)
        );
        assert!(
            view.steps
                .iter()
                .any(|step| step.phase == SparkReplayPhase::SellerSettled)
        );
        assert!(
            view.steps
                .iter()
                .any(|step| step.phase == SparkReplayPhase::BuyerWalletConfirmed)
        );
    }

    #[test]
    fn spark_replay_marks_log_backfill_steps_as_degraded() {
        let view = build_view_from_facts(&[fact_with_history(
            Nip90PaymentFactSourceQuality::LogBackfill,
        )])
        .expect("replay view should exist");

        assert!(view.steps.iter().all(|step| step.degraded));
    }

    fn fact_with_history(source_quality: Nip90PaymentFactSourceQuality) -> Nip90PaymentFact {
        Nip90PaymentFact {
            fact_id: "fact-replay".to_string(),
            request_id: "req-replay".to_string(),
            request_type: "kind5050".to_string(),
            request_event_id: Some("request-event".to_string()),
            result_event_id: Some("result-event".to_string()),
            invoice_event_id: Some("invoice-event".to_string()),
            seller_feedback_event_id: Some("seller-event".to_string()),
            buyer_nostr_pubkey: Some("npub1buyer".to_string()),
            provider_nostr_pubkey: Some("npub1providera".to_string()),
            invoice_provider_pubkey: Some("npub1providera".to_string()),
            result_provider_pubkey: Some("npub1providera".to_string()),
            invoice_observed_relays: Vec::new(),
            result_observed_relays: Vec::new(),
            lightning_destination_pubkey: None,
            buyer_payment_pointer: Some("pointer-replay".to_string()),
            seller_payment_pointer: Some("seller-pointer".to_string()),
            buyer_payment_hash: Some("hash-replay".to_string()),
            amount_sats: Some(21),
            fees_sats: Some(1),
            total_debit_sats: Some(22),
            wallet_method: Some("lightning".to_string()),
            status: Nip90PaymentFactStatus::SellerWalletSettled,
            settlement_authority: "wallet.reconciliation".to_string(),
            request_published_at: Some(10),
            result_observed_at: Some(20),
            invoice_observed_at: Some(26),
            buyer_payment_pointer_at: Some(30),
            seller_settlement_feedback_at: Some(40),
            buyer_wallet_confirmed_at: Some(52),
            seller_wallet_confirmed_at: Some(48),
            selected_relays: Vec::new(),
            publish_accepted_relays: Vec::new(),
            publish_rejected_relays: Vec::new(),
            provider_observation_history: vec![
                history_event(
                    1,
                    20_000,
                    NetworkRequestProviderObservationHistoryKind::ResultObserved,
                    Some("npub1providera"),
                    Some("result-a"),
                    None,
                    false,
                ),
                history_event(
                    2,
                    22_000,
                    NetworkRequestProviderObservationHistoryKind::ResultObserved,
                    Some("npub1providerb"),
                    Some("result-b"),
                    None,
                    false,
                ),
                history_event(
                    3,
                    26_000,
                    NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
                    Some("npub1providera"),
                    Some("invoice-a"),
                    Some("payment-required"),
                    true,
                ),
                history_event(
                    4,
                    28_000,
                    NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected,
                    Some("npub1providera"),
                    Some("result-a"),
                    None,
                    false,
                ),
                history_event(
                    5,
                    40_000,
                    NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
                    Some("npub1providera"),
                    Some("seller-a"),
                    Some("success"),
                    false,
                ),
            ],
            source_quality,
        }
    }

    fn history_event(
        order: u32,
        observed_at_epoch_ms: u64,
        kind: NetworkRequestProviderObservationHistoryKind,
        provider_pubkey: Option<&str>,
        observed_event_id: Option<&str>,
        status: Option<&str>,
        bolt11_present: bool,
    ) -> NetworkRequestProviderObservationHistoryEvent {
        NetworkRequestProviderObservationHistoryEvent {
            history_id: format!("history-{order}"),
            observed_order: order,
            observed_at_epoch_ms,
            kind,
            provider_pubkey: provider_pubkey.map(ToString::to_string),
            relay_urls: Vec::new(),
            observed_event_id: observed_event_id.map(ToString::to_string),
            status: status.map(ToString::to_string),
            status_extra: None,
            amount_msats: None,
            bolt11_present,
            previous_provider_pubkey: None,
            winner_result_event_id: observed_event_id.map(ToString::to_string),
            winner_feedback_event_id: None,
            selection_source: None,
        }
    }
}
