use std::collections::BTreeMap;

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::{CadError, CadResult};
use crate::kernel::CadKernelAdapter;
use crate::policy;
use crate::primitives::{PrimitiveSpec, build_primitives};

/// Minimal eval plan for early adapter-boundary validation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvalPlan {
    pub primitives: Vec<PrimitiveSpec>,
}

/// Resolve the default evaluation tolerance in canonical units.
pub fn eval_tolerance_mm() -> f64 {
    policy::resolve_tolerance_mm(None)
}

/// Evaluate the plan by routing all primitive creation through the kernel adapter.
pub fn evaluate_plan<K: CadKernelAdapter>(
    kernel: &mut K,
    plan: &EvalPlan,
) -> CadResult<Vec<K::Solid>> {
    let _effective_tolerance_mm = eval_tolerance_mm();
    build_primitives(kernel, &plan.primitives)
}

/// Deterministic per-feature rebuild record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureRebuildRecord {
    pub feature_id: String,
    pub operation_key: String,
    pub dependency_hashes: Vec<String>,
    pub params_fingerprint: String,
    pub geometry_hash: String,
}

/// Deterministic rebuild output for a feature graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicRebuildResult {
    pub ordered_feature_ids: Vec<String>,
    pub feature_hashes: BTreeMap<String, String>,
    pub records: Vec<FeatureRebuildRecord>,
    pub rebuild_hash: String,
}

/// Deterministic rebuild receipt for observability/logging.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeterministicRebuildReceipt {
    pub ordered_feature_ids: Vec<String>,
    pub rebuild_hash: String,
    pub feature_count: usize,
}

impl DeterministicRebuildResult {
    pub fn receipt(&self) -> DeterministicRebuildReceipt {
        DeterministicRebuildReceipt {
            ordered_feature_ids: self.ordered_feature_ids.clone(),
            rebuild_hash: self.rebuild_hash.clone(),
            feature_count: self.records.len(),
        }
    }
}

/// Evaluate a feature graph in stable topological order and emit reproducible hashes.
pub fn evaluate_feature_graph_deterministic(
    graph: &FeatureGraph,
) -> CadResult<DeterministicRebuildResult> {
    let ordered_feature_ids = graph.deterministic_topo_order()?;
    let node_by_id: BTreeMap<&str, &FeatureNode> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut feature_hashes = BTreeMap::<String, String>::new();
    let mut records = Vec::<FeatureRebuildRecord>::with_capacity(ordered_feature_ids.len());

    for feature_id in &ordered_feature_ids {
        let node = node_by_id
            .get(feature_id.as_str())
            .copied()
            .ok_or_else(|| CadError::EvalFailed {
                reason: format!(
                    "deterministic rebuild missing node lookup for feature {}",
                    feature_id
                ),
            })?;

        let mut dep_ids = node.depends_on.clone();
        dep_ids.sort();
        let mut dependency_hashes = Vec::with_capacity(dep_ids.len());
        for dep_id in dep_ids {
            let dep_hash =
                feature_hashes
                    .get(&dep_id)
                    .cloned()
                    .ok_or_else(|| CadError::EvalFailed {
                        reason: format!(
                            "deterministic rebuild missing dependency hash for {} -> {}",
                            node.id, dep_id
                        ),
                    })?;
            dependency_hashes.push(format!("{dep_id}:{dep_hash}"));
        }

        let params_fingerprint = node
            .params
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");

        let payload = format!(
            "feature|id={}|op={}|deps={}|params={}",
            node.id,
            node.operation_key,
            dependency_hashes.join(","),
            params_fingerprint
        );
        let geometry_hash = format!("{:016x}", fnv1a64(payload.as_bytes()));
        feature_hashes.insert(node.id.clone(), geometry_hash.clone());
        records.push(FeatureRebuildRecord {
            feature_id: node.id.clone(),
            operation_key: node.operation_key.clone(),
            dependency_hashes,
            params_fingerprint,
            geometry_hash,
        });
    }

    let rebuild_payload = records
        .iter()
        .map(|entry| format!("{}:{}", entry.feature_id, entry.geometry_hash))
        .collect::<Vec<_>>()
        .join(",");
    let rebuild_hash = format!("{:016x}", fnv1a64(rebuild_payload.as_bytes()));

    Ok(DeterministicRebuildResult {
        ordered_feature_ids,
        feature_hashes,
        records,
        rebuild_hash,
    })
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{
        EvalPlan, eval_tolerance_mm, evaluate_feature_graph_deterministic, evaluate_plan,
    };
    use crate::CadResult;
    use crate::feature_graph::{FeatureGraph, FeatureNode};
    use crate::kernel::CadKernelAdapter;
    use crate::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec};
    use std::collections::BTreeMap;

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
        assert_eq!(eval_tolerance_mm(), crate::policy::BASE_TOLERANCE_MM);
    }

    fn node(id: &str, op: &str, depends_on: &[&str], params: &[(&str, &str)]) -> FeatureNode {
        FeatureNode {
            id: id.to_string(),
            name: id.to_string(),
            operation_key: op.to_string(),
            depends_on: depends_on.iter().map(|value| (*value).to_string()).collect(),
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
                node("feature.vent_pattern", "linear.pattern.v1", &["feature.hole"], &[]),
                node(
                    "feature.base",
                    "primitive.box.v1",
                    &[],
                    &[("depth_mm", "200"), ("height_mm", "80"), ("width_mm", "120")],
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
                    &[("height_mm", "80"), ("width_mm", "120"), ("depth_mm", "200")],
                ),
                node(
                    "feature.hole",
                    "cut.hole.v1",
                    &["feature.base"],
                    &[("radius_mm", "4"), ("depth_mm", "12")],
                ),
                node("feature.vent_pattern", "linear.pattern.v1", &["feature.hole"], &[]),
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
    }
}
