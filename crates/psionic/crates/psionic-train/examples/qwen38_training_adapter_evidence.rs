use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::{
    QWEN38_LM_HEAD_ADAPTER_ARTIFACT_REF, qwen38_training_adapter_artifact_bytes,
    qwen38_training_adapter_evidence_report,
};

const DEFAULT_OUTPUT: &str = "fixtures/qwen38/reports/qwen38_training_adapter_evidence_v1.json";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    let artifact_output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(QWEN38_LM_HEAD_ADAPTER_ARTIFACT_REF));
    let report = qwen38_training_adapter_evidence_report()?;
    let artifact_bytes = qwen38_training_adapter_artifact_bytes()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    if let Some(parent) = artifact_output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&artifact_output, artifact_bytes)?;
    println!("{}", output.display());
    println!("{}", artifact_output.display());
    Ok(())
}
