use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;
use wgpui::{
    Bounds, Component, Hsla, PaintContext, Point, Quad, RiveFitMode, RiveHandle, RiveSurface,
    SvgQuad, theme,
};
use winit::keyboard::{Key as WinitLogicalKey, NamedKey as WinitNamedKey};

use crate::app_state::{
    MissionControlLocalRuntimeLane, PaneLoadState, RenderState, mission_control_local_runtime_lane,
};
use crate::pane_registry::{
    HOTBAR_COMMAND_PALETTE_SHORTCUT, HOTBAR_COMMAND_PALETTE_TOOLTIP, HOTBAR_SLOT_EARNINGS_JOBS,
    HOTBAR_SLOT_LOG_STREAM, HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_PROVIDER_CONTROL,
    HOTBAR_SLOT_SPARK_WALLET, pane_spec_for_hotbar_slot,
};
use crate::pane_renderer::{
    mission_control_cyan_color, mission_control_green_color, mission_control_panel_border_color,
    mission_control_panel_color, mission_control_text_color, paint_disabled_button,
    paint_mission_control_section_panel, paint_primary_button, split_text_for_display,
};
use crate::pane_system::mission_control_layout_for_mode;
use crate::rive_assets::simple_fui_hud_asset;
use crate::runtime_log;

const ONBOARDING_SCHEMA_VERSION: u32 = 1;
const ONBOARDING_VERSION: &str = env!("CARGO_PKG_VERSION");
const MODAL_HEADER_HEIGHT: f32 = 28.0;
const MODAL_OUTER_PAD: f32 = 18.0;
const MODAL_COLUMN_GAP: f32 = 18.0;
const MODAL_STEP_ROW_HEIGHT: f32 = 56.0;
const HOTKEYS_TARGET_WIDTH: f32 = 296.0;
const HOTKEYS_TARGET_HEIGHT: f32 = 84.0;
const HOTKEYS_TARGET_INSET: f32 = 26.0;
const TOUR_FOCUS_INSET: f32 = 10.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OnboardingPhase {
    SetupModal,
    TourHotkeys,
    TourSellCompute,
    Done,
    Skipped,
}

impl OnboardingPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SetupModal => "setup_modal",
            Self::TourHotkeys => "tour_hotkeys",
            Self::TourSellCompute => "tour_sell_compute",
            Self::Done => "done",
            Self::Skipped => "skipped",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::SetupModal | Self::TourHotkeys | Self::TourSellCompute
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SetupStepId {
    LightningWallet,
    NetworkConfiguration,
    EstablishingConnection,
}

