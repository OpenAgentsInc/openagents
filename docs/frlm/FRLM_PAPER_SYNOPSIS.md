# Federated Recursive Language Models (FRLM) - Comprehensive Synopsis

**Paper:** Draft - "Federated Recursive Language Models: Trace-Native Distributed Recursion Beyond Fixed Context"
**Authors:** *[TBD]*
**Status:** Draft with placeholders for experimental results
**Source:** `docs/frlm/paper.md` + `docs/frlm/chatgpt.md` (implementation analysis)

---

## Executive Summary

Federated Recursive Language Models (FRLM) extend the RLM paradigm (Zhang et al., arXiv:2512.24601) to **distributed execution across heterogeneous compute networks**. While RLMs demonstrate that LLMs can solve tasks beyond their context window via recursive sub-queries, existing implementations are sequential and tightly coupled to wall-clock latency. FRLM addresses this by introducing: (i) **trace-native orchestration** where every sub-query is a first-class span with causal links and replayability; (ii) **verification tiers** combining redundancy, objective validators, and reputation-weighted routing for untrusted contributors; and (iii) **compute mobility** allowing identical recursive programs to execute locally, on edge swarms, or on datacenter backends without changing semantics. (§Abstract)

---

## 1. Core Problem: Sequential Sub-calls and Operability Gap

The original RLM paper demonstrated powerful results but identified two critical limitations (§1, §2.2):

### 1.1 Sequential Sub-calls
- RLM implementations execute sub-queries **serially**
- Latency scales linearly with number of recursive calls
- The original paper explicitly calls out "async sub-calls and sandboxed REPLs could significantly reduce runtime/cost"

### 1.2 Lack of End-to-End Operability
- Recursion treated as "just code" without:
  - Standardized provenance
  - Budget governance
  - Verification mechanisms
  - Replay capabilities
  - System-level introspection

### 1.3 Underutilized Heterogeneous Compute
- Vast pool of edge devices, prosumer hardware, microclusters exists
- Poorly matched to monolithic inference patterns
- Naturally suited to **parallelizable, asynchronous** workloads

---

## 2. The FRLM Architecture

### 2.1 Three-Role Separation (§3)

| Role | Responsibility |
|------|----------------|
| **Conductor (Root Orchestrator)** | Maintains environment, schedules sub-queries, aggregates results, enforces budgets/policies |
| **Workers (Federated Sub-Query Executors)** | Execute `llm_query` calls on fragments, return structured outputs |
| **Validators (Audit Executors)** | Perform redundancy checks, objective validations, reputation updates |

### 2.2 Compute Mobility (§3.1)

FRLM treats compute location as a **routing decision**, not an algorithm change:

| Venue | Description | Use Case |
|-------|-------------|----------|
| **Local** | Same-machine execution | Development, privacy-sensitive |
| **Edge Swarm** | Network of voluntary/provisioned nodes (e.g., Apple Silicon devices) | Cheap parallel sub-queries |
| **Datacenter Pods** | High-throughput, SLA-backed nodes | Premium inference, high-stakes verification |

**Key insight:** A single run can **mix venues** - cheap sub-queries on edge nodes, critical verification on datacenter validators.

### 2.3 Trace-Native Execution (§3.2)

Every step emits structured events enabling:
- Real-time visualization ("execution movie")
- Replay and diff
- Cost attribution and receipts
- Auditability across tool use and compute

---

## 3. FRLM Execution Model (§4)

### 3.1 Recursive Program Semantics (§4.1)

Programs are **deterministic** given:
- Initial inputs (documents/repo snapshots)
- Random seeds for sampling
- Model backend configuration
- Tool availability and policies

### 3.2 Environment Functions

The conductor exposes:
```
select_fragments(query, index)   # Find relevant fragments
load_fragment(fragment_id)       # Load specific fragment
llm_query(prompt, fragment)      # Query LLM over fragment
reduce(results)                  # Aggregate sub-query results
verify(result, policy)           # Validate against policy
```

### 3.3 Asynchronous Fanout (§4.2)

FRLM generalizes `llm_query` to a **batch/futures API**:

1. Submit set of sub-queries **concurrently**
2. Wait for quorum or best-effort completion (tier-dependent)
3. Aggregate as results arrive
4. Cancel or deprioritize stragglers beyond timeout

**Design principle:** *"Sub-queries are cheap and parallel; aggregation is precious."*

### 3.4 Straggler and Timeout Policy (§4.3)

