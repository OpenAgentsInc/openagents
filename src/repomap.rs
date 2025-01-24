use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Query, QueryCursor};
use lazy_static::lazy_static;

lazy_static! {
    static ref RUST_LANGUAGE: tree_sitter::Language = tree_sitter_rust::language();
}

pub fn generate_repo_map(repo_path: &Path) -> String {
    let mut parser = Parser::new();
    parser.set_language(*RUST_LANGUAGE).expect("Error loading Rust grammar");

    let mut repo_map = String::new();
    let query = Query::new(
        *RUST_LANGUAGE,
        r#"
        (function_item
            name: (identifier) @function.name)
        (impl_item
            trait: (type_identifier) @impl.trait
            type: (type_identifier) @impl.type)
        (trait_item
            name: (type_identifier) @trait.name)
        "#,
    ).expect("Error creating query");

    let mut cursor = QueryCursor::new();

    walk_dir(repo_path, &mut |path| {
        if path.extension().map_or(false, |ext| ext == "rs") {
            if let Ok(source_code) = fs::read_to_string(path) {
                let tree = parser.parse(&source_code, None).unwrap();
                let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());

                let mut file_map = String::new();
                
                // Get relative path by stripping the repo_path prefix
                let relative_path = path.strip_prefix(repo_path)
                    .unwrap_or(path)
                    .to_string_lossy();
                file_map.push_str(&format!("{}:\n", relative_path));

                for match_result in matches {
                    for capture in match_result.captures {
                        let text = &source_code[capture.node.byte_range()];
                        match capture.index {
                            0 => file_map.push_str(&format!("│fn {}\n", text)),
                            1 | 2 => file_map.push_str(&format!("│impl {} for {}\n", text, text)),
                            3 => file_map.push_str(&format!("│trait {}\n", text)),
                            _ => {}
                        }
                    }
                }

                if !file_map.is_empty() {
                    repo_map.push_str(&file_map);
                    repo_map.push('\n');
                }
            }
        }
    });

    repo_map
}

fn walk_dir(dir: &Path, callback: &mut dyn FnMut(&Path)) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, callback);
            } else {
                callback(&path);
            }
        }
    }
}