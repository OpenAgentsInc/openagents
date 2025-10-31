use std::path::Path;
use mermaid_viewer::render_mermaid_docs_index;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Show a sidebar of Tinyvex diagrams found under docs/tinyvex
    let viewer = render_mermaid_docs_index(Path::new("docs/tinyvex"))?;
    viewer.run()?;
    Ok(())
}
