//! Integration test for per-layer bounties in stacked PRs

use gitafter::nostr::cache::EventCache;
use gitafter::nostr::events::{BountyOfferBuilder, PullRequestBuilder};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

fn create_test_pr(
    repo_addr: &str,
    title: &str,
    stack_id: Option<&str>,
    layer: Option<(u32, u32)>,
) -> nostr::Event {
    let mut builder = PullRequestBuilder::new(repo_addr, title, "Test PR description")
        .commit("abc123")
        .clone_url("https://example.com/repo.git");

    if let Some(sid) = stack_id {
        builder = builder.stack(sid);
    }

    if let Some((current, total)) = layer {
        builder = builder.layer(current, total);
    }

    let template = builder.build();

    // Create a mock signed event
    nostr::Event {
        id: format!("pr-{}", title.replace(' ', "-")),
        kind: template.kind,
        pubkey: "test_pubkey".to_string(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        content: template.content,
        tags: template.tags,
        sig: "mock_signature".to_string(),
    }
}

fn create_test_bounty(
    pr_event_id: &str,
    repo_addr: &str,
    amount: u64,
    stack_id: Option<&str>,
    layer: Option<(u32, u32)>,
) -> nostr::Event {
    let mut builder = BountyOfferBuilder::new(pr_event_id, repo_addr, amount);

    if let Some(sid) = stack_id {
        builder = builder.stack(sid);
    }

    if let Some((current, total)) = layer {
        builder = builder.layer(current, total);
    }

    let template = builder.build();

    nostr::Event {
        id: format!("bounty-{}-{}", pr_event_id, amount),
        kind: template.kind,
        pubkey: "test_pubkey".to_string(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        content: template.content,
        tags: template.tags,
        sig: "mock_signature".to_string(),
    }
}

#[test]
fn test_bounty_builder_with_stack_and_layer() {
    let bounty_template = BountyOfferBuilder::new("pr-event-id", "30617:pubkey:repo", 50000)
        .stack("stack-uuid-123")
        .layer(2, 4)
        .build();

    assert_eq!(bounty_template.kind, 1636);

    // Verify stack tag exists
    let has_stack_tag = bounty_template
        .tags
        .iter()
        .any(|tag| tag.len() >= 2 && tag[0] == "stack" && tag[1] == "stack-uuid-123");
    assert!(has_stack_tag, "Bounty should have stack tag");

    // Verify layer tag exists
    let has_layer_tag = bounty_template
        .tags
        .iter()
        .any(|tag| tag.len() >= 3 && tag[0] == "layer" && tag[1] == "2" && tag[2] == "4");
    assert!(has_layer_tag, "Bounty should have layer tag (2 of 4)");
}

#[test]
fn test_per_layer_bounty_attachment() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = EventCache::new(temp_dir.path().join("test.db")).expect("Failed to create cache");

    let stack_id = "stack-abc-123";
    let repo_addr = "30617:pubkey:openagents";

    // Create 3-layer stack
    let pr1 = create_test_pr(repo_addr, "Layer 1", Some(stack_id), Some((1, 3)));
    let pr2 = create_test_pr(repo_addr, "Layer 2", Some(stack_id), Some((2, 3)));
    let pr3 = create_test_pr(repo_addr, "Layer 3", Some(stack_id), Some((3, 3)));

    cache.insert_event(&pr1).expect("Failed to insert PR1");
    cache.insert_event(&pr2).expect("Failed to insert PR2");
    cache.insert_event(&pr3).expect("Failed to insert PR3");

    // Attach different bounties to each layer
    let bounty1 = create_test_bounty(&pr1.id, repo_addr, 10000, Some(stack_id), Some((1, 3)));
    let bounty2 = create_test_bounty(&pr2.id, repo_addr, 25000, Some(stack_id), Some((2, 3)));
    let bounty3 = create_test_bounty(&pr3.id, repo_addr, 50000, Some(stack_id), Some((3, 3)));

    cache
        .insert_event(&bounty1)
        .expect("Failed to insert bounty1");
    cache
        .insert_event(&bounty2)
        .expect("Failed to insert bounty2");
    cache
        .insert_event(&bounty3)
        .expect("Failed to insert bounty3");

    // Query bounties for specific layer
    let layer2_bounties = cache
        .get_bounties_for_layer(stack_id, 2)
        .expect("Failed to get layer 2 bounties");

    assert_eq!(
        layer2_bounties.len(),
        1,
        "Should find exactly 1 bounty for layer 2"
    );
    assert_eq!(layer2_bounties[0].id, bounty2.id);

    // Verify amount from tags
    let amount = layer2_bounties[0]
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "amount")
        .and_then(|tag| tag[1].parse::<u64>().ok())
        .expect("Bounty should have amount tag");

    assert_eq!(amount, 25000, "Layer 2 bounty should be 25000 sats");
}

