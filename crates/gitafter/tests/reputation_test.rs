//! Integration tests for GitAfter agent reputation tracking and filtering

mod helpers;

use anyhow::Result;
use helpers::test_app::TestApp;
use nostr::EventTemplate;

#[tokio::test]
async fn test_create_reputation_label() -> Result<()> {
    let app = TestApp::new().await?;

    // Create a reputation label for an agent (kind:1985, NIP-32)
    let agent_pubkey = "agent123pubkey456";

    let label_template = EventTemplate {
        kind: 1985, // LABEL
        tags: vec![
            vec![
                "L".to_string(), // Label namespace
                "gitafter.reputation".to_string(),
            ],
            vec![
                "l".to_string(), // Label value
                "trusted".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec![
                "p".to_string(), // Agent being labeled
                agent_pubkey.to_string(),
            ],
            vec![
                "quality".to_string(), // Reputation quality indicator
                "0.85".to_string(),
            ],
            vec!["merged_prs".to_string(), "12".to_string()],
            vec!["rejected_prs".to_string(), "2".to_string()],
            vec!["issues_fixed".to_string(), "15".to_string()],
        ],
        content: "High-quality contributor with strong track record".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let label = app.publish_event(label_template).await?;

    // Verify label structure
    assert_eq!(label.kind, 1985);

    // Verify namespace tag
    let namespace_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "L").unwrap_or(false))
        .expect("L tag should exist");
    assert_eq!(
        namespace_tag.get(1),
        Some(&"gitafter.reputation".to_string())
    );

    // Verify label value
    let label_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "l").unwrap_or(false))
        .expect("l tag should exist");
    assert_eq!(label_tag.get(1), Some(&"trusted".to_string()));

    // Verify agent reference
    let agent_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "p").unwrap_or(false))
        .expect("p tag should exist");
    assert_eq!(agent_tag.get(1), Some(&agent_pubkey.to_string()));

    // Verify quality score
    let quality_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "quality").unwrap_or(false))
        .expect("quality tag should exist");
    assert_eq!(quality_tag.get(1), Some(&"0.85".to_string()));

    // Verify metrics
    let merged_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "merged_prs").unwrap_or(false))
        .expect("merged_prs tag should exist");
    assert_eq!(merged_tag.get(1), Some(&"12".to_string()));

    // Verify label stored
    let labels = app.relay.get_events_by_kind(1985).await;
    assert_eq!(labels.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_multiple_reputation_labels_for_agent() -> Result<()> {
    let app = TestApp::new().await?;

    let agent_pubkey = "agent-xyz-789";

    // First reputation label (high quality)
    let label1_template = EventTemplate {
        kind: 1985,
        tags: vec![
            vec!["L".to_string(), "gitafter.reputation".to_string()],
            vec![
                "l".to_string(),
                "trusted".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec!["p".to_string(), agent_pubkey.to_string()],
            vec!["quality".to_string(), "0.90".to_string()],
            vec!["merged_prs".to_string(), "20".to_string()],
        ],
        content: "Excellent contributor".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let _label1 = app.publish_event(label1_template).await?;

    // Second reputation label (different labeler or updated)
    let label2_template = EventTemplate {
        kind: 1985,
        tags: vec![
            vec!["L".to_string(), "gitafter.reputation".to_string()],
            vec![
                "l".to_string(),
                "verified".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec!["p".to_string(), agent_pubkey.to_string()],
            vec!["quality".to_string(), "0.88".to_string()],
            vec!["issues_fixed".to_string(), "25".to_string()],
        ],
        content: "Verified agent with good track record".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let _label2 = app.publish_event(label2_template).await?;

    // Verify both labels stored
    let labels = app.relay.get_events_by_kind(1985).await;
    assert_eq!(labels.len(), 2);

    // Verify both reference same agent
    for label in &labels {
        let agent_tag = label
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "p").unwrap_or(false))
            .expect("p tag should exist");
        assert_eq!(agent_tag.get(1), Some(&agent_pubkey.to_string()));
    }

    Ok(())
}

#[tokio::test]
async fn test_reputation_score_calculation() -> Result<()> {
    let app = TestApp::new().await?;

    let agent_pubkey = "agent-score-test";

    // Create label with detailed metrics for score calculation
    let label_template = EventTemplate {
        kind: 1985,
        tags: vec![
            vec!["L".to_string(), "gitafter.reputation".to_string()],
            vec![
                "l".to_string(),
                "trusted".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec!["p".to_string(), agent_pubkey.to_string()],
            vec!["quality".to_string(), "0.75".to_string()],
            vec!["merged_prs".to_string(), "10".to_string()],
            vec!["rejected_prs".to_string(), "3".to_string()],
            vec!["issues_fixed".to_string(), "8".to_string()],
            vec!["total_commits".to_string(), "150".to_string()],
            vec!["code_reviews".to_string(), "25".to_string()],
        ],
        content: "Detailed reputation metrics".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let label = app.publish_event(label_template).await?;

    // Verify all metric tags present
    let metrics = [
        "merged_prs",
        "rejected_prs",
        "issues_fixed",
        "total_commits",
        "code_reviews",
    ];

    for metric in &metrics {
        let tag = label
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == *metric).unwrap_or(false))
            .expect(&format!("{} tag should exist", metric));
        assert!(tag.get(1).is_some());
    }

    // In a real implementation, we would:
    // - Parse these metrics
    // - Calculate a composite score
    // - Compare against thresholds
    // For now, we verify the quality score is accessible
    let quality_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "quality").unwrap_or(false))
        .expect("quality tag should exist");

    let quality_value = quality_tag.get(1).unwrap().parse::<f64>().unwrap();
    assert!(quality_value > 0.0 && quality_value <= 1.0);

    Ok(())
}

#[tokio::test]
async fn test_negative_reputation_labels() -> Result<()> {
    let app = TestApp::new().await?;

    let agent_pubkey = "agent-negative-test";

    // Create negative reputation label
    let label_template = EventTemplate {
        kind: 1985,
        tags: vec![
            vec!["L".to_string(), "gitafter.reputation".to_string()],
            vec![
                "l".to_string(),
                "spam".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec!["p".to_string(), agent_pubkey.to_string()],
            vec!["quality".to_string(), "0.10".to_string()],
            vec!["spam_reports".to_string(), "5".to_string()],
            vec!["merged_prs".to_string(), "0".to_string()],
            vec!["rejected_prs".to_string(), "8".to_string()],
        ],
        content: "Multiple spam PRs detected".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let label = app.publish_event(label_template).await?;

    // Verify negative label
    let label_value_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "l").unwrap_or(false))
        .expect("l tag should exist");
    assert_eq!(label_value_tag.get(1), Some(&"spam".to_string()));

    // Verify low quality score
    let quality_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "quality").unwrap_or(false))
        .expect("quality tag should exist");

    let quality_value = quality_tag.get(1).unwrap().parse::<f64>().unwrap();
    assert!(quality_value < 0.5);

    Ok(())
}

