use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_TCM_V1_MODEL_ID: &str = "tcm.v1";
pub const TASSADAR_TCM_V1_MODEL_REF: &str = "fixtures/tassadar/reports/tassadar_tcm_v1_model.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUniversalMachineFamily {
    TwoRegisterMachine,
    SingleTapeMachine,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalSubstrateSemanticRow {
    pub semantic_id: String,
    pub supported: bool,
    pub source_profile_ids: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalSubstrateModel {
    pub schema_version: u16,
    pub model_id: String,
    pub computation_style: String,
    pub control_rows: Vec<TassadarUniversalSubstrateSemanticRow>,
    pub memory_rows: Vec<TassadarUniversalSubstrateSemanticRow>,
    pub continuation_rows: Vec<TassadarUniversalSubstrateSemanticRow>,
    pub effect_boundary_rows: Vec<TassadarUniversalSubstrateSemanticRow>,
    pub unsupported_feature_families: Vec<String>,
    pub theory_scope: String,
    pub refusal_boundary: String,
    pub claim_boundary: String,
    pub summary: String,
    pub model_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalSubstrateModelError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_universal_substrate_model() -> TassadarUniversalSubstrateModel {
    let control_rows = vec![
        semantic_row(
            "conditional_control",
            &[
                "tassadar.internal_compute.article_closeout.v1",
                "tassadar.internal_compute.call_stack_and_recursion.v1",
            ],
            "the substrate model assumes exact conditional branch, structured loop, and call-frame behavior under declared bounded slices",
        ),
        semantic_row(
            "indirect_dispatch",
            &[
                "tassadar.proposal_profile.component_linking_interface_types.v1",
                "tassadar.internal_compute.component_model_abi.v1",
            ],
            "the substrate model includes indirect or table-mediated dispatch only through declared interface and component rows, not implicit host calls",
        ),
    ];
    let memory_rows = vec![
        semantic_row(
            "mutable_heap_segments",
            &[
                "tassadar.internal_compute.generalized_abi.v1",
                "tassadar.internal_compute.spill_tape_store.v1",
            ],
            "the substrate model includes mutable heap state with exact byte-addressed mutation and explicit spill-tape extension",
        ),
        semantic_row(
            "checkpoint_backed_extension",
            &[
                "tassadar.internal_compute.process_objects.v1",
                "tassadar.internal_compute.installed_process_lifecycle.v1",
            ],
            "state may extend beyond one slice only through persisted checkpoint, process-object, and lifecycle artifacts",
        ),
    ];
    let continuation_rows = vec![
        semantic_row(
            "bounded_slice_resume",
            &[
                "tassadar.internal_compute.execution_checkpoint.v1",
                "tassadar.internal_compute.spill_tape_store.v1",
            ],
            "the substrate model is explicitly resumable rather than infinite in-core; continuation happens by exact slice resume with persisted state",
        ),
        semantic_row(
            "persistent_process_identity",
            &[
                "tassadar.internal_compute.process_objects.v1",
                "tassadar.internal_compute.session_process.v1",
            ],
            "the substrate model includes stable process identity, tape head, and work-queue facts across resumptions",
        ),
    ];
    let effect_boundary_rows = vec![
        semantic_row(
            "declared_effect_profiles_only",
            &[
                "tassadar.effect_profile.virtual_fs_mounts.v1",
                "tassadar.effect_profile.simulator_backed_io.v1",
            ],
            "effectful behavior is only in-model when it stays inside declared simulator or virtual-fs profiles with replay and refusal truth",
        ),
        semantic_row(
            "ambient_host_effects_refused",
            &[
                "tassadar.effect_profile.replay_challenge_receipts.v1",
                "tassadar.effect_profile.async_lifecycle.v1",
            ],
            "ambient host power does not count as part of TCM.v1; undeclared host effects must refuse explicitly",
        ),
    ];
    let unsupported_feature_families = vec![
        String::from("ambient host io"),
        String::from("implicit public publication"),
        String::from("undeclared semantic-window widening"),
        String::from("arbitrary Wasm execution"),
    ];
    let mut model = TassadarUniversalSubstrateModel {
        schema_version: 1,
        model_id: String::from(TASSADAR_TCM_V1_MODEL_ID),
        computation_style: String::from(
            "bounded small-step resumable machine with persistent state, checkpoint-backed continuation, spill-tape extension, and explicit refusal on out-of-model effects",
        ),
        control_rows,
        memory_rows,
        continuation_rows,
        effect_boundary_rows,
        unsupported_feature_families,
        theory_scope: String::from(
            "TCM.v1 is the declared terminal substrate model for Psionic/Tassadar. Universal computation, if claimed later, must be a construction over this explicit substrate rather than over vague broadness language.",
        ),
        refusal_boundary: String::from(
            "features outside the declared control, memory, continuation, and effect rows are not implicit extensions of TCM.v1. They require either an explicit later semantic row or a typed refusal.",
        ),
        claim_boundary: String::from(
            "this model declares the substrate only. It does not by itself prove universal-machine encodings, witness suites, final gates, or public served universality posture.",
        ),
        summary: String::new(),
        model_digest: String::new(),
    };
    model.summary = format!(
        "TCM.v1 declares control_rows={}, memory_rows={}, continuation_rows={}, effect_rows={}, unsupported_feature_families={}.",
        model.control_rows.len(),
        model.memory_rows.len(),
        model.continuation_rows.len(),
        model.effect_boundary_rows.len(),
        model.unsupported_feature_families.len(),
    );
    model.model_digest = stable_digest(b"psionic_tassadar_tcm_v1_model|", &model);
    model
}

fn semantic_row(
    semantic_id: &str,
    source_profile_ids: &[&str],
    note: &str,
) -> TassadarUniversalSubstrateSemanticRow {
    TassadarUniversalSubstrateSemanticRow {
        semantic_id: String::from(semantic_id),
        supported: true,
        source_profile_ids: source_profile_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

#[must_use]
pub fn tassadar_universal_substrate_model_path() -> PathBuf {
    repo_root().join(TASSADAR_TCM_V1_MODEL_REF)
}

pub fn write_tassadar_universal_substrate_model(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalSubstrateModel, TassadarUniversalSubstrateModelError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalSubstrateModelError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let model = build_tassadar_universal_substrate_model();
    let json = serde_json::to_string_pretty(&model)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalSubstrateModelError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(model)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarUniversalSubstrateModelError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarUniversalSubstrateModelError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalSubstrateModelError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universal_substrate_model, read_repo_json,
        tassadar_universal_substrate_model_path, TassadarUniversalSubstrateModel,
        TASSADAR_TCM_V1_MODEL_ID, TASSADAR_TCM_V1_MODEL_REF,
    };

    #[test]
    fn universal_substrate_model_declares_tcm_v1_rows() {
        let model = build_tassadar_universal_substrate_model();

        assert_eq!(model.model_id, TASSADAR_TCM_V1_MODEL_ID);
        assert_eq!(model.control_rows.len(), 2);
        assert_eq!(model.memory_rows.len(), 2);
        assert_eq!(model.continuation_rows.len(), 2);
        assert_eq!(model.effect_boundary_rows.len(), 2);
    }

    #[test]
    fn universal_substrate_model_matches_committed_truth() {
        let generated = build_tassadar_universal_substrate_model();
        let committed: TassadarUniversalSubstrateModel =
            read_repo_json(TASSADAR_TCM_V1_MODEL_REF).expect("committed model");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_universal_substrate_model_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_tcm_v1_model.json")
        );
    }
}
