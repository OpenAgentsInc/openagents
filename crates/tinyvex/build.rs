use std::fs;
use std::path::PathBuf;

fn main() {
    // Ensure export directory exists
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let export_dir = crate_dir.join("../../docs/types");
    let _ = fs::create_dir_all(&export_dir);

    // Export Tinyvex row types to docs/types as .ts interfaces
    ts_rs::export! {
        ThreadRow, MessageRow, ToolCallRow => "../../docs/types/"
    }

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}