#[test]
fn test_query_all_stack_bounties() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = EventCache::new(temp_dir.path().join("test.db")).expect("Failed to create cache");

    let stack_id = "stack-xyz-789";
    let repo_addr = "30617:pubkey:repo";

    // Create 2 layers with bounties
    let pr1 = create_test_pr(repo_addr, "Foundation", Some(stack_id), Some((1, 2)));
    let pr2 = create_test_pr(repo_addr, "Feature", Some(stack_id), Some((2, 2)));

    cache.insert_event(&pr1).expect("Failed to insert PR1");
    cache.insert_event(&pr2).expect("Failed to insert PR2");

    let bounty1 = create_test_bounty(&pr1.id, repo_addr, 30000, Some(stack_id), Some((1, 2)));
    let bounty2 = create_test_bounty(&pr2.id, repo_addr, 70000, Some(stack_id), Some((2, 2)));

    cache
        .insert_event(&bounty1)
        .expect("Failed to insert bounty1");
    cache
        .insert_event(&bounty2)
        .expect("Failed to insert bounty2");

    // Query all bounties for the stack
    let stack_bounties = cache
        .get_bounties_for_stack(stack_id)
        .expect("Failed to get stack bounties");

    assert_eq!(
        stack_bounties.len(),
        2,
        "Should find 2 bounties for the stack"
    );

    // Calculate total stack bounty
    let total_bounty: u64 = stack_bounties
        .iter()
        .filter_map(|bounty| {
            bounty
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "amount")
                .and_then(|tag| tag[1].parse::<u64>().ok())
        })
        .sum();

    assert_eq!(
        total_bounty, 100000,
        "Total stack bounty should be 100k sats"
    );
}

#[test]
fn test_partial_stack_bounties() {
    let cache = EventCache::new_in_memory().expect("Failed to create cache");

    let stack_id = "stack-partial-456";
    let repo_addr = "30617:pubkey:repo";

    // Create 4-layer stack but only attach bounties to layers 1 and 3
    let pr1 = create_test_pr(repo_addr, "Layer 1", Some(stack_id), Some((1, 4)));
    let pr2 = create_test_pr(repo_addr, "Layer 2", Some(stack_id), Some((2, 4)));
    let pr3 = create_test_pr(repo_addr, "Layer 3", Some(stack_id), Some((3, 4)));
    let pr4 = create_test_pr(repo_addr, "Layer 4", Some(stack_id), Some((4, 4)));

    cache.insert_event(&pr1).expect("Failed to insert PR1");
    cache.insert_event(&pr2).expect("Failed to insert PR2");
    cache.insert_event(&pr3).expect("Failed to insert PR3");
    cache.insert_event(&pr4).expect("Failed to insert PR4");

    // Only layers 1 and 3 have bounties
    let bounty1 = create_test_bounty(&pr1.id, repo_addr, 10000, Some(stack_id), Some((1, 4)));
    let bounty3 = create_test_bounty(&pr3.id, repo_addr, 40000, Some(stack_id), Some((3, 4)));

    cache
        .insert_event(&bounty1)
        .expect("Failed to insert bounty1");
    cache
        .insert_event(&bounty3)
        .expect("Failed to insert bounty3");

    // Layer 2 should have no bounties
    let layer2_bounties = cache
        .get_bounties_for_layer(stack_id, 2)
        .expect("Failed to query layer 2");
    assert_eq!(layer2_bounties.len(), 0, "Layer 2 should have no bounties");

    // Layer 3 should have bounty
    let layer3_bounties = cache
        .get_bounties_for_layer(stack_id, 3)
        .expect("Failed to query layer 3");
    assert_eq!(layer3_bounties.len(), 1, "Layer 3 should have 1 bounty");

    // Stack should have 2 total bounties
    let stack_bounties = cache
        .get_bounties_for_stack(stack_id)
        .expect("Failed to get stack bounties");
    assert_eq!(
        stack_bounties.len(),
        2,
        "Stack should have 2 bounties total"
    );
}

#[test]
fn test_single_issue_bounty_not_affected() {
    let cache = EventCache::new_in_memory().expect("Failed to create cache");

    let repo_addr = "30617:pubkey:repo";

    // Regular issue bounty (no stack, no layer)
    let issue_bounty = create_test_bounty("issue-123", repo_addr, 100000, None, None);

    cache
        .insert_event(&issue_bounty)
        .expect("Failed to insert issue bounty");

    // Stack queries should not return issue bounties
    let stack_bounties = cache
        .get_bounties_for_stack("any-stack-id")
        .expect("Failed to query stack");
    assert_eq!(
        stack_bounties.len(),
        0,
        "Issue bounties should not appear in stack queries"
    );

    // Issue bounty query should still work
    let issue_bounties = cache
        .get_bounties_for_issue("issue-123")
        .expect("Failed to get issue bounties");
    assert_eq!(issue_bounties.len(), 1, "Should find the issue bounty");
}
