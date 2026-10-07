use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_PROCESS_OBJECT_BUNDLE_FILE, TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF,
    build_tassadar_process_object_runtime_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = repo_root()
        .join(TASSADAR_PROCESS_OBJECT_RUN_ROOT_REF)
        .join(TASSADAR_PROCESS_OBJECT_BUNDLE_FILE);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_process_object_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n"))?;
    println!(
        "wrote process-object bundle to {} ({})",
        output_path.display(),
        bundle.bundle_id
    );
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}