| Tier | Behavior |
|------|----------|
| **Best-effort** | Proceed once minimum fraction returns |
| **Redundant** | Require N-of-M agreement |
| **High-trust** | Require validator attestation |

Timeouts emit trace spans and influence reputation scores.

---

## 4. Verification and Trust Model (§5)

**Implementation:** `crates/frlm/src/verification.rs`

RLM outputs can be subjective (summaries) or objective (hashable transformations). FRLM supports **tiered verification**:

```rust
pub enum VerificationTier {
    None,  // Trust provider
    Redundancy { n: usize, m: usize, similarity_threshold: f32 },
    Objective { schema: Option<String> },
    Validated { validator_pubkey: String },
}
```

### 4.1 Objective Verification (§5.1)

**Implementation:** `Verifier::verify_objective()`

For deterministic outputs:
- **Type validation:** Check JSON type (object, array, string, number, boolean, null)
- **Required fields:** Validate presence of specified fields
- **Hash verification:** SHA256 content hash matching (`sha256:abc123...` format)

```rust
// Schema format
{"type": "object", "required": ["name", "age"]}
{"hash": "sha256:abc123..."}
```

Uses `sha2` and `hex` crates for cryptographic hashing.

### 4.2 Redundancy Verification / Consensus (§5.2)

**Implementation:** `Verifier::verify_redundancy()`

For subjective tasks:
- Run sub-queries on **N workers**, require **M agreement**
- **Similarity calculation:**
  - Short strings (<100 chars): Character-based prefix matching
  - Long strings: Word-based Jaccard similarity
- Default similarity threshold: **0.8**

```rust
// Helper constructors
VerificationTier::redundancy(3, 2)      // 2-of-3 agreement
VerificationTier::redundancy_3_of_5()   // 3-of-5 preset
```

### 4.3 Validator Pods and Attestations (§5.3)

**Implementation:** `Verifier::verify_validated()` + `check_attestation()`

Validator attestations stored in result metadata:
```rust
result.metadata.insert("attestation_pubkey", validator_pubkey);
result.metadata.insert("attestation_sig", signature);
```

Verification steps:
1. Check `attestation_pubkey` matches expected validator
2. Compute SHA256 hash of content
3. Verify signature covers content hash
4. Future: Full Schnorr signature verification via Nostr

### 4.4 Reputation-Weighted Routing (§5.4)

Workers accumulate scores based on:
- Completion rate
- Consistency with redundancy checks
- Objective validation pass rate
- Latency distribution

Routing **prefers higher-tier providers** with explicit budget tradeoffs.

---

## 5. Trace-Native Orchestration (§6)

### 5.1 Event Taxonomy (§6.1)

**Implementation:** `crates/frlm/src/trace.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TraceEvent {
    // Run lifecycle
    RunInit { run_id, program, fragment_count, timestamp_ms },
    RunDone { run_id, output, iterations, total_cost_sats, total_duration_ms, timestamp_ms },

    // Environment
    EnvLoadFragment { run_id, fragment_id, size_bytes, timestamp_ms },
    EnvSelectFragments { run_id, query, fragment_ids, timestamp_ms },

    // Sub-query lifecycle
    SubQuerySubmit { run_id, query_id, prompt_preview, fragment_id, timestamp_ms },
    SubQueryExecute { run_id, query_id, provider_id, venue, timestamp_ms },
    SubQueryReturn { run_id, query_id, result_preview, duration_ms, cost_sats, success, timestamp_ms },
    SubQueryTimeout { run_id, query_id, elapsed_ms, timestamp_ms },

    // Verification
    VerifyRedundant { run_id, query_id, agreement, n_of_m, passed, timestamp_ms },
    VerifyObjective { run_id, query_id, check_type, passed, timestamp_ms },

    // Budget
    BudgetReserve { run_id, query_id, amount_sats, remaining_sats, timestamp_ms },
    BudgetSettle { run_id, query_id, actual_sats, refund_sats, timestamp_ms },

    // Aggregation & Fallback
    Aggregate { run_id, input_count, output_preview, timestamp_ms },
    FallbackLocal { run_id, reason, timestamp_ms },
}
```

**TraceEmitter** manages event emission:
```rust
pub struct TraceEmitter {
    run_id: String,
    start_time: web_time::Instant,
    sender: Option<mpsc::Sender<TraceEvent>>,
    buffer: Vec<TraceEvent>,
    buffering: bool,
}
```

