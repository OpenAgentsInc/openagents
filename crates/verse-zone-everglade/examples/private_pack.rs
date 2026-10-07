//! Compiles a private build directory into a private pack without
//! uploading it, for a local proof of a conversion
//! (`docs/verse/everglade-medieval-refactor.md`, the proof).
//!
//! ```text
//! cargo run -p verse-zone-everglade --example private_pack -- BUILD_DIR OUT_DIR
//! ```
//!
//! `BUILD_DIR` holds the six files `compile::private::FILES` names, as a
//! private conversion script writes them. The pack is written to
//! `OUT_DIR/<sha256>.vtp` with mode 0600, and the digest and length are
//! printed, ready for an owner-local placement. Both directories must be
//! outside the repository: the pack is licensed content.

use std::path::{Path, PathBuf};

use verse_zone_everglade::zones::everglade_pack::compile::private;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1).map(PathBuf::from);
    let (Some(build), Some(out)) = (args.next(), args.next()) else {
        return Err("usage: private_pack BUILD_DIR OUT_DIR".into());
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    for directory in [&build, &out] {
        let full = directory.canonicalize().unwrap_or(directory.clone());
        if full.starts_with(&repository) {
            return Err(format!(
                "{} is inside the repository; private builds stay outside it",
                directory.display()
            ));
        }
    }
    let compiled = private::compile(&build)?;
    let pack = private::decode(&compiled.bytes)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join(format!("{}.vtp", compiled.sha256));
    std::fs::write(&path, &compiled.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    let triangles: Vec<u64> = pack.forms.iter().map(|f| f.triangles()).collect();
    println!(
        "{{\"sha256\":\"{}\",\"bytes\":{},\"triangles\":{:?},\"path\":\"{}\"}}",
        compiled.sha256,
        compiled.bytes.len(),
        triangles,
        path.display()
    );
    Ok(())
}