impl SetupStepId {
    const ALL: [Self; 3] = [
        Self::LightningWallet,
        Self::NetworkConfiguration,
        Self::EstablishingConnection,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::LightningWallet => "lightning_wallet_setup",
            Self::NetworkConfiguration => "network_configuration",
            Self::EstablishingConnection => "establishing_connection",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::LightningWallet => "Lightning wallet setup",
            Self::NetworkConfiguration => "Network configuration",
            Self::EstablishingConnection => "Establishing connection",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SetupRowStatus {
    Pending,
    Active,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SetupCompletionSnapshot {
    wallet_ready: bool,
    network_ready: bool,
    connection_ready: bool,
}

impl SetupCompletionSnapshot {
    const fn all_complete(self) -> bool {
        self.wallet_ready && self.network_ready && self.connection_ready
    }

    const fn flags(self) -> [bool; 3] {
        [self.wallet_ready, self.network_ready, self.connection_ready]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OnboardingDocumentV1 {
    schema_version: u32,
    phase: OnboardingPhase,
    shown_at_epoch_ms: Option<u64>,
    setup_completed_at_epoch_ms: Option<u64>,
    completed_at_epoch_ms: Option<u64>,
    skipped_at_epoch_ms: Option<u64>,
    last_seen_app_version: Option<String>,
}

pub struct OnboardingState {
    file_path: PathBuf,
    pub phase: OnboardingPhase,
    pub shown_at_epoch_ms: Option<u64>,
    pub setup_completed_at_epoch_ms: Option<u64>,
    pub completed_at_epoch_ms: Option<u64>,
    pub skipped_at_epoch_ms: Option<u64>,
    pub last_seen_app_version: Option<String>,
    pub animation_surface: Option<RiveSurface>,
    pub animation_last_error: Option<String>,
    pub animation_last_action: Option<String>,
    animation_last_applied_fit_mode: Option<RiveFitMode>,
    animation_last_applied_playing: Option<bool>,
    logged_active_step: Option<SetupStepId>,
    logged_tour_hotkeys: bool,
    logged_tour_sell_compute: bool,
}

impl OnboardingState {
    pub fn load_or_default() -> Self {
        Self::load_or_default_at(default_file_path())
    }

    fn load_or_default_at(file_path: PathBuf) -> Self {
        let mut state = Self {
            file_path,
            phase: OnboardingPhase::SetupModal,
            shown_at_epoch_ms: None,
            setup_completed_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            skipped_at_epoch_ms: None,
            last_seen_app_version: Some(ONBOARDING_VERSION.to_string()),
            animation_surface: None,
            animation_last_error: None,
            animation_last_action: None,
            animation_last_applied_fit_mode: None,
            animation_last_applied_playing: None,
            logged_active_step: None,
            logged_tour_hotkeys: false,
            logged_tour_sell_compute: false,
        };

        let raw = match std::fs::read_to_string(&state.file_path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return state,
            Err(error) => {
                tracing::warn!(
                    "Autopilot onboarding state read failed for {}: {}",
                    state.file_path.display(),
                    error
                );
                return state;
            }
        };

        let Ok(document) = serde_json::from_str::<OnboardingDocumentV1>(&raw) else {
            tracing::warn!(
                "Autopilot onboarding state parse failed for {}",
                state.file_path.display()
            );
            return state;
        };
        if document.schema_version != ONBOARDING_SCHEMA_VERSION {
            tracing::warn!(
                "Unsupported onboarding schema {} in {}",
                document.schema_version,
                state.file_path.display()
            );
            return state;
        }

        state.phase = document.phase;
        state.shown_at_epoch_ms = document.shown_at_epoch_ms;
        state.setup_completed_at_epoch_ms = document.setup_completed_at_epoch_ms;
        state.completed_at_epoch_ms = document.completed_at_epoch_ms;
        state.skipped_at_epoch_ms = document.skipped_at_epoch_ms;
        state.last_seen_app_version = document
            .last_seen_app_version
            .or_else(|| Some(ONBOARDING_VERSION.to_string()));
        state
    }

    pub const fn is_active(&self) -> bool {
        self.phase.is_active()
    }

    fn persist(&mut self) {
        self.last_seen_app_version = Some(ONBOARDING_VERSION.to_string());
        let document = OnboardingDocumentV1 {
            schema_version: ONBOARDING_SCHEMA_VERSION,
            phase: self.phase,
            shown_at_epoch_ms: self.shown_at_epoch_ms,
            setup_completed_at_epoch_ms: self.setup_completed_at_epoch_ms,
            completed_at_epoch_ms: self.completed_at_epoch_ms,
            skipped_at_epoch_ms: self.skipped_at_epoch_ms,
            last_seen_app_version: self.last_seen_app_version.clone(),
        };
        let payload = match serde_json::to_string_pretty(&document) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!("Autopilot onboarding encode failed: {}", error);
                return;
            }
        };
        if let Some(parent) = self.file_path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                "Autopilot onboarding dir init failed for {}: {}",
                parent.display(),
                error
            );
            return;
        }
        let temp_path = self.file_path.with_extension("tmp");
        if let Err(error) = std::fs::write(&temp_path, payload) {
            tracing::warn!(
                "Autopilot onboarding temp write failed for {}: {}",
                temp_path.display(),
                error
            );
            return;
        }
        if let Err(error) = std::fs::rename(&temp_path, &self.file_path) {
            tracing::warn!(
                "Autopilot onboarding persist failed for {}: {}",
                self.file_path.display(),
                error
            );
        }
    }

    fn note_shown_if_needed(&mut self) {
        if self.shown_at_epoch_ms.is_some() {
            return;
        }
        let now_epoch_ms = current_timestamp_ms();
        self.shown_at_epoch_ms = Some(now_epoch_ms);
        runtime_log::record_control_event(
            "onboarding.shown",
            "Displayed first-run onboarding overlay",
            json!({
                "phase": self.phase.label(),
                "app_version": ONBOARDING_VERSION,
            }),
        );
        runtime_log::record_control_event(
            "onboarding.setup_started",
            "Started first-run setup flow",
            json!({
                "phase": self.phase.label(),
            }),
        );
        self.persist();
    }

    fn note_setup_complete_if_needed(&mut self) {
        if self.setup_completed_at_epoch_ms.is_some() {
            return;
        }
        self.setup_completed_at_epoch_ms = Some(current_timestamp_ms());
        runtime_log::record_control_event(
            "onboarding.setup_completed",
            "Completed onboarding setup checks",
            json!({
                "phase": self.phase.label(),
            }),
        );
        self.persist();
    }

    fn note_tour_phase_if_needed(&mut self) {
        match self.phase {
            OnboardingPhase::TourHotkeys if !self.logged_tour_hotkeys => {
                self.logged_tour_hotkeys = true;
                runtime_log::record_control_event(
                    "onboarding.tour_hotkeys_shown",
                    "Displayed hotkeys onboarding coachmark",
                    json!({
                        "phase": self.phase.label(),
                    }),
                );
            }
            OnboardingPhase::TourSellCompute if !self.logged_tour_sell_compute => {
                self.logged_tour_sell_compute = true;
                runtime_log::record_control_event(
                    "onboarding.tour_sell_compute_shown",
                    "Displayed sell compute onboarding coachmark",
                    json!({
                        "phase": self.phase.label(),
                    }),
                );
            }
            _ => {}
        }
    }

    fn note_active_step_if_needed(&mut self, step: Option<SetupStepId>) {
        if self.logged_active_step == step {
            return;
        }
        self.logged_active_step = step;
        let Some(step) = step else {
            return;
        };
        runtime_log::record_control_event(
            "onboarding.setup_step_started",
            format!("Active onboarding step: {}", step.label()),
            json!({
                "phase": self.phase.label(),
                "step": step.key(),
            }),
        );
    }

    fn advance_from_setup_cta(&mut self) {
        if self.phase != OnboardingPhase::SetupModal {
            return;
        }
        runtime_log::record_control_event(
            "onboarding.cta_clicked",
            "Dismissed onboarding setup modal",
            json!({
                "phase": self.phase.label(),
                "cta": "start_earning_bitcoin",
            }),
        );
        self.phase = OnboardingPhase::TourHotkeys;
        self.persist();
        self.note_tour_phase_if_needed();
    }

    fn advance_tour(&mut self) {
        if self.phase != OnboardingPhase::TourHotkeys {
            return;
        }
        self.phase = OnboardingPhase::TourSellCompute;
        self.persist();
        self.note_tour_phase_if_needed();
    }

    fn complete_tour(&mut self) {
        if !matches!(
            self.phase,
            OnboardingPhase::TourHotkeys | OnboardingPhase::TourSellCompute
        ) {
            return;
        }
        self.phase = OnboardingPhase::Done;
        self.completed_at_epoch_ms = Some(current_timestamp_ms());
        runtime_log::record_control_event(
            "onboarding.completed",
            "Completed first-run onboarding tour",
            json!({
                "phase": self.phase.label(),
            }),
        );
        self.persist();
    }

    fn skip_tour(&mut self) {
        if !matches!(
            self.phase,
            OnboardingPhase::TourHotkeys | OnboardingPhase::TourSellCompute
        ) {
            return;
        }
        self.phase = OnboardingPhase::Skipped;
        self.skipped_at_epoch_ms = Some(current_timestamp_ms());
        runtime_log::record_control_event(
            "onboarding.skipped",
            "Skipped first-run onboarding tour",
            json!({
                "phase": self.phase.label(),
            }),
        );
        self.persist();
    }
}

#[derive(Clone, Debug)]
struct SetupProgressView {
    statuses: [SetupRowStatus; 3],
    active_step: Option<SetupStepId>,
    cta_enabled: bool,
    detail_lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct OnboardingView {
    pub phase: OnboardingPhase,
    progress: Option<SetupProgressView>,
}

#[derive(Clone, Copy, Debug)]
struct SetupModalLayout {
    modal: Bounds,
    left_column: Bounds,
    right_column: Bounds,
    step_rows: [Bounds; 3],
    status_block: Bounds,
    cta: Bounds,
    animation_bounds: Bounds,
}

#[derive(Clone, Copy, Debug)]
struct TourHotkeysLayout {
    target: Bounds,
    card: Bounds,
    advance_button: Bounds,
    caret: Bounds,
}

#[derive(Clone, Copy, Debug)]
struct TourSellComputeLayout {
    focus: Bounds,
    card: Bounds,
    close_button: Bounds,
    caret: Bounds,
}

#[derive(Clone, Debug)]
struct HotkeyLegendEntry {
    key: String,
    label: String,
}

pub fn sync_progress(state: &mut RenderState) {
    if !state.onboarding.is_active() {
        return;
    }
    state.onboarding.note_shown_if_needed();
    if state.onboarding.phase == OnboardingPhase::SetupModal {
        let progress = derive_setup_progress(state);
        state
            .onboarding
            .note_active_step_if_needed(progress.active_step);
        if progress.cta_enabled {
            state.onboarding.note_setup_complete_if_needed();
        }
    } else {
        state.onboarding.note_tour_phase_if_needed();
    }
}

pub fn build_view(state: &RenderState) -> OnboardingView {
    OnboardingView {
        phase: state.onboarding.phase,
        progress: (state.onboarding.phase == OnboardingPhase::SetupModal)
            .then(|| derive_setup_progress(state)),
    }
}

pub fn blocks_root_input(state: &RenderState) -> bool {
    state.onboarding.is_active()
}

pub fn handle_mouse_down(state: &mut RenderState) -> bool {
    state.onboarding.is_active()
}

pub fn handle_mouse_up(state: &mut RenderState, point: Point) -> bool {
    if !state.onboarding.is_active() {
        return false;
    }
    let root_bounds = root_bounds_for_state(state);
    match state.onboarding.phase {
        OnboardingPhase::SetupModal => {
            let progress = derive_setup_progress(state);
            if progress.cta_enabled && setup_modal_layout(root_bounds).cta.contains(point) {
                state.onboarding.advance_from_setup_cta();
            }
            true
        }
        OnboardingPhase::TourHotkeys => {
            if tour_hotkeys_layout(root_bounds)
                .advance_button
                .contains(point)
            {
                state.onboarding.advance_tour();
            }
            true
        }
        OnboardingPhase::TourSellCompute => {
            if tour_sell_compute_layout(root_bounds)
                .close_button
                .contains(point)
            {
                state.onboarding.complete_tour();
            }
            true
        }
        OnboardingPhase::Done | OnboardingPhase::Skipped => false,
    }
}

pub fn handle_keyboard(state: &mut RenderState, logical_key: &WinitLogicalKey) -> bool {
    if !state.onboarding.is_active() {
        return false;
    }
    if matches!(logical_key, WinitLogicalKey::Named(WinitNamedKey::Escape))
        && matches!(
            state.onboarding.phase,
            OnboardingPhase::TourHotkeys | OnboardingPhase::TourSellCompute
        )
    {
        state.onboarding.skip_tour();
        return true;
    }
    true
}

pub fn paint_overlay(
    onboarding: &mut OnboardingState,
    view: &OnboardingView,
    root_bounds: Bounds,
    paint: &mut PaintContext,
) {
    if !onboarding.is_active() {
        return;
    }
    match view.phase {
        OnboardingPhase::SetupModal => {
            if let Some(progress) = view.progress.as_ref() {
                paint_setup_modal(onboarding, progress, root_bounds, paint);
            }
        }
        OnboardingPhase::TourHotkeys => paint_hotkeys_tour(root_bounds, paint),
        OnboardingPhase::TourSellCompute => paint_sell_compute_tour(root_bounds, paint),
        OnboardingPhase::Done | OnboardingPhase::Skipped => {}
    }
}

fn paint_setup_modal(
    onboarding: &mut OnboardingState,
    progress: &SetupProgressView,
    root_bounds: Bounds,
    paint: &mut PaintContext,
) {
    let layout = setup_modal_layout(root_bounds);
    paint_overlay_scrim(root_bounds, paint);
    paint_mission_control_section_panel(
        layout.modal,
        "Initializing User Account",
        mission_control_green_color(),
        false,
        paint,
    );

    let step_fill = Bounds::new(
        layout.left_column.origin.x,
        layout.step_rows[0].origin.y,
        layout.left_column.size.width,
        layout.cta.max_y() - layout.step_rows[0].origin.y,
    );
    paint.scene.draw_quad(
        Quad::new(step_fill)
            .with_background(mission_control_panel_color().with_alpha(0.44))
            .with_corner_radius(8.0),
    );

    for (index, step) in SetupStepId::ALL.into_iter().enumerate() {
        paint_setup_step_row(
            layout.step_rows[index],
            step.label(),
            progress.statuses[index],
            paint,
        );
    }

    paint_status_block(layout.status_block, &progress.detail_lines, paint);
    if progress.cta_enabled {
        paint_primary_button(layout.cta, "Start Earning Bitcoin", paint);
    } else {
        paint_disabled_button(layout.cta, "Start Earning Bitcoin", paint);
    }

    paint.scene.draw_quad(
        Quad::new(layout.right_column)
            .with_background(mission_control_panel_color().with_alpha(0.44))
            .with_border(mission_control_panel_border_color(), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "LOCAL RIVE",
        Point::new(
            layout.right_column.origin.x + 14.0,
            layout.right_column.origin.y + 14.0,
        ),
        10.0,
        mission_control_cyan_color(),
    ));
    paint_setup_animation(onboarding, layout.animation_bounds, paint);
}

fn paint_hotkeys_tour(root_bounds: Bounds, paint: &mut PaintContext) {
    let layout = tour_hotkeys_layout(root_bounds);
    paint_focus_scrim(root_bounds, layout.target, paint);
    paint_hotkeys_target(layout.target, paint);
    paint_focus_outline(layout.target, mission_control_cyan_color(), paint);
    paint_white_callout(layout.card, paint);
    paint.scene.draw_text(paint.text.layout(
        "Hotkeys",
        Point::new(layout.card.origin.x + 16.0, layout.card.origin.y + 18.0),
        14.0,
        Hsla::black(),
    ));
    for (index, line) in split_text_for_display(
        "Open control, identity, wallet, earnings, logs, and command palette instantly.",
        34,
    )
    .into_iter()
    .enumerate()
    {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(
                layout.card.origin.x + 16.0,
                layout.card.origin.y + 42.0 + index as f32 * 14.0,
            ),
            11.0,
            Hsla::black().with_alpha(0.86),
        ));
    }
    paint.scene.draw_quad(
        Quad::new(layout.advance_button)
            .with_background(Hsla::black())
            .with_corner_radius(3.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        ">",
        Point::new(
            layout.advance_button.origin.x + 14.0,
            layout.advance_button.origin.y + 8.0,
        ),
        16.0,
        Hsla::white(),
    ));
    paint_callout_caret(layout.caret, CalloutCaretDirection::Down, paint);
}

