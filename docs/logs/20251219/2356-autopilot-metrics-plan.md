# Plan: Autopilot Metrics & Analysis System

## Goal
Build measurement infrastructure for all 50 metrics in IMPROVEMENT-DIMENSIONS.md so we can measure autopilot improvements.

## Summary
Add `cargo autopilot analyze` command that computes metrics from trajectory JSON files, with both single-file and aggregate modes.

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `crates/autopilot/src/analyze.rs` | **NEW** - Core analysis module |
| `crates/autopilot/src/main.rs` | Add `Analyze` subcommand |
| `crates/autopilot/src/lib.rs` | Export `analyze` module |

---

## Phase 1: Core Analysis Command

### 1.1 Create `analyze.rs` with metric structs

```rust
// Key structs to add:
pub struct TrajectoryAnalysis {
    pub session_id: String,
    pub model: String,
    pub performance: PerformanceMetrics,
    pub cost: CostMetrics,
    pub errors: ErrorMetrics,
    pub quality: QualityMetrics,
    pub tool_usage: ToolUsageMetrics,
}

pub struct PerformanceMetrics {
    pub total_duration_ms: u64,
    pub tool_latency_stats: LatencyStats,  // min/max/p50/p95
    pub parallel_tool_batches: u32,
}

pub struct CostMetrics {
    pub total_cost_usd: f64,
    pub cache_hit_rate: f64,
    pub tokens_by_step_type: HashMap<String, u64>,
}

pub struct ErrorMetrics {
    pub success: bool,
    pub tool_error_rate: f64,
    pub errors_by_tool: HashMap<String, u32>,
}

pub struct QualityMetrics {
    pub num_turns: u32,
    pub thinking_blocks: u32,
    pub tool_diversity: f64,
}

pub struct ToolUsageMetrics {
    pub calls_by_tool: HashMap<String, u32>,
    pub success_rate_by_tool: HashMap<String, f64>,
}
```

### 1.2 Add CLI command to `main.rs`

```rust
/// Analyze trajectory metrics
Analyze {
    /// Path to trajectory JSON file or directory
    path: PathBuf,

    /// Aggregate metrics across all files in directory
    #[arg(long)]
    aggregate: bool,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}
```

### 1.3 Implement single-file analysis

Key computations:
- **Performance**: Pair ToolCall→ToolResult by tool_id, compute latencies
- **Cost**: Sum tokens by step type, compute cache_hit_rate
- **Errors**: Count failed ToolResults, group by tool name
- **Quality**: Count thinking blocks, unique tools / total calls

---

## Phase 2: Aggregate Analysis

### 2.1 Directory scanning
- Find all `.json` files in directory
- Load and analyze each trajectory
- Compute aggregate statistics

### 2.2 Aggregate metrics struct

```rust
pub struct AggregateAnalysis {
    pub trajectory_count: usize,
    pub by_model: HashMap<String, ModelStats>,
    pub avg_cost: f64,
    pub avg_duration_ms: f64,
    pub overall_success_rate: f64,
    pub avg_tool_error_rate: f64,
}
```

---

## Phase 3: Output Formatting

### 3.1 Human-readable output
```
================================================================================
Trajectory Analysis: be8b05ba
================================================================================
PERFORMANCE
  Duration:        15.5s
  Tool Latency:    p50=42ms, p95=3.7s
  Parallel Batches: 2

COST
  Total:           $0.0653
  Cache Hit Rate:  99.3%

ERRORS
  Success:         YES
  Tool Error Rate: 0%

QUALITY
  Turns:           3
  Tool Diversity:  100%
================================================================================
```

### 3.2 JSON output (for dashboards)
Use serde_json to serialize `TrajectoryAnalysis` struct.

---

## Implementation Order

1. **Create `analyze.rs`** with metric structs (~50 lines)
2. **Add Analyze command** to main.rs CLI (~20 lines)
3. **Implement `analyze_trajectory()`** - loads JSON, computes metrics (~150 lines)
4. **Implement text output** using colored crate (~50 lines)
5. **Implement JSON output** using serde (~10 lines)
6. **Implement aggregate mode** - directory scan + stats (~100 lines)
7. **Test** with existing trajectories in `docs/logs/20251219/`

Total: ~400 lines of new code

---

## Key Algorithms

### Tool Latency Computation
```rust
// Pair ToolCall with ToolResult by tool_id
for step in trajectory.steps {
    match step.step_type {
        ToolCall { tool_id, .. } => pending.insert(tool_id, step.timestamp),
        ToolResult { tool_id, .. } => {
            if let Some(start) = pending.remove(&tool_id) {
                latencies.push(step.timestamp - start);
            }
        }
    }
}
```

### Cache Hit Rate
```rust
let cache_hit_rate = usage.cache_read_tokens as f64
    / (usage.cache_read_tokens + usage.input_tokens) as f64;
```

### Tool Error Rate
```rust
let failed = steps.iter().filter(|s| matches!(s, ToolResult { success: false, .. })).count();
let total = steps.iter().filter(|s| matches!(s, ToolCall { .. })).count();
let error_rate = failed as f64 / total as f64;
```

---

## Usage Examples

```bash
# Analyze single trajectory
cargo autopilot analyze docs/logs/20251219/1610-list-the-cargo-toml-files.json

# JSON output for scripting
cargo autopilot analyze --json docs/logs/20251219/1610-list-the-cargo-toml-files.json

# Aggregate across all runs today
cargo autopilot analyze --aggregate docs/logs/20251219/

# Aggregate with JSON for dashboard
cargo autopilot analyze --aggregate --json docs/logs/20251219/
```

---

## Future Extensions (not in this plan)

- `cargo autopilot benchmark` - Standard tasks for consistent measurement
- Dashboard web UI
- Trend detection and alerting
- Integration with CI for regression testing
