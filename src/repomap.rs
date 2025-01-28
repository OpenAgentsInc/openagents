use lazy_static::lazy_static;
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};

lazy_static! {
    static ref RUST_LANGUAGE: tree_sitter::Language = tree_sitter_rust::language();
}

pub fn generate_repo_map(repo_path: &Path) -> String {
    generate_repo_map_with_blacklist(repo_path, &[])
}

pub fn generate_repo_map_with_blacklist(repo_path: &Path, blacklist: &[&str]) -> String {
    let mut parser = Parser::new();
    parser
        .set_language(*RUST_LANGUAGE)
        .expect("Error loading Rust grammar");

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
    )
    .expect("Error creating query");

    let mut cursor = QueryCursor::new();

    walk_dir(repo_path, &mut |path| {
        // Skip blacklisted paths
        if blacklist
            .iter()
            .any(|item| path.to_string_lossy().contains(item))
        {
            return;
        }

        let ext = path.extension().and_then(|e| e.to_str());

        match ext {
            Some("rs") => {
                if let Ok(source_code) = fs::read_to_string(path) {
                    let tree = parser.parse(&source_code, None).unwrap();
                    let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());

                    let mut file_map = String::new();

                    let relative_path = path
                        .strip_prefix(repo_path)
                        .unwrap_or(path)
                        .to_string_lossy();
                    file_map.push_str(&format!("{}:\n", relative_path));

                    for match_result in matches {
                        for capture in match_result.captures {
                            let text = &source_code[capture.node.byte_range()];
                            match capture.index {
                                0 => file_map.push_str(&format!("│fn {}\n", text)),
                                1 | 2 => {
                                    file_map.push_str(&format!("│impl {} for {}\n", text, text))
                                }
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
            Some("html") | Some("htm") => {
                if let Ok(content) = fs::read_to_string(path) {
                    let relative_path = path
                        .strip_prefix(repo_path)
                        .unwrap_or(path)
                        .to_string_lossy();
                    let mut file_map = format!("{}:\n", relative_path);

                    // Basic HTML structure detection
                    if content.contains("<body") {
                        file_map.push_str("│<body>\n");
                    }
                    if content.contains("<head") {
                        file_map.push_str("│<head>\n");
                    }
                    // Extract IDs
                    for line in content.lines() {
                        if line.contains("id=\"") {
                            if let Some(id) = extract_id(line) {
                                file_map.push_str(&format!("│#id: {}\n", id));
                            }
                        }
                    }

                    repo_map.push_str(&file_map);
                    repo_map.push('\n');
                }
            }
            Some("css") => {
                if let Ok(content) = fs::read_to_string(path) {
                    let relative_path = path
                        .strip_prefix(repo_path)
                        .unwrap_or(path)
                        .to_string_lossy();
                    let mut file_map = format!("{}:\n", relative_path);

                    // Extract CSS selectors
                    for line in content.lines() {
                        if line.contains("{") {
                            let selector = line.split('{').next().unwrap_or("").trim();
                            if !selector.is_empty() {
                                file_map.push_str(&format!("│{}\n", selector));
                            }
                        }
                    }

                    repo_map.push_str(&file_map);
                    repo_map.push('\n');
                }
            }
            Some("js") | Some("jsx") | Some("ts") | Some("tsx") => {
                if let Ok(content) = fs::read_to_string(path) {
                    let relative_path = path
                        .strip_prefix(repo_path)
                        .unwrap_or(path)
                        .to_string_lossy();
                    let mut file_map = format!("{}:\n", relative_path);

                    // Basic JS function and class detection
                    for line in content.lines() {
                        if line.contains("function ") {
                            if let Some(name) = extract_function_name(line) {
                                file_map.push_str(&format!("│function {}\n", name));
                            }
                        }
                        if line.contains("class ") {
                            if let Some(name) = extract_class_name(line) {
                                file_map.push_str(&format!("│class {}\n", name));
                            }
                        }
                        if line.contains("const ") && line.contains(" = ") {
                            if let Some(name) = extract_const_name(line) {
                                file_map.push_str(&format!("│const {}\n", name));
                            }
                        }
                    }

                    repo_map.push_str(&file_map);
                    repo_map.push('\n');
                }
            }
            _ => {}
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

fn extract_id(line: &str) -> Option<&str> {
    if let Some(start) = line.find("id=\"") {
        let start = start + 4;
        if let Some(end) = line[start..].find('"') {
            return Some(&line[start..start + end]);
        }
    }
    None
}

fn extract_function_name(line: &str) -> Option<&str> {
    if let Some(start) = line.find("function ") {
        let start = start + 9;
        let rest = &line[start..];
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].trim());
        }
    }
    None
}

fn extract_class_name(line: &str) -> Option<&str> {
    if let Some(start) = line.find("class ") {
        let start = start + 6;
        let rest = &line[start..];
        if let Some(end) = rest.find('{') {
            return Some(rest[..end].trim());
        }
        if let Some(end) = rest.find(' ') {
            return Some(rest[..end].trim());
        }
    }
    None
}

fn extract_const_name(line: &str) -> Option<&str> {
    if let Some(start) = line.find("const ") {
        let start = start + 6;
        let rest = &line[start..];
        if let Some(end) = rest.find(" = ") {
            return Some(rest[..end].trim());
        }
    }
    None
}
