use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Ensure the TS export directory exists for ts-rs output.
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let export_dir = crate_dir.join("../../expo/lib/generated");
    let _ = fs::create_dir_all(&export_dir);

    // Re-run if the build script changes.
    println!("cargo:rerun-if-changed=build.rs");
}
