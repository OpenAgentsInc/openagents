use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_RAYTRACE_QUALITY_MODE_ISSUE_ID: &str = "VCAD-PARITY-101";
pub const RAYTRACE_QUALITY_MODE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/raytrace_quality_mode_vcad_reference.json";
const RAYTRACE_QUALITY_MODE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/raytrace_quality_mode_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceQualityModeParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub default_quality_match: bool,
    pub quality_mode_set_match: bool,
    pub quality_scale_table_match: bool,
    pub interaction_override_match: bool,
    pub sample_set_match: bool,
    pub cap_enforcement_match: bool,
    pub deterministic_replay_match: bool,
    pub default_quality: String,
    pub quality_modes: Vec<String>,
    pub quality_scales: Vec<RaytraceQualityScaleSnapshot>,
    pub interaction_policy: RaytraceInteractionPolicySnapshot,
    pub samples: Vec<RaytraceQualityModeSample>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceQualityModeReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_default_quality: String,
    expected_quality_modes: Vec<String>,
    expected_quality_scales: Vec<RaytraceQualityScaleSnapshot>,
    expected_interaction_policy: RaytraceInteractionPolicySnapshot,
    expected_samples: Vec<RaytraceQualityModeSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceQualityModeSnapshot {
    default_quality: String,
    quality_modes: Vec<String>,
    quality_scales: Vec<RaytraceQualityScaleSnapshot>,
    interaction_policy: RaytraceInteractionPolicySnapshot,
    samples: Vec<RaytraceQualityModeSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceQualityScaleSnapshot {
    pub quality: String,
    pub scale: f64,
    pub max_pixels: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceInteractionPolicySnapshot {
    pub interacting_frame_threshold: u32,
    pub interacting_effective_quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceQualityModeSample {
    pub case_id: String,
    pub requested_quality: String,
    pub frame_index: u32,
    pub viewport: [u32; 2],
    pub effective_quality: String,
    pub max_pixels: u32,
    pub render_size: [u32; 2],
    pub total_pixels: u32,
    pub cap_scale_factor: f64,
    pub cap_applied: bool,
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

    fn scale(self) -> f64 {
        match self {
            Self::Draft => 0.5,
            Self::Standard => 1.0,
            Self::High => 2.0,
        }
    }

    fn max_pixels(self) -> u32 {
        match self {
            // Mirrors vcad RayTracedViewport quality budgets.
            Self::Draft => 640 * 480,
            Self::Standard => 1280 * 720,
            Self::High => 1920 * 1080,
        }
    }

    fn all() -> [Self; 3] {
        [Self::Draft, Self::Standard, Self::High]
    }
}

#[derive(Debug, Clone)]
struct SampleInput {
    case_id: &'static str,
    requested_quality: RaytraceQuality,
    frame_index: u32,
    viewport: [u32; 2],
}

pub fn build_raytrace_quality_mode_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<RaytraceQualityModeParityManifest> {
    let reference: RaytraceQualityModeReferenceFixture =
        serde_json::from_str(RAYTRACE_QUALITY_MODE_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing raytrace quality mode reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 =
        sha256_hex(RAYTRACE_QUALITY_MODE_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let default_quality_match = snapshot.default_quality == reference.expected_default_quality;
    let quality_mode_set_match = snapshot.quality_modes == reference.expected_quality_modes;
    let quality_scale_table_match = sorted_scales(snapshot.quality_scales.clone())
        == sorted_scales(reference.expected_quality_scales.clone());
    let interaction_override_match =
        snapshot.interaction_policy == reference.expected_interaction_policy;
    let sample_set_match = samples_match(
        snapshot.samples.clone(),
        reference.expected_samples.clone(),
        1e-9,
    );
    let cap_enforcement_match = snapshot
        .samples
        .iter()
        .all(|sample| sample.total_pixels <= sample.max_pixels);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        default_quality_match,
        quality_mode_set_match,
        quality_scale_table_match,
        interaction_override_match,
        sample_set_match,
        cap_enforcement_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(RaytraceQualityModeParityManifest {
        manifest_version: 1,
        issue_id: PARITY_RAYTRACE_QUALITY_MODE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: RAYTRACE_QUALITY_MODE_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        default_quality_match,
        quality_mode_set_match,
        quality_scale_table_match,
        interaction_override_match,
        sample_set_match,
        cap_enforcement_match,
        deterministic_replay_match,
        default_quality: snapshot.default_quality,
        quality_modes: snapshot.quality_modes,
        quality_scales: snapshot.quality_scales,
        interaction_policy: snapshot.interaction_policy,
        samples: snapshot.samples,
        deterministic_signature,
        parity_contracts: vec![
            "raytrace quality enum remains draft/standard/high with deterministic ordering"
                .to_string(),
            "quality scales remain draft=0.5x, standard=1.0x, high=2.0x"
                .to_string(),
            "quality pixel caps remain draft=640x480, standard=1280x720, high=1920x1080"
                .to_string(),
            "camera interaction (frame<=1) forces effective draft budget regardless of selected quality"
                .to_string(),
            "render resolution is floor(viewport*selected_scale) then downscaled to enforce effective max pixel budget"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> RaytraceQualityModeSnapshot {
    // Mirrors ui-store default for raytrace quality.
    let default_quality = RaytraceQuality::Draft.as_str().to_string();

    let quality_modes = RaytraceQuality::all()
        .iter()
        .map(|quality| quality.as_str().to_string())
        .collect::<Vec<_>>();

    let quality_scales = RaytraceQuality::all()
        .iter()
        .map(|quality| RaytraceQualityScaleSnapshot {
            quality: quality.as_str().to_string(),
            scale: canonical_f64(quality.scale()),
            max_pixels: quality.max_pixels(),
        })
        .collect::<Vec<_>>();

    let interaction_policy = RaytraceInteractionPolicySnapshot {
        interacting_frame_threshold: 1,
        interacting_effective_quality: RaytraceQuality::Draft.as_str().to_string(),
    };

    let mut samples = vec![
        sample(SampleInput {
            case_id: "draft_idle_1280x720",
            requested_quality: RaytraceQuality::Draft,
            frame_index: 8,
            viewport: [1280, 720],
        }),
        sample(SampleInput {
            case_id: "standard_idle_1280x720",
            requested_quality: RaytraceQuality::Standard,
            frame_index: 8,
            viewport: [1280, 720],
        }),
        sample(SampleInput {
            case_id: "high_idle_1920x1080",
            requested_quality: RaytraceQuality::High,
            frame_index: 8,
            viewport: [1920, 1080],
        }),
        sample(SampleInput {
            case_id: "standard_interacting_1280x720",
            requested_quality: RaytraceQuality::Standard,
            frame_index: 1,
            viewport: [1280, 720],
        }),
        sample(SampleInput {
            case_id: "high_interacting_1920x1080",
            requested_quality: RaytraceQuality::High,
            frame_index: 1,
            viewport: [1920, 1080],
        }),
        sample(SampleInput {
            case_id: "high_idle_3440x1440",
            requested_quality: RaytraceQuality::High,
            frame_index: 8,
            viewport: [3440, 1440],
        }),
    ];
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    RaytraceQualityModeSnapshot {
        default_quality,
        quality_modes,
        quality_scales,
        interaction_policy,
        samples,
    }
}

fn sample(input: SampleInput) -> RaytraceQualityModeSample {
    let requested_scale = input.requested_quality.scale();
    let requested_quality = input.requested_quality.as_str().to_string();

    let effective_quality = effective_quality(input.requested_quality, input.frame_index);
    let max_pixels = effective_quality.max_pixels();

    let mut render_width = ((input.viewport[0] as f64) * requested_scale).floor() as u32;
    let mut render_height = ((input.viewport[1] as f64) * requested_scale).floor() as u32;
    render_width = render_width.max(1);
    render_height = render_height.max(1);

    let initial_total = render_width.saturating_mul(render_height);
    let mut cap_scale_factor = 1.0;
    let mut cap_applied = false;

    if initial_total > max_pixels {
        cap_applied = true;
        cap_scale_factor = ((max_pixels as f64) / (initial_total as f64)).sqrt();
        render_width = ((render_width as f64) * cap_scale_factor).floor() as u32;
        render_height = ((render_height as f64) * cap_scale_factor).floor() as u32;
        render_width = render_width.max(1);
        render_height = render_height.max(1);
    }

    let total_pixels = render_width.saturating_mul(render_height);

    RaytraceQualityModeSample {
        case_id: input.case_id.to_string(),
        requested_quality,
        frame_index: input.frame_index,
        viewport: input.viewport,
        effective_quality: effective_quality.as_str().to_string(),
        max_pixels,
        render_size: [render_width, render_height],
        total_pixels,
        cap_scale_factor: canonical_f64(cap_scale_factor),
        cap_applied,
    }
}

fn effective_quality(requested_quality: RaytraceQuality, frame_index: u32) -> RaytraceQuality {
    if frame_index <= 1 {
        // Mirrors RayTracedViewport: currentFrame<=1 forces draft for responsiveness.
        RaytraceQuality::Draft
    } else {
        requested_quality
    }
}

fn sorted_scales(
    mut values: Vec<RaytraceQualityScaleSnapshot>,
) -> Vec<RaytraceQualityScaleSnapshot> {
    values.sort_by(|left, right| left.quality.cmp(&right.quality));
    values
}

fn samples_match(
    mut actual: Vec<RaytraceQualityModeSample>,
    mut expected: Vec<RaytraceQualityModeSample>,
    epsilon: f64,
) -> bool {
    actual.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    if actual.len() != expected.len() {
        return false;
    }

    actual
        .iter()
        .zip(expected.iter())
        .all(|(left, right)| sample_approx_eq(left, right, epsilon))
}

fn sample_approx_eq(
    left: &RaytraceQualityModeSample,
    right: &RaytraceQualityModeSample,
    epsilon: f64,
) -> bool {
    left.case_id == right.case_id
        && left.requested_quality == right.requested_quality
        && left.frame_index == right.frame_index
        && left.viewport == right.viewport
        && left.effective_quality == right.effective_quality
        && left.max_pixels == right.max_pixels
        && left.render_size == right.render_size
        && left.total_pixels == right.total_pixels
        && left.cap_applied == right.cap_applied
        && approx_eq(left.cap_scale_factor, right.cap_scale_factor, epsilon)
}

fn canonical_f64(value: f64) -> f64 {
    let rounded = (value * 1_000_000_000.0).round() / 1_000_000_000.0;
    if rounded.abs() < 1e-12 { 0.0 } else { rounded }
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &RaytraceQualityModeSnapshot,
    reference_commit_match: bool,
    default_quality_match: bool,
    quality_mode_set_match: bool,
    quality_scale_table_match: bool,
    interaction_override_match: bool,
    sample_set_match: bool,
    cap_enforcement_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        default_quality_match,
        quality_mode_set_match,
        quality_scale_table_match,
        interaction_override_match,
        sample_set_match,
        cap_enforcement_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize raytrace quality mode parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{RaytraceQuality, effective_quality};

    #[test]
    fn interaction_uses_draft_quality_for_first_frame() {
        assert_eq!(
            effective_quality(RaytraceQuality::High, 1),
            RaytraceQuality::Draft
        );
        assert_eq!(
            effective_quality(RaytraceQuality::Standard, 0),
            RaytraceQuality::Draft
        );
        assert_eq!(
            effective_quality(RaytraceQuality::High, 2),
            RaytraceQuality::High
        );
    }
}
