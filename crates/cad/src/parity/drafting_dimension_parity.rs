use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{
    AngleDefinition, AngularDimension, DimensionStyle, LinearDimension, LinearDimensionType,
    OrdinateDimension, Point2D, RadialDimension,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_DIMENSION_ISSUE_ID: &str = "VCAD-PARITY-070";
pub const DRAFTING_DIMENSION_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_dimension_vcad_reference.json";
const DRAFTING_DIMENSION_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_dimension_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingDimensionParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub measurement_cases: Vec<DimensionMeasurementCase>,
    pub render_cases: Vec<DimensionRenderCase>,
    pub measurement_contract_match: bool,
    pub render_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingDimensionReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    measurement_tolerance: f64,
    expected_measurement_cases: Vec<DimensionMeasurementCase>,
    expected_render_cases: Vec<DimensionRenderCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionMeasurementCase {
    pub case_id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DimensionRenderCase {
    pub case_id: String,
    pub line_count: usize,
    pub arc_count: usize,
    pub text_count: usize,
    pub primary_label: String,
}

pub fn build_drafting_dimension_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingDimensionParityManifest> {
    let corpus: DraftingDimensionReferenceCorpus =
        serde_json::from_str(DRAFTING_DIMENSION_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting dimension reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_DIMENSION_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_dimension_snapshot();
    let replay_snapshot = collect_dimension_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_measurement_cases = sorted_measurements(corpus.expected_measurement_cases);
    let expected_render_cases = sorted_render_cases(corpus.expected_render_cases);

    let measurement_contract_match = measurements_match(
        &snapshot.measurement_cases,
        &expected_measurement_cases,
        corpus.measurement_tolerance,
    );
    let render_contract_match = snapshot.render_cases == expected_render_cases;

    let deterministic_signature = parity_signature(
        &snapshot.measurement_cases,
        &snapshot.render_cases,
        reference_commit_match,
        measurement_contract_match,
        render_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingDimensionParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_DIMENSION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_DIMENSION_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        measurement_cases: snapshot.measurement_cases,
        render_cases: snapshot.render_cases,
        measurement_contract_match,
        render_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "linear dimensions match vcad horizontal/vertical/aligned/rotated measurement semantics"
                .to_string(),
            "angular/radial/ordinate dimension values align with vcad reference outputs"
                .to_string(),
            "dimension render contracts (line/arc/text counts + labels) remain deterministic"
                .to_string(),
            "drafting dimension parity fixtures replay deterministically".to_string(),
        ],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct DimensionSnapshot {
    measurement_cases: Vec<DimensionMeasurementCase>,
    render_cases: Vec<DimensionRenderCase>,
}

fn collect_dimension_snapshot() -> DimensionSnapshot {
    let style = DimensionStyle::default();

    let linear_horizontal = LinearDimension {
        start: Point2D::new(0.0, 0.0),
        end: Point2D::new(12.0, 5.0),
        dimension_type: LinearDimensionType::Horizontal,
        offset: 3.0,
        style,
        override_text: None,
        geometry_ref: None,
    };
    let linear_vertical = LinearDimension {
        start: Point2D::new(0.0, 0.0),
        end: Point2D::new(12.0, 5.0),
        dimension_type: LinearDimensionType::Vertical,
        offset: 3.0,
        style,
        override_text: None,
        geometry_ref: None,
    };
    let linear_aligned = LinearDimension {
        start: Point2D::new(0.0, 0.0),
        end: Point2D::new(12.0, 5.0),
        dimension_type: LinearDimensionType::Aligned,
        offset: 3.0,
        style,
        override_text: None,
        geometry_ref: None,
    };
    let linear_rotated = LinearDimension {
        start: Point2D::new(0.0, 0.0),
        end: Point2D::new(10.0, 10.0),
        dimension_type: LinearDimensionType::Rotated {
            angle_radians: std::f64::consts::FRAC_PI_4,
        },
        offset: 3.0,
        style,
        override_text: None,
        geometry_ref: None,
    };

    let angular = AngularDimension {
        definition: AngleDefinition::FromPoints {
            start: Point2D::new(1.0, 0.0),
            vertex: Point2D::new(0.0, 0.0),
            end: Point2D::new(0.0, 1.0),
        },
        radius: 2.0,
        style,
        override_text: None,
        geometry_ref: None,
    };

    let radial_radius = RadialDimension {
        center: Point2D::new(0.0, 0.0),
        rim_point: Point2D::new(0.0, 5.0),
        is_diameter: false,
        style,
        override_text: None,
        geometry_ref: None,
    };
    let radial_diameter = RadialDimension {
        center: Point2D::new(0.0, 0.0),
        rim_point: Point2D::new(0.0, 5.0),
        is_diameter: true,
        style,
        override_text: None,
        geometry_ref: None,
    };

    let ordinate_x = OrdinateDimension {
        datum: Point2D::new(0.0, 0.0),
        target: Point2D::new(7.5, 4.25),
        is_x: true,
        style,
        override_text: None,
        geometry_ref: None,
    };
    let ordinate_y = OrdinateDimension {
        datum: Point2D::new(0.0, 0.0),
        target: Point2D::new(7.5, 4.25),
        is_x: false,
        style,
        override_text: None,
        geometry_ref: None,
    };

    let measurement_cases = sorted_measurements(vec![
        DimensionMeasurementCase {
            case_id: "angular_from_points".to_string(),
            value: angular.measurement_degrees(),
        },
        DimensionMeasurementCase {
            case_id: "linear_aligned".to_string(),
            value: linear_aligned.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "linear_horizontal".to_string(),
            value: linear_horizontal.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "linear_rotated_45".to_string(),
            value: linear_rotated.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "linear_vertical".to_string(),
            value: linear_vertical.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "ordinate_x".to_string(),
            value: ordinate_x.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "ordinate_y".to_string(),
            value: ordinate_y.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "radial_diameter".to_string(),
            value: radial_diameter.measurement_value(),
        },
        DimensionMeasurementCase {
            case_id: "radial_radius".to_string(),
            value: radial_radius.measurement_value(),
        },
    ]);

    let render_cases = sorted_render_cases(vec![
        render_case("linear_horizontal", linear_horizontal.render()),
        render_case("angular_from_points", angular.render()),
        render_case("radial_radius", radial_radius.render()),
        render_case("radial_diameter", radial_diameter.render()),
        render_case("ordinate_x", ordinate_x.render()),
        render_case("ordinate_y", ordinate_y.render()),
    ]);

    DimensionSnapshot {
        measurement_cases,
        render_cases,
    }
}

fn render_case(case_id: &str, rendered: crate::drafting::RenderedDimension) -> DimensionRenderCase {
    DimensionRenderCase {
        case_id: case_id.to_string(),
        line_count: rendered.lines.len(),
        arc_count: rendered.arcs.len(),
        text_count: rendered.texts.len(),
        primary_label: rendered
            .texts
            .first()
            .map(|text| text.text.clone())
            .unwrap_or_default(),
    }
}

fn sorted_measurements(mut cases: Vec<DimensionMeasurementCase>) -> Vec<DimensionMeasurementCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_render_cases(mut cases: Vec<DimensionRenderCase>) -> Vec<DimensionRenderCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn measurements_match(
    actual: &[DimensionMeasurementCase],
    expected: &[DimensionMeasurementCase],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id && (left.value - right.value).abs() <= tolerance
    })
}

fn parity_signature(
    measurement_cases: &[DimensionMeasurementCase],
    render_cases: &[DimensionRenderCase],
    reference_commit_match: bool,
    measurement_contract_match: bool,
    render_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            measurement_cases,
            render_cases,
            reference_commit_match,
            measurement_contract_match,
            render_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting dimension parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::DimensionMeasurementCase;
    use super::DimensionRenderCase;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let measurements = vec![DimensionMeasurementCase {
            case_id: "linear_horizontal".to_string(),
            value: 12.0,
        }];
        let renders = vec![DimensionRenderCase {
            case_id: "linear_horizontal".to_string(),
            line_count: 1,
            arc_count: 0,
            text_count: 1,
            primary_label: "12.00".to_string(),
        }];

        let first = parity_signature(&measurements, &renders, true, true, true, true, "sha");
        let second = parity_signature(&measurements, &renders, true, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