Each event includes:
- **run_id** - Causal link to parent run
- **timestamp_ms** - Milliseconds since run start
- **Metrics** - duration_ms, cost_sats, size_bytes as appropriate
- **Previews** - Truncated content for logs (max 100-500 chars)

### 5.2 Visual Grammar (§6.2)

Five visualization primitives:

| Primitive | Represents |
|-----------|------------|
| **Fill** | Budgets, cache occupancy, progress |
| **Pulse** | Token emissions, dispatch, completion |
| **Flow** | Data movement (fragments, results, receipts) |
| **Heat** | Latency, uncertainty, divergence, error rates |
| **Topology** | Agents, services, workers, pods |

### 5.3 Replay and Diff (§6.3)

Traces are replayable with checkpoints:
1. Load checkpoint state
2. Replay events forward
3. Diff two traces to locate divergence in orchestration decisions

---

## 6. Implementation Details (§7)

### 6.1 Conductor Runtime (§7.1)

**Implementation:** `crates/frlm/src/conductor.rs` (Rust + async)

```rust
pub struct FrlmConductor {
    policy: FrlmPolicy,
    trace: TraceEmitter,
    scheduler: SubQueryScheduler,
    budget_spent: u64,
    context: HashMap<String, String>,
    fragments: HashMap<String, Fragment>,
    trace_rx: Option<mpsc::Receiver<TraceEvent>>,
}
```

**Key Methods:**
- `run()` - Main async entry point for program execution
- `build_fragment_queries()` - Converts fragments to sub-queries
- `run_fanout()` - Parallel sub-query submission
- `verify_results()` - Applies verification tier
- `aggregate_results()` - Combines verified results

**Extension Traits:**
```rust
#[async_trait]
pub trait SubQuerySubmitter: Send + Sync {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> Result<Vec<(String, String)>>;
    async fn is_available(&self) -> bool;
}

#[async_trait]
pub trait LocalExecutor: Send + Sync {
    async fn execute(&self, query: &str) -> Result<String>;
}
```

### 6.2 Worker Runtime (§7.2)

**Nostr Submitter** (`crates/pylon-desktop/src/frlm_integration.rs`):
```rust
pub struct NostrSubmitter {
    command_tx: mpsc::Sender<NostrCommand>,
    relay_connected: bool,
}
```
- Implements `SubQuerySubmitter` via NIP-90 job requests
- Converts `SubQuery` to `BatchJobRequest { id, prompt, model, max_tokens }`

**Local Executor** (FM Bridge fallback):
```rust
pub struct FmLocalExecutor {
    fm_bridge_url: String,
}
```
- Implements `LocalExecutor` via HTTP POST to `/generate` endpoint
- Seamless fallback when swarm unavailable

### 6.3 Transport and Relay (§7.3)

**Protocol:** Nostr NIP-90 (Data Vending Machine)

```rust
pub enum NostrCommand {
    PublishJobBatch { jobs: Vec<BatchJobRequest> },
    SubscribeJobResults { request_ids: Vec<String> },
    // ...
}

pub struct BatchJobRequest {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}
```

**Events:**
- `JobBatchPublished { job_mappings: Vec<(local_id, job_id)> }`
- `JobResult { request_id, content, amount_msats, bolt11 }`

### 6.4 Budgeting and Receipts (§7.4)

**Budget Policy** (`crates/frlm/src/policy.rs`):
```rust
pub struct BudgetPolicy {
    pub limit_sats: u64,           // Total budget for run
    pub per_query_limit_sats: Option<u64>,  // Per sub-query cap
    pub reserve_multiplier: f32,   // Overcommit factor (default 1.2)
}
```

**Reserve/Settle Pattern:**
```rust
// Before submitting sub-query
conductor.reserve_budget(query_id, estimated_cost)?;
trace.budget_reserve(query_id, amount, remaining);

// After receiving result
conductor.settle_budget(query_id, actual_cost, reserved);
trace.budget_settle(query_id, actual, refund);
```

**Real Bitcoin Integration:** Costs in satoshis via Spark wallet, with actual Lightning payments for job completion.

---

## 7. Mapping to "Fracked Macs" / Edge Swarm (from chatgpt.md)

The companion analysis identifies Apple Silicon devices as ideal FRLM workers:

### 7.1 Why Edge Macs Fit

