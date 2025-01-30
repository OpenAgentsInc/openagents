use crate::solver::types::{Change, ChangeError, ChangeResult};
use regex::Regex;

/// Parses a string containing SEARCH/REPLACE blocks into a list of changes
pub fn parse_search_replace(content: &str) -> ChangeResult<Vec<Change>> {
    let mut changes = Vec::new();
    let mut current_path = None;
    let mut current_search = String::new();
    let mut current_replace = String::new();
    let mut in_search = false;
    let mut in_replace = false;

    // Extract path and content blocks
    let path_re = Regex::new(r"^([^:\n]+):$").unwrap();
    let search_start = "<<<<<<< SEARCH";
    let search_end = "=======";
    let replace_end = ">>>>>>> REPLACE";

    for line in content.lines() {
        if let Some(caps) = path_re.captures(line) {
            // Save previous change if any
            if let Some(path) = current_path.take() {
                changes.push(Change::new(
                    path,
                    current_search.trim().to_string(),
                    current_replace.trim().to_string(),
                ));
                current_search.clear();
                current_replace.clear();
            }
            current_path = Some(caps[1].to_string());
            in_search = false;
            in_replace = false;
        } else if line == search_start {
            in_search = true;
            in_replace = false;
        } else if line == search_end {
            in_search = false;
            in_replace = true;
        } else if line == replace_end {
            in_search = false;
            in_replace = false;
        } else if in_search {
            current_search.push_str(line);
            current_search.push('\n');
        } else if in_replace {
            current_replace.push_str(line);
            current_replace.push('\n');
        }
    }

    // Save final change if any
    if let Some(path) = current_path {
        changes.push(Change::new(
            path,
            current_search.trim().to_string(),
            current_replace.trim().to_string(),
        ));
    }

    if changes.is_empty() {
        return Err(ChangeError::InvalidFormat);
    }

    Ok(changes)
}