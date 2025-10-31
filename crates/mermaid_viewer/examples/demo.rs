use std::path::PathBuf;
use mermaid_viewer::render_mermaid_docs_index;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve absolute path to repo/docs/tinyvex regardless of CWD
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir.parent().and_then(|p| p.parent()).unwrap();
    let docs_dir = repo_root.join("docs").join("tinyvex");
    let viewer = render_mermaid_docs_index(&docs_dir)?;
    viewer.run()?;
    Ok(())
}
