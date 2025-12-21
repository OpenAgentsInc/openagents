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

/// B-006: Issue Workflow
///
/// Task: Claim an issue, implement a simple fix, and mark it complete
pub struct B006IssueWorkflow;

impl BenchmarkTask for B006IssueWorkflow {
    fn id(&self) -> &str {
        "B-006"
    }

    fn name(&self) -> &str {
        "Issue Workflow"
    }

    fn category(&self) -> &str {
        "autopilot"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create a simple autopilot.db with one issue
        let db_path = workspace.join("autopilot.db");
        let conn = rusqlite::Connection::open(&db_path)
            .context("Failed to create autopilot.db")?;

        // Create minimal schema
        conn.execute(
            "CREATE TABLE issues (
                number INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL,
                claimed_by TEXT
            )",
            [],
        )?;

        // Insert one issue
        conn.execute(
            "INSERT INTO issues (number, title, description, status, claimed_by)
             VALUES (1, 'Fix typo in README', 'Change \"teh\" to \"the\" in README.md', 'open', NULL)",
            [],
        )?;

        // Create README with typo
        std::fs::write(workspace.join("README.md"), "# Project\n\nThis is teh README.\n")?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "There's an issue #1 in autopilot.db about fixing a typo. Fix the typo in README.md and update the issue status to 'done'"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check README was fixed
        let readme_content = std::fs::read_to_string(workspace.join("README.md"))?;
        let typo_fixed = !readme_content.contains("teh") && readme_content.contains("the");

        if typo_fixed {
            messages.push("✓ Typo fixed in README".to_string());
        } else {
            messages.push("✗ Typo still present in README".to_string());
        }

        // Check issue status was updated
        let db_path = workspace.join("autopilot.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let status: String = conn.query_row(
            "SELECT status FROM issues WHERE number = 1",
            [],
            |row| row.get(0),
        )?;

        let status_updated = status == "done";
        if status_updated {
            messages.push("✓ Issue #1 marked as done".to_string());
        } else {
            messages.push(format!("✗ Issue #1 status is '{}', not 'done'", status));
        }

        custom_metrics.insert("typo_fixed".to_string(), if typo_fixed { 1.0 } else { 0.0 });
        custom_metrics.insert("status_updated".to_string(), if status_updated { 1.0 } else { 0.0 });

