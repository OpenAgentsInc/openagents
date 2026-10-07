use std::path::PathBuf;

use psionic_serve::builtin_psion_family_serve_vocabulary_packet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();
    builtin_psion_family_serve_vocabulary_packet().write_fixture(repo_root)?;
    Ok(())
}
