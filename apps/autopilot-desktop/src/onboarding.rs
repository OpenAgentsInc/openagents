use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};
use wgpui::tools::load_image_from_path;
use wgpui::{
    theme, Bounds, Hsla, ImageData, ImageQuad, ImageSource, PaintContext, Point, Quad, SvgQuad,
};
use winit::keyboard::{Key as WinitLogicalKey, NamedKey as WinitNamedKey};
use winit::window::CursorIcon;

use openagents_spark::NetworkStatus;

use crate::app_state::{PaneLoadState, RelayConnectionStatus, RenderState, SyncRecoveryPhase};
use crate::pane_registry::{
    pane_spec_for_hotbar_slot, HOTBAR_COMMAND_PALETTE_SHORTCUT, HOTBAR_COMMAND_PALETTE_TOOLTIP,
    HOTBAR_SLOT_EARNINGS_JOBS, HOTBAR_SLOT_LOG_STREAM, HOTBAR_SLOT_NOSTR_IDENTITY,
    HOTBAR_SLOT_PROVIDER_CONTROL, HOTBAR_SLOT_SPARK_WALLET,
};
use crate::pane_renderer::{
    mission_control_cyan_color, mission_control_green_color, mission_control_panel_border_color,
    mission_control_panel_color, mission_control_text_color, paint_disabled_button,
    paint_mission_control_go_online_button, paint_mission_control_section_panel,
    split_text_for_display,
};
use crate::pane_system::mission_control_layout_for_mode;
use crate::runtime_log;

const ONBOARDING_SCHEMA_VERSION: u32 = 1;
const ONBOARDING_VERSION: &str = env!("CARGO_PKG_VERSION");
const MODAL_HEADER_HEIGHT: f32 = 28.0;
const MODAL_OUTER_PAD: f32 = 18.0;
const MODAL_COLUMN_GAP: f32 = 18.0;
const MODAL_STEP_ROW_HEIGHT: f32 = 56.0;
const SETUP_LOADING_DOT_INTERVAL_MS: u64 = 400;
const ONBOARDING_LOTTIE_SUPERSAMPLE_SCALE: f32 = 2.0;
const HOTKEYS_TARGET_WIDTH: f32 = 296.0;
const HOTKEYS_TARGET_HEIGHT: f32 = 84.0;
const HOTKEYS_TARGET_INSET: f32 = 26.0;
const TOUR_FOCUS_INSET: f32 = 10.0;
const ONBOARDING_LOTTIE_JSON: &str = include_str!("../resources/lottie/onboarding-hud-effect.json");
const ONBOARDING_LOTTIE_CACHE_KEY: &str = "autopilot-onboarding-hud-effect";
const FORCE_ONBOARDING_ENV: &str = "AUTOPILOT_FORCE_ONBOARDING";
const ONBOARDING_LOTTIE_VISIBLE_START_FRAME: usize = 96;
const TOUR_ADVANCE_ARROW_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M566.6 342.6C579.1 330.1 579.1 309.8 566.6 297.3L406.6 137.3C394.1 124.8 373.8 124.8 361.3 137.3C348.8 149.8 348.8 170.1 361.3 182.6L466.7 288L96 288C78.3 288 64 302.3 64 320C64 337.7 78.3 352 96 352L466.7 352L361.3 457.4C348.8 469.9 348.8 490.2 361.3 502.7C373.8 515.2 394.1 515.2 406.6 502.7L566.6 342.7z"/></svg>"##;
const TOUR_CLOSE_X_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M183.1 137.4C170.6 124.9 150.3 124.9 137.8 137.4C125.3 149.9 125.3 170.2 137.8 182.7L275.2 320L137.9 457.4C125.4 469.9 125.4 490.2 137.9 502.7C150.4 515.2 170.7 515.2 183.2 502.7L320.5 365.3L457.9 502.6C470.4 515.1 490.7 515.1 503.2 502.6C515.7 490.1 515.7 469.8 503.2 457.3L365.8 320L503.1 182.6C515.6 170.1 515.6 149.8 503.1 137.3C490.6 124.8 470.3 124.8 457.8 137.3L320.5 274.7L183.1 137.4z"/></svg>"##;

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

struct OnboardingLottiePlayer {
    composition: OnboardingLottieComposition,
    started_at: Instant,
    last_frame: Option<usize>,
    cached_size: Option<(usize, usize)>,
    cached_image: Option<ImageData>,
}

impl OnboardingLottiePlayer {
    fn from_packaged_json() -> Result<Self, String> {
        let composition = OnboardingLottieComposition::from_packaged_json()?;
        Ok(Self {
            composition,
            started_at: Instant::now(),
            last_frame: None,
            cached_size: None,
            cached_image: None,
        })
    }

