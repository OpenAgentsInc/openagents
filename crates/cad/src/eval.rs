use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::kernel::CadKernelAdapter;
use crate::policy;
use crate::primitives::{PrimitiveSpec, build_primitives};
use crate::{CadError, CadResult};

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

/// Deterministic plan describing which feature nodes are invalidated by parameter edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterInvalidationPlan {
    pub changed_params: Vec<String>,
    pub directly_affected_features: Vec<String>,
    pub invalidated_feature_ids: Vec<String>,
    pub retained_feature_hashes: BTreeMap<String, String>,
}

/// Compute downstream invalidation from changed parameter names.
pub fn compute_parameter_invalidation_plan(
    graph: &FeatureGraph,
    changed_params: &BTreeSet<String>,
    previous_hashes: &BTreeMap<String, String>,
) -> CadResult<ParameterInvalidationPlan> {
    graph.validate()?;
    let ordered_feature_ids = graph.deterministic_topo_order()?;
    let node_by_id: BTreeMap<&str, &FeatureNode> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut downstream: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in &graph.nodes {
        downstream.entry(node.id.as_str()).or_default();
    }
    for node in &graph.nodes {
        for dep in &node.depends_on {
            downstream
                .entry(dep.as_str())
                .or_default()
                .push(node.id.as_str());
        }
    }
    for children in downstream.values_mut() {
        children.sort_unstable();
    }

    let mut directly_affected = BTreeSet::<String>::new();
    for feature_id in &ordered_feature_ids {
        let node = node_by_id
            .get(feature_id.as_str())
            .copied()
            .ok_or_else(|| CadError::EvalFailed {
                reason: format!("missing feature node {}", feature_id),
            })?;
        if node_uses_changed_params(node, changed_params) {
            directly_affected.insert(node.id.clone());
        }
    }

    let mut invalidated = BTreeSet::<String>::new();
    let mut queue = VecDeque::<String>::new();
    for feature_id in &directly_affected {
        invalidated.insert(feature_id.clone());
        queue.push_back(feature_id.clone());
    }

    while let Some(current) = queue.pop_front() {
        if let Some(children) = downstream.get(current.as_str()) {
            for child in children {
                if invalidated.insert((*child).to_string()) {
                    queue.push_back((*child).to_string());
                }
            }
        }
    }

    let invalidated_feature_ids = ordered_feature_ids
        .iter()
        .filter(|feature_id| invalidated.contains(*feature_id))
        .cloned()
        .collect::<Vec<_>>();

    let retained_feature_hashes = ordered_feature_ids
        .iter()
        .filter(|feature_id| !invalidated.contains(*feature_id))
        .filter_map(|feature_id| {
            previous_hashes
                .get(feature_id)
                .map(|hash| (feature_id.clone(), hash.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    Ok(ParameterInvalidationPlan {
        changed_params: changed_params.iter().cloned().collect(),
        directly_affected_features: directly_affected.into_iter().collect(),
        invalidated_feature_ids,
        retained_feature_hashes,
    })
}

/// Deterministic key for per-feature eval cache entries.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EvalCacheKey {
    pub document_revision: u64,
    pub feature_node_id: String,
    pub params_hash: String,
}

impl EvalCacheKey {
    pub fn from_feature_node(document_revision: u64, node: &FeatureNode) -> Self {
        Self {
            document_revision,
            feature_node_id: node.id.clone(),
            params_hash: feature_params_hash(node),
        }
    }
}

/// Cache entry for deterministic feature eval output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalCacheEntry {
    pub geometry_hash: String,
}

/// Cache observability counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EvalCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Deterministic in-memory eval cache with LRU eviction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalCacheStore {
    capacity: usize,
    entries: BTreeMap<EvalCacheKey, EvalCacheEntry>,
    lru_order: VecDeque<EvalCacheKey>,
    stats: EvalCacheStats,
}

impl EvalCacheStore {
    pub fn new(capacity: usize) -> CadResult<Self> {
        if capacity == 0 {
            return Err(CadError::InvalidPolicy {
                reason: "eval cache capacity must be > 0".to_string(),
            });
        }
        Ok(Self {
            capacity,
            entries: BTreeMap::new(),
            lru_order: VecDeque::new(),
            stats: EvalCacheStats::default(),
        })
    }

    pub fn get(&mut self, key: &EvalCacheKey) -> Option<&EvalCacheEntry> {
        if self.entries.contains_key(key) {
            self.stats.hits = self.stats.hits.saturating_add(1);
            self.touch_lru(key);
            return self.entries.get(key);
        }
        self.stats.misses = self.stats.misses.saturating_add(1);
        None
    }

    pub fn insert(&mut self, key: EvalCacheKey, entry: EvalCacheEntry) {
        let existed = self.entries.insert(key.clone(), entry).is_some();
        self.touch_lru(&key);
        if !existed && self.entries.len() > self.capacity {
            self.evict_one();
        }
    }

    pub fn stats(&self) -> EvalCacheStats {
        self.stats
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn touch_lru(&mut self, key: &EvalCacheKey) {
        if let Some(position) = self.lru_order.iter().position(|existing| existing == key) {
            let _ = self.lru_order.remove(position);
        }
        self.lru_order.push_back(key.clone());
    }

    fn evict_one(&mut self) {
        while let Some(candidate) = self.lru_order.pop_front() {
            if self.entries.remove(&candidate).is_some() {
                self.stats.evictions = self.stats.evictions.saturating_add(1);
                break;
            }
        }
    }
}

fn feature_params_hash(node: &FeatureNode) -> String {
    let payload = node
        .params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{:016x}", fnv1a64(payload.as_bytes()))
}

fn node_uses_changed_params(node: &FeatureNode, changed_params: &BTreeSet<String>) -> bool {
    if changed_params.is_empty() {
        return false;
    }
    node.params.values().any(|value| {
        changed_params.iter().any(|param| {
            value == param
                || value == &format!("${param}")
                || value
                    .split(|ch: char| {
                        !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
                    })
                    .any(|token| token == param)
        })
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
        EvalCacheEntry, EvalCacheKey, EvalCacheStats, EvalCacheStore, EvalPlan,
        compute_parameter_invalidation_plan, eval_tolerance_mm,
        evaluate_feature_graph_deterministic, evaluate_plan,
    };
    use crate::CadResult;
    use crate::feature_graph::{FeatureGraph, FeatureNode};
    use crate::kernel::CadKernelAdapter;
    use crate::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec};
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
        assert_eq!(eval_tolerance_mm(), crate::policy::BASE_TOLERANCE_MM);
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
        let second =
            compute_parameter_invalidation_plan(&graph, &changed, &baseline.feature_hashes)
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
}
