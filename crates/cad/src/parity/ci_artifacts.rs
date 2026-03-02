use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;

pub const PARITY_CI_ARTIFACTS_ISSUE_ID: &str = "VCAD-PARITY-008";

pub const PARITY_CI_SOURCE_PATHS: [&str; 76] = [
    "crates/cad/parity/vcad_reference_manifest.json",
    "crates/cad/parity/openagents_start_manifest.json",
    "crates/cad/parity/vcad_capabilities_inventory.json",
    "crates/cad/parity/openagents_capabilities_inventory.json",
    "crates/cad/parity/vcad_openagents_gap_matrix.json",
    "crates/cad/parity/parity_scorecard.json",
    "crates/cad/parity/parity_risk_register.json",
    "crates/cad/parity/parity_dashboard.json",
    "crates/cad/parity/kernel_adapter_v2_manifest.json",
    "crates/cad/parity/kernel_math_parity_manifest.json",
    "crates/cad/parity/kernel_topology_parity_manifest.json",
    "crates/cad/parity/kernel_geom_parity_manifest.json",
    "crates/cad/parity/kernel_primitives_parity_manifest.json",
    "crates/cad/parity/kernel_tessellate_parity_manifest.json",
    "crates/cad/parity/kernel_booleans_parity_manifest.json",
    "crates/cad/parity/kernel_boolean_diagnostics_parity_manifest.json",
    "crates/cad/parity/kernel_boolean_brep_parity_manifest.json",
    "crates/cad/parity/kernel_nurbs_parity_manifest.json",
    "crates/cad/parity/kernel_text_parity_manifest.json",
    "crates/cad/parity/kernel_fillet_parity_manifest.json",
    "crates/cad/parity/kernel_shell_parity_manifest.json",
    "crates/cad/parity/kernel_step_parity_manifest.json",
    "crates/cad/parity/kernel_precision_parity_manifest.json",
    "crates/cad/parity/primitive_contracts_parity_manifest.json",
    "crates/cad/parity/transform_parity_manifest.json",
    "crates/cad/parity/pattern_parity_manifest.json",
    "crates/cad/parity/shell_feature_graph_parity_manifest.json",
    "crates/cad/parity/fillet_feature_graph_parity_manifest.json",
    "crates/cad/parity/chamfer_feature_graph_parity_manifest.json",
    "crates/cad/parity/expanded_finishing_parity_manifest.json",
    "crates/cad/parity/sweep_parity_manifest.json",
    "crates/cad/parity/loft_parity_manifest.json",
    "crates/cad/parity/topology_repair_parity_manifest.json",
    "crates/cad/parity/material_assignment_parity_manifest.json",
    "crates/cad/parity/vcad_eval_receipts_parity_manifest.json",
    "crates/cad/parity/feature_op_hash_parity_manifest.json",
    "crates/cad/parity/modeling_edge_case_parity_manifest.json",
    "crates/cad/parity/core_modeling_checkpoint_parity_manifest.json",
    "crates/cad/parity/sketch_entity_set_parity_manifest.json",
    "crates/cad/parity/sketch_plane_parity_manifest.json",
    "crates/cad/parity/sketch_constraint_enum_parity_manifest.json",
    "crates/cad/parity/sketch_iterative_lm_parity_manifest.json",
    "crates/cad/parity/sketch_jacobian_residual_parity_manifest.json",
    "crates/cad/parity/sketch_constraint_status_parity_manifest.json",
    "crates/cad/parity/sketch_extrude_parity_manifest.json",
    "crates/cad/parity/sketch_interaction_parity_manifest.json",
    "crates/cad/parity/sketch_fixture_equivalence_parity_manifest.json",
    "crates/cad/parity/sketch_undo_redo_parity_manifest.json",
    "crates/cad/parity/sketch_constraints_checkpoint_parity_manifest.json",
    "crates/cad/parity/assembly_schema_parity_manifest.json",
    "crates/cad/parity/assembly_part_instance_parity_manifest.json",
    "crates/cad/parity/assembly_joint_frs_parity_manifest.json",
    "crates/cad/parity/assembly_joint_cb_parity_manifest.json",
    "crates/cad/parity/assembly_joint_limits_state_parity_manifest.json",
    "crates/cad/parity/assembly_fk_parity_manifest.json",
    "crates/cad/parity/assembly_ground_delete_parity_manifest.json",
    "crates/cad/parity/assembly_ui_selection_edit_parity_manifest.json",
    "crates/cad/parity/assembly_serialization_replay_parity_manifest.json",
    "crates/cad/parity/assembly_acceptance_scenes_parity_manifest.json",
    "crates/cad/parity/sketch_loft_parity_manifest.json",
    "crates/cad/parity/sketch_profile_validity_parity_manifest.json",
    "crates/cad/parity/sketch_revolve_parity_manifest.json",
    "crates/cad/parity/sketch_sweep_parity_manifest.json",
    "crates/cad/parity/fixtures/feature_op_hash_vcad_reference_corpus.json",
    "crates/cad/parity/fixtures/sketch_vcad_reference_corpus.json",
    "crates/cad/parity/fixtures/assembly_schema_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_part_instance_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_joint_frs_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_joint_cb_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_joint_limits_state_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_fk_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_ground_delete_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_ui_selection_edit_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_serialization_replay_vcad_reference.json",
    "crates/cad/parity/fixtures/assembly_acceptance_scenes_vcad_reference.json",
    "crates/cad/parity/fixtures/parity_fixture_corpus.json",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityCiArtifactManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub parity_check_entrypoint: String,
    pub source_artifact_count: usize,
    pub artifacts: Vec<ParityCiArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCiArtifact {
    pub artifact_id: String,
    pub source_path: String,
    pub sha256: String,
    pub bytes: usize,
}

pub fn build_ci_artifact_manifest(
    scorecard: &ParityScorecard,
    generated_from_scorecard: &str,
    repo_root: &Path,
) -> io::Result<ParityCiArtifactManifest> {
    let mut artifacts = Vec::with_capacity(PARITY_CI_SOURCE_PATHS.len());
    for path in PARITY_CI_SOURCE_PATHS {
        let artifact_path = repo_root.join(path);
        let bytes = fs::read(&artifact_path)?;
        artifacts.push(ParityCiArtifact {
            artifact_id: artifact_id(path),
            source_path: normalize_path(path),
            sha256: sha256_hex(&bytes),
            bytes: bytes.len(),
        });
    }
    artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));

    Ok(ParityCiArtifactManifest {
        manifest_version: 1,
        issue_id: PARITY_CI_ARTIFACTS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: normalize_path(generated_from_scorecard),
        parity_check_entrypoint: "scripts/cad/parity_check.sh".to_string(),
        source_artifact_count: artifacts.len(),
        artifacts,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn artifact_id(path: &str) -> String {
    let path = path
        .strip_prefix("crates/cad/parity/")
        .or_else(|| path.strip_prefix("crates/cad/"))
        .unwrap_or(path);
    path.replace('/', "_").replace(".json", "")
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn copy_ci_sources(repo_root: &Path, output_dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(output_dir)?;
    let mut copied = Vec::with_capacity(PARITY_CI_SOURCE_PATHS.len());
    for source in PARITY_CI_SOURCE_PATHS {
        let src = repo_root.join(source);
        let basename = source
            .replace("crates/cad/parity/", "")
            .replace('/', "_")
            .replace('\\', "_");
        let dst = output_dir.join(basename);
        fs::copy(&src, &dst)?;
        copied.push(dst);
    }
    copied.sort();
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::artifact_id;

    #[test]
    fn artifact_id_removes_known_prefixes() {
        assert_eq!(
            artifact_id("crates/cad/parity/vcad_reference_manifest.json"),
            "vcad_reference_manifest"
        );
        assert_eq!(
            artifact_id("crates/cad/parity/fixtures/parity_fixture_corpus.json"),
            "fixtures_parity_fixture_corpus"
        );
    }
}
