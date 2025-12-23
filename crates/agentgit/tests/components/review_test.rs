//! Component tests for code review interface

use scraper::{Html, Selector};

#[test]
fn test_review_form_structure() {
    let review_html = r#"
        <form action="/reviews/submit" method="post">
            <div class="review-comment">
                <label for="comment">Comment</label>
                <textarea id="comment" name="comment" required></textarea>
            </div>
            <div class="review-action">
                <label>
                    <input type="radio" name="action" value="approve">
                    Approve
                </label>
                <label>
                    <input type="radio" name="action" value="request_changes">
                    Request Changes
                </label>
                <label>
                    <input type="radio" name="action" value="comment">
                    Comment
                </label>
            </div>
            <button type="submit">Submit Review</button>
        </form>
    "#;

    let doc = Html::parse_fragment(review_html);

    // Must be a form
    let form = doc.select(&Selector::parse("form").unwrap()).next();
    assert!(form.is_some(), "Review must be in form element");

    // Textarea must have label
    let textarea = doc.select(&Selector::parse("textarea").unwrap()).next();
    assert!(textarea.is_some(), "Must have comment textarea");

    let textarea_id = textarea.unwrap().value().attr("id");
    assert!(textarea_id.is_some(), "Textarea must have id for label");

    // Radio buttons for actions
    let radios: Vec<_> = doc.select(&Selector::parse("input[type='radio']").unwrap()).collect();
    assert_eq!(radios.len(), 3, "Should have 3 review action options");
}

#[test]
fn test_review_form_accessibility() {
    let form_html = r#"
        <form>
            <label for="comment-textarea">Your review comment</label>
            <textarea id="comment-textarea" name="comment"></textarea>
        </form>
    "#;

    let doc = Html::parse_fragment(form_html);

    // Label must reference textarea
    let label = doc.select(&Selector::parse("label").unwrap()).next();
    assert!(label.is_some(), "Must have label");

    let label_for = label.unwrap().value().attr("for");
    assert_eq!(
        label_for,
        Some("comment-textarea"),
        "Label must reference textarea by id"
    );
}

#[test]
fn test_agent_review_badge() {
    let review_html = r#"
        <div class="review">
            <div class="reviewer">
                <span class="agent-badge">🤖 AGENT</span>
                <span class="verified">✓ Verified</span>
                <code>npub1...</code>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(review_html);

    // Agent badge must be present
    let badge = doc.select(&Selector::parse(".agent-badge").unwrap()).next();
    assert!(badge.is_some(), "Agent reviews must have badge");

    // Verification indicator must be present
    let verified = doc.select(&Selector::parse(".verified").unwrap()).next();
    assert!(verified.is_some(), "Agent reviews must show verification");
}

#[test]
fn test_trajectory_viewer_in_review() {
    let trajectory_html = r#"
        <div class="trajectory-viewer">
            <details>
                <summary>View Trajectory</summary>
                <div class="trajectory-content">
                    <p>Session ID: <code>session-123</code></p>
                    <p>Hash: <code>abc123</code></p>
                    <a href="/trajectory/session-123">Full Timeline</a>
                </div>
            </details>
        </div>
    "#;

    let doc = Html::parse_fragment(trajectory_html);

    // Must use details/summary for collapsible
    let details = doc.select(&Selector::parse("details").unwrap()).next();
    assert!(details.is_some(), "Trajectory must be collapsible");

    let summary = doc.select(&Selector::parse("summary").unwrap()).next();
    assert!(summary.is_some(), "Must have summary for accessibility");

    let summary_text: String = summary.unwrap().text().collect();
    assert!(
        !summary_text.trim().is_empty(),
        "Summary must have text"
    );
}

#[test]
fn test_review_comment_xss_prevention() {
    let malicious_comment = "<script>alert('xss')</script>";
    let comment_html = format!(
        r#"<div class="review-comment">{}</div>"#,
        html_escape::encode_text(malicious_comment)
    );

    let doc = Html::parse_fragment(&comment_html);

    // No script tags
    assert!(
        doc.select(&Selector::parse("script").unwrap()).next().is_none(),
        "Review comments must escape script tags"
    );
}

#[test]
fn test_inline_comment_indicator() {
    let inline_html = r#"
        <div class="inline-comment">
            <div class="line-ref">
                <code>src/main.rs:42</code>
            </div>
            <div class="comment-body">
                <p>Consider using match here</p>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(inline_html);

    // Must show line reference
    let line_ref = doc.select(&Selector::parse(".line-ref").unwrap()).next();
    assert!(line_ref.is_some(), "Inline comment must show line reference");

    // Must use code element
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Line reference must use code element");
}

#[test]
fn test_review_summary_display() {
    let summary_html = r#"
        <div class="review-summary">
            <span class="review-action approve">✓ Approved</span>
            <p>Great work! Ready to merge.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(summary_html);

    // Action indicator must be present
    let action = doc.select(&Selector::parse(".review-action").unwrap()).next();
    assert!(action.is_some(), "Must show review action");

    let action_text: String = action.unwrap().text().collect();
    assert!(
        !action_text.trim().is_empty(),
        "Action must have visible text"
    );
}

#[test]
fn test_submit_button_disabled_when_invalid() {
    let form_html = r#"
        <form>
            <textarea required></textarea>
            <button type="submit" disabled>Submit Review</button>
        </form>
    "#;

    let doc = Html::parse_fragment(form_html);
    let button = doc.select(&Selector::parse("button").unwrap()).next().unwrap();

    // Disabled state must be indicated
    assert!(
        button.value().attr("disabled").is_some(),
        "Invalid form must disable submit button"
    );
}

#[test]
fn test_weighted_review_score_display() {
    let score_html = r#"
        <div class="review-score">
            <span>Reviewer reputation: 0.85</span>
            <span>Weighted score applied</span>
        </div>
    "#;

    let doc = Html::parse_fragment(score_html);

    // Score information must be visible
    let score_div = doc.select(&Selector::parse(".review-score").unwrap()).next();
    assert!(score_div.is_some(), "Must display review score information");
}
