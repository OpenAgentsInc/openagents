# Running Tests

This guide covers how to run tests in the OpenAgents project, interpret results, and debug failures.

## Quick Start

```bash
# Run all tests in the workspace
cargo test --workspace

# Run tests with output (show println! statements)
cargo test --workspace -- --nocapture

# Run tests in parallel (default) or sequentially
cargo test --workspace -- --test-threads=4
cargo test --workspace -- --test-threads=1
```

## Running Tests by Crate

```bash
# Run all tests in a specific crate
cargo test --package autopilot
cargo test --package nostr
cargo test --package marketplace
cargo test --package compute
cargo test --package issues
cargo test --package recorder

# Short form with -p
cargo test -p autopilot
```

## Running Specific Tests

```bash
# Run a specific test file
cargo test --package autopilot --test guardrails

# Run tests matching a pattern
cargo test test_issue_claim

# Run tests in a specific module
cargo test autopilot::timestamp

# Run a single test function
cargo test test_issue_claim_already_claimed_fails

# Run tests with name containing a substring
cargo test claim -- --nocapture
```

## Running Integration vs Unit Tests

```bash
# Run only integration tests (tests/ directory)
cargo test --package autopilot --test '*'

# Run only unit tests (inline #[test] in src/)
cargo test --package autopilot --lib

# Run only doc tests
cargo test --package autopilot --doc
```

## Test Output Options

```bash
# Show output for passing tests
cargo test -- --nocapture

# Show only failures
cargo test -- --quiet

# Show test execution time
cargo test -- --show-output

# Pretty print output
cargo test -- --format=pretty

# JSON output (for CI/tooling)
cargo test -- --format=json
```

## Filtering and Selection

```bash
# Run tests, but skip slow ones
cargo test -- --skip slow

# Run only ignored tests
cargo test -- --ignored

# Run all tests including ignored
cargo test -- --include-ignored

# Run tests matching exact name
cargo test --exact test_name
```

## Debugging Test Failures

### 1. Run with Detailed Output

```bash
# Show all output including passed tests
cargo test test_name -- --nocapture --show-output

# Run sequentially to avoid interleaved output
cargo test test_name -- --test-threads=1 --nocapture
```

### 2. Use RUST_BACKTRACE

```bash
# Show full backtrace on panic
RUST_BACKTRACE=1 cargo test test_name

# Show full backtrace with source
RUST_BACKTRACE=full cargo test test_name
```

### 3. Use RUST_LOG for Logging

```bash
# Enable debug logging
RUST_LOG=debug cargo test test_name -- --nocapture

# Enable trace logging for specific module
RUST_LOG=autopilot::database=trace cargo test -- --nocapture

# Multiple modules
RUST_LOG=autopilot=debug,issues=trace cargo test
```

### 4. Run Single Test

```bash
# Isolate the failing test
cargo test test_failing_name -- --exact --nocapture
```

## Common Test Patterns

### Testing with Temporary Files

```rust
#[test]
fn test_with_temp_dir() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test.db");

    // Test code using path

    // temp_dir automatically cleaned up on drop
}
```

### Async Tests

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### Testing Expected Failures

```rust
#[test]
#[should_panic(expected = "error message")]
fn test_panic() {
    panic!("error message");
}

#[test]
fn test_result() -> Result<(), Box<dyn std::error::Error>> {
    let result = fallible_function()?;
    assert_eq!(result, expected);
    Ok(())
}
```

## Performance Testing

```bash
# Run tests with timing
cargo test -- --show-output | grep -E "(test|elapsed)"

# Use criterion for benchmarks (if configured)
cargo bench

# Count test execution time
time cargo test --package nostr
```

## Continuous Integration

```bash
# Run tests as CI would
cargo test --workspace --all-features --no-fail-fast

# Check for warnings
cargo test --workspace -- -D warnings

# Generate coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace
```

## Test Organization Best Practices

### 1. Use Descriptive Names

```rust
// Good
#[test]
fn test_issue_claim_already_claimed_returns_error() { }

// Avoid
#[test]
fn test1() { }
```

### 2. Group Related Tests

```rust
#[cfg(test)]
mod database_tests {
    use super::*;

    #[test]
    fn test_insert() { }

    #[test]
    fn test_update() { }
}
```

### 3. Use Setup Functions

```rust
#[cfg(test)]
mod tests {
    fn setup() -> TestContext {
        // Common setup
    }

    #[test]
    fn test_something() {
        let ctx = setup();
        // Test code
    }
}
```

## Troubleshooting

### Tests Hang

```bash
# Run with timeout
timeout 60 cargo test

# Run single-threaded to identify culprit
cargo test -- --test-threads=1
```

### Flaky Tests

```bash
# Run test multiple times to check for flakiness
for i in {1..10}; do cargo test test_name || break; done

# Run with different thread counts
cargo test test_name -- --test-threads=1
cargo test test_name -- --test-threads=8
```

### Database Conflicts

```bash
# Use unique database per test
#[test]
fn test_with_db() {
    let db_path = format!("test_{}.db", std::process::id());
    // Use db_path
    std::fs::remove_file(db_path).ok();
}
```

## Test Coverage Script

Use the automated test counting script to see test distribution:

```bash
./scripts/count_tests.sh
```

This generates a comprehensive report of all tests across the codebase.

## Additional Resources

- **Coverage Summary**: See `docs/testing/coverage-summary.md` for test counts by module
- **Writing Tests**: See `docs/testing/coverage-summary.md` for testing best practices
- **Cargo Test Documentation**: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- **Rust Book Testing Chapter**: https://doc.rust-lang.org/book/ch11-00-testing.html
