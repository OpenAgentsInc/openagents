//! Writes a document's mass-properties feed (`cad-mass-feed.v1`), as CAD
//! Lab's `cad_export` `feed` does for a saved version, for a document kept
//! in the repository:
//!
//! ```sh
//! cargo run -p cad --example mass_feed -- DOCUMENT.json VERSION OUTPUT.json
//! ```
//!
//! The source digest is the SHA-256 of the document file. Lagrange 2's
//! sample ship (`crates/verse-lagrange2/cad/`) is regenerated this way.

use sha2::{Digest, Sha256};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [document, version, output] = args.as_slice() else {
        return Err("usage: mass_feed DOCUMENT.json VERSION OUTPUT.json".into());
    };
    let bytes = std::fs::read(document).map_err(|e| format!("{document}: {e}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let feed = cad::feed::from_document(
        &value,
        version.parse().map_err(|_| "VERSION is a whole number")?,
        &format!("{:x}", Sha256::digest(&bytes)),
    )?;
    let text = serde_json::to_string_pretty(&feed).map_err(|e| e.to_string())? + "\n";
    std::fs::write(output, text).map_err(|e| format!("{output}: {e}"))?;
    println!(
        "{output}: {} parts, {:.1} kg",
        feed.parts.len(),
        feed.total.mass_kg
    );
    Ok(())
}
