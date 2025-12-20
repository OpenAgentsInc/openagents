//! File guardrails for Read/Edit operations
//!
//! Provides validation for file operations:
//! - Validates paths exist before Read
//! - Validates paths are files (not directories)
//! - Tracks read files and requires Read before Edit
//! - Validates paths exist before Edit

use std::collections::HashSet;
use std::path::Path;
use std::sync::RwLock;

lazy_static::lazy_static! {
    /// Set of file paths that have been read in this session
    static ref READ_FILES: RwLock<HashSet<String>> = RwLock::new(HashSet::new());
}

/// Reset the read files tracking (for testing or session reset)
pub fn reset_read_files() {
    let mut files = READ_FILES.write().expect("Failed to acquire write lock");
    files.clear();
}

/// Mark a file as having been read
pub fn mark_file_read(path: &str) {
    let normalized = normalize_path(path);
    let mut files = READ_FILES.write().expect("Failed to acquire write lock");
    files.insert(normalized);
}

/// Check if a file has been read
pub fn was_file_read(path: &str) -> bool {
    let normalized = normalize_path(path);
    let files = READ_FILES.read().expect("Failed to acquire read lock");
    files.contains(&normalized)
}

/// Normalize a path for consistent comparison
fn normalize_path(path: &str) -> String {
    // Canonicalize if path exists, otherwise just clean it
    if let Ok(canonical) = std::fs::canonicalize(path) {
        canonical.to_string_lossy().to_string()
    } else {
        path.to_string()
    }
}

/// Validate a Read operation
///
/// Checks:
/// - Path exists
/// - Path is a file (not a directory)
///
/// Returns Ok(()) if valid, Err(message) if not
pub fn validate_read(path: &str) -> Result<(), String> {
    let p = Path::new(path);

    // Check path exists
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    // Check it's a file, not a directory
    if p.is_dir() {
        return Err(format!(
            "Cannot read directory '{}'. Use Bash with 'ls' to list directory contents.",
            path
        ));
    }

    // Mark as read for Edit tracking
    mark_file_read(path);

    Ok(())
}

/// Validate an Edit operation
///
/// Checks:
/// - Path exists
/// - Path is a file (not a directory)
/// - File has been read first
///
/// Returns Ok(()) if valid, Err(message) if not
pub fn validate_edit(path: &str) -> Result<(), String> {
    let p = Path::new(path);

    // Check path exists
    if !p.exists() {
        return Err(format!(
            "Cannot edit non-existent file: {}. Use Write to create new files.",
            path
        ));
    }

    // Check it's a file, not a directory
    if p.is_dir() {
        return Err(format!("Cannot edit directory: {}", path));
    }

    // Check file was read first
    if !was_file_read(path) {
        return Err(format!(
            "File has not been read yet: {}. Use Read tool first to understand the file contents before editing.",
            path
        ));
    }

    Ok(())
}

/// Validate a Write operation
///
/// Checks:
/// - Parent directory exists
/// - Path is not a directory
///
/// Returns Ok(()) if valid, Err(message) if not
pub fn validate_write(path: &str) -> Result<(), String> {
    let p = Path::new(path);

    // Check if path is a directory
    if p.exists() && p.is_dir() {
        return Err(format!("Cannot write to directory: {}", path));
    }

    // Check parent directory exists
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(format!(
                "Parent directory does not exist: {}. Create it first with mkdir.",
                parent.display()
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup() -> TempDir {
        reset_read_files();
        TempDir::new().unwrap()
    }

    #[test]
    fn test_validate_read_existing_file() {
        let dir = setup();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        assert!(validate_read(file_path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_validate_read_nonexistent_file() {
        let dir = setup();
        let file_path = dir.path().join("nonexistent.txt");

        let result = validate_read(file_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_validate_read_directory() {
        let dir = setup();
        let sub_dir = dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let result = validate_read(sub_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot read directory"));
    }

    #[test]
    fn test_validate_edit_requires_read() {
        let dir = setup();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        // Edit without read should fail
        let result = validate_edit(file_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("has not been read yet"));

        // Read the file
        validate_read(file_path.to_str().unwrap()).unwrap();

        // Now edit should succeed
        assert!(validate_edit(file_path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_validate_edit_nonexistent_file() {
        let dir = setup();
        let file_path = dir.path().join("nonexistent.txt");

        let result = validate_edit(file_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-existent"));
    }

    #[test]
    fn test_validate_edit_directory() {
        let dir = setup();
        let sub_dir = dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let result = validate_edit(sub_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot edit directory"));
    }

    #[test]
    fn test_validate_write_to_directory() {
        let dir = setup();
        let sub_dir = dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let result = validate_write(sub_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot write to directory"));
    }

    #[test]
    fn test_validate_write_missing_parent() {
        let dir = setup();
        let file_path = dir.path().join("nonexistent_parent").join("test.txt");

        let result = validate_write(file_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Parent directory does not exist"));
    }

    #[test]
    fn test_validate_write_valid_path() {
        let dir = setup();
        let file_path = dir.path().join("new_file.txt");

        assert!(validate_write(file_path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_read_files_tracking() {
        reset_read_files();

        assert!(!was_file_read("/some/path/file.txt"));

        mark_file_read("/some/path/file.txt");

        assert!(was_file_read("/some/path/file.txt"));
    }

    #[test]
    fn test_reset_read_files() {
        mark_file_read("/test/file.txt");
        assert!(was_file_read("/test/file.txt"));

        reset_read_files();
        assert!(!was_file_read("/test/file.txt"));
    }
}