    fn source_width(&self) -> f32 {
        self.composition.source_width as f32
    }

    fn source_height(&self) -> f32 {
        self.composition.source_height as f32
    }

    fn frame_for_now(&self, now: Instant) -> usize {
        let total_frames = self.composition.total_frames.max(1);
        let fps = self.composition.fps.max(1.0);
        let elapsed = now.saturating_duration_since(self.started_at).as_secs_f64();
        let visible_start =
            ONBOARDING_LOTTIE_VISIBLE_START_FRAME.min(total_frames.saturating_sub(1));
        let visible_len = total_frames.saturating_sub(visible_start).max(1);
        visible_start + (((elapsed * fps).floor() as usize) % visible_len)
    }

    fn render_image(&mut self, bounds: Bounds) -> Option<ImageQuad> {
        let fitted_bounds = aspect_fit_bounds(bounds, self.source_width(), self.source_height());
        let width = (fitted_bounds.size.width * ONBOARDING_LOTTIE_SUPERSAMPLE_SCALE)
            .round()
            .max(1.0) as usize;
        let height = (fitted_bounds.size.height * ONBOARDING_LOTTIE_SUPERSAMPLE_SCALE)
            .round()
            .max(1.0) as usize;
        let frame = self.frame_for_now(Instant::now());
        let cache_key = (width, height);
        if self.last_frame != Some(frame)
            || self.cached_size != Some(cache_key)
            || self.cached_image.is_none()
        {
            self.cached_image =
                self.composition
                    .render_frame(frame as f32, width as u32, height as u32);
            self.last_frame = Some(frame);
            self.cached_size = Some(cache_key);
        }

        Some(ImageQuad::new(
            fitted_bounds,
            ImageSource::Rgba8(self.cached_image.clone()?),
        ))
    }
}

#[derive(Clone)]
struct OnboardingLottieComposition {
    source_width: u32,
    source_height: u32,
    fps: f64,
    total_frames: usize,
    layers: Vec<OnboardingLottieLayer>,
    assets: std::collections::HashMap<String, Pixmap>,
}

#[derive(Clone)]
struct OnboardingLottieLayer {
    ref_id: String,
    in_frame: f32,
    out_frame: f32,
    opacity: AnimatedScalar,
    rotation_degrees: AnimatedScalar,
    position: AnimatedVec2,
    anchor: AnimatedVec2,
    scale: AnimatedVec2,
}

#[derive(Clone)]
enum AnimatedScalar {
    Static(f32),
    Keyframes(Vec<ScalarKeyframe>),
}

#[derive(Clone)]
struct ScalarKeyframe {
    t: f32,
    s: f32,
}

#[derive(Clone)]
enum AnimatedVec2 {
    Static([f32; 2]),
    Keyframes(Vec<Vec2Keyframe>),
}

#[derive(Clone)]
struct Vec2Keyframe {
    t: f32,
    s: [f32; 2],
}

#[derive(Deserialize)]
struct OnboardingLottieDocument {
    w: u32,
    h: u32,
    fr: f64,
    op: f32,
    assets: Vec<OnboardingLottieAsset>,
    layers: Vec<OnboardingLottieLayerDocument>,
}

#[derive(Deserialize)]
struct OnboardingLottieAsset {
    id: String,
    u: Option<String>,
    p: Option<String>,
}

#[derive(Deserialize)]
struct OnboardingLottieLayerDocument {
    ty: u8,
    #[serde(rename = "refId")]
    ref_id: Option<String>,
    parent: Option<i64>,
    ip: Option<f32>,
    op: Option<f32>,
    ks: OnboardingLottieTransformDocument,
}

#[derive(Deserialize)]
struct OnboardingLottieTransformDocument {
    o: Value,
    r: Value,
    p: Value,
    a: Value,
    s: Value,
}

