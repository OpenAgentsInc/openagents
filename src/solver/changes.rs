use crate::server::services::gateway::Gateway;
use crate::solver::{Change, SolutionContext};
use anyhow::{anyhow, Result};
use std::fs;

/// Generate changes for a specific file based on the issue
pub async fn generate_changes(
    gateway: &impl Gateway,
    file_path: &str,
    file_content: &str,
    issue_title: &str,
    issue_body: &str,
) -> Result<Vec<Change>> {
    let prompt = format!(
        r#"You are an expert software developer tasked with implementing a solution for a GitHub issue.
Generate the necessary code changes using SEARCH/REPLACE blocks.

Issue Title: {}
Issue Description: {}

File to modify: {}
Current content:
```rust
{}
```

Output SEARCH/REPLACE blocks for the changes needed in this file.
Use this format:

{}
<<<<<<< SEARCH
[exact lines to find]
=======
[lines to replace them with]
>>>>>>> REPLACE

Rules:
1. SEARCH must contain exact lines from the file (check whitespace)
2. For new content, use empty SEARCH block
3. Break large changes into multiple small blocks
4. Include enough context for unique matches
5. Ensure replacement code is valid Rust

Generate the changes now:"#,
        issue_title,
        issue_body,
        file_path,
        file_content,
        file_path
    );

    let (response, _) = gateway.chat(prompt, false).await?;
    
    // Parse the changes from the response
    let changes = crate::solver::parser::parse_changes(&response)?;

    // Validate all changes reference this file
    for change in &changes {
        if change.path != file_path {
            return Err(anyhow!(
                "Change references wrong file: {} (expected {})",
                change.path,
                file_path
            ));
        }
    }

    Ok(changes)
}

/// Apply a list of changes to files in the temporary directory
pub fn apply_changes(context: &mut SolutionContext, changes: &[Change]) -> Result<()> {
    for change in changes {
        let file_path = context.temp_dir.join(&change.path);
        println!("Processing change for file: {:?}", file_path);
        
        // Check if file exists (except for empty search blocks which create new files)
        if !file_path.exists() && !change.search.trim().is_empty() {
            println!("File not found: {}", change.path);
            return Err(anyhow!("File not found: {}", change.path));
        }

        // Read existing content or create new file
        let current_content = if file_path.exists() {
            println!("Reading existing file: {:?}", file_path);
            fs::read_to_string(&file_path)?
        } else {
            println!("Creating new file: {:?}", file_path);
            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            String::new()
        };

        // Apply the change
        let new_content = if change.search.trim().is_empty() {
            println!("Empty search block - appending content");
            // Empty search block means append/create
            if current_content.trim().is_empty() {
                change.replace.clone()
            } else {
                format!("{}\n{}", current_content.trim(), change.replace)
            }
        } else {
            println!("Searching for content to replace");
            println!("Search pattern:\n{}", change.search);
            println!("Current content:\n{}", current_content);

            // First try exact match
            if current_content.contains(&change.search) {
                println!("Found exact match");
                let start_idx = current_content.find(&change.search).unwrap();
                let end_idx = start_idx + change.search.len();
                format!(
                    "{}{}{}",
                    &current_content[..start_idx],
                    change.replace,
                    &current_content[end_idx..]
                )
            } else {
                // Try matching with normalized whitespace
                println!("No exact match, trying with normalized whitespace");
                let search_lines: Vec<_> = change.search.lines().map(|l| l.trim()).collect();
                let current_lines: Vec<_> = current_content.lines().map(|l| l.trim()).collect();

                let mut found_match = false;
                let mut start_line = 0;
                let mut end_line = 0;

                'outer: for (i, window) in current_lines.windows(search_lines.len()).enumerate() {
                    let mut matches = true;
                    for (a, b) in window.iter().zip(search_lines.iter()) {
                        let a_norm = a.split_whitespace().collect::<Vec<_>>().join(" ");
                        let b_norm = b.split_whitespace().collect::<Vec<_>>().join(" ");
                        if a_norm != b_norm {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        println!("Found matching block at line {}", i);
                        found_match = true;
                        start_line = i;
                        end_line = i + search_lines.len();
                        break 'outer;
                    }
                }

                if !found_match {
                    println!("No matching content found");
                    return Err(anyhow!(
                        "No matching content found in {}",
                        change.path
                    ));
                }

                // Reconstruct the content
                let mut result = String::new();
                
                // Add lines before the match
                for line in current_content.lines().take(start_line) {
                    result.push_str(line);
                    result.push('\n');
                }

                // Add the replacement
                result.push_str(&change.replace);
                result.push('\n');

                // Add lines after the match
                for line in current_content.lines().skip(end_line) {
                    result.push_str(line);
                    result.push('\n');
                }

                result
            }
        };

        println!("Writing new content:\n{}", new_content);
        // Write the modified content
        fs::write(&file_path, new_content)?;

        // Track modified file
        if !context.modified_files.contains(&change.path) {
            println!("Adding {} to modified files", change.path);
            context.modified_files.push(change.path.clone());
        }
    }

    Ok(())
}