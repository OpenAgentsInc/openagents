# Autopilot Performance Profiling

This directory contains performance profiling data for autopilot runs.

## Quick Start

### Install cargo-flamegraph

```bash
cargo install flamegraph
```

Note: On Linux, you may need to enable perf access:
```bash
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

### Profile an Autopilot Run

```bash
# Profile a single autopilot run
cargo flamegraph --bin autopilot -- run

# Profile with specific options
cargo flamegraph --bin autopilot --output ./docs/profiles/flamegraph-$(date +%Y%m%d-%H%M%S).svg -- run

# Profile the daemon
cargo flamegraph --bin autopilotd
```

The flamegraph will be saved as `flamegraph.svg` (or the path specified with `--output`).

### View the Flamegraph

Open the SVG file in a web browser:
```bash
firefox flamegraph.svg
# or
google-chrome flamegraph.svg
```

## Understanding Flamegraphs

- **X-axis**: Alphabetical order (not time)
- **Y-axis**: Stack depth
- **Width**: CPU time percentage
- **Color**: Random (for differentiation)

### What to Look For

1. **Wide bars**: Functions consuming significant CPU time
2. **Tall stacks**: Deep call chains (potential optimization target)
3. **Repeated patterns**: Code that could be memoized or cached

## Common Bottlenecks

Based on profiling, common autopilot bottlenecks include:

- **Tool execution**: Bash, Read, Write operations
- **Regex matching**: Grep, pattern matching in logs
- **JSON serialization**: Event serialization, database writes
- **Network I/O**: Nostr relay communication

## Best Practices

1. **Profile representative workloads**: Use real issues, not synthetic tasks
2. **Multiple samples**: Run 3-5 profiles and look for patterns
3. **Compare before/after**: Profile before and after optimizations
4. **Focus on hot paths**: Optimize the widest bars first
5. **Document findings**: Add notes to this README

## Advanced Profiling

### Memory Profiling

For memory profiling, use `heaptrack`:

```bash
# Install heaptrack
sudo apt install heaptrack  # Ubuntu/Debian
sudo dnf install heaptrack  # Fedora

# Profile memory usage
heaptrack cargo run --bin autopilot -- run

# Analyze results
heaptrack_gui heaptrack.autopilot.*.gz
```

### Continuous Profiling

For ongoing performance monitoring:

```bash
# Profile every autopilot run and save to timestamped file
cargo autopilot run --profile  # (future enhancement)
```

## Profiling Results Archive

Profiling results are stored here with naming convention:
- `flamegraph-YYYYMMDD-HHMMSS.svg` - CPU flamegraphs
- `heaptrack-YYYYMMDD-HHMMSS.gz` - Memory profiles
- `notes-YYYYMMDD.md` - Analysis notes

**Note**: Profiling data is not committed to git (see `.gitignore`)

## Resources

- [Flamegraph documentation](https://github.com/flamegraph-rs/flamegraph)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [cargo-flamegraph crate](https://crates.io/crates/flamegraph)
