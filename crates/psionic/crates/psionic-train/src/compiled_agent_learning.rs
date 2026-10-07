use std::{
    fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

use psionic_eval::{
    COMPILED_AGENT_DEFAULT_ROW_FIXTURE_PATH, CompiledAgentDefaultLearnedRowContract,
    canonical_compiled_agent_default_row_contract,
};

#[derive(Debug, Error)]
pub enum CompiledAgentLearningArtifactsError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error(
        "compiled-agent default row fixture `{path}` drifted from the canonical generator output"
    )]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub const COMPILED_AGENT_DEFAULT_ROW_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_default_row_contract.rs";
pub const COMPILED_AGENT_DEFAULT_ROW_PROBE_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_default_row_probe.rs";

#[must_use]
pub fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}

#[must_use]
pub fn repo_relative_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

#[must_use]
pub fn default_contract_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_DEFAULT_ROW_FIXTURE_PATH)
}

pub fn write_compiled_agent_default_row_contract(
    output_path: &Path,
) -> Result<CompiledAgentDefaultLearnedRowContract, CompiledAgentLearningArtifactsError> {
    let contract = canonical_compiled_agent_default_row_contract();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentLearningArtifactsError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| CompiledAgentLearningArtifactsError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

pub fn write_default_contract_fixture()
-> Result<CompiledAgentDefaultLearnedRowContract, CompiledAgentLearningArtifactsError> {
    write_compiled_agent_default_row_contract(default_contract_fixture_path().as_path())
}

pub fn verify_default_contract_fixture()
-> Result<CompiledAgentDefaultLearnedRowContract, CompiledAgentLearningArtifactsError> {
    let expected = canonical_compiled_agent_default_row_contract();
    let path = default_contract_fixture_path();
    let bytes = fs::read(&path).map_err(|error| CompiledAgentLearningArtifactsError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let committed: CompiledAgentDefaultLearnedRowContract = serde_json::from_slice(&bytes)?;
    if committed != expected {
        return Err(CompiledAgentLearningArtifactsError::FixtureDrift {
            path: path.display().to_string(),
        });
    }
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use psionic_eval::COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION;

    use super::{verify_default_contract_fixture, write_compiled_agent_default_row_contract};

    #[test]
    fn compiled_agent_default_row_writer_emits_the_canonical_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let output_path = dir.path().join("compiled_agent_default_row_v1.json");
        let contract = write_compiled_agent_default_row_contract(output_path.as_path())?;
        assert_eq!(
            contract.schema_version,
            COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION
        );
        assert!(output_path.exists());
        Ok(())
    }

    #[test]
    fn compiled_agent_default_row_fixture_verifier_accepts_the_committed_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = verify_default_contract_fixture()?;
        assert_eq!(
            fixture.schema_version,
            COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION
        );
        Ok(())
    }
}
