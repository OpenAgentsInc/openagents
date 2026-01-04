# FRLM Architecture

## System Overview

FRLM (Federated Recursive Language Models) provides distributed execution of LLM queries across a decentralized swarm of providers.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              PYLON DESKTOP                                │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────────┐  │
│  │  NostrRuntime  │  │   FmRuntime    │  │     WalletRuntime          │  │
│  │  (NIP-90 jobs) │  │ (local FM)     │  │   (Spark sats)             │  │
│  └───────┬────────┘  └───────┬────────┘  └──────────┬─────────────────┘  │
│          │                   │                      │                     │
│          └───────────────────┼──────────────────────┘                     │
│                              │                                            │
│  ┌───────────────────────────▼───────────────────────────────────────┐   │
│  │                      FRLM CONDUCTOR                                │   │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌─────────────────┐   │   │
│  │  │ Scheduler│  │  Policy   │  │  Trace   │  │   Verifier      │   │   │
│  │  │ (fanout) │  │ (budget)  │  │ (events) │  │ (redundancy)    │   │   │
│  │  └──────────┘  └───────────┘  └──────────┘  └─────────────────┘   │   │
│  └───────────────────────────────────────────────────────────────────┘   │
│                              │                                            │
│  ┌───────────────────────────▼───────────────────────────────────────┐   │
│  │                         FmVizState                                 │   │
│  │  frlm_active_run, frlm_subquery_status, frlm_runs_completed       │   │
│  └───────────────────────────┬───────────────────────────────────────┘   │
│                              │                                            │
│  ┌───────────────────────────▼───────────────────────────────────────┐   │
│  │                        FRLM UI Panel                               │   │
│  │  Budget meter, Timeline view, Query lanes                         │   │
│  └───────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Query Submission

```
User Input
    │
    ▼
FrlmProgram { query, fragments, context }
    │
    ▼
FrlmConductor.run()
    │
    ├─► Check swarm availability
    │
    ├─► Build sub-queries from fragments
    │
    └─► Reserve budget
```

### 2. Fanout Execution

```
Sub-queries
    │
    ▼
SubQueryScheduler.enqueue()
    │
    ▼
SubQuerySubmitter.submit_batch()
    │
    ├─► NostrRuntime.publish_job_batch()
    │       │
    │       ▼
    │   NIP-90 Job Requests → Relay → Providers
    │
    └─► Trace: SubQuery.Submit events
```

### 3. Result Collection

```
Provider Responses
    │
    ▼
NostrEvent::JobResult
    │
    ▼
SubQueryScheduler.record_result()
    │
    ├─► Check quorum
    │
    ├─► Check timeout
    │
    └─► Trace: SubQuery.Return events
```

### 4. Verification & Aggregation

```
Collected Results
    │
    ▼
Verifier.verify(results, tier)
    │
    ├─► Redundancy check (N-of-M agreement)
    │
    └─► Trace: Verify events
    │
    ▼
Aggregate results
    │
    ▼
FrlmResult { output, cost, duration }
```

## Key Components

### FrlmConductor (`conductor.rs`)

The main orchestrator that coordinates:
- Environment management (fragments, context)
- Sub-query scheduling
- Budget tracking
- Result aggregation
- Local fallback

```rust
pub struct FrlmConductor {
    policy: FrlmPolicy,
    trace: TraceEmitter,
    scheduler: SubQueryScheduler,
    budget_spent: u64,
    context: HashMap<String, String>,
    fragments: HashMap<String, Fragment>,
}
```

### SubQueryScheduler (`scheduler.rs`)

Handles parallel execution and collection:
- Enqueue queries for submission
- Track in-flight queries
- Collect results with timeout/quorum
- Mark timeouts

```rust
pub struct SubQueryScheduler {
    status: HashMap<String, SubQueryStatus>,
    results: HashMap<String, SubQueryResult>,
    pending: Vec<SubQuery>,
    result_rx: mpsc::Receiver<SubQueryResult>,
    result_tx: mpsc::Sender<SubQueryResult>,
}
```

### FrlmPolicy (`policy.rs`)

Configuration for execution behavior:

```rust
pub struct FrlmPolicy {
    pub budget: BudgetPolicy,       // Spending limits
    pub timeout: TimeoutPolicy,     // Per-query and total timeouts
    pub quorum: QuorumPolicy,       // How many results needed
    pub verification: VerificationTier,  // How to verify
    pub allow_local_fallback: bool, // Fall back to local FM
}
```

### TraceEmitter (`trace.rs`)

Structured event emission for observability:

