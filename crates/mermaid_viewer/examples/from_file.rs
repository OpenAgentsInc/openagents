use std::env;
use std::fs;
use std::path::Path;

use mermaid_viewer::{render_mermaid, render_mermaid_code};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: from_file <path-to-.mmd|.md|.svg>")?;
    let data = fs::read_to_string(&path)?;

    let ext = Path::new(&path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // If it's Markdown, try to extract the first ```mermaid block
    let content = if ext == "md" || ext == "markdown" {
        if let Some(start) = data.find("```mermaid") {
            if let Some(rest) = data[start + 10..].find("```") {
                data[start + 10..start + 10 + rest].trim().to_string()
            } else {
                data
            }
        } else {
            data
        }
    } else {
        data
    };

    let is_svg = content.trim_start().starts_with("<svg");
    let viewer = if is_svg {
        render_mermaid(&content)?
    } else {
        render_mermaid_code(&content)?
    };

    viewer.run()?;
    Ok(())
}

