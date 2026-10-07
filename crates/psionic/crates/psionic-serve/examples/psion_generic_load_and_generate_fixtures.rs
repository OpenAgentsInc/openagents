use std::{error::Error, fs, path::PathBuf};

use psionic_serve::{
    PSION_GENERIC_LOAD_AND_GENERATE_FIXTURE_PATH, builtin_psion_generic_load_and_generate_packet,
};

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_path = repo_root.join(PSION_GENERIC_LOAD_AND_GENERATE_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let packet = builtin_psion_generic_load_and_generate_packet()?;
    fs::write(fixture_path, serde_json::to_string_pretty(&packet)?)?;
    Ok(())
}
