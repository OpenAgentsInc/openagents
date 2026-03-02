use super::primitives::evaluate_cut_hole_feature_with_boolean;
use super::{
    BoxFeatureOp, CircularPatternFeatureOp, CutHoleFeatureOp, CylinderFeatureOp,
    FilletPlaceholderFeatureOp, FilletPlaceholderKind, LinearPatternFeatureOp, TransformFeatureOp,
    compose_transform_sequence, evaluate_box_feature, evaluate_circular_pattern_feature,
    evaluate_cut_hole_feature, evaluate_cylinder_feature, evaluate_fillet_placeholder_feature,
    evaluate_linear_pattern_feature, evaluate_transform_feature,
};
use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::kernel::CadKernelAdapter;
use crate::keys::feature_params as feature_keys;
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::{CadError, CadResult};
use std::collections::BTreeMap;

#[derive(Default)]
struct MockKernel {
    box_calls: usize,
    cylinder_calls: usize,
}

impl CadKernelAdapter for MockKernel {
    type Solid = &'static str;

    fn create_box(
        &mut self,
        _primitive: &crate::primitives::BoxPrimitive,
    ) -> CadResult<Self::Solid> {
        self.box_calls = self.box_calls.saturating_add(1);
        Ok("solid-box")
    }

    fn create_cylinder(
        &mut self,
        _primitive: &crate::primitives::CylinderPrimitive,
    ) -> CadResult<Self::Solid> {
        self.cylinder_calls = self.cylinder_calls.saturating_add(1);
        Ok("solid-cylinder")
    }
}

