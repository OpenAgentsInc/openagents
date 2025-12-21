# Autopilot Benchmarks

Performance benchmarks for critical autopilot operations.

## NIP-SA State Encryption (`nip_sa_state.rs`)

Benchmarks NIP-44 encryption/decryption performance for agent state management.

### Benchmarks

1. **nip44_encryption**: NIP-44 encryption at various state sizes (1KB, 10KB, 100KB)
2. **nip44_decryption**: NIP-44 decryption at various state sizes
3. **state_serialization**: JSON serialization performance
4. **state_deserialization**: JSON deserialization performance
5. **full_encrypt_cycle**: Complete serialize → encrypt flow
6. **full_decrypt_cycle**: Complete decrypt → deserialize flow

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p autopilot --bench nip_sa_state

# Run specific benchmark
cargo bench -p autopilot --bench nip_sa_state -- nip44_encryption

# Generate HTML report
cargo bench -p autopilot --bench nip_sa_state -- --output-format html
```

### Interpreting Results

Benchmarks report:
- **Throughput**: Bytes processed per second
- **Time**: Mean execution time with standard deviation
- **Comparison**: Performance across different data sizes

Expected performance targets:
- 1KB state: < 1ms encryption/decryption
- 10KB state: < 5ms encryption/decryption
- 100KB state: < 50ms encryption/decryption

### Implementation Notes

- Uses criterion for statistical analysis
- Tests realistic state sizes based on actual agent state
- Includes both individual operations and full cycles
- Random keys generated per benchmark iteration

## Future Benchmarks

Planned benchmarks for additional operations:

1. **Relay Publishing**: Nostr event publishing latency
2. **Trajectory Collection**: Overhead of trajectory tracking
3. **Database Operations**: SQLite query performance
4. **Git Operations**: Commit and push performance
5. **Issue Management**: MCP tool call performance

## Performance Monitoring

Benchmark results should be tracked over time to detect regressions:

```bash
# Save baseline
cargo bench -p autopilot --bench nip_sa_state --save-baseline main

# Compare against baseline
cargo bench -p autopilot --bench nip_sa_state --baseline main
```

Store baseline results in `target/criterion/` for historical comparison.
