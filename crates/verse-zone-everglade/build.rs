//! Compiles every townsfolk definition under `townsfolk/npcs/`, and every
//! rumor under `townsfolk/rumors/`, into the client
//! (`zones::everglade::townsfolk::files` and `rumor_files`), so each is
//! data: adding a file needs no code change. The client loads only the
//! ones the roster, `townsfolk/town.json`, admits by digest.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let root =
        Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("townsfolk");
    println!(
        "cargo:rerun-if-changed={}",
        root.join("town.json").display()
    );
    let mut out = list(
        &root.join("npcs"),
        "/// Every definition file under `townsfolk/npcs/`, in name order.",
        "NPC_FILES",
    );
    out.push_str(&list(
        &root.join("rumors"),
        "/// Every rumor file under `townsfolk/rumors/`, in name order.",
        "RUMOR_FILES",
    ));
    let dest =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets it")).join("townsfolk_files.rs");
    std::fs::write(dest, out).expect("OUT_DIR is writable");
}

/// A constant `name` listing every JSON file in `dir`.
fn list(dir: &Path, doc: &str, name: &str) -> String {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = format!("{doc}\npub const {name}: &[&str] = &[\n");
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        writeln!(out, "    include_str!({:?}),", file.display().to_string()).expect("a string");
    }
    out.push_str("];\n");
    out
}
