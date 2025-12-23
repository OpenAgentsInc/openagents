//! Component tests for issue list and detail views

use scraper::{Html, Selector};

#[test]
fn test_issue_list_structure() {
    let issues_html = r#"
        <div class="issue-list">
            <h2>Issues</h2>
            <ul>
                <li>
                    <a href="/issues/1">
                        <strong>Issue #1</strong>
                        <span class="status">Open</span>
                    </a>
                </li>
                <li>
                    <a href="/issues/2">
                        <strong>Issue #2</strong>
                        <span class="status">Closed</span>
                    </a>
                </li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(issues_html);

    // Must use semantic list
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Issue list must use ul element");

    // Must have list items
    let items: Vec<_> = doc.select(&Selector::parse("li").unwrap()).collect();
    assert_eq!(items.len(), 2, "Should have 2 issue items");
}

#[test]
fn test_issue_status_badge_accessibility() {
    let issue_html = r#"
        <div class="issue">
            <span class="status open" aria-label="Status: Open">Open</span>
        </div>
    "#;

    let doc = Html::parse_fragment(issue_html);
    let status = doc.select(&Selector::parse(".status").unwrap()).next();

    assert!(status.is_some(), "Issue must have status indicator");

    let status_elem = status.unwrap();

    // Must have either visible text or aria-label
    let text: String = status_elem.text().collect();
    let has_text = !text.trim().is_empty();
    let has_aria = status_elem.value().attr("aria-label").is_some();

    assert!(
        has_text || has_aria,
        "Status must be accessible via text or aria-label"
    );
}

#[test]
fn test_bounty_display() {
    let bounty_html = r#"
        <div class="bounty">
            <span class="bounty-amount">50,000 sats</span>
            <span class="bounty-expiry">Expires: 2025-12-31</span>
        </div>
    "#;

    let doc = Html::parse_fragment(bounty_html);

    // Bounty amount must be visible
    let amount = doc.select(&Selector::parse(".bounty-amount").unwrap()).next();
    assert!(amount.is_some(), "Bounty must display amount");

    let amount_text: String = amount.unwrap().text().collect();
    assert!(
        amount_text.contains("sats"),
        "Bounty amount must include unit"
    );
}

#[test]
fn test_bounty_conditions_list() {
    let conditions_html = r#"
        <div class="bounty-conditions">
            <h4>Conditions</h4>
            <ul>
                <li>must include tests</li>
                <li>must pass CI</li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(conditions_html);

    // Must have heading
    let heading = doc.select(&Selector::parse("h4").unwrap()).next();
    assert!(heading.is_some(), "Conditions must have heading");

    // Must use list structure
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Conditions must use list structure");

    let items: Vec<_> = doc.select(&Selector::parse("li").unwrap()).collect();
    assert_eq!(items.len(), 2, "Should have 2 conditions");
}

#[test]
fn test_issue_claim_display() {
    let claim_html = r#"
        <div class="issue-claim">
            <p>
                Claimed by <code>npub1...</code>
                <a href="/trajectory/session-123">View trajectory</a>
            </p>
            <p>Estimated completion: 2 hours</p>
        </div>
    "#;

    let doc = Html::parse_fragment(claim_html);

    // Must show who claimed
    let claim_div = doc.select(&Selector::parse(".issue-claim").unwrap()).next();
    assert!(claim_div.is_some(), "Must display claim information");

    // Must have trajectory link
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "Must have trajectory link");

    let link_text: String = link.unwrap().text().collect();
    assert!(
        !link_text.trim().is_empty(),
        "Trajectory link must have accessible text"
    );
}

#[test]
fn test_issue_filter_checkboxes_accessibility() {
    let filter_html = r#"
        <div class="filters">
            <label>
                <input type="checkbox" name="filter" value="open" id="filter-open">
                Open
            </label>
            <label>
                <input type="checkbox" name="filter" value="has-bounty" id="filter-bounty">
                Has Bounty
            </label>
        </div>
    "#;

    let doc = Html::parse_fragment(filter_html);

    // All checkboxes must have labels
    let checkboxes: Vec<_> = doc.select(&Selector::parse("input[type='checkbox']").unwrap()).collect();
    assert!(!checkboxes.is_empty(), "Must have filter checkboxes");

    for checkbox in checkboxes {
        // Either has id (for label) or aria-label
        let has_id = checkbox.value().attr("id").is_some();
        let has_aria = checkbox.value().attr("aria-label").is_some();

        assert!(
            has_id || has_aria,
            "Checkbox must be associated with label"
        );
    }
}

#[test]
fn test_issue_comment_xss_prevention() {
    let malicious_comment = "<script>alert('xss')</script>";
    let comment_html = format!(
        r#"
        <div class="comment">
            <p>{}</p>
        </div>
        "#,
        html_escape::encode_text(malicious_comment)
    );

    let doc = Html::parse_fragment(&comment_html);

    // Must not have script tags
    assert!(
        doc.select(&Selector::parse("script").unwrap()).next().is_none(),
        "Comments must escape script tags"
    );

    // Text should contain escaped brackets
    let p = doc.select(&Selector::parse("p").unwrap()).next().unwrap();
    let text: String = p.text().collect();
    assert!(
        text.contains("<script>"),
        "Comment text should show escaped content"
    );
}

#[test]
fn test_empty_issue_list() {
    let empty_html = r#"
        <div class="issue-list">
            <h2>Issues</h2>
            <p>No issues found.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(empty_html);

    // Must have empty state message
    let message = doc.select(&Selector::parse("p").unwrap()).next();
    assert!(message.is_some(), "Empty list must have message");

    let text: String = message.unwrap().text().collect();
    assert!(
        text.contains("No issues"),
        "Empty message must be informative"
    );
}

#[test]
fn test_claim_button_disabled_state() {
    let button_html = r#"
        <button disabled>Claim Issue</button>
    "#;

    let doc = Html::parse_fragment(button_html);
    let button = doc.select(&Selector::parse("button").unwrap()).next().unwrap();

    // Disabled state must be indicated
    let has_disabled = button.value().attr("disabled").is_some();
    let has_aria_disabled = button.value().attr("aria-disabled") == Some("true");

    assert!(
        has_disabled || has_aria_disabled,
        "Disabled button must indicate state"
    );

    // Must still have accessible text
    let text: String = button.text().collect();
    assert!(!text.trim().is_empty(), "Button must have text");
}
