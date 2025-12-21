# CI/CD Integration for Benchmarks

This guide explains how to integrate autopilot benchmarks into your CI/CD pipeline to detect performance regressions.

## GitHub Actions

### Basic Benchmark Run

```yaml
name: Autopilot Benchmarks

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run Benchmarks
        run: |
          cargo autopilot benchmark --db benchmarks.db

      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: benchmarks.db
```

### Regression Detection

```yaml
name: Benchmark Regression Check

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0  # Need full history

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Checkout main branch
        run: git checkout main

      - name: Run baseline benchmarks
        run: |
          cargo autopilot benchmark --save-baseline main

      - name: Checkout PR branch
        run: git checkout ${{ github.head_ref }}

      - name: Run PR benchmarks
        run: |
          cargo autopilot benchmark --baseline main > benchmark-report.txt

      - name: Comment on PR
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('benchmark-report.txt', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: '## Benchmark Results\n\n```\n' + report + '\n```'
            });
```

### Nightly Benchmark Suite

```yaml
name: Nightly Benchmarks

on:
  schedule:
    - cron: '0 0 * * *'  # Run at midnight every day

jobs:
  full-benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run all benchmarks
        run: |
          cargo autopilot benchmark \
            --save-baseline nightly-$(date +%Y%m%d) \
            --db benchmarks-nightly.db

      - name: Upload to S3
        run: |
          aws s3 cp benchmarks-nightly.db \
            s3://my-bucket/benchmarks/$(date +%Y%m%d)/
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
```

## GitLab CI

```yaml
benchmark:
  stage: test
  script:
    - cargo autopilot benchmark --db benchmarks.db
  artifacts:
    paths:
      - benchmarks.db
    expire_in: 30 days
  only:
    - main
    - merge_requests

benchmark:regression:
  stage: test
  script:
    - git checkout $CI_MERGE_REQUEST_TARGET_BRANCH_NAME
    - cargo autopilot benchmark --save-baseline main
    - git checkout $CI_COMMIT_REF_NAME
    - cargo autopilot benchmark --baseline main | tee report.txt
  artifacts:
    reports:
      junit: report.txt
  only:
    - merge_requests
```

## Jenkins

```groovy
pipeline {
    agent any

    stages {
        stage('Benchmark') {
            steps {
                sh 'cargo autopilot benchmark --db benchmarks.db'
            }
        }

        stage('Compare to Baseline') {
            when {
                branch 'main'
            }
            steps {
                sh '''
                    cargo autopilot benchmark --baseline production
                    cargo autopilot benchmark --save-baseline main
                '''
            }
        }
    }

    post {
        always {
            archiveArtifacts artifacts: 'benchmarks.db', fingerprint: true
        }
    }
}
```

## Custom Regression Thresholds

Create a script to enforce thresholds:

```bash
#!/bin/bash
# check-benchmark-regression.sh

set -e

# Run benchmarks against baseline
cargo autopilot benchmark --baseline main > results.txt

# Parse results for regressions
if grep -q "REGRESSION" results.txt; then
    echo "❌ Performance regression detected:"
    grep "REGRESSION" results.txt

    # Allow small regressions
    MAX_DURATION_INCREASE=10  # 10% slower allowed
    MAX_TOKEN_INCREASE=5      # 5% more tokens allowed

    # Extract actual regression percentages
    duration_regression=$(grep "duration" results.txt | awk '{print $3}' | tr -d '%')
    token_regression=$(grep "tokens" results.txt | awk '{print $3}' | tr -d '%')

    if (( $(echo "$duration_regression > $MAX_DURATION_INCREASE" | bc -l) )); then
        echo "❌ Duration regression ($duration_regression%) exceeds threshold ($MAX_DURATION_INCREASE%)"
        exit 1
    fi

    if (( $(echo "$token_regression > $MAX_TOKEN_INCREASE" | bc -l) )); then
        echo "❌ Token usage regression ($token_regression%) exceeds threshold ($MAX_TOKEN_INCREASE%)"
        exit 1
    fi

    echo "⚠️  Regressions within acceptable thresholds"
else
    echo "✅ No performance regressions detected"
fi
```

Use in CI:

```yaml
- name: Check for regressions
  run: ./scripts/check-benchmark-regression.sh
```

## Benchmark Result Storage

### SQLite in Git

For small teams, commit benchmark database:

```bash
# .gitignore
# Don't ignore benchmarks.db
!benchmarks.db
```

Pros:
- Simple, no external dependencies
- Full history in git

Cons:
- Binary file in git (not ideal)
- Merge conflicts possible

### External Storage

Store results externally and track references:

```yaml
- name: Upload to artifact storage
  run: |
    VERSION=$(git rev-parse HEAD)
    aws s3 cp benchmarks.db s3://bucket/benchmarks/$VERSION.db
    echo $VERSION > .benchmark-version
    git add .benchmark-version
```

## Monitoring and Alerts

### Slack Notifications

```yaml
- name: Notify Slack on regression
  if: failure()
  uses: 8398a7/action-slack@v3
  with:
    status: ${{ job.status }}
    text: 'Benchmark regression detected in ${{ github.ref }}'
    webhook_url: ${{ secrets.SLACK_WEBHOOK }}
```

### Grafana Dashboard

Export metrics to Prometheus/Grafana:

```bash
# parse-benchmarks-to-prometheus.sh
#!/bin/bash

sqlite3 benchmarks.db <<SQL | curl --data-binary @- http://pushgateway:9091/metrics/job/benchmarks
SELECT
  'benchmark_duration_ms{id="' || benchmark_id || '"} ' ||
  (metrics->>'duration_ms')
FROM benchmark_results
WHERE timestamp > datetime('now', '-1 day');
SQL
```

## Best Practices

1. **Run on every PR**: Catch regressions early
2. **Compare to main branch**: Use main as baseline, not previous commit
3. **Allow variance**: AI responses vary; use statistical significance
4. **Archive results**: Keep historical data for trend analysis
5. **Set thresholds**: Define acceptable regression levels
6. **Monitor costs**: Track token usage and API costs
7. **Categorize failures**: Distinguish flakes from real regressions

## Example: Complete GitHub Action

```yaml
name: Autopilot Quality Gate

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write

    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0

      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Cache baseline
        uses: actions/cache@v3
        with:
          path: baseline.db
          key: benchmark-baseline-${{ github.base_ref }}

      - name: Build baseline
        run: |
          git checkout ${{ github.base_ref }}
          cargo build --release
          cargo autopilot benchmark --db baseline.db

      - name: Build PR
        run: |
          git checkout ${{ github.head_ref }}
          cargo build --release

      - name: Run benchmarks
        run: |
          cargo autopilot benchmark --db pr.db --baseline-db baseline.db > report.md

      - name: Post results
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('report.md', 'utf8');

            await github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: report
            });

      - name: Check thresholds
        run: ./scripts/check-benchmark-regression.sh
```

This will automatically run benchmarks on every PR and post results as a comment.
