# FRLM - Federated Recursive Language Models

FRLM extends the RLM (Recursive Language Model) execution model with federation capabilities, enabling distributed sub-query execution across a swarm of providers via NIP-90.

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          FRLM CONDUCTOR                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │   Environment   │  │   Scheduler     │  │      Budget/Policy      │  │
│  │  (context vars, │  │  (fanout,       │  │  (caps, timeouts,       │  │
│  │   fragments)    │  │   quorum)       │  │   verification tier)    │  │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘  │
│           │                    │                        │                │
│           └────────────────────┼────────────────────────┘                │
│                                │                                         │
│  ┌─────────────────────────────▼─────────────────────────────────────┐  │
│  │                        Trace Emitter                               │  │
│  │    Run.Init → SubQuery.Submit → SubQuery.Return → Run.Done        │  │
│  └─────────────────────────────┬─────────────────────────────────────┘  │
└────────────────────────────────┼────────────────────────────────────────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
                    ▼            ▼            ▼
              ┌──────────┐ ┌──────────┐ ┌──────────┐
              │  Local   │ │  Swarm   │ │  Remote  │
              │ (FM/RLM) │ │ (NIP-90) │ │  (API)   │
              └──────────┘ └──────────┘ └──────────┘
```

## Core Concepts

### FrlmConductor

The main orchestrator that manages federated execution:

```rust
use frlm::{FrlmConductor, FrlmPolicy};

let policy = FrlmPolicy::default()
    .with_budget_sats(1000)
    .with_timeout_secs(30)
    .with_quorum_fraction(0.8);

let mut conductor = FrlmConductor::new(policy);
```

### Sub-Query Fanout

Split a query into multiple sub-queries and execute them in parallel:

```rust
use frlm::types::{FrlmProgram, Fragment};

let program = FrlmProgram::new("Summarize this document")
    .with_fragments(vec![
        Fragment::new("frag-1", "Chapter 1 content..."),
        Fragment::new("frag-2", "Chapter 2 content..."),
        Fragment::new("frag-3", "Chapter 3 content..."),
    ]);

let result = conductor.run(program, &submitter, Some(&local_executor)).await?;
```

### Quorum Policies

Control how many results are required before proceeding:

```rust
use frlm::policy::Quorum;

// Wait for all results
Quorum::All

// Wait for 80% of results
Quorum::Fraction(0.8)

// Wait for at least 5 results
Quorum::MinCount(5)

// Take whatever we have after timeout
Quorum::BestEffort
```

### Verification Tiers

Verify results from untrusted workers:

```rust
use frlm::policy::VerificationTier;

// No verification - trust provider
VerificationTier::None

// Redundancy: N-of-M agreement with similarity threshold
VerificationTier::redundancy(3, 2)  // 2 of 3 must agree

// Objective: Check against JSON schema
VerificationTier::objective(Some(schema_string))

// Validated: Require attestation from trusted validator
VerificationTier::validated("validator_pubkey")
```

### Budget Tracking

Track costs in real Bitcoin sats:

```rust
let policy = FrlmPolicy::default()
    .with_budget_sats(1000);  // 1000 sats limit

// During execution
conductor.can_afford(100);      // Check if 100 sats available
conductor.budget_remaining();   // Get remaining budget
```

## Trace Events

Every operation emits structured trace events for observability:

| Event | Description |
|-------|-------------|
| `RunInit` | FRLM run started |
| `EnvLoadFragment` | Fragment loaded into environment |
| `SubQuerySubmit` | Sub-query submitted to provider |
| `SubQueryExecute` | Sub-query execution started |
| `SubQueryReturn` | Sub-query completed |
| `SubQueryTimeout` | Sub-query timed out |
| `VerifyRedundant` | Redundancy verification result |
| `BudgetReserve` | Budget reserved for query |
| `BudgetSettle` | Budget settled after completion |
| `Aggregate` | Results aggregated |
| `FallbackLocal` | Fell back to local execution |
| `RunDone` | FRLM run completed |

## Integration

### With Nostr (NIP-90)

Implement `SubQuerySubmitter` to submit jobs via Nostr:

```rust
use frlm::conductor::SubQuerySubmitter;

struct NostrSubmitter { /* ... */ }

#[async_trait]
impl SubQuerySubmitter for NostrSubmitter {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> Result<Vec<(String, String)>> {
        // Publish NIP-90 job requests
        // Return mapping of query_id -> job_id
    }

    async fn is_available(&self) -> bool {
        // Check relay connection
    }
}
```

### Local Fallback

Implement `LocalExecutor` for fallback when swarm is unavailable:

```rust
use frlm::conductor::LocalExecutor;

struct FmLocalExecutor { /* ... */ }

#[async_trait]
impl LocalExecutor for FmLocalExecutor {
    async fn execute(&self, query: &str) -> Result<String> {
        // Execute using local FM bridge
    }
}
```

## Module Structure

```
crates/frlm/
├── src/
│   ├── lib.rs           # Module exports
│   ├── conductor.rs     # FrlmConductor orchestrator
│   ├── scheduler.rs     # Async fanout scheduler
│   ├── policy.rs        # Budget, timeout, quorum policies
│   ├── verification.rs  # Result verification
│   ├── trace.rs         # Trace event taxonomy
│   ├── types.rs         # Core types
│   └── error.rs         # Error types
└── docs/
    └── README.md        # This file
```

## Example Flow

1. **Run Init**: Conductor initializes with policy and fragments
2. **Fragment Split**: Query split into per-fragment sub-queries
3. **Fanout**: Sub-queries submitted to swarm in parallel
4. **Collection**: Results collected with quorum/timeout
5. **Verification**: Results verified (redundancy check)
6. **Aggregation**: Results combined into final output
7. **Budget Settlement**: Actual costs recorded

```
User Query: "Summarize this 100-page document"
     │
     ▼
┌─────────────────────────────────────────┐
│ Split into 10 fragments (10 pages each) │
└─────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────┐
│ Fanout: 10 NIP-90 jobs to swarm         │
│ Budget: Reserve 100 sats per job        │
└─────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────┐
│ Collect: Wait for 80% (8 of 10)         │
│ Timeout: 30 seconds                     │
└─────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────┐
│ Aggregate: Combine 8 summaries          │
│ Budget: Settle actual costs             │
└─────────────────────────────────────────┘
     │
     ▼
Final Summary
```

## See Also

- [RLM Crate](../../rlm/docs/README.md) - Base RLM execution engine
- [Viz FRLM](../../viz/docs/FRLM.md) - FRLM visualization components
- [NIP-90 Spec](https://github.com/nostr-protocol/nips/blob/master/90.md) - Data Vending Machine
