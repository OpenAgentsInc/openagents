//! Integration tests for Git clone operations
//!
//! Tests cloning from different URL schemes with real repositories.

use git2::{Repository, Signature};
use gitafter::git::{clone_repository, get_workspace_path};
use std::path::Path;
use tempfile::TempDir;

#[test]
#[ignore] // Requires network access
fn test_clone_https_public_repo() {
    let temp_dir = TempDir::new().unwrap();
    let dest_path = temp_dir.path().join("test-repo");

    // Clone a small public repository via HTTPS
    let url = "https://github.com/octocat/Hello-World.git";
    let result = clone_repository(url, &dest_path, None);

    assert!(result.is_ok(), "Clone should succeed: {:?}", result.err());

    // Verify repository was cloned
    assert!(dest_path.exists());
    assert!(dest_path.join(".git").exists());
}

#[test]
fn test_clone_invalid_url_scheme() {
    let temp_dir = TempDir::new().unwrap();
    let dest_path = temp_dir.path().join("test-repo");

    // Try to clone with unsupported scheme
    let url = "ftp://example.com/repo.git";
    let result = clone_repository(url, &dest_path, None);

    assert!(result.is_err());
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(err_msg.contains("Unsupported URL scheme"));
}

#[test]
fn test_clone_local_repo_via_file_url() {
    let source_dir = TempDir::new().unwrap();
    let source_repo = Repository::init(source_dir.path()).unwrap();

    std::fs::write(source_dir.path().join("README.md"), "hello clone").unwrap();

    let mut index = source_repo.index().unwrap();
    index.add_path(Path::new("README.md")).unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = source_repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("GitAfter Test", "test@example.com").unwrap();
    source_repo
        .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
        .unwrap();

    let temp_dir = TempDir::new().unwrap();
    let dest_path = temp_dir.path().join("cloned-repo");
    let url = format!("file://{}", source_dir.path().display());

    let result = clone_repository(&url, &dest_path, None);

    assert!(result.is_ok(), "Local file clone should succeed");
    assert!(dest_path.join(".git").exists());
}

#[test]
fn test_clone_to_nonexistent_directory() {
    let temp_dir = TempDir::new().unwrap();
    let dest_path = temp_dir
        .path()
        .join("deeply")
        .join("nested")
        .join("test-repo");

    // Should create parent directories automatically
    // Using a fake URL that will fail at the network stage, but parent dirs should be created
    let url = "https://github.com/nonexistent-user-12345/nonexistent-repo-67890.git";

    // This will fail but we don't care about the error type
    let _ = clone_repository(url, &dest_path, None);

    // Parent directory should exist even if clone fails
    assert!(dest_path.parent().unwrap().exists());
}

#[test]
fn test_workspace_path_exists() {
    let workspace = get_workspace_path();

    // Workspace path should be deterministic
    assert!(workspace.to_string_lossy().contains("gitafter"));
    assert!(workspace.to_string_lossy().contains("repos"));
}

#[test]
fn test_clone_url_validation() {
    let temp_dir = TempDir::new().unwrap();

    // Test various URL formats
    let test_cases = vec![
        ("https://github.com/user/repo.git", false), // Would require network
        ("git@github.com:user/repo.git", false),     // Would require SSH agent
        ("ssh://git@github.com/user/repo.git", false), // Would require SSH agent
        ("git://github.com/user/repo.git", false),   // Would require network
        ("file:///tmp/local-repo", false),           // Should pass validation
        ("http://example.com/repo.git", true),       // Should fail validation
        ("ftp://example.com/repo.git", true),        // Should fail validation
        ("invalid-url", true),                       // Should fail validation
    ];

    for (url, should_fail_validation) in test_cases {
        let dest = temp_dir.path().join(format!("repo-{}", url.len()));
        let result = clone_repository(url, &dest, None);

        if should_fail_validation {
            assert!(
                result.is_err(),
                "URL {} should fail validation but succeeded",
                url
            );
            let err_msg = format!("{:?}", result.err().unwrap());
            assert!(
                err_msg.contains("Unsupported"),
                "Error should mention unsupported URL scheme for {}, got: {}",
                url,
                err_msg
            );
        }
        // Note: URLs that pass validation may still fail to clone due to network/auth
    }
}
