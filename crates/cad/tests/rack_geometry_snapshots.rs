#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};

use openagents_cad::eval::evaluate_feature_graph_deterministic;
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};
use openagents_cad::params::{ParameterStore, ScalarUnit, ScalarValue};
use openagents_cad::rack::{
    MacStudioRackTemplate, MacStudioRackTemplateParams, RackObjectivePreset,
    generate_mac_studio_rack_template, generate_objective_variants,
};
use openagents_cad::tessellation::tessellate_rebuild_result;
use serde::{Deserialize, Serialize};

const GOLDEN_FILE: &str = "rack_geometry_snapshots.json";
const OBJECTIVE_SEED: u64 = 777;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RackVariantGeometrySnapshot {
    variant_id: String,
    objective: String,
    seed: u64,
    rebuild_hash: String,
    ordered_feature_ids: Vec<String>,
    feature_hashes: BTreeMap<String, String>,
    feature_records: Vec<String>,
    mesh_hash: String,
    mesh_id: String,
    mesh_vertex_count: usize,
    mesh_triangle_count: usize,
    mesh_edge_count: usize,
    mesh_bounds: String,
    key_parameters: BTreeMap<String, String>,
    semantic_refs: BTreeMap<String, String>,
}

fn fixture_path(path: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    format!("{root}/tests/goldens/{path}")
}

fn objective_label(objective: RackObjectivePreset) -> &'static str {
    match objective {
        RackObjectivePreset::LowestWeight => "lowest_weight",
        RackObjectivePreset::LowestCost => "lowest_cost",
        RackObjectivePreset::HighestStiffness => "highest_stiffness",
        RackObjectivePreset::AirflowBiased => "airflow_biased",
    }
}

fn unit_label(unit: ScalarUnit) -> &'static str {
    match unit {
        ScalarUnit::Millimeter => "mm",
        ScalarUnit::Degree => "deg",
        ScalarUnit::Unitless => "unitless",
    }
}

fn format_scalar(value: ScalarValue) -> String {
    format!("{:.6} {}", value.value, unit_label(value.unit))
}

fn key_parameter_snapshot(params: &ParameterStore) -> BTreeMap<String, String> {
    const PARAM_KEYS: [&str; 14] = [
        "frame_width_mm",
        "frame_depth_mm",
        "frame_height_mm",
        "wall_thickness_mm",
        "effective_wall_thickness_mm",
        "vent_rows",
        "vent_cols",
        "vent_spacing_mm",
        "vent_hole_radius_mm",
        "vent_density_scale",
        "opt_rib_count",
        "opt_rib_spacing_mm",
        "opt_rib_thickness_mm",
        "wall_mount_hole_count",
    ];
    let mut snapshot = BTreeMap::new();
    for key in PARAM_KEYS {
        if let Some(value) = params.get(key) {
            snapshot.insert(key.to_string(), format_scalar(value));
        }
    }
    snapshot
}

