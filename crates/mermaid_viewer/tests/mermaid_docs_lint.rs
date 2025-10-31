use std::fs;

fn read(dir: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(dir) { for e in rd.flatten() {
        let p = e.path(); if p.is_file() {
            if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                if matches!(ext, "md"|"markdown") {
                    if let Ok(s) = fs::read_to_string(&p) {
                        out.push((p.file_name().unwrap().to_string_lossy().into_owned(), s));
                    }
                }
            }
        }
    }}
    out
}

fn extract_mermaid_blocks(s: &str) -> Vec<String> {
    let mut blocks = vec![];
    let mut i = 0;
    while let Some(start) = s[i..].find("```mermaid") { let a = i + start + "```mermaid".len(); // after fence
        if let Some(end_rel) = s[a..].find("```") { let b = a + end_rel; blocks.push(s[a..b].trim().to_string()); i = b + 3; } else { break; }
    }
    blocks
}

fn validate_with_mmdc(block: &str) -> Result<(), String> {
    if std::env::var("MERMAID_VALIDATE").ok().as_deref() != Some("1") {
        return Ok(()); // opt-in only to avoid CI flakiness
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let in_path = dir.path().join("block.mmd");
    let out_path = dir.path().join("out.svg");
    fs::write(&in_path, block).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("npx")
        .args(["--yes", "@mermaid-js/mermaid-cli@10.9.1", "-i"]) 
        .arg(&in_path)
        .args(["-o"]).arg(&out_path)
        .status().map_err(|e| e.to_string())?;
    if !status.success() { return Err("mermaid-cli failed".into()); }
    Ok(())
}

#[test]
fn lint_tinyvex_mermaid_docs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().and_then(|p| p.parent()).unwrap()
        .join("docs").join("tinyvex");
    let docs = read(root.to_str().unwrap());
    assert!(!docs.is_empty(), "expected docs/tinyvex to have files");
    for (name, content) in docs {
        let blocks = extract_mermaid_blocks(&content);
        if blocks.is_empty() { continue; } // skip non-mermaid docs
        for (idx, b) in blocks.iter().enumerate() {
            // Heuristics to catch common mistakes that break rendering on GitHub and our viewer
            // Allow literal \n sequences; the viewer no longer relies on HTML labels
            // Avoid non-ASCII arrows in labels for portability
            assert!(!b.contains('→'), "{}[block {}]: contains non-ASCII arrow; use '->' instead", name, idx);
            // Flowchart should start with a keyword
            if b.trim_start().starts_with("flowchart") {
                // basic check for at least one node and edge
                assert!(b.contains("--"), "{}[block {}]: flowchart has no edges", name, idx);
            }
            // erDiagram: allow quoted relation labels; just ensure there are relationships
            if b.trim_start().starts_with("erDiagram") {
                assert!(b.contains("||") || b.contains("o{"), "{}[block {}]: erDiagram missing relationships", name, idx);
            }
            if b.trim_start().starts_with("sequenceDiagram") {
                // Must contain at least one participant
                assert!(b.contains("participant"), "{}[block {}]: sequenceDiagram missing participants", name, idx);
            }
            if let Err(e) = validate_with_mmdc(b) { panic!("{}[block {}]: {}", name, idx, e); }
        }
    }
}
