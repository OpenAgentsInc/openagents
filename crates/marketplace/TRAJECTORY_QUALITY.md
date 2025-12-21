# Trajectory Quality Scoring System

This document describes how the marketplace trajectory contribution system evaluates and rewards developer trajectory data.

## Overview

The trajectory quality scoring system evaluates contributions based on three key dimensions:
1. **Completeness** - Git commit correlation (initial state → final state)
2. **Complexity** - Token count and tool call diversity
3. **Reward Signal** - CI/CD test and build results

## Quality Score Calculation

Quality scores range from 0.0 to 1.0 and are calculated using a weighted average:

```
quality_score = (completeness × 0.4) + (complexity × 0.3) + (reward_signal × 0.3)
```

### 1. Completeness Score (0-1.0, weight: 40%)

Measures whether the trajectory captures a complete coding session with git commits:

| Component | Points | Description |
|-----------|--------|-------------|
| Initial commit detected | 0.4 | Has git commit at session start |
| Final commit detected | 0.4 | Has git commit at session end |
| Session end timestamp | 0.2 | Clean session termination |

**Git Commit Detection Patterns**:
- `[branch hash] message` - from `git commit` output
- `commit hash` - from `git log` or `git show`
- `hash1..hash2` - from `git push` output
- Hash extraction from lines containing "git" and "commit"

**Example**:
```bash
# Initial commit (git status before changes)
$ git log -1 --oneline
abc123f Previous commit

# ... coding work happens ...

# Final commit (after changes)
$ git commit -m "Add feature X"
[main def456a] Add feature X
```

Completeness score = 0.4 (initial) + 0.4 (final) + 0.2 (ended) = **1.0**

### 2. Complexity Score (0-1.0, weight: 30%)

Measures the amount and diversity of work performed:

#### Token Count (up to 0.5 points)
| Threshold | Points | Reasoning |
|-----------|--------|-----------|
| > 100 tokens | +0.2 | Non-trivial session |
| > 500 tokens | +0.2 | Substantial work |
| > 2000 tokens | +0.1 | Complex task |

#### Tool Calls (up to 0.5 points)
| Threshold | Points | Reasoning |
|-----------|--------|-----------|
| > 3 calls | +0.2 | Basic interaction |
| > 10 calls | +0.2 | Active coding |
| > 20 calls | +0.1 | Extensive work |

**Example**:
- Session with 3000 tokens and 25 tool calls
- Token score: 0.2 + 0.2 + 0.1 = 0.5
- Tool call score: 0.2 + 0.2 + 0.1 = 0.5
- Complexity score = **1.0**

### 3. Reward Signal Score (0-1.0, weight: 30%)

Measures whether the code changes were successful based on CI/CD results:

| Outcome | Score | Description |
|---------|-------|-------------|
| CI/CD passed | 1.0 | Tests green, build succeeded |
| CI/CD failed | 0.3 | Tests failed (still valuable as negative signal) |
| No CI data | 0.0 | No automated verification |

**CI/CD Detection Patterns**:

**Test Detection**:
- Commands: `cargo test`, `npm test`, `pytest`
- Success: "test result: ok", "all tests passed"
- Failure: "test result: failed", "failures:"

**Build Detection**:
- Commands: `cargo build`, `npm build`, `make`
- Success: "Finished dev profile", "Finished release profile"
- Exit codes: `Exit code 0` (success), `Exit code 1` (failure)

**Example**:
```bash
$ cargo test
   Compiling myproject v0.1.0
    Finished test [unoptimized + debuginfo] target(s) in 2.50s
     Running unittests src/lib.rs

running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored
```

Reward signal score = **1.0** (tests passed)

## Example Quality Calculations

### High Quality Trajectory (0.9)

```yaml
initial_commit: "abc123f"
final_commit: "def456a"
ci_passed: true
token_count: 3000
tool_calls: 25
```

**Calculation**:
- Completeness: 0.4 + 0.4 + 0.2 = 1.0
- Complexity: 0.5 (tokens) + 0.5 (tools) = 1.0
- Reward Signal: 1.0 (CI passed)
- **Quality Score**: (1.0 × 0.4) + (1.0 × 0.3) + (1.0 × 0.3) = **1.0**

### Medium Quality Trajectory (0.64)

```yaml
initial_commit: "abc123f"
final_commit: "def456a"
ci_passed: false  # Tests failed
token_count: 800
tool_calls: 12
```

