# Plan: Update FRLM Paper Synopsis with Implementation Details

**Goal:** Enhance `docs/frlm/FRLM_PAPER_SYNOPSIS.md` with concrete implementation details from the working codebase, replacing [TBD] placeholders and adding test coverage documentation.

**Status:** Ready for implementation

---

## Summary of Changes

The synopsis currently has many [TBD] placeholders and lacks concrete implementation details. We have a fully working FRLM implementation with 23 tests. This update will:

1. Replace [TBD] sections with actual implementation details
2. Add test coverage documentation
3. Add concrete type definitions and API examples
4. Document the Pylon integration

---

## Sections to Update

### 1. Section 6: Implementation Details (§7)

**Current:** Generic descriptions with [TBD] markers

**Update with:**

#### §7.1 Conductor Runtime
- Actual struct: `FrlmConductor` with fields: `policy`, `trace`, `scheduler`, `budget_spent`, `context`, `fragments`
- Key methods: `run()`, `build_fragment_queries()`, `run_fanout()`, `verify_results()`, `aggregate_results()`
- Traits: `SubQuerySubmitter`, `LocalExecutor`

#### §7.2 Worker Runtime
- `NostrSubmitter` implementation using NIP-90
- `FmLocalExecutor` for local FM Bridge fallback
- `FrlmManager` for coordinating both

#### §7.3 Transport and Relay
- NIP-90 via `NostrCommand::PublishJobBatch`
- `BatchJobRequest` struct with `id`, `prompt`, `model`, `max_tokens`

#### §7.4 Budgeting and Receipts
- `BudgetPolicy` struct: `limit_sats`, `per_query_limit_sats`, `reserve_multiplier`
- Reserve/settle pattern with `reserve_budget()`, `settle_budget()`
- Trace events: `BudgetReserve`, `BudgetSettle`

---

### 2. Section 5.1 Event Taxonomy (§6.1)

**Current:** Generic event names

**Update with actual TraceEvent variants:**
```rust
pub enum TraceEvent {
    RunInit { run_id, program, fragment_count, timestamp_ms },
    RunDone { run_id, output, iterations, total_cost_sats, total_duration_ms, timestamp_ms },
    EnvLoadFragment { run_id, fragment_id, size_bytes, timestamp_ms },
    EnvSelectFragments { run_id, query, fragment_ids, timestamp_ms },
    SubQuerySubmit { run_id, query_id, prompt_preview, fragment_id, timestamp_ms },
    SubQueryExecute { run_id, query_id, provider_id, venue, timestamp_ms },
    SubQueryReturn { run_id, query_id, result_preview, duration_ms, cost_sats, success, timestamp_ms },
    SubQueryTimeout { run_id, query_id, elapsed_ms, timestamp_ms },
    VerifyRedundant { run_id, query_id, agreement, n_of_m, passed, timestamp_ms },
    VerifyObjective { run_id, query_id, check_type, passed, timestamp_ms },
    BudgetReserve { run_id, query_id, amount_sats, remaining_sats, timestamp_ms },
    BudgetSettle { run_id, query_id, actual_sats, refund_sats, timestamp_ms },
    Aggregate { run_id, input_count, output_preview, timestamp_ms },
    FallbackLocal { run_id, reason, timestamp_ms },
}
```

---

### 3. Section 4: Verification (§5)

**Current:** Conceptual description

**Update with actual implementation:**

#### §5.1 Objective Verification
- JSON schema validation: type checking, required fields
- SHA256 hash verification: `sha256:abc123...` format
- Uses `sha2` and `hex` crates

#### §5.2 Redundancy Verification
- Similarity calculation: Jaccard (word-based for long strings), Levenshtein-like (char-based for short)
- Configurable threshold (default 0.8)
- N-of-M agreement checking

#### §5.3 Validated Verification
- Attestation metadata: `attestation_pubkey`, `attestation_sig`
- Content hash verification
- Future: full Schnorr signature verification

---

### 4. NEW Section: Test Coverage

**Add new section documenting 23 tests:**

| Module | Tests | Coverage |
|--------|-------|----------|
| conductor | `test_local_fallback`, `test_budget_tracking` | Fallback behavior, budget management |
| scheduler | `test_scheduler_basic`, `test_collect_sync`, `test_subquery_builder` | Queue management, collection, query building |
| policy | `test_quorum_all`, `test_quorum_fraction`, `test_quorum_min_count`, `test_budget_estimate`, `test_verification_tier` | Policy validation |
| trace | `test_trace_emitter`, `test_preview_truncation` | Event emission, text truncation |
| verification | 11 tests covering all tiers | Redundancy, objective, validated verification |

---

### 5. Section 8: Experimental Setup (§8)

**Current:** [PLACEHOLDERS]

**Update with:**
- Note that test suite validates core functionality
- Reference actual test data patterns used
- Document mock implementations for testing

---

### 6. Section 17: OpenAgents Implementation Notes

**Current:** Conceptual phases

**Update with actual implementation:**

#### Actual Phase Implementation
- **Phase 1-4**: Complete in `crates/frlm/`
- **Phase 5**: Verification tiers implemented with tests
- **Pylon Integration**: `FrlmIntegration` struct, trace event polling

#### Pylon UI Integration
- `frlm_panel.rs` - Budget bar, sub-query timeline
- Real-time status updates via trace events
- 3-column layout when FRLM active

---

## Files to Modify

| File | Changes |
|------|---------|
| `docs/frlm/FRLM_PAPER_SYNOPSIS.md` | Update sections 5-8, 17; add test coverage section |

---

## Implementation Notes

- Keep existing structure and section numbering
- Replace [TBD] and [PLACEHOLDER] with actual details
- Add code examples from real implementation
- Preserve theoretical content, enhance with concrete details
- Add links to source files where appropriate
