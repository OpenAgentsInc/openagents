# Testing Infrastructure

Welcome to the OpenAgents testing infrastructure documentation. This guide will help you understand how to write, run, and debug tests across the OpenAgents codebase.

## Quick Start

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run unit tests only (fast)
cargo test --workspace --lib

# Run integration tests (sequential)
cargo test --workspace --test '*' -- --test-threads=1

# Run tests for a specific crate
cargo test -p autopilot
cargo test -p agentgit
cargo test -p nostr-core

# Run a specific test
cargo test -p agentgit --test full_workflow_test

# Run with coverage
cargo install cargo-llvm-cov
cargo llvm-cov --workspace --html
# Open target/llvm-cov/html/index.html
```

### Writing Your First Test

**Unit Test** (in `crates/<crate>/src/lib.rs` or module file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        let result = my_function("input");
        assert_eq!(result, "expected");
    }
}
```

**Integration Test** (in `crates/<crate>/tests/my_test.rs`):

```rust
use agentgit::TestApp;

#[tokio::test]
async fn test_full_workflow() {
    let app = TestApp::new().await.unwrap();

    // Test your feature
    let result = app.create_repository("test", "Test", "Description")
        .await
        .unwrap();

    assert_eq!(result.kind, 30617);
    app.shutdown().await;
}
```

## Documentation Sections

1. **[Testing Guide](./GUIDE.md)** - Comprehensive guide to testing patterns and best practices
2. **[Test Infrastructure](./INFRASTRUCTURE.md)** - Details on TestApp, mocks, and fixtures
3. **[Coverage & CI/CD](./COVERAGE.md)** - How coverage tracking and CI integration works
4. **[Troubleshooting](./TROUBLESHOOTING.md)** - Common issues and how to fix them

## Test Categories

| Category | Description | Location | Runner |
|----------|-------------|----------|--------|
| Unit | Module/function level | `crates/<crate>/src/tests/` | `cargo test --lib` |
| Component | UI components + accessibility | `crates/ui/tests/` | `cargo test -p ui` |
| Integration | API routes, WebSocket, DB | `crates/<crate>/tests/` | `cargo test --test '*'` |
| Protocol | Nostr NIPs, relay comms | `crates/nostr/tests/integration/` | `cargo test -p nostr-core` |
| E2E Flows | Full user journeys | `tests/e2e/` | `cargo test --test e2e` |

## Coverage Requirements

| Category | Minimum | Enforcement |
|----------|---------|-------------|
| Unit tests (--lib) | 70% | CI gate |
| Public API | 100% | PR review |
| P0 user stories | 100% | Release gate |
| P1 user stories | 80% | Release gate |

## Key Principles

1. **Test Behavior, Not Implementation** - Focus on what code does, not how
2. **Fast Feedback** - Unit tests should run in milliseconds
3. **Isolated Tests** - Each test should be independent
4. **Descriptive Names** - Test names should describe what they verify
5. **Arrange-Act-Assert** - Structure tests clearly

## Common Commands

```bash
# Update snapshots
cargo insta test
cargo insta review

# Run property tests with more iterations
QUICKCHECK_TESTS=10000 cargo test --lib

# Run benchmarks
cargo bench -p autopilot

# Check test coverage
cargo llvm-cov --workspace
cargo llvm-cov --workspace --json > coverage.json
```

## Getting Help

- See [GUIDE.md](./GUIDE.md) for detailed testing patterns
- See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) for common issues
- Check existing tests in the codebase for examples
- Ask in the team chat or open an issue

## Related Directives

- **d-013**: Comprehensive Testing Framework (parent directive)
- **d-012**: No Stubs - Production-Ready Code Only (tests verify real implementations)
