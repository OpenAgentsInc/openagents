use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_DRAWING_MODE_UI_ISSUE_ID: &str = "VCAD-PARITY-074";
pub const DRAFTING_DRAWING_MODE_UI_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_drawing_mode_ui_vcad_reference.json";
const DRAFTING_DRAWING_MODE_UI_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_drawing_mode_ui_vcad_reference.json");

const MIN_ZOOM: f64 = 0.1;
const MAX_ZOOM: f64 = 10.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingDrawingModeUiParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub ui_case_snapshots: Vec<DrawingModeUiCaseSnapshot>,
    pub ui_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingDrawingModeUiReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    scalar_tolerance: f64,
    expected_ui_case_snapshots: Vec<DrawingModeUiCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DrawingModeUiSnapshot {
    ui_case_snapshots: Vec<DrawingModeUiCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrawingModeUiCaseSnapshot {
    pub case_id: String,
    pub view_mode: String,
    pub view_direction: String,
    pub show_hidden_lines: bool,
    pub show_dimensions: bool,
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub detail_count: usize,
    pub detail_ids: Vec<String>,
    pub detail_labels: Vec<String>,
    pub detail_scales: Vec<f64>,
    pub detail_centers_x: Vec<f64>,
    pub detail_centers_y: Vec<f64>,
    pub detail_widths: Vec<f64>,
    pub detail_heights: Vec<f64>,
    pub next_detail_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawingViewMode {
    ThreeD,
    TwoD,
}

impl DrawingViewMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ThreeD => "3d",
            Self::TwoD => "2d",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawingViewDirection {
    Front,
    Back,
    Top,
    Bottom,
    Left,
    Right,
    Isometric,
}

impl DrawingViewDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Back => "back",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
            Self::Isometric => "isometric",
        }
    }
}

#[derive(Debug, Clone)]
struct DrawingDetailView {
    id: String,
    label: String,
    center_x: f64,
    center_y: f64,
    width: f64,
    height: f64,
    scale: f64,
}

#[derive(Debug, Clone)]
struct DrawingDetailViewParams {
    center_x: f64,
    center_y: f64,
    width: f64,
    height: f64,
    scale: f64,
    label: String,
}

#[derive(Debug, Clone)]
struct DrawingUiState {
    view_mode: DrawingViewMode,
    view_direction: DrawingViewDirection,
    show_hidden_lines: bool,
    show_dimensions: bool,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    detail_views: Vec<DrawingDetailView>,
    next_detail_id: u64,
}

impl Default for DrawingUiState {
    fn default() -> Self {
        Self {
            view_mode: DrawingViewMode::ThreeD,
            view_direction: DrawingViewDirection::Front,
            show_hidden_lines: true,
            show_dimensions: true,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            detail_views: Vec::new(),
            next_detail_id: 1,
        }
    }
}

impl DrawingUiState {
    fn set_view_mode(&mut self, mode: DrawingViewMode) {
        self.view_mode = mode;
    }

    fn set_view_direction(&mut self, direction: DrawingViewDirection) {
        self.view_direction = direction;
        self.reset_view();
    }

    fn toggle_hidden_lines(&mut self) {
        self.show_hidden_lines = !self.show_hidden_lines;
    }

    fn toggle_dimensions(&mut self) {
        self.show_dimensions = !self.show_dimensions;
    }

    fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn adjust_zoom(&mut self, delta: f64) {
        self.zoom = (self.zoom * (1.0 + delta)).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn adjust_pan(&mut self, dx: f64, dy: f64) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    fn add_detail_view(&mut self, params: DrawingDetailViewParams) -> String {
        let id = format!("detail-{}", self.next_detail_id);
        self.next_detail_id = self.next_detail_id.saturating_add(1);
        self.detail_views.push(DrawingDetailView {
            id: id.clone(),
            label: params.label,
            center_x: params.center_x,
            center_y: params.center_y,
            width: params.width,
            height: params.height,
            scale: params.scale,
        });
        id
    }

    fn clear_detail_views(&mut self) {
        self.detail_views.clear();
    }

    fn snapshot(&self, case_id: &str) -> DrawingModeUiCaseSnapshot {
        DrawingModeUiCaseSnapshot {
            case_id: case_id.to_string(),
            view_mode: self.view_mode.as_str().to_string(),
            view_direction: self.view_direction.as_str().to_string(),
            show_hidden_lines: self.show_hidden_lines,
            show_dimensions: self.show_dimensions,
            zoom: self.zoom,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            detail_count: self.detail_views.len(),
            detail_ids: self
                .detail_views
                .iter()
                .map(|detail| detail.id.clone())
                .collect(),
            detail_labels: self
                .detail_views
                .iter()
                .map(|detail| detail.label.clone())
                .collect(),
            detail_scales: self
                .detail_views
                .iter()
                .map(|detail| detail.scale)
                .collect(),
            detail_centers_x: self
                .detail_views
                .iter()
                .map(|detail| detail.center_x)
                .collect(),
            detail_centers_y: self
                .detail_views
                .iter()
                .map(|detail| detail.center_y)
                .collect(),
            detail_widths: self
                .detail_views
                .iter()
                .map(|detail| detail.width)
                .collect(),
            detail_heights: self
                .detail_views
                .iter()
                .map(|detail| detail.height)
                .collect(),
            next_detail_id: self.next_detail_id,
        }
    }
}

pub fn build_drafting_drawing_mode_ui_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingDrawingModeUiParityManifest> {
    let corpus: DraftingDrawingModeUiReferenceCorpus =
        serde_json::from_str(DRAFTING_DRAWING_MODE_UI_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!(
                    "failed to parse drafting drawing-mode UI reference corpus: {error}"
                ),
            }
        })?;

    let reference_corpus_sha256 =
        sha256_hex(DRAFTING_DRAWING_MODE_UI_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_drawing_ui_snapshot();
    let replay_snapshot = collect_drawing_ui_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_ui_case_snapshots = sorted_case_snapshots(corpus.expected_ui_case_snapshots);
    let ui_contract_match = ui_case_snapshots_match(
        &snapshot.ui_case_snapshots,
        &expected_ui_case_snapshots,
        corpus.scalar_tolerance,
    );

    let deterministic_signature = parity_signature(
        &snapshot.ui_case_snapshots,
        reference_commit_match,
        ui_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingDrawingModeUiParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_DRAWING_MODE_UI_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_DRAWING_MODE_UI_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        ui_case_snapshots: snapshot.ui_case_snapshots,
        ui_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "drawing mode defaults to 3D/front with hidden lines and dimensions enabled"
                .to_string(),
            "changing drawing view direction resets zoom and pan to defaults".to_string(),
            "drawing zoom clamps to 0.1x..10x and pan updates deterministically".to_string(),
            "detail view ids are monotonic and clear does not reset next detail id".to_string(),
        ],
    })
}

fn collect_drawing_ui_snapshot() -> DrawingModeUiSnapshot {
    let mut ui_case_snapshots = vec![
        case_default_state(),
        case_mode_switch_and_direction_reset(),
        case_visibility_toggles(),
        case_zoom_and_pan_clamp(),
        case_detail_view_lifecycle(),
        case_direction_round_trip(),
    ];
    ui_case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    DrawingModeUiSnapshot { ui_case_snapshots }
}

fn case_default_state() -> DrawingModeUiCaseSnapshot {
    DrawingUiState::default().snapshot("default_state")
}

fn case_mode_switch_and_direction_reset() -> DrawingModeUiCaseSnapshot {
    let mut state = DrawingUiState::default();
    state.set_view_mode(DrawingViewMode::TwoD);
    state.adjust_pan(12.0, -8.0);
    state.adjust_zoom(0.4);
    state.set_view_direction(DrawingViewDirection::Top);
    state.snapshot("mode_switch_and_direction_reset")
}

fn case_visibility_toggles() -> DrawingModeUiCaseSnapshot {
    let mut state = DrawingUiState::default();
    state.set_view_mode(DrawingViewMode::TwoD);
    state.toggle_hidden_lines();
    state.toggle_dimensions();
    state.snapshot("visibility_toggles")
}