#[tokio::test]
async fn test_agent_contribution_history() -> Result<()> {
    let app = TestApp::new().await?;

    let agent_pubkey = app.pubkey();

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create multiple PRs to build contribution history
    for i in 0..3 {
        let pr_template = EventTemplate {
            kind: 1618,
            tags: vec![
                vec!["a".to_string(), format!("30617:{}:test-repo", agent_pubkey)],
                vec!["subject".to_string(), format!("PR #{}", i + 1)],
                vec!["c".to_string(), format!("commit{}", i + 1)],
            ],
            content: format!("PR number {}", i + 1),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        app.publish_event(pr_template).await?;
    }

    // Verify PRs stored
    let prs = app.relay.get_events_by_kind(1618).await;
    assert_eq!(prs.len(), 3);

    // Verify all PRs from same agent
    for pr in &prs {
        assert_eq!(pr.pubkey, agent_pubkey);
    }

    // Create reputation label based on this contribution history
    let label_template = EventTemplate {
        kind: 1985,
        tags: vec![
            vec!["L".to_string(), "gitafter.reputation".to_string()],
            vec![
                "l".to_string(),
                "active".to_string(),
                "gitafter.reputation".to_string(),
            ],
            vec!["p".to_string(), agent_pubkey.clone()],
            vec!["quality".to_string(), "0.80".to_string()],
            vec!["merged_prs".to_string(), "3".to_string()],
            vec!["active_repos".to_string(), "1".to_string()],
        ],
        content: "Active contributor".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let label = app.publish_event(label_template).await?;

    // Verify label reflects contribution count
    let merged_tag = label
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "merged_prs").unwrap_or(false))
        .expect("merged_prs tag should exist");
    assert_eq!(merged_tag.get(1), Some(&"3".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_reputation_oracle_publishing() -> Result<()> {
    let app = TestApp::new().await?;

    // Oracle publishes reputation labels for multiple agents
    let agents = ["agent1", "agent2", "agent3"];

    for (i, agent) in agents.iter().enumerate() {
        let quality = 0.5 + (i as f64 * 0.15); // 0.5, 0.65, 0.80

        let label_template = EventTemplate {
            kind: 1985,
            tags: vec![
                vec!["L".to_string(), "gitafter.reputation".to_string()],
                vec![
                    "l".to_string(),
                    if quality > 0.7 {
                        "trusted"
                    } else {
                        "unverified"
                    }
                    .to_string(),
                    "gitafter.reputation".to_string(),
                ],
                vec!["p".to_string(), agent.to_string()],
                vec!["quality".to_string(), format!("{:.2}", quality)],
                vec!["oracle".to_string(), app.pubkey()], // Oracle identifier
            ],
            content: format!("Oracle reputation assessment for {}", agent),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        app.publish_event(label_template).await?;
    }

    // Verify all labels published by oracle
    let labels = app.relay.get_events_by_kind(1985).await;
    assert_eq!(labels.len(), 3);

    // Verify oracle tag on all labels
    for label in &labels {
        let oracle_tag = label
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "oracle").unwrap_or(false))
            .expect("oracle tag should exist");
        assert_eq!(oracle_tag.get(1), Some(&app.pubkey()));
    }

    Ok(())
}

#[tokio::test]
async fn test_filter_agents_by_reputation_threshold() -> Result<()> {
    let app = TestApp::new().await?;

    // Create labels for different agents with varying quality
    let agents_data = [
        ("agent-high", 0.90, "trusted"),
        ("agent-medium", 0.75, "verified"),
        ("agent-low", 0.30, "unverified"),
    ];

    for (agent, quality, label_value) in &agents_data {
        let label_template = EventTemplate {
            kind: 1985,
            tags: vec![
                vec!["L".to_string(), "gitafter.reputation".to_string()],
                vec![
                    "l".to_string(),
                    label_value.to_string(),
                    "gitafter.reputation".to_string(),
                ],
                vec!["p".to_string(), agent.to_string()],
                vec!["quality".to_string(), format!("{:.2}", quality)],
            ],
            content: format!("Reputation for {}", agent),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        app.publish_event(label_template).await?;
    }

    // Fetch all reputation labels
    let all_labels = app.relay.get_events_by_kind(1985).await;
    assert_eq!(all_labels.len(), 3);

    // Filter by quality threshold (>= 0.70)
    let high_quality: Vec<_> = all_labels
        .iter()
        .filter(|label| {
            label
                .tags
                .iter()
                .find(|t| t.first().map(|s| s.as_str() == "quality").unwrap_or(false))
                .and_then(|t| t.get(1))
                .and_then(|v| v.parse::<f64>().ok())
                .map(|q| q >= 0.70)
                .unwrap_or(false)
        })
        .collect();

    // Should only have agents with quality >= 0.70 (agent-high and agent-medium)
    assert_eq!(high_quality.len(), 2);

    // Filter by quality threshold (>= 0.85)
    let very_high_quality: Vec<_> = all_labels
        .iter()
        .filter(|label| {
            label
                .tags
                .iter()
                .find(|t| t.first().map(|s| s.as_str() == "quality").unwrap_or(false))
                .and_then(|t| t.get(1))
                .and_then(|v| v.parse::<f64>().ok())
                .map(|q| q >= 0.85)
                .unwrap_or(false)
        })
        .collect();

    // Should only have agent-high
    assert_eq!(very_high_quality.len(), 1);

    Ok(())
}
