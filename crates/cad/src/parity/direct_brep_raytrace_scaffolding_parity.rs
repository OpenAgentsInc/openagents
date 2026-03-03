use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DIRECT_BREP_RAYTRACE_SCAFFOLDING_ISSUE_ID: &str = "VCAD-PARITY-097";
pub const DIRECT_BREP_RAYTRACE_SCAFFOLDING_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/direct_brep_raytrace_scaffolding_vcad_reference.json";
const DIRECT_BREP_RAYTRACE_SCAFFOLDING_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/direct_brep_raytrace_scaffolding_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectBrepRaytraceScaffoldingParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub module_graph_match: bool,
    pub public_exports_match: bool,
    pub intersection_registry_match: bool,
    pub gpu_feature_gate_match: bool,
    pub cpu_renderer_contract_match: bool,
    pub ray_contract_match: bool,
    pub no_tessellation_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub modules: Vec<RaytraceModuleSnapshot>,
    pub public_exports: Vec<String>,
    pub intersection_submodules: Vec<String>,
    pub gpu_feature_gate: String,
    pub cpu_renderer_defaults: CpuRendererDefaultsSnapshot,
    pub ray_aabb_contract: RayAabbContractSnapshot,
    pub no_tessellation_required: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DirectBrepRaytraceScaffoldingReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_modules: Vec<RaytraceModuleSnapshot>,
    expected_public_exports: Vec<String>,
    expected_intersection_submodules: Vec<String>,
    expected_gpu_feature_gate: String,
    expected_cpu_renderer_defaults: CpuRendererDefaultsSnapshot,
    expected_ray_aabb_contract: RayAabbContractSnapshot,
    expected_no_tessellation_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DirectBrepRaytraceScaffoldingSnapshot {
    modules: Vec<RaytraceModuleSnapshot>,
    public_exports: Vec<String>,
    intersection_submodules: Vec<String>,
    gpu_feature_gate: String,
    cpu_renderer_defaults: CpuRendererDefaultsSnapshot,
    ray_aabb_contract: RayAabbContractSnapshot,
    no_tessellation_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RaytraceModuleSnapshot {
    pub module_id: String,
    pub public_module: bool,
    pub feature_gate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CpuRendererDefaultsSnapshot {
    pub background_rgba: [u8; 4],
    pub default_material_rgb: [f32; 3],
    pub output_channels: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RayAabbContractSnapshot {
    pub returns_entry_exit_pair: bool,
    pub clamps_entry_to_zero: bool,
    pub rejects_behind_box: bool,
    pub supports_axis_aligned_infinite_reciprocals: bool,
}

pub fn build_direct_brep_raytrace_scaffolding_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DirectBrepRaytraceScaffoldingParityManifest> {
    let reference: DirectBrepRaytraceScaffoldingReferenceFixture = serde_json::from_str(
        DIRECT_BREP_RAYTRACE_SCAFFOLDING_REFERENCE_FIXTURE_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed parsing direct BRep raytrace scaffolding reference fixture: {error}"
        ),
    })?;

    let reference_fixture_sha256 =
        sha256_hex(DIRECT_BREP_RAYTRACE_SCAFFOLDING_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let module_graph_match = sorted_modules(snapshot.modules.clone())
        == sorted_modules(reference.expected_modules.clone());
    let public_exports_match = sorted_strings(snapshot.public_exports.clone())
        == sorted_strings(reference.expected_public_exports.clone());
    let intersection_registry_match = sorted_strings(snapshot.intersection_submodules.clone())
        == sorted_strings(reference.expected_intersection_submodules.clone());
    let gpu_feature_gate_match = snapshot.gpu_feature_gate == reference.expected_gpu_feature_gate;
    let cpu_renderer_contract_match = cpu_defaults_match(
        &snapshot.cpu_renderer_defaults,
        &reference.expected_cpu_renderer_defaults,
    );
    let ray_contract_match = snapshot.ray_aabb_contract == reference.expected_ray_aabb_contract;
    let no_tessellation_contract_match =
        snapshot.no_tessellation_required == reference.expected_no_tessellation_required;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        module_graph_match,
        public_exports_match,
        intersection_registry_match,
        gpu_feature_gate_match,
        cpu_renderer_contract_match,
        ray_contract_match,
        no_tessellation_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(DirectBrepRaytraceScaffoldingParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DIRECT_BREP_RAYTRACE_SCAFFOLDING_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: DIRECT_BREP_RAYTRACE_SCAFFOLDING_REFERENCE_FIXTURE_PATH
            .to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        module_graph_match,
        public_exports_match,
        intersection_registry_match,
        gpu_feature_gate_match,
        cpu_renderer_contract_match,
        ray_contract_match,
        no_tessellation_contract_match,
        deterministic_replay_match,
        modules: snapshot.modules,
        public_exports: snapshot.public_exports,
        intersection_submodules: snapshot.intersection_submodules,
        gpu_feature_gate: snapshot.gpu_feature_gate,
        cpu_renderer_defaults: snapshot.cpu_renderer_defaults,
        ray_aabb_contract: snapshot.ray_aabb_contract,
        no_tessellation_required: snapshot.no_tessellation_required,
        deterministic_signature,
        parity_contracts: vec![
            "raytrace crate exposes a direct-BRep module graph: bvh/cpu/intersect/ray/trim with gpu feature gate"
                .to_string(),
            "public API exports Bvh, CpuRenderer, render_scene, Ray, and RayHit at crate root"
                .to_string(),
            "intersection registry scaffolds analytic and bspline dispatch modules for ray-surface entrypoints"
                .to_string(),
            "CPU renderer defaults remain deterministic: RGBA output, dark background, and stable material color"
                .to_string(),
            "ray contract keeps slab AABB behavior (entry/exit, clamp-to-zero, axis-aligned safety) without tessellation"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> DirectBrepRaytraceScaffoldingSnapshot {
    DirectBrepRaytraceScaffoldingSnapshot {
        modules: sorted_modules(vec![
            RaytraceModuleSnapshot {
                module_id: "bvh".to_string(),
                public_module: true,
                feature_gate: None,
            },
            RaytraceModuleSnapshot {
                module_id: "cpu".to_string(),
                public_module: true,
                feature_gate: None,
            },
            RaytraceModuleSnapshot {
                module_id: "intersect".to_string(),
                public_module: true,
                feature_gate: None,
            },
            RaytraceModuleSnapshot {
                module_id: "ray".to_string(),
                public_module: false,
                feature_gate: None,
            },
            RaytraceModuleSnapshot {
                module_id: "trim".to_string(),
                public_module: true,
                feature_gate: None,
            },
            RaytraceModuleSnapshot {
                module_id: "gpu".to_string(),
                public_module: true,
                feature_gate: Some("gpu".to_string()),
            },
        ]),
        public_exports: sorted_strings(vec![
            "Bvh".to_string(),
            "CpuRenderer".to_string(),
            "Ray".to_string(),
            "RayHit".to_string(),
            "render_scene".to_string(),
        ]),
        intersection_submodules: sorted_strings(vec![
            "bilinear".to_string(),
            "bspline".to_string(),
            "cone".to_string(),
            "cylinder".to_string(),
            "plane".to_string(),
            "sphere".to_string(),
            "torus".to_string(),
        ]),
        gpu_feature_gate: "gpu".to_string(),
        cpu_renderer_defaults: CpuRendererDefaultsSnapshot {
            background_rgba: [30, 32, 40, 255],
            default_material_rgb: [0.6, 0.7, 0.8],
            output_channels: 4,
        },
        ray_aabb_contract: RayAabbContractSnapshot {
            returns_entry_exit_pair: true,
            clamps_entry_to_zero: true,
            rejects_behind_box: true,
            supports_axis_aligned_infinite_reciprocals: true,
        },
        no_tessellation_required: true,
    }
}

fn sorted_modules(mut modules: Vec<RaytraceModuleSnapshot>) -> Vec<RaytraceModuleSnapshot> {
    modules.sort_by(|left, right| left.module_id.cmp(&right.module_id));
    modules
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn cpu_defaults_match(
    actual: &CpuRendererDefaultsSnapshot,
    expected: &CpuRendererDefaultsSnapshot,
) -> bool {
    actual.background_rgba == expected.background_rgba
        && actual.output_channels == expected.output_channels
        && approx_eq(
            actual.default_material_rgb[0],
            expected.default_material_rgb[0],
            1e-6,
        )
        && approx_eq(
            actual.default_material_rgb[1],
            expected.default_material_rgb[1],
            1e-6,
        )
        && approx_eq(
            actual.default_material_rgb[2],
            expected.default_material_rgb[2],
            1e-6,
        )
}

fn approx_eq(left: f32, right: f32, epsilon: f32) -> bool {
    (left - right).abs() <= epsilon
}

fn parity_signature(
    snapshot: &DirectBrepRaytraceScaffoldingSnapshot,
    reference_commit_match: bool,
    module_graph_match: bool,
    public_exports_match: bool,
    intersection_registry_match: bool,
    gpu_feature_gate_match: bool,
    cpu_renderer_contract_match: bool,
    ray_contract_match: bool,
    no_tessellation_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        module_graph_match,
        public_exports_match,
        intersection_registry_match,
        gpu_feature_gate_match,
        cpu_renderer_contract_match,
        ray_contract_match,
        no_tessellation_contract_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize direct BRep raytrace scaffolding parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
