//! Integration test for syntax-highlighted diff viewer
//!
//! Verifies that the diff viewer properly integrates with PR detail views
//! and provides syntax highlighting, collapsible sections, and keyboard shortcuts.

use gitafter::views::diff::{
    parse_diff_lines, render_diff_optimized, DiffLineType, DiffRenderConfig, InlineComment,
};

#[test]
fn test_diff_parsing_rust_code() {
    let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 123456..789abc 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
+    println!("Hello, world!");
     let x = 42;
-    let y = 13;
+    let y = 13; // Updated comment
+    let z = x + y;
 }
"#;

    let lines = parse_diff_lines(diff);

    // Verify we parsed all lines
    assert!(lines.len() > 0, "Should parse diff lines");

    // Check for additions
    let additions: Vec<_> = lines
        .iter()
        .filter(|l| l.line_type == DiffLineType::Addition)
        .collect();
    assert_eq!(
        additions.len(),
        3,
        "Should have 3 additions (println, updated comment, z)"
    );

    // Check for deletions
    let deletions: Vec<_> = lines
        .iter()
        .filter(|l| l.line_type == DiffLineType::Deletion)
        .collect();
    assert_eq!(deletions.len(), 1, "Should have 1 deletion (old y)");

    // Check for headers
    let headers: Vec<_> = lines
        .iter()
        .filter(|l| l.line_type == DiffLineType::Header)
        .collect();
    assert!(headers.len() >= 2, "Should have at least 2 headers");

    // Verify file paths are set
    for line in &lines {
        if line.line_type != DiffLineType::Header {
            assert_eq!(
                line.file_path, "src/main.rs",
                "File path should be set correctly"
            );
        }
    }
}

#[test]
fn test_diff_rendering_with_config() {
    let diff = r#"diff --git a/test.js b/test.js
@@ -1,20 +1,22 @@
 function example() {
     const a = 1;
     const b = 2;
     const c = 3;
     const d = 4;
     const e = 5;
     const f = 6;
     const g = 7;
     const h = 8;
     const i = 9;
     const j = 10;
     const k = 11;
+    const l = 12; // New line
     const m = 13;
 }
"#;

    let config = DiffRenderConfig {
        collapse_threshold: 10,
        context_lines: 3,
    };

    let comments: Vec<InlineComment> = vec![];
    let markup = render_diff_optimized(diff, &comments, "test-pr", "test-repo", config);

    let html = markup.into_string();

    // Verify diff container exists
    assert!(
        html.contains("diff-container"),
        "Should render diff container"
    );

    // Verify addition is rendered
    assert!(
        html.contains("diff-line-add"),
        "Should render addition lines"
    );

    // Verify keyboard shortcuts are included
    assert!(
        html.contains("document.addEventListener('keydown'"),
        "Should include keyboard shortcuts"
    );

    // Verify expand/collapse toggle exists
    assert!(
        html.contains("collapsed-chunk") || html.contains("expand-toggle"),
        "Should include collapse functionality"
    );
}

#[test]
fn test_diff_with_multiple_files() {
    let diff = r#"diff --git a/src/foo.rs b/src/foo.rs
@@ -1,3 +1,4 @@
 fn foo() {
+    println!("foo");
 }
diff --git a/src/bar.rs b/src/bar.rs
@@ -1,3 +1,4 @@
 fn bar() {
+    println!("bar");
 }
"#;

    let lines = parse_diff_lines(diff);

    // Find lines from foo.rs
    let foo_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.file_path.contains("foo.rs"))
        .collect();
    assert!(foo_lines.len() > 0, "Should have lines from foo.rs");

    // Find lines from bar.rs
    let bar_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.file_path.contains("bar.rs"))
        .collect();
    assert!(bar_lines.len() > 0, "Should have lines from bar.rs");

    // Verify file paths are correctly assigned
    for line in foo_lines {
        assert!(line.file_path.ends_with("foo.rs"));
    }
    for line in bar_lines {
        assert!(line.file_path.ends_with("bar.rs"));
    }
}

#[test]
fn test_render_includes_styles() {
    let diff = r#"diff --git a/test.py b/test.py
@@ -1,2 +1,3 @@
 def test():
+    print("hello")
     pass
"#;

    let config = DiffRenderConfig::default();
    let comments: Vec<InlineComment> = vec![];
    let markup = render_diff_optimized(diff, &comments, "pr-1", "repo-1", config);

    let html = markup.into_string();

    // Verify styles are embedded
    assert!(html.contains("<style>"), "Should include <style> tag");
    assert!(
        html.contains(".diff-container"),
        "Should include diff container styles"
    );
    assert!(
        html.contains(".diff-line-add"),
        "Should include addition styles"
    );
    assert!(
        html.contains(".diff-line-del"),
        "Should include deletion styles"
    );

    // Verify sharp corners (codebase policy - no rounded borders)
    assert!(
        !html.contains("rounded"),
        "Should NOT contain rounded borders (violates codebase policy)"
    );
}

#[test]
fn test_collapsible_sections_with_large_unchanged_blocks() {
    let diff = r#"diff --git a/large.rs b/large.rs
@@ -1,30 +1,31 @@
 fn large() {
     let a = 1;
     let b = 2;
     let c = 3;
     let d = 4;
     let e = 5;
     let f = 6;
     let g = 7;
     let h = 8;
     let i = 9;
     let j = 10;
     let k = 11;
     let l = 12;
     let m = 13;
     let n = 14;
+    let o = 15; // NEW LINE
     let p = 16;
     let q = 17;
     let r = 18;
     let s = 19;
     let t = 20;
 }
"#;

    let config = DiffRenderConfig {
        collapse_threshold: 10,
        context_lines: 3,
    };

    let comments: Vec<InlineComment> = vec![];
    let markup = render_diff_optimized(diff, &comments, "pr-collapse", "repo-1", config);

    let html = markup.into_string();

    // With >10 unchanged lines before the addition, sections should be collapsible
    assert!(
        html.contains("collapsed-chunk") || html.contains("expand-toggle"),
        "Should include collapsible sections for large unchanged blocks"
    );

    // Verify expand/collapse icon
    assert!(
        html.contains("expand-icon"),
        "Should include expand icon"
    );

    // Verify keyboard shortcuts are present
    assert!(html.contains("key === 'e'"), "Should have 'e' to expand all");
    assert!(html.contains("key === 'c'"), "Should have 'c' to collapse all");
}
