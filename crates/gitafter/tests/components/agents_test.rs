//! Component tests for agent profile and reputation views

use scraper::{Html, Selector};

#[test]
fn test_agent_profile_structure() {
    let profile_html = r#"
        <div class="agent-profile">
            <div class="agent-header">
                <span class="agent-badge">🤖 AGENT</span>
                <code class="npub">npub1...</code>
            </div>
            <div class="agent-stats">
                <div>
                    <strong>10</strong>
                    <span>Merged PRs</span>
                </div>
                <div>
                    <strong>2</strong>
                    <span>Rejected PRs</span>
                </div>
                <div>
                    <strong>15</strong>
                    <span>Issues Fixed</span>
                </div>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(profile_html);

    // Must have agent badge
    let badge = doc.select(&Selector::parse(".agent-badge").unwrap()).next();
    assert!(badge.is_some(), "Profile must have agent badge");

    // Must show npub
    let npub = doc.select(&Selector::parse(".npub").unwrap()).next();
    assert!(npub.is_some(), "Profile must show npub");

    // Stats must be present
    let stats = doc.select(&Selector::parse(".agent-stats").unwrap()).next();
    assert!(stats.is_some(), "Profile must show statistics");
}

#[test]
fn test_reputation_score_display() {
    let reputation_html = r#"
        <div class="reputation">
            <h4>Reputation Score</h4>
            <div class="score">
                <span class="score-value">0.85</span>
                <span class="score-label">High Quality</span>
            </div>
        </div>
    "#;

    let doc = Html::parse_fragment(reputation_html);

    // Score must be visible
    let score = doc.select(&Selector::parse(".score-value").unwrap()).next();
    assert!(score.is_some(), "Reputation score must be displayed");

    let score_text: String = score.unwrap().text().collect();
    assert!(
        !score_text.trim().is_empty(),
        "Score must have visible value"
    );

    // Label must provide context
    let label = doc.select(&Selector::parse(".score-label").unwrap()).next();
    assert!(label.is_some(), "Score must have descriptive label");
}

#[test]
fn test_contribution_history_list() {
    let history_html = r#"
        <div class="contribution-history">
            <h4>Recent Contributions</h4>
            <ul>
                <li>
                    <a href="/prs/1">PR #1: Add feature X</a>
                    <span class="status merged">Merged</span>
                </li>
                <li>
                    <a href="/prs/2">PR #2: Fix bug Y</a>
                    <span class="status open">Open</span>
                </li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(history_html);

    // Must have heading
    let heading = doc.select(&Selector::parse("h4").unwrap()).next();
    assert!(heading.is_some(), "Contribution history must have heading");

    // Must use list
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Contributions must be in list");

    // Each contribution must have link
    let items: Vec<_> = doc.select(&Selector::parse("li").unwrap()).collect();
    for item in items {
        let link = item.select(&Selector::parse("a").unwrap()).next();
        assert!(link.is_some(), "Each contribution must have link");
    }
}

#[test]
fn test_reputation_metrics_table() {
    let metrics_html = r#"
        <table class="reputation-metrics">
            <thead>
                <tr>
                    <th>Metric</th>
                    <th>Value</th>
                </tr>
            </thead>
            <tbody>
                <tr>
                    <td>Quality Score</td>
                    <td>0.85</td>
                </tr>
                <tr>
                    <td>Merged PRs</td>
                    <td>12</td>
                </tr>
                <tr>
                    <td>Total Commits</td>
                    <td>150</td>
                </tr>
            </tbody>
        </table>
    "#;

    let doc = Html::parse_fragment(metrics_html);

    // Must use table with headers
    let table = doc.select(&Selector::parse("table").unwrap()).next();
    assert!(table.is_some(), "Metrics must use table element");

    let thead = doc.select(&Selector::parse("thead").unwrap()).next();
    assert!(thead.is_some(), "Table must have thead");

    let tbody = doc.select(&Selector::parse("tbody").unwrap()).next();
    assert!(tbody.is_some(), "Table must have tbody");
}

#[test]
fn test_agent_filter_by_reputation() {
    let filter_html = r#"
        <form class="agent-filter">
            <label for="min-score">Minimum reputation score:</label>
            <input type="number" id="min-score" name="min_score" min="0" max="1" step="0.1" value="0.7">
            <button type="submit">Filter</button>
        </form>
    "#;

    let doc = Html::parse_fragment(filter_html);

    // Input must have label
    let label = doc.select(&Selector::parse("label").unwrap()).next();
    assert!(label.is_some(), "Filter input must have label");

    let label_for = label.unwrap().value().attr("for");
    assert_eq!(
        label_for,
        Some("min-score"),
        "Label must reference input by id"
    );

    // Input must have constraints
    let input = doc.select(&Selector::parse("input").unwrap()).next().unwrap();
    assert!(
        input.value().attr("min").is_some(),
        "Number input must have min constraint"
    );
    assert!(
        input.value().attr("max").is_some(),
        "Number input must have max constraint"
    );
}

#[test]
fn test_reputation_label_display() {
    let label_html = r#"
        <div class="reputation-labels">
            <span class="label trusted">Trusted</span>
            <span class="label verified">Verified</span>
        </div>
    "#;

    let doc = Html::parse_fragment(label_html);

    // Labels must be visible
    let labels: Vec<_> = doc.select(&Selector::parse(".label").unwrap()).collect();
    assert!(!labels.is_empty(), "Must display reputation labels");

    // Each label must have text
    for label in labels {
        let text: String = label.text().collect();
        assert!(!text.trim().is_empty(), "Label must have text");
    }
}

#[test]
fn test_oracle_attribution() {
    let oracle_html = r#"
        <div class="reputation-oracle">
            <p>Assessed by oracle: <code>npub1oracle...</code></p>
        </div>
    "#;

    let doc = Html::parse_fragment(oracle_html);

    // Oracle pubkey must be in code
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Oracle pubkey must use code element");
}

#[test]
fn test_negative_reputation_indicator() {
    let negative_html = r#"
        <div class="reputation warning">
            <span class="label spam">Spam</span>
            <p>Quality score: 0.10</p>
            <p>Spam reports: 5</p>
        </div>
    "#;

    let doc = Html::parse_fragment(negative_html);

    // Warning class must be present
    let warning_div = doc.select(&Selector::parse(".warning").unwrap()).next();
    assert!(warning_div.is_some(), "Negative reputation must show warning");

    // Spam label must be visible
    let spam = doc.select(&Selector::parse(".spam").unwrap()).next();
    assert!(spam.is_some(), "Must show spam indicator");
}

#[test]
fn test_empty_agent_list() {
    let empty_html = r#"
        <div class="agent-list">
            <h2>Agents</h2>
            <p>No agents found matching filters.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(empty_html);

    let message = doc.select(&Selector::parse("p").unwrap()).next();
    assert!(message.is_some(), "Empty list must have message");

    let text: String = message.unwrap().text().collect();
    assert!(
        text.contains("No agents"),
        "Empty message must be informative"
    );
}

#[test]
fn test_agent_specialties_display() {
    let specialties_html = r#"
        <div class="agent-specialties">
            <h4>Specialties</h4>
            <ul>
                <li>Rust</li>
                <li>TypeScript</li>
                <li>Systems Programming</li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(specialties_html);

    // Must have heading
    let heading = doc.select(&Selector::parse("h4").unwrap()).next();
    assert!(heading.is_some(), "Specialties must have heading");

    // Must use list
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Specialties must be in list");
}
