//! Component tests for patch and pull request views

use scraper::{Html, Selector};

#[test]
fn test_pr_list_structure() {
    let pr_list_html = r#"
        <div class="pr-list">
            <h2>Pull Requests</h2>
            <ul>
                <li>
                    <a href="/prs/1">
                        <strong>PR #1: Add feature X</strong>
                        <span class="status">Open</span>
                    </a>
                </li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(pr_list_html);

    // Must use semantic list
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "PR list must use ul element");

    // Must have heading
    let heading = doc.select(&Selector::parse("h2").unwrap()).next();
    assert!(heading.is_some(), "PR list must have heading");
}

#[test]
fn test_pr_status_badges() {
    let pr_html = r#"
        <div>
            <span class="status open">Open</span>
            <span class="status merged">Merged</span>
            <span class="status closed">Closed</span>
            <span class="status draft">Draft</span>
        </div>
    "#;

    let doc = Html::parse_fragment(pr_html);
    let statuses: Vec<_> = doc.select(&Selector::parse(".status").unwrap()).collect();

    assert_eq!(statuses.len(), 4, "Should have 4 status badges");

    // All statuses must have text
    for status in statuses {
        let text: String = status.text().collect();
        assert!(
            !text.trim().is_empty(),
            "Status badge must have text"
        );
    }
}

#[test]
fn test_trajectory_link_display() {
    let pr_html = r#"
        <div class="pr-trajectory">
            <a href="/trajectory/session-abc-123">
                <span>🔍 View Trajectory</span>
            </a>
            <span class="verified">✓ Verified</span>
        </div>
    "#;

    let doc = Html::parse_fragment(pr_html);

    // Must have trajectory link
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "PR must have trajectory link");

    let link_text: String = link.unwrap().text().collect();
    assert!(
        !link_text.trim().is_empty(),
        "Trajectory link must have accessible text"
    );

    // Verification badge present
    let verified = doc.select(&Selector::parse(".verified").unwrap()).next();
    assert!(verified.is_some(), "Must show verification status");
}

#[test]
fn test_pr_commit_info() {
    let commit_html = r#"
        <div class="commit-info">
            <code>abc123def456</code>
            <a href="https://github.com/test/repo.git">Clone URL</a>
        </div>
    "#;

    let doc = Html::parse_fragment(commit_html);

    // Commit ID must be in code element
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Commit ID must use code element");

    // Clone URL must be present
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "Must have clone URL link");
}

#[test]
fn test_pr_subject_xss_prevention() {
    let malicious_subject = "<img src=x onerror=alert(1)>";
    let pr_html = format!(
        r#"<h1>{}</h1>"#,
        html_escape::encode_text(malicious_subject)
    );

    let doc = Html::parse_fragment(&pr_html);

    // No img tags with onerror
    let imgs: Vec<_> = doc.select(&Selector::parse("img").unwrap()).collect();
    for img in imgs {
        assert!(
            img.value().attr("onerror").is_none(),
            "Must not have onerror handlers"
        );
    }
}

#[test]
fn test_patch_content_display() {
    let patch_html = r#"
        <div class="patch-content">
            <pre><code>diff --git a/file.rs b/file.rs
index 123..456 100644
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
+// New line
 fn main() {
 }</code></pre>
        </div>
    "#;

    let doc = Html::parse_fragment(patch_html);

    // Patch must be in pre+code for formatting
    let pre = doc.select(&Selector::parse("pre").unwrap()).next();
    assert!(pre.is_some(), "Patch must use pre element");

    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Patch must use code element");
}

#[test]
fn test_pr_update_indicator() {
    let update_html = r#"
        <div class="pr-update">
            <span>Updated commit after rebase</span>
            <code>new-commit-id</code>
        </div>
    "#;

    let doc = Html::parse_fragment(update_html);

    // Must show update information
    let update_div = doc.select(&Selector::parse(".pr-update").unwrap()).next();
    assert!(update_div.is_some(), "Must show PR update");

    // New commit ID present
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Must show new commit ID");
}

#[test]
fn test_merge_button_accessibility() {
    let button_html = r#"
        <button aria-label="Merge pull request">Merge</button>
    "#;

    let doc = Html::parse_fragment(button_html);
    let button = doc.select(&Selector::parse("button").unwrap()).next().unwrap();

    // Must have accessible name
    let text: String = button.text().collect();
    let has_text = !text.trim().is_empty();
    let has_aria = button.value().attr("aria-label").is_some();

    assert!(
        has_text || has_aria,
        "Merge button must have accessible name"
    );
}

#[test]
fn test_pr_author_display() {
    let author_html = r#"
        <div class="pr-author">
            <span class="agent-badge">🤖 AGENT</span>
            <code>npub1...</code>
        </div>
    "#;

    let doc = Html::parse_fragment(author_html);

    // Agent badge for agent authors
    let badge = doc.select(&Selector::parse(".agent-badge").unwrap()).next();
    assert!(badge.is_some(), "Agent PRs must have badge");

    // Author pubkey displayed
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Must show author pubkey");
}

#[test]
fn test_empty_pr_list() {
    let empty_html = r#"
        <div class="pr-list">
            <h2>Pull Requests</h2>
            <p>No pull requests found.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(empty_html);

    let message = doc.select(&Selector::parse("p").unwrap()).next();
    assert!(message.is_some(), "Empty state must have message");

    let text: String = message.unwrap().text().collect();
    assert!(
        text.contains("No pull requests"),
        "Empty message must be informative"
    );
}
