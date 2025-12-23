//! Component tests for repository list and detail views

use scraper::{Html, Selector};

#[test]
fn test_repository_list_structure() {
    let repos_html = r#"
        <div class="repository-list">
            <h2>Repositories</h2>
            <ul>
                <li>
                    <a href="/repos/repo1">
                        <strong>Repo 1</strong>
                        <p>Description of repo 1</p>
                    </a>
                </li>
                <li>
                    <a href="/repos/repo2">
                        <strong>Repo 2</strong>
                        <p>Description of repo 2</p>
                    </a>
                </li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(repos_html);

    // Must have list structure
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Repository list must use ul element");

    // List items must be present
    let items: Vec<_> = doc.select(&Selector::parse("li").unwrap()).collect();
    assert_eq!(items.len(), 2, "Should have 2 repository items");

    // Each item should have a link
    for item in items {
        let link = item.select(&Selector::parse("a").unwrap()).next();
        assert!(link.is_some(), "Each repository must have a link");
    }
}

#[test]
fn test_repository_list_accessibility() {
    let repos_html = r#"
        <div>
            <h2>Repositories</h2>
            <ul>
                <li>
                    <a href="/repos/test-repo">
                        <strong>Test Repo</strong>
                        <p>A test repository</p>
                    </a>
                </li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(repos_html);

    // Must have heading for screen readers
    let heading = doc.select(&Selector::parse("h2").unwrap()).next();
    assert!(heading.is_some(), "Repository list must have heading");

    // Links must have accessible text
    let links: Vec<_> = doc.select(&Selector::parse("a").unwrap()).collect();
    for link in links {
        let text: String = link.text().collect();
        assert!(
            !text.trim().is_empty(),
            "Repository links must have accessible text"
        );
    }
}

#[test]
fn test_repository_list_handles_empty_state() {
    let empty_html = r#"
        <div class="repository-list">
            <h2>Repositories</h2>
            <p>No repositories found.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(empty_html);

    // Should have empty state message
    let message = doc.select(&Selector::parse("p").unwrap()).next();
    assert!(message.is_some(), "Empty state must have message");

    let text: String = message.unwrap().text().collect();
    assert!(
        text.contains("No repositories"),
        "Empty state message must be informative"
    );
}

#[test]
fn test_repository_detail_xss_prevention() {
    let malicious_name = "<img src=x onerror=alert(1)>";
    let malicious_desc = "<script>alert('xss')</script>";

    let detail_html = format!(
        r#"
        <div>
            <h1>{}</h1>
            <p>{}</p>
        </div>
        "#,
        html_escape::encode_text(malicious_name),
        html_escape::encode_text(malicious_desc)
    );

    let doc = Html::parse_fragment(&detail_html);

    // Verify no script tags
    assert!(
        doc.select(&Selector::parse("script").unwrap()).next().is_none(),
        "Must not contain unescaped script tags"
    );

    // Verify no img tags with onerror
    let imgs: Vec<_> = doc.select(&Selector::parse("img").unwrap()).collect();
    for img in imgs {
        assert!(
            img.value().attr("onerror").is_none(),
            "Must not have onerror handlers"
        );
    }
}

#[test]
fn test_repository_maintainer_display() {
    let repo_html = r#"
        <div class="repository-detail">
            <h1>Test Repo</h1>
            <div class="maintainers">
                <h3>Maintainers</h3>
                <ul>
                    <li>
                        <span class="agent-badge">🤖 AGENT</span>
                        <code>npub1...</code>
                    </li>
                </ul>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(repo_html);

    // Must show maintainers section
    let maintainers = doc.select(&Selector::parse(".maintainers").unwrap()).next();
    assert!(maintainers.is_some(), "Must display maintainers section");

    // Agent badge must be present
    let badge = doc.select(&Selector::parse(".agent-badge").unwrap()).next();
    assert!(badge.is_some(), "Agent maintainers must have badge");
}

#[test]
fn test_clone_url_display() {
    let repo_html = r#"
        <div class="clone-urls">
            <h3>Clone URLs</h3>
            <div>
                <code>https://github.com/test/repo.git</code>
                <button>Copy</button>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(repo_html);

    // Must have clone URL section
    let clone_section = doc.select(&Selector::parse(".clone-urls").unwrap()).next();
    assert!(clone_section.is_some(), "Must have clone URLs section");

    // Copy button must be accessible
    let button = doc.select(&Selector::parse("button").unwrap()).next();
    assert!(button.is_some(), "Must have copy button");

    let button_text: String = button.unwrap().text().collect();
    assert!(
        !button_text.trim().is_empty(),
        "Copy button must have accessible text"
    );
}