| RLM Requirement | Mac Swarm Capability |
|-----------------|---------------------|
| REPL + sub-calls | Each Mac is a sub-LM worker |
| Async sub-calls | Spin up many subcalls in parallel |
| Cost efficiency | No datacenter prices per token |
| Sandboxing | REPL runs locally or sharded |

### 7.2 Map/Reduce Analogy

FRLM is effectively **map/reduce where map steps are LLM calls**:

```
Root (Conductor):
  - Holds REPL state + buffers + hypothesis
  - Decides chunks to examine (regex, indexing, sampling)

Workers (Fracked Macs):
  - Each llm_query(chunk) becomes a job routed to:
    - Local Mac running Apple FM / open model
    - LAN bundle (Exo-style)
    - Datacenter fallback

Merge:
  - Workers return summaries/classifications/facts
  - Root stores in buffers, queries again or computes final answer
```

### 7.3 Simple Implementation Plan

1. **Root RLM** runs in Pylon (or cloud sandbox) with persistent REPL state
2. Replace `llm_query()` with `swarm_query()`:
   - Emits Nostr/NIP-90 job: `{chunk_id, prompt, schema}`
   - Routes to best available Mac provider
3. **Async fanout:** Issue 10-200 chunk jobs at once (budget-bounded), then reduce
4. **Trace into HUD:** Each subcall is a span (queued → running → done), buffers become visible state objects

---

## 8. Implementation Validation (§8)

### 8.1 Test Suite

The implementation includes **23 unit tests** validating core functionality:

| Module | Tests | Coverage |
|--------|-------|----------|
| **conductor** | `test_local_fallback`, `test_budget_tracking` | Fallback behavior, budget reserve/settle |
| **scheduler** | `test_scheduler_basic`, `test_collect_sync`, `test_subquery_builder` | Queue management, collection, query building |
| **policy** | `test_quorum_all`, `test_quorum_fraction`, `test_quorum_min_count`, `test_budget_estimate`, `test_verification_tier` | Quorum policies, budget estimation |
| **trace** | `test_trace_emitter`, `test_preview_truncation` | Event emission, text truncation |
| **verification** | 11 tests | All verification tiers |

**Verification Tests (11 total):**
- `test_verify_none` - No verification tier
- `test_verify_redundancy_success/failure` - N-of-M agreement
- `test_verify_objective_type/required_fields/missing_field` - JSON schema validation
- `test_verify_validated_success/wrong_validator/no_attestation` - Attestation checking
- `test_similarity_exact/partial` - String similarity calculation

### 8.2 Mock Implementations

Tests use mock implementations for isolation:

```rust
struct MockSubmitter { should_succeed: bool }
struct MockExecutor { response: String }
```

### 8.3 Test Data Patterns

- **Redundancy tests:** Use distinct strings ("the sky is blue", "water is wet") to validate disagreement detection
- **Objective tests:** JSON objects with type/required field validation
- **Attestation tests:** Simulated pubkey/signature with content hash verification

### 8.4 Future Benchmarks

| Category | Planned Datasets |
|----------|------------------|
| Long-context QA | Multi-document summarization |
| Repository QA | Code understanding tasks |
| Tool-use tasks | Autonomous sandbox execution |

### 8.5 Metrics (Planned)

| Metric | Description |
|--------|-------------|
| End-to-end latency | p50/p95 |
| Cost | Satoshis per task |
| Accuracy | Task success rate |
| Divergence | Under redundancy sampling |
| Churn tolerance | Provider availability resilience |
| Trace overhead | Event volume, logging time |

---

## 9. Expected Results (§9) [PLACEHOLDERS]

### 9.1 Latency and Throughput
- **Table 1:** End-to-end latency vs baseline - [TBD]
- **Figure 1:** Latency breakdown rail visualization - [PLACEHOLDER]

### 9.2 Cost and Efficiency
- **Table 2:** Cost per solved task - [TBD]
- **Figure 2:** Budget utilization over time - [PLACEHOLDER]

### 9.3 Accuracy and Quality
- **Table 3:** Task success rate and confidence intervals - [TBD]

### 9.4 Trust and Verification
- **Table 4:** Fraud/low-quality detection rate - [TBD]
- **Figure 3:** Reputation evolution and routing shift - [PLACEHOLDER]

### 9.5 Diagnosability Case Studies
Trace replay identifies:
- Straggler bottlenecks
- Fragment selection mistakes
- Tool-call loops
- Cost overruns
- **Figure 4:** Trace diff highlighting divergence - [PLACEHOLDER]

---

## 10. When FRLM Helps Most (§10.1)