fn case_zoom_and_pan_clamp() -> DrawingModeUiCaseSnapshot {
    let mut state = DrawingUiState::default();
    state.set_view_mode(DrawingViewMode::TwoD);
    state.set_zoom(0.01);
    state.adjust_zoom(120.0);
    state.adjust_pan(150.25, -80.5);
    state.snapshot("zoom_and_pan_clamp")
}

fn case_detail_view_lifecycle() -> DrawingModeUiCaseSnapshot {
    let mut state = DrawingUiState::default();
    state.set_view_mode(DrawingViewMode::TwoD);
    state.add_detail_view(DrawingDetailViewParams {
        center_x: 25.0,
        center_y: 25.0,
        width: 50.0,
        height: 50.0,
        scale: 2.0,
        label: "A".to_string(),
    });
    state.add_detail_view(DrawingDetailViewParams {
        center_x: 0.0,
        center_y: 0.0,
        width: 40.0,
        height: 40.0,
        scale: 3.0,
        label: "B".to_string(),
    });
    state.clear_detail_views();
    state.add_detail_view(DrawingDetailViewParams {
        center_x: 5.0,
        center_y: -5.0,
        width: 20.0,
        height: 20.0,
        scale: 4.0,
        label: "C".to_string(),
    });
    state.snapshot("detail_view_lifecycle")
}

fn case_direction_round_trip() -> DrawingModeUiCaseSnapshot {
    let mut state = DrawingUiState::default();
    state.set_view_mode(DrawingViewMode::TwoD);
    state.set_view_direction(DrawingViewDirection::Back);
    state.set_view_direction(DrawingViewDirection::Bottom);
    state.set_view_direction(DrawingViewDirection::Left);
    state.set_view_direction(DrawingViewDirection::Right);
    state.set_view_direction(DrawingViewDirection::Isometric);
    state.set_view_direction(DrawingViewDirection::Front);
    state.snapshot("direction_round_trip")
}

fn sorted_case_snapshots(
    mut snapshots: Vec<DrawingModeUiCaseSnapshot>,
) -> Vec<DrawingModeUiCaseSnapshot> {
    snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    snapshots
}

fn ui_case_snapshots_match(
    actual: &[DrawingModeUiCaseSnapshot],
    expected: &[DrawingModeUiCaseSnapshot],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && left.view_mode == right.view_mode
            && left.view_direction == right.view_direction
            && left.show_hidden_lines == right.show_hidden_lines
            && left.show_dimensions == right.show_dimensions
            && left.detail_count == right.detail_count
            && left.detail_ids == right.detail_ids
            && left.detail_labels == right.detail_labels
            && left.next_detail_id == right.next_detail_id
            && approx_eq(left.zoom, right.zoom, tolerance)
            && approx_eq(left.pan_x, right.pan_x, tolerance)
            && approx_eq(left.pan_y, right.pan_y, tolerance)
            && detail_scales_match(&left.detail_scales, &right.detail_scales, tolerance)
            && detail_scales_match(&left.detail_centers_x, &right.detail_centers_x, tolerance)
            && detail_scales_match(&left.detail_centers_y, &right.detail_centers_y, tolerance)
            && detail_scales_match(&left.detail_widths, &right.detail_widths, tolerance)
            && detail_scales_match(&left.detail_heights, &right.detail_heights, tolerance)
    })
}

fn detail_scales_match(actual: &[f64], expected: &[f64], tolerance: f64) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| approx_eq(*left, *right, tolerance))
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn parity_signature(
    ui_case_snapshots: &[DrawingModeUiCaseSnapshot],
    reference_commit_match: bool,
    ui_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            ui_case_snapshots,
            reference_commit_match,
            ui_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting drawing mode parity payload"),
    );
    let digest = hasher.finalize();
    format!(
        "{:016x}",
        u64::from_be_bytes(digest[..8].try_into().expect("digest prefix"))
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
