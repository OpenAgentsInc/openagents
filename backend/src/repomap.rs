use glob::Pattern;
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
pub fn generate_repo_map(base_path: &Path) -> String {
    debug!("Generating repo map for path: {:?}", base_path);
    let mut output = String::new();
    let mut entries: Vec<(PathBuf, String)> = Vec::new();

    // Compile glob patterns
    let patterns: Vec<Pattern> = DEFAULT_BLACKLIST
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();
    debug!("Compiled {} blacklist patterns", patterns.len());

    // Use ignore crate to respect .gitignore and flatten the iterator
    for entry in Walk::new(base_path).flatten() {
        let path = entry.path();
        debug!("Processing path: {:?}", path);

        // Skip if path matches any blacklist pattern
        if let Ok(rel_path) = path.strip_prefix(base_path) {
            if patterns
                .iter()
                .any(|pattern| pattern.matches_path(rel_path))
            {
                debug!("Skipping blacklisted path: {:?}", rel_path);
                continue;
            }

            // Only process files
            if path.is_file() {
                // Special handling for docs directory - just list the files
                if rel_path.starts_with("docs/") {
                    debug!("Adding docs file: {:?}", rel_path);
                    entries.push((rel_path.to_path_buf(), String::new()));
                    continue;
                }

                debug!("Reading file: {:?}", path);
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
                    if !ids.is_empty()
                        || !functions.is_empty()
                        || !classes.is_empty()
                        || !consts.is_empty()
                    {
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
                            debug!("Adding entry for path: {:?}", rel_path);
                            entries.push((rel_path.to_path_buf(), file_content));
                        }
                    } else {
                        debug!("No identifiable content in file: {:?}", path);
                    }
                } else {
                    debug!("Failed to read file: {:?}", path);
                }
            }
        } else {
            debug!("Failed to get relative path for: {:?}", path);
        }
    }

    // Sort entries by path for consistent output
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    debug!("Sorted {} entries", entries.len());

    // Build final output
    for (path, content) in entries {
        if let Some(path_str) = path.to_str() {
            debug!("Adding to output: {}", path_str);
            output.push_str(&format!("{}:\n", path_str));
            // Only add content lines if there is content (non-docs files)
            if !content.is_empty() {
                for line in content.lines() {
                    output.push_str(&format!("│{}\n", line));
                }
            }
            output.push('\n');
        }
    }

    debug!("Final output length: {}", output.len());
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
    use tracing_subscriber::fmt::format::FmtSpan;

    fn init_logging() {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_span_events(FmtSpan::CLOSE)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .try_init();
        if subscriber.is_err() {
            eprintln!("Failed to initialize logging");
        }
    }

    fn setup_test_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        debug!("Created test directory: {:?}", dir.path());

        // Create some test files
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();

        // Source files
        let main_rs_path = dir.path().join("src/main.rs");
        debug!("Writing main.rs to: {:?}", main_rs_path);
        fs::write(main_rs_path, "fn main() {}\nfn helper() {}\n").unwrap();

        // CSS file that should be ignored
        let css_path = dir.path().join("assets/style.css");
        debug!("Writing style.css to: {:?}", css_path);
        fs::write(css_path, ".class { color: red; }\n").unwrap();

        // Docs file that should be listed without content
        let docs_path = dir.path().join("docs/api.md");
        debug!("Writing api.md to: {:?}", docs_path);
        fs::write(docs_path, "# API Documentation\n## Functions\n").unwrap();

        dir
    }

    #[test]
    fn test_repo_map_generation() {
        init_logging();
        let dir = setup_test_repo();
        debug!("Test repo setup at: {:?}", dir.path());

        let map = generate_repo_map(dir.path());
        debug!("Generated map:\n{}", map);

        // Should include source files with content
        assert!(
            map.contains("src/main.rs"),
            "Map should contain src/main.rs"
        );
        assert!(map.contains("fn main"), "Map should contain fn main");
        assert!(map.contains("fn helper"), "Map should contain fn helper");

        // Should include docs files without content
        assert!(
            map.contains("docs/api.md"),
            "Map should contain docs/api.md"
        );
        assert!(
            !map.contains("# API Documentation"),
            "Map should not contain docs file content"
        );

        // Should not include CSS files
        assert!(
            !map.contains("assets/style.css"),
            "Map should not contain assets/style.css"
        );
        assert!(!map.contains(".class"), "Map should not contain .class");
    }

    #[test]
    fn test_extractors() {
        assert_eq!(extract_id(r#"<div id="test">"#), Some("test"));
        assert_eq!(extract_function_name("fn test_func() {"), Some("test_func"));
        assert_eq!(extract_class_name("class TestClass {"), Some("TestClass"));
        assert_eq!(
            extract_const_name("const TEST_CONST: i32 = 42;"),
            Some("TEST_CONST")
        );
    }
}
