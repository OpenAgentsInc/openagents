# Testing Guide

This document describes the testing infrastructure and conventions for OpenAgents.

See also: [Directive d-013: Comprehensive Testing Framework](../../.openagents/directives/d-013.md)

## Running Tests

```bash
# All tests
cargo test --workspace

# Unit tests only (fast)
cargo test --workspace --lib

# Integration tests (slower, sequential)
cargo test --workspace --test '*' -- --test-threads=1

# Specific crate
cargo test -p autopilot

# With output
cargo test -- --nocapture

# Specific test
cargo test test_issue_lifecycle
```

## Code Coverage

We use `cargo-llvm-cov` to track code coverage. Minimum requirement: **70%**

### Installation

```bash
cargo install cargo-llvm-cov
```

### Generate Coverage Report

```bash
# Generate coverage (HTML + lcov)
cargo llvm-cov --workspace

# View HTML report
open target/llvm-cov/html/index.html

# Check coverage percentage
cargo llvm-cov --workspace --json | jq '.data[0].totals.lines.percent'
```

### Coverage in CI

Coverage is automatically checked in CI. PRs that decrease coverage below 70% will fail.

### Configuration

Coverage exclusions are configured in `.cargo/llvm-cov.toml`:
- Storybook (visual tool)
- Examples
- Benchmarks
- Main entrypoints
- Test files themselves

## Test Structure

### Unit Tests

Located in `crates/<crate>/src/tests/` or inline with `#[cfg(test)] mod tests`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        let result = my_function(42);
        assert_eq!(result, expected_value);
    }
}
```

### Integration Tests

Located in `crates/<crate>/tests/`.

```rust
use testing::TestApp;

#[tokio::test]
async fn test_integration_flow() {
    let app = TestApp::new().await;
    // Test complete workflow
}
```

## Shared Testing Utilities

The `crates/testing` crate provides:

### TestApp

Pattern for isolated integration tests:

```rust
use testing::TestApp;

#[tokio::test]
async fn test_with_test_app() {
    let app = TestApp::new().await;
    let db = app.db();
    // Use isolated database
}
```

### Mocks

Mock implementations for testing:

```rust
use testing::{MockWallet, MockRelayPool};

let wallet = MockWallet::new(1000); // Initial balance
wallet.send_payment("addr".into(), 100).unwrap();
assert_eq!(wallet.get_balance(), 900);

let pool = MockRelayPool::new();
pool.publish("event_id".into());
assert_eq!(pool.get_published_events().len(), 1);
```

### Fixtures

Test data factories:

```rust
use testing::{test_keypair, test_npub, IssueFactory};

let (sk, pk) = test_keypair(); // Deterministic
let npub = test_npub();

let issue = IssueFactory::new()
    .title("Test Issue")
    .priority("high")
    .build();
```

## Testing Best Practices

### 1. Test Naming

```rust
// Unit tests
#[test]
fn test_<function>_<scenario>() { }

// Integration tests
#[tokio::test]
async fn test_<feature>_<flow>() { }
```

### 2. Arrange-Act-Assert

```rust
#[test]
fn test_wallet_send_deducts_balance() {
    // Arrange
    let wallet = MockWallet::new(1000);

    // Act
    wallet.send_payment("addr".into(), 300).unwrap();

    // Assert
    assert_eq!(wallet.get_balance(), 700);
}
```

### 3. Test One Thing

Each test should verify one specific behavior.

### 4. Use Descriptive Assertions

```rust
// Good
assert_eq!(result.status, IssueStatus::Claimed,
    "Issue should be claimed after claim() succeeds");

// Less helpful
assert_eq!(result.status, IssueStatus::Claimed);
```

### 5. Clean Up

Tests should not leave artifacts or affect other tests.

## Coverage Requirements

| Category | Minimum | Enforcement |
|----------|---------|-------------|
| Unit tests | 70% | CI gate |
| Public API | 100% | PR review |
| P0 user stories | 100% | Release gate |

## Troubleshooting

### Tests Hang

Sequential execution may be needed:

```bash
cargo test --test '*' -- --test-threads=1
```

### Flaky Tests

- Avoid time-based assertions
- Use deterministic test data
- Isolate test state

### Coverage Not Updating

```bash
# Clean and rebuild
cargo clean
cargo llvm-cov --workspace
```

## See Also

- [Directive d-013: Comprehensive Testing Framework](../../.openagents/directives/d-013.md)
- [Git Hooks](git-hooks.md) - Pre-commit test checks
- [crates/testing](../../crates/testing/) - Shared test utilities
