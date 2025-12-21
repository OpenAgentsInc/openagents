//! Concrete benchmark task implementations

use super::{BenchmarkTask, ValidationResult};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// B-001: Simple File Edit
///
/// Task: Change the version number in version.txt from 1.0.0 to 1.0.1
pub struct B001SimpleFileEdit;

impl BenchmarkTask for B001SimpleFileEdit {
    fn id(&self) -> &str {
        "B-001"
    }

    fn name(&self) -> &str {
        "Simple File Edit"
    }

    fn category(&self) -> &str {
        "file-ops"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        let version_file = workspace.join("version.txt");
        std::fs::write(&version_file, "version = 1.0.0\n")
            .context("Failed to create version.txt")?;
        Ok(())
    }

    fn prompt(&self) -> &str {
        "Change the version number in version.txt from 1.0.0 to 1.0.1"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let version_file = workspace.join("version.txt");
        let content = std::fs::read_to_string(&version_file)
            .context("Failed to read version.txt")?;

        let success = content.contains("1.0.1");
        let mut messages = Vec::new();

        if success {
            messages.push("✓ Version correctly updated to 1.0.1".to_string());
        } else {
            messages.push(format!("✗ Expected '1.0.1', found: {}", content.trim()));
        }

        Ok(ValidationResult {
            success,
            messages,
            custom_metrics: HashMap::new(),
        })
    }

    fn teardown(&self, workspace: &Path) -> Result<()> {
        if workspace.exists() {
            std::fs::remove_dir_all(workspace)?;
        }
        Ok(())
    }
}

/// B-002: Multi-File Search and Edit
///
/// Task: Replace all occurrences of 'OLD_API' with 'NEW_API' across all .rs files
pub struct B002MultiFileEdit;

impl BenchmarkTask for B002MultiFileEdit {
    fn id(&self) -> &str {
        "B-002"
    }

    fn name(&self) -> &str {
        "Multi-File Search and Edit"
    }

    fn category(&self) -> &str {
        "file-ops"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create 10 Rust files with 5 occurrences each
        for i in 1..=10 {
            let file_path = workspace.join(format!("file{}.rs", i));
            let content = format!(
                "// File {}\n\
                 const API1: &str = \"OLD_API\";\n\
                 const API2: &str = \"OLD_API\";\n\
                 const API3: &str = \"OLD_API\";\n\
                 const API4: &str = \"OLD_API\";\n\
                 const API5: &str = \"OLD_API\";\n",
                i
            );
            std::fs::write(&file_path, content)
                .with_context(|| format!("Failed to create file{}.rs", i))?;
        }
        Ok(())
    }

    fn prompt(&self) -> &str {
        "Replace all occurrences of 'OLD_API' with 'NEW_API' across all .rs files"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut total_occurrences = 0;
        let mut files_with_old_api = Vec::new();
        let mut messages = Vec::new();

        for i in 1..=10 {
            let file_path = workspace.join(format!("file{}.rs", i));
            let content = std::fs::read_to_string(&file_path)
                .with_context(|| format!("Failed to read file{}.rs", i))?;

            let new_count = content.matches("NEW_API").count();
            let old_count = content.matches("OLD_API").count();

            total_occurrences += new_count;

            if old_count > 0 {
                files_with_old_api.push(format!("file{}.rs ({} remaining)", i, old_count));
            }
        }

        let success = total_occurrences == 50 && files_with_old_api.is_empty();

        if success {
            messages.push("✓ All 50 occurrences successfully replaced".to_string());
        } else {
            messages.push(format!(
                "✗ Expected 50 replacements, found {}. Files with OLD_API: {}",
                total_occurrences,
                files_with_old_api.join(", ")
            ));
        }

        let mut custom_metrics = HashMap::new();
        custom_metrics.insert("replacements".to_string(), total_occurrences as f64);
        custom_metrics.insert(
            "files_missed".to_string(),
            files_with_old_api.len() as f64,
        );

        Ok(ValidationResult {
            success,
            messages,
            custom_metrics,
        })
    }

    fn teardown(&self, workspace: &Path) -> Result<()> {
        if workspace.exists() {
            std::fs::remove_dir_all(workspace)?;
        }
        Ok(())
    }
}

/// B-003: Complex Refactoring
///
/// Task: Rename struct User to Account and update all references
pub struct B003StructRename;

impl BenchmarkTask for B003StructRename {
    fn id(&self) -> &str {
        "B-003"
    }

    fn name(&self) -> &str {
        "Complex Refactoring"
    }

