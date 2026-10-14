//! Pin the actual renderer sources for private preview-cache keys.
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn files(path: &Path, output: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).expect("read renderer sources") {
            files(&entry.expect("source entry").path(), output);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension == "rs" || extension == "wgsl")
    {
        output.push(path.into());
    }
}
fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace = root.join("../..").canonicalize().unwrap();
    let mut sources = Vec::new();
    for path in [
        "crates/verse/src/render.rs",
        "crates/verse/src/robot_lab.rs",
        "crates/verse-pbr/src",
        "crates/verse-gfx/src",
        "crates/robot-workbench/src/server.rs",
        "crates/robot-workbench/src/record.rs",
        "crates/robot-workbench/src/view.rs",
    ] {
        files(&workspace.join(path), &mut sources);
    }
    sources.sort();
    let mut hash = Sha256::new();
    for key in ["TARGET", "PROFILE", "OPT_LEVEL"] {
        hash.update(std::env::var(key).unwrap_or_default());
        hash.update([0]);
    }
    for path in sources {
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(&path).unwrap();
        hash.update(
            path.strip_prefix(&workspace)
                .unwrap()
                .to_string_lossy()
                .as_bytes(),
        );
        hash.update([0]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!(
        "cargo:rustc-env=ROBOT_RENDER_BUILD_SHA={:x}",
        hash.finalize()
    );
}