| Condition | Benefit |
|-----------|---------|
| Task decomposes into many independent sub-queries | Maximum parallelism |
| Fragments large relative to base model window | Efficient chunking |
| Abundant idle compute available | Cost savings |
| Verification is objective or redundancy-friendly | Trust without expensive validation |
| Latency dominated by sequential subcalls | Direct speedup |

---

## 11. Limits of Federation (§10.2)

| Condition | Challenge |
|-----------|-----------|
| Tight sequential dependence | Cannot parallelize |
| Sub-queries too small | Overhead dominates |
| Verification expensive/subjective without redundancy | Trust costs high |
| Data locality cannot be managed | Transfer costs dominate |

---

## 12. Implications for Tool-Using Agents (§10.3)

RLM recursion and tool-using agents **converge**:

| RLM Concept | Agent Equivalent |
|-------------|------------------|
| Fragment selection | Retrieval/tool selection |
| Verification tiers | Unit tests/sandbox outputs |
| Traces | Receipts for autonomous work |

---

## 13. Limitations and Future Work (§11)

### 13.1 Open Challenges
- Privacy-preserving fragment distribution
- Stronger adversarial robustness
- Richer reward shaping for tool-use pipelines
- Fully on-device speculative execution improvements
- Standardized interoperability with existing tracing ecosystems

### 13.2 Future Directions
- **Training-time recursion** (not just inference)
- **Federated adapter learning** for on-device models
- **Market-based scheduling** under budget constraints
- **Multi-agent recursion:** Recursive programs spawning specialized agents

---

## 14. FRLM Pseudocode (Appendix A)

```python
function FRLM_Run(program, env, policy, budget):
    trace.start("Run.Init")
    state = env.init()

    while program.has_next(state):
        plan = program.next(state)

        # Select relevant fragments
        fragments = env.select(plan.query)
        trace.event("Env.SelectFragments", count=len(fragments))

        # Submit sub-queries in parallel
        tasks = []
        for frag in fragments:
            tasks.append(submit_subquery(plan.prompt, frag, policy.routing))

        # Collect with quorum/timeout policy
        results = collect(tasks, policy.quorum, policy.timeout)
        trace.event("SubQuery.Collect", received=len(results))

        # Verify according to tier
        verified = verify(results, policy.verify_tier)
        trace.event("Verify", tier=policy.verify_tier)

        # Reduce into state
        state = program.reduce(state, verified)

    trace.end("Run.Done")
    return program.output(state)
```

---

## 15. Comparison: RLM vs FRLM

| Aspect | RLM (Zhang et al.) | FRLM |
|--------|-------------------|--------|
| **Execution** | Sequential sub-calls | Async fanout across federation |
| **Compute** | Single machine/API | Local + Edge Swarm + Datacenter |
| **Observability** | Ad-hoc logging | Trace-native with replay |
| **Trust** | Implicit (trusted API) | Tiered verification + reputation |
| **Budget** | Per-run cost tracking | Hierarchical budgets + receipts |
| **Straggler handling** | None (blocking) | Policy-controlled timeouts |
| **Scalability** | Limited by sequential latency | Scales with available workers |

---

## 16. Key Takeaways for Implementation

### For Conductor Implementation
1. **Deterministic program execution** - same inputs → same execution plan
2. **Futures-based sub-query API** - submit batch, collect with quorum
3. **Policy enforcement** - budgets, timeouts, verification tiers
4. **Trace emission** - every decision is a span

### For Worker Implementation
1. **Stateless execution** - receive fragment + prompt, return result
2. **Multiple backend support** - local model, API endpoint
3. **Job acceptance policy** - can decline based on load/capability
4. **Trace contribution** - emit execution spans

### For Verification Implementation
1. **Objective validators** - hash/schema/replay verification
2. **Redundancy sampling** - N-of-M consensus for subjective tasks
3. **Reputation tracking** - completion, consistency, latency scores
4. **Attestation signing** - cryptographic proof of validation

### Integration with OpenAgents/Nostr
- `swarm_query()` emits NIP-90 job requests
- Routing based on provider reputation + budget
- Receipts as Nostr events for attribution
- HUD visualization of trace spans

---

## 17. OpenAgents-Specific Implementation Notes

### Implementation Status: **COMPLETE**

All phases implemented in `crates/frlm/` with 23 passing tests.

