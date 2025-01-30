use ignore::Walk;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Default patterns to ignore when generating repository map
const DEFAULT_BLACKLIST: &[&str] = &[
    "assets/**/*.css",
    "assets/fonts/**/*",
    "node_modules/**/*",
    "target/**/*",
    "**/*.min.js",
    "**/*.min.css",
    "**/*.map",
    "**/*.lock",
];

/// Generates a repository map with standard file filtering
pub fn generate_repo_map(path: &Path) -> String {
    let mut output = String::new();
    let mut entries: Vec<(PathBuf, String)> = Vec::new();

    // Use ignore crate to respect .gitignore
    for result in Walk::new(path) {
        if let Ok(entry) = result {
            let path = entry.path();
            
            // Skip if path matches any blacklist pattern
            if DEFAULT_BLACKLIST.iter().any(|pattern| {
                ignore::gitignore::Pattern::new(pattern)
                    .map(|p| p.matches_path(path))
                    .unwrap_or(false)
            }) {
                debug!("Skipping blacklisted path: {:?}", path);
                continue;
            }

            // Only process files
            if path.is_file() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let mut ids = Vec::new();
                    let mut functions = Vec::new();
                    let mut classes = Vec::new();
                    let mut consts = Vec::new();

                    // Extract identifiers from content
                    for line in content.lines() {
                        if let Some(id) = extract_id(line) {
                            ids.push(id);
                        }
                        if let Some(name) = extract_function_name(line) {
                            functions.push(name);
                        }
                        if let Some(name) = extract_class_name(line) {
                            classes.push(name);
                        }
                        if let Some(name) = extract_const_name(line) {
                            consts.push(name);
                        }
                    }

                    // Only include files that have identifiable content
                    if !ids.is_empty() || !functions.is_empty() || !classes.is_empty() || !consts.is_empty() {
                        let mut file_content = String::new();
                        
                        // Add IDs
                        for id in ids {
                            file_content.push_str(&format!("#id: {}\n", id));
                        }

                        // Add functions
                        for func in functions {
                            file_content.push_str(&format!("fn {}\n", func));
                        }

                        // Add classes
                        for class in classes {
                            file_content.push_str(&format!("class {}\n", class));
                        }

                        // Add constants
                        for const_name in consts {
                            file_content.push_str(&format!("const {}\n", const_name));
                        }

                        if !file_content.is_empty() {
                            if let Ok(rel_path) = path.strip_prefix(path) {
                                entries.push((rel_path.to_path_buf(), file_content));
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort entries by path for consistent output
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Build final output
    for (path, content) in entries {
        if let Some(path_str) = path.to_str() {
            output.push_str(&format!("{}:\n", path_str));
            for line in content.lines() {
                output.push_str(&format!("│{}\n", line));
            }
            output.push('\n');
        }
    }

    output
}

fn extract_id(line: &str) -> Option<&str> {
    if line.contains("id=\"") {
        let start = line.find("id=\"")? + 4;
        let end = line[start..].find('\"')? + start;
        Some(&line[start..end])
    } else {
        None
    }
}

fn extract_function_name(line: &str) -> Option<&str> {
    if line.contains("fn ") {
        let start = line.find("fn ")? + 3;
        let end = line[start..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + start)
            .unwrap_or(line.len());
        Some(&line[start..end])
    } else {
        None
    }
}

fn extract_class_name(line: &str) -> Option<&str> {
    if line.contains("class ") {
        let start = line.find("class ")? + 6;
        let end = line[start..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + start)
            .unwrap_or(line.len());
        Some(&line[start..end])
    } else {
        None
    }
}

fn extract_const_name(line: &str) -> Option<&str> {
    if line.contains("const ") {
        let start = line.find("const ")? + 6;
        let end = line[start..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + start)
            .unwrap_or(line.len());
        Some(&line[start..end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        
        // Create some test files
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        
        // Source files
        fs::write(
            dir.path().join("src/main.rs"),
            "fn main() {}\nfn helper() {}\n",
        ).unwrap();
        
        // CSS file that should be ignored
        fs::write(
            dir.path().join("assets/style.css"),
            ".class { color: red; }\n",
        ).unwrap();

        dir
    }

    #[test]
    fn test_repo_map_generation() {
        let dir = setup_test_repo();
        let map = generate_repo_map(dir.path());
        
        // Should include source files
        assert!(map.contains("src/main.rs"));
        assert!(map.contains("fn main"));
        assert!(map.contains("fn helper"));
        
        // Should not include CSS files
        assert!(!map.contains("assets/style.css"));
        assert!(!map.contains(".class"));
    }

    #[test]
    fn test_extractors() {
        assert_eq!(extract_id(r#"<div id="test">"#), Some("test"));
        assert_eq!(extract_function_name("fn test_func() {"), Some("test_func"));
        assert_eq!(extract_class_name("class TestClass {"), Some("TestClass"));
        assert_eq!(extract_const_name("const TEST_CONST: i32 = 42;"), Some("TEST_CONST"));
    }
}