fn paint_sell_compute_tour(root_bounds: Bounds, paint: &mut PaintContext) {
    let layout = tour_sell_compute_layout(root_bounds);
    paint_focus_scrim(root_bounds, layout.focus, paint);
    paint_focus_outline(layout.focus, mission_control_green_color(), paint);
    paint_white_callout(layout.card, paint);
    paint.scene.draw_text(paint.text.layout(
        "Earn Bitcoin",
        Point::new(layout.card.origin.x + 16.0, layout.card.origin.y + 18.0),
        14.0,
        Hsla::black(),
    ));
    let supporting_lines = split_text_for_display(
        "Sell your compute to other agents and stack sats in the process.",
        28,
    );
    for (index, line) in supporting_lines.into_iter().enumerate() {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(
                layout.card.origin.x + 16.0,
                layout.card.origin.y + 44.0 + index as f32 * 14.0,
            ),
            11.0,
            Hsla::black().with_alpha(0.86),
        ));
    }
    paint.scene.draw_quad(
        Quad::new(layout.close_button)
            .with_background(Hsla::black().with_alpha(0.08))
            .with_corner_radius(3.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "X",
        Point::new(
            layout.close_button.origin.x + 7.0,
            layout.close_button.origin.y + 6.0,
        ),
        12.0,
        Hsla::black(),
    ));
    paint_callout_caret(layout.caret, CalloutCaretDirection::Left, paint);
}