impl OnboardingLottieComposition {
    fn from_packaged_json() -> Result<Self, String> {
        let document = serde_json::from_str::<OnboardingLottieDocument>(ONBOARDING_LOTTIE_JSON)
            .map_err(|error| format!("Failed to parse packaged onboarding Lottie JSON: {error}"))?;

        let mut assets = std::collections::HashMap::new();
        for asset in &document.assets {
            let Some(path) = asset_file_path(asset) else {
                continue;
            };
            let decoded = load_image_from_path(&path)
                .map_err(|_| format!("Failed to load Lottie asset {}", path.display()))?;
            let Some(size) = IntSize::from_wh(decoded.width, decoded.height) else {
                return Err(format!(
                    "Invalid Lottie asset dimensions for {}: {}x{}",
                    path.display(),
                    decoded.width,
                    decoded.height
                ));
            };
            let Some(pixmap) = Pixmap::from_vec(decoded.pixels, size) else {
                return Err(format!("Failed to decode Lottie pixmap {}", path.display()));
            };
            assets.insert(asset.id.clone(), pixmap);
        }

        let mut layers = Vec::new();
        for layer in document.layers {
            if layer.ty != 2 || layer.parent.is_some() {
                continue;
            }
            let Some(ref_id) = layer.ref_id else {
                continue;
            };
            if !assets.contains_key(&ref_id) {
                continue;
            }
            layers.push(OnboardingLottieLayer {
                ref_id,
                in_frame: layer.ip.unwrap_or(0.0),
                out_frame: layer.op.unwrap_or(document.op),
                opacity: AnimatedScalar::parse(&layer.ks.o)?,
                rotation_degrees: AnimatedScalar::parse(&layer.ks.r)?,
                position: AnimatedVec2::parse(&layer.ks.p)?,
                anchor: AnimatedVec2::parse(&layer.ks.a)?,
                scale: AnimatedVec2::parse(&layer.ks.s)?,
            });
        }

        Ok(Self {
            source_width: document.w,
            source_height: document.h,
            fps: document.fr,
            total_frames: document.op.ceil().max(1.0) as usize,
            layers,
            assets,
        })
    }

    fn render_frame(&self, frame: f32, width: u32, height: u32) -> Option<ImageData> {
        let mut canvas = Pixmap::new(width, height)?;
        let root_scale_x = width as f32 / self.source_width.max(1) as f32;
        let root_scale_y = height as f32 / self.source_height.max(1) as f32;

        for layer in self.layers.iter().rev() {
            if frame < layer.in_frame || frame >= layer.out_frame {
                continue;
            }
            let Some(asset) = self.assets.get(&layer.ref_id) else {
                continue;
            };

            let opacity = (layer.opacity.value_at(frame) / 100.0).clamp(0.0, 1.0);
            if opacity <= 0.001 {
                continue;
            }
            let scale = layer.scale.value_at(frame);
            let scale_x = scale[0] / 100.0;
            let scale_y = scale[1] / 100.0;
            if scale_x.abs() <= 0.0001 || scale_y.abs() <= 0.0001 {
                continue;
            }
            let position = layer.position.value_at(frame);
            let anchor = layer.anchor.value_at(frame);
            let rotation = layer.rotation_degrees.value_at(frame).to_radians();
            let cos = rotation.cos();
            let sin = rotation.sin();
            let transform = Transform::from_row(
                root_scale_x * cos * scale_x,
                root_scale_y * sin * scale_x,
                root_scale_x * -sin * scale_y,
                root_scale_y * cos * scale_y,
                root_scale_x
                    * (position[0] - cos * scale_x * anchor[0] + sin * scale_y * anchor[1]),
                root_scale_y
                    * (position[1] - sin * scale_x * anchor[0] - cos * scale_y * anchor[1]),
            );
            let mut paint = PixmapPaint::default();
            paint.opacity = opacity;
            paint.quality = FilterQuality::Bilinear;
            canvas.draw_pixmap(0, 0, asset.as_ref(), &paint, transform, None);
        }

        ImageData::rgba8(width, height, Arc::<[u8]>::from(canvas.take()))
    }
}

impl AnimatedScalar {
    fn parse(value: &Value) -> Result<Self, String> {
        let animated = value.get("a").and_then(Value::as_u64).unwrap_or_default() == 1;
        let Some(keyframes) = value.get("k") else {
            return Err("Missing animated scalar payload".to_string());
        };
        if !animated {
            return Ok(Self::Static(json_scalar_value(keyframes)?));
        }
        let Some(items) = keyframes.as_array() else {
            return Err("Animated scalar keyframes were not an array".to_string());
        };
        let mut parsed = Vec::new();
        for item in items {
            let t = item.get("t").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let Some(s) = item.get("s") else {
                continue;
            };
            parsed.push(ScalarKeyframe {
                t,
                s: json_scalar_value(s)?,
            });
        }
        if parsed.is_empty() {
            return Err("Animated scalar keyframes were empty".to_string());
        }
        Ok(Self::Keyframes(parsed))
    }

    fn value_at(&self, frame: f32) -> f32 {
        match self {
            Self::Static(value) => *value,
            Self::Keyframes(keyframes) => interpolate_scalar_keyframes(keyframes, frame),
        }
    }
}