    fn category(&self) -> &str {
        "file-ops"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create struct definition
        std::fs::write(
            workspace.join("types.rs"),
            "pub struct User {\n\
             pub id: u64,\n\
             pub name: String,\n\
             }\n",
        )?;

        // Create files with references
        std::fs::write(
            workspace.join("handlers.rs"),
            "use crate::types::User;\n\
             \n\
             pub fn get_user() -> User {\n\
             User { id: 1, name: \"test\".to_string() }\n\
             }\n\
             \n\
             pub fn create_user(user: User) -> User {\n\
             user\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("db.rs"),
            "use crate::types::User;\n\
             \n\
             pub fn save_user(user: &User) {}\n\
             pub fn load_user() -> User {\n\
             User { id: 1, name: \"test\".to_string() }\n\
             }\n",
        )?;

        // Create Cargo.toml for compilation check
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"refactor-test\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        // Create lib.rs
        std::fs::write(
            workspace.join("lib.rs"),
            "pub mod types;\n\
             pub mod handlers;\n\
             pub mod db;\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Rename struct User to Account and update all references across the codebase"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check that User was renamed to Account
        let types_content = std::fs::read_to_string(workspace.join("types.rs"))?;
        let has_account = types_content.contains("struct Account");
        let has_user = types_content.contains("struct User");

        if has_account && !has_user {
            messages.push("✓ Struct renamed from User to Account in types.rs".to_string());
        } else {
            messages.push(format!(
                "✗ Struct rename incomplete: has_account={}, has_user={}",
                has_account, has_user
            ));
        }

        // Check all references were updated
        let handlers_content = std::fs::read_to_string(workspace.join("handlers.rs"))?;
        let db_content = std::fs::read_to_string(workspace.join("db.rs"))?;

        let user_refs_handlers = handlers_content.matches("User").count();
        let user_refs_db = db_content.matches("User").count();
        let account_refs_handlers = handlers_content.matches("Account").count();
        let account_refs_db = db_content.matches("Account").count();

        messages.push(format!(
            "Handlers: {} Account refs, {} User refs",
            account_refs_handlers, user_refs_handlers
        ));
        messages.push(format!(
            "DB: {} Account refs, {} User refs",
            account_refs_db, user_refs_db
        ));

        custom_metrics.insert("user_refs_remaining".to_string(), (user_refs_handlers + user_refs_db) as f64);
        custom_metrics.insert("account_refs".to_string(), (account_refs_handlers + account_refs_db) as f64);

        // Check compilation (optional - requires cargo)
        let compiles = std::process::Command::new("cargo")
            .arg("check")
            .current_dir(workspace)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if compiles {
            messages.push("✓ Code compiles successfully".to_string());
        } else {
            messages.push("✗ Code does not compile".to_string());
        }

        custom_metrics.insert("compiles".to_string(), if compiles { 1.0 } else { 0.0 });

        let success = has_account
            && !has_user
            && user_refs_handlers == 0
            && user_refs_db == 0
            && compiles;

        Ok(ValidationResult {
            success,
            messages,
            custom_metrics,
        })
    }

    fn teardown(&self, workspace: &Path) -> Result<()> {
        if workspace.exists() {
            std::fs::remove_dir_all(workspace)?;
        }
        Ok(())
    }
}

/// B-004: Simple Commit
///
/// Task: Commit the changes with message 'Update version'
pub struct B004SimpleCommit;

impl BenchmarkTask for B004SimpleCommit {
    fn id(&self) -> &str {
        "B-004"
    }

    fn name(&self) -> &str {
        "Simple Commit"
    }

    fn category(&self) -> &str {
        "git"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(workspace)
            .output()
            .context("Failed to init git repo")?;

        // Configure git
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(workspace)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(workspace)
            .output()?;

        // Create and commit initial file
        std::fs::write(workspace.join("version.txt"), "1.0.0\n")?;

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(workspace)
            .output()?;

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(workspace)
            .output()?;

        // Modify the file
        std::fs::write(workspace.join("version.txt"), "1.0.1\n")?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Commit the changes with message 'Update version'"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();

        // Check that there's a commit
        let output = std::process::Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(workspace)
            .output()
            .context("Failed to run git log")?;

        let log = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<&str> = log.lines().collect();

        messages.push(format!("Total commits: {}", commits.len()));

        // Check for the commit message
        let has_update_commit = log.contains("Update version");

        if has_update_commit {
            messages.push("✓ Found commit with message 'Update version'".to_string());
        } else {
            messages.push("✗ No commit with message 'Update version' found".to_string());
        }

        // Check working tree is clean
        let status_output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(workspace)
            .output()?;

        let status = String::from_utf8_lossy(&status_output.stdout);
        let is_clean = status.trim().is_empty();

        if is_clean {
            messages.push("✓ Working tree is clean".to_string());
        } else {
            messages.push(format!("✗ Working tree has uncommitted changes: {}", status));
        }

        let success = has_update_commit && is_clean && commits.len() == 2;

        Ok(ValidationResult {
            success,
            messages,
            custom_metrics: HashMap::new(),
        })
    }