```rust
pub struct TraceEmitter {
    run_id: String,
    start_time: Instant,
    events: Vec<TraceEvent>,
    sender: Option<Sender<TraceEvent>>,
}
```

## Integration Points

### SubQuerySubmitter Trait

Implement to connect to different backends:

```rust
#[async_trait]
pub trait SubQuerySubmitter: Send + Sync {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> Result<Vec<(String, String)>>;
    async fn is_available(&self) -> bool;
}
```

**Implementations:**
- `NostrSubmitter` - NIP-90 job submission via relay
- Future: Direct API calls, other networks

### LocalExecutor Trait

Implement for local fallback:

```rust
#[async_trait]
pub trait LocalExecutor: Send + Sync {
    async fn execute(&self, query: &str) -> Result<String>;
}
```

**Implementations:**
- `FmLocalExecutor` - Apple Foundation Models via FM Bridge

## State Updates

### From Conductor to UI

```rust
// In Pylon event loop
if let Some(run_state) = conductor.run_state() {
    state.frlm_active_run = Some(FrlmRunState {
        run_id: run_state.run_id.clone(),
        pending_queries: run_state.pending,
        completed_queries: run_state.completed,
        budget_used_sats: run_state.spent,
        budget_remaining_sats: run_state.remaining,
        // ...
    });
}

// Update sub-query status from trace events
for event in conductor.take_trace_events() {
    match event {
        TraceEvent::SubQuerySubmit { query_id, .. } => {
            state.frlm_subquery_status.insert(
                query_id,
                SubQueryDisplayStatus::Submitted { job_id: query_id }
            );
        }
        // ...
    }
}
```

### From Nostr Events to Scheduler

```rust
// In NostrRuntime handler
NostrEvent::JobResult { request_id, content, .. } => {
    if let Some(sender) = conductor.result_sender() {
        let _ = sender.send(SubQueryResult::success(
            request_id,
            content,
            Venue::Swarm,
            duration_ms,
        ));
    }
}
```

## Verification Flow

```
Results from Swarm
    │
    ▼
┌─────────────────────────────────────────┐
│         VerificationTier                 │
├─────────────────────────────────────────┤
│ None:                                    │
│   Accept first successful result         │
├─────────────────────────────────────────┤
│ Redundancy { n, m, threshold }:          │
│   Calculate pairwise similarity          │
│   Find result with most agreement        │
│   Pass if agreement >= threshold         │
├─────────────────────────────────────────┤
│ Objective { schema }:                    │
│   Validate against JSON schema           │
├─────────────────────────────────────────┤
│ Validated { validator_pubkey }:          │
│   Check for attestation signature        │
└─────────────────────────────────────────┘
    │
    ▼
VerifyResult { passed, accepted_result, agreement }
```

## Budget Flow

```
Query Submission
    │
    ▼
reserve_budget(query_id, estimated_cost)
    │
    ├─► budget_spent += estimated_cost
    │
    └─► Trace: BudgetReserve event
    │
    ▼
... execution ...
    │
    ▼
settle_budget(query_id, actual_cost, reserved)
    │
    ├─► refund = reserved - actual
    │
    ├─► budget_spent -= refund
    │
    └─► Trace: BudgetSettle event
```

## Error Handling

| Error | Handling |
|-------|----------|
| `BudgetExceeded` | Reject query before submission |
| `Timeout` | Mark queries as timed out, check quorum |
| `QuorumNotMet` | Return error or trigger local fallback |
| `VerificationFailed` | Reject results, optionally retry |
| `NoProviders` | Trigger local fallback if allowed |

## Configuration

### Default Policy

```rust
FrlmPolicy {
    budget: BudgetPolicy {
        limit_sats: 10000,
        cost_per_1k_tokens: 10,
    },
    timeout: TimeoutPolicy {
        per_query: Duration::from_secs(30),
        total: Duration::from_secs(300),
    },
    quorum: QuorumPolicy {
        quorum: Quorum::Fraction(0.8),
        allow_partial: true,
    },
    verification: VerificationTier::None,
    allow_local_fallback: true,
}
```

### Production Policy

```rust
FrlmPolicy::default()
    .with_budget_sats(5000)
    .with_timeout_secs(60)
    .with_quorum_fraction(0.9)
    .with_verification(VerificationTier::redundancy(3, 2))
    .with_local_fallback(true)
```

## Future Enhancements

1. **Compute Mobility**: Route queries to optimal venue (local/swarm/datacenter)
2. **Dynamic Pricing**: Adjust costs based on provider bids
3. **Reputation System**: Weight providers by historical reliability
4. **Caching**: Cache results for repeated queries
5. **Streaming**: Stream partial results as they arrive