| Phase | Status | Location |
|-------|--------|----------|
| Phase 1: Conductor Core | ✅ Complete | `crates/frlm/src/conductor.rs` |
| Phase 2: Async Fanout | ✅ Complete | `crates/frlm/src/scheduler.rs` |
| Phase 3: Trace Integration | ✅ Complete | `crates/frlm/src/trace.rs` |
| Phase 4: Policy & Budget | ✅ Complete | `crates/frlm/src/policy.rs` |
| Phase 5: Verification Tiers | ✅ Complete | `crates/frlm/src/verification.rs` |

### Pylon Desktop Integration

**File:** `crates/pylon-desktop/src/frlm_integration.rs`

```rust
pub struct FrlmIntegration {
    trace_rx: Option<mpsc::Receiver<TraceEvent>>,
    conductor: Option<FrlmConductor>,
    policy: FrlmPolicy,
    manager: Option<FrlmManager>,
}

pub struct FrlmManager {
    submitter: Arc<NostrSubmitter>,
    local_executor: Option<Arc<FmLocalExecutor>>,
}
```

**Key Methods:**
- `init()` - Wire NostrRuntime + FM Bridge URL
- `start_run()` - Initialize conductor and UI state
- `poll()` - Process trace events, update `FmVizState`
- `finish_run()` - Cleanup conductor state

**Trace Event Processing:**
```rust
// In PylonCore::poll()
if self.frlm.poll(&mut self.state) {
    processed = true;
}
```

Events map to UI state updates:
- `SubQuerySubmit` → `SubQueryDisplayStatus::Submitted`
- `SubQueryExecute` → `SubQueryDisplayStatus::Executing`
- `SubQueryReturn` → `SubQueryDisplayStatus::Complete` + budget update
- `BudgetReserve/Settle` → Budget bar updates

### Pylon UI Panel

**File:** `crates/pylon-desktop/src/ui/frlm_panel.rs`

Features:
- **Budget bar:** Real-time spend vs limit visualization
- **Sub-query timeline:** Horizontal rail with query status
- **3-column layout:** When FRLM active, shows trace panel alongside chat
- **Status indicators:** Pending/executing/complete/timeout per query

### Running an FRLM Program

```rust
// In application code
let program = FrlmProgram {
    run_id: uuid::Uuid::new_v4().to_string(),
    query: "Analyze this codebase".to_string(),
    fragments: vec![...],
    reduce_prompt: "Combine the analyses...".to_string(),
};

core.frlm.start_run(program, &mut core.state);
// Poll loop handles trace events automatically
```

---

## Appendix: Reference Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         CONDUCTOR                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Environment │  │  Scheduler  │  │   Budget    │              │
│  │   (REPL)    │  │  (Fanout)   │  │  (Policy)   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│         │                │                │                      │
│         └────────────────┼────────────────┘                      │
│                          │                                       │
│                    ┌─────▼─────┐                                 │
│                    │   Trace   │                                 │
│                    │  Emitter  │                                 │
│                    └───────────┘                                 │
└─────────────────────────┬───────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
    ┌──────────┐    ┌──────────┐    ┌──────────┐
    │  Local   │    │   Edge   │    │Datacenter│
    │  Worker  │    │  Swarm   │    │   Pod    │
    └──────────┘    └──────────┘    └──────────┘
          │               │               │
          │         ┌─────┴─────┐         │
          │         │           │         │
          │    ┌────▼───┐ ┌────▼───┐     │
          │    │ Mac 1  │ │ Mac 2  │     │
          │    └────────┘ └────────┘     │
          │                              │
          └──────────────┬───────────────┘
                         │
                         ▼
                  ┌─────────────┐
                  │  Validator  │
                  │    Pods     │
                  └─────────────┘
```

---

## Appendix: Glossary

| Term | Definition |
|------|------------|
| **Conductor** | Root orchestrator maintaining environment and scheduling |
| **Worker** | Federated node executing sub-queries |
| **Validator** | Node performing verification and attestation |
| **Span** | Single traceable event with causal links |
| **Fanout** | Parallel submission of multiple sub-queries |
| **Quorum** | Minimum responses required before aggregation |
| **Reputation** | Score based on completion, consistency, latency |
| **Compute Mobility** | Ability to route same job to local/edge/datacenter |
| **Trace-Native** | Every operation emits structured, replayable events |

---

*Synopsis generated from draft paper (docs/frlm/paper.md) and implementation analysis (docs/frlm/chatgpt.md). Note: Experimental results sections contain [TBD] placeholders pending actual evaluation.*
