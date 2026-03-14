use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_DECK_SOURCE: &str = "content/five-markets.deck.md";
const OUTPUT_DECK_NAME: &str = "embedded.deck.md";

fn main() -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=OPENAGENTS_DECK_SOURCE");
    println!("cargo:rerun-if-changed={DEFAULT_DECK_SOURCE}");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR")
            .map_err(|error| format!("CARGO_MANIFEST_DIR must be set for build.rs: {error}"))?,
    );
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);

    let selected_source = match env::var("OPENAGENTS_DECK_SOURCE") {
        Ok(raw) if !raw.trim().is_empty() => {
            resolve_custom_source(&raw, &manifest_dir, workspace_root.as_deref())?
        }
        Ok(_) | Err(env::VarError::NotPresent) => manifest_dir.join(DEFAULT_DECK_SOURCE),
        Err(env::VarError::NotUnicode(_)) => {
            return Err("OPENAGENTS_DECK_SOURCE must be valid UTF-8".to_string());
        }
    };

    println!("cargo:rerun-if-changed={}", selected_source.display());
    println!(
        "cargo:rustc-env=OPENAGENTS_EMBEDDED_DECK_PATH={}",
        selected_source.display()
    );

    let deck_source = fs::read_to_string(&selected_source).map_err(|error| {
        format!(
            "failed to read deck source '{}': {error}",
            selected_source.display()
        )
    })?;

    let out_dir = PathBuf::from(
        env::var("OUT_DIR")
            .map_err(|error| format!("OUT_DIR must be set for build.rs execution: {error}"))?,
    );
    let embedded_path = out_dir.join(OUTPUT_DECK_NAME);
    fs::write(&embedded_path, deck_source).map_err(|error| {
        format!(
            "failed to write embedded deck '{}': {error}",
            embedded_path.display()
        )
    })?;
    Ok(())
}

fn resolve_custom_source(
    raw: &str,
    manifest_dir: &Path,
    workspace_root: Option<&Path>,
) -> Result<PathBuf, String> {
    let requested = PathBuf::from(raw);
    if requested.is_absolute() {
        return validate_candidate(requested, raw);
    }

    let mut candidates = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join(&requested));
    }

    candidates.push(manifest_dir.join(&requested));

    if let Some(root) = workspace_root {
        candidates.push(root.join(&requested));
    }

    candidates.sort();
    candidates.dedup();

    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    let attempted = candidates
        .iter()
        .map(|candidate| format!("  - {}", candidate.display()))
        .collect::<Vec<_>>()
        .join("\n");

    Err(format!(
        "OPENAGENTS_DECK_SOURCE='{raw}' was not found.\nTried:\n{attempted}"
    ))
}

fn validate_candidate(candidate: PathBuf, raw: &str) -> Result<PathBuf, String> {
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "OPENAGENTS_DECK_SOURCE='{raw}' does not point to a readable file: {}",
            candidate.display()
        ))
    }
}
