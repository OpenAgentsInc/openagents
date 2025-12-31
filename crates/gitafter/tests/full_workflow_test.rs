//! Comprehensive integration test for full GitAfter workflow
//!
//! Tests the complete flow from repository announcement through issue resolution
//! to PR merge and bounty claim, verifying all Nostr events and their relationships.

mod helpers;

use helpers::TestApp;

#[tokio::test]
async fn test_full_gitafter_workflow() {
    let app = TestApp::new().await.unwrap();

    // Step 1: Announce repository (kind:30617)
    let repo = app
        .create_repository("openagents", "OpenAgents", "Agentic OS desktop foundation")
        .await
        .unwrap();

    assert_eq!(repo.kind, 30617);
    let repo_tag = format!("30617:{}:openagents", app.pubkey());

    // Step 2: Publish issue (kind:1621)
    let issue = app
        .create_issue(
            "openagents",
            "Add dark mode support",
            "Implement dark theme toggle in settings",
        )
        .await
        .unwrap();

    assert_eq!(issue.kind, 1621);
    assert_eq!(issue.content, "Implement dark theme toggle in settings");

    // Verify issue links to repository
    let issue_repo_tag = issue
        .tags
        .iter()
        .find(|t| t[0] == "a")
        .expect("issue should have 'a' tag");
    assert_eq!(issue_repo_tag[1], repo_tag);

    // Step 3: Attach bounty (kind:1636)
    let bounty = app.create_bounty(&issue.id, 50000).await.unwrap();

    assert_eq!(bounty.kind, 1636);

    // Verify bounty links to issue
    let bounty_issue_tag = bounty
        .tags
        .iter()
        .find(|t| t[0] == "e" && t.len() > 3 && t[3] == "root")
        .expect("bounty should have root 'e' tag");
    assert_eq!(bounty_issue_tag[1], issue.id);

    // Verify bounty amount
    let amount_tag = bounty
        .tags
        .iter()
        .find(|t| t[0] == "amount")
        .expect("bounty should have amount tag");
    assert_eq!(amount_tag[1], "50000");

    // Step 4: Claim issue (kind:1634)
    let claim = app.claim_issue(&issue.id).await.unwrap();

    assert_eq!(claim.kind, 1634);

    // Verify claim links to issue
    let claim_issue_tag = claim
        .tags
        .iter()
        .find(|t| t[0] == "e" && t.len() > 3 && t[3] == "root")
        .expect("claim should have root 'e' tag");
    assert_eq!(claim_issue_tag[1], issue.id);

    // Step 5: Create and publish PR (kind:1618) with trajectory link
    let trajectory_session_id = "session-abc123";
    let pr = app
        .create_pr(
            "openagents",
            "Add dark mode toggle",
            "commit-hash-123",
            "https://github.com/example/openagents.git",
            Some(trajectory_session_id),
        )
        .await
        .unwrap();

    assert_eq!(pr.kind, 1618);

    // Verify PR links to repository
    let pr_repo_tag = pr
        .tags
        .iter()
        .find(|t| t[0] == "a")
        .expect("PR should have 'a' tag");
    assert_eq!(pr_repo_tag[1], repo_tag);

    // Verify PR has commit and clone tags
    let commit_tag = pr
        .tags
        .iter()
        .find(|t| t[0] == "c")
        .expect("PR should have commit tag");
    assert_eq!(commit_tag[1], "commit-hash-123");

    let clone_tag = pr
        .tags
        .iter()
        .find(|t| t[0] == "clone")
        .expect("PR should have clone tag");
    assert_eq!(clone_tag[1], "https://github.com/example/openagents.git");

    // Verify trajectory link
    let trajectory_tag = pr
        .tags
        .iter()
        .find(|t| t[0] == "trajectory")
        .expect("PR should have trajectory tag");
    assert_eq!(trajectory_tag[1], trajectory_session_id);

    // Step 6: Post review comment (NIP-22 reply, kind:1)
    let review = app
        .comment_on_issue(&pr.id, "Looks good to me! LGTM 👍")
        .await
        .unwrap();

    assert_eq!(review.kind, 1);

    // Verify review links to PR
    let review_pr_tag = review
        .tags
        .iter()
        .find(|t| t[0] == "e" && t.len() > 3 && t[3] == "root")
        .expect("review should link to PR");
    assert_eq!(review_pr_tag[1], pr.id);

    // Step 7: Merge PR and update status (kind:1631)
    let merge_status = app.merge_pr(&pr.id).await.unwrap();

    assert_eq!(merge_status.kind, 1631); // APPLIED/MERGED status

    // Verify merge status links to PR
    let status_pr_tag = merge_status
        .tags
        .iter()
        .find(|t| t[0] == "e")
        .expect("merge status should have 'e' tag");
    assert_eq!(status_pr_tag[1], pr.id);

    // Step 8: Claim bounty (kind:1637)
    let bounty_claim = app
        .claim_bounty(&bounty.id, &pr.id, trajectory_session_id)
        .await
        .unwrap();

    assert_eq!(bounty_claim.kind, 1637);

    // Verify bounty claim links to bounty and PR
    let bc_bounty_tag = bounty_claim
        .tags
        .iter()
        .find(|t| t[0] == "e" && t.len() > 3 && t[3] == "root")
        .expect("bounty claim should link to bounty");
    assert_eq!(bc_bounty_tag[1], bounty.id);

    let bc_pr_tag = bounty_claim
        .tags
        .iter()
        .find(|t| t[0] == "e" && t.len() > 3 && t[3] == "mention")
        .expect("bounty claim should mention PR");
    assert_eq!(bc_pr_tag[1], pr.id);

    // Verify trajectory hash in bounty claim
    let bc_trajectory_tag = bounty_claim
        .tags
        .iter()
        .find(|t| t[0] == "trajectory")
        .expect("bounty claim should have trajectory tag");
    assert_eq!(bc_trajectory_tag[1], trajectory_session_id);

    // Step 9: Release payment via NIP-57 zap (kind:9735)
    let bounty_amount_msats = 50000 * 1000; // Convert sats to millisats
    let agent_pubkey = app.pubkey();
    let payment = app
        .pay_bounty(&bounty_claim.id, &agent_pubkey, bounty_amount_msats)
        .await
        .unwrap();

    assert_eq!(payment.kind, 9735); // ZAP_RECEIPT

    // Verify payment links to bounty claim
    let payment_claim_tag = payment
        .tags
        .iter()
        .find(|t| t[0] == "e")
        .expect("payment should link to bounty claim");
    assert_eq!(payment_claim_tag[1], bounty_claim.id);

    // Verify payment amount
    let payment_amount_tag = payment
        .tags
        .iter()
        .find(|t| t[0] == "amount")
        .expect("payment should have amount tag");
    assert_eq!(payment_amount_tag[1], bounty_amount_msats.to_string());

    // Verify payment recipient
    let payment_recipient_tag = payment
        .tags
        .iter()
        .find(|t| t[0] == "p")
        .expect("payment should have recipient tag");
    assert_eq!(payment_recipient_tag[1], agent_pubkey);

    // Step 10: Verify all events published to relay
    let all_events = app.get_all_events().await;
    assert_eq!(all_events.len(), 9); // repo, issue, bounty, claim, PR, review, merge, bounty_claim, payment

    // Step 11: Verify event relationships via tags
    let repos = app.get_events_by_kind(30617).await;
    assert_eq!(repos.len(), 1);

    let issues = app.get_events_by_kind(1621).await;
    assert_eq!(issues.len(), 1);

    let bounties = app.get_events_by_kind(1636).await;
    assert_eq!(bounties.len(), 1);

    let claims = app.get_events_by_kind(1634).await;
    assert_eq!(claims.len(), 1);

    let prs = app.get_events_by_kind(1618).await;
    assert_eq!(prs.len(), 1);

    let comments = app.get_events_by_kind(1).await;
    assert_eq!(comments.len(), 1);

    let statuses = app.get_events_by_kind(1631).await;
    assert_eq!(statuses.len(), 1);

    let bounty_claims = app.get_events_by_kind(1637).await;
    assert_eq!(bounty_claims.len(), 1);

    let payments = app.get_events_by_kind(9735).await;
    assert_eq!(payments.len(), 1);

    app.shutdown().await;
}

