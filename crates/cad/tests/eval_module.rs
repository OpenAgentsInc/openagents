#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use openagents_cad::CadResult;
use openagents_cad::eval::{
    EvalCacheEntry, EvalCacheKey, EvalCacheStats, EvalCacheStore, EvalPlan,
    compute_parameter_invalidation_plan, eval_tolerance_mm, evaluate_feature_graph_deterministic,
    evaluate_plan,
};
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};
use openagents_cad::kernel::CadKernelAdapter;
use openagents_cad::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Default)]
struct MockKernel {
    calls: Vec<String>,
}

impl CadKernelAdapter for MockKernel {
    type Solid = String;

    fn create_box(&mut self, primitive: &BoxPrimitive) -> CadResult<Self::Solid> {
        self.calls.push(format!(
            "box:{:.1}:{:.1}:{:.1}",
            primitive.width_mm, primitive.depth_mm, primitive.height_mm
        ));
        Ok("solid-box".to_string())
    }

    fn create_cylinder(&mut self, primitive: &CylinderPrimitive) -> CadResult<Self::Solid> {
        self.calls.push(format!(
            "cylinder:{:.1}:{:.1}",
            primitive.radius_mm, primitive.height_mm
        ));
        Ok("solid-cylinder".to_string())
    }
}

#[test]
fn evaluate_plan_routes_all_primitives_through_kernel_adapter() {
    let mut kernel = MockKernel::default();
    let plan = EvalPlan {
        primitives: vec![
            PrimitiveSpec::Box(BoxPrimitive {
                width_mm: 10.0,
                depth_mm: 20.0,
                height_mm: 30.0,
            }),
            PrimitiveSpec::Cylinder(CylinderPrimitive {
                radius_mm: 5.0,
                height_mm: 12.0,
            }),
        ],
    };

    let result = evaluate_plan(&mut kernel, &plan);
    assert!(result.is_ok(), "plan eval should succeed");

    if let Ok(solids) = result {
        assert_eq!(
            solids,
            vec!["solid-box".to_string(), "solid-cylinder".to_string()]
        );
        assert_eq!(
            kernel.calls,
            vec![
                "box:10.0:20.0:30.0".to_string(),
                "cylinder:5.0:12.0".to_string()
            ]
        );
    }
}

#[test]
fn eval_uses_policy_default_tolerance() {
    assert_eq!(
        eval_tolerance_mm(),
        openagents_cad::policy::BASE_TOLERANCE_MM
    );
}