fn paint_setup_step_row(
    bounds: Bounds,
    label: &str,
    status: SetupRowStatus,
    paint: &mut PaintContext,
) {
    let (fill, border, text_color) = match status {
        SetupRowStatus::Pending => (
            mission_control_panel_color().with_alpha(0.55),
            mission_control_panel_border_color(),
            mission_control_text_color().with_alpha(0.68),
        ),
        SetupRowStatus::Active => (
            mission_control_panel_color().with_alpha(0.9),
            mission_control_cyan_color(),
            mission_control_text_color(),
        ),
        SetupRowStatus::Complete => (
            mission_control_panel_color().with_alpha(0.75),
            mission_control_green_color().with_alpha(0.8),
            mission_control_text_color(),
        ),
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(fill)
            .with_border(border, 1.0)
            .with_corner_radius(8.0),
    );
    let icon_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + bounds.size.height * 0.5 - 10.0,
        20.0,
        20.0,
    );
    match status {
        SetupRowStatus::Complete => {
            draw_check_icon(icon_bounds, mission_control_green_color(), paint)
        }
        SetupRowStatus::Active => draw_pulse_dot(icon_bounds, mission_control_cyan_color(), paint),
        SetupRowStatus::Pending => draw_pending_dot(icon_bounds, paint),
    }
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(icon_bounds.max_x() + 12.0, bounds.origin.y + 20.0),
        12.0,
        text_color,
    ));
}

