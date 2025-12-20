//! Additional integration tests for guardrails module

use autopilot::guardrails::*;
use std::fs::{self, File};
use std::io::Write as IoWrite;
use tempfile::TempDir;

fn setup() -> TempDir {
    reset_read_files();
    TempDir::new().unwrap()
}

// =========================================================================
// Path normalization tests
// =========================================================================

#[test]
fn test_read_same_file_different_paths() {
    let dir = setup();
    let file_path = dir.path().join("test.txt");
    File::create(&file_path).unwrap();

    // Read with absolute path
    let abs_path = file_path.to_str().unwrap();
    validate_read(abs_path).unwrap();

    // Should be marked as read for subsequent edit
    assert!(validate_edit(abs_path).is_ok());
}

#[test]
fn test_multiple_file_reads_tracked() {
    let dir = setup();

    let file1 = dir.path().join("file1.txt");
    let file2 = dir.path().join("file2.txt");
    let file3 = dir.path().join("file3.txt");

    File::create(&file1).unwrap();
    File::create(&file2).unwrap();
    File::create(&file3).unwrap();

    // Read all files
    validate_read(file1.to_str().unwrap()).unwrap();
    validate_read(file2.to_str().unwrap()).unwrap();
    validate_read(file3.to_str().unwrap()).unwrap();

    // All should be editable
    assert!(validate_edit(file1.to_str().unwrap()).is_ok());
    assert!(validate_edit(file2.to_str().unwrap()).is_ok());
    assert!(validate_edit(file3.to_str().unwrap()).is_ok());
}

// =========================================================================
// Edge cases for validate_read
// =========================================================================