#[tokio::test]
async fn test_workflow_error_cases() {
    let app = TestApp::new().await.unwrap();

    // Create repository and issue
    let _repo = app
        .create_repository("test", "Test", "Test repo")
        .await
        .unwrap();
    let issue = app
        .create_issue("test", "Bug fix", "Fix the bug")
        .await
        .unwrap();

    // Test duplicate claims - wait 1 second to ensure different timestamps
    // (Nostr event IDs are based on created_at in seconds, so events created
    // in the same second with identical content will have the same ID)
    let claim1 = app.claim_issue(&issue.id).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let claim2 = app.claim_issue(&issue.id).await.unwrap();

    // Both claims should succeed (enforcement is policy, not protocol)
    assert_eq!(claim1.kind, 1634);
    assert_eq!(claim2.kind, 1634);

    let claims = app.get_events_by_kind(1634).await;
    assert_eq!(claims.len(), 2);

    // Test PR without trajectory (trajectory is optional)
    let pr_no_trajectory = app
        .create_pr(
            "test",
            "Fix without trajectory",
            "commit-xyz",
            "https://example.com/repo.git",
            None,
        )
        .await
        .unwrap();

    assert_eq!(pr_no_trajectory.kind, 1618);

    // Verify no trajectory tag
    let has_trajectory = pr_no_trajectory.tags.iter().any(|t| t[0] == "trajectory");
    assert!(!has_trajectory);

    app.shutdown().await;
}
