//! Integration tests for GitAfter diff viewer and inline comments
//!
//! Tests the complete diff rendering pipeline from git commits through the UI.

mod helpers;

use anyhow::Result;
use git2::Repository;
use helpers::test_app::TestApp;
use nostr::EventTemplate;
use std::fs;
use tempfile::TempDir;

/// Create a test git repository with commits
fn create_test_repo_with_commits() -> (TempDir, Repository, String, String) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    // Configure user
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();

    // Create initial file and commit
    fs::write(dir.path().join("test.txt"), "line1\n").unwrap();

    let sig = repo.signature().unwrap();
    let commit1_oid = {
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        ).unwrap()
    };

    // Modify and create second commit
    fs::write(dir.path().join("test.txt"), "line1\nline2\nline3\n").unwrap();

    let commit2_oid = {
        let tree_id2 = {
            let mut index = repo.index().unwrap();
            index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree2 = repo.find_tree(tree_id2).unwrap();
        let parent = repo.find_commit(commit1_oid).unwrap();

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Add lines",
            &tree2,
            &[&parent],
        ).unwrap()
    };

    (dir, repo, commit1_oid.to_string(), commit2_oid.to_string())
}

#[tokio::test]
async fn test_diff_generation_from_commits() -> Result<()> {
    let (_dir, repo, commit1, commit2) = create_test_repo_with_commits();

    // Use the diff module to generate diff
    use gitafter::git::diff::diff_commits;

    let diff = diff_commits(repo.path(), &commit1, &commit2)?;

    // Verify diff contains the expected additions
    assert!(diff.contains("+line2"), "Diff should contain added line2");
    assert!(diff.contains("+line3"), "Diff should contain added line3");
    assert!(diff.contains("test.txt"), "Diff should reference test.txt");

    // Verify it's a proper unified diff format
    assert!(diff.contains("@@"), "Diff should contain hunk headers");

    Ok(())
}

#[tokio::test]
async fn test_diff_line_parsing() -> Result<()> {
    use gitafter::views::diff::parse_diff_lines;

    let diff_text = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello");
     println!("World");
 }
"#;

    let lines = parse_diff_lines(diff_text);

    // Verify we parsed lines correctly
    assert!(lines.len() > 0, "Should parse at least one line");

    // Find the addition line
    let additions: Vec<_> = lines.iter()
        .filter(|l| matches!(l.line_type, gitafter::views::diff::DiffLineType::Addition))
        .collect();

    assert_eq!(additions.len(), 1, "Should have one addition");
    assert!(additions[0].content.contains("Hello"), "Addition should contain 'Hello'");
    assert_eq!(additions[0].file_path, "src/main.rs", "Should track file path");

    Ok(())
}

