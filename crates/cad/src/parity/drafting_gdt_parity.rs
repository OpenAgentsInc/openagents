use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{
    DatumFeatureSymbol, DatumRef, FeatureControlFrame, GdtSymbol, GeometryRef, MaterialCondition,
    Point2D,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_GDT_ISSUE_ID: &str = "VCAD-PARITY-071";
pub const DRAFTING_GDT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_gdt_vcad_reference.json";
const DRAFTING_GDT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_gdt_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingGdtParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub symbol_cases: Vec<GdtSymbolCase>,
    pub material_condition_cases: Vec<GdtMaterialConditionCase>,
    pub frame_cases: Vec<GdtFrameCase>,
    pub datum_cases: Vec<GdtDatumCase>,
    pub symbol_contract_match: bool,
    pub material_condition_contract_match: bool,
    pub frame_contract_match: bool,
    pub datum_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingGdtReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_symbol_cases: Vec<GdtSymbolCase>,
    expected_material_condition_cases: Vec<GdtMaterialConditionCase>,
    expected_frame_cases: Vec<GdtFrameCase>,
    expected_datum_cases: Vec<GdtDatumCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdtSymbolCase {
    pub symbol: String,
    pub dxf_text: String,
    pub requires_datum: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdtMaterialConditionCase {
    pub condition: String,
    pub dxf_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdtFrameCase {
    pub case_id: String,
    pub render_text: String,
    pub datum_requirement_satisfied: bool,
    pub line_count: usize,
    pub text_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdtDatumCase {
    pub case_id: String,
    pub label: String,
    pub line_count: usize,
    pub text_count: usize,
}

pub fn build_drafting_gdt_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingGdtParityManifest> {
    let corpus: DraftingGdtReferenceCorpus =
        serde_json::from_str(DRAFTING_GDT_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting gdt reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_GDT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_gdt_snapshot();
    let replay_snapshot = collect_gdt_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_symbol_cases = sorted_symbol_cases(corpus.expected_symbol_cases);
    let expected_material_condition_cases =
        sorted_material_condition_cases(corpus.expected_material_condition_cases);
    let expected_frame_cases = sorted_frame_cases(corpus.expected_frame_cases);
    let expected_datum_cases = sorted_datum_cases(corpus.expected_datum_cases);

    let symbol_contract_match = snapshot.symbol_cases == expected_symbol_cases;
    let material_condition_contract_match =
        snapshot.material_condition_cases == expected_material_condition_cases;
    let frame_contract_match = snapshot.frame_cases == expected_frame_cases;
    let datum_contract_match = snapshot.datum_cases == expected_datum_cases;

    let deterministic_signature = parity_signature(
        &snapshot.symbol_cases,
        &snapshot.material_condition_cases,
        &snapshot.frame_cases,
        &snapshot.datum_cases,
        reference_commit_match,
        symbol_contract_match,
        material_condition_contract_match,
        frame_contract_match,
        datum_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingGdtParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_GDT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_GDT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        symbol_cases: snapshot.symbol_cases,
        material_condition_cases: snapshot.material_condition_cases,
        frame_cases: snapshot.frame_cases,
        datum_cases: snapshot.datum_cases,
        symbol_contract_match,
        material_condition_contract_match,
        frame_contract_match,
        datum_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "gd&t symbol DXF tokens and datum requirements match vcad contracts".to_string(),
            "gd&t material condition modifiers serialize with vcad-style markers".to_string(),
            "feature control frame text and rendered primitive counts are deterministic"
                .to_string(),
            "datum feature symbol rendering and replay are deterministic".to_string(),
        ],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct GdtSnapshot {
    symbol_cases: Vec<GdtSymbolCase>,
    material_condition_cases: Vec<GdtMaterialConditionCase>,
    frame_cases: Vec<GdtFrameCase>,
    datum_cases: Vec<GdtDatumCase>,
}

fn collect_gdt_snapshot() -> GdtSnapshot {
    let symbols = [
        GdtSymbol::Straightness,
        GdtSymbol::Flatness,
        GdtSymbol::Circularity,
        GdtSymbol::Cylindricity,
        GdtSymbol::ProfileOfLine,
        GdtSymbol::ProfileOfSurface,
        GdtSymbol::Angularity,
        GdtSymbol::Perpendicularity,
        GdtSymbol::Parallelism,
        GdtSymbol::Position,
        GdtSymbol::Concentricity,
        GdtSymbol::Symmetry,
        GdtSymbol::CircularRunout,
        GdtSymbol::TotalRunout,
    ];
    let symbol_cases = sorted_symbol_cases(
        symbols
            .iter()
            .map(|symbol| GdtSymbolCase {
                symbol: gdt_symbol_name(*symbol).to_string(),
                dxf_text: symbol.dxf_text().to_string(),
                requires_datum: symbol.requires_datum(),
            })
            .collect(),
    );

    let conditions = [
        MaterialCondition::Mmc,
        MaterialCondition::Lmc,
        MaterialCondition::Rfs,
    ];
    let material_condition_cases = sorted_material_condition_cases(
        conditions
            .iter()
            .map(|condition| GdtMaterialConditionCase {
                condition: material_condition_name(*condition).to_string(),
                dxf_text: condition.dxf_text().to_string(),
            })
            .collect(),
    );

    let position_frame = FeatureControlFrame {
        symbol: GdtSymbol::Position,
        tolerance: 0.125,
        tolerance_is_diameter: true,
        material_condition: Some(MaterialCondition::Mmc),
        datum_a: Some(DatumRef {
            label: "A".to_string(),
            material_condition: None,
        }),
        datum_b: Some(DatumRef {
            label: "B".to_string(),
            material_condition: Some(MaterialCondition::Lmc),
        }),
        datum_c: Some(DatumRef {
            label: "C".to_string(),
            material_condition: None,
        }),
        position: Point2D::new(20.0, 10.0),
        leader_to: Some(GeometryRef::Point(Point2D::new(25.0, 4.0))),
    };
    let flatness_frame = FeatureControlFrame {
        symbol: GdtSymbol::Flatness,
        tolerance: 0.05,
        tolerance_is_diameter: false,
        material_condition: None,
        datum_a: None,
        datum_b: None,
        datum_c: None,
        position: Point2D::new(8.0, 6.0),
        leader_to: None,
    };
    let perpendicularity_missing_datum_frame = FeatureControlFrame {
        symbol: GdtSymbol::Perpendicularity,
        tolerance: 0.02,
        tolerance_is_diameter: false,
        material_condition: None,
        datum_a: None,
        datum_b: None,
        datum_c: None,
        position: Point2D::new(5.0, 3.0),
        leader_to: None,
    };

    let frame_cases = sorted_frame_cases(vec![
        frame_case("position_diameter_mmc", &position_frame),
        frame_case("flatness_no_datum", &flatness_frame),
        frame_case(
            "perpendicularity_missing_datum",
            &perpendicularity_missing_datum_frame,
        ),
    ]);

    let datum_a = DatumFeatureSymbol {
        label: "A".to_string(),
        position: Point2D::new(4.0, 4.0),
        leader_to: Some(GeometryRef::Point(Point2D::new(6.0, 1.0))),
    };
    let datum_b = DatumFeatureSymbol {
        label: "B".to_string(),
        position: Point2D::new(9.0, 7.5),
        leader_to: None,
    };

    let datum_cases = sorted_datum_cases(vec![
        datum_case("datum_a_with_leader", &datum_a),
        datum_case("datum_b_without_leader", &datum_b),
    ]);

    GdtSnapshot {
        symbol_cases,
        material_condition_cases,
        frame_cases,
        datum_cases,
    }
}

fn frame_case(case_id: &str, frame: &FeatureControlFrame) -> GdtFrameCase {
    let rendered = frame.render();
    GdtFrameCase {
        case_id: case_id.to_string(),
        render_text: frame.render_text(),
        datum_requirement_satisfied: frame.datum_requirement_satisfied(),
        line_count: rendered.lines.len(),
        text_count: rendered.texts.len(),
    }
}

fn datum_case(case_id: &str, datum: &DatumFeatureSymbol) -> GdtDatumCase {
    let rendered = datum.render();
    GdtDatumCase {
        case_id: case_id.to_string(),
        label: datum.label.clone(),
        line_count: rendered.lines.len(),
        text_count: rendered.texts.len(),
    }
}

fn gdt_symbol_name(symbol: GdtSymbol) -> &'static str {
    match symbol {
        GdtSymbol::Straightness => "straightness",
        GdtSymbol::Flatness => "flatness",
        GdtSymbol::Circularity => "circularity",
        GdtSymbol::Cylindricity => "cylindricity",
        GdtSymbol::ProfileOfLine => "profile_of_line",
        GdtSymbol::ProfileOfSurface => "profile_of_surface",
        GdtSymbol::Angularity => "angularity",
        GdtSymbol::Perpendicularity => "perpendicularity",
        GdtSymbol::Parallelism => "parallelism",
        GdtSymbol::Position => "position",
        GdtSymbol::Concentricity => "concentricity",
        GdtSymbol::Symmetry => "symmetry",
        GdtSymbol::CircularRunout => "circular_runout",
        GdtSymbol::TotalRunout => "total_runout",
    }
}

fn material_condition_name(condition: MaterialCondition) -> &'static str {
    match condition {
        MaterialCondition::Mmc => "mmc",
        MaterialCondition::Lmc => "lmc",
        MaterialCondition::Rfs => "rfs",
    }
}

fn sorted_symbol_cases(mut cases: Vec<GdtSymbolCase>) -> Vec<GdtSymbolCase> {
    cases.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    cases
}

fn sorted_material_condition_cases(
    mut cases: Vec<GdtMaterialConditionCase>,
) -> Vec<GdtMaterialConditionCase> {
    cases.sort_by(|left, right| left.condition.cmp(&right.condition));
    cases
}

fn sorted_frame_cases(mut cases: Vec<GdtFrameCase>) -> Vec<GdtFrameCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_datum_cases(mut cases: Vec<GdtDatumCase>) -> Vec<GdtDatumCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn parity_signature(
    symbol_cases: &[GdtSymbolCase],
    material_condition_cases: &[GdtMaterialConditionCase],
    frame_cases: &[GdtFrameCase],
    datum_cases: &[GdtDatumCase],
    reference_commit_match: bool,
    symbol_contract_match: bool,
    material_condition_contract_match: bool,
    frame_contract_match: bool,
    datum_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            symbol_cases,
            material_condition_cases,
            frame_cases,
            datum_cases,
            reference_commit_match,
            symbol_contract_match,
            material_condition_contract_match,
            frame_contract_match,
            datum_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting gdt parity payload"),
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
    use super::GdtDatumCase;
    use super::GdtFrameCase;
    use super::GdtMaterialConditionCase;
    use super::GdtSymbolCase;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let symbols = vec![GdtSymbolCase {
            symbol: "flatness".to_string(),
            dxf_text: "%%cF".to_string(),
            requires_datum: false,
        }];
        let conditions = vec![GdtMaterialConditionCase {
            condition: "mmc".to_string(),
            dxf_text: "(M)".to_string(),
        }];
        let frames = vec![GdtFrameCase {
            case_id: "flatness_no_datum".to_string(),
            render_text: "FLAT|0.050".to_string(),
            datum_requirement_satisfied: true,
            line_count: 5,
            text_count: 2,
        }];
        let datums = vec![GdtDatumCase {
            case_id: "datum_a_with_leader".to_string(),
            label: "A".to_string(),
            line_count: 4,
            text_count: 1,
        }];

        let first = parity_signature(
            &symbols,
            &conditions,
            &frames,
            &datums,
            true,
            true,
            true,
            true,
            true,
            true,
            "sha",
        );
        let second = parity_signature(
            &symbols,
            &conditions,
            &frames,
            &datums,
            true,
            true,
            true,
            true,
            true,
            true,
            "sha",
        );
        assert_eq!(first, second);
    }
}
