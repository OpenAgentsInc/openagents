use crate::repo::{cleanup_temp_dir, clone_repository, RepoContext};
use crate::repomap::generate_repo_map;
use crate::server::services::openrouter::service::OpenRouterService;
use crate::solver::Change;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

pub struct SolutionContext {
    pub temp_dir: PathBuf,
    pub repo_context: RepoContext,
    pub modified_files: Vec<String>,
    openrouter: OpenRouterService,
}

impl SolutionContext {
    pub fn new(
        issue_number: i32,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("solver_{}", issue_number));

        // Clean up any existing temp directory first
        if temp_dir.exists() {
            println!("Cleaning up existing temp directory: {:?}", temp_dir);
            fs::remove_dir_all(&temp_dir)?;
        }

        // Create the temporary directory
        println!("Creating temporary directory: {:?}", temp_dir);
        fs::create_dir_all(&temp_dir)?;

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key.clone(), github_token);
        let openrouter = OpenRouterService::new(openrouter_key)?;

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            openrouter,
        })
    }

    pub fn new_with_dir(
        temp_dir: PathBuf,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        // Create the temporary directory if it doesn't exist
        if !temp_dir.exists() {
            println!("Creating temporary directory: {:?}", temp_dir);
            fs::create_dir_all(&temp_dir)?;
        }

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key.clone(), github_token);
        let openrouter = OpenRouterService::new(openrouter_key)?;

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            openrouter,
        })
    }

    pub fn clone_repository(&self, repo_url: &str) -> Result<()> {
        clone_repository(repo_url, &self.repo_context.temp_dir)?;
        Ok(())
    }

    pub fn generate_repo_map(&self) -> String {
        generate_repo_map(&self.repo_context.temp_dir)
    }

    pub fn cleanup(&self) {
        if self.temp_dir.exists() {
            println!("Cleaning up temp directory: {:?}", self.temp_dir);
            let _ = fs::remove_dir_all(&self.temp_dir);
        }
    }

    /// Generate a list of files that need to be modified based on the issue
    pub async fn generate_file_list(
        &self,
        issue_title: &str,
        issue_body: &str,
        repo_map: &str,
    ) -> Result<Vec<String>> {
        let prompt = format!(
            r#"You are an expert software developer tasked with implementing a solution for a GitHub issue.
Based on the issue details and repository structure, list the files that need to be modified.

Issue Title: {}
Issue Description: {}

Repository Structure:
{}

Output a valid JSON array of file paths that need to be modified to implement this solution.
Only include files that actually exist in the repository.
Format: ["path/to/file1", "path/to/file2", ...]"#,
            issue_title, issue_body, repo_map
        );

        let (response, _) = self.openrouter.chat(prompt, false).await?;
        
        // Extract JSON array from response
        let json_str = response
            .lines()
            .find(|line| line.trim().starts_with('['))
            .ok_or_else(|| anyhow!("No JSON array found in response"))?;

        let files: Vec<String> = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse file list: {}", e))?;

        // Validate all files exist
        for file in &files {
            let file_path = self.temp_dir.join(file);
            if !file_path.exists() {
                return Err(anyhow!("Listed file does not exist: {}", file));
            }
        }

        Ok(files)
    }

    /// Generate changes for a specific file based on the issue
    pub async fn generate_changes(
        &self,
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

        let (response, _) = self.openrouter.chat(prompt, false).await?;
        
        // Parse the changes from the response
        let mut changes = Vec::new();
        let mut current_path = None;
        let mut current_search = None;
        let mut in_search = false;
        let mut in_replace = false;
        let mut search_content = String::new();
        let mut replace_content = String::new();

        for line in response.lines() {
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
    pub fn apply_changes(&mut self, changes: &[Change]) -> Result<()> {
        for change in changes {
            let file_path = self.temp_dir.join(&change.path);
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
            if !self.modified_files.contains(&change.path) {
                println!("Adding {} to modified files", change.path);
                self.modified_files.push(change.path.clone());
            }
        }

        Ok(())
    }
}

impl Drop for SolutionContext {
    fn drop(&mut self) {
        self.cleanup();
    }
}