fn params() -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "width_mm",
            ScalarValue {
                value: 120.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("width should set");
    params
        .set(
            "depth_mm",
            ScalarValue {
                value: 200.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("depth should set");
    params
        .set(
            "height_mm",
            ScalarValue {
                value: 80.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("height should set");
    params
}

fn golden_hashes() -> BTreeMap<String, String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{root}/tests/goldens/feature_box_geometry_hashes.json");
    let payload = std::fs::read_to_string(&path).expect("feature hash fixture should be readable");
    serde_json::from_str::<BTreeMap<String, String>>(&payload)
        .expect("feature hash fixture should parse")
}

fn cylinder_golden_hashes() -> BTreeMap<String, String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{root}/tests/goldens/feature_cylinder_geometry_hashes.json");
    let payload =
        std::fs::read_to_string(&path).expect("cylinder feature hash fixture should be readable");
    serde_json::from_str::<BTreeMap<String, String>>(&payload)
        .expect("cylinder feature hash fixture should parse")
}

#[test]
fn box_feature_op_resolves_and_calls_kernel() {
    let mut kernel = MockKernel::default();
    let op = BoxFeatureOp {
        feature_id: "feature.base".to_string(),
        width_param: "width_mm".to_string(),
        depth_param: "depth_mm".to_string(),
        height_param: "height_mm".to_string(),
    };
    let result = evaluate_box_feature(&mut kernel, &op, &params());
    assert!(result.is_ok(), "box feature eval should succeed");
    assert_eq!(kernel.box_calls, 1);
    let result = result.expect("result should be present");
    assert_eq!(result.feature_id, "feature.base");
    assert_eq!(result.solid, "solid-box");
    let golden = golden_hashes();
    assert_eq!(
        result.geometry_hash,
        golden["feature.base|120.000000|200.000000|80.000000"]
    );
}

#[test]
fn box_feature_rejects_invalid_param_units() {
    let mut params = params();
    params
        .set(
            "height_mm",
            ScalarValue {
                value: 80.0,
                unit: ScalarUnit::Degree,
            },
        )
        .expect("override should set");

    let op = BoxFeatureOp {
        feature_id: "feature.base".to_string(),
        width_param: "width_mm".to_string(),
        depth_param: "depth_mm".to_string(),
        height_param: "height_mm".to_string(),
    };

    let mut kernel = MockKernel::default();
    let result = evaluate_box_feature(&mut kernel, &op, &params);
    assert!(result.is_err(), "unit mismatch must be rejected");
    assert_eq!(kernel.box_calls, 0);
    assert_eq!(kernel.cylinder_calls, 0);
}

#[test]
fn geometry_hash_matches_second_representative_fixture() {
    let mut params = ParameterStore::default();
    params
        .set(
            "w",
            ScalarValue {
                value: 180.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("w should set");
    params
        .set(
            "d",
            ScalarValue {
                value: 210.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("d should set");
    params
        .set(
            "h",
            ScalarValue {
                value: 95.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("h should set");

    let op = BoxFeatureOp {
        feature_id: "feature.rack".to_string(),
        width_param: "w".to_string(),
        depth_param: "d".to_string(),
        height_param: "h".to_string(),
    };
    let primitive = op
        .resolve_primitive(&params)
        .expect("primitive should resolve");
    let hash = op.geometry_hash(&primitive);
    let golden = golden_hashes();
    assert_eq!(hash, golden["feature.rack|180.000000|210.000000|95.000000"]);
}

#[test]
fn cylinder_feature_op_resolves_and_calls_kernel() {
    let mut params = ParameterStore::default();
    params
        .set(
            "radius_mm",
            ScalarValue {
                value: 6.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    params
        .set(
            "height_mm",
            ScalarValue {
                value: 40.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("height should set");

    let op = CylinderFeatureOp {
        feature_id: "feature.mount_post".to_string(),
        radius_param: "radius_mm".to_string(),
        height_param: "height_mm".to_string(),
    };
    let mut kernel = MockKernel::default();
    let result = evaluate_cylinder_feature(&mut kernel, &op, &params)
        .expect("cylinder feature eval should succeed");
    assert_eq!(kernel.cylinder_calls, 1);
    assert_eq!(result.feature_id, "feature.mount_post");
    assert_eq!(result.solid, "solid-cylinder");
    let golden = cylinder_golden_hashes();
    assert_eq!(
        result.geometry_hash,
        golden["feature.mount_post|6.000000|40.000000"]
    );
}

#[test]
fn cylinder_feature_rejects_tolerance_edge_case_radius() {
    let mut params = ParameterStore::default();
    params
        .set(
            "radius_mm",
            ScalarValue {
                value: crate::policy::BASE_TOLERANCE_MM,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    params
        .set(
            "height_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("height should set");

    let op = CylinderFeatureOp {
        feature_id: "feature.edge_case".to_string(),
        radius_param: "radius_mm".to_string(),
        height_param: "height_mm".to_string(),
    };
    let mut kernel = MockKernel::default();
    let result = evaluate_cylinder_feature(&mut kernel, &op, &params);
    assert!(result.is_err(), "radius at tolerance must be rejected");
    assert_eq!(kernel.cylinder_calls, 0);
}

#[test]
fn cylinder_geometry_hash_matches_second_representative_fixture() {
    let mut params = ParameterStore::default();
    params
        .set(
            "r",
            ScalarValue {
                value: 3.2,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("r should set");
    params
        .set(
            "h",
            ScalarValue {
                value: 18.5,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("h should set");

    let op = CylinderFeatureOp {
        feature_id: "feature.vent_tube".to_string(),
        radius_param: "r".to_string(),
        height_param: "h".to_string(),
    };
    let primitive = op
        .resolve_primitive(&params)
        .expect("primitive should resolve");
    let hash = op.geometry_hash(&primitive);
    let golden = cylinder_golden_hashes();
    assert_eq!(hash, golden["feature.vent_tube|3.200000|18.500000"]);
}

#[test]
fn transform_feature_rejects_invalid_scale() {
    let op = TransformFeatureOp {
        feature_id: "feature.transform".to_string(),
        source_feature_id: "feature.base".to_string(),
        translation_mm: [0.0, 0.0, 0.0],
        rotation_deg_xyz: [0.0, 0.0, 0.0],
        scale_xyz: [0.0, 1.0, 1.0],
    };
    let result = evaluate_transform_feature(&op, "abc123");
    assert!(result.is_err(), "zero scale must be rejected");
}

#[test]
fn transform_sequence_is_deterministic_for_same_order() {
    let ops = vec![
        TransformFeatureOp {
            feature_id: "feature.move".to_string(),
            source_feature_id: "feature.base".to_string(),
            translation_mm: [12.0, 0.0, -4.0],
            rotation_deg_xyz: [0.0, 45.0, 0.0],
            scale_xyz: [1.0, 1.0, 1.0],
        },
        TransformFeatureOp {
            feature_id: "feature.scale".to_string(),
            source_feature_id: "feature.move".to_string(),
            translation_mm: [0.0, 0.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 0.0],
            scale_xyz: [1.2, 1.2, 1.2],
        },
    ];

    let first = compose_transform_sequence(&ops).expect("compose should succeed");
    let second = compose_transform_sequence(&ops).expect("compose should succeed");
    assert_eq!(first, second);
}

#[test]
fn transform_sequence_output_depends_on_order() {
    let a = TransformFeatureOp {
        feature_id: "feature.a".to_string(),
        source_feature_id: "feature.base".to_string(),
        translation_mm: [10.0, 0.0, 0.0],
        rotation_deg_xyz: [0.0, 0.0, 90.0],
        scale_xyz: [1.0, 1.0, 1.0],
    };
    let b = TransformFeatureOp {
        feature_id: "feature.b".to_string(),
        source_feature_id: "feature.a".to_string(),
        translation_mm: [0.0, 5.0, 0.0],
        rotation_deg_xyz: [0.0, 0.0, 0.0],
        scale_xyz: [2.0, 1.0, 1.0],
    };

    let ab = compose_transform_sequence(&[a.clone(), b.clone()]).expect("ab compose");
    let ba = compose_transform_sequence(&[b, a]).expect("ba compose");
    assert_ne!(ab, ba, "transform composition must preserve input order");
}

#[test]
fn cut_hole_returns_structured_failure_when_boolean_unavailable() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_radius_mm",
            ScalarValue {
                value: 3.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    params
        .set(
            "hole_depth_mm",
            ScalarValue {
                value: 12.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("depth should set");

    let op = CutHoleFeatureOp {
        feature_id: "feature.hole_001".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "hole_radius_mm".to_string(),
        depth_param: "hole_depth_mm".to_string(),
        tolerance_mm: None,
    };
    let mut kernel = MockKernel::default();
    let result = evaluate_cut_hole_feature(&mut kernel, &"solid-base", "hash-base", &op, &params);
    assert!(
        result.is_err(),
        "missing boolean backend must return structured failure"
    );
    let error = result.expect_err("error should be present");
    assert!(
        matches!(error, CadError::EvalFailed { .. }),
        "cut/hole failures must map to EvalFailed"
    );
}

#[test]
fn cut_hole_returns_valid_result_when_boolean_succeeds() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_radius_mm",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    params
        .set(
            "hole_depth_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("depth should set");
    let op = CutHoleFeatureOp {
        feature_id: "feature.hole_002".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "hole_radius_mm".to_string(),
        depth_param: "hole_depth_mm".to_string(),
        tolerance_mm: Some(0.001),
    };
    let mut kernel = MockKernel::default();
    let result = evaluate_cut_hole_feature_with_boolean(
        &mut kernel,
        &"solid-base",
        "hash-base",
        &op,
        &params,
        |_kernel, _left, _right, _tol| Ok("solid-with-hole"),
    )
    .expect("cut/hole should succeed");
    assert_eq!(result.feature_id, "feature.hole_002");
    assert_eq!(result.solid, "solid-with-hole");
    assert!(!result.geometry_hash.is_empty());
}

#[test]
fn linear_pattern_indices_are_stable_across_rebuilds() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_count",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "hole_spacing_mm",
            ScalarValue {
                value: 18.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("spacing should set");

    let op = LinearPatternFeatureOp {
        feature_id: "feature.vents".to_string(),
        source_feature_id: "feature.hole_002".to_string(),
        count_param: "hole_count".to_string(),
        spacing_param: "hole_spacing_mm".to_string(),
        direction_unit_xyz: [1.0, 0.0, 0.0],
        start_index: 100,
    };

    let first =
        evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");
    let second =
        evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");

    let first_indexes: Vec<u32> = first
        .instances
        .iter()
        .map(|entry| entry.pattern_index)
        .collect();
    let second_indexes: Vec<u32> = second
        .instances
        .iter()
        .map(|entry| entry.pattern_index)
        .collect();
    assert_eq!(first_indexes, vec![100, 101, 102, 103]);
    assert_eq!(first_indexes, second_indexes);
    assert_eq!(first.instances, second.instances);
    assert_eq!(first.pattern_hash, second.pattern_hash);
}

#[test]
fn linear_pattern_rejects_non_integer_count() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_count",
            ScalarValue {
                value: 2.5,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "hole_spacing_mm",
            ScalarValue {
                value: 15.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("spacing should set");
    let op = LinearPatternFeatureOp {
        feature_id: "feature.vents".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "hole_count".to_string(),
        spacing_param: "hole_spacing_mm".to_string(),
        direction_unit_xyz: [1.0, 0.0, 0.0],
        start_index: 0,
    };
    let error = evaluate_linear_pattern_feature(&op, &params, "hash-hole")
        .expect_err("fractional count must be rejected");
    assert!(matches!(error, CadError::InvalidParameter { .. }));
}

#[test]
fn linear_pattern_rejects_zero_direction_vector() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_count",
            ScalarValue {
                value: 3.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "hole_spacing_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("spacing should set");
    let op = LinearPatternFeatureOp {
        feature_id: "feature.vents".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "hole_count".to_string(),
        spacing_param: "hole_spacing_mm".to_string(),
        direction_unit_xyz: [0.0, 0.0, 0.0],
        start_index: 0,
    };
    let error = evaluate_linear_pattern_feature(&op, &params, "hash-hole")
        .expect_err("zero direction must be rejected");
    assert!(matches!(error, CadError::InvalidPrimitive { .. }));
}

#[test]
fn linear_pattern_preserves_prefix_indices_when_count_increases() {
    let mut params = ParameterStore::default();
    params
        .set(
            "hole_count",
            ScalarValue {
                value: 3.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "hole_spacing_mm",
            ScalarValue {
                value: 22.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("spacing should set");
    let op = LinearPatternFeatureOp {
        feature_id: "feature.vents".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "hole_count".to_string(),
        spacing_param: "hole_spacing_mm".to_string(),
        direction_unit_xyz: [0.0, 1.0, 0.0],
        start_index: 10,
    };
    let baseline =
        evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("baseline eval");
    params
        .set(
            "hole_count",
            ScalarValue {
                value: 5.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count update should set");
    let expanded =
        evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("expanded eval");
    assert_eq!(
        &expanded.instances[..baseline.instances.len()],
        baseline.instances.as_slice()
    );
}

#[test]
fn circular_pattern_positions_are_stable_and_match_quadrants() {
    let mut params = ParameterStore::default();
    params
        .set(
            "bolt_count",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "bolt_span_deg",
            ScalarValue {
                value: 360.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("span should set");
    params
        .set(
            "bolt_radius_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");

    let op = CircularPatternFeatureOp {
        feature_id: "feature.bolt_circle".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "bolt_count".to_string(),
        angle_deg_param: "bolt_span_deg".to_string(),
        radius_param: "bolt_radius_mm".to_string(),
        axis_origin_mm: [0.0, 0.0, 0.0],
        axis_direction_xyz: [0.0, 0.0, 1.0],
        start_index: 50,
    };

    let first =
        evaluate_circular_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");
    let second =
        evaluate_circular_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");
    assert_eq!(first, second);
    assert_eq!(first.instances.len(), 4);
    assert_eq!(first.instances[0].pattern_index, 50);
    assert_eq!(first.instances[1].pattern_index, 51);
    assert_eq!(first.instances[2].pattern_index, 52);
    assert_eq!(first.instances[3].pattern_index, 53);

    let expected = [
        [0.0, 10.0, 0.0],
        [-10.0, 0.0, 0.0],
        [0.0, -10.0, 0.0],
        [10.0, 0.0, 0.0],
    ];
    for (instance, expected_center) in first.instances.iter().zip(expected.iter()) {
        assert!(
            (instance.center_mm[0] - expected_center[0]).abs() < 1e-9
                && (instance.center_mm[1] - expected_center[1]).abs() < 1e-9
                && (instance.center_mm[2] - expected_center[2]).abs() < 1e-9,
            "center mismatch: {:?} vs {:?}",
            instance.center_mm,
            expected_center
        );
    }
}

#[test]
fn circular_pattern_rejects_non_integer_count() {
    let mut params = ParameterStore::default();
    params
        .set(
            "bolt_count",
            ScalarValue {
                value: 3.5,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "bolt_span_deg",
            ScalarValue {
                value: 360.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("span should set");
    params
        .set(
            "bolt_radius_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    let op = CircularPatternFeatureOp {
        feature_id: "feature.bolt_circle".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "bolt_count".to_string(),
        angle_deg_param: "bolt_span_deg".to_string(),
        radius_param: "bolt_radius_mm".to_string(),
        axis_origin_mm: [0.0, 0.0, 0.0],
        axis_direction_xyz: [0.0, 0.0, 1.0],
        start_index: 0,
    };
    let error = evaluate_circular_pattern_feature(&op, &params, "hash-hole")
        .expect_err("fractional count must be rejected");
    assert!(matches!(error, CadError::InvalidParameter { .. }));
}

#[test]
fn circular_pattern_rejects_zero_axis_direction() {
    let mut params = ParameterStore::default();
    params
        .set(
            "bolt_count",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "bolt_span_deg",
            ScalarValue {
                value: 360.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("span should set");
    params
        .set(
            "bolt_radius_mm",
            ScalarValue {
                value: 10.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    let op = CircularPatternFeatureOp {
        feature_id: "feature.bolt_circle".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "bolt_count".to_string(),
        angle_deg_param: "bolt_span_deg".to_string(),
        radius_param: "bolt_radius_mm".to_string(),
        axis_origin_mm: [0.0, 0.0, 0.0],
        axis_direction_xyz: [0.0, 0.0, 0.0],
        start_index: 0,
    };
    let error = evaluate_circular_pattern_feature(&op, &params, "hash-hole")
        .expect_err("zero axis must be rejected");
    assert!(matches!(error, CadError::InvalidPrimitive { .. }));
}

#[test]
fn circular_pattern_rejects_negative_radius() {
    let mut params = ParameterStore::default();
    params
        .set(
            "bolt_count",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("count should set");
    params
        .set(
            "bolt_span_deg",
            ScalarValue {
                value: 360.0,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("span should set");
    params
        .set(
            "bolt_radius_mm",
            ScalarValue {
                value: -1.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    let op = CircularPatternFeatureOp {
        feature_id: "feature.bolt_circle".to_string(),
        source_feature_id: "feature.hole".to_string(),
        count_param: "bolt_count".to_string(),
        angle_deg_param: "bolt_span_deg".to_string(),
        radius_param: "bolt_radius_mm".to_string(),
        axis_origin_mm: [0.0, 0.0, 0.0],
        axis_direction_xyz: [0.0, 0.0, 1.0],
        start_index: 0,
    };
    let error = evaluate_circular_pattern_feature(&op, &params, "hash-hole")
        .expect_err("negative radius must be rejected");
    assert!(matches!(error, CadError::InvalidParameter { .. }));
}

#[test]
fn fillet_placeholder_survives_save_load_and_rebuild() {
    let op = FilletPlaceholderFeatureOp {
        feature_id: "feature.fillet_marker".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "fillet_radius_mm".to_string(),
        kind: FilletPlaceholderKind::Fillet,
    };
    let node = op
        .to_feature_node()
        .expect("feature node conversion should succeed");
    let graph = FeatureGraph {
        nodes: vec![
            FeatureNode {
                id: "feature.base".to_string(),
                name: "base".to_string(),
                operation_key: "primitive.box.v1".to_string(),
                depends_on: vec![],
                params: BTreeMap::new(),
            },
            node,
        ],
    };
    let payload = serde_json::to_string(&graph).expect("graph should serialize");
    let parsed: FeatureGraph = serde_json::from_str(&payload).expect("graph should parse");
    let restored_marker = parsed
        .nodes
        .iter()
        .find(|entry| entry.operation_key == super::FILLET_PLACEHOLDER_OPERATION_KEY)
        .expect("marker node should be retained");
    let restored_op =
        FilletPlaceholderFeatureOp::from_feature_node(restored_marker).expect("restore op");
    assert_eq!(op, restored_op);

    let mut params = ParameterStore::default();
    params
        .set(
            "fillet_radius_mm",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");

    let first = evaluate_fillet_placeholder_feature(&restored_op, &params, "hash-base")
        .expect("first rebuild");
    let second = evaluate_fillet_placeholder_feature(&restored_op, &params, "hash-base")
        .expect("second rebuild");
    assert_eq!(first, second, "placeholder rebuild must be deterministic");
    assert_eq!(first.passthrough_source_hash, "hash-base");
}

#[test]
fn fillet_placeholder_rejects_invalid_radius() {
    let op = FilletPlaceholderFeatureOp {
        feature_id: "feature.chamfer_marker".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "chamfer_radius_mm".to_string(),
        kind: FilletPlaceholderKind::Chamfer,
    };
    let mut params = ParameterStore::default();
    params
        .set(
            "chamfer_radius_mm",
            ScalarValue {
                value: 0.0,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("radius should set");
    let error = evaluate_fillet_placeholder_feature(&op, &params, "hash-base")
        .expect_err("zero radius must fail");
    assert!(matches!(error, CadError::InvalidParameter { .. }));
}

#[test]
fn fillet_placeholder_from_node_rejects_wrong_operation_key() {
    let node = FeatureNode {
        id: "feature.not_marker".to_string(),
        name: "bad".to_string(),
        operation_key: "primitive.box.v1".to_string(),
        depends_on: vec!["feature.base".to_string()],
        params: BTreeMap::from([
            (feature_keys::KIND.owned(), "fillet".to_string()),
            (feature_keys::RADIUS_PARAM.owned(), "radius_mm".to_string()),
        ]),
    };
    let error = FilletPlaceholderFeatureOp::from_feature_node(&node).expect_err("wrong key fails");
    assert!(matches!(error, CadError::InvalidPrimitive { .. }));
}
