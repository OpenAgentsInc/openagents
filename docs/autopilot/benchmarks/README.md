# Autopilot Benchmark Suite

The autopilot benchmark suite provides standardized performance testing for the autonomous coding agent system. Benchmarks measure correctness, efficiency, and cost across a variety of realistic development tasks.

## Quick Start

```bash
# Run all benchmarks
cargo autopilot benchmark

# Run specific benchmark
cargo autopilot benchmark B-001

# Run by category
cargo autopilot benchmark --category file-ops

# Save results as baseline
cargo autopilot benchmark --save-baseline v1.0.0

# Compare against baseline
cargo autopilot benchmark --baseline v1.0.0

# List all baselines
cargo autopilot benchmark --list-baselines
```

## Benchmark Categories

- **file-ops**: File and directory operations (read, write, search, edit)
- **git**: Git workflows (commits, branches, merges)
- **issue-management**: Issue tracking and workflow
- **refactoring**: Code restructuring and improvement
- **testing**: Test-driven development and debugging
- **documentation**: Documentation generation
- **dependencies**: Package management
- **error-handling**: Recovery and resilience
- **context**: Information gathering and analysis
- **consistency**: Cross-file coordination
- **performance**: Optimization tasks
- **security**: Security vulnerability fixes

## Available Benchmarks

| ID | Name | Category | Description |
|----|------|----------|-------------|
| B-001 | Simple File Edit | file-ops | Change version in a single file |
| B-002 | Multi-File Search and Edit | file-ops | Replace text across multiple files |
| B-003 | Struct Rename | refactoring | Rename struct across codebase |
| B-004 | Simple Commit | git | Create a git commit |
| B-005 | Branch Workflow | git | Branch, edit, commit, merge workflow |
| B-006 | Issue Workflow | issue-management | Create and manage issues |
| B-007 | Multi-Step Refactor | refactoring | Complex refactoring task |
| B-008 | Test-Driven Fix | testing | Fix failing tests |
| B-009 | Documentation Generation | documentation | Generate API documentation |
| B-010 | Dependency Update | dependencies | Update package dependencies |
| B-011 | Error Recovery | error-handling | Handle and recover from errors |
| B-012 | Context Gathering | context | Search and analyze codebase |
| B-013 | Cross-File Consistency | consistency | Maintain consistency across files |
| B-014 | Performance Optimization | performance | Optimize slow code |
| B-015 | Security Fix | security | Fix security vulnerabilities |

## Metrics Collected

For each benchmark run, the system collects:

- **Success**: Whether the task was completed correctly
- **Duration**: Total execution time in milliseconds
- **Tokens**: Input, output, and cached token usage
- **Cost**: Total cost in USD
- **Tool Calls**: Number of tools used and errors encountered
- **Custom Metrics**: Benchmark-specific measurements

## Baseline Comparison

Baselines enable performance regression detection:

```bash
# Create baseline from current version
cargo autopilot benchmark --save-baseline v1.0.0

# Compare new code against baseline
cargo autopilot benchmark --baseline v1.0.0
```

The comparison shows:
- Success rate changes
- Performance regressions (duration, tokens, cost)
- Statistical significance of changes

## Database

Benchmark results are stored in SQLite database:
- Default location: `autopilot-benchmarks.db`
- Custom location: `--db path/to/benchmarks.db`

Schema includes:
- `benchmark_results`: Individual run results
- `baselines`: Named baseline versions
- Indexed for fast queries and comparisons

## Writing New Benchmarks

See [WRITING_BENCHMARKS.md](WRITING_BENCHMARKS.md) for details on creating new benchmark tasks.

## CI Integration

Benchmarks can run in CI to detect regressions:

```yaml
- name: Run Autopilot Benchmarks
  run: |
    cargo autopilot benchmark --save-baseline ci-${{ github.sha }}
    cargo autopilot benchmark --baseline main --fail-on-regression
```

## Troubleshooting

### Workspace Issues

Benchmarks execute in isolated temporary workspaces. If a benchmark fails:

1. Check the workspace path in error messages
2. Verify file permissions
3. Ensure git is configured (for git benchmarks)

### Performance Variations

Token usage and timing can vary between runs due to:
- Model response variations
- Cache state
- System load

Run multiple times and use baselines for statistical comparison.

## Further Reading

- [Benchmark Task Details](tasks/)
- [Writing Custom Benchmarks](WRITING_BENCHMARKS.md)
- [CI/CD Integration](CI_INTEGRATION.md)
