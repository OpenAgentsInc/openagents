use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_BVH_BUILD_TRAVERSE_ISSUE_ID: &str = "VCAD-PARITY-100";
pub const BVH_BUILD_TRAVERSE_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/bvh_build_traverse_vcad_reference.json";
const BVH_BUILD_TRAVERSE_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/bvh_build_traverse_vcad_reference.json");

const LEAF_FACE_THRESHOLD: usize = 4;
const SAH_NUM_BUCKETS: usize = 12;
const SAH_TRAVERSAL_COST: f64 = 0.125;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BvhBuildTraverseParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub sah_constants_match: bool,
    pub leaf_partition_match: bool,
    pub trace_ordering_match: bool,
    pub closest_hit_match: bool,
    pub flatten_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub primary_leaf_sizes: Vec<usize>,
    pub fallback_leaf_sizes: Vec<usize>,
    pub trace_t_values: Vec<f64>,
    pub closest_t: Option<f64>,
    pub flatten_node_count: usize,
    pub flatten_leaf_node_count: usize,
    pub flatten_face_count: usize,
    pub fallback_triggered: bool,
    pub sah_num_buckets: usize,
    pub sah_traversal_cost: f64,
    pub leaf_face_threshold: usize,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BvhBuildTraverseReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_primary_leaf_sizes: Vec<usize>,
    expected_fallback_leaf_sizes: Vec<usize>,
    expected_trace_t_values: Vec<f64>,
    expected_closest_t: f64,
    expected_flatten_node_count: usize,
    expected_flatten_leaf_node_count: usize,
    expected_flatten_face_count: usize,
    expected_num_buckets: usize,
    expected_traversal_cost: f64,
    expected_leaf_threshold: usize,
    expected_fallback_triggered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BvhBuildTraverseSnapshot {
    primary_leaf_sizes: Vec<usize>,
    fallback_leaf_sizes: Vec<usize>,
    trace_t_values: Vec<f64>,
    closest_t: Option<f64>,
    flatten_node_count: usize,
    flatten_leaf_node_count: usize,
    flatten_face_count: usize,
    fallback_triggered: bool,
    sah_num_buckets: usize,
    sah_traversal_cost: f64,
    leaf_face_threshold: usize,
}

#[derive(Debug, Clone, Copy)]
struct Ray3 {
    origin: [f64; 3],
    inv_direction: [f64; 3],
    sign: [usize; 3],
}

#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: [f64; 3],
    max: [f64; 3],
}

#[derive(Debug, Clone)]
struct FaceProxy {
    face_id: u32,
    aabb: Aabb,
    centroid: [f64; 3],
}

#[derive(Debug, Clone)]
enum Node {
    Leaf {
        aabb: Aabb,
        faces: Vec<u32>,
    },
    Internal {
        aabb: Aabb,
        left: Box<Node>,
        right: Box<Node>,
    },
}

type FlatNode = (Aabb, bool, u32, u32);

