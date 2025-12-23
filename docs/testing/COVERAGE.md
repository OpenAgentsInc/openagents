# Coverage & CI/CD

This document explains how test coverage tracking and CI/CD integration works in OpenAgents.

## Coverage Tracking

### Installation

```bash
cargo install cargo-llvm-cov
```

### Basic Usage

```bash
# Generate coverage report
cargo llvm-cov --workspace

# Generate HTML report
cargo llvm-cov --workspace --html
open target/llvm-cov/html/index.html

# Generate JSON for analysis
cargo llvm-cov --workspace --json > coverage.json

# Generate LCOV for CI tools
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

### Coverage Requirements

| Category | Minimum | Enforcement |
|----------|---------|-------------|
| Unit tests (`--lib`) | 70% | CI gate |
| Public API | 100% | PR review |
| P0 user stories | 100% | Release gate |
| P1 user stories | 80% | Release gate |

### Coverage Exclusions

Configured in `.cargo/llvm-cov.toml`:

```toml
[llvm-cov]
exclude = [
    "crates/storybook/*",     # Visual explorer
    "*/examples/*",           # Example code
    "*/benches/*",            # Benchmarks
    "src/main.rs",            # CLI entrypoint
]
```

### Per-Crate Coverage

```bash
# Check coverage for specific crate
cargo llvm-cov -p autopilot --html

# Compare coverage between crates
cargo llvm-cov --workspace --json | jq '.data[].files'
```

### Coverage in PRs

The CI workflow automatically:
1. Runs coverage on all tests
2. Generates coverage report
3. Fails if coverage drops below 70%
4. Comments coverage diff on PR

Example output:
```
Coverage: 72.3% → 73.1% (+0.8%)
✅ Coverage increased
```

## CI/CD Integration

### GitHub Actions Workflow

Location: `.github/workflows/test.yml`

```yaml
name: Tests

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Run Unit Tests
        run: cargo test --workspace --lib

      - name: Run Integration Tests
        run: cargo test --workspace --test '*' -- --test-threads=1

      - name: Generate Coverage
        run: |
          cargo install cargo-llvm-cov
          cargo llvm-cov --workspace --lcov --output-path lcov.info

      - name: Check Coverage Threshold
        run: |
          COV=$(cargo llvm-cov --workspace --json | jq '.data[0].totals.lines.percent')
          if (( $(echo "$COV < 70" | bc -l) )); then
            echo "Coverage $COV% below 70% threshold"
            exit 1
          fi

  snapshots:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Check Snapshots
        run: cargo insta test --review=fail
```

### Pre-Commit Hooks

Location: `.githooks/pre-commit`

```bash
#!/bin/bash

# Run fast unit tests
echo "Running unit tests..."
cargo test --workspace --lib -- --test-threads=4 || exit 1

# Check for snapshot changes
if cargo insta test --review=fail 2>&1 | grep -q "pending"; then
    echo "ERROR: Uncommitted snapshot changes. Run 'cargo insta review'"
    exit 1
fi

echo "✅ Pre-commit checks passed"
```

Enable hooks:

```bash
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit
```

### Pre-Push Hooks

Location: `.git/hooks/pre-push`

The pre-push hook runs:
1. `cargo build` - Fail on build errors
2. Fail on build warnings
3. Border radius check (custom validation)
4. Stub pattern detection (d-012 compliance)

```bash
#!/bin/bash

echo "🔍 Running cargo build to check for errors and warnings..."

build_output=$(cargo build 2>&1)
build_exit_code=$?

# Check for errors
if [ $build_exit_code -ne 0 ]; then
  echo "❌❌❌  PUSH REJECTED: BUILD ERRORS DETECTED  ❌❌❌"
  echo "$build_output"
  exit 1
fi

# Check for warnings
if echo "$build_output" | grep -q "warning:"; then
  echo "⚠️⚠️⚠️  PUSH REJECTED: BUILD WARNINGS DETECTED  ⚠️⚠️⚠️"
  echo "$build_output" | grep -A 5 "warning:"
  exit 1
fi

echo "✅ Build check passed"
```

## Test Execution Strategy

### Local Development

```bash
# Fast feedback loop (< 10s)
cargo test --lib

# Before pushing (< 1 min)
cargo test --workspace --lib

