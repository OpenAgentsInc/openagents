# Nostr Relay Performance Benchmarks

Comprehensive performance benchmarks for the nostr-relay crate using Criterion.

## Running Benchmarks

Run all benchmarks:
```bash
cargo bench --package nostr-relay --features full
```

Run specific benchmark group:
```bash
cargo bench --package nostr-relay --features full --bench relay_benchmarks insertion_benches
cargo bench --package nostr-relay --features full --bench relay_benchmarks query_benches
cargo bench --package nostr-relay --features full --bench relay_benchmarks filter_benches
cargo bench --package nostr-relay --features full --bench relay_benchmarks scaling_benches
cargo bench --package nostr-relay --features full --bench relay_benchmarks special_benches
```

Run specific benchmark:
```bash
cargo bench --package nostr-relay --features full insert_single_event
cargo bench --package nostr-relay --features full query_by_kind
```

## Benchmark Groups

### insertion_benches
- **insert_single_event**: Single event insertion performance
- **batch_insert**: Batch insertion with varying sizes (10, 50, 100, 500, 1000 events)
- **concurrent_inserts**: 10 threads each inserting 10 events

### query_benches
- **query_by_id**: Query single event by ID
- **query_by_kind**: Query events filtered by kind
- **query_by_author**: Query events by author pubkey
- **query_time_range**: Query events within time range (since/until)
- **query_with_limit**: Query with varying limit sizes (10, 50, 100, 500, 1000)
- **query_by_tag**: Query events by tag filters (#e, #p, etc.)

### filter_benches
- **filter_match_simple**: Simple filter matching (kinds only)
- **filter_match_complex**: Complex filter with kinds, authors, tags, and time range
- **match_10_filters**: Match event against 10 different filters

### scaling_benches
- **query_scaling**: Query performance vs database size (100, 1000, 10000 events)
- **insert_scaling**: Insert performance vs database size (100, 1000, 10000 events)

### special_benches
- **replaceable_event_update**: Update replaceable events (kind 0 metadata)
- **event_to_json**: Event serialization to JSON
- **event_from_json**: Event deserialization from JSON

## Performance Targets

Based on typical relay requirements:

- **Single event insertion**: < 1ms
- **Query by ID**: < 0.5ms
- **Query by kind (100 results)**: < 5ms
- **Filter matching**: < 10μs
- **Concurrent inserts**: > 1000 events/sec

## Output

Criterion generates HTML reports in `target/criterion/`:
```
target/criterion/
├── insert_single_event/
│   ├── report/
│   │   └── index.html
│   └── base/
├── query_by_kind/
│   ├── report/
│   │   └── index.html
│   └── base/
└── ...
```

Open `target/criterion/report/index.html` to view all benchmark results.

## Continuous Benchmarking

To track performance over time:
1. Run benchmarks before changes: `cargo bench --package nostr-relay --features full`
2. Make code changes
3. Run benchmarks again: `cargo bench --package nostr-relay --features full`
4. Criterion will automatically compare against baseline

To save a baseline:
```bash
cargo bench --package nostr-relay --features full -- --save-baseline my-baseline
```

To compare against a baseline:
```bash
cargo bench --package nostr-relay --features full -- --baseline my-baseline
```

## Profiling

For detailed profiling, use perf or flamegraph:

```bash
# Install flamegraph
cargo install flamegraph

# Profile a specific benchmark
sudo cargo flamegraph --bench relay_benchmarks --features full -- --bench insert_single_event
```

## Notes

- Benchmarks use temporary SQLite databases (created per test)
- Event data is randomly generated for each benchmark run
- Connection pool settings use default configuration
- All benchmarks run with `--features full` to enable cryptographic verification
