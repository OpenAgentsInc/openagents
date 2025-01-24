use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};

pub fn generate_repo_map(repo_path: &Path) -> String {
    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_rust::language() };
    parser.set_language(&language).expect("Error loading Rust grammar");

    let mut repo_map = String::new();
    let query = Query::new(
        &language,
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
                file_map.push_str(&format!("{}:\n", path.display()));

                while let Some(m) = streaming_iterator::StreamingIterator::next(&mut matches) {
                    for capture in m.captures {
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