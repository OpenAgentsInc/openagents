use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
}

impl Change {
    pub fn new(path: String, search: String, replace: String) -> Self {
        Self {
            path,
            search,
            replace,
        }
    }

    pub fn validate(&self) -> Result<()> {
        // Path must not be empty
        if self.path.is_empty() {
            return Err(anyhow!("Path cannot be empty"));
        }

        // Path must be relative (not start with /)
        if self.path.starts_with('/') {
            return Err(anyhow!("Path must be relative (not start with /)"));
        }

        // Replace must not be empty
        if self.replace.is_empty() {
            return Err(anyhow!("Replace content cannot be empty"));
        }

        // If search is empty, this is a new file
        if self.search.is_empty() {
            return Ok(());
        }

        // Search must be contained in replace for modifications
        if !self.replace.contains(&self.search) {
            return Err(anyhow!(
                "Replace content must contain original content for modifications"
            ));
        }

        Ok(())
    }
}

pub fn validate_pr_title(title: &str) -> Result<()> {
    // Title must contain one of these words
    if !title.contains("solver") && !title.contains("solution") && !title.contains("PR") {
        return Err(anyhow!(
            "PR title must contain 'solver', 'solution', or 'PR'"
        ));
    }

    // Title must not be too short
    if title.len() < 10 {
        return Err(anyhow!("PR title must be at least 10 characters"));
    }

    Ok(())
}