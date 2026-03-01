use std::collections::BTreeMap;

use openagents_cad::document::CadDocument;
use openagents_cad::format::ApcadDocumentEnvelope;

fn golden(path: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let full_path = format!("{root}/tests/goldens/{path}");
    std::fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("failed to read golden fixture {full_path}: {error}"))
}

#[test]
fn apcad_envelope_empty_matches_snapshot_fixture() {
    let envelope = ApcadDocumentEnvelope::new("doc-empty");
    let actual = envelope
        .to_pretty_json()
        .expect("empty envelope serialization should succeed");
    let expected = golden("apcad_envelope_empty.json");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
fn apcad_envelope_single_feature_matches_snapshot_fixture() {
    let mut envelope = ApcadDocumentEnvelope::new("doc-minimal");
    envelope
        .stable_ids
        .insert("feature.base".to_string(), "sid-001".to_string());
    envelope
        .metadata
        .insert("material".to_string(), "6061-T6".to_string());
    envelope
        .metadata
        .insert("title".to_string(), "Mac Studio Rack".to_string());
    envelope.analysis_cache = Some(BTreeMap::from([(
        "weight_kg".to_string(),
        "2.71".to_string(),
    )]));

    let actual = envelope
        .to_pretty_json()
        .expect("single-feature envelope serialization should succeed");
    let expected = golden("apcad_envelope_single_feature.json");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
fn apcad_envelope_with_sketch_entities_matches_snapshot_fixture() {
    let mut envelope = ApcadDocumentEnvelope::new("doc-sketch");
    envelope
        .metadata
        .insert("title".to_string(), "Sketch Kickoff".to_string());
    envelope
        .stable_ids
        .insert("feature.base".to_string(), "sid-001".to_string());
    envelope
        .sketch
        .insert_plane(openagents_cad::sketch::CadSketchPlane {
            id: "plane.front".to_string(),
            name: "Front".to_string(),
            origin_mm: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
        })
        .expect("sketch plane should insert");
    envelope
        .sketch
        .insert_entity(openagents_cad::sketch::CadSketchEntity::Line {
            id: "entity.line.001".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [120.0, 0.0],
            anchor_ids: ["anchor.l.start".to_string(), "anchor.l.end".to_string()],
            construction: false,
        })
        .expect("line entity should insert");
    envelope
        .sketch
        .insert_entity(openagents_cad::sketch::CadSketchEntity::Arc {
            id: "entity.arc.001".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [60.0, 40.0],
            radius_mm: 20.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.a.center".to_string(),
                "anchor.a.start".to_string(),
                "anchor.a.end".to_string(),
            ],
            construction: false,
        })
        .expect("arc entity should insert");
    envelope
        .sketch
        .insert_entity(openagents_cad::sketch::CadSketchEntity::Point {
            id: "entity.point.001".to_string(),
            plane_id: "plane.front".to_string(),
            position_mm: [15.0, 12.0],
            anchor_id: "anchor.p.001".to_string(),
            construction: true,
        })
        .expect("point entity should insert");

    let actual = envelope
        .to_pretty_json()
        .expect("sketch envelope serialization should succeed");
    let expected = golden("apcad_envelope_with_sketch_entities.json");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
fn cad_document_minimal_snapshot_round_trip_is_stable() {
    let fixture = golden("cad_document_minimal.json");
    let parsed = CadDocument::from_json(&fixture).expect("fixture should parse as CadDocument");
    let serialized = parsed
        .to_pretty_json()
        .expect("fixture should serialize with deterministic ordering");
    assert_eq!(serialized.trim_end(), fixture.trim_end());
}
