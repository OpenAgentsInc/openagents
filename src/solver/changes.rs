use anyhow::Result;
use std::fs;
use std::path::Path;

/// Normalize a line by trimming whitespace and joining multiple spaces
pub fn normalize_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Check if two blocks of code match, ignoring whitespace differences
pub fn blocks_match(a: &str, b: &str) -> bool {
    let a_lines: Vec<_> = a.lines().map(normalize_line).collect();
    let b_lines: Vec<_> = b.lines().map(normalize_line).collect();
    a_lines == b_lines
}

/// Apply a change to a file
pub fn apply_change(file_path: &Path, search: &str, replace: &str) -> Result<()> {
    let content = fs::read_to_string(file_path)?;
    
    // Handle empty search block (new file/append)
    if search.trim().is_empty() {
        if content.trim().is_empty() {
            fs::write(file_path, replace)?;
        } else {
            let new_content = format!("{}\n{}", content.trim(), replace);
            fs::write(file_path, new_content)?;
        }
        return Ok(());
    }

    // Try exact match first
    if content.contains(search) {
        let new_content = content.replace(search, replace);
        fs::write(file_path, new_content)?;
        return Ok(());
    }

    // Try matching with normalized whitespace
    let search_lines: Vec<_> = search.lines().collect();
    let content_lines: Vec<_> = content.lines().collect();

    for window in content_lines.windows(search_lines.len()) {
        if blocks_match(&window.join("\n"), search) {
            let start_idx = content_lines.iter().position(|&l| l == window[0]).unwrap();
            let end_idx = start_idx + search_lines.len();

            let mut new_content = String::new();
            
            // Add lines before match
            for line in &content_lines[..start_idx] {
                new_content.push_str(line);
                new_content.push('\n');
            }

            // Add replacement
            new_content.push_str(replace);
            new_content.push('\n');

            // Add lines after match
            for line in &content_lines[end_idx..] {
                new_content.push_str(line);
                new_content.push('\n');
            }

            fs::write(file_path, new_content)?;
            return Ok(());
        }
    }

    Err(anyhow::anyhow!(
        "No matching content found in {}",
        file_path.display()
    ))
}