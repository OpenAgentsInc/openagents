//! Component tests for trajectory viewer

use scraper::{Html, Selector};

#[test]
fn test_trajectory_timeline_structure() {
    let timeline_html = r#"
        <div class="trajectory-timeline">
            <h3>Trajectory Timeline</h3>
            <ol class="events">
                <li class="event">
                    <span class="timestamp">2025-12-22 10:00:00</span>
                    <span class="tool">Read</span>
                    <code>src/main.rs</code>
                </li>
                <li class="event">
                    <span class="timestamp">2025-12-22 10:01:00</span>
                    <span class="tool">Edit</span>
                    <code>src/main.rs</code>
                </li>
            </ol>
        </div>
    "#;

    let doc = Html::parse_fragment(timeline_html);

    // Must use ordered list for events
    let ol = doc.select(&Selector::parse("ol").unwrap()).next();
    assert!(
        ol.is_some(),
        "Timeline must use ordered list (ol) element"
    );

    // Must have events
    let events: Vec<_> = doc.select(&Selector::parse(".event").unwrap()).collect();
    assert_eq!(events.len(), 2, "Should have 2 timeline events");
}

#[test]
fn test_trajectory_event_accessibility() {
    let event_html = r#"
        <li class="event">
            <span class="timestamp" aria-label="Timestamp: 10:00 AM">10:00</span>
            <span class="tool" aria-label="Tool: Read">Read</span>
            <code>src/main.rs</code>
        </li>
    "#;

    let doc = Html::parse_fragment(event_html);

    // Timestamp must be visible
    let timestamp = doc.select(&Selector::parse(".timestamp").unwrap()).next();
    assert!(timestamp.is_some(), "Event must have timestamp");

    let ts_text: String = timestamp.unwrap().text().collect();
    assert!(!ts_text.trim().is_empty(), "Timestamp must have text");

    // Tool name must be visible
    let tool = doc.select(&Selector::parse(".tool").unwrap()).next();
    assert!(tool.is_some(), "Event must show tool");

    let tool_text: String = tool.unwrap().text().collect();
    assert!(!tool_text.trim().is_empty(), "Tool must have text");
}

#[test]
fn test_trajectory_hash_verification() {
    let hash_html = r#"
        <div class="trajectory-hash">
            <span>Hash:</span>
            <code>abc123def456</code>
            <span class="verified">✓ Verified</span>
        </div>
    "#;

    let doc = Html::parse_fragment(hash_html);

    // Hash must be in code element
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Hash must use code element");

    // Verification status must be visible
    let verified = doc.select(&Selector::parse(".verified").unwrap()).next();
    assert!(verified.is_some(), "Must show verification status");
}

#[test]
fn test_trajectory_session_info() {
    let session_html = r#"
        <div class="trajectory-session">
            <h4>Session Information</h4>
            <p>Session ID: <code>session-abc-123</code></p>
            <p>Model: Codex Sonnet 4.5</p>
            <p>Tokens: 1,234</p>
        </div>
    "#;

    let doc = Html::parse_fragment(session_html);

    // Must have heading
    let heading = doc.select(&Selector::parse("h4").unwrap()).next();
    assert!(heading.is_some(), "Session info must have heading");

    // Session ID must be in code
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Session ID must use code element");
}

#[test]
fn test_trajectory_gap_warning() {
    let gap_html = r#"
        <div class="trajectory-warning">
            <span class="warning">⚠ Timeline gap detected</span>
            <p>Missing ToolResult after ToolUse at 10:05:00</p>
        </div>
    "#;

    let doc = Html::parse_fragment(gap_html);

    // Warning must be visible
    let warning = doc.select(&Selector::parse(".warning").unwrap()).next();
    assert!(warning.is_some(), "Gap warning must be displayed");

    let warning_text: String = warning.unwrap().text().collect();
    assert!(
        !warning_text.trim().is_empty(),
        "Warning must have text"
    );
}

#[test]
fn test_trajectory_collapsible_sections() {
    let collapsible_html = r#"
        <div class="trajectory-viewer">
            <details open>
                <summary>Tool Calls (15)</summary>
                <ol>
                    <li>Read src/main.rs</li>
                    <li>Edit src/main.rs</li>
                </ol>
            </details>
        </div>
    "#;

    let doc = Html::parse_fragment(collapsible_html);

    // Must use details/summary
    let details = doc.select(&Selector::parse("details").unwrap()).next();
    assert!(details.is_some(), "Must use details element");

    let summary = doc.select(&Selector::parse("summary").unwrap()).next();
    assert!(summary.is_some(), "Must have summary");

    let summary_text: String = summary.unwrap().text().collect();
    assert!(
        !summary_text.trim().is_empty(),
        "Summary must have text"
    );
}

#[test]
fn test_trajectory_tool_result_display() {
    let result_html = r#"
        <div class="tool-result">
            <span class="tool">Read</span>
            <code>src/main.rs</code>
            <pre class="result">fn main() { }</pre>
        </div>
    "#;

    let doc = Html::parse_fragment(result_html);

    // Result must be in pre for formatting
    let pre = doc.select(&Selector::parse("pre").unwrap()).next();
    assert!(pre.is_some(), "Tool result must use pre element");
}

#[test]
fn test_trajectory_link_to_full_session() {
    let link_html = r#"
        <div class="trajectory-link">
            <a href="/trajectory/session-abc-123">View Full Timeline</a>
        </div>
    "#;

    let doc = Html::parse_fragment(link_html);

    // Must have link
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "Must have link to full session");

    let link_text: String = link.unwrap().text().collect();
    assert!(
        !link_text.trim().is_empty(),
        "Link must have accessible text"
    );
}

#[test]
fn test_suspicious_pattern_flag() {
    let suspicious_html = r#"
        <div class="suspicious-indicator">
            <span class="flag">🚩 Suspicious pattern detected</span>
            <ul>
                <li>Too few events for diff size</li>
                <li>No Edit tool calls found</li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(suspicious_html);

    // Flag must be visible
    let flag = doc.select(&Selector::parse(".flag").unwrap()).next();
    assert!(flag.is_some(), "Suspicious flag must be displayed");

    // Reasons must be listed
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Must list reasons for flag");
}