#[test]
fn test_validate_read_empty_file() {
    let dir = setup();
    let file_path = dir.path().join("empty.txt");
    File::create(&file_path).unwrap();

    // Empty file should be readable
    assert!(validate_read(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_read_large_file_path() {
    let dir = setup();
    // Create a long filename (but within OS limits)
    let long_name = "a".repeat(200);
    let file_path = dir.path().join(format!("{}.txt", long_name));
    File::create(&file_path).unwrap();

    assert!(validate_read(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_read_special_chars_in_filename() {
    let dir = setup();
    let file_path = dir.path().join("test file with spaces.txt");
    File::create(&file_path).unwrap();

    assert!(validate_read(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_read_unicode_filename() {
    let dir = setup();
    let file_path = dir.path().join("测试文件.txt");
    File::create(&file_path).unwrap();

    assert!(validate_read(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_read_nested_directory() {
    let dir = setup();
    let nested = dir.path().join("level1").join("level2").join("level3");
    fs::create_dir_all(&nested).unwrap();

    // Try to read the nested directory (should fail)
    let result = validate_read(nested.to_str().unwrap());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot read directory"));
}

// =========================================================================
// Edge cases for validate_edit
// =========================================================================

#[test]
fn test_validate_edit_after_multiple_reads() {
    let dir = setup();
    let file_path = dir.path().join("test.txt");
    File::create(&file_path).unwrap();

    // Read multiple times
    validate_read(file_path.to_str().unwrap()).unwrap();
    validate_read(file_path.to_str().unwrap()).unwrap();
    validate_read(file_path.to_str().unwrap()).unwrap();

    // Should still be editable
    assert!(validate_edit(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_edit_symlink_supported() {
    // Note: This test only runs on Unix
    #[cfg(unix)]
    {
        let dir = setup();
        let file_path = dir.path().join("original.txt");
        let link_path = dir.path().join("link.txt");

        File::create(&file_path).unwrap();
        std::os::unix::fs::symlink(&file_path, &link_path).unwrap();

        // Read original
        validate_read(file_path.to_str().unwrap()).unwrap();

        // Edit via symlink should work (gets canonicalized to same path)
        let result = validate_edit(link_path.to_str().unwrap());
        assert!(result.is_ok(), "Symlinks should be canonicalized to original path");
    }
}

#[test]
fn test_validate_edit_readonly_file() {
    let dir = setup();
    let file_path = dir.path().join("readonly.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"content").unwrap();
    drop(file);

    // Make file read-only
    let mut perms = fs::metadata(&file_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&file_path, perms).unwrap();

    // Read should work
    validate_read(file_path.to_str().unwrap()).unwrap();

    // Edit validation should pass (actual write will fail later)
    assert!(validate_edit(file_path.to_str().unwrap()).is_ok());
}

// =========================================================================
// Edge cases for validate_write
// =========================================================================

#[test]
fn test_validate_write_overwrite_existing() {
    let dir = setup();
    let file_path = dir.path().join("existing.txt");
    File::create(&file_path).unwrap();

    // Writing to existing file should be allowed
    assert!(validate_write(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_write_new_file_in_existing_dir() {
    let dir = setup();
    let file_path = dir.path().join("new_file.txt");

    assert!(validate_write(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_write_deeply_nested() {
    let dir = setup();
    let nested_dir = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested_dir).unwrap();

    let file_path = nested_dir.join("file.txt");
    assert!(validate_write(file_path.to_str().unwrap()).is_ok());
}

#[test]
fn test_validate_write_hidden_file() {
    let dir = setup();
    let file_path = dir.path().join(".hidden");

    // Hidden files should be writable
    assert!(validate_write(file_path.to_str().unwrap()).is_ok());
}

// =========================================================================
// Workflow tests
// =========================================================================

#[test]
fn test_full_read_edit_workflow() {
    let dir = setup();
    let file_path = dir.path().join("workflow.txt");
    File::create(&file_path).unwrap();

    // Step 1: Read
    let result = validate_read(file_path.to_str().unwrap());
    assert!(result.is_ok(), "Read should succeed");

    // Step 2: Edit
    let result = validate_edit(file_path.to_str().unwrap());
    assert!(result.is_ok(), "Edit should succeed after read");
}

#[test]
fn test_write_then_read_workflow() {
    let dir = setup();
    let file_path = dir.path().join("new.txt");

    // Step 1: Validate write (file doesn't exist yet)
    let result = validate_write(file_path.to_str().unwrap());
    assert!(result.is_ok(), "Write validation should succeed");

    // Step 2: Actually create the file
    File::create(&file_path).unwrap();

    // Step 3: Read it
    let result = validate_read(file_path.to_str().unwrap());
    assert!(result.is_ok(), "Read should succeed");
}

#[test]
fn test_concurrent_file_operations() {
    let dir = setup();
    let file1 = dir.path().join("file1.txt");
    let file2 = dir.path().join("file2.txt");

    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    // Interleaved operations on different files
    validate_read(file1.to_str().unwrap()).unwrap();
    validate_read(file2.to_str().unwrap()).unwrap();
    validate_edit(file1.to_str().unwrap()).unwrap();
    validate_edit(file2.to_str().unwrap()).unwrap();
}

// =========================================================================
// Error message quality tests
// =========================================================================

#[test]
fn test_read_error_message_clarity() {
    let dir = setup();
    let file_path = dir.path().join("nonexistent.txt");

    let result = validate_read(file_path.to_str().unwrap());
    assert!(result.is_err());

    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("does not exist"));
    assert!(error_msg.contains("nonexistent.txt"));
}

#[test]
fn test_edit_error_message_suggests_read() {
    let dir = setup();
    let file_path = dir.path().join("unread.txt");
    File::create(&file_path).unwrap();

    let result = validate_edit(file_path.to_str().unwrap());
    assert!(result.is_err());

    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("has not been read yet"));
    assert!(error_msg.contains("Use Read tool first"));
}

#[test]
fn test_edit_error_message_suggests_write() {
    let dir = setup();
    let file_path = dir.path().join("nonexistent.txt");

    let result = validate_edit(file_path.to_str().unwrap());
    assert!(result.is_err());

    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("non-existent"));
    assert!(error_msg.contains("Use Write"));
}

#[test]
fn test_write_error_message_suggests_mkdir() {
    let dir = setup();
    let file_path = dir.path().join("missing").join("file.txt");

    let result = validate_write(file_path.to_str().unwrap());
    assert!(result.is_err());

    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("Parent directory does not exist"));
    assert!(error_msg.contains("mkdir"));
}

// =========================================================================
// State reset tests
// =========================================================================

#[test]
fn test_reset_clears_all_read_files() {
    let dir = setup();

    // Mark multiple files as read
    mark_file_read(dir.path().join("file1.txt").to_str().unwrap());
    mark_file_read(dir.path().join("file2.txt").to_str().unwrap());
    mark_file_read(dir.path().join("file3.txt").to_str().unwrap());

    // Verify all marked
    assert!(was_file_read(dir.path().join("file1.txt").to_str().unwrap()));
    assert!(was_file_read(dir.path().join("file2.txt").to_str().unwrap()));
    assert!(was_file_read(dir.path().join("file3.txt").to_str().unwrap()));

    // Reset
    reset_read_files();

    // Verify all cleared
    assert!(!was_file_read(dir.path().join("file1.txt").to_str().unwrap()));
    assert!(!was_file_read(dir.path().join("file2.txt").to_str().unwrap()));
    assert!(!was_file_read(dir.path().join("file3.txt").to_str().unwrap()));
}

// =========================================================================
// Extension-specific tests
// =========================================================================

#[test]
fn test_validate_various_file_extensions() {
    let dir = setup();

    let extensions = vec!["txt", "rs", "json", "toml", "md", "py", "js"];

    for ext in extensions {
        let file_path = dir.path().join(format!("test.{}", ext));
        File::create(&file_path).unwrap();

        assert!(
            validate_read(file_path.to_str().unwrap()).is_ok(),
            "Should read .{} files",
            ext
        );
    }
}

#[test]
fn test_validate_no_extension() {
    let dir = setup();
    let file_path = dir.path().join("Makefile");
    File::create(&file_path).unwrap();

    // Files without extension should work
    assert!(validate_read(file_path.to_str().unwrap()).is_ok());
}
