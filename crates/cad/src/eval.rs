use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::hash::stable_hex_digest;
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
#[derive(Clone, Debug, PartialEq)]
pub struct DeterministicRebuildReceipt {
    pub ordered_feature_ids: Vec<String>,
    pub rebuild_hash: String,
    pub feature_count: usize,
    pub vcad_eval_timing: VcadEvalTimingReceipt,
}

/// vcad-eval style per-node timing payload.
#[derive(Clone, Debug, PartialEq)]
pub struct VcadEvalNodeTiming {
    pub op: String,
    pub eval_ms: f64,
    pub mesh_ms: f64,
}

/// vcad-eval style timing envelope used by deterministic rebuild receipts.
#[derive(Clone, Debug, PartialEq)]
pub struct VcadEvalTimingReceipt {
    pub total_ms: f64,
    pub parse_ms: Option<f64>,
    pub serialize_ms: Option<f64>,
    pub tessellate_ms: f64,
    pub clash_ms: f64,
    pub assembly_ms: f64,
    pub nodes: BTreeMap<String, VcadEvalNodeTiming>,
}

impl DeterministicRebuildResult {
    pub fn receipt(&self) -> DeterministicRebuildReceipt {
        DeterministicRebuildReceipt {
            ordered_feature_ids: self.ordered_feature_ids.clone(),
            rebuild_hash: self.rebuild_hash.clone(),
            feature_count: self.records.len(),
            vcad_eval_timing: build_vcad_eval_timing(&self.records),
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
        let geometry_hash = stable_hex_digest(payload.as_bytes());
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
    let rebuild_hash = stable_hex_digest(rebuild_payload.as_bytes());

    Ok(DeterministicRebuildResult {
        ordered_feature_ids,
        feature_hashes,
        records,
        rebuild_hash,
    })
}

fn build_vcad_eval_timing(records: &[FeatureRebuildRecord]) -> VcadEvalTimingReceipt {
    let mut nodes = BTreeMap::new();
    let mut eval_total_ms = 0.0;
    let mut tessellate_ms = 0.0;

    for record in records {
        let eval_ms = deterministic_eval_ms(record);
        let mesh_ms = deterministic_mesh_ms(record);
        eval_total_ms += eval_ms;
        tessellate_ms += mesh_ms;
        nodes.insert(
            record.feature_id.clone(),
            VcadEvalNodeTiming {
                op: record.operation_key.clone(),
                eval_ms,
                mesh_ms,
            },
        );
    }

    let clash_ms = 0.0;
    let assembly_ms = 0.0;
    let total_ms = round_ms(eval_total_ms + tessellate_ms + clash_ms + assembly_ms);

    VcadEvalTimingReceipt {
        total_ms,
        parse_ms: None,
        serialize_ms: None,
        tessellate_ms: round_ms(tessellate_ms),
        clash_ms,
        assembly_ms,
        nodes,
    }
}

fn deterministic_eval_ms(record: &FeatureRebuildRecord) -> f64 {
    let dependency_weight = record.dependency_hashes.len() as f64 * 0.17;
    let parameter_count = if record.params_fingerprint.is_empty() {
        1
    } else {
        record.params_fingerprint.split(',').count()
    };
    let parameter_weight = parameter_count as f64 * 0.11;
    let seed = stable_timing_seed(&record.geometry_hash);
    let jitter = (seed % 11) as f64 * 0.01;
    round_ms(0.23 + dependency_weight + parameter_weight + jitter)
}

fn deterministic_mesh_ms(record: &FeatureRebuildRecord) -> f64 {
    let operation_weight = record.operation_key.len().min(32) as f64 * 0.01;
    let seed = stable_timing_seed(&record.feature_id);
    let jitter = (seed % 7) as f64 * 0.005;
    round_ms(0.08 + operation_weight + jitter)
}

fn stable_timing_seed(input: &str) -> u64 {
    input
        .chars()
        .take(16)
        .filter_map(|ch| ch.to_digit(16))
        .fold(0_u64, |seed, value| (seed << 4) | u64::from(value))
}

fn round_ms(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
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
    stable_hex_digest(payload.as_bytes())
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