fn paint_status_block(bounds: Bounds, lines: &[String], paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color().with_alpha(0.3))
            .with_corner_radius(6.0),
    );
    let mut y = bounds.origin.y + 12.0;
    for line in lines.iter().take(3) {
        for wrapped in split_text_for_display(line, 48) {
            paint.scene.draw_text(paint.text.layout(
                &wrapped,
                Point::new(bounds.origin.x + 12.0, y),
                10.5,
                mission_control_text_color().with_alpha(0.72),
            ));
            y += 14.0;
        }
    }
}

fn paint_setup_animation(
    onboarding: &mut OnboardingState,
    bounds: Bounds,
    paint: &mut PaintContext,
) {
    ensure_animation_loaded(onboarding);
    sync_animation_state(onboarding);
    if let Some(surface) = onboarding.animation_surface.as_mut() {
        surface.paint(bounds, paint);
        return;
    }
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color().with_alpha(0.55))
            .with_border(mission_control_panel_border_color(), 1.0)
            .with_corner_radius(8.0),
    );
    let fallback = onboarding
        .animation_last_error
        .clone()
        .unwrap_or_else(|| "Packaged Rive placeholder unavailable".to_string());
    for (index, line) in split_text_for_display(&fallback, 28)
        .into_iter()
        .enumerate()
    {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(
                bounds.origin.x + 14.0,
                bounds.origin.y + 24.0 + index as f32 * 14.0,
            ),
            10.0,
            mission_control_text_color().with_alpha(0.72),
        ));
    }
}

fn ensure_animation_loaded(onboarding: &mut OnboardingState) {
    if onboarding.animation_surface.is_some() || onboarding.animation_last_error.is_some() {
        return;
    }
    let asset = simple_fui_hud_asset();
    match RiveSurface::from_bytes_with_handles(
        asset.bytes,
        RiveHandle::Default,
        RiveHandle::Default,
        None,
    ) {
        Ok(surface) => {
            onboarding.animation_surface = Some(surface);
            onboarding.animation_last_error = None;
            onboarding.animation_last_action =
                Some("Loaded packaged onboarding Rive surface".to_string());
        }
        Err(error) => {
            onboarding.animation_last_error = Some(error.to_string());
            onboarding.animation_last_action =
                Some("Failed to load packaged onboarding Rive surface".to_string());
        }
    }
}

fn sync_animation_state(onboarding: &mut OnboardingState) {
    let Some(surface) = onboarding.animation_surface.as_mut() else {
        return;
    };
    let desired_fit_mode = RiveFitMode::Contain;
    let desired_playing = false;
    let mut changed = false;
    if onboarding.animation_last_applied_fit_mode != Some(desired_fit_mode) {
        surface.controller_mut().set_fit_mode(desired_fit_mode);
        onboarding.animation_last_applied_fit_mode = Some(desired_fit_mode);
        changed = true;
    }
    if onboarding.animation_last_applied_playing != Some(desired_playing) {
        surface.controller_mut().pause();
        onboarding.animation_last_applied_playing = Some(desired_playing);
        changed = true;
    }
    if changed {
        surface.mark_dirty();
    }
}

fn paint_hotkeys_target(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color().with_alpha(0.96))
            .with_border(mission_control_panel_border_color(), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "HOTKEYS",
        Point::new(bounds.origin.x + 14.0, bounds.origin.y + 10.0),
        10.0,
        mission_control_green_color(),
    ));
    let entries = hotkey_legend_entries();
    let column_width = (bounds.size.width - 24.0) * 0.5;
    for (index, entry) in entries.into_iter().enumerate() {
        let row = index % 3;
        let column = index / 3;
        let x = bounds.origin.x + 12.0 + column as f32 * column_width;
        let y = bounds.origin.y + 28.0 + row as f32 * 17.0;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(x, y, 20.0, 14.0))
                .with_background(mission_control_green_color().with_alpha(0.18))
                .with_border(mission_control_green_color().with_alpha(0.5), 1.0)
                .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &entry.key,
            Point::new(x + 5.0, y + 2.0),
            9.0,
            mission_control_text_color(),
        ));
        paint.scene.draw_text(paint.text.layout(
            &entry.label,
            Point::new(x + 28.0, y + 1.0),
            10.0,
            mission_control_text_color().with_alpha(0.86),
        ));
    }
}

fn hotkey_legend_entries() -> Vec<HotkeyLegendEntry> {
    [
        HOTBAR_SLOT_PROVIDER_CONTROL,
        HOTBAR_SLOT_NOSTR_IDENTITY,
        HOTBAR_SLOT_SPARK_WALLET,
        HOTBAR_SLOT_EARNINGS_JOBS,
        HOTBAR_SLOT_LOG_STREAM,
    ]
    .into_iter()
    .filter_map(|slot| {
        let spec = pane_spec_for_hotbar_slot(slot)?;
        let hotbar = spec.hotbar?;
        Some(HotkeyLegendEntry {
            key: hotbar
                .shortcut
                .unwrap_or(match slot {
                    HOTBAR_SLOT_PROVIDER_CONTROL => "1",
                    HOTBAR_SLOT_NOSTR_IDENTITY => "2",
                    HOTBAR_SLOT_SPARK_WALLET => "3",
                    HOTBAR_SLOT_EARNINGS_JOBS => "4",
                    HOTBAR_SLOT_LOG_STREAM => "5",
                    _ => "?",
                })
                .to_string(),
            label: hotbar.tooltip.to_string(),
        })
    })
    .chain(std::iter::once(HotkeyLegendEntry {
        key: HOTBAR_COMMAND_PALETTE_SHORTCUT.to_string(),
        label: HOTBAR_COMMAND_PALETTE_TOOLTIP.to_string(),
    }))
    .collect()
}

