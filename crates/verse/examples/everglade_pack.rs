//! Compiles the admitted Everglade sources into the pinned zone pack.
//!
//! ```text
//! cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
//! cargo run --release -p verse --example everglade_pack -- assets/verse/everglade --check
//! ```
//!
//! The first form writes `<sha256>.vtp` beside the source sets, removes
//! earlier packs there, and prints the `PACK_SHA256` and `PACK_BYTES` values
//! to pin in `verse::zones::everglade_pack`. `--check` writes nothing and
//! fails unless the sources compile to the pinned pack.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verse::zones::everglade_pack::{self, Limits, compile};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("everglade_pack: {error}");
            ExitCode::FAILURE
        }
    }
}

fn is_pack_name(name: &str) -> bool {
    name.strip_suffix(&format!(".{}", everglade_pack::PACK_EXTENSION))
        .is_some_and(|stem| {
            stem.len() == 64
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

fn remove_stale_packs(root: &Path, keep: &str) -> Result<(), String> {
    let entries = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if is_pack_name(name) && name != keep && entry.file_type().is_ok_and(|t| t.is_file()) {
            std::fs::remove_file(entry.path()).map_err(|e| format!("{name}: {e}"))?;
            println!("removed earlier pack {name}");
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .unwrap_or_else(|| everglade_pack::PACK_DIRECTORY.to_owned()),
    );
    let check = match args.next().as_deref() {
        None => false,
        Some("--check") => true,
        Some(other) => return Err(format!("unknown argument: {other}")),
    };
    let limits = Limits::EVERGLADE;
    let compiled = compile::compile(
        &root,
        &compile::SETS,
        Some(&root.join(compile::PLAYER_SOURCES)),
        &limits,
    )?;
    let name = format!("{}.{}", compiled.sha256, everglade_pack::PACK_EXTENSION);
    let length = compiled.bytes.len() as u64;
    println!(
        "{} models, {} triangles, {} decoded texture bytes",
        compiled.models, compiled.triangles, compiled.decoded_texture_bytes
    );
    println!(
        "committed: {} source bytes + {length} pack bytes = {} of {}",
        compiled.source_bytes,
        compiled.source_bytes + length,
        limits.committed_bytes
    );
    if check {
        if compiled.sha256 != everglade_pack::PACK_SHA256 || length != everglade_pack::PACK_BYTES {
            return Err(format!(
                "sources compile to {} ({length} bytes), not the pinned pack",
                compiled.sha256
            ));
        }
        println!("the sources compile to the pinned pack");
        return Ok(());
    }
    std::fs::write(root.join(&name), &compiled.bytes).map_err(|e| format!("{name}: {e}"))?;
    remove_stale_packs(&root, &name)?;
    println!("wrote {}", root.join(&name).display());
    println!("pub const PACK_SHA256: &str = \"{}\";", compiled.sha256);
    println!("pub const PACK_BYTES: u64 = {length};");
    Ok(())
}