    fn teardown(&self, workspace: &Path) -> Result<()> {
        if workspace.exists() {
            std::fs::remove_dir_all(workspace)?;
        }
        Ok(())
    }
}

/// B-005: Branch and PR Workflow
///
/// Task: Create branch 'feature-x', commit changes, push (simulated)
pub struct B005BranchWorkflow;

impl BenchmarkTask for B005BranchWorkflow {
    fn id(&self) -> &str {
        "B-005"
    }

    fn name(&self) -> &str {
        "Branch and PR Workflow"
    }

    fn category(&self) -> &str {
        "git"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(workspace)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(workspace)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(workspace)
            .output()?;

        // Create initial commit on main
        std::fs::write(workspace.join("README.md"), "# Project\n")?;

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(workspace)
            .output()?;

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(workspace)
            .output()?;

        // Create a file to be modified
        std::fs::write(workspace.join("feature.txt"), "initial\n")?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Create a new branch called 'feature-x', modify feature.txt to say 'updated', \
         commit with message 'Add feature X', and switch back to main"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check that branch exists
        let branch_output = std::process::Command::new("git")
            .args(["branch"])
            .current_dir(workspace)
            .output()?;

        let branches = String::from_utf8_lossy(&branch_output.stdout);
        let has_feature_branch = branches.contains("feature-x");

        if has_feature_branch {
            messages.push("✓ Branch 'feature-x' exists".to_string());
        } else {
            messages.push("✗ Branch 'feature-x' not found".to_string());
            messages.push(format!("Available branches: {}", branches));
        }

        // Check current branch is main
        let current_branch_output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(workspace)
            .output()?;

        let current_branch = String::from_utf8_lossy(&current_branch_output.stdout).trim().to_string();
        let on_main = current_branch == "main" || current_branch == "master";

        if on_main {
            messages.push(format!("✓ Currently on {} branch", current_branch));
        } else {
            messages.push(format!("✗ Not on main branch (on: {})", current_branch));
        }

        // Check commit exists on feature branch
        let log_output = std::process::Command::new("git")
            .args(["log", "--all", "--oneline"])
            .current_dir(workspace)
            .output()?;

        let log = String::from_utf8_lossy(&log_output.stdout);
        let has_feature_commit = log.contains("Add feature X");

        if has_feature_commit {
            messages.push("✓ Commit 'Add feature X' found".to_string());
        } else {
            messages.push("✗ Commit 'Add feature X' not found".to_string());
        }

        custom_metrics.insert("has_branch".to_string(), if has_feature_branch { 1.0 } else { 0.0 });
        custom_metrics.insert("on_main".to_string(), if on_main { 1.0 } else { 0.0 });

        let success = has_feature_branch && on_main && has_feature_commit;

        Ok(ValidationResult {
            success,
            messages,
            custom_metrics,
        })
    }

    fn teardown(&self, workspace: &Path) -> Result<()> {
        if workspace.exists() {
            std::fs::remove_dir_all(workspace)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_b001_setup_and_validate() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workspace = temp_dir.path();

        let task = B001SimpleFileEdit;
        task.setup(workspace)?;

        // Validate initial state should fail
        let result = task.validate(workspace)?;
        assert!(!result.success);

        // Manually fix
        std::fs::write(workspace.join("version.txt"), "version = 1.0.1\n")?;

        // Validate fixed state should pass
        let result = task.validate(workspace)?;
        assert!(result.success);

        task.teardown(workspace)?;
        Ok(())
    }

    #[test]
    fn test_b002_multi_file_edit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let workspace = temp_dir.path();

        let task = B002MultiFileEdit;
        task.setup(workspace)?;

        // Manually replace in all files
        for i in 1..=10 {
            let file_path = workspace.join(format!("file{}.rs", i));
            let content = std::fs::read_to_string(&file_path)?;
            let updated = content.replace("OLD_API", "NEW_API");
            std::fs::write(&file_path, updated)?;
        }

        let result = task.validate(workspace)?;
        assert!(result.success);
        assert_eq!(result.custom_metrics.get("replacements"), Some(&50.0));

        task.teardown(workspace)?;
        Ok(())
    }
}
