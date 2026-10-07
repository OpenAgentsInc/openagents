use std::{env, error::Error, fs, io::Cursor, path::PathBuf};

use psionic_train::{
    PsionReferencePilotJointContributionRequest, build_psion_actual_pretraining_joint_contribution,
};
use zstd::stream::{decode_all as zstd_decode_all, encode_all as zstd_encode_all};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let request_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("missing request path")?;
    let output_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("missing output path")?;
    let request: PsionReferencePilotJointContributionRequest = read_zstd_json(&request_path)?;
    let receipt = build_psion_actual_pretraining_joint_contribution(root.as_path(), &request)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_zstd_json(&output_path, &receipt)?;
    println!(
        "psion actual pretraining joint contribution completed: step={} contributor={} backend={} output={}",
        receipt.global_step,
        receipt.contributor_id,
        receipt.runtime_backend,
        output_path.display()
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn read_zstd_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T, Box<dyn Error>> {
    let encoded_bytes = fs::read(path)?;
    let decoded_bytes = zstd_decode_all(Cursor::new(encoded_bytes))?;
    Ok(serde_json::from_slice(&decoded_bytes)?)
}

fn write_zstd_json<T: serde::Serialize>(
    path: &PathBuf,
    value: &T,
) -> Result<(), Box<dyn Error>> {
    let json_bytes = serde_json::to_vec(value)?;
    let encoded_bytes = zstd_encode_all(Cursor::new(json_bytes), 7)?;
    fs::write(path, encoded_bytes)?;
    Ok(())
}