**Calculation**:
- Completeness: 0.4 + 0.4 + 0.2 = 1.0
- Complexity: 0.4 (tokens) + 0.4 (tools) = 0.8
- Reward Signal: 0.3 (CI failed, but still signal)
- **Quality Score**: (1.0 × 0.4) + (0.8 × 0.3) + (0.3 × 0.3) = **0.73**

### Low Quality Trajectory (0.2)

```yaml
initial_commit: null
final_commit: null
ci_passed: null
token_count: 75
tool_calls: 2
```

**Calculation**:
- Completeness: 0.0 (no commits)
- Complexity: 0.0 (below thresholds)
- Reward Signal: 0.0 (no CI data)
- **Quality Score**: **0.0**

## Reward Calculation

Rewards are calculated based on quality score and session characteristics:

```rust
pub struct RewardInfo {
    base_sats: u64,              // 100 sats
    quality_bonus_sats: u64,     // Up to 50 sats
    ci_bonus_sats: u64,          // 200 sats if CI present
    complexity_bonus_sats: u64,  // Tokens + tool calls
    total_sats: u64,
}
```

### Default Reward Rates

| Component | Rate | Description |
|-----------|------|-------------|
| Base reward | 100 sats | Every valid trajectory |
| Quality bonus | 50 sats/point | Per quality point above minimum |
| CI signal bonus | 200 sats | If CI/CD result available |
| Token bonus | 10 sats/1k | Per 1000 tokens |
| Tool call bonus | 5 sats/call | Per tool invocation |

### Example Reward Calculation

For a trajectory with:
- Quality score: 0.8 (minimum threshold: 0.5)
- Token count: 2000
- Tool calls: 15
- CI passed: true

```
base_sats = 100
quality_bonus = (0.8 - 0.5) × 50 = 15 sats
ci_bonus = 200 sats (CI present)
token_bonus = (2000 / 1000) × 10 = 20 sats
tool_bonus = 15 × 5 = 75 sats
complexity_bonus = 20 + 75 = 95 sats

total_reward = 100 + 15 + 200 + 95 = 410 sats
```

## Quality Thresholds

Configurable minimum quality thresholds determine which trajectories are accepted:

| Threshold | Use Case |
|-----------|----------|
| 0.3 | Accept most trajectories (inclusive) |
| 0.5 | Standard quality bar (default) |
| 0.7 | High quality only |
| 0.9 | Exceptional trajectories only |

## Configuration

The quality scoring system is configurable via `~/.openagents/marketplace.toml`:

```toml
[trajectories]
# Minimum quality score to accept (0.0 - 1.0)
min_quality_score = 0.5

# Require CI/CD signal to be present
require_ci_signal = false

# Redaction level: standard, strict, paranoid
redaction_level = "standard"

# Reward settings
min_reward_sats = 10  # Don't contribute if reward < 10 sats
```

## Implementation Details

### File Locations

- **Quality Scoring**: `crates/marketplace/src/trajectories/validate.rs`
- **Reward Calculation**: `crates/marketplace/src/trajectories/rewards.rs`
- **Git Detection**: `crates/marketplace/src/trajectories/collect.rs`

### Testing

All components have comprehensive test coverage:

```bash
# Run trajectory quality tests
cargo test --package marketplace --lib trajectories::validate

# Run git commit detection tests
cargo test --package marketplace --lib trajectories::collect::tests::test_extract_git_commits

# Run CI/CD detection tests
cargo test --package marketplace --lib trajectories::collect::tests::test_detect_ci

# Run reward calculation tests
cargo test --package marketplace --lib trajectories::rewards
```

## Usage

```bash
# Scan local trajectories and preview quality scores
cargo marketplace trajectories scan

# Preview what would be contributed (with quality filtering)
cargo marketplace trajectories preview

# Contribute trajectories that meet quality threshold
cargo marketplace trajectories contribute

# Check contribution status and rewards
cargo marketplace trajectories earnings --detail
```

## Why This Matters

The quality scoring system ensures that trajectory contributions are:

1. **Complete** - Capture full coding sessions with measurable outcomes
2. **Valuable** - Represent real work with meaningful complexity
3. **Verified** - Include automated test/build results as reward signals
4. **Fair** - Contributors are compensated based on data quality

This creates a positive flywheel:
- High quality data → better model training
- Better models → more productivity
- More productivity → more valuable trajectories
- More trajectories → better models

## Related Documentation

- [Trajectory Contribution System](./README.md#trajectory-contribution-system)
- [Directive d-008](../../.openagents/directives/d-008.md)
- [Redaction and Privacy](./REDACTION.md)
