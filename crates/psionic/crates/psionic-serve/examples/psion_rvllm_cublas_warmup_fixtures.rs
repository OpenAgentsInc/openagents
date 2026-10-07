use std::{fs, path::Path};

use psionic_serve::{
    PSION_RVLLM_CUBLAS_WARMUP_FIXTURE_PATH, builtin_psion_rvllm_cublas_warmup_packet,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packet = builtin_psion_rvllm_cublas_warmup_packet();
    let path = Path::new(PSION_RVLLM_CUBLAS_WARMUP_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&packet)?)?;
    Ok(())
}
