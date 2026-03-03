use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_RENDER_MODE_ISSUE_ID: &str = "VCAD-PARITY-094";
pub const RENDER_MODE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/render_mode_vcad_reference.json";
const RENDER_MODE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/render_mode_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderModeParityManifest {
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
    pub wireframe_toggle_match: bool,
    pub hidden_line_toggle_match: bool,
    pub variant_profiles_match: bool,
    pub cycle_sequence_match: bool,
    pub alias_resolution_match: bool,
    pub deterministic_replay_match: bool,
    pub default_state: RenderModeStateSnapshot,
    pub toggle_snapshots: Vec<RenderModeToggleSnapshot>,
    pub variant_profiles: Vec<RenderVariantProfileSnapshot>,
    pub cycle_sequence: Vec<String>,
    pub alias_resolutions: Vec<RenderModeAliasResolutionSnapshot>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RenderModeReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_defaults: RenderModeDefaultsExpectation,
    expected_wireframe_toggle: [bool; 2],
    expected_hidden_line_toggle: [bool; 2],
    expected_variant_profiles: Vec<RenderVariantProfileSnapshot>,
    expected_cycle_sequence: Vec<String>,
    expected_aliases: Vec<RenderModeAliasExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RenderModeDefaultsExpectation {
    render_mode: String,
    view_mode: String,
    show_wireframe: bool,
    show_hidden_lines: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RenderModeAliasExpectation {
    token: String,
    resolved_variant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RenderModeSnapshot {
    default_state: RenderModeStateSnapshot,
    toggle_snapshots: Vec<RenderModeToggleSnapshot>,
    variant_profiles: Vec<RenderVariantProfileSnapshot>,
    cycle_sequence: Vec<String>,
    alias_resolutions: Vec<RenderModeAliasResolutionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderModeStateSnapshot {
    pub render_mode: String,
    pub view_mode: String,
    pub show_wireframe: bool,
    pub show_hidden_lines: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderModeToggleSnapshot {
    pub step_id: String,
    pub show_wireframe: bool,
    pub show_hidden_lines: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderVariantProfileSnapshot {
    pub variant_id: String,
    pub render_mode: String,
    pub view_mode: String,
    pub show_wireframe: bool,
    pub show_hidden_lines: bool,
    pub face_fill_enabled: bool,
    pub edge_overlay_enabled: bool,
    pub occluded_edges_dashed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderModeAliasResolutionSnapshot {
    pub token: String,
    pub recognized: bool,
    pub resolved_variant_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderPipelineMode {
    Standard,
    Raytrace,
}

impl RenderPipelineMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Raytrace => "raytrace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderViewMode {
    ThreeD,
    TwoD,
}

impl RenderViewMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ThreeD => "3d",
            Self::TwoD => "2d",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderVariant {
    Standard,
    Wire,
    HiddenLine,
}

impl RenderVariant {
    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Wire => "wire",
            Self::HiddenLine => "hidden-line",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Standard => Self::Wire,
            Self::Wire => Self::HiddenLine,
            Self::HiddenLine => Self::Standard,
        }
    }

    fn parse(token: &str) -> Option<Self> {
        let normalized = token
            .trim()
            .to_ascii_lowercase()
            .replace('_', "-")
            .replace(' ', "-");
        match normalized.as_str() {
            "standard" | "shaded" => Some(Self::Standard),
            "wire" | "wireframe" | "shaded-edges" => Some(Self::Wire),
            "hidden-line" | "hiddenline" => Some(Self::HiddenLine),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct RenderModeState {
    render_mode: RenderPipelineMode,
    view_mode: RenderViewMode,
    show_wireframe: bool,
    show_hidden_lines: bool,
}

impl Default for RenderModeState {
    fn default() -> Self {
        // Mirrors vcad store defaults:
        // - ui-store: renderMode="standard", showWireframe=false
        // - drawing-store: viewMode="3d", showHiddenLines=true
        Self {
            render_mode: RenderPipelineMode::Standard,
            view_mode: RenderViewMode::ThreeD,
            show_wireframe: false,
            show_hidden_lines: true,
        }
    }
}

impl RenderModeState {
    fn toggle_wireframe(&mut self) {
        self.show_wireframe = !self.show_wireframe;
    }

    fn toggle_hidden_lines(&mut self) {
        self.show_hidden_lines = !self.show_hidden_lines;
    }

    fn apply_variant(&mut self, variant: RenderVariant) {
        self.render_mode = RenderPipelineMode::Standard;
        match variant {
            RenderVariant::Standard => {
                self.view_mode = RenderViewMode::ThreeD;
                self.show_wireframe = false;
                self.show_hidden_lines = false;
            }
            RenderVariant::Wire => {
                self.view_mode = RenderViewMode::ThreeD;
                self.show_wireframe = true;
                self.show_hidden_lines = false;
            }
            RenderVariant::HiddenLine => {
                self.view_mode = RenderViewMode::TwoD;
                self.show_wireframe = false;
                self.show_hidden_lines = true;
            }
        }
    }

    fn snapshot(&self) -> RenderModeStateSnapshot {
        RenderModeStateSnapshot {
            render_mode: self.render_mode.as_str().to_string(),
            view_mode: self.view_mode.as_str().to_string(),
            show_wireframe: self.show_wireframe,
            show_hidden_lines: self.show_hidden_lines,
        }
    }
}

pub fn build_render_mode_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<RenderModeParityManifest> {
    let reference: RenderModeReferenceFixture =
        serde_json::from_str(RENDER_MODE_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing render mode reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 = sha256_hex(RENDER_MODE_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let default_state_match =
        default_state_matches(&snapshot.default_state, &reference.expected_defaults);
    let wireframe_toggle_match = wireframe_toggle_matches(
        &snapshot.toggle_snapshots,
        reference.expected_wireframe_toggle,
    );
    let hidden_line_toggle_match = hidden_line_toggle_matches(
        &snapshot.toggle_snapshots,
        reference.expected_hidden_line_toggle,
    );
    let variant_profiles_match = sorted_variant_profiles(snapshot.variant_profiles.clone())
        == sorted_variant_profiles(reference.expected_variant_profiles.clone());
    let cycle_sequence_match = snapshot.cycle_sequence == reference.expected_cycle_sequence;
    let alias_resolution_match =
        alias_resolution_matches(&snapshot.alias_resolutions, &reference.expected_aliases);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        default_state_match,
        wireframe_toggle_match,
        hidden_line_toggle_match,
        variant_profiles_match,
        cycle_sequence_match,
        alias_resolution_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(RenderModeParityManifest {
        manifest_version: 1,
        issue_id: PARITY_RENDER_MODE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: RENDER_MODE_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        default_state_match,
        wireframe_toggle_match,
        hidden_line_toggle_match,
        variant_profiles_match,
        cycle_sequence_match,
        alias_resolution_match,
        deterministic_replay_match,
        default_state: snapshot.default_state,
        toggle_snapshots: snapshot.toggle_snapshots,
        variant_profiles: snapshot.variant_profiles,
        cycle_sequence: snapshot.cycle_sequence,
        alias_resolutions: snapshot.alias_resolutions,
        deterministic_signature,
        parity_contracts: vec![
            "default render state mirrors vcad standard mode defaults (standard pipeline, wireframe off)"
                .to_string(),
            "wireframe toggle semantics match vcad ui-store behavior (false->true->false)"
                .to_string(),
            "hidden-line toggle semantics match vcad drawing-store behavior (true->false->true)"
                .to_string(),
            "render variants standard/wire/hidden-line map to deterministic style profiles"
                .to_string(),
            "variant alias parsing and mode cycling are deterministic across replay".to_string(),
        ],
    })
}

fn collect_snapshot() -> RenderModeSnapshot {
    let default_state = RenderModeState::default().snapshot();

    let mut state = RenderModeState::default();
    state.toggle_wireframe();
    let wireframe_on = RenderModeToggleSnapshot {
        step_id: "wireframe_toggle_on".to_string(),
        show_wireframe: state.show_wireframe,
        show_hidden_lines: state.show_hidden_lines,
    };
    state.toggle_wireframe();
    let wireframe_off = RenderModeToggleSnapshot {
        step_id: "wireframe_toggle_off".to_string(),
        show_wireframe: state.show_wireframe,
        show_hidden_lines: state.show_hidden_lines,
    };
    state.toggle_hidden_lines();
    let hidden_lines_off = RenderModeToggleSnapshot {
        step_id: "hidden_line_toggle_off".to_string(),
        show_wireframe: state.show_wireframe,
        show_hidden_lines: state.show_hidden_lines,
    };
    state.toggle_hidden_lines();
    let hidden_lines_on = RenderModeToggleSnapshot {
        step_id: "hidden_line_toggle_on".to_string(),
        show_wireframe: state.show_wireframe,
        show_hidden_lines: state.show_hidden_lines,
    };

    let variant_profiles = vec![
        variant_profile(RenderVariant::Standard),
        variant_profile(RenderVariant::Wire),
        variant_profile(RenderVariant::HiddenLine),
    ];

    let mut cycle_variant = RenderVariant::Standard;
    let mut cycle_sequence = vec![cycle_variant.as_str().to_string()];
    for _ in 0..3 {
        cycle_variant = cycle_variant.next();
        cycle_sequence.push(cycle_variant.as_str().to_string());
    }

    let alias_tokens = [
        "standard",
        "wire",
        "wireframe",
        "hidden-line",
        "hidden_line",
        "unknown",
    ];
    let alias_resolutions = alias_tokens
        .iter()
        .map(|token| {
            let variant = RenderVariant::parse(token).unwrap_or(RenderVariant::Standard);
            RenderModeAliasResolutionSnapshot {
                token: (*token).to_string(),
                recognized: RenderVariant::parse(token).is_some(),
                resolved_variant_id: variant.as_str().to_string(),
            }
        })
        .collect::<Vec<_>>();

    RenderModeSnapshot {
        default_state,
        toggle_snapshots: vec![
            wireframe_on,
            wireframe_off,
            hidden_lines_off,
            hidden_lines_on,
        ],
        variant_profiles: sorted_variant_profiles(variant_profiles),
        cycle_sequence,
        alias_resolutions: sorted_alias_resolutions(alias_resolutions),
    }
}

fn variant_profile(variant: RenderVariant) -> RenderVariantProfileSnapshot {
    let mut state = RenderModeState {
        // Render mode parity intentionally excludes raytrace mode in VCAD-PARITY-094.
        render_mode: RenderPipelineMode::Raytrace,
        ..RenderModeState::default()
    };
    state.apply_variant(variant);
    RenderVariantProfileSnapshot {
        variant_id: variant.as_str().to_string(),
        render_mode: state.render_mode.as_str().to_string(),
        view_mode: state.view_mode.as_str().to_string(),
        show_wireframe: state.show_wireframe,
        show_hidden_lines: state.show_hidden_lines,
        face_fill_enabled: variant == RenderVariant::Standard,
        edge_overlay_enabled: variant != RenderVariant::Standard,
        occluded_edges_dashed: variant == RenderVariant::HiddenLine,
    }
}

fn default_state_matches(
    state: &RenderModeStateSnapshot,
    expected: &RenderModeDefaultsExpectation,
) -> bool {
    state.render_mode == expected.render_mode
        && state.view_mode == expected.view_mode
        && state.show_wireframe == expected.show_wireframe
        && state.show_hidden_lines == expected.show_hidden_lines
}

fn wireframe_toggle_matches(
    toggles: &[RenderModeToggleSnapshot],
    expected_wireframe_toggle: [bool; 2],
) -> bool {
    toggles
        .iter()
        .find(|snapshot| snapshot.step_id == "wireframe_toggle_on")
        .is_some_and(|snapshot| snapshot.show_wireframe == expected_wireframe_toggle[0])
        && toggles
            .iter()
            .find(|snapshot| snapshot.step_id == "wireframe_toggle_off")
            .is_some_and(|snapshot| snapshot.show_wireframe == expected_wireframe_toggle[1])
}

fn hidden_line_toggle_matches(
    toggles: &[RenderModeToggleSnapshot],
    expected_hidden_line_toggle: [bool; 2],
) -> bool {
    toggles
        .iter()
        .find(|snapshot| snapshot.step_id == "hidden_line_toggle_off")
        .is_some_and(|snapshot| snapshot.show_hidden_lines == expected_hidden_line_toggle[0])
        && toggles
            .iter()
            .find(|snapshot| snapshot.step_id == "hidden_line_toggle_on")
            .is_some_and(|snapshot| snapshot.show_hidden_lines == expected_hidden_line_toggle[1])
}

fn alias_resolution_matches(
    actual: &[RenderModeAliasResolutionSnapshot],
    expected_aliases: &[RenderModeAliasExpectation],
) -> bool {
    let mut expected = expected_aliases
        .iter()
        .map(|alias| RenderModeAliasResolutionSnapshot {
            token: alias.token.clone(),
            recognized: true,
            resolved_variant_id: alias.resolved_variant_id.clone(),
        })
        .collect::<Vec<_>>();
    expected.push(RenderModeAliasResolutionSnapshot {
        token: "unknown".to_string(),
        recognized: false,
        resolved_variant_id: RenderVariant::Standard.as_str().to_string(),
    });

    sorted_alias_resolutions(actual.to_vec()) == sorted_alias_resolutions(expected)
}

fn sorted_variant_profiles(
    mut profiles: Vec<RenderVariantProfileSnapshot>,
) -> Vec<RenderVariantProfileSnapshot> {
    profiles.sort_by(|left, right| left.variant_id.cmp(&right.variant_id));
    profiles
}

fn sorted_alias_resolutions(
    mut aliases: Vec<RenderModeAliasResolutionSnapshot>,
) -> Vec<RenderModeAliasResolutionSnapshot> {
    aliases.sort_by(|left, right| left.token.cmp(&right.token));
    aliases
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &RenderModeSnapshot,
    reference_commit_match: bool,
    default_state_match: bool,
    wireframe_toggle_match: bool,
    hidden_line_toggle_match: bool,
    variant_profiles_match: bool,
    cycle_sequence_match: bool,
    alias_resolution_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        default_state_match,
        wireframe_toggle_match,
        hidden_line_toggle_match,
        variant_profiles_match,
        cycle_sequence_match,
        alias_resolution_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize render mode parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PARITY_RENDER_MODE_ISSUE_ID, build_render_mode_parity_manifest, collect_snapshot};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "1b59e7948efcdb848d8dba6848785d57aa310e81".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 1,
                crates_reference_count: 1,
                commands_reference_count: 1,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn snapshot_is_deterministic() {
        let a = collect_snapshot();
        let b = collect_snapshot();
        assert_eq!(a, b);
    }

    #[test]
    fn render_mode_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_render_mode_parity_manifest(&scorecard, "scorecard")
            .expect("build render mode parity manifest");
        assert_eq!(manifest.issue_id, PARITY_RENDER_MODE_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.default_state_match);
        assert!(manifest.wireframe_toggle_match);
        assert!(manifest.hidden_line_toggle_match);
        assert!(manifest.variant_profiles_match);
        assert!(manifest.cycle_sequence_match);
        assert!(manifest.alias_resolution_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.variant_profiles.len(), 3);
    }
}
