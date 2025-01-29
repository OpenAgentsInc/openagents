use crate::solver::types::{Change, ChangeError, ChangeResult};
use tracing::{debug, error};

/// Parses SEARCH/REPLACE blocks from text
pub fn parse_search_replace(content: &str) -> ChangeResult<Vec<Change>> {
    let mut changes = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_search: Option<String> = None;
    let mut current_replace = String::new();
    let mut in_search = false;
    let mut in_replace = false;

    debug!("Parsing SEARCH/REPLACE blocks from:\n{}", content);

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim_end();
        debug!("Line {}: {}", line_num + 1, line);

        // Look for file path markers
        if line.ends_with(".rs:") || line.ends_with(".toml:") {
            debug!("Found file path marker: {}", line);
            // Save previous change if any
            if let (Some(path), Some(search)) = (&current_path, &current_search) {
                if !current_replace.is_empty() {
                    let change = Change::new(
                        path.clone(),
                        search.clone(),
                        current_replace.trim().to_string(),
                    );
                    match change.validate() {
                        Ok(_) => {
                            debug!("Valid change: {:?}", change);
                            changes.push(change);
                        }
                        Err(e) => {
                            error!("Invalid change for {}: {:?}", path, e);
                        }
                    }
                }
            }

            // Reset state for new file
            current_path = Some(line.trim_end_matches(':').to_string());
            current_search = None;
            current_replace.clear();
            in_search = false;
            in_replace = false;
            continue;
        }

        // Handle SEARCH/REPLACE markers
        match line {
            "<<<<<<< SEARCH" => {
                debug!("Found SEARCH marker");
                in_search = true;
                current_search = Some(String::new());
                continue;
            }
            "=======" => {
                debug!("Found separator marker");
                in_search = false;
                in_replace = true;
                current_replace.clear();
                continue;
            }
            ">>>>>>> REPLACE" => {
                debug!("Found REPLACE marker");
                in_replace = false;
                // Save the change
                if let (Some(path), Some(search)) = (&current_path, &current_search) {
                    let change = Change::new(
                        path.clone(),
                        search.clone(),
                        current_replace.trim().to_string(),
                    );
                    match change.validate() {
                        Ok(_) => {
                            debug!("Valid change: {:?}", change);
                            changes.push(change);
                        }
                        Err(e) => {
                            error!("Invalid change for {}: {:?}", path, e);
                        }
                    }
                }
                current_search = None;
                current_replace.clear();
                continue;
            }
            _ => {}
        }

        // Collect content
        if in_search {
            if let Some(ref mut search) = current_search {
                if !search.is_empty() {
                    search.push('\n');
                }
                search.push_str(line);
                debug!("Added to search: {}", line);
            }
        } else if in_replace {
            if !current_replace.is_empty() {
                current_replace.push('\n');
            }
            current_replace.push_str(line);
            debug!("Added to replace: {}", line);
        }
    }

    // Handle any remaining change
    if let (Some(path), Some(search)) = (&current_path, &current_search) {
        if !current_replace.is_empty() {
            let change = Change::new(
                path.clone(),
                search.clone(),
                current_replace.trim().to_string(),
            );
            match change.validate() {
                Ok(_) => {
                    debug!("Valid change: {:?}", change);
                    changes.push(change);
                }
                Err(e) => {
                    error!("Invalid change for {}: {:?}", path, e);
                }
            }
        }
    }

    if changes.is_empty() {
        error!("No valid changes found in content");
        return Err(ChangeError::InvalidFormat);
    }

    debug!("Successfully parsed {} changes", changes.len());
    Ok(changes)
}