fn demo_tessellation_graph(variant_id: &str) -> FeatureGraph {
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

fn collect_variant_snapshot(
    variant_id: &str,
    objective: &str,
    seed: u64,
    template: &MacStudioRackTemplate,
) -> RackVariantGeometrySnapshot {
    let demo_graph = demo_tessellation_graph(variant_id);
    let rebuild = evaluate_feature_graph_deterministic(&demo_graph)
        .expect("demo rack graph should evaluate deterministically");
    let feature_hashes = rebuild
        .records
        .iter()
        .map(|record| (record.feature_id.clone(), record.geometry_hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let feature_records = rebuild
        .records
        .iter()
        .map(|record| {
            let deps = if record.dependency_hashes.is_empty() {
                "none".to_string()
            } else {
                record.dependency_hashes.join(",")
            };
            format!(
                "{}|{}|deps={}|params={}|hash={}",
                record.feature_id,
                record.operation_key,
                deps,
                record.params_fingerprint,
                record.geometry_hash
            )
        })
        .collect::<Vec<_>>();
    let (mesh_payload, _mesh_receipt) =
        tessellate_rebuild_result(&demo_graph, &rebuild, 42, variant_id)
            .expect("rack template should tessellate deterministically");
    let mesh_encoded = mesh_payload
        .to_binary_payload()
        .expect("mesh payload should encode deterministically");

    RackVariantGeometrySnapshot {
        variant_id: variant_id.to_string(),
        objective: objective.to_string(),
        seed,
        rebuild_hash: rebuild.rebuild_hash,
        ordered_feature_ids: rebuild.ordered_feature_ids,
        feature_hashes,
        feature_records,
        mesh_hash: mesh_encoded.deterministic_hash,
        mesh_id: mesh_payload.mesh_id,
        mesh_vertex_count: mesh_payload.vertices.len(),
        mesh_triangle_count: mesh_payload.triangle_indices.len() / 3,
        mesh_edge_count: mesh_payload.edges.len(),
        mesh_bounds: format!(
            "min=[{:.3},{:.3},{:.3}] max=[{:.3},{:.3},{:.3}]",
            mesh_payload.bounds.min_mm[0],
            mesh_payload.bounds.min_mm[1],
            mesh_payload.bounds.min_mm[2],
            mesh_payload.bounds.max_mm[0],
            mesh_payload.bounds.max_mm[1],
            mesh_payload.bounds.max_mm[2],
        ),
        key_parameters: key_parameter_snapshot(&template.params),
        semantic_refs: template.semantic_refs.to_stable_ids(),
    }
}

fn collect_rack_geometry_snapshots() -> BTreeMap<String, RackVariantGeometrySnapshot> {
    let baseline_params = MacStudioRackTemplateParams::default();
    let baseline_template = generate_mac_studio_rack_template(&baseline_params)
        .expect("baseline rack template should generate");
    let mut snapshots = BTreeMap::new();
    snapshots.insert(
        "variant.baseline".to_string(),
        collect_variant_snapshot("variant.baseline", "baseline", 0, &baseline_template),
    );

    let objective_variants = generate_objective_variants(&baseline_params, OBJECTIVE_SEED)
        .expect("objective variants should generate deterministically");
    for variant in objective_variants {
        if !matches!(
            variant.variant_id.as_str(),
            "variant.lightweight" | "variant.low-cost" | "variant.stiffness"
        ) {
            continue;
        }
        snapshots.insert(
            variant.variant_id.clone(),
            collect_variant_snapshot(
                &variant.variant_id,
                objective_label(variant.objective),
                variant.seed,
                &variant.template,
            ),
        );
    }
    snapshots
}

fn semantic_geometry_diff(
    expected: &BTreeMap<String, RackVariantGeometrySnapshot>,
    actual: &BTreeMap<String, RackVariantGeometrySnapshot>,
) -> String {
    let mut lines = Vec::<String>::new();

    for variant in expected.keys() {
        if !actual.contains_key(variant) {
            lines.push(format!("missing variant snapshot: {variant}"));
        }
    }
    for variant in actual.keys() {
        if !expected.contains_key(variant) {
            lines.push(format!("unexpected variant snapshot: {variant}"));
        }
    }

    for (variant, expected_snapshot) in expected {
        let Some(actual_snapshot) = actual.get(variant) else {
            continue;
        };
        if expected_snapshot.rebuild_hash != actual_snapshot.rebuild_hash {
            lines.push(format!(
                "{variant} rebuild_hash expected={} actual={}",
                expected_snapshot.rebuild_hash, actual_snapshot.rebuild_hash
            ));
        }
        if expected_snapshot.mesh_hash != actual_snapshot.mesh_hash {
            lines.push(format!(
                "{variant} mesh_hash expected={} actual={}",
                expected_snapshot.mesh_hash, actual_snapshot.mesh_hash
            ));
        }
        if expected_snapshot.mesh_vertex_count != actual_snapshot.mesh_vertex_count {
            lines.push(format!(
                "{variant} mesh_vertex_count expected={} actual={}",
                expected_snapshot.mesh_vertex_count, actual_snapshot.mesh_vertex_count
            ));
        }
        if expected_snapshot.mesh_triangle_count != actual_snapshot.mesh_triangle_count {
            lines.push(format!(
                "{variant} mesh_triangle_count expected={} actual={}",
                expected_snapshot.mesh_triangle_count, actual_snapshot.mesh_triangle_count
            ));
        }
        if expected_snapshot.mesh_edge_count != actual_snapshot.mesh_edge_count {
            lines.push(format!(
                "{variant} mesh_edge_count expected={} actual={}",
                expected_snapshot.mesh_edge_count, actual_snapshot.mesh_edge_count
            ));
        }
        if expected_snapshot.mesh_bounds != actual_snapshot.mesh_bounds {
            lines.push(format!(
                "{variant} mesh_bounds expected={} actual={}",
                expected_snapshot.mesh_bounds, actual_snapshot.mesh_bounds
            ));
        }
        if expected_snapshot.ordered_feature_ids != actual_snapshot.ordered_feature_ids {
            lines.push(format!("{variant} ordered_feature_ids changed"));
        }
        diff_semantic_map(
            &mut lines,
            variant,
            "feature_hash",
            &expected_snapshot.feature_hashes,
            &actual_snapshot.feature_hashes,
        );
        diff_semantic_map(
            &mut lines,
            variant,
            "key_parameter",
            &expected_snapshot.key_parameters,
            &actual_snapshot.key_parameters,
        );
    }

    if lines.is_empty() {
        "no semantic geometry diff".to_string()
    } else {
        lines.join("\n")
    }
}

fn diff_semantic_map(
    lines: &mut Vec<String>,
    variant: &str,
    label: &str,
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) {
    let mut keys = BTreeSet::new();
    keys.extend(expected.keys().cloned());
    keys.extend(actual.keys().cloned());
    for key in keys {
        match (expected.get(&key), actual.get(&key)) {
            (Some(expected_value), Some(actual_value)) if expected_value != actual_value => {
                lines.push(format!(
                    "{variant} {label} {key} expected={} actual={}",
                    expected_value, actual_value
                ));
            }
            (Some(_), None) => lines.push(format!("{variant} {label} {key} missing in actual")),
            (None, Some(_)) => lines.push(format!("{variant} {label} {key} unexpected in actual")),
            _ => {}
        }
    }
}

#[test]
fn rack_geometry_snapshots_match_golden_fixture() {
    let actual = collect_rack_geometry_snapshots();
    let actual_json = serde_json::to_string_pretty(&actual)
        .expect("rack geometry snapshot should serialize deterministically");
    let path = fixture_path(GOLDEN_FILE);

    if std::env::var("CAD_UPDATE_GOLDENS").as_deref() == Ok("1") {
        std::fs::write(&path, &actual_json).expect("golden fixture should write");
        return;
    }

    let expected_json = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing golden fixture {path}: {error}\nset CAD_UPDATE_GOLDENS=1 to regenerate.\nactual snapshot:\n{actual_json}"
        )
    });
    let expected =
        serde_json::from_str::<BTreeMap<String, RackVariantGeometrySnapshot>>(&expected_json)
            .expect("golden fixture should parse");

    if expected != actual {
        let diff = semantic_geometry_diff(&expected, &actual);
        panic!(
            "rack geometry snapshot mismatch against {path}\nsemantic diff:\n{diff}\n\nactual snapshot:\n{actual_json}"
        );
    }
}

#[test]
fn rack_geometry_snapshot_diff_reports_variant_feature_context() {
    let expected = collect_rack_geometry_snapshots();
    let mut actual = expected.clone();
    let baseline = actual
        .get_mut("variant.baseline")
        .expect("baseline snapshot should exist");
    let feature_id = baseline
        .feature_hashes
        .keys()
        .next()
        .cloned()
        .expect("baseline feature hashes should not be empty");
    baseline
        .feature_hashes
        .insert(feature_id.clone(), "deadbeefdeadbeef".to_string());

    let diff = semantic_geometry_diff(&expected, &actual);
    assert!(diff.contains("variant.baseline"));
    assert!(diff.contains("feature_hash"));
    assert!(diff.contains(feature_id.as_str()));
    assert!(diff.contains("deadbeefdeadbeef"));
}