fn draw_check_icon(bounds: Bounds, color: Hsla, paint: &mut PaintContext) {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path fill="none" stroke="#{hex:06X}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" d="M4 10.5 8.1 14.4 16 5.8"/></svg>"##,
        hex = 0x1F8A44
    );
    paint.scene.draw_svg(SvgQuad {
        bounds,
        svg_data: std::sync::Arc::from(svg.into_bytes().into_boxed_slice()),
        tint: Some(color),
        opacity: 1.0,
    });
}

fn draw_pending_dot(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 6.0,
            bounds.origin.y + 6.0,
            8.0,
            8.0,
        ))
        .with_background(mission_control_panel_border_color())
        .with_corner_radius(4.0),
    );
}

fn draw_pulse_dot(bounds: Bounds, color: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 3.0,
            bounds.origin.y + 3.0,
            14.0,
            14.0,
        ))
        .with_background(color.with_alpha(0.22))
        .with_corner_radius(7.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 7.0,
            bounds.origin.y + 7.0,
            6.0,
            6.0,
        ))
        .with_background(color)
        .with_corner_radius(3.0),
    );
}

fn paint_white_callout(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::white())
            .with_corner_radius(3.0),
    );
}

#[derive(Clone, Copy)]
enum CalloutCaretDirection {
    Down,
    Left,
}

fn paint_callout_caret(bounds: Bounds, direction: CalloutCaretDirection, paint: &mut PaintContext) {
    let points = match direction {
        CalloutCaretDirection::Down => "0,0 16,0 8,10",
        CalloutCaretDirection::Left => "10,0 10,16 0,8",
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><polygon points="{points}" fill="#FFFFFF"/></svg>"##
    );
    paint.scene.draw_svg(SvgQuad {
        bounds,
        svg_data: std::sync::Arc::from(svg.into_bytes().into_boxed_slice()),
        tint: Some(Hsla::white()),
        opacity: 1.0,
    });
}

fn paint_overlay_scrim(bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds).with_background(theme::theme().colors.overlay_scrim.with_alpha(0.86)),
    );
}

fn paint_focus_scrim(root_bounds: Bounds, focus_bounds: Bounds, paint: &mut PaintContext) {
    let focus = expand_bounds(focus_bounds, TOUR_FOCUS_INSET);
    let top_height = (focus.origin.y - root_bounds.origin.y).max(0.0);
    let left_width = (focus.origin.x - root_bounds.origin.x).max(0.0);
    let right_x = focus.max_x().min(root_bounds.max_x());
    let bottom_y = focus.max_y().min(root_bounds.max_y());
    let scrim = theme::theme().colors.overlay_scrim.with_alpha(0.88);
    if top_height > 0.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                root_bounds.origin.x,
                root_bounds.origin.y,
                root_bounds.size.width,
                top_height,
            ))
            .with_background(scrim),
        );
    }
    if left_width > 0.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                root_bounds.origin.x,
                focus.origin.y,
                left_width,
                focus.size.height,
            ))
            .with_background(scrim),
        );
    }
    let right_width = (root_bounds.max_x() - right_x).max(0.0);
    if right_width > 0.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                right_x,
                focus.origin.y,
                right_width,
                focus.size.height,
            ))
            .with_background(scrim),
        );
    }
    let bottom_height = (root_bounds.max_y() - bottom_y).max(0.0);
    if bottom_height > 0.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                root_bounds.origin.x,
                bottom_y,
                root_bounds.size.width,
                bottom_height,
            ))
            .with_background(scrim),
        );
    }
}

fn paint_focus_outline(bounds: Bounds, color: Hsla, paint: &mut PaintContext) {
    let outline = expand_bounds(bounds, 2.0);
    paint.scene.draw_quad(
        Quad::new(outline)
            .with_border(color.with_alpha(0.92), 2.0)
            .with_corner_radius(10.0),
    );
}