pub fn build_bvh_build_traverse_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<BvhBuildTraverseParityManifest> {
    let reference: BvhBuildTraverseReferenceFixture =
        serde_json::from_str(BVH_BUILD_TRAVERSE_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing BVH build/traverse reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 = sha256_hex(BVH_BUILD_TRAVERSE_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let sah_constants_match = snapshot.sah_num_buckets == reference.expected_num_buckets
        && approx_eq(
            snapshot.sah_traversal_cost,
            reference.expected_traversal_cost,
            1e-12,
        )
        && snapshot.leaf_face_threshold == reference.expected_leaf_threshold;

    let leaf_partition_match = sorted_usize(snapshot.primary_leaf_sizes.clone())
        == sorted_usize(reference.expected_primary_leaf_sizes.clone())
        && sorted_usize(snapshot.fallback_leaf_sizes.clone())
            == sorted_usize(reference.expected_fallback_leaf_sizes.clone())
        && snapshot.fallback_triggered == reference.expected_fallback_triggered;

    let trace_ordering_match = approx_vec(
        &snapshot.trace_t_values,
        &reference.expected_trace_t_values,
        1e-9,
    ) && snapshot
        .trace_t_values
        .windows(2)
        .all(|pair| pair[0] <= pair[1] + 1e-9);

    let closest_hit_match = snapshot
        .closest_t
        .is_some_and(|t| approx_eq(t, reference.expected_closest_t, 1e-9));

    let flatten_contract_match = snapshot.flatten_node_count
        == reference.expected_flatten_node_count
        && snapshot.flatten_leaf_node_count == reference.expected_flatten_leaf_node_count
        && snapshot.flatten_face_count == reference.expected_flatten_face_count;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        sah_constants_match,
        leaf_partition_match,
        trace_ordering_match,
        closest_hit_match,
        flatten_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(BvhBuildTraverseParityManifest {
        manifest_version: 1,
        issue_id: PARITY_BVH_BUILD_TRAVERSE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: BVH_BUILD_TRAVERSE_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        sah_constants_match,
        leaf_partition_match,
        trace_ordering_match,
        closest_hit_match,
        flatten_contract_match,
        deterministic_replay_match,
        primary_leaf_sizes: snapshot.primary_leaf_sizes,
        fallback_leaf_sizes: snapshot.fallback_leaf_sizes,
        trace_t_values: snapshot.trace_t_values,
        closest_t: snapshot.closest_t,
        flatten_node_count: snapshot.flatten_node_count,
        flatten_leaf_node_count: snapshot.flatten_leaf_node_count,
        flatten_face_count: snapshot.flatten_face_count,
        fallback_triggered: snapshot.fallback_triggered,
        sah_num_buckets: snapshot.sah_num_buckets,
        sah_traversal_cost: snapshot.sah_traversal_cost,
        leaf_face_threshold: snapshot.leaf_face_threshold,
        deterministic_signature,
        parity_contracts: vec![
            "BVH construction keeps leaf threshold at 4 faces and uses SAH buckets/traversal constants"
                .to_string(),
            "partition failure falls back to deterministic midpoint split to avoid empty child nodes"
                .to_string(),
            "trace returns hits sorted by ascending t after traversal"
                .to_string(),
            "trace_closest uses nearest-AABB child ordering with early-out on current closest t"
                .to_string(),
            "flatten contract preserves node metadata and leaf face-count accounting for GPU upload"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> BvhBuildTraverseSnapshot {
    let primary_faces = make_primary_faces();
    let (primary_root, _) = build_tree(primary_faces.clone());
    let primary_leaf_sizes = leaf_sizes(&primary_root);

    let mut face_hits = BTreeMap::new();
    for face_id in 0u32..8u32 {
        face_hits.insert(face_id, vec![face_id as f64 + 1.0]);
    }

    let ray = Ray3::new([-1.0, 0.5, 0.5], [1.0, 0.0, 0.0]);
    let trace_t_values = trace(&primary_root, &ray, &face_hits)
        .into_iter()
        .map(canonical_f64)
        .collect::<Vec<_>>();
    let closest_t = trace_closest(&primary_root, &ray, &face_hits).map(canonical_f64);

    let (flat_nodes, flat_faces) = flatten(&primary_root);
    let flatten_node_count = flat_nodes.len();
    let flatten_leaf_node_count = flat_nodes.iter().filter(|node| node.1).count();
    let flatten_face_count = flat_faces.len();

    let fallback_faces = make_fallback_faces();
    let (fallback_root, fallback_triggered) = build_tree(fallback_faces);
    let fallback_leaf_sizes = leaf_sizes(&fallback_root);

    BvhBuildTraverseSnapshot {
        primary_leaf_sizes,
        fallback_leaf_sizes,
        trace_t_values,
        closest_t,
        flatten_node_count,
        flatten_leaf_node_count,
        flatten_face_count,
        fallback_triggered,
        sah_num_buckets: SAH_NUM_BUCKETS,
        sah_traversal_cost: SAH_TRAVERSAL_COST,
        leaf_face_threshold: LEAF_FACE_THRESHOLD,
    }
}

fn make_primary_faces() -> Vec<FaceProxy> {
    (0u32..8u32)
        .map(|face_id| {
            let min_x = face_id as f64 * 2.0;
            let max_x = min_x + 1.0;
            let aabb = Aabb {
                min: [min_x, 0.0, 0.0],
                max: [max_x, 1.0, 1.0],
            };
            FaceProxy {
                face_id,
                centroid: [
                    (aabb.min[0] + aabb.max[0]) / 2.0,
                    (aabb.min[1] + aabb.max[1]) / 2.0,
                    (aabb.min[2] + aabb.max[2]) / 2.0,
                ],
                aabb,
            }
        })
        .collect()
}

fn make_fallback_faces() -> Vec<FaceProxy> {
    (100u32..106u32)
        .map(|face_id| {
            let aabb = Aabb {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 1.0, 1.0],
            };
            FaceProxy {
                face_id,
                centroid: [0.5, 0.5, 0.5],
                aabb,
            }
        })
        .collect()
}

fn build_tree(mut faces: Vec<FaceProxy>) -> (Node, bool) {
    let mut fallback_triggered = false;
    let root = build_node(faces.as_mut_slice(), &mut fallback_triggered);
    (root, fallback_triggered)
}

fn build_node(faces: &mut [FaceProxy], fallback_triggered: &mut bool) -> Node {
    let bounds = bounds_for_faces(faces);

    if faces.len() <= LEAF_FACE_THRESHOLD {
        return Node::Leaf {
            aabb: bounds,
            faces: faces.iter().map(|face| face.face_id).collect(),
        };
    }

    let (best_axis, best_pos) = find_best_split(faces, bounds);
    let mid = partition_faces(faces, best_axis, best_pos);

    if mid == 0 || mid == faces.len() {
        *fallback_triggered = true;
        let fallback_mid = faces.len() / 2;
        let (left, right) = faces.split_at_mut(fallback_mid);
        return Node::Internal {
            aabb: bounds,
            left: Box::new(build_node(left, fallback_triggered)),
            right: Box::new(build_node(right, fallback_triggered)),
        };
    }

    let (left, right) = faces.split_at_mut(mid);
    Node::Internal {
        aabb: bounds,
        left: Box::new(build_node(left, fallback_triggered)),
        right: Box::new(build_node(right, fallback_triggered)),
    }
}

fn bounds_for_faces(faces: &[FaceProxy]) -> Aabb {
    let mut bounds = Aabb::empty();
    for face in faces {
        bounds.include_point(face.aabb.min);
        bounds.include_point(face.aabb.max);
    }
    bounds
}

fn find_best_split(faces: &[FaceProxy], bounds: Aabb) -> (usize, f64) {
    let extent = [
        bounds.max[0] - bounds.min[0],
        bounds.max[1] - bounds.min[1],
        bounds.max[2] - bounds.min[2],
    ];

    let mut best_cost = f64::INFINITY;
    let mut best_axis = 0usize;
    let mut best_pos = 0.0;

    for axis in 0..3usize {
        if extent[axis] < 1e-10 {
            continue;
        }

        let axis_min = bounds.min[axis];
        let mut bucket_counts = [0usize; SAH_NUM_BUCKETS];
        let mut bucket_bounds = [Aabb::empty(); SAH_NUM_BUCKETS];

        for face in faces {
            let c = face.centroid[axis];
            let mut bucket = ((c - axis_min) / extent[axis] * SAH_NUM_BUCKETS as f64) as usize;
            bucket = bucket.min(SAH_NUM_BUCKETS - 1);

            bucket_counts[bucket] += 1;
            bucket_bounds[bucket].include_point(face.aabb.min);
            bucket_bounds[bucket].include_point(face.aabb.max);
        }

        for split in 1..SAH_NUM_BUCKETS {
            let mut left_count = 0usize;
            let mut left_bounds = Aabb::empty();
            for index in 0..split {
                left_count += bucket_counts[index];
                if bucket_counts[index] > 0 {
                    left_bounds.include_point(bucket_bounds[index].min);
                    left_bounds.include_point(bucket_bounds[index].max);
                }
            }

            let mut right_count = 0usize;
            let mut right_bounds = Aabb::empty();
            for index in split..SAH_NUM_BUCKETS {
                right_count += bucket_counts[index];
                if bucket_counts[index] > 0 {
                    right_bounds.include_point(bucket_bounds[index].min);
                    right_bounds.include_point(bucket_bounds[index].max);
                }
            }

            if left_count == 0 || right_count == 0 {
                continue;
            }

            let total_area = surface_area(bounds);
            let cost = SAH_TRAVERSAL_COST
                + surface_area(left_bounds) / total_area * left_count as f64
                + surface_area(right_bounds) / total_area * right_count as f64;
            if cost < best_cost {
                best_cost = cost;
                best_axis = axis;
                best_pos = axis_min + (split as f64 / SAH_NUM_BUCKETS as f64) * extent[axis];
            }
        }
    }

    (best_axis, best_pos)
}

fn partition_faces(faces: &mut [FaceProxy], axis: usize, pos: f64) -> usize {
    let mut left = 0usize;
    let mut right = faces.len();

    while left < right {
        if faces[left].centroid[axis] < pos {
            left += 1;
        } else {
            right -= 1;
            faces.swap(left, right);
        }
    }

    left
}

fn trace(node: &Node, ray: &Ray3, face_hits: &BTreeMap<u32, Vec<f64>>) -> Vec<f64> {
    let mut hits = Vec::new();
    trace_node(node, ray, face_hits, &mut hits);
    hits.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    hits
}

fn trace_node(node: &Node, ray: &Ray3, face_hits: &BTreeMap<u32, Vec<f64>>, hits: &mut Vec<f64>) {
    match node {
        Node::Leaf { aabb, faces } => {
            if ray.intersect_aabb(*aabb).is_some() {
                for face_id in faces {
                    if let Some(face_ts) = face_hits.get(face_id) {
                        hits.extend(face_ts.iter().copied().filter(|t| *t >= 0.0));
                    }
                }
            }
        }
        Node::Internal { aabb, left, right } => {
            if ray.intersect_aabb(*aabb).is_some() {
                trace_node(left, ray, face_hits, hits);
                trace_node(right, ray, face_hits, hits);
            }
        }
    }
}

fn trace_closest(node: &Node, ray: &Ray3, face_hits: &BTreeMap<u32, Vec<f64>>) -> Option<f64> {
    let mut closest_t = f64::INFINITY;
    let mut closest = None;
    trace_node_closest(node, ray, face_hits, &mut closest, &mut closest_t);
    closest
}

fn trace_node_closest(
    node: &Node,
    ray: &Ray3,
    face_hits: &BTreeMap<u32, Vec<f64>>,
    closest: &mut Option<f64>,
    closest_t: &mut f64,
) {
    match node {
        Node::Leaf { aabb, faces } => {
            if let Some((t_min, _)) = ray.intersect_aabb(*aabb) {
                if t_min >= *closest_t {
                    return;
                }
                for face_id in faces {
                    if let Some(face_ts) = face_hits.get(face_id)
                        && let Some(candidate) = face_ts
                            .iter()
                            .copied()
                            .filter(|value| *value >= 0.0)
                            .min_by(|left, right| {
                                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                            })
                        && candidate < *closest_t
                    {
                        *closest_t = candidate;
                        *closest = Some(candidate);
                    }
                }
            }
        }
        Node::Internal { aabb, left, right } => {
            if let Some((t_min, _)) = ray.intersect_aabb(*aabb) {
                if t_min >= *closest_t {
                    return;
                }

                let left_t = ray.intersect_aabb(node_aabb(left)).map(|(t, _)| t);
                let right_t = ray.intersect_aabb(node_aabb(right)).map(|(t, _)| t);

                match (left_t, right_t) {
                    (Some(lt), Some(rt)) => {
                        if lt < rt {
                            trace_node_closest(left, ray, face_hits, closest, closest_t);
                            trace_node_closest(right, ray, face_hits, closest, closest_t);
                        } else {
                            trace_node_closest(right, ray, face_hits, closest, closest_t);
                            trace_node_closest(left, ray, face_hits, closest, closest_t);
                        }
                    }
                    (Some(_), None) => trace_node_closest(left, ray, face_hits, closest, closest_t),
                    (None, Some(_)) => {
                        trace_node_closest(right, ray, face_hits, closest, closest_t)
                    }
                    (None, None) => {}
                }
            }
        }
    }
}

fn flatten(root: &Node) -> (Vec<FlatNode>, Vec<u32>) {
    let mut nodes = Vec::new();
    let mut faces = Vec::new();
    flatten_node(root, &mut nodes, &mut faces);
    (nodes, faces)
}

fn flatten_node(node: &Node, nodes: &mut Vec<FlatNode>, faces: &mut Vec<u32>) -> usize {
    let index = nodes.len();
    match node {
        Node::Leaf {
            aabb,
            faces: leaf_faces,
        } => {
            let start = faces.len() as u32;
            let count = leaf_faces.len() as u32;
            faces.extend(leaf_faces.iter().copied());
            nodes.push((*aabb, true, start, count));
        }
        Node::Internal { aabb, left, right } => {
            nodes.push((*aabb, false, 0, 0));
            let left_idx = flatten_node(left, nodes, faces);
            let right_idx = flatten_node(right, nodes, faces);
            nodes[index].2 = left_idx as u32;
            nodes[index].3 = right_idx as u32;
        }
    }
    index
}

fn leaf_sizes(node: &Node) -> Vec<usize> {
    let mut sizes = Vec::new();
    collect_leaf_sizes(node, &mut sizes);
    sizes.sort();
    sizes
}

fn collect_leaf_sizes(node: &Node, out: &mut Vec<usize>) {
    match node {
        Node::Leaf { faces, .. } => out.push(faces.len()),
        Node::Internal { left, right, .. } => {
            collect_leaf_sizes(left, out);
            collect_leaf_sizes(right, out);
        }
    }
}

fn node_aabb(node: &Node) -> Aabb {
    match node {
        Node::Leaf { aabb, .. } => *aabb,
        Node::Internal { aabb, .. } => *aabb,
    }
}

impl Aabb {
    const fn empty() -> Self {
        Self {
            min: [f64::INFINITY, f64::INFINITY, f64::INFINITY],
            max: [f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
        }
    }

    fn include_point(&mut self, point: [f64; 3]) {
        for axis in 0..3 {
            if point[axis] < self.min[axis] {
                self.min[axis] = point[axis];
            }
            if point[axis] > self.max[axis] {
                self.max[axis] = point[axis];
            }
        }
    }
}

impl Ray3 {
    fn new(origin: [f64; 3], direction: [f64; 3]) -> Self {
        let len = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt();
        let direction = if len <= 1e-12 {
            [1.0, 0.0, 0.0]
        } else {
            [direction[0] / len, direction[1] / len, direction[2] / len]
        };
        let inv_direction = [1.0 / direction[0], 1.0 / direction[1], 1.0 / direction[2]];
        let sign = [
            if inv_direction[0] < 0.0 { 1 } else { 0 },
            if inv_direction[1] < 0.0 { 1 } else { 0 },
            if inv_direction[2] < 0.0 { 1 } else { 0 },
        ];
        Self {
            origin,
            inv_direction,
            sign,
        }
    }

    fn intersect_aabb(&self, aabb: Aabb) -> Option<(f64, f64)> {
        let bounds = [aabb.min, aabb.max];

        let tx1 = (bounds[self.sign[0]][0] - self.origin[0]) * self.inv_direction[0];
        let tx2 = (bounds[1 - self.sign[0]][0] - self.origin[0]) * self.inv_direction[0];

        let mut t_min = tx1;
        let mut t_max = tx2;

        let ty1 = (bounds[self.sign[1]][1] - self.origin[1]) * self.inv_direction[1];
        let ty2 = (bounds[1 - self.sign[1]][1] - self.origin[1]) * self.inv_direction[1];
        t_min = t_min.max(ty1);
        t_max = t_max.min(ty2);

        let tz1 = (bounds[self.sign[2]][2] - self.origin[2]) * self.inv_direction[2];
        let tz2 = (bounds[1 - self.sign[2]][2] - self.origin[2]) * self.inv_direction[2];
        t_min = t_min.max(tz1);
        t_max = t_max.min(tz2);

        if t_max >= t_min && t_max >= 0.0 {
            Some((t_min.max(0.0), t_max))
        } else {
            None
        }
    }
}

fn surface_area(aabb: Aabb) -> f64 {
    let dx = aabb.max[0] - aabb.min[0];
    let dy = aabb.max[1] - aabb.min[1];
    let dz = aabb.max[2] - aabb.min[2];
    2.0 * (dx * dy + dy * dz + dz * dx)
}

fn sorted_usize(mut values: Vec<usize>) -> Vec<usize> {
    values.sort();
    values
}

fn approx_vec(left: &[f64], right: &[f64], epsilon: f64) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(a, b)| approx_eq(*a, *b, epsilon))
}

fn canonical_f64(value: f64) -> f64 {
    let rounded = (value * 1_000_000_000.0).round() / 1_000_000_000.0;
    if rounded.abs() < 1e-12 { 0.0 } else { rounded }
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &BvhBuildTraverseSnapshot,
    reference_commit_match: bool,
    sah_constants_match: bool,
    leaf_partition_match: bool,
    trace_ordering_match: bool,
    closest_hit_match: bool,
    flatten_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        sah_constants_match,
        leaf_partition_match,
        trace_ordering_match,
        closest_hit_match,
        flatten_contract_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize BVH build/traverse parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
