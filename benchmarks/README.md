# Autopilot Benchmark Baselines

This directory stores baseline benchmark results for autopilot performance regression testing.

## Structure

```
benchmarks/
├── README.md                    # This file
├── baselines/                   # Baseline results by git commit
│   ├── main-{short-sha}.db     # Baseline from main branch
│   └── ...
├── ci-results.db               # Latest CI run results
└── main-results.db             # Latest main branch results
```

## Usage

### Running Benchmarks Locally

```bash
# Run all benchmarks
cargo autopilot benchmark

# Run specific benchmark
cargo autopilot benchmark B-001

# Run benchmark category
cargo autopilot benchmark --category file-ops

# Save results as baseline
cargo autopilot benchmark --save-baseline my-baseline

# Compare against baseline
cargo autopilot benchmark --baseline my-baseline
```

### Comparing Commits

```bash
# Compare two git commits
cargo autopilot benchmark --compare-commits abc123..def456
```

## CI Integration

The GitHub Actions workflow (`.github/workflows/benchmarks.yml`) automatically:

1. **On Pull Requests**:
   - Triggers on changes to: `crates/**`, `Cargo.toml`, `Cargo.lock`, workflow file
   - Runs full benchmark suite on PR code
   - Runs benchmark suite on main branch
   - Compares results
   - ⚠️ Fails CI if any benchmark regresses >10%
   - 📬 Sends Slack notification if regression detected (requires `SLACK_WEBHOOK_URL` secret)

2. **On Main Branch Push**:
   - Triggers on changes to: `crates/**`, `Cargo.toml`, `Cargo.lock`
   - Runs benchmark suite
   - Stores results as new baseline in `baselines/main-{sha}.db`

3. **Nightly Scheduled Runs**:
   - Runs daily at 2 AM UTC
   - Tracks performance drift over time
   - Establishes trend baselines
   - Detects regressions from dependency updates or environmental changes

### Workflow Trigger Scope

The workflow runs on changes to ANY crate, not just `crates/autopilot/`. This is intentional because:
- Changes to `crates/nostr/**` affect event handling performance
- Changes to `crates/wallet/**` affect payment integration speed
- Changes to `crates/issues/**` affect database query performance
- Dependency updates in `Cargo.toml` can impact overall performance

This broad trigger scope ensures performance regressions are caught regardless of where they originate.

### Setting Up Regression Alerts

To receive Slack notifications when benchmarks regress:

1. Create a Slack incoming webhook: https://api.slack.com/messaging/webhooks
2. Add the webhook URL as a GitHub secret:
   - Go to repository Settings → Secrets and variables → Actions
   - Create new secret: `SLACK_WEBHOOK_URL`
   - Paste your webhook URL
3. Notifications will automatically be sent when regressions are detected

The notification includes:
- PR number
- Link to workflow run
- Commit SHA
- Details of which benchmarks regressed

## Benchmark Suite

Current benchmarks (see `crates/autopilot/src/benchmark/tasks.rs`):

| ID | Category | Description |
|----|----------|-------------|
| B-001 | file-ops | Create a simple text file |
| B-002 | file-ops | Edit existing file |
| B-003 | file-ops | Rename file |
| B-004 | file-ops | Delete file |
| B-005 | file-ops | Create directory structure |
| B-006 | git | Initialize git repository |
| B-007 | git | Create branch and commit |
| B-008 | git | Merge branches |
| B-009 | testing | Write unit test |
| B-010 | testing | Fix failing test |
| B-011 | refactor | Extract function |
| B-012 | refactor | Rename variable |
| B-013 | debug | Fix syntax error |
| B-014 | debug | Fix logic error |
| B-015 | integration | Multi-step workflow |

## Metrics Tracked

For each benchmark run:
- **success**: Whether validation passed
- **duration_ms**: Total execution time
- **tokens_in**: Input tokens used
- **tokens_out**: Output tokens generated
- **tokens_cached**: Cached tokens used
- **cost_usd**: Estimated cost
- **tool_calls**: Number of tool invocations
- **tool_errors**: Number of tool errors
- **apm**: Actions per minute (velocity metric)

## Regression Criteria

A benchmark is considered **regressed** if:
- Success rate drops below baseline
- Duration increases >10%
- Token usage increases >10%
- Cost increases >10%

## Baselines

Baselines are stored as SQLite databases with schema:
- `benchmark_runs` - Individual run results
- `benchmark_details` - Custom metrics per run
- `benchmark_messages` - Validation messages
- `benchmark_baselines` - Aggregate baseline statistics

## Notes

- Baselines are git-ignored except for the `baselines/` directory
- Each main branch commit gets a baseline snapshot
- PR baselines are ephemeral (artifacts only)
- Historical baselines enable tracking performance over time

## Notifications

Autopilot can send notifications when benchmarks regress or other critical events occur.

### Configuration

Copy the example config:
```bash
cp benchmarks/notifications.toml.example ~/.openagents/notifications.toml
```

Edit `~/.openagents/notifications.toml` to add:
- Slack webhook URLs
- Discord webhook URLs
- Email addresses (with SMTP config)
- Custom webhook endpoints

### Supported Services

**Slack:**
1. Create incoming webhook at https://api.slack.com/messaging/webhooks
2. Add URL to `webhook` array in config

**Discord:**
1. Server Settings → Integrations → Webhooks
2. Copy webhook URL
3. Add to `webhook` array in config

**Email:**
1. Configure SMTP settings in `[smtp]` section
2. Add recipient addresses to `email` array
3. Use app-specific password for Gmail/Google Workspace

**Custom:**
- Receives POST with JSON payload
- See `notifications.toml.example` for schema

### Notification Events

Notifications are sent for:
- Benchmark regression detected (>10% by default)
- Metric anomaly (>2 std dev from baseline)
- Autopilot daemon crash
- CI test failures

### Rate Limiting

Default: 10 notifications per hour to prevent spam.

Adjust in config:
```toml
rate_limit_per_hour = 20
```

## Refs

- Directive: d-004 (Continual Constant Improvement of Autopilot)
- Issue: #753 (Implement benchmark suite CI integration)
- Issue: #755 (Alert notification delivery)
- Phase: d-004 Phase 6 (Dashboard & Visibility) and Phase 7 (Benchmark Suite)
