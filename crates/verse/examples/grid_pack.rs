//! Regenerates the pinned Grid engine pack under `assets/verse/grid/`.
//! Usage: grid_pack [DIR]
fn main() -> Result<(), String> {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(verse::grid_pack::pinned_dir, std::path::PathBuf::from);
    let pack = verse::grid_pack::compile(&dir)?;
    println!(
        "wrote {} models, {} placements, revision {} to {}",
        pack.models.len(),
        pack.placements.len(),
        pack.source_revision,
        dir.display()
    );
    Ok(())
}