        Ok(ValidationResult {
            success: typo_fixed && status_updated,
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

/// B-007: Multi-Step Refactor
///
/// Task: Extract common logic into a shared utility module
pub struct B007MultiStepRefactor;

impl BenchmarkTask for B007MultiStepRefactor {
    fn id(&self) -> &str {
        "B-007"
    }

    fn name(&self) -> &str {
        "Multi-Step Refactor"
    }

    fn category(&self) -> &str {
        "file-ops"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create two modules with duplicated validation logic
        std::fs::write(
            workspace.join("auth.rs"),
            "pub fn validate_user(name: &str) -> bool {\n\
             if name.is_empty() { return false; }\n\
             if name.len() < 3 { return false; }\n\
             true\n\
             }\n\
             \n\
             pub fn create_user(name: &str) {\n\
             if !validate_user(name) { panic!(\"Invalid user\"); }\n\
             println!(\"Creating user: {}\", name);\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("admin.rs"),
            "pub fn validate_admin(name: &str) -> bool {\n\
             if name.is_empty() { return false; }\n\
             if name.len() < 3 { return false; }\n\
             true\n\
             }\n\
             \n\
             pub fn create_admin(name: &str) {\n\
             if !validate_admin(name) { panic!(\"Invalid admin\"); }\n\
             println!(\"Creating admin: {}\", name);\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"refactor-test\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        std::fs::write(
            workspace.join("lib.rs"),
            "pub mod auth;\n\
             pub mod admin;\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Extract the duplicated validation logic from auth.rs and admin.rs into a new shared 'utils.rs' module. Update both modules to use the shared function."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check if utils.rs was created
        let utils_path = workspace.join("utils.rs");
        let utils_exists = utils_path.exists();

        if utils_exists {
            messages.push("✓ utils.rs module created".to_string());
            let utils_content = std::fs::read_to_string(&utils_path)?;
            let has_validation_fn = utils_content.contains("pub fn validate_name")
                || utils_content.contains("pub fn validate");

            if has_validation_fn {
                messages.push("✓ Validation function found in utils.rs".to_string());
            } else {
                messages.push("✗ No validation function in utils.rs".to_string());
            }
            custom_metrics.insert("has_validation_fn".to_string(), if has_validation_fn { 1.0 } else { 0.0 });
        } else {
            messages.push("✗ utils.rs module not created".to_string());
            custom_metrics.insert("has_validation_fn".to_string(), 0.0);
        }

        // Check if lib.rs includes utils module
        let lib_content = std::fs::read_to_string(workspace.join("lib.rs"))?;
        let includes_utils = lib_content.contains("mod utils");
        if includes_utils {
            messages.push("✓ utils module added to lib.rs".to_string());
        } else {
            messages.push("✗ utils module not added to lib.rs".to_string());
        }

        // Check if auth.rs and admin.rs use the utils module
        let auth_content = std::fs::read_to_string(workspace.join("auth.rs"))?;
        let admin_content = std::fs::read_to_string(workspace.join("admin.rs"))?;

        let auth_uses_utils = auth_content.contains("use") && auth_content.contains("utils");
        let admin_uses_utils = admin_content.contains("use") && admin_content.contains("utils");

        if auth_uses_utils {
            messages.push("✓ auth.rs uses utils module".to_string());
        }
        if admin_uses_utils {
            messages.push("✓ admin.rs uses utils module".to_string());
        }

        // Check compilation
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

        custom_metrics.insert("utils_created".to_string(), if utils_exists { 1.0 } else { 0.0 });
        custom_metrics.insert("compiles".to_string(), if compiles { 1.0 } else { 0.0 });

        let success = utils_exists && includes_utils && compiles;

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

/// B-008: Test-Driven Fix
///
/// Task: Fix a bug so the failing test passes
pub struct B008TestDrivenFix;

impl BenchmarkTask for B008TestDrivenFix {
    fn id(&self) -> &str {
        "B-008"
    }

    fn name(&self) -> &str {
        "Test-Driven Fix"
    }

    fn category(&self) -> &str {
        "testing"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create buggy implementation
        std::fs::write(
            workspace.join("calculator.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n\
             a - b  // Bug: using subtraction instead of addition\n\
             }\n\
             \n\
             pub fn multiply(a: i32, b: i32) -> i32 {\n\
             a * b\n\
             }\n",
        )?;

        // Create test that fails
        std::fs::write(
            workspace.join("tests.rs"),
            "#[cfg(test)]\n\
             mod tests {\n\
             use super::calculator::*;\n\
             \n\
             #[test]\n\
             fn test_add() {\n\
             assert_eq!(add(2, 3), 5);\n\
             assert_eq!(add(10, 5), 15);\n\
             }\n\
             \n\
             #[test]\n\
             fn test_multiply() {\n\
             assert_eq!(multiply(2, 3), 6);\n\
             }\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"calculator\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        std::fs::write(
            workspace.join("lib.rs"),
            "pub mod calculator;\n\
             pub mod tests;\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "The test_add test is failing. Fix the bug in calculator.rs so all tests pass."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Run tests
        let test_output = std::process::Command::new("cargo")
            .args(["test", "--", "--nocapture"])
            .current_dir(workspace)
            .output()?;

        let tests_pass = test_output.status.success();
        let output_str = String::from_utf8_lossy(&test_output.stdout);

        if tests_pass {
            messages.push("✓ All tests pass".to_string());
        } else {
            messages.push("✗ Tests still failing".to_string());
            messages.push(format!("Test output: {}", output_str));
        }

        // Check if the bug was actually fixed (not just tests commented out)
        let calculator_content = std::fs::read_to_string(workspace.join("calculator.rs"))?;
        let bug_fixed = calculator_content.contains("a + b") || calculator_content.contains("a+b");

        if bug_fixed {
            messages.push("✓ Bug fixed (using addition)".to_string());
        } else {
            messages.push("✗ Bug not properly fixed".to_string());
        }

        custom_metrics.insert("tests_pass".to_string(), if tests_pass { 1.0 } else { 0.0 });
        custom_metrics.insert("bug_fixed".to_string(), if bug_fixed { 1.0 } else { 0.0 });

        Ok(ValidationResult {
            success: tests_pass && bug_fixed,
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

/// B-009: Documentation Generation
///
/// Task: Add rustdoc comments to all public functions
pub struct B009DocumentationGeneration;

impl BenchmarkTask for B009DocumentationGeneration {
    fn id(&self) -> &str {
        "B-009"
    }

    fn name(&self) -> &str {
        "Documentation Generation"
    }

    fn category(&self) -> &str {
        "docs"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create module with undocumented functions
        std::fs::write(
            workspace.join("math.rs"),
            "pub fn square(x: i32) -> i32 {\n\
             x * x\n\
             }\n\
             \n\
             pub fn cube(x: i32) -> i32 {\n\
             x * x * x\n\
             }\n\
             \n\
             pub fn is_even(x: i32) -> bool {\n\
             x % 2 == 0\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"math-lib\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        std::fs::write(
            workspace.join("lib.rs"),
            "pub mod math;\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Add rustdoc comments (///) to all three public functions in math.rs explaining what they do"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        let math_content = std::fs::read_to_string(workspace.join("math.rs"))?;

        // Count doc comments
        let doc_comment_count = math_content.matches("///").count();
        let has_square_doc = math_content.contains("/// ") &&
            (math_content[..math_content.find("fn square").unwrap_or(0)].contains("///"));
        let has_cube_doc = math_content.contains("/// ") &&
            (math_content[..math_content.find("fn cube").unwrap_or(0)].contains("///"));
        let has_is_even_doc = math_content.contains("/// ") &&
            (math_content[..math_content.find("fn is_even").unwrap_or(0)].contains("///"));

        if has_square_doc {
            messages.push("✓ square() has documentation".to_string());
        } else {
            messages.push("✗ square() missing documentation".to_string());
        }

        if has_cube_doc {
            messages.push("✓ cube() has documentation".to_string());
        } else {
            messages.push("✗ cube() missing documentation".to_string());
        }

        if has_is_even_doc {
            messages.push("✓ is_even() has documentation".to_string());
        } else {
            messages.push("✗ is_even() missing documentation".to_string());
        }

        // Try to build docs
        let doc_output = std::process::Command::new("cargo")
            .args(["doc", "--no-deps"])
            .current_dir(workspace)
            .output()?;

        let doc_builds = doc_output.status.success();
        if doc_builds {
            messages.push("✓ cargo doc builds successfully".to_string());
        } else {
            messages.push("✗ cargo doc failed".to_string());
        }

        custom_metrics.insert("doc_comments".to_string(), doc_comment_count as f64);
        custom_metrics.insert("documented_functions".to_string(),
            [has_square_doc, has_cube_doc, has_is_even_doc].iter().filter(|&&x| x).count() as f64);

        let success = has_square_doc && has_cube_doc && has_is_even_doc && doc_builds;

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

/// B-010: Dependency Update
///
/// Task: Update a dependency version in Cargo.toml
pub struct B010DependencyUpdate;

impl BenchmarkTask for B010DependencyUpdate {
    fn id(&self) -> &str {
        "B-010"
    }

    fn name(&self) -> &str {
        "Dependency Update"
    }

    fn category(&self) -> &str {
        "tooling"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create Cargo.toml with old serde version
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"myproject\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\
             \n\
             [dependencies]\n\
             serde = \"1.0.100\"\n",
        )?;

        // Create simple lib that uses serde
        std::fs::write(
            workspace.join("lib.rs"),
            "use serde::{Serialize, Deserialize};\n\
             \n\
             #[derive(Serialize, Deserialize)]\n\
             pub struct Config {\n\
             pub name: String,\n\
             }\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Update the serde dependency in Cargo.toml to the latest 1.0.x version (at least 1.0.200)"
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        let cargo_content = std::fs::read_to_string(workspace.join("Cargo.toml"))?;

        // Parse version - look for serde = "1.0.XXX"
        let version_updated = if let Some(serde_line) = cargo_content.lines().find(|l| l.contains("serde")) {
            // Extract version number
            if let Some(version_str) = serde_line.split('"').nth(1) {
                if let Some(patch) = version_str.split('.').nth(2) {
                    if let Ok(patch_num) = patch.parse::<u32>() {
                        patch_num >= 200
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if version_updated {
            messages.push("✓ serde updated to 1.0.200 or later".to_string());
        } else {
            messages.push("✗ serde version not updated to 1.0.200+".to_string());
            messages.push(format!("Cargo.toml content: {}", cargo_content));
        }

        // Check if code still compiles
        let compiles = std::process::Command::new("cargo")
            .arg("check")
            .current_dir(workspace)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if compiles {
            messages.push("✓ Code compiles with updated dependency".to_string());
        } else {
            messages.push("✗ Code does not compile".to_string());
        }

        custom_metrics.insert("version_updated".to_string(), if version_updated { 1.0 } else { 0.0 });
        custom_metrics.insert("compiles".to_string(), if compiles { 1.0 } else { 0.0 });

        Ok(ValidationResult {
            success: version_updated && compiles,
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

/// B-011: Error Recovery
///
/// Task: Complete a task despite initial errors (missing file)
pub struct B011ErrorRecovery;

impl BenchmarkTask for B011ErrorRecovery {
    fn id(&self) -> &str {
        "B-011"
    }

    fn name(&self) -> &str {
        "Error Recovery"
    }

    fn category(&self) -> &str {
        "resilience"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create a config file with a reference to a missing data file
        std::fs::write(
            workspace.join("config.json"),
            r#"{"data_file": "data.json", "output": "result.txt"}"#,
        )?;

        // Deliberately NOT creating data.json - this will cause an error
        // The agent needs to either create it or handle the error gracefully

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Read config.json, load the data file specified in it, and write 'SUCCESS' to the output file. Handle any errors you encounter."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check if output file exists
        let output_path = workspace.join("result.txt");
        let output_exists = output_path.exists();

        if output_exists {
            let content = std::fs::read_to_string(&output_path)?;
            let has_success = content.contains("SUCCESS");

            if has_success {
                messages.push("✓ Output file created with SUCCESS".to_string());
                custom_metrics.insert("output_correct".to_string(), 1.0);
            } else {
                messages.push(format!("✗ Output file exists but content is wrong: {}", content));
                custom_metrics.insert("output_correct".to_string(), 0.0);
            }
        } else {
            messages.push("✗ Output file not created".to_string());
            custom_metrics.insert("output_correct".to_string(), 0.0);
        }

        // Check if data.json was created (one way to handle the error)
        let data_created = workspace.join("data.json").exists();
        if data_created {
            messages.push("✓ Missing data file was created".to_string());
        }
        custom_metrics.insert("data_created".to_string(), if data_created { 1.0 } else { 0.0 });

        let success = output_exists && output_path.exists();

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

/// B-012: Context Gathering
///
/// Task: Find specific functionality in a large codebase
pub struct B012ContextGathering;

impl BenchmarkTask for B012ContextGathering {
    fn id(&self) -> &str {
        "B-012"
    }

    fn name(&self) -> &str {
        "Context Gathering"
    }

    fn category(&self) -> &str {
        "exploration"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create a realistic directory structure with 50+ files
        let dirs = vec!["src", "src/api", "src/models", "src/utils", "src/auth", "tests", "docs"];
        for dir in dirs {
            std::fs::create_dir_all(workspace.join(dir))?;
        }

        // Create various files
        for i in 1..=20 {
            std::fs::write(
                workspace.join(format!("src/api/endpoint{}.rs", i)),
                format!("pub fn handle_request{}() {{}}\n", i),
            )?;
        }

        // Create the actual auth logic (what we're looking for)
        std::fs::write(
            workspace.join("src/auth/login.rs"),
            "pub fn authenticate_user(username: &str, password: &str) -> bool {\n\
             // Authentication logic here\n\
             verify_password_hash(username, password)\n\
             }\n\
             \n\
             fn verify_password_hash(username: &str, password: &str) -> bool {\n\
             true // Simplified\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("src/auth/mod.rs"),
            "pub mod login;\n\
             pub mod session;\n",
        )?;

        std::fs::write(
            workspace.join("src/auth/session.rs"),
            "pub fn create_session(user_id: u64) -> String {\n\
             format!(\"session_{}\", user_id)\n\
             }\n",
        )?;

        // Create many unrelated files
        for i in 1..=30 {
            std::fs::write(
                workspace.join(format!("src/models/model{}.rs", i)),
                format!("pub struct Model{} {{}}\n", i),
            )?;
        }

        // Create answer file where agent should document findings
        std::fs::write(
            workspace.join("ANSWER.md"),
            "# Authentication Implementation\n\n(Write your findings here)\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Find where the authentication logic is implemented in this codebase. Update ANSWER.md with the file paths and a brief description of what you found."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        let answer_content = std::fs::read_to_string(workspace.join("ANSWER.md"))?;

        // Check if key files were identified
        let found_login = answer_content.contains("auth/login") || answer_content.contains("login.rs");
        let found_session = answer_content.contains("session") || answer_content.contains("session.rs");
        let found_authenticate = answer_content.contains("authenticate") || answer_content.contains("verify");

        if found_login {
            messages.push("✓ Identified auth/login.rs".to_string());
        } else {
            messages.push("✗ Did not identify login module".to_string());
        }

        if found_authenticate {
            messages.push("✓ Mentioned authentication function".to_string());
        } else {
            messages.push("✗ Did not describe authentication".to_string());
        }

        custom_metrics.insert("found_login".to_string(), if found_login { 1.0 } else { 0.0 });
        custom_metrics.insert("found_session".to_string(), if found_session { 1.0 } else { 0.0 });
        custom_metrics.insert("found_authenticate".to_string(), if found_authenticate { 1.0 } else { 0.0 });

        let success = found_login && found_authenticate;

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

/// B-013: Cross-File Consistency
///
/// Task: Add a field to a struct and update all usages
pub struct B013CrossFileConsistency;

impl BenchmarkTask for B013CrossFileConsistency {
    fn id(&self) -> &str {
        "B-013"
    }

    fn name(&self) -> &str {
        "Cross-File Consistency"
    }

    fn category(&self) -> &str {
        "refactor"
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

        // Create file with constructor
        std::fs::write(
            workspace.join("repository.rs"),
            "use crate::types::User;\n\
             \n\
             pub fn create_user(id: u64, name: String) -> User {\n\
             User { id, name }\n\
             }\n\
             \n\
             pub fn default_user() -> User {\n\
             User { id: 0, name: String::from(\"Guest\") }\n\
             }\n",
        )?;

        // Create file with another usage
        std::fs::write(
            workspace.join("service.rs"),
            "use crate::types::User;\n\
             \n\
             pub fn make_admin(id: u64) -> User {\n\
             User { id, name: String::from(\"Admin\") }\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"consistency-test\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        std::fs::write(
            workspace.join("lib.rs"),
            "pub mod types;\n\
             pub mod repository;\n\
             pub mod service;\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Add a new field 'email: String' to the User struct in types.rs. Update all constructors in repository.rs and service.rs to include this field. Use empty string as default for email."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        // Check types.rs has the new field
        let types_content = std::fs::read_to_string(workspace.join("types.rs"))?;
        let field_added = types_content.contains("email") && types_content.contains("String");

        if field_added {
            messages.push("✓ Email field added to User struct".to_string());
        } else {
            messages.push("✗ Email field not added to User struct".to_string());
        }

        // Check repository.rs constructors updated
        let repo_content = std::fs::read_to_string(workspace.join("repository.rs"))?;
        let repo_create_updated = repo_content.contains("email");
        let repo_default_updated = repo_content.contains("email");

        if repo_create_updated && repo_default_updated {
            messages.push("✓ All constructors in repository.rs updated".to_string());
        } else {
            messages.push("✗ Not all constructors in repository.rs updated".to_string());
        }

        // Check service.rs updated
        let service_content = std::fs::read_to_string(workspace.join("service.rs"))?;
        let service_updated = service_content.contains("email");

        if service_updated {
            messages.push("✓ Constructor in service.rs updated".to_string());
        } else {
            messages.push("✗ Constructor in service.rs not updated".to_string());
        }

        // Check compilation
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

        custom_metrics.insert("field_added".to_string(), if field_added { 1.0 } else { 0.0 });
        custom_metrics.insert("repo_updated".to_string(), if repo_create_updated && repo_default_updated { 1.0 } else { 0.0 });
        custom_metrics.insert("service_updated".to_string(), if service_updated { 1.0 } else { 0.0 });
        custom_metrics.insert("compiles".to_string(), if compiles { 1.0 } else { 0.0 });

        let success = field_added && repo_create_updated && repo_default_updated && service_updated && compiles;

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

/// B-014: Performance Optimization
///
/// Task: Optimize inefficient code to reduce allocations
pub struct B014PerformanceOptimization;

impl BenchmarkTask for B014PerformanceOptimization {
    fn id(&self) -> &str {
        "B-014"
    }

    fn name(&self) -> &str {
        "Performance Optimization"
    }

    fn category(&self) -> &str {
        "optimization"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create inefficient code that allocates unnecessarily
        std::fs::write(
            workspace.join("lib.rs"),
            "pub fn process_numbers(numbers: &[i32]) -> Vec<i32> {\n\
             let mut result = Vec::new();\n\
             for num in numbers {\n\
             let doubled = num * 2;\n\
             let temp_vec = vec![doubled]; // Unnecessary allocation!\n\
             result.push(temp_vec[0]);\n\
             }\n\
             result\n\
             }\n\
             \n\
             #[cfg(test)]\n\
             mod tests {\n\
             use super::*;\n\
             \n\
             #[test]\n\
             fn test_process_numbers() {\n\
             assert_eq!(process_numbers(&[1, 2, 3]), vec![2, 4, 6]);\n\
             }\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"optimize-test\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Optimize the process_numbers function in lib.rs to remove unnecessary allocations. The function should still pass the test."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        let lib_content = std::fs::read_to_string(workspace.join("lib.rs"))?;

        // Check that temp_vec allocation was removed
        let allocation_removed = !lib_content.contains("vec![doubled]") && !lib_content.contains("temp_vec");

        if allocation_removed {
            messages.push("✓ Unnecessary allocation removed".to_string());
        } else {
            messages.push("✗ Unnecessary allocation still present".to_string());
        }

        // Check that the logic is simpler now
        let is_optimized = allocation_removed && lib_content.contains("result.push(");

        if is_optimized {
            messages.push("✓ Code appears optimized".to_string());
        }

        // Run tests to ensure correctness
        let test_output = std::process::Command::new("cargo")
            .args(["test"])
            .current_dir(workspace)
            .output()?;

        let tests_pass = test_output.status.success();
        if tests_pass {
            messages.push("✓ Tests still pass".to_string());
        } else {
            messages.push("✗ Tests failing after optimization".to_string());
        }

        custom_metrics.insert("allocation_removed".to_string(), if allocation_removed { 1.0 } else { 0.0 });
        custom_metrics.insert("tests_pass".to_string(), if tests_pass { 1.0 } else { 0.0 });

        let success = allocation_removed && tests_pass;

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

/// B-015: Security Fix
///
/// Task: Fix SQL injection vulnerability
pub struct B015SecurityFix;

impl BenchmarkTask for B015SecurityFix {
    fn id(&self) -> &str {
        "B-015"
    }

    fn name(&self) -> &str {
        "Security Fix"
    }

    fn category(&self) -> &str {
        "security"
    }

    fn setup(&self, workspace: &Path) -> Result<()> {
        // Create code with SQL injection vulnerability
        std::fs::write(
            workspace.join("lib.rs"),
            "use rusqlite::{Connection, Result};\n\
             \n\
             pub fn get_user_by_name(conn: &Connection, name: &str) -> Result<Option<String>> {\n\
             // VULNERABLE: String concatenation with user input\n\
             let query = format!(\"SELECT email FROM users WHERE name = '{}'\", name);\n\
             let mut stmt = conn.prepare(&query)?;\n\
             let mut rows = stmt.query([])?;\n\
             \n\
             if let Some(row) = rows.next()? {\n\
             let email: String = row.get(0)?;\n\
             Ok(Some(email))\n\
             } else {\n\
             Ok(None)\n\
             }\n\
             }\n",
        )?;

        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\n\
             name = \"security-test\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\
             \n\
             [dependencies]\n\
             rusqlite = \"0.32\"\n",
        )?;

        Ok(())
    }

    fn prompt(&self) -> &str {
        "Fix the SQL injection vulnerability in lib.rs by using parameterized queries instead of string concatenation."
    }

    fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
        let mut messages = Vec::new();
        let mut custom_metrics = HashMap::new();

        let lib_content = std::fs::read_to_string(workspace.join("lib.rs"))?;

        // Check that format! with SQL is removed
        let format_removed = !lib_content.contains("format!(") || !lib_content.contains("SELECT");

        // Check that parameterized query is used
        let uses_params = lib_content.contains("?") && lib_content.contains("query(");

        if format_removed {
            messages.push("✓ String concatenation removed from SQL query".to_string());
        } else {
            messages.push("✗ String concatenation still used in SQL query".to_string());
        }

        if uses_params {
            messages.push("✓ Parameterized query used".to_string());
        } else {
            messages.push("✗ Parameters not used properly".to_string());
        }

        // Check compilation
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

        custom_metrics.insert("format_removed".to_string(), if format_removed { 1.0 } else { 0.0 });
        custom_metrics.insert("uses_params".to_string(), if uses_params { 1.0 } else { 0.0 });
        custom_metrics.insert("compiles".to_string(), if compiles { 1.0 } else { 0.0 });

        let success = format_removed && uses_params && compiles;

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