impl AnimatedVec2 {
    fn parse(value: &Value) -> Result<Self, String> {
        let animated = value.get("a").and_then(Value::as_u64).unwrap_or_default() == 1;
        let Some(keyframes) = value.get("k") else {
            return Err("Missing animated vec2 payload".to_string());
        };
        if !animated {
            return Ok(Self::Static(json_vec2_value(keyframes)?));
        }
        let Some(items) = keyframes.as_array() else {
            return Err("Animated vec2 keyframes were not an array".to_string());
        };
        let mut parsed = Vec::new();
        for item in items {
            let t = item.get("t").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let Some(s) = item.get("s") else {
                continue;
            };
            parsed.push(Vec2Keyframe {
                t,
                s: json_vec2_value(s)?,
            });
        }
        if parsed.is_empty() {
            return Err("Animated vec2 keyframes were empty".to_string());
        }
        Ok(Self::Keyframes(parsed))
    }

    fn value_at(&self, frame: f32) -> [f32; 2] {
        match self {
            Self::Static(value) => *value,
            Self::Keyframes(keyframes) => interpolate_vec2_keyframes(keyframes, frame),
        }
    }
}

pub struct OnboardingState {
    file_path: PathBuf,
    pub phase: OnboardingPhase,
    pub shown_at_epoch_ms: Option<u64>,
    pub setup_completed_at_epoch_ms: Option<u64>,
    pub completed_at_epoch_ms: Option<u64>,
    pub skipped_at_epoch_ms: Option<u64>,
    pub last_seen_app_version: Option<String>,
    animation_player: Option<OnboardingLottiePlayer>,
    pub animation_last_error: Option<String>,
    pub animation_last_action: Option<String>,
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
            animation_player: None,
            animation_last_error: None,
            animation_last_action: None,
            logged_active_step: None,
            logged_tour_hotkeys: false,
            logged_tour_sell_compute: false,
        };

        if force_onboarding_override_enabled() {
            return state;
        }

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

    fn animation_needs_redraw(&self) -> bool {
        self.phase == OnboardingPhase::SetupModal
            && self.animation_last_error.is_none()
            && self.animation_player.is_some()
    }

    fn persist(&mut self) {
        if force_onboarding_override_enabled() {
            return;
        }
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
    loading_dot_count: usize,
}

#[derive(Clone, Debug, Default)]
struct SetupRuntimeSnapshot {
    wallet_connected: bool,
    wallet_balance_known: bool,
    wallet_error: Option<String>,
    identity_ready: bool,
    identity_error: Option<String>,
    configured_relay_count: usize,
    connected_relay_count: usize,
    connecting_relay_count: usize,
    errored_relay_count: usize,
    control_runtime_ready: bool,
    control_runtime_error: Option<String>,
    sync_required: bool,
    sync_ready: bool,
    sync_error: Option<String>,
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

pub fn animation_needs_redraw(state: &RenderState) -> bool {
    state.onboarding.animation_needs_redraw() || derive_setup_progress(state).active_step.is_some()
}

pub fn blocks_root_input(state: &RenderState) -> bool {
    state.onboarding.is_active()
}

pub fn cursor_icon(state: &RenderState, point: Point) -> Option<CursorIcon> {
    if !state.onboarding.is_active() {
        return None;
    }
    let root_bounds = root_bounds_for_state(state);
    match state.onboarding.phase {
        OnboardingPhase::SetupModal => {
            let progress = derive_setup_progress(state);
            let cta_bounds = setup_modal_layout(root_bounds).cta;
            if progress.cta_enabled && cta_bounds.contains(point) {
                Some(CursorIcon::Pointer)
            } else {
                Some(CursorIcon::Default)
            }
        }
        OnboardingPhase::TourHotkeys => {
            let layout = tour_hotkeys_layout(root_bounds);
            if layout.advance_button.contains(point) {
                Some(CursorIcon::Pointer)
            } else {
                Some(CursorIcon::Default)
            }
        }
        OnboardingPhase::TourSellCompute => {
            let layout = tour_sell_compute_layout(root_bounds);
            if layout.close_button.contains(point) {
                Some(CursorIcon::Pointer)
            } else {
                Some(CursorIcon::Default)
            }
        }
        OnboardingPhase::Done | OnboardingPhase::Skipped => None,
    }
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
        "INITIALIZING USER ACCOUNT",
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
            (progress.active_step == Some(step)).then_some(progress.loading_dot_count),
            paint,
        );
    }

    paint_status_block(layout.status_block, &progress.detail_lines, paint);
    if progress.cta_enabled {
        paint_mission_control_go_online_button(
            layout.cta,
            "START EARNING BITCOIN",
            true,
            mission_control_green_color(),
            paint,
        );
    } else {
        paint_disabled_button(layout.cta, "START EARNING BITCOIN", paint);
    }

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
    paint.scene.draw_svg(SvgQuad {
        bounds: Bounds::new(
            layout.advance_button.origin.x + 6.0,
            layout.advance_button.origin.y + 6.0,
            16.0,
            16.0,
        ),
        svg_data: Arc::from(TOUR_ADVANCE_ARROW_SVG.as_bytes()),
        tint: Some(Hsla::white()),
        opacity: 1.0,
    });
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
            .with_background(Hsla::black())
            .with_corner_radius(3.0),
    );
    paint.scene.draw_svg(SvgQuad {
        bounds: Bounds::new(
            layout.close_button.origin.x + 6.0,
            layout.close_button.origin.y + 6.0,
            16.0,
            16.0,
        ),
        svg_data: Arc::from(TOUR_CLOSE_X_SVG.as_bytes()),
        tint: Some(Hsla::white()),
        opacity: 1.0,
    });
    paint_callout_caret(layout.caret, CalloutCaretDirection::Left, paint);
}

