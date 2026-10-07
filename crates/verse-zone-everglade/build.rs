//! Compiles every townsfolk definition under `townsfolk/npcs/` into the
//! client (`zones::everglade::townsfolk::files`), so a definition is data:
//! adding a file needs no code change. The client loads only the ones the
//! roster, `townsfolk/town.json`, admits by digest.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let root =
        Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("townsfolk");
    let npcs = root.join("npcs");
    println!(
        "cargo:rerun-if-changed={}",
        root.join("town.json").display()
    );
    println!("cargo:rerun-if-changed={}", npcs.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&npcs)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::from(
        "/// Every definition file under `townsfolk/npcs/`, in name order.\n\
         pub const NPC_FILES: &[&str] = &[\n",
    );
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        writeln!(out, "    include_str!({:?}),", file.display().to_string()).expect("a string");
    }
    out.push_str("];\n");
    let dest =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets it")).join("townsfolk_files.rs");
    std::fs::write(dest, out).expect("OUT_DIR is writable");
}
