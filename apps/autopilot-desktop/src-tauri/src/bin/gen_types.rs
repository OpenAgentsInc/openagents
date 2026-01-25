use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = manifest_dir.join("..").join("src").join("gen");
    std::fs::create_dir_all(&out_dir)?;
    let out_file = out_dir.join("tauri-contracts.ts");

    autopilot_desktop_backend::contracts::export_ts(&out_file)?;
    println!("Generated {}", out_file.display());
    Ok(())
}