fn paint_setup_step_row(
    bounds: Bounds,
    label: &str,
    status: SetupRowStatus,
    loading_dot_count: Option<usize>,
    paint: &mut PaintContext,
) {
    let loading_gray = Hsla::from_hex(0x555B66);
    let (text_color, icon_color) = match status {
        SetupRowStatus::Pending | SetupRowStatus::Active => (loading_gray, loading_gray),
        SetupRowStatus::Complete => (Hsla::white(), mission_control_green_color()),
    };
    let icon_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + bounds.size.height * 0.5 - 10.0,
        20.0,
        20.0,
    );
    draw_check_icon(icon_bounds, icon_color, paint);

    let display_label = if let Some(dot_count) = loading_dot_count {
        let dots = ".".repeat(dot_count.clamp(1, 3));
        format!("{label} {dots}")
    } else {
        label.to_string()
    };
    let label_y = bounds.origin.y + bounds.size.height * 0.5 - 12.0;
    paint.scene.draw_text(paint.text.layout(
        &display_label,
        Point::new(icon_bounds.max_x() + 12.0, label_y),
        14.0,
        text_color,
    ));
}

fn paint_status_block(bounds: Bounds, lines: &[String], paint: &mut PaintContext) {
    if lines.is_empty() {
        return;
    }
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
    if let Some(player) = onboarding.animation_player.as_mut()
        && let Some(image) = player.render_image(bounds)
    {
        paint.scene.draw_image(image);
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
        .unwrap_or_else(|| "Packaged onboarding Lottie preview unavailable".to_string());
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
    if onboarding.animation_player.is_some() || onboarding.animation_last_error.is_some() {
        return;
    }
    match OnboardingLottiePlayer::from_packaged_json() {
        Ok(player) => {
            onboarding.animation_player = Some(player);
            onboarding.animation_last_error = None;
            onboarding.animation_last_action =
                Some("Loaded packaged onboarding Lottie animation".to_string());
        }
        Err(error) => {
            onboarding.animation_last_error = Some(error);
            onboarding.animation_last_action =
                Some("Failed to load packaged onboarding Lottie animation".to_string());
        }
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
        mission_control_cyan_color(),
    ));
    let entries = hotkey_legend_entries();
    let column_width = (bounds.size.width - 24.0) * 0.5;
    let chip_width = 24.0;
    let chip_height = 14.0;
    let chip_accent = mission_control_cyan_color();
    for (index, entry) in entries.into_iter().enumerate() {
        let row = index % 3;
        let column = index / 3;
        let x = bounds.origin.x + 12.0 + column as f32 * column_width;
        let y = bounds.origin.y + 28.0 + row as f32 * 17.0;
        let chip_bounds = Bounds::new(x, y, chip_width, chip_height);
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(chip_accent.with_alpha(0.18))
                .with_border(chip_accent.with_alpha(0.5), 1.0)
                .with_corner_radius(3.0),
        );
        let key_font_size = 9.0;
        let estimated_key_width = entry.key.chars().count() as f32 * key_font_size * 0.6;
        paint.scene.draw_text(paint.text.layout_mono(
            &entry.key,
            Point::new(
                chip_bounds.origin.x + ((chip_width - estimated_key_width) * 0.5).max(2.0),
                chip_bounds.origin.y + 2.0,
            ),
            key_font_size,
            mission_control_text_color(),
        ));
        paint.scene.draw_text(paint.text.layout(
            &entry.label,
            Point::new(chip_bounds.max_x() + 8.0, y + 1.0),
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
    let cta_left_inset = 12.0;
    let cta_right_inset = 20.0;
    let cta = Bounds::new(
        left_column.origin.x + cta_left_inset,
        left_column.max_y() - 59.0,
        (left_column.size.width - cta_left_inset - cta_right_inset).max(220.0),
        44.0,
    );
    let animation_bounds = Bounds::new(
        right_column.origin.x,
        right_column.origin.y,
        right_column.size.width.max(0.0),
        right_column.size.height.max(0.0),
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
    let close_button = Bounds::new(card.max_x() - 40.0, card.origin.y + 10.0, 28.0, 28.0);
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

fn packaged_lottie_resource_dir() -> &'static Path {
    static RESOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    RESOURCE_DIR
        .get_or_init(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("lottie")
        })
        .as_path()
}

fn asset_file_path(asset: &OnboardingLottieAsset) -> Option<PathBuf> {
    let mut path = packaged_lottie_resource_dir().to_path_buf();
    let relative_dir = asset
        .u
        .as_deref()
        .unwrap_or_default()
        .trim_start_matches('/');
    if !relative_dir.is_empty() {
        path = path.join(relative_dir);
    }
    path = path.join(asset.p.as_deref()?);
    Some(path)
}

fn force_onboarding_override_enabled() -> bool {
    #[cfg(debug_assertions)]
    {
        matches!(
            std::env::var(FORCE_ONBOARDING_ENV).ok().as_deref(),
            Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
        )
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

fn json_scalar_value(value: &Value) -> Result<f32, String> {
    if let Some(number) = value.as_f64() {
        return Ok(number as f32);
    }
    let Some(items) = value.as_array() else {
        return Err("Expected scalar or scalar array".to_string());
    };
    items
        .first()
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .ok_or_else(|| "Expected scalar array item".to_string())
}

fn json_vec2_value(value: &Value) -> Result<[f32; 2], String> {
    let Some(items) = value.as_array() else {
        return Err("Expected vec2 array".to_string());
    };
    let x = items
        .first()
        .and_then(Value::as_f64)
        .ok_or_else(|| "Expected vec2 x component".to_string())? as f32;
    let y = items
        .get(1)
        .and_then(Value::as_f64)
        .ok_or_else(|| "Expected vec2 y component".to_string())? as f32;
    Ok([x, y])
}

fn interpolate_scalar_keyframes(keyframes: &[ScalarKeyframe], frame: f32) -> f32 {
    if keyframes.len() == 1 {
        return keyframes[0].s;
    }
    for window in keyframes.windows(2) {
        let current = &window[0];
        let next = &window[1];
        if frame <= current.t {
            return current.s;
        }
        if frame < next.t {
            let span = (next.t - current.t).max(f32::EPSILON);
            let progress = ((frame - current.t) / span).clamp(0.0, 1.0);
            return current.s + (next.s - current.s) * progress;
        }
    }
    keyframes.last().map(|frame| frame.s).unwrap_or_default()
}

fn interpolate_vec2_keyframes(keyframes: &[Vec2Keyframe], frame: f32) -> [f32; 2] {
    if keyframes.len() == 1 {
        return keyframes[0].s;
    }
    for window in keyframes.windows(2) {
        let current = &window[0];
        let next = &window[1];
        if frame <= current.t {
            return current.s;
        }
        if frame < next.t {
            let span = (next.t - current.t).max(f32::EPSILON);
            let progress = ((frame - current.t) / span).clamp(0.0, 1.0);
            return [
                current.s[0] + (next.s[0] - current.s[0]) * progress,
                current.s[1] + (next.s[1] - current.s[1]) * progress,
            ];
        }
    }
    keyframes.last().map(|frame| frame.s).unwrap_or([0.0, 0.0])
}

fn aspect_fit_bounds(bounds: Bounds, source_width: f32, source_height: f32) -> Bounds {
    if source_width <= 0.0
        || source_height <= 0.0
        || bounds.size.width <= 0.0
        || bounds.size.height <= 0.0
    {
        return bounds;
    }
    let scale = (bounds.size.width / source_width)
        .min(bounds.size.height / source_height)
        .max(0.0);
    let width = (source_width * scale).max(1.0);
    let height = (source_height * scale).max(1.0);
    Bounds::new(
        bounds.origin.x + (bounds.size.width - width) * 0.5,
        bounds.origin.y + (bounds.size.height - height) * 0.5,
        width,
        height,
    )
}

fn derive_setup_progress(state: &RenderState) -> SetupProgressView {
    setup_progress_from_runtime(&setup_runtime_snapshot(state))
}

fn setup_runtime_snapshot(state: &RenderState) -> SetupRuntimeSnapshot {
    let wallet_connected = matches!(
        state
            .spark_wallet
            .network_status
            .as_ref()
            .map(|status| status.status),
        Some(NetworkStatus::Connected)
    );
    let configured_relay_count = state.configured_provider_relay_urls().len();
    let connected_relay_count = state
        .relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == RelayConnectionStatus::Connected)
        .count();
    let connecting_relay_count = state
        .relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == RelayConnectionStatus::Connecting)
        .count();
    let errored_relay_count = state
        .relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == RelayConnectionStatus::Error)
        .count();
    let control_runtime_ready = state.desktop_control.enabled
        && state
            .desktop_control
            .listen_addr
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && state
            .desktop_control
            .base_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && state.desktop_control.last_error.is_none();
    let sync_required = crate::sync_bootstrap::spacetime_sync_enabled_from_env();
    let sync_ready = !sync_required
        || (state.sync_bootstrap_error.is_none()
            && state.sync_health.load_state == PaneLoadState::Ready
            && state.sync_health.recovery_phase == SyncRecoveryPhase::Ready);

    SetupRuntimeSnapshot {
        wallet_connected,
        wallet_balance_known: state.spark_wallet.balance.is_some(),
        wallet_error: state.spark_wallet.last_error.clone(),
        identity_ready: state.nostr_identity.is_some(),
        identity_error: state.nostr_identity_error.clone(),
        configured_relay_count,
        connected_relay_count,
        connecting_relay_count,
        errored_relay_count,
        control_runtime_ready,
        control_runtime_error: state.desktop_control.last_error.clone(),
        sync_required,
        sync_ready,
        sync_error: if sync_required {
            state
                .sync_bootstrap_error
                .clone()
                .or_else(|| state.sync_health.last_error.clone())
        } else {
            None
        },
    }
}

fn setup_progress_from_runtime(snapshot: &SetupRuntimeSnapshot) -> SetupProgressView {
    let completed = [
        setup_wallet_complete(snapshot),
        setup_network_configuration_complete(snapshot),
        setup_connection_complete(snapshot),
    ];
    let active_step = SetupStepId::ALL
        .into_iter()
        .zip(completed)
        .find_map(|(step, is_complete)| (!is_complete).then_some(step));
    let cta_enabled = active_step.is_none();

    SetupProgressView {
        statuses: setup_row_statuses(completed),
        active_step,
        cta_enabled,
        detail_lines: setup_detail_lines(snapshot, active_step),
        loading_dot_count: ((current_timestamp_ms() / SETUP_LOADING_DOT_INTERVAL_MS) % 3 + 1)
            as usize,
    }
}

fn setup_wallet_complete(snapshot: &SetupRuntimeSnapshot) -> bool {
    snapshot.wallet_error.is_none() && snapshot.wallet_connected && snapshot.wallet_balance_known
}

fn setup_network_configuration_complete(snapshot: &SetupRuntimeSnapshot) -> bool {
    snapshot.identity_ready && snapshot.configured_relay_count > 0
}

fn setup_connection_complete(snapshot: &SetupRuntimeSnapshot) -> bool {
    snapshot.control_runtime_ready
        && snapshot.connected_relay_count > 0
        && (!snapshot.sync_required || snapshot.sync_ready)
}

fn setup_detail_lines(
    snapshot: &SetupRuntimeSnapshot,
    active_step: Option<SetupStepId>,
) -> Vec<String> {
    let mut lines = Vec::new();
    match active_step {
        Some(SetupStepId::LightningWallet) => {
            if let Some(error) = snapshot.wallet_error.as_deref() {
                push_setup_detail(&mut lines, format!("Spark wallet error: {error}"));
            } else {
                if !snapshot.wallet_connected {
                    push_setup_detail(
                        &mut lines,
                        "Waiting for Spark wallet transport to connect.".to_string(),
                    );
                }
                if !snapshot.wallet_balance_known {
                    push_setup_detail(
                        &mut lines,
                        "Hydrating wallet balance and payment history.".to_string(),
                    );
                }
            }
        }
        Some(SetupStepId::NetworkConfiguration) => {
            if let Some(error) = snapshot.identity_error.as_deref() {
                push_setup_detail(&mut lines, format!("Nostr identity error: {error}"));
            } else if !snapshot.identity_ready {
                push_setup_detail(
                    &mut lines,
                    "Waiting for local Nostr identity to load.".to_string(),
                );
            }
            if snapshot.configured_relay_count == 0 {
                push_setup_detail(
                    &mut lines,
                    "No relay transport is configured yet.".to_string(),
                );
            }
        }
        Some(SetupStepId::EstablishingConnection) => {
            if let Some(error) = snapshot.control_runtime_error.as_deref() {
                push_setup_detail(&mut lines, format!("Desktop control error: {error}"));
            } else if !snapshot.control_runtime_ready {
                push_setup_detail(
                    &mut lines,
                    "Starting local desktop control runtime.".to_string(),
                );
            }
            if snapshot.connected_relay_count == 0 {
                let relay_line = if snapshot.connecting_relay_count > 0 {
                    "Connecting to configured relay transport."
                } else if snapshot.errored_relay_count > 0 {
                    "Relay transport reported startup errors."
                } else {
                    "Waiting for the first relay connection."
                };
                push_setup_detail(&mut lines, relay_line.to_string());
            }
            if snapshot.sync_required {
                if let Some(error) = snapshot.sync_error.as_deref() {
                    push_setup_detail(&mut lines, format!("Sync session error: {error}"));
                } else if !snapshot.sync_ready {
                    push_setup_detail(
                        &mut lines,
                        "Waiting for authenticated sync session.".to_string(),
                    );
                }
            }
        }
        None => {
            push_setup_detail(
                &mut lines,
                "Wallet, identity, and network transport are ready.".to_string(),
            );
            push_setup_detail(
                &mut lines,
                "Click Start Earning Bitcoin to continue into Mission Control.".to_string(),
            );
        }
    }
    lines
}

fn push_setup_detail(lines: &mut Vec<String>, line: String) {
    let normalized = line.trim();
    if normalized.is_empty() || lines.iter().any(|existing| existing == normalized) {
        return;
    }
    lines.push(normalized.to_string());
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
        hotkeys_target_bounds, setup_progress_from_runtime, setup_row_statuses,
        tour_sell_compute_layout, OnboardingLottiePlayer, OnboardingPhase, OnboardingState,
        SetupRowStatus, SetupRuntimeSnapshot,
    };
    use wgpui::{Bounds, ImageSource};

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
    fn packaged_onboarding_lottie_renders_visible_pixels() {
        let mut player = OnboardingLottiePlayer::from_packaged_json()
            .expect("packaged onboarding lottie should load");
        let image = player
            .render_image(Bounds::new(0.0, 0.0, 320.0, 320.0))
            .expect("packaged onboarding lottie should render an image");
        let ImageSource::Rgba8(image_data) = image.source else {
            panic!("expected rgba8 onboarding image");
        };
        let pixel_bytes = image_data.rgba8.as_ref();
        let visible_pixels = pixel_bytes
            .chunks_exact(4)
            .filter(|px| px[3] > 0 && (px[0] > 0 || px[1] > 0 || px[2] > 0))
            .count();
        assert!(
            visible_pixels > 0,
            "packaged onboarding lottie rendered only transparent/black pixels"
        );
    }

    #[test]
    fn setup_progress_unlocks_cta_only_after_real_init_readiness() {
        let progress = setup_progress_from_runtime(&SetupRuntimeSnapshot {
            wallet_connected: true,
            wallet_balance_known: true,
            identity_ready: true,
            configured_relay_count: 2,
            connected_relay_count: 1,
            control_runtime_ready: true,
            sync_required: false,
            sync_ready: true,
            ..SetupRuntimeSnapshot::default()
        });
        assert_eq!(progress.active_step, None);
        assert!(progress.cta_enabled);
        assert_eq!(
            progress.statuses,
            [
                SetupRowStatus::Complete,
                SetupRowStatus::Complete,
                SetupRowStatus::Complete
            ]
        );
    }

    #[test]
    fn setup_progress_reports_sync_errors_when_sync_is_required() {
        let progress = setup_progress_from_runtime(&SetupRuntimeSnapshot {
            wallet_connected: true,
            wallet_balance_known: true,
            identity_ready: true,
            configured_relay_count: 2,
            connected_relay_count: 1,
            control_runtime_ready: true,
            sync_required: true,
            sync_error: Some("missing control base url".to_string()),
            ..SetupRuntimeSnapshot::default()
        });

        assert_eq!(
            progress.active_step,
            Some(super::SetupStepId::EstablishingConnection)
        );
        assert!(!progress.cta_enabled);
        assert!(progress
            .detail_lines
            .iter()
            .any(|line| line.contains("Sync session error")));
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
