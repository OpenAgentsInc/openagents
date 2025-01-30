use crate::solver::changes::parsing::parse_search_replace;
use crate::solver::file_list::generate_file_list;
use crate::solver::github::GitHubContext;
use crate::solver::types::{Change, ChangeError, ChangeResult};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error};

pub struct SolverContext {
    pub temp_dir: PathBuf,
    pub github: Option<GitHubContext>,
}

impl SolverContext {
    pub fn new() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?.into_path();
        Ok(Self {
            temp_dir,
            github: None,
        })
    }

    pub fn new_with_dir(temp_dir: PathBuf) -> Self {
        Self {
            temp_dir,
            github: None,
        }
    }

    pub fn with_github(mut self, github: GitHubContext) -> Self {
        self.github = Some(github);
        self
    }

    pub async fn create_branch(&self, branch_name: &str, base_branch: &str) -> Result<()> {
        if let Some(github) = &self.github {
            github.create_branch(branch_name, base_branch).await?;
        }
        Ok(())
    }

    pub async fn create_pull_request(
        &self,
        branch_name: &str,
        base_branch: &str,
        context: &str,
        description: &str,
        issue_number: i32,
    ) -> Result<()> {
        if let Some(github) = &self.github {
            github
                .create_pull_request(branch_name, base_branch, context, description, issue_number)
                .await?;
        }
        Ok(())
    }

    pub async fn generate_file_list(
        &self,
        title: &str,
        description: &str,
        repo_map: &str,
        ollama_url: &str,
    ) -> Result<(Vec<String>, String)> {
        generate_file_list(title, description, repo_map, ollama_url).await
    }

    pub async fn generate_changes(
        &self,
        path: &str,
        content: &str,
        title: &str,
        description: &str,
        ollama_url: &str,
    ) -> Result<(Vec<Change>, String)> {
        crate::solver::changes::generation::generate_changes(
            path,
            content,
            title,
            description,
            ollama_url,
        )
        .await
    }

    pub fn parse_changes(&self, content: &str) -> ChangeResult<Vec<Change>> {
        parse_search_replace(content)
    }

    pub fn apply_changes(&self, changes: &[Change]) -> ChangeResult<()> {
        for change in changes {
            let path = self.temp_dir.join(&change.path);

            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(ChangeError::IoError)?;
            }

            // Read existing content if file exists
            let current_content = if path.exists() {
                fs::read_to_string(&path).map_err(ChangeError::IoError)?
            } else if !change.search.is_empty() {
                return Err(ChangeError::FileNotFound(path));
            } else {
                String::new()
            };

            // For new files, just write the content
            if change.search.is_empty() {
                debug!("Creating new file: {}", change.path);
                fs::write(&path, &change.replace).map_err(ChangeError::IoError)?;
                continue;
            }

            // For existing files, replace content
            if !current_content.contains(&change.search) {
                error!(
                    "Search content not found in {}\nSearch: {}\nCurrent: {}",
                    change.path, change.search, current_content
                );
                return Err(ChangeError::NoMatch);
            }

            let new_content = current_content.replace(&change.search, &change.replace);
            debug!("Writing changes to: {}", change.path);
            fs::write(&path, new_content).map_err(ChangeError::IoError)?;
        }

        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        // Just remove the temp directory
        if self.temp_dir.exists() {
            fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_apply_changes() {
        let temp_dir = tempdir().unwrap();
        let context = SolverContext::new_with_dir(temp_dir.path().to_path_buf());

        // Test new file creation
        let changes = vec![Change::with_reason(
            "test.rs".to_string(),
            "".to_string(),
            "fn test() {}".to_string(),
            "Add test function".to_string(),
        )];
        assert!(context.apply_changes(&changes).is_ok());

        // Test content modification
        let changes = vec![Change::with_reason(
            "test.rs".to_string(),
            "fn test() {}".to_string(),
            "fn test() { println!(\"test\"); }".to_string(),
            "Add print statement".to_string(),
        )];
        assert!(context.apply_changes(&changes).is_ok());
    }

    #[test]
    fn test_cleanup() {
        let temp_dir = tempdir().unwrap();
        let context = SolverContext::new_with_dir(temp_dir.path().to_path_buf());

        // Create some test files
        let test_file = context.temp_dir.join("test.txt");
        fs::write(&test_file, "test").unwrap();

        // Cleanup should remove everything
        assert!(context.cleanup().is_ok());
        assert!(!test_file.exists());
    }
}