fn setup_modal_layout(root_bounds: Bounds) -> SetupModalLayout {
    let width = (root_bounds.size.width - 72.0).min(840.0).max(640.0);
    let height = (root_bounds.size.height - 88.0).min(452.0).max(380.0);
    let modal = Bounds::new(
        root_bounds.origin.x + (root_bounds.size.width - width) * 0.5,
        root_bounds.origin.y + (root_bounds.size.height - height) * 0.5,
        width,
        height,
    );
    let content_y = modal.origin.y + MODAL_HEADER_HEIGHT + MODAL_OUTER_PAD;
    let content_height = (modal.max_y() - content_y - MODAL_OUTER_PAD).max(0.0);
    let left_width = (modal.size.width * 0.6 - MODAL_OUTER_PAD * 2.0).max(260.0);
    let right_width =
        (modal.size.width - left_width - MODAL_OUTER_PAD * 2.0 - MODAL_COLUMN_GAP).max(180.0);
    let left_column = Bounds::new(
        modal.origin.x + MODAL_OUTER_PAD,
        content_y,
        left_width,
        content_height,
    );
    let right_column = Bounds::new(
        left_column.max_x() + MODAL_COLUMN_GAP,
        content_y,
        right_width,
        content_height,
    );
    let mut step_rows = [Bounds::ZERO; 3];
    for (index, row_bounds) in step_rows.iter_mut().enumerate() {
        *row_bounds = Bounds::new(
            left_column.origin.x,
            left_column.origin.y + index as f32 * (MODAL_STEP_ROW_HEIGHT + 10.0),
            left_column.size.width,
            MODAL_STEP_ROW_HEIGHT,
        );
    }
    let status_block = Bounds::new(
        left_column.origin.x,
        step_rows[2].max_y() + 12.0,
        left_column.size.width,
        52.0,
    );
    let cta = Bounds::new(
        left_column.origin.x,
        left_column.max_y() - 44.0,
        left_column.size.width,
        44.0,
    );
    let animation_bounds = Bounds::new(
        right_column.origin.x + 12.0,
        right_column.origin.y + 24.0,
        (right_column.size.width - 24.0).max(0.0),
        (right_column.size.height - 36.0).max(0.0),
    );
    SetupModalLayout {
        modal,
        left_column,
        right_column,
        step_rows,
        status_block,
        cta,
        animation_bounds,
    }
}

fn tour_hotkeys_layout(root_bounds: Bounds) -> TourHotkeysLayout {
    let target = hotkeys_target_bounds(root_bounds);
    let card_width: f32 = 292.0;
    let card_height: f32 = 94.0;
    let card = Bounds::new(
        target.origin.x,
        (target.origin.y - card_height - 20.0).max(root_bounds.origin.y + 18.0),
        card_width.min(root_bounds.size.width - 36.0),
        card_height,
    );
    let advance_button = Bounds::new(card.max_x() - 40.0, card.origin.y + 12.0, 28.0, 28.0);
    let caret = Bounds::new(
        target.origin.x + target.size.width * 0.5 - 8.0,
        card.max_y(),
        16.0,
        10.0,
    );
    TourHotkeysLayout {
        target,
        card,
        advance_button,
        caret,
    }
}

fn tour_sell_compute_layout(root_bounds: Bounds) -> TourSellComputeLayout {
    let focus = mission_control_layout_for_mode(root_bounds, false).sell_panel;
    let card_width = 280.0;
    let card_height = 110.0;
    let card_x = (focus.max_x() + 26.0).min(root_bounds.max_x() - card_width - 20.0);
    let card_y = (focus.origin.y + 20.0).min(root_bounds.max_y() - card_height - 20.0);
    let card = Bounds::new(card_x, card_y, card_width, card_height);
    let close_button = Bounds::new(card.max_x() - 28.0, card.origin.y + 10.0, 18.0, 18.0);
    let caret = Bounds::new(card.origin.x - 10.0, card.origin.y + 28.0, 10.0, 16.0);
    TourSellComputeLayout {
        focus,
        card,
        close_button,
        caret,
    }
}

fn hotkeys_target_bounds(root_bounds: Bounds) -> Bounds {
    Bounds::new(
        root_bounds.origin.x + HOTKEYS_TARGET_INSET,
        root_bounds.max_y() - HOTKEYS_TARGET_INSET - HOTKEYS_TARGET_HEIGHT,
        HOTKEYS_TARGET_WIDTH.min(root_bounds.size.width - HOTKEYS_TARGET_INSET * 2.0),
        HOTKEYS_TARGET_HEIGHT,
    )
}

fn derive_setup_progress(state: &RenderState) -> SetupProgressView {
    let completion = derive_setup_completion(state);
    let statuses = setup_row_statuses(completion.flags());
    let active_step = statuses
        .iter()
        .position(|status| *status == SetupRowStatus::Active)
        .map(|index| SetupStepId::ALL[index]);
    SetupProgressView {
        statuses,
        active_step,
        cta_enabled: completion.all_complete(),
        detail_lines: derive_setup_detail_lines(state, completion),
    }
}

fn derive_setup_completion(state: &RenderState) -> SetupCompletionSnapshot {
    let configured_relays = state.configured_provider_relay_urls();
    let wallet_ready = state.nostr_identity.is_some()
        && state.spark_wallet.identity_path.is_some()
        && state.spark_wallet.last_error.is_none()
        && (state.spark_wallet.network_status.is_some()
            || state.spark_wallet.balance.is_some()
            || state.spark_wallet.spark_address.is_some()
            || state.spark_wallet.bitcoin_address.is_some());
    let network_ready = state.settings.load_state == PaneLoadState::Ready
        && state.settings.last_error.is_none()
        && !configured_relays.is_empty()
        && state.provider_nip90_lane.configured_relays == configured_relays;
    let connection_ready = match mission_control_local_runtime_lane(
        state.desktop_shell_mode,
        &state.gpt_oss_execution,
    ) {
        Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
            state.provider_runtime.apple_fm.bridge_status.is_some()
                || state.provider_runtime.apple_fm.reachable
                || state.provider_runtime.apple_fm.ready_model.is_some()
                || state
                    .provider_runtime
                    .apple_fm
                    .availability_error_message()
                    .is_some()
        }
        Some(MissionControlLocalRuntimeLane::GptOss) => {
            state.gpt_oss_execution.reachable
                || state.gpt_oss_execution.ready_model.is_some()
                || state.gpt_oss_execution.artifact_present
                || state.gpt_oss_execution.last_error.is_some()
        }
        None => true,
    };
    SetupCompletionSnapshot {
        wallet_ready,
        network_ready,
        connection_ready,
    }
}

