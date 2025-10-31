use std::fs;
use std::path::Path;

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
    let bytes = s.as_bytes();
    while let Some(start) = s[i..].find("```mermaid") { let a = i + start + 9; // after fence
        if let Some(end_rel) = s[a..].find("```") { let b = a + end_rel; blocks.push(s[a..b].trim().to_string()); i = b + 3; } else { break; }
    }
    blocks
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
            assert!(!b.contains("\\n"), "{}[block {}]: contains literal \\n sequences; use <br/> in labels", name, idx);
            // Avoid non-ASCII arrows in labels for portability
            assert!(!b.contains('→'), "{}[block {}]: contains non-ASCII arrow; use '->' instead", name, idx);
            // Flowchart should start with a keyword
            if b.trim_start().starts_with("flowchart") {
                // basic check for at least one node and edge
                assert!(b.contains("--"), "{}[block {}]: flowchart has no edges", name, idx);
            }
            if b.trim_start().starts_with("erDiagram") {
                // relationships should not contain quotes which sometimes break parsing
                assert!(!b.contains('\"'), "{}[block {}]: erDiagram relation labels should avoid quotes", name, idx);
            }
            if b.trim_start().starts_with("sequenceDiagram") {
                // Must contain at least one participant
                assert!(b.contains("participant"), "{}[block {}]: sequenceDiagram missing participants", name, idx);
            }
        }
    }
}
