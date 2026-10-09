//! Compiles the medieval kit build into the kit pack and prints its pin
//! (`docs/verse/everglade-medieval-refactor.md`, the pack pipeline).
//!
//! ```text
//! cargo run --release -p verse-zone-everglade --example everglade_kit -- [--check] [BUILD_DIR]
//! ```
//!
//! `BUILD_DIR` is what `scripts/unreal/medieval_kit_build.py` writes,
//! `~/.openagents/verse/private/medieval-town/kit-build` by default. The pack
//! is written outside the repository, to
//! `~/.openagents/verse/private/medieval-town/packs/<sha256>.vtp`, and into
//! the desktop's zone cache (`~/.openagents/verse/zones-cache`, or
//! `$VERSE_HOME/zones-cache`), both with mode 0600. It prints the two pin
//! lines of `everglade_pack::kit`, which the artifact queue writes
//! (`artifacts/everglade-kit.json`). `--check` compiles again and fails
//! unless the pack is the pinned one. Every piece must keep to its
//! committed box. The upload to the private bucket is the owner's
//! (`gcloud storage cp PACK gs://openagentsgemini-verse-private-assets/packs/`).
//!
//! `--phone [--check]` derives the phone tier's pack from the pinned full
//! pack in the private `packs/` directory instead (images at most
//! `compile::kit::PHONE_EDGE` pixels, #10908) and prints the two
//! `KIT_PHONE_` pin lines (`artifacts/everglade-kit-phone.json`).

use std::path::{Path, PathBuf};

use verse_zone_everglade::zones::everglade_pack::compile::kit as compiled;
use verse_zone_everglade::zones::everglade_pack::{ZonePack, kit};

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Derives the phone tier's pack from the pinned full pack.
fn phone(check: bool) -> Result<(), String> {
    let packs = home().join(".openagents/verse/private/medieval-town/packs");
    let full = std::fs::read(packs.join(format!("{}.vtp", kit::KIT_SHA256)))
        .map_err(|e| format!("the pinned kit pack is not in {}: {e}", packs.display()))?;
    kit::pinned().verify(&full)?;
    let compiled = compiled::phone(&full)?;
    if check {
        if compiled.sha256 != kit::KIT_PHONE_SHA256
            || compiled.bytes.len() as u64 != kit::KIT_PHONE_BYTES
        {
            return Err(format!(
                "the phone pack derives to {} ({} bytes), not the pinned {} ({} bytes)",
                compiled.sha256,
                compiled.bytes.len(),
                kit::KIT_PHONE_SHA256,
                kit::KIT_PHONE_BYTES
            ));
        }
        println!("the pinned kit derives to the pinned phone pack");
        return Ok(());
    }
    let name = format!("{}.vtp", compiled.sha256);
    private_write(&packs.join(&name), &compiled.bytes)?;
    let verse = std::env::var_os("VERSE_HOME")
        .map_or_else(|| home().join(".openagents/verse"), PathBuf::from);
    private_write(&verse.join("zones-cache").join(&name), &compiled.bytes)?;
    eprintln!(
        "phone kit pack: {} models, {} decoded texture bytes, {} bytes, {}",
        compiled.models,
        compiled.decoded_texture_bytes,
        compiled.bytes.len(),
        packs.join(&name).display()
    );
    println!(
        "pub const KIT_PHONE_SHA256: &str = \"{}\";",
        compiled.sha256
    );
    println!("pub const KIT_PHONE_BYTES: u64 = {};", compiled.bytes.len());
    Ok(())
}

fn main() -> Result<(), String> {
    let mut check = false;
    let mut phone_tier = false;
    let mut build = home().join(".openagents/verse/private/medieval-town/kit-build");
    for arg in std::env::args().skip(1) {
        if arg == "--check" {
            check = true;
        } else if arg == "--phone" {
            phone_tier = true;
        } else {
            build = PathBuf::from(arg);
        }
    }
    if phone_tier {
        return phone(check);
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if build
        .canonicalize()
        .unwrap_or_else(|_| build.clone())
        .starts_with(&repository)
    {
        return Err("the kit build is inside the repository; it stays outside it".into());
    }
    let compiled = compiled::compile(&build, kit::grade)?;
    let pack = compiled::decode(&compiled.bytes)?;
    // Every piece is in the pack and keeps to its committed box.
    let mut empty = ZonePack {
        textures: Vec::new(),
        materials: Vec::new(),
        models: Vec::new(),
        character: None,
        forms: Vec::new(),
    };
    let report = kit::install(&mut empty, Some(&pack));
    if report.proxies != 0 || !report.refused.is_empty() {
        let missing: Vec<&str> = kit::PIECES
            .iter()
            .map(|p| p.model)
            .filter(|m| pack.model(m).is_none())
            .collect();
        return Err(format!(
            "the kit lacks or misfits pieces: missing {missing:?}, outside their boxes {:?}",
            report.refused
        ));
    }
    for far in pack
        .models
        .iter()
        .filter(|m| m.name.starts_with(compiled::FAR_PREFIX))
    {
        if empty.model(&far.name).is_none() {
            return Err(format!(
                "The compiled far level was not admitted: {}",
                far.name
            ));
        }
    }
    for house in verse_zone_everglade::zones::everglade::house_lod::houses() {
        for level in [1, 2] {
            let name = verse_zone_everglade::zones::everglade::house_lod::model(house, level);
            if pack.model(&name).is_none() || empty.model(&name).is_none() {
                return Err(format!(
                    "The compiled house level is missing or refused: {name}"
                ));
            }
        }
    }
    if check {
        if compiled.sha256 != kit::KIT_SHA256 || compiled.bytes.len() as u64 != kit::KIT_BYTES {
            return Err(format!(
                "the kit build compiles to {} ({} bytes), not the pinned {} ({} bytes)",
                compiled.sha256,
                compiled.bytes.len(),
                kit::KIT_SHA256,
                kit::KIT_BYTES
            ));
        }
        println!("the kit build compiles to the pinned pack");
        return Ok(());
    }
    let name = format!("{}.vtp", compiled.sha256);
    let packs = home().join(".openagents/verse/private/medieval-town/packs");
    private_write(&packs.join(&name), &compiled.bytes)?;
    let verse = std::env::var_os("VERSE_HOME")
        .map_or_else(|| home().join(".openagents/verse"), PathBuf::from);
    private_write(&verse.join("zones-cache").join(&name), &compiled.bytes)?;
    eprintln!(
        "kit pack: {} models, {} triangles, {} decoded texture bytes, {}",
        compiled.models,
        compiled.triangles,
        compiled.decoded_texture_bytes,
        packs.join(&name).display()
    );
    println!("pub const KIT_SHA256: &str = \"{}\";", compiled.sha256);
    println!("pub const KIT_BYTES: u64 = {};", compiled.bytes.len());
    Ok(())
}
