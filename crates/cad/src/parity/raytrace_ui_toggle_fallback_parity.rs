use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_RAYTRACE_UI_TOGGLE_FALLBACK_ISSUE_ID: &str = "VCAD-PARITY-103";
pub const RAYTRACE_UI_TOGGLE_FALLBACK_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/raytrace_ui_toggle_fallback_vcad_reference.json";
const RAYTRACE_UI_TOGGLE_FALLBACK_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/raytrace_ui_toggle_fallback_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceUiToggleFallbackParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub default_state_match: bool,
    pub init_outcome_match: bool,
    pub keyboard_guard_match: bool,
    pub quality_selection_toggle_match: bool,
    pub overlay_gate_match: bool,
    pub fallback_behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub default_state: RaytraceUiStateSnapshot,
    pub init_outcomes: Vec<RaytraceInitOutcomeSnapshot>,
    pub toggle_samples: Vec<RaytraceToggleSample>,
    pub overlay_samples: Vec<RaytraceOverlaySample>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceUiToggleFallbackReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_default_state: RaytraceUiStateSnapshot,
    expected_init_outcomes: Vec<RaytraceInitOutcomeSnapshot>,
    expected_toggle_samples: Vec<RaytraceToggleSample>,
    expected_overlay_samples: Vec<RaytraceOverlaySample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceUiToggleFallbackSnapshot {
    default_state: RaytraceUiStateSnapshot,
    init_outcomes: Vec<RaytraceInitOutcomeSnapshot>,
    toggle_samples: Vec<RaytraceToggleSample>,
    overlay_samples: Vec<RaytraceOverlaySample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceUiStateSnapshot {
    pub render_mode: String,
    pub raytrace_quality: String,
    pub raytrace_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceInitOutcomeSnapshot {
    pub case_id: String,
    pub gpu_available: bool,
    pub raytracer_init_success: bool,
    pub raytrace_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceToggleSample {
    pub case_id: String,
    pub initial_render_mode: String,
    pub raytrace_available: bool,
    pub action: String,
    pub selected_quality: Option<String>,
    pub final_render_mode: String,
    pub final_quality: String,
    pub mode_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceOverlaySample {
    pub case_id: String,
    pub electronics_active: bool,
    pub render_mode: String,
    pub raytrace_available: bool,
    pub raytrace_menu_visible: bool,
    pub viewport_overlay_active: bool,
    pub viewport_sync_active: bool,
    pub fallback_to_standard_raster: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Standard,
    Raytrace,
}

impl RenderMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Raytrace => "raytrace",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Standard => Self::Raytrace,
            Self::Raytrace => Self::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaytraceQuality {
    Draft,
    Standard,
    High,
}

impl RaytraceQuality {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Standard => "standard",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone)]
struct UiState {
    render_mode: RenderMode,
    raytrace_quality: RaytraceQuality,
    raytrace_available: bool,
}

impl Default for UiState {
    fn default() -> Self {
        // Mirrors vcad ui-store defaults.
        Self {
            render_mode: RenderMode::Standard,
            raytrace_quality: RaytraceQuality::Draft,
            raytrace_available: false,
        }
    }
}

impl UiState {
    fn toggle_render_mode(&mut self) {
        self.render_mode = self.render_mode.toggle();
    }

    fn select_quality_from_menu(&mut self, quality: RaytraceQuality) {
        // Mirrors CornerIcons quality click handler.
        if self.render_mode != RenderMode::Raytrace {
            self.toggle_render_mode();
        }
        self.raytrace_quality = quality;
    }

    fn keyboard_alt_r_toggle(&mut self) {
        // Mirrors keyboard shortcut guard.
        if self.raytrace_available {
            self.toggle_render_mode();
        }
    }

    fn snapshot(&self) -> RaytraceUiStateSnapshot {
        RaytraceUiStateSnapshot {
            render_mode: self.render_mode.as_str().to_string(),
            raytrace_quality: self.raytrace_quality.as_str().to_string(),
            raytrace_available: self.raytrace_available,
        }
    }
}

pub fn build_raytrace_ui_toggle_fallback_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<RaytraceUiToggleFallbackParityManifest> {
    let reference: RaytraceUiToggleFallbackReferenceFixture = serde_json::from_str(
        RAYTRACE_UI_TOGGLE_FALLBACK_REFERENCE_FIXTURE_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed parsing raytrace ui-toggle/fallback reference fixture: {error}"),
    })?;

    let reference_fixture_sha256 =
        sha256_hex(RAYTRACE_UI_TOGGLE_FALLBACK_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let default_state_match = snapshot.default_state == reference.expected_default_state;
    let init_outcome_match = sorted_init(snapshot.init_outcomes.clone())
        == sorted_init(reference.expected_init_outcomes.clone());
    let keyboard_guard_match = keyboard_guard_holds(&snapshot.toggle_samples);
    let quality_selection_toggle_match = quality_click_turns_on_raytrace(&snapshot.toggle_samples);
    let overlay_gate_match = overlay_samples_match(
        snapshot.overlay_samples.clone(),
        reference.expected_overlay_samples.clone(),
    );
    let fallback_behavior_match = snapshot.overlay_samples.iter().all(|sample| {
        sample.fallback_to_standard_raster
            == !(sample.viewport_overlay_active && sample.viewport_sync_active)
    });

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        default_state_match,
        init_outcome_match,
        keyboard_guard_match,
        quality_selection_toggle_match,
        overlay_gate_match,
        fallback_behavior_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(RaytraceUiToggleFallbackParityManifest {
        manifest_version: 1,
        issue_id: PARITY_RAYTRACE_UI_TOGGLE_FALLBACK_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: RAYTRACE_UI_TOGGLE_FALLBACK_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        default_state_match,
        init_outcome_match,
        keyboard_guard_match,
        quality_selection_toggle_match,
        overlay_gate_match,
        fallback_behavior_match,
        deterministic_replay_match,
        default_state: snapshot.default_state,
        init_outcomes: snapshot.init_outcomes,
        toggle_samples: snapshot.toggle_samples,
        overlay_samples: snapshot.overlay_samples,
        deterministic_signature,
        parity_contracts: vec![
            "raytrace ui state defaults to renderMode=standard, raytraceQuality=draft, raytraceAvailable=false"
                .to_string(),
            "raytrace availability is only enabled when GPU init succeeds and raytracer init succeeds"
                .to_string(),
            "Alt+R keyboard toggle is guarded by raytraceAvailable and no-ops when unavailable"
                .to_string(),
            "quality selection from the raytrace menu auto-toggles renderMode to raytrace when currently off"
                .to_string(),
            "viewport raytrace overlay/sync are gated by renderMode==raytrace AND raytraceAvailable AND !electronicsActive"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> RaytraceUiToggleFallbackSnapshot {
    let default_state = UiState::default().snapshot();

    let mut init_outcomes = vec![
        init_outcome("gpu_unavailable", false, false),
        init_outcome("gpu_ok_raytracer_fail", true, false),
        init_outcome("gpu_ok_raytracer_ok", true, true),
    ];
    init_outcomes.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let mut toggle_samples = vec![
        toggle_sample(
            "quality_click_from_off",
            RenderMode::Standard,
            true,
            "quality_click",
            Some(RaytraceQuality::High),
        ),
        toggle_sample(
            "quality_click_standard_from_off",
            RenderMode::Standard,
            true,
            "quality_click",
            Some(RaytraceQuality::Standard),
        ),
        toggle_sample(
            "off_click_from_raytrace",
            RenderMode::Raytrace,
            true,
            "menu_off_click",
            None,
        ),
        toggle_sample(
            "keyboard_alt_r_available",
            RenderMode::Standard,
            true,
            "keyboard_alt_r",
            None,
        ),
        toggle_sample(
            "keyboard_alt_r_unavailable",
            RenderMode::Standard,
            false,
            "keyboard_alt_r",
            None,
        ),
    ];
    toggle_samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let mut overlay_samples = vec![
        overlay_sample("standard_mode_available", false, RenderMode::Standard, true),
        overlay_sample(
            "raytrace_mode_unavailable",
            false,
            RenderMode::Raytrace,
            false,
        ),
        overlay_sample("raytrace_mode_available", false, RenderMode::Raytrace, true),
        overlay_sample(
            "raytrace_mode_available_electronics",
            true,
            RenderMode::Raytrace,
            true,
        ),
    ];
    overlay_samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    RaytraceUiToggleFallbackSnapshot {
        default_state,
        init_outcomes,
        toggle_samples,
        overlay_samples,
    }
}

fn init_outcome(
    case_id: &str,
    gpu_available: bool,
    raytracer_init_success: bool,
) -> RaytraceInitOutcomeSnapshot {
    let raytrace_available = gpu_available && raytracer_init_success;
    RaytraceInitOutcomeSnapshot {
        case_id: case_id.to_string(),
        gpu_available,
        raytracer_init_success,
        raytrace_available,
    }
}

fn toggle_sample(
    case_id: &str,
    initial_render_mode: RenderMode,
    raytrace_available: bool,
    action: &str,
    selected_quality: Option<RaytraceQuality>,
) -> RaytraceToggleSample {
    let mut state = UiState {
        render_mode: initial_render_mode,
        raytrace_quality: RaytraceQuality::Draft,
        raytrace_available,
    };

    match action {
        "menu_off_click" => state.toggle_render_mode(),
        "keyboard_alt_r" => state.keyboard_alt_r_toggle(),
        "quality_click" => {
            let quality = selected_quality.expect("quality_click requires selected quality");
            state.select_quality_from_menu(quality);
        }
        _ => panic!("unsupported action: {action}"),
    }

    RaytraceToggleSample {
        case_id: case_id.to_string(),
        initial_render_mode: initial_render_mode.as_str().to_string(),
        raytrace_available,
        action: action.to_string(),
        selected_quality: selected_quality.map(|quality| quality.as_str().to_string()),
        final_render_mode: state.render_mode.as_str().to_string(),
        final_quality: state.raytrace_quality.as_str().to_string(),
        mode_changed: state.render_mode != initial_render_mode,
    }
}

fn overlay_sample(
    case_id: &str,
    electronics_active: bool,
    render_mode: RenderMode,
    raytrace_available: bool,
) -> RaytraceOverlaySample {
    let raytrace_menu_visible = raytrace_available;
    let overlay_gate =
        !electronics_active && render_mode == RenderMode::Raytrace && raytrace_available;

    RaytraceOverlaySample {
        case_id: case_id.to_string(),
        electronics_active,
        render_mode: render_mode.as_str().to_string(),
        raytrace_available,
        raytrace_menu_visible,
        viewport_overlay_active: overlay_gate,
        viewport_sync_active: overlay_gate,
        fallback_to_standard_raster: !overlay_gate,
    }
}

fn keyboard_guard_holds(samples: &[RaytraceToggleSample]) -> bool {
    samples
        .iter()
        .filter(|sample| sample.action == "keyboard_alt_r" && !sample.raytrace_available)
        .all(|sample| {
            !sample.mode_changed && sample.final_render_mode == sample.initial_render_mode
        })
}

fn quality_click_turns_on_raytrace(samples: &[RaytraceToggleSample]) -> bool {
    samples
        .iter()
        .filter(|sample| sample.action == "quality_click")
        .all(|sample| sample.final_render_mode == "raytrace")
}

fn sorted_init(mut values: Vec<RaytraceInitOutcomeSnapshot>) -> Vec<RaytraceInitOutcomeSnapshot> {
    values.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    values
}

fn overlay_samples_match(
    mut actual: Vec<RaytraceOverlaySample>,
    mut expected: Vec<RaytraceOverlaySample>,
) -> bool {
    actual.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    actual == expected
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &RaytraceUiToggleFallbackSnapshot,
    reference_commit_match: bool,
    default_state_match: bool,
    init_outcome_match: bool,
    keyboard_guard_match: bool,
    quality_selection_toggle_match: bool,
    overlay_gate_match: bool,
    fallback_behavior_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        default_state_match,
        init_outcome_match,
        keyboard_guard_match,
        quality_selection_toggle_match,
        overlay_gate_match,
        fallback_behavior_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize raytrace ui-toggle/fallback parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{RenderMode, UiState};

    #[test]
    fn keyboard_toggle_is_guarded_by_availability() {
        let mut unavailable = UiState::default();
        unavailable.keyboard_alt_r_toggle();
        assert_eq!(unavailable.render_mode, RenderMode::Standard);

        let mut available = UiState::default();
        available.raytrace_available = true;
        available.keyboard_alt_r_toggle();
        assert_eq!(available.render_mode, RenderMode::Raytrace);
    }
}
