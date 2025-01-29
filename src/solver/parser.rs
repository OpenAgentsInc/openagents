use crate::solver::Change;
use anyhow::{anyhow, Result};

/// Parse SEARCH/REPLACE blocks from LLM output into a list of changes
pub fn parse_changes(content: &str) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let mut current_path = None;
    let mut current_search = None;
    let mut in_search = false;
    let mut in_replace = false;
    let mut search_content = String::new();
    let mut replace_content = String::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Check for file path
        if !line.contains("SEARCH") && !line.contains("REPLACE") && !in_search && !in_replace {
            current_path = Some(line.to_string());
            continue;
        }

        // Handle SEARCH block
        if line.contains("<<<<<<< SEARCH") {
            if current_path.is_none() {
                return Err(anyhow!("Found SEARCH block before file path"));
            }
            in_search = true;
            search_content.clear();
            continue;
        }

        // Handle separator
        if line.contains("=======") {
            if !in_search {
                return Err(anyhow!("Found separator outside of SEARCH/REPLACE block"));
            }
            in_search = false;
            in_replace = true;
            current_search = Some(search_content.clone());
            replace_content.clear();
            continue;
        }

        // Handle REPLACE block end
        if line.contains(">>>>>>> REPLACE") {
            if !in_replace {
                return Err(anyhow!("Found REPLACE end outside of REPLACE block"));
            }
            in_replace = false;

            // Create change
            if let (Some(path), Some(search)) = (&current_path, &current_search) {
                changes.push(Change {
                    path: path.clone(),
                    search: search.clone(),
                    replace: replace_content.clone(),
                });
            }
            continue;
        }

        // Collect content
        if in_search {
            if !search_content.is_empty() {
                search_content.push('\n');
            }
            search_content.push_str(line);
        } else if in_replace {
            if !replace_content.is_empty() {
                replace_content.push('\n');
            }
            replace_content.push_str(line);
        }
    }

    Ok(changes)
}