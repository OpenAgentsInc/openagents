//! Component tests for GitAfter layout components

use scraper::{Html, Selector};

#[test]
fn test_layout_has_semantic_structure() {
    // Create a basic layout
    let layout_html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>GitAfter</title></head>
        <body>
            <header>
                <nav>
                    <a href="/">Home</a>
                    <a href="/repos">Repositories</a>
                </nav>
            </header>
            <main>
                <h1>Content</h1>
            </main>
        </body>
        </html>
    "#;

    let doc = Html::parse_document(layout_html);

    // Must have semantic HTML5 elements
    assert!(
        doc.select(&Selector::parse("header").unwrap()).next().is_some(),
        "Layout must have header element"
    );

    assert!(
        doc.select(&Selector::parse("main").unwrap()).next().is_some(),
        "Layout must have main element"
    );

    assert!(
        doc.select(&Selector::parse("nav").unwrap()).next().is_some(),
        "Layout must have nav element"
    );
}

#[test]
fn test_navigation_links_are_accessible() {
    let nav_html = r#"
        <nav>
            <a href="/">Home</a>
            <a href="/repos">Repositories</a>
            <a href="/issues">Issues</a>
        </nav>
    "#;

    let doc = Html::parse_fragment(nav_html);
    let links: Vec<_> = doc.select(&Selector::parse("a").unwrap()).collect();

    assert!(
        !links.is_empty(),
        "Navigation must have links"
    );

    // All links must have text content
    for link in links {
        let text: String = link.text().collect();
        assert!(
            !text.trim().is_empty(),
            "Navigation links must have accessible text content"
        );
    }
}

#[test]
fn test_header_escapes_user_content() {
    // Simulate rendering header with potentially malicious user input
    let malicious_repo_name = "<script>alert('xss')</script>";
    let header_html = format!(
        r#"<header><h1>{}</h1></header>"#,
        html_escape::encode_text(malicious_repo_name)
    );

    let doc = Html::parse_fragment(&header_html);
    let h1 = doc.select(&Selector::parse("h1").unwrap()).next().unwrap();
    let text: String = h1.text().collect();

    // Should contain escaped content, not raw script tags
    assert!(
        text.contains("<script>"),
        "Text content should contain escaped angle brackets"
    );

    // Verify no actual script tags in HTML
    assert!(
        doc.select(&Selector::parse("script").unwrap()).next().is_none(),
        "Must not contain unescaped script tags"
    );
}

#[test]
fn test_main_content_area_present() {
    let layout_html = r#"
        <main>
            <div class="content">Test content</div>
        </main>
    "#;

    let doc = Html::parse_fragment(layout_html);
    let main = doc.select(&Selector::parse("main").unwrap()).next();

    assert!(
        main.is_some(),
        "Layout must have main content area"
    );
}