# Before PR (< 5 min)
cargo test --workspace
```

### CI Pipeline

1. **Fast Checks** (parallel, ~2 min)
   - Lint (`cargo clippy`)
   - Format (`cargo fmt --check`)
   - Unit tests (`cargo test --lib`)

2. **Integration Tests** (sequential, ~5 min)
   - API integration tests
   - Database tests
   - WebSocket tests

3. **E2E Tests** (parallel, ~10 min)
   - Full workflow tests
   - Cross-crate integration

4. **Coverage** (~3 min)
   - Generate coverage report
   - Check threshold
   - Upload to codecov

### Optimization Tips

**Faster compilation:**
```bash
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache

# Use cranelift backend for faster debug builds
export CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift
```

**Parallel test execution:**
```bash
# Run independent tests in parallel
cargo test --workspace --lib -- --test-threads=8

# Run integration tests sequentially (they share resources)
cargo test --workspace --test '*' -- --test-threads=1
```

**Skip slow tests locally:**
```rust
#[test]
#[ignore] // Skip in `cargo test`, run with `cargo test -- --ignored`
fn slow_integration_test() {
    // ...
}
```

## Continuous Monitoring

### Coverage Trends

Track coverage over time:

```bash
# Generate historical coverage data
git log --oneline | head -20 | while read commit msg; do
    git checkout $commit
    coverage=$(cargo llvm-cov --workspace --json | jq '.data[0].totals.lines.percent')
    echo "$commit,$coverage" >> coverage-history.csv
done
```

### Performance Benchmarks

Run benchmarks on CI:

```yaml
- name: Run Benchmarks
  run: cargo bench --workspace | tee bench-output.txt

- name: Compare with Baseline
  run: |
    # Compare with main branch benchmarks
    # Fail if performance regression > 10%
```

### Test Flakiness Detection

Detect flaky tests by running multiple times:

```bash
# Run tests 10 times to detect flakiness
for i in {1..10}; do
    cargo test --workspace || echo "Failure on run $i"
done
```

## Release Process

### Pre-Release Checklist

```bash
# 1. All tests pass
cargo test --workspace

# 2. Coverage meets requirements
cargo llvm-cov --workspace
# Check >= 70%

# 3. No warnings
cargo build --release

# 4. Benchmarks show no regressions
cargo bench --workspace

# 5. Snapshots up to date
cargo insta test
```

### Version Tagging

```bash
# Tag release with test results
git tag -a v0.1.0 -m "Release v0.1.0

Test Results:
- Unit: 723 passed
- Integration: 145 passed
- E2E: 23 passed
- Coverage: 73.2%
- Benchmarks: No regressions
"

git push origin v0.1.0
```

## Debugging CI Failures

### Local Reproduction

```bash
# Run exact commands from CI
cargo test --workspace --lib
cargo test --workspace --test '*' -- --test-threads=1

# Use same environment as CI
docker run -v $(pwd):/workspace rust:latest
cd /workspace
cargo test --workspace
```

### Common CI Issues

**1. Tests pass locally but fail in CI:**
- Check for race conditions
- Verify test isolation
- Look for environment-specific dependencies

**2. Coverage differs between local and CI:**
- Ensure same exclusions in `.cargo/llvm-cov.toml`
- Check for platform-specific code paths

**3. Snapshots mismatch:**
- Review snapshot diffs: `cargo insta review`
- Update if changes are intentional
- Commit updated snapshots

### CI Logs Analysis

```bash
# Download CI logs
gh run download <run-id>

# Extract failed tests
grep "FAILED" test-output.log

# Find flaky tests
grep -A 5 "thread panicked" test-output.log
```

## Best Practices

1. **Run tests before pushing** - Use pre-commit hooks
2. **Check coverage locally** - Don't wait for CI
3. **Fix warnings immediately** - Don't accumulate technical debt
4. **Review snapshot changes** - Understand what changed
5. **Monitor benchmark trends** - Catch performance regressions early
6. **Keep CI fast** - Optimize slow tests or move to nightly runs
7. **Use parallel execution** - Where tests are independent
8. **Cache dependencies** - Use sccache or CI caching
9. **Fail fast** - Run fastest checks first
10. **Document flaky tests** - Track and fix intermittent failures
