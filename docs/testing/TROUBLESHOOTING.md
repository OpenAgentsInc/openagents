# Troubleshooting Tests

Common test failures and how to fix them.

## Test Failures

### "File has not been read" in Edit tool tests

**Error:**
```
Error: File has not been read yet
```

**Cause:** The Edit tool requires files to be read before editing (safety check).

**Fix:**
```rust
// Bad
app.edit_file("path.rs", "old", "new").await?;

// Good
app.read_file("path.rs").await?;
app.edit_file("path.rs", "old", "new").await?;
```

### Snapshot mismatches

**Error:**
```
Snapshot mismatch for 'test_name'
```

**Fix:**
```bash
# Review changes
cargo insta review

# Accept if intentional
cargo insta accept

# Reject if not
cargo insta reject
```

### Database locked errors

**Error:**
```
Error: database is locked
```

**Cause:** Multiple tests accessing same database file.

**Fix:**
```rust
// Use in-memory database for tests
let db = MetricsDb::open_in_memory().unwrap();

// Or use unique file per test
let db_path = format!("test-{}.db", uuid::Uuid::new_v4());
let db = MetricsDb::open(&db_path).unwrap();
```

### WebSocket connection refused

**Error:**
```
Error: Connection refused (os error 111)
```

**Cause:** Server not started or wrong port.

**Fix:**
```rust
// Wait for server to start
tokio::time::sleep(Duration::from_millis(100)).await;

// Or use TestApp which handles this
let app = TestApp::new().await?;
```

### Async test hangs forever

**Cause:** Deadlock, infinite loop, or missing `.await`.

**Fix:**
```rust
// Add timeout
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_with_timeout() {
    let result = timeout(
        Duration::from_secs(5),
        my_async_function()
    ).await;

    assert!(result.is_ok(), "Test timed out");
}
```

### Property test failures

**Error:**
```
quickcheck: Test failed. Falsified after 42 tests.
```

**Fix:**
```rust
// Add debug output to see failing case
quickcheck! {
    fn prop_test(input: String) -> bool {
        eprintln!("Testing with: {:?}", input);
        // ... test logic
    }
}

// Reduce test space if too broad
fn arbitrary_bounded_string(g: &mut Gen) -> String {
    let len = usize::arbitrary(g) % 100; // Limit length
    // ...
}
```

## Performance Issues

### Tests are slow

**Solutions:**
1. Run unit tests only: `cargo test --lib`
2. Use parallel execution: `cargo test -- --test-threads=8`
3. Mock expensive operations
4. Use `#[ignore]` for slow tests

### Out of memory in tests

**Solutions:**
1. Use pagination in data generation
2. Clear caches between tests
3. Limit data size in fixtures
4. Run tests sequentially: `--test-threads=1`

## Coverage Issues

### Coverage below threshold

**Check:**
```bash
# See which files lack coverage
cargo llvm-cov --workspace --html
open target/llvm-cov/html/index.html
```

**Fix:**
- Add unit tests for uncovered functions
- Add integration tests for uncovered paths
- Remove dead code

### Coverage not updating

**Solutions:**
```bash
# Clean and regenerate
cargo clean
cargo llvm-cov --workspace

# Check for excluded files
cat .cargo/llvm-cov.toml
```

## CI/CD Issues

### Tests pass locally but fail in CI

**Common causes:**
1. **Race conditions** - Add proper synchronization
2. **Environment differences** - Check OS-specific code
3. **Resource limits** - Reduce parallel tests
4. **Time zone differences** - Use UTC explicitly

**Debug:**
```bash
# Reproduce CI environment locally
docker run -v $(pwd):/workspace rust:latest bash
cd /workspace
cargo test --workspace
```

### Flaky tests

**Identify:**
```bash
# Run test multiple times
for i in {1..10}; do cargo test test_name || echo "Failed on run $i"; done
```

**Fix:**
- Add proper cleanup
- Use deterministic randomness
- Add timeouts
- Fix race conditions

## Build Issues

### Compilation errors in tests

**Error:**
```
error[E0425]: cannot find function `test_helper`
```

**Fix:**
- Check imports: `use crate::test_helpers::*;`
- Ensure `#[cfg(test)]` on test modules
- Add `testing` to `dev-dependencies`

### Dependency resolution issues

**Error:**
```
error: failed to select a version for `testing`
```

**Fix:**
```toml
[dev-dependencies]
testing = { path = "../testing" }
# Or
testing = { version = "0.1", path = "../testing" }
```

## Getting Help

1. Check this guide first
2. Search existing tests for similar patterns
3. Run with verbose output: `cargo test -- --nocapture`
4. Check test logs in CI artifacts
5. Ask in team chat with error details