fn derive_setup_detail_lines(
    state: &RenderState,
    completion: SetupCompletionSnapshot,
) -> Vec<String> {
    if !completion.wallet_ready {
        let wallet_status = state.spark_wallet.network_status_label();
        let mut lines = vec![format!("Wallet bootstrap: {}", wallet_status)];
        if let Some(action) = state.spark_wallet.last_action.as_deref() {
            lines.push(action.to_string());
        }
        if let Some(error) = state.spark_wallet.last_error.as_deref() {
            lines.push(format!("Wallet error: {error}"));
        }
        return lines;
    }
    if !completion.network_ready {
        let relay_count = state.configured_provider_relay_urls().len();
        let mut lines = vec![format!("Relay bundle: {relay_count} configured relay(s)")];
        if let Some(action) = state.provider_nip90_lane.last_action.as_deref() {
            lines.push(action.to_string());
        }
        if let Some(error) = state.provider_nip90_lane.last_error.as_deref() {
            lines.push(format!("Relay lane error: {error}"));
        }
        return lines;
    }
    if !completion.connection_ready {
        let mut lines = Vec::new();
        if let Some(status) = state.provider_runtime.apple_fm.bridge_status.as_deref() {
            lines.push(format!("Apple FM bridge: {status}"));
        } else if !state.gpt_oss_execution.backend_label.trim().is_empty() {
            lines.push(format!(
                "Runtime backend: {}",
                state.gpt_oss_execution.backend_label
            ));
        }
        if let Some(action) = state.provider_control.last_action.as_deref() {
            lines.push(action.to_string());
        }
        if let Some(error) = state.provider_control.last_error.as_deref() {
            lines.push(format!("Runtime note: {error}"));
        }
        if lines.is_empty() {
            lines.push("Preparing local runtime preflight.".to_string());
        }
        return lines;
    }
    vec![
        "User setup complete.".to_string(),
        "Click Start Earning Bitcoin to continue into Mission Control.".to_string(),
    ]
}

fn setup_row_statuses(completed: [bool; 3]) -> [SetupRowStatus; 3] {
    let mut statuses = [SetupRowStatus::Pending; 3];
    let mut prior_steps_complete = true;
    for (index, is_complete) in completed.into_iter().enumerate() {
        statuses[index] = if prior_steps_complete && is_complete {
            SetupRowStatus::Complete
        } else if prior_steps_complete {
            SetupRowStatus::Active
        } else {
            SetupRowStatus::Pending
        };
        prior_steps_complete &= is_complete;
    }
    statuses
}

fn root_bounds_for_state(state: &RenderState) -> Bounds {
    let logical = crate::render::logical_size(&state.config, state.scale_factor);
    Bounds::new(0.0, 0.0, logical.width, logical.height)
}

fn expand_bounds(bounds: Bounds, inset: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x - inset,
        bounds.origin.y - inset,
        bounds.size.width + inset * 2.0,
        bounds.size.height + inset * 2.0,
    )
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn default_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-onboarding-v1.json")
}

#[cfg(test)]
mod tests {
    use super::{
        OnboardingPhase, OnboardingState, SetupRowStatus, hotkeys_target_bounds,
        setup_row_statuses, tour_sell_compute_layout,
    };
    use wgpui::Bounds;

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "openagents-onboarding-{label}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    #[test]
    fn setup_row_statuses_stay_sequential() {
        assert_eq!(
            setup_row_statuses([false, true, true]),
            [
                SetupRowStatus::Active,
                SetupRowStatus::Pending,
                SetupRowStatus::Pending
            ]
        );
        assert_eq!(
            setup_row_statuses([true, false, true]),
            [
                SetupRowStatus::Complete,
                SetupRowStatus::Active,
                SetupRowStatus::Pending
            ]
        );
        assert_eq!(
            setup_row_statuses([true, true, true]),
            [
                SetupRowStatus::Complete,
                SetupRowStatus::Complete,
                SetupRowStatus::Complete
            ]
        );
    }

    #[test]
    fn onboarding_state_round_trips_done_phase() {
        let path = unique_temp_path("persist");
        let mut state = OnboardingState::load_or_default_at(path.clone());
        state.phase = OnboardingPhase::TourSellCompute;
        state.complete_tour();

        let reloaded = OnboardingState::load_or_default_at(path.clone());
        assert_eq!(reloaded.phase, OnboardingPhase::Done);
        assert!(reloaded.completed_at_epoch_ms.is_some());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn hotkeys_target_stays_bottom_left() {
        let root = Bounds::new(0.0, 0.0, 1280.0, 800.0);
        let target = hotkeys_target_bounds(root);
        assert!(target.origin.x < root.size.width * 0.25);
        assert!(target.max_y() > root.size.height - 120.0);
    }

    #[test]
    fn sell_compute_callout_stays_to_the_right_of_focus() {
        let root = Bounds::new(0.0, 0.0, 1280.0, 800.0);
        let layout = tour_sell_compute_layout(root);
        assert!(layout.card.origin.x >= layout.focus.max_x() - 12.0);
    }

    #[test]
    fn escape_skips_only_tour_phases() {
        let path = unique_temp_path("skip");
        let mut setup = OnboardingState::load_or_default_at(path.clone());
        setup.skip_tour();
        assert_eq!(setup.phase, OnboardingPhase::SetupModal);

        let mut tour = OnboardingState::load_or_default_at(path.clone());
        tour.phase = OnboardingPhase::TourHotkeys;
        tour.skip_tour();
        assert_eq!(tour.phase, OnboardingPhase::Skipped);

        let _ = std::fs::remove_file(path);
    }
}