fn node(id: &str, op: &str, depends_on: &[&str], params: &[(&str, &str)]) -> FeatureNode {
    FeatureNode {
        id: id.to_string(),
        name: id.to_string(),
        operation_key: op.to_string(),
        depends_on: depends_on
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        params: params
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

#[test]
fn deterministic_rebuild_is_stable_across_runs_and_insertion_order() {
    let graph_a = FeatureGraph {
        nodes: vec![
            node(
                "feature.vent_pattern",
                "linear.pattern.v1",
                &["feature.hole"],
                &[],
            ),
            node(
                "feature.base",
                "primitive.box.v1",
                &[],
                &[
                    ("depth_mm", "200"),
                    ("height_mm", "80"),
                    ("width_mm", "120"),
                ],
            ),
            node(
                "feature.hole",
                "cut.hole.v1",
                &["feature.base"],
                &[("depth_mm", "12"), ("radius_mm", "4")],
            ),
        ],
    };

    let graph_b = FeatureGraph {
        nodes: vec![
            node(
                "feature.base",
                "primitive.box.v1",
                &[],
                &[
                    ("height_mm", "80"),
                    ("width_mm", "120"),
                    ("depth_mm", "200"),
                ],
            ),
            node(
                "feature.hole",
                "cut.hole.v1",
                &["feature.base"],
                &[("radius_mm", "4"), ("depth_mm", "12")],
            ),
            node(
                "feature.vent_pattern",
                "linear.pattern.v1",
                &["feature.hole"],
                &[],
            ),
        ],
    };

    let first = evaluate_feature_graph_deterministic(&graph_a).expect("first rebuild");
    let second = evaluate_feature_graph_deterministic(&graph_a).expect("second rebuild");
    let reorder = evaluate_feature_graph_deterministic(&graph_b).expect("reordered rebuild");

    assert_eq!(first, second, "repeat rebuild should be identical");
    assert_eq!(
        first.rebuild_hash, reorder.rebuild_hash,
        "insertion order must not affect rebuild hash"
    );
    assert_eq!(first.records, reorder.records);
}

#[test]
fn deterministic_rebuild_receipt_is_stable_and_complete() {
    let graph = FeatureGraph {
        nodes: vec![
            node("feature.base", "primitive.box.v1", &[], &[]),
            node(
                "feature.fillet_marker",
                "fillet.placeholder.v1",
                &["feature.base"],
                &[("kind", "fillet"), ("radius_param", "fillet_radius_mm")],
            ),
        ],
    };
    let result = evaluate_feature_graph_deterministic(&graph).expect("rebuild should succeed");
    let receipt = result.receipt();
    assert_eq!(
        receipt.ordered_feature_ids,
        vec![
            "feature.base".to_string(),
            "feature.fillet_marker".to_string()
        ]
    );
    assert_eq!(receipt.feature_count, 2);
    assert_eq!(receipt.rebuild_hash, result.rebuild_hash);
    assert_eq!(receipt.vcad_eval_timing.parse_ms, None);
    assert_eq!(receipt.vcad_eval_timing.serialize_ms, None);
    assert_eq!(receipt.vcad_eval_timing.clash_ms, 0.0);
    assert_eq!(receipt.vcad_eval_timing.assembly_ms, 0.0);
    assert_eq!(receipt.vcad_eval_timing.nodes.len(), 2);
    assert_eq!(
        receipt
            .vcad_eval_timing
            .nodes
            .get("feature.base")
            .expect("base timing should exist")
            .op,
        "primitive.box.v1".to_string()
    );
    assert_eq!(
        receipt
            .vcad_eval_timing
            .nodes
            .get("feature.fillet_marker")
            .expect("fillet timing should exist")
            .op,
        "fillet.placeholder.v1".to_string()
    );
    let node_sum = receipt
        .vcad_eval_timing
        .nodes
        .values()
        .fold(0.0, |sum, node| sum + node.eval_ms + node.mesh_ms);
    let expected_total = (node_sum * 1_000.0).round() / 1_000.0;
    assert_eq!(receipt.vcad_eval_timing.total_ms, expected_total);
}

#[test]
fn deterministic_rebuild_receipt_vcad_timing_is_order_stable() {
    let graph_a = FeatureGraph {
        nodes: vec![
            node(
                "feature.top",
                "linear.pattern.v1",
                &["feature.root"],
                &[("count_param", "count")],
            ),
            node("feature.root", "primitive.box.v1", &[], &[("w", "100")]),
        ],
    };
    let graph_b = FeatureGraph {
        nodes: vec![
            node("feature.root", "primitive.box.v1", &[], &[("w", "100")]),
            node(
                "feature.top",
                "linear.pattern.v1",
                &["feature.root"],
                &[("count_param", "count")],
            ),
        ],
    };

    let receipt_a = evaluate_feature_graph_deterministic(&graph_a)
        .expect("graph a rebuild should succeed")
        .receipt();
    let receipt_b = evaluate_feature_graph_deterministic(&graph_b)
        .expect("graph b rebuild should succeed")
        .receipt();
    assert_eq!(receipt_a.vcad_eval_timing, receipt_b.vcad_eval_timing);
}

#[test]
fn parameter_invalidation_prunes_to_affected_downstream_features() {
    let graph = FeatureGraph {
        nodes: vec![
            node(
                "feature.base",
                "primitive.box.v1",
                &[],
                &[
                    ("width_param", "width_mm"),
                    ("depth_param", "depth_mm"),
                    ("height_param", "height_mm"),
                ],
            ),
            node(
                "feature.hole",
                "cut.hole.v1",
                &["feature.base"],
                &[
                    ("radius_param", "hole_radius_mm"),
                    ("depth_param", "hole_depth_mm"),
                ],
            ),
            node(
                "feature.vent_pattern",
                "linear.pattern.v1",
                &["feature.hole"],
                &[
                    ("count_param", "vent_count"),
                    ("spacing_param", "vent_spacing_mm"),
                ],
            ),
            node(
                "feature.fillet_marker",
                "fillet.placeholder.v1",
                &["feature.base"],
                &[("radius_param", "fillet_radius_mm"), ("kind", "fillet")],
            ),
        ],
    };
    let baseline = evaluate_feature_graph_deterministic(&graph).expect("baseline rebuild");
    let changed = BTreeSet::from(["hole_radius_mm".to_string()]);
    let plan = compute_parameter_invalidation_plan(&graph, &changed, &baseline.feature_hashes)
        .expect("invalidation should compute");
    assert_eq!(plan.changed_params, vec!["hole_radius_mm".to_string()]);
    assert_eq!(
        plan.directly_affected_features,
        vec!["feature.hole".to_string()]
    );
    assert_eq!(
        plan.invalidated_feature_ids,
        vec![
            "feature.hole".to_string(),
            "feature.vent_pattern".to_string()
        ]
    );
    assert_eq!(
        plan.retained_feature_hashes.get("feature.base"),
        baseline.feature_hashes.get("feature.base")
    );
    assert_eq!(
        plan.retained_feature_hashes.get("feature.fillet_marker"),
        baseline.feature_hashes.get("feature.fillet_marker")
    );
}

#[test]
fn parameter_invalidation_is_deterministic_and_keeps_upstream_hashes() {
    let graph = FeatureGraph {
        nodes: vec![
            node(
                "feature.base",
                "primitive.box.v1",
                &[],
                &[("width_param", "width_mm"), ("height_param", "height_mm")],
            ),
            node(
                "feature.mount_hole",
                "cut.hole.v1",
                &["feature.base"],
                &[
                    ("radius_param", "hole_radius_mm"),
                    ("depth_param", "hole_depth_mm"),
                ],
            ),
        ],
    };
    let baseline = evaluate_feature_graph_deterministic(&graph).expect("baseline rebuild");
    let changed = BTreeSet::from(["width_mm".to_string()]);

    let first = compute_parameter_invalidation_plan(&graph, &changed, &baseline.feature_hashes)
        .expect("first plan");
    let second = compute_parameter_invalidation_plan(&graph, &changed, &baseline.feature_hashes)
        .expect("second plan");
    assert_eq!(first, second);
    assert_eq!(
        first.invalidated_feature_ids,
        vec!["feature.base".to_string(), "feature.mount_hole".to_string()]
    );
    assert!(
        first.retained_feature_hashes.is_empty(),
        "all features are downstream of changed root"
    );
}

#[test]
fn eval_cache_key_is_deterministic_for_same_params_regardless_of_insertion_order() {
    let node_a = node(
        "feature.base",
        "primitive.box.v1",
        &[],
        &[("height_param", "height_mm"), ("width_param", "width_mm")],
    );
    let node_b = node(
        "feature.base",
        "primitive.box.v1",
        &[],
        &[("width_param", "width_mm"), ("height_param", "height_mm")],
    );
    let key_a = EvalCacheKey::from_feature_node(7, &node_a);
    let key_b = EvalCacheKey::from_feature_node(7, &node_b);
    assert_eq!(key_a, key_b);
}

#[test]
fn eval_cache_tracks_hits_misses_and_returns_entries() {
    let node = node(
        "feature.base",
        "primitive.box.v1",
        &[],
        &[("width_param", "width_mm")],
    );
    let key = EvalCacheKey::from_feature_node(10, &node);
    let mut cache = EvalCacheStore::new(2).expect("cache should initialize");

    assert!(
        cache.get(&key).is_none(),
        "first lookup should be a miss for empty cache"
    );
    cache.insert(
        key.clone(),
        EvalCacheEntry {
            geometry_hash: "hash-base".to_string(),
        },
    );
    let entry = cache.get(&key).expect("entry should be present on hit");
    assert_eq!(entry.geometry_hash, "hash-base");
    assert_eq!(
        cache.stats(),
        EvalCacheStats {
            hits: 1,
            misses: 1,
            evictions: 0
        }
    );
}

#[test]
fn eval_cache_evicts_least_recently_used_entry_when_capacity_exceeded() {
    let node_a = node("feature.a", "primitive.box.v1", &[], &[("w", "wa")]);
    let node_b = node("feature.b", "primitive.box.v1", &[], &[("w", "wb")]);
    let node_c = node("feature.c", "primitive.box.v1", &[], &[("w", "wc")]);
    let key_a = EvalCacheKey::from_feature_node(1, &node_a);
    let key_b = EvalCacheKey::from_feature_node(1, &node_b);
    let key_c = EvalCacheKey::from_feature_node(1, &node_c);

    let mut cache = EvalCacheStore::new(2).expect("cache should initialize");
    cache.insert(
        key_a.clone(),
        EvalCacheEntry {
            geometry_hash: "hash-a".to_string(),
        },
    );
    cache.insert(
        key_b.clone(),
        EvalCacheEntry {
            geometry_hash: "hash-b".to_string(),
        },
    );
    let _ = cache.get(&key_a);
    cache.insert(
        key_c.clone(),
        EvalCacheEntry {
            geometry_hash: "hash-c".to_string(),
        },
    );

    assert_eq!(cache.len(), 2);
    assert!(
        cache.get(&key_b).is_none(),
        "least-recently-used key should be evicted"
    );
    assert!(
        cache.get(&key_a).is_some(),
        "recently touched key must be retained"
    );
    assert!(cache.get(&key_c).is_some(), "new key must be retained");
    let stats = cache.stats();
    assert_eq!(stats.evictions, 1);
    assert!(
        stats.hits >= 3 && stats.misses >= 1,
        "hit/miss counters should include the probe and validations"
    );
}
