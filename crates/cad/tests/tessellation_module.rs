use std::collections::BTreeMap;

use openagents_cad::CadError;
use openagents_cad::eval::evaluate_feature_graph_deterministic;
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};
use openagents_cad::mesh::CadMeshPayload;
use openagents_cad::tessellation::tessellate_rebuild_result;

fn encode_for_golden(payload: &CadMeshPayload, hash: &str) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        payload.variant_id,
        payload.mesh_id,
        payload.vertices.len(),
        payload.triangle_indices.len() / 3,
        payload.edges.len(),
        payload.material_slots.len(),
        payload.bounds.min_mm[0],
        payload.bounds.min_mm[1],
        payload.bounds.max_mm[0],
        payload.bounds.max_mm[1],
        hash
    )
}

fn rack_demo_graph(variant_id: &str) -> FeatureGraph {
    let (width_param, vent_spacing_param, vent_count_param) = match variant_id {
        "variant.lightweight" => ("width_light_mm", "vent_spacing_wide_mm", "vent_count_low"),
        "variant.low-cost" => ("width_cost_mm", "vent_spacing_cost_mm", "vent_count_mid"),
        "variant.stiffness" => ("width_stiff_mm", "vent_spacing_tight_mm", "vent_count_high"),
        _ => ("width_base_mm", "vent_spacing_base_mm", "vent_count_base"),
    };
    FeatureGraph {
        nodes: vec![
            FeatureNode {
                id: "feature.base".to_string(),
                name: "base".to_string(),
                operation_key: "primitive.box.v1".to_string(),
                depends_on: Vec::new(),
                params: BTreeMap::from([
                    ("width_param".to_string(), width_param.to_string()),
                    ("depth_param".to_string(), "depth_mm".to_string()),
                    ("height_param".to_string(), "height_mm".to_string()),
                    ("variant".to_string(), variant_id.to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.mount_hole".to_string(),
                name: "mount_hole".to_string(),
                operation_key: "cut.hole.v1".to_string(),
                depends_on: vec!["feature.base".to_string()],
                params: BTreeMap::from([
                    (
                        "radius_param".to_string(),
                        "mount_hole_radius_mm".to_string(),
                    ),
                    ("depth_param".to_string(), "mount_hole_depth_mm".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.vent_pattern".to_string(),
                name: "vent_pattern".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.mount_hole".to_string()],
                params: BTreeMap::from([
                    ("count_param".to_string(), vent_count_param.to_string()),
                    ("spacing_param".to_string(), vent_spacing_param.to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.edge_marker".to_string(),
                name: "edge_marker".to_string(),
                operation_key: "fillet.placeholder.v1".to_string(),
                depends_on: vec!["feature.base".to_string()],
                params: BTreeMap::from([
                    ("radius_param".to_string(), "edge_radius_mm".to_string()),
                    ("kind".to_string(), "fillet".to_string()),
                ]),
            },
        ],
    }
}

fn golden_receipts() -> BTreeMap<String, String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{root}/tests/goldens/tessellation_rack_primitives.json");
    let payload = std::fs::read_to_string(path).expect("tessellation golden should be readable");
    serde_json::from_str(&payload).expect("tessellation golden should parse")
}

#[test]
fn tessellation_is_deterministic_for_demo_rack_variants() {
    let goldens = golden_receipts();
    let variants = [
        "variant.baseline",
        "variant.lightweight",
        "variant.low-cost",
        "variant.stiffness",
    ];
    for variant in variants {
        let graph = rack_demo_graph(variant);
        let rebuild = evaluate_feature_graph_deterministic(&graph)
            .expect("demo graph should deterministically rebuild");
        let (payload_a, receipt_a) = tessellate_rebuild_result(&graph, &rebuild, 12, variant)
            .expect("tessellation should succeed");
        let (payload_b, receipt_b) = tessellate_rebuild_result(&graph, &rebuild, 12, variant)
            .expect("tessellation should remain deterministic");
        assert_eq!(payload_a, payload_b);
        assert_eq!(receipt_a, receipt_b);
        let encoded = payload_a
            .to_binary_payload()
            .expect("mesh payload should encode for golden");
        let golden_key = encode_for_golden(&payload_a, &encoded.deterministic_hash);
        assert_eq!(
            goldens.get(variant).map(String::as_str),
            Some(golden_key.as_str()),
            "variant {variant} tessellation output diverged from golden"
        );
    }
}

#[test]
fn tessellation_rejects_unknown_operation_keys() {
    let graph = FeatureGraph {
        nodes: vec![FeatureNode {
            id: "feature.unknown".to_string(),
            name: "unknown".to_string(),
            operation_key: "unknown.operation.v1".to_string(),
            depends_on: Vec::new(),
            params: BTreeMap::new(),
        }],
    };
    let rebuild = evaluate_feature_graph_deterministic(&graph)
        .expect("graph should still evaluate deterministically");
    let error = tessellate_rebuild_result(&graph, &rebuild, 1, "variant.baseline")
        .expect_err("unknown operation key must fail");
    assert_eq!(
        error,
        CadError::EvalFailed {
            reason: "tessellation has no handler for operation_key=unknown.operation.v1 feature_id=feature.unknown".to_string(),
        }
    );
}