#[tokio::test]
async fn test_inline_comment_extraction() -> Result<()> {
    use gitafter::views::diff::extract_inline_comments;

    let app = TestApp::new().await?;

    // Create a comment event with line tag
    // NIP-22 format: ["line", "file_path", "line_number", "position"]
    let comment_template = EventTemplate {
        kind: 1, // NIP-22 comment
        tags: vec![
            vec!["e".to_string(), "pr-event-id-123".to_string()],
            vec!["line".to_string(), "src/main.rs".to_string(), "42".to_string(), "after".to_string()],
        ],
        content: "This line needs refactoring".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let comment_event = app.publish_event(comment_template).await?;

    // Extract inline comments
    let comments = extract_inline_comments(&[comment_event.clone()]);

    assert_eq!(comments.len(), 1, "Should extract one comment");
    assert_eq!(comments[0].file_path, "src/main.rs");
    assert_eq!(comments[0].line_number, 42);
    assert_eq!(comments[0].event.content, "This line needs refactoring");

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_inline_comment_extraction_invalid_tags() -> Result<()> {
    use gitafter::views::diff::extract_inline_comments;

    let app = TestApp::new().await?;

    // Create comment without line tag (should be ignored)
    let comment_template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["e".to_string(), "pr-event-id".to_string()],
        ],
        content: "General comment".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let comment_event = app.publish_event(comment_template).await?;

    // Extract should return empty for non-inline comments
    let comments = extract_inline_comments(&[comment_event]);

    assert_eq!(comments.len(), 0, "Should not extract comments without line tags");

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_diff_rendering_with_empty_comments() -> Result<()> {
    use gitafter::views::diff::render_diff_with_comments;

    let diff_text = r#"diff --git a/test.txt b/test.txt
@@ -1 +1,2 @@
 line1
+line2
"#;

    // Render with no comments
    let html = render_diff_with_comments(diff_text, &[], "pr-id", "repo-id");

    let html_str = html.into_string();

    // Should render diff without errors
    assert!(html_str.contains("line1"), "Should contain line1");
    assert!(html_str.contains("line2"), "Should contain line2");

    // Should not have comment sections
    assert!(!html_str.contains("comment-thread"), "Should not have comment threads");

    Ok(())
}

#[tokio::test]
async fn test_diff_rendering_with_inline_comments() -> Result<()> {
    use gitafter::views::diff::{render_diff_with_comments, InlineComment, LinePosition};

    let app = TestApp::new().await?;

    let diff_text = r#"diff --git a/test.txt b/test.txt
@@ -1 +1,2 @@
 line1
+line2
"#;

    // Create a comment event
    let comment_template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["line".to_string(), "test.txt".to_string(), "2".to_string(), "after".to_string()],
        ],
        content: "Good addition!".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let comment_event = app.publish_event(comment_template).await?;

    // Create inline comment struct
    let inline_comment = InlineComment {
        event: comment_event,
        file_path: "test.txt".to_string(),
        line_number: 2,
        position: LinePosition::After,
        author_pubkey: app.pubkey(),
        layer_info: None,
    };

    // Render with comment
    let html = render_diff_with_comments(diff_text, &[inline_comment], "pr-id", "repo-id");

    let html_str = html.into_string();

    // Should render diff with comment
    assert!(html_str.contains("line2"), "Should contain line2");
    assert!(html_str.contains("Good addition!"), "Should contain comment text");

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_urlencoding_for_patch_download() -> Result<()> {
    let diff_text = "diff --git a/file.txt b/file.txt\n+new line with special chars: <>&\"";

    // Test that urlencoding works
    let encoded = urlencoding::encode(diff_text);

    // Verify special chars are encoded
    assert!(encoded.contains("%3C"), "Should encode <");
    assert!(encoded.contains("%3E"), "Should encode >");
    assert!(encoded.contains("%26"), "Should encode &");

    // Verify it can be decoded back
    let decoded = urlencoding::decode(&encoded)?;
    assert_eq!(decoded, diff_text, "Should decode back to original");

    Ok(())
}

#[tokio::test]
async fn test_pr_with_diff_complete_flow() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repo and issue
    let _repo = app.create_repository("test-repo", "Test Repo", "Description").await?;
    let issue = app.create_issue("test-repo", "Add feature", "Body").await?;

    // Create test git repo with diff
    let (_dir, _git_repo, commit1, commit2) = create_test_repo_with_commits();

    // Create PR with commit reference
    let pr_template = EventTemplate {
        kind: 1618, // PULL_REQUEST
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["e".to_string(), issue.id.clone()],
            vec!["subject".to_string(), "Add feature implementation".to_string()],
            vec!["c".to_string(), commit2.clone()], // commit ID
            vec!["parent".to_string(), commit1.clone()], // parent commit
            vec!["clone".to_string(), "https://github.com/test/repo".to_string()],
        ],
        content: "## Summary\nAdded new feature\n\n## Changes\n- Added lines to test.txt".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Verify PR has commit tag
    let commit_tag = pr.tags.iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("c"))
        .expect("PR should have commit tag");
    assert_eq!(commit_tag.get(1).unwrap(), &commit2);

    // Verify PR has parent tag
    let parent_tag = pr.tags.iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("parent"))
        .expect("PR should have parent tag");
    assert_eq!(parent_tag.get(1).unwrap(), &commit1);

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_comment_grouping_by_file_and_line() -> Result<()> {
    use gitafter::views::diff::extract_inline_comments;

    let app = TestApp::new().await?;

    // Create multiple comments on different files and lines
    let comment1_template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["e".to_string(), "pr-id".to_string()],
            vec!["line".to_string(), "file1.rs".to_string(), "10".to_string(), "after".to_string()],
        ],
        content: "Comment on file1 line 10".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let comment2_template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["e".to_string(), "pr-id".to_string()],
            vec!["line".to_string(), "file1.rs".to_string(), "10".to_string(), "after".to_string()],
        ],
        content: "Another comment on file1 line 10".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let comment3_template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["e".to_string(), "pr-id".to_string()],
            vec!["line".to_string(), "file2.rs".to_string(), "20".to_string(), "before".to_string()],
        ],
        content: "Comment on file2 line 20".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let c1 = app.publish_event(comment1_template).await?;
    let c2 = app.publish_event(comment2_template).await?;
    let c3 = app.publish_event(comment3_template).await?;

    // Extract all comments
    let comments = extract_inline_comments(&[c1, c2, c3]);

    assert_eq!(comments.len(), 3, "Should extract all three comments");

    // Group by file and line
    let file1_line10: Vec<_> = comments.iter()
        .filter(|c| c.file_path == "file1.rs" && c.line_number == 10)
        .collect();

    assert_eq!(file1_line10.len(), 2, "Should have 2 comments on file1 line 10");

    let file2_line20: Vec<_> = comments.iter()
        .filter(|c| c.file_path == "file2.rs" && c.line_number == 20)
        .collect();

    assert_eq!(file2_line20.len(), 1, "Should have 1 comment on file2 line 20");

    app.shutdown().await;
    Ok(())
}
