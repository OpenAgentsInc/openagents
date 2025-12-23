# Testing Guide

Comprehensive guide to testing patterns, best practices, and strategies for the OpenAgents codebase.

## Table of Contents

1. [Testing Philosophy](#testing-philosophy)
2. [Test Categories](#test-categories)
3. [Writing Good Tests](#writing-good-tests)
4. [Common Patterns](#common-patterns)
5. [Anti-Patterns](#anti-patterns)
6. [Testing Strategies](#testing-strategies)
7. [Real-World Examples](#real-world-examples)

## Testing Philosophy

### Core Principles

**1. Test Behavior, Not Implementation**

Focus on what the code does (observable behavior), not how it does it (internal implementation).

```rust
// ❌ Bad - Testing implementation details
#[test]
fn test_repository_uses_hashmap() {
    let repo = Repository::new();
    assert!(repo.internal_storage.is_hashmap());  // Implementation detail
}

// ✅ Good - Testing behavior
#[test]
fn test_repository_stores_and_retrieves_events() {
    let repo = Repository::new();
    repo.add_event("event-1", event);

    let retrieved = repo.get_event("event-1").unwrap();
    assert_eq!(retrieved.id, "event-1");
}
```

**2. Fast Feedback Loop**

Unit tests should run in milliseconds. Integration tests in seconds.

```rust
// Unit tests - fast, no external dependencies
#[test]
fn test_npub_encoding() {  // < 1ms
    let bytes = [42u8; 32];
    let npub = encode_npub(&bytes);
    assert!(npub.starts_with("npub1"));
}

// Integration tests - slower, test real interactions
#[tokio::test]
async fn test_full_pr_workflow() {  // < 1s with mocks
    let app = TestApp::new().await?;
    // ... test workflow
}
```

**3. Test Isolation**

Each test should be completely independent. No shared state between tests.

```rust
// ❌ Bad - Shared state
static mut COUNTER: i32 = 0;

#[test]
fn test_increment() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);  // Fails if test order changes
}

// ✅ Good - Isolated state
#[test]
fn test_increment() {
    let mut counter = 0;
    counter += 1;
    assert_eq!(counter, 1);
}
```

**4. Arrange-Act-Assert (AAA) Pattern**

Structure tests clearly in three phases:

```rust
#[tokio::test]
async fn test_bounty_claim_workflow() -> Result<()> {
    // ARRANGE - Set up test environment
    let app = TestApp::new().await?;
    let repo = app.create_repository("test", "Test", "Description").await?;
    let issue = app.create_issue("test", "Fix bug", "Description").await?;
    let bounty = app.create_bounty(&issue.id, 50000).await?;

    // ACT - Perform the action being tested
    let claim = app.claim_bounty(&bounty.id).await?;

    // ASSERT - Verify the outcome
    assert_eq!(claim.kind, 1639); // BOUNTY_CLAIM
    assert_eq!(claim.tags.iter().find(|t| t[0] == "e").unwrap()[1], bounty.id);
    Ok(())
}
```

**5. Descriptive Test Names**

Test names should describe what they verify, not how they do it.

```rust
// ❌ Bad - Vague names
#[test]
fn test_function() { }

#[test]
fn test_case_1() { }

// ✅ Good - Descriptive names
#[test]
fn test_npub_encoding_produces_bech32_with_npub_prefix() { }

#[test]
fn test_bounty_claim_fails_when_pr_not_merged() { }

#[test]
fn test_trajectory_hash_verification_rejects_tampered_events() { }
```

## Test Categories

### Unit Tests

**Purpose:** Test individual functions and modules in isolation.

**Location:** `crates/<crate>/src/tests/` or inline with `#[cfg(test)]`

**Characteristics:**
- Fast (< 1ms per test)
- No external dependencies
- No network/filesystem access
- Pure function testing

```rust
// In crates/nostr/src/encoding.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npub_encoding_roundtrip() {
        let original = [42u8; 32];
        let npub = encode_npub(&original);
        let decoded = decode_npub(&npub).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_npub_encoding_has_correct_prefix() {
        let bytes = [0u8; 32];
        let npub = encode_npub(&bytes);
        assert!(npub.starts_with("npub1"));
    }

    #[test]
    fn test_npub_decoding_rejects_invalid_prefix() {
        let result = decode_npub("nsec1invalid");
        assert!(result.is_err());
    }
}
```

### Component Tests

**Purpose:** Test UI components and rendering logic.

**Location:** `crates/ui/tests/` and `crates/agentgit/tests/components/`

**Characteristics:**
- Test HTML structure
- Verify accessibility attributes
- Snapshot testing for complex UIs
- No browser automation

```rust
// In crates/agentgit/tests/components/repos_test.rs

use agentgit::views::repository_card;
use insta::assert_snapshot;

#[test]
fn test_repository_card_renders_all_fields() {
    let html = repository_card("openagents", "OpenAgents Desktop", "Test description")
        .into_string();

    assert!(html.contains("openagents"));
    assert!(html.contains("OpenAgents Desktop"));
    assert!(html.contains("Test description"));
}

#[test]
fn test_repository_card_snapshot() {
    let html = repository_card("test-repo", "Test Repo", "Description")
        .into_string();

    assert_snapshot!(html);
}
```

### Integration Tests

**Purpose:** Test interactions between modules, API endpoints, database operations.

**Location:** `crates/<crate>/tests/`

**Characteristics:**
- Use `TestApp` for isolated environments
- Mock external services (Nostr relays, wallets)
- Test real database operations
- Test API routes and WebSocket connections

```rust
// In crates/agentgit/tests/pr_creation_test.rs

use anyhow::Result;
use helpers::test_app::TestApp;

#[tokio::test]
async fn test_pr_creation_workflow() -> Result<()> {
    // Create isolated test environment
    let app = TestApp::new().await?;

    // Create repository
    let repo = app.create_repository(
        "test-repo",
        "Test Repository",
        "Test description"
    ).await?;

    // Create issue
    let issue = app.create_issue(
        "test-repo",
        "Add feature",
        "Feature description"
    ).await?;

    // Create PR that addresses the issue
    let pr = app.create_pr(
        "test-repo",
        "feature-branch",
        "Add new feature",
        &issue.id
    ).await?;

    // Verify PR structure
    assert_eq!(pr.kind, 1631); // PATCH
    assert!(pr.tags.iter().any(|t| t[0] == "e" && t[1] == issue.id));

    // Verify PR stored in relay
    let prs = app.relay.get_events_by_kind(1631).await;
    assert_eq!(prs.len(), 1);

    app.shutdown().await;
    Ok(())
}
```

### Protocol Tests

**Purpose:** Test Nostr protocol compliance (NIPs).

**Location:** `crates/nostr/tests/integration/`

**Characteristics:**
- Test NIP specifications
- Verify event structure
- Test relay communication
- Signature verification

```rust
// In crates/nostr/tests/integration_tests.rs

#[test]
fn test_event_id_calculation_nip01() {
    // NIP-01: Event ID must be sha256 of serialized event
    let event = create_test_event();
    let expected_id = calculate_event_id(&event);
    assert_eq!(event.id, expected_id);
}

#[tokio::test]
async fn test_repository_announcement_nip34() {
    // NIP-34: Repository announcements use kind 30617
    let app = TestApp::new().await?;
    let repo = app.create_repository("test", "Test", "Description").await?;

    assert_eq!(repo.kind, 30617);
    assert!(repo.tags.iter().any(|t| t[0] == "d")); // Unique identifier
    assert!(repo.tags.iter().any(|t| t[0] == "name"));
    assert!(repo.tags.iter().any(|t| t[0] == "description"));
}
```

### E2E Tests

**Purpose:** Test complete user journeys across multiple crates.

**Location:** `tests/e2e/`

**Characteristics:**
- Test full workflows
- Cross-crate integration
- Real user scenarios
- May use actual services (with caution)

```rust
// In tests/e2e/bounty_payment_e2e.rs

#[tokio::test]
async fn test_complete_bounty_workflow() -> Result<()> {
    let app = TestApp::new().await?;

    // 1. Maintainer creates repository
    let repo = app.create_repository("project", "Project", "Description").await?;

    // 2. Maintainer creates issue
    let issue = app.create_issue("project", "Bug fix needed", "Description").await?;

    // 3. Maintainer attaches bounty
    let bounty = app.create_bounty(&issue.id, 50000).await?;

    // 4. Agent claims issue
    let claim = app.claim_issue(&issue.id).await?;

    // 5. Agent creates PR
    let pr = app.create_pr("project", "fix-branch", "Fix bug", &issue.id).await?;

    // 6. Maintainer merges PR
    app.merge_pr(&pr.id).await?;

    // 7. Agent claims bounty
    let bounty_claim = app.claim_bounty(&bounty.id).await?;

    // 8. Maintainer pays bounty (NIP-57)
    let payment = app.pay_bounty(&bounty_claim.id, 50000).await?;

    // Verify payment
    assert_eq!(payment.kind, 9735); // NIP-57 zap receipt
    assert!(payment.tags.iter().any(|t| t[0] == "amount"));

    app.shutdown().await;
    Ok(())
}
```

## Writing Good Tests

### Test Structure

```rust
#[tokio::test]
async fn test_descriptive_name() -> Result<()> {
    // ARRANGE - Set up test data and environment
    let app = TestApp::new().await?;
    let initial_state = setup_initial_state(&app).await?;

    // ACT - Perform the action being tested
    let result = perform_action(&app, &initial_state).await?;

    // ASSERT - Verify the outcome
    assert_eq!(result.status, "success");
    assert!(result.data.is_some());

    // CLEANUP (if needed)
    app.shutdown().await;

    Ok(())
}
```

### Error Cases

Test both success and failure paths.

```rust
#[tokio::test]
async fn test_bounty_claim_succeeds_when_pr_merged() -> Result<()> {
    let app = TestApp::new().await?;
    let bounty = setup_merged_pr_with_bounty(&app).await?;

    let claim = app.claim_bounty(&bounty.id).await?;

    assert_eq!(claim.kind, 1639);
    Ok(())
}

#[tokio::test]
async fn test_bounty_claim_fails_when_pr_not_merged() -> Result<()> {
    let app = TestApp::new().await?;
    let bounty = setup_open_pr_with_bounty(&app).await?;

    let result = app.claim_bounty(&bounty.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("PR not merged"));
    Ok(())
}

#[tokio::test]
async fn test_bounty_claim_fails_when_bounty_already_claimed() -> Result<()> {
    let app = TestApp::new().await?;
    let bounty = setup_merged_pr_with_bounty(&app).await?;

    // First claim succeeds
    app.claim_bounty(&bounty.id).await?;

    // Second claim fails
    let result = app.claim_bounty(&bounty.id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already claimed"));

    Ok(())
}
```

### Edge Cases

```rust
#[test]
fn test_empty_input() {
    let result = process_events(&[]);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_single_item() {
    let result = process_events(&[event1]);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_maximum_items() {
    let events: Vec<_> = (0..1000).map(|i| create_event(i)).collect();
    let result = process_events(&events);
    assert_eq!(result.len(), 1000);
}

#[test]
fn test_whitespace_only_content() {
    let result = parse_content("   \n\t  ");
    assert!(result.is_err());
}

#[test]
fn test_unicode_content() {
    let result = parse_content("Hello 👋 世界 🦀");
    assert!(result.is_ok());
}
```

### Async Testing

```rust
// Basic async test
#[tokio::test]
async fn test_async_operation() {
    let result = fetch_data().await;
    assert!(result.is_ok());
}

// With timeout
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_operation_completes_quickly() {
    let result = timeout(
        Duration::from_secs(1),
        slow_operation()
    ).await;

    assert!(result.is_ok(), "Operation timed out");
}

// Parallel async operations
#[tokio::test]
async fn test_concurrent_operations() {
    let (result1, result2, result3) = tokio::join!(
        operation1(),
        operation2(),
        operation3(),
    );

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
}
```

## Common Patterns

### Pattern 1: Test Data Builders

Use builder pattern for complex test data.

```rust
// Define a builder
struct TestEventBuilder {
    kind: u16,
    content: String,
    tags: Vec<Vec<String>>,
}

impl TestEventBuilder {
    fn new() -> Self {
        Self {
            kind: 1,
            content: String::new(),
            tags: Vec::new(),
        }
    }

    fn kind(mut self, kind: u16) -> Self {
        self.kind = kind;
        self
    }

    fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    fn tag(mut self, key: &str, value: &str) -> Self {
        self.tags.push(vec![key.to_string(), value.to_string()]);
        self
    }

    fn build(self) -> Event {
        Event {
            kind: self.kind,
            content: self.content,
            tags: self.tags,
            // ... other fields with defaults
        }
    }
}

// Use in tests
#[test]
fn test_issue_event_structure() {
    let issue = TestEventBuilder::new()
        .kind(1621)
        .content("Fix authentication bug")
        .tag("a", "30617:pubkey:repo")
        .tag("status", "open")
        .build();

    assert_eq!(issue.kind, 1621);
}
```

### Pattern 2: Test Fixtures

Reuse common test data.

```rust
// In crates/<crate>/tests/fixtures.rs

pub fn test_repository() -> Repository {
    Repository {
        identifier: "test-repo".to_string(),
        name: "Test Repository".to_string(),
        description: "Test description".to_string(),
    }
}

pub fn test_issue(repo_id: &str) -> Event {
    TestEventBuilder::new()
        .kind(1621)
        .content("Test issue")
        .tag("a", &format!("30617:pubkey:{}", repo_id))
        .build()
}

// Use in tests
#[tokio::test]
async fn test_workflow() {
    let app = TestApp::new().await?;
    let repo = test_repository();
    let issue = test_issue(&repo.identifier);
    // ... test logic
}
```

### Pattern 3: Helper Functions

Extract common test logic.

```rust
// Setup helpers
async fn setup_repo_with_issue(app: &TestApp) -> Result<(Event, Event)> {
    let repo = app.create_repository("test", "Test", "Description").await?;
    let issue = app.create_issue("test", "Fix bug", "Description").await?;
    Ok((repo, issue))
}

async fn setup_merged_pr(app: &TestApp) -> Result<Event> {
    let (_repo, issue) = setup_repo_with_issue(app).await?;
    let pr = app.create_pr("test", "fix-branch", "Fix", &issue.id).await?;
    app.merge_pr(&pr.id).await?;
    Ok(pr)
}

// Assertion helpers
fn assert_has_tag(event: &Event, key: &str, value: &str) {
    let tag = event.tags.iter()
        .find(|t| t.first().map(|k| k == key).unwrap_or(false))
        .expect(&format!("Expected tag '{}' not found", key));

    assert_eq!(tag.get(1), Some(&value.to_string()));
}

// Use in tests
#[tokio::test]
async fn test_pr_has_correct_tags() {
    let app = TestApp::new().await?;
    let pr = setup_merged_pr(&app).await?;

    assert_has_tag(&pr, "status", "merged");
    assert_has_tag(&pr, "a", "30617:pubkey:test");
}
```

### Pattern 4: Property-Based Testing

Test properties that should hold for all inputs.

```rust
use quickcheck::quickcheck;

quickcheck! {
    // Property: Encoding and decoding should be inverse operations
    fn prop_npub_roundtrip(bytes: [u8; 32]) -> bool {
        let npub = encode_npub(&bytes);
        decode_npub(&npub)
            .map(|decoded| decoded == bytes)
            .unwrap_or(false)
    }

    // Property: Event ID should be deterministic
    fn prop_event_id_deterministic(content: String) -> bool {
        let event1 = create_event_with_content(&content);
        let event2 = create_event_with_content(&content);
        event1.id == event2.id
    }

    // Property: Signature should verify for valid events
    fn prop_signature_verifies(content: String) -> bool {
        let event = create_and_sign_event(&content);
        verify_signature(&event)
    }
}
```

### Pattern 5: Snapshot Testing

Test complex structured output.

```rust
use insta::assert_snapshot;

#[test]
fn test_dashboard_html_structure() {
    let dashboard = Dashboard::new()
        .with_repository_count(5)
        .with_issue_count(12)
        .with_pr_count(3)
        .render();

    assert_snapshot!(dashboard.into_string());
}

#[test]
fn test_json_serialization() {
    let event = create_test_event();
    let json = serde_json::to_string_pretty(&event).unwrap();

    assert_snapshot!(json);
}
```

## Anti-Patterns

### Anti-Pattern 1: Testing Implementation Details

```rust
// ❌ Bad - Tests implementation, not behavior
#[test]
fn test_uses_hashmap_internally() {
    let storage = Storage::new();
    assert!(storage.data.is_hashmap()); // Internal detail
}

// ✅ Good - Tests observable behavior
#[test]
fn test_stores_and_retrieves_data() {
    let storage = Storage::new();
    storage.insert("key", "value");
    assert_eq!(storage.get("key"), Some("value"));
}
```

### Anti-Pattern 2: Test Interdependence

```rust
// ❌ Bad - Tests depend on each other
#[tokio::test]
async fn test_step_1_create_repo() {
    let repo = create_repository("test").await;
    GLOBAL_REPO_ID = repo.id;  // Shared state!
}

#[tokio::test]
async fn test_step_2_create_issue() {
    create_issue(GLOBAL_REPO_ID).await;  // Depends on test_step_1
}

// ✅ Good - Each test is independent
#[tokio::test]
async fn test_create_issue() {
    let app = TestApp::new().await?;
    let repo = app.create_repository("test", "Test", "Desc").await?;
    let issue = app.create_issue("test", "Issue", "Desc").await?;
    assert!(issue.id.len() > 0);
}
```

### Anti-Pattern 3: Overly Complex Tests

```rust
// ❌ Bad - Tests too much in one test
#[tokio::test]
async fn test_entire_application() {
    // Creates repo
    // Creates 10 issues
    // Creates 5 PRs
    // Merges some PRs
    // Pays bounties
    // Tests dashboard rendering
    // Tests search functionality
    // ... 200 lines of test code
}

// ✅ Good - Focused tests
#[tokio::test]
async fn test_pr_creation() {
    let app = TestApp::new().await?;
    let pr = app.create_pr("test", "branch", "Title", "issue-id").await?;
    assert_eq!(pr.kind, 1631);
}

#[tokio::test]
async fn test_pr_merge() {
    let app = TestApp::new().await?;
    let pr = setup_pr(&app).await?;
    app.merge_pr(&pr.id).await?;

    let merged_pr = app.get_pr(&pr.id).await?;
    assert_has_tag(&merged_pr, "status", "merged");
}
```

### Anti-Pattern 4: Mocking Everything

```rust
// ❌ Bad - Mocking too much loses integration value
#[tokio::test]
async fn test_bounty_payment() {
    let mock_relay = MockRelay::new();
    let mock_wallet = MockWallet::new();
    let mock_events = MockEventStore::new();
    let mock_db = MockDatabase::new();

    // Test becomes a test of mocks, not real code
}

// ✅ Good - Mock external services, use real internal code
#[tokio::test]
async fn test_bounty_payment() {
    let app = TestApp::new().await?;  // Real relay, real DB (in-memory)
    // Test real workflow with mock external services only
}
```

### Anti-Pattern 5: Ignoring Test Failures

```rust
// ❌ Bad - Commenting out failing tests
// #[test]
// fn test_broken_feature() {
//     // TODO: Fix this test later
// }

// ❌ Bad - Suppressing errors
#[test]
fn test_might_fail() {
    let _ = operation_that_might_fail();  // Ignoring Result
}

// ✅ Good - Fix the test or remove the feature
#[test]
fn test_working_feature() -> Result<()> {
    let result = operation()?;
    assert_eq!(result, expected);
    Ok(())
}
```

## Testing Strategies

### Strategy 1: Test Pyramid

Follow the test pyramid - many unit tests, fewer integration tests, minimal E2E.

```
        /\
       /E2E\      ← Few (5-10) - Full workflows
      /______\
     /        \
    /Integration\  ← Some (50-100) - Module interaction
   /____________\
  /              \
 /   Unit Tests   \  ← Many (500+) - Functions/modules
/__________________\
```

### Strategy 2: Test-Driven Development (TDD)

Write tests before implementation.

```rust
// 1. Write failing test
#[test]
fn test_encode_npub() {
    let bytes = [42u8; 32];
    let npub = encode_npub(&bytes);
    assert!(npub.starts_with("npub1"));
}

// 2. Implement minimal code to pass
pub fn encode_npub(bytes: &[u8; 32]) -> String {
    format!("npub1{}", hex::encode(bytes))
}

// 3. Refactor
pub fn encode_npub(bytes: &[u8; 32]) -> String {
    bech32::encode("npub", bytes).expect("encoding failed")
}
```

### Strategy 3: Coverage-Guided Testing

Use coverage reports to find untested code paths.

```bash
# Generate coverage report
cargo llvm-cov --workspace --html
open target/llvm-cov/html/index.html

# Identify uncovered lines, write tests
```

### Strategy 4: Regression Testing

When fixing bugs, first write a failing test that reproduces the bug.

```rust
// Bug report: "Bounty claim fails when amount is exactly 21000000 sats"

#[tokio::test]
async fn test_bounty_claim_with_max_bitcoin_amount() -> Result<()> {
    let app = TestApp::new().await?;
    let bounty = setup_bounty(&app, 21_000_000).await?;

    // This test initially fails (reproducing the bug)
    let claim = app.claim_bounty(&bounty.id).await?;

    // After fix, test passes
    assert!(claim.id.len() > 0);
    Ok(())
}
```

## Real-World Examples

### Example 1: Testing Nostr Event Creation

```rust
#[tokio::test]
async fn test_repository_announcement_complies_with_nip34() -> Result<()> {
    // ARRANGE
    let app = TestApp::new().await?;

    // ACT
    let repo = app.create_repository(
        "openagents",
        "OpenAgents",
        "Decentralized agent marketplace"
    ).await?;

    // ASSERT - NIP-34 compliance
    assert_eq!(repo.kind, 30617, "Must use kind 30617 for repository announcements");

    // Must have 'd' tag with unique identifier
    let d_tag = repo.tags.iter()
        .find(|t| t.first().map(|k| k == "d").unwrap_or(false))
        .expect("Repository must have 'd' tag");
    assert_eq!(d_tag.get(1), Some(&"openagents".to_string()));

    // Must have 'name' tag
    let name_tag = repo.tags.iter()
        .find(|t| t.first().map(|k| k == "name").unwrap_or(false))
        .expect("Repository must have 'name' tag");
    assert_eq!(name_tag.get(1), Some(&"OpenAgents".to_string()));

    // Must have valid signature
    assert!(verify_event_signature(&repo), "Event signature must be valid");

    app.shutdown().await;
    Ok(())
}
```

### Example 2: Testing WebSocket Real-Time Updates

```rust
#[tokio::test]
async fn test_websocket_broadcasts_pr_updates() -> Result<()> {
    // ARRANGE
    let app = TestApp::new().await?;
    let ws_url = format!("ws://127.0.0.1:{}/ws", app.port);

    // Connect WebSocket client
    let (mut ws_stream, _) = connect_async(&ws_url).await?;

    // Subscribe to PR events
    ws_stream.send(Message::Text(
        r#"{"type":"subscribe","filter":{"kinds":[1631]}}"#.into()
    )).await?;

    // ACT - Create a PR (should trigger WebSocket broadcast)
    let pr = app.create_pr("test", "branch", "Title", "issue-id").await?;

    // ASSERT - Receive PR event via WebSocket
    let msg = tokio::time::timeout(
        Duration::from_secs(1),
        ws_stream.next()
    ).await??;

    if let Some(Ok(Message::Text(text))) = msg {
        let event: Event = serde_json::from_str(&text)?;
        assert_eq!(event.id, pr.id);
        assert_eq!(event.kind, 1631);
    } else {
        panic!("Expected text message with PR event");
    }

    app.shutdown().await;
    Ok(())
}
```

### Example 3: Testing Database Transactions

```rust
#[test]
fn test_session_metrics_stored_atomically() -> Result<()> {
    // ARRANGE
    let db = MetricsDb::open_in_memory()?;

    let session = SessionMetrics {
        session_id: "session-1".to_string(),
        issues_completed: 5,
        duration_seconds: 3600.0,
        success_rate: 0.8,
    };

    // ACT - Store session (should be atomic)
    db.store_session(&session)?;

    // ASSERT - All fields stored correctly
    let retrieved = db.get_session("session-1")?;
    assert_eq!(retrieved.session_id, session.session_id);
    assert_eq!(retrieved.issues_completed, session.issues_completed);
    assert_eq!(retrieved.duration_seconds, session.duration_seconds);
    assert_eq!(retrieved.success_rate, session.success_rate);

    Ok(())
}

#[test]
fn test_transaction_rollback_on_error() -> Result<()> {
    // ARRANGE
    let db = MetricsDb::open_in_memory()?;

    // ACT - Start transaction and intentionally cause error
    let result = db.transaction(|tx| {
        tx.execute("INSERT INTO sessions (session_id) VALUES (?)", params!["session-1"])?;
        tx.execute("INSERT INTO sessions (session_id) VALUES (?)", params!["session-2"])?;

        // Intentional error - duplicate key
        tx.execute("INSERT INTO sessions (session_id) VALUES (?)", params!["session-1"])?;

        Ok(())
    });

    // ASSERT - Transaction rolled back, no sessions stored
    assert!(result.is_err());
    assert_eq!(db.get_all_sessions()?.len(), 0);

    Ok(())
}
```

### Example 4: Testing Error Handling

```rust
#[tokio::test]
async fn test_bounty_claim_error_cases() -> Result<()> {
    let app = TestApp::new().await?;

    // Error case 1: Bounty doesn't exist
    let result = app.claim_bounty("non-existent-id").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Bounty not found"));

    // Error case 2: PR not merged yet
    let bounty = setup_open_pr_with_bounty(&app).await?;
    let result = app.claim_bounty(&bounty.id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("PR not merged"));

    // Error case 3: Bounty already claimed
    let bounty = setup_merged_pr_with_bounty(&app).await?;
    app.claim_bounty(&bounty.id).await?;  // First claim succeeds

    let result = app.claim_bounty(&bounty.id).await;  // Second claim fails
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already claimed"));

    app.shutdown().await;
    Ok(())
}
```

### Example 5: Testing Concurrent Operations

```rust
#[tokio::test]
async fn test_concurrent_pr_creation() -> Result<()> {
    let app = TestApp::new().await?;
    let repo = app.create_repository("test", "Test", "Description").await?;

    // Create 10 PRs concurrently
    let pr_futures: Vec<_> = (0..10)
        .map(|i| {
            let app = &app;
            async move {
                app.create_pr(
                    "test",
                    &format!("branch-{}", i),
                    &format!("PR {}", i),
                    "issue-id"
                ).await
            }
        })
        .collect();

    let results = futures::future::join_all(pr_futures).await;

    // All PRs should be created successfully
    assert_eq!(results.len(), 10);
    assert!(results.iter().all(|r| r.is_ok()));

    // All PRs should have unique IDs
    let ids: Vec<_> = results.iter()
        .filter_map(|r| r.as_ref().ok().map(|pr| &pr.id))
        .collect();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len());

    app.shutdown().await;
    Ok(())
}
```

## Next Steps

- Review [Test Infrastructure](./INFRASTRUCTURE.md) for TestApp and mock details
- Check [Coverage & CI/CD](./COVERAGE.md) for CI integration
- See [Troubleshooting](./TROUBLESHOOTING.md) for common issues
- Study existing tests in `crates/*/tests/` for more examples

## Related Directives

- **d-013**: Comprehensive Testing Framework
- **d-012**: No Stubs - Production-Ready Code Only
