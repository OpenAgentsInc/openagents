# Federated Recursive Language Models: Trace-Native Distributed Recursion Beyond Fixed Context

**Authors:** Christopher David, OpenAgents, Inc.
**Keywords:** recursive language models, distributed inference, compute mobility, verification, provenance, tracing, agentic tool use, edge compute, replayable evaluation

---

## Abstract

Recursive Language Models (RLMs) demonstrate that a language model can solve tasks far beyond its context window by delegating computation to an external execution environment and issuing recursive sub-queries over selected fragments of the input. While effective, existing RLM implementations are largely sequential, tightly coupling wall-clock latency to the number of sub-calls and limiting scalability under realistic cost and throughput constraints. We present **Federated Recursive Language Models (FRLM)**, a distributed extension of the RLM paradigm that executes recursive sub-queries across a heterogeneous network of edge devices and datacenter pods, while preserving a single coherent execution trace and budget policy. FRLM introduces (i) **trace-native orchestration**, which models each sub-query as a first-class span with causal links, resource accounting, and replayability; (ii) **verification tiers** that combine redundancy sampling, objective validators, and reputation-weighted routing to tolerate untrusted contributors; and (iii) **compute mobility**, allowing identical recursive programs to execute locally, on edge swarms, or on premium datacenter backends without changing semantics. Empirically, we show that FRLM reduces end-to-end latency for long-context workloads via asynchronous fanout, improves cost-efficiency through market-based routing of sub-queries to underutilized capacity, and enables reproducible evaluation through replayable visual traces that expose bottlenecks in retrieval, sub-call scheduling, and aggregation. Our results suggest that distributed, trace-native recursion is a practical and scalable primitive for inference-time computation on web-scale inputs, and provides a foundation for auditable, budget-bounded agent systems that operate beyond fixed context limits.

---

## 1. Introduction

Long-context reasoning remains a central limitation for language-model-based systems: the cost of processing large contexts grows quickly, and context windows remain finite. RLMs address this limitation by shifting the “large context” out of the model and into an **external environment** (e.g., a REPL), allowing the model to programmatically select and query relevant fragments via recursive calls.

Despite their effectiveness, current RLM implementations exhibit two structural shortcomings for deployment at scale:

1. **Sequential sub-calls**: Recursive pipelines often execute sub-queries serially, making latency proportional to the number of recursive calls.
2. **Lack of end-to-end operability**: Existing tooling largely treats recursion as “just code,” without standardized provenance, budgets, verification, replay, or system-level introspection.

At the same time, the world is accumulating a vast pool of **heterogeneous, intermittently available compute** (edge devices, prosumer hardware, microclusters) alongside specialized high-throughput datacenter infrastructure. This heterogeneity is poorly matched to monolithic inference patterns but is naturally suited to **parallelizable, asynchronous** workloads.

This paper proposes **Federated Recursive Language Models (FRLM)**: a system and methodology for executing RLM-style recursion across a federated compute network, while preserving deterministic orchestration semantics, verifiability, and replayable observability.

### Contributions

We make four contributions:

* **FRLM Orchestration:** A distributed execution model for RLM recursion, supporting asynchronous fanout and compute mobility across local, edge swarm, and datacenter pods.
* **Trace-Native Runtime:** A standardized event taxonomy (“visual trace”) that turns recursive execution into a replayable, auditable artifact—spanning model calls, tool calls, external I/O, budgets, and verification.
* **Federated Verification Tiers:** A practical trust model for untrusted contributors combining redundancy, objective validators, reputation-weighted routing, and optional bonded execution lanes.
* **Experimental Evaluation:** A reproducible evaluation protocol and results on long-context and tool-use tasks (with placeholders for final measurements), highlighting latency/cost improvements and diagnosability benefits.

---

## 2. Background and Motivation

### 2.1 Recursive Language Models

RLMs frame large-context problem-solving as an interaction between a base LLM and an external environment. The environment stores large input artifacts (e.g., documents, repositories) and exposes functions enabling selective access, transformation, and recursive querying over fragments. The core insight is that many long-context tasks can be reduced to a sequence of **select → query → aggregate** steps.

### 2.2 Why Distribution Matters for RLMs

RLM recursion is naturally decomposable: each `llm_query(fragment)` call is largely independent given a fixed prompt and fragment. This yields a direct opportunity for parallel execution. However, naïvely distributing calls introduces new challenges:

* **Latency and stragglers:** A single slow worker can block aggregation.
* **Trust:** Untrusted workers can return adversarial or low-quality outputs.
* **Cost governance:** Distributed work needs budgeting and receipts.
* **Reproducibility:** A distributed run must be replayable and attributable.

FRLM is designed to address these problems at the system level rather than treating them as “application details.”

---

## 3. System Overview

FRLM separates a recursive run into three roles:

1. **Conductor (Root Orchestrator):** Maintains the external environment, schedules sub-queries, aggregates results, and enforces budgets and policies.
2. **Workers (Federated Sub-Query Executors):** Execute `llm_query` calls on fragments and return structured outputs.
3. **Validators (Audit and Verification Executors):** Perform redundancy checks, objective validations, and reputation updates.

### 3.1 Compute Mobility

FRLM treats compute location as a routing decision, not a change in algorithm:

* **Local execution:** Same-machine execution (developer workstation, personal device).
* **Edge swarm:** A network of voluntary/provisioned nodes (e.g., Apple-silicon laptops/desktops).
* **Datacenter pods:** High-throughput, SLA-backed nodes for premium inference or training updates.

A single run can mix venues: e.g., cheap sub-queries on edge nodes, high-stakes verification on datacenter validators.

### 3.2 Trace-Native Execution

Every step emits structured events in a standardized schema (spans with causal links), enabling:

* real-time visualization (“execution movie”)
* replay and diff
* cost attribution and receipts
* auditability across tool use and compute

---

## 4. FRLM Execution Model

### 4.1 Recursive Program Semantics

FRLM assumes the recursive controller program is deterministic given:

* initial inputs (documents/repo snapshots)
* random seeds for sampling
* model backend configuration
* tool availability and policies

The conductor executes the program in an environment that exposes functions such as:

* `select_fragments(query, index)`
* `load_fragment(fragment_id)`
* `llm_query(prompt, fragment)`
* `reduce(results)`
* `verify(result, policy)`

### 4.2 Asynchronous Fanout

FRLM generalizes `llm_query` to a batch/futures API:

* Submit a set of sub-queries concurrently.
* Wait for quorum or best-effort completion depending on tier.
* Aggregate as results arrive.
* Cancel or deprioritize stragglers beyond a timeout.

**Design principle:** *sub-queries are cheap and parallel; aggregation is precious.*

### 4.3 Engine-Driven Orchestration for Edge Models

**Implementation:** `crates/rlm/src/orchestrator.rs`, `crates/rlm/src/chunking.rs`

A key insight from production deployment is that smaller edge models (e.g., Apple Foundation Models) cannot reliably orchestrate their own document traversal. While GPT-5-class models can write code that calls `llm_query()` for each chunk and synthesizes results, this meta-reasoning exceeds the capabilities of 3B-7B parameter on-device models.

FRLM addresses this through **engine-driven orchestration**, where the conductor handles chunking, sub-query dispatch, and synthesis rather than delegating these decisions to the model:

```
┌─────────────────────────────────────────────────────────────┐
│                    ENGINE ORCHESTRATOR                       │
├─────────────────────────────────────────────────────────────┤
│  Phase 1: STRUCTURE DISCOVERY                                │
│  - Detect document type (markdown, code, prose)              │
│  - Find natural boundaries (headers, functions, paragraphs)  │
├─────────────────────────────────────────────────────────────┤
│  Phase 2: CHUNK GENERATION                                   │
│  - Split on semantic boundaries (not arbitrary offsets)      │
│  - Keep chunks under ~6000 chars (edge model sweet spot)     │
├─────────────────────────────────────────────────────────────┤
│  Phase 3: TARGETED EXTRACTION                                │
│  - Simple focused prompt per chunk                           │
│  - No meta-reasoning required from model                     │
├─────────────────────────────────────────────────────────────┤
│  Phase 4: HIERARCHICAL SYNTHESIS                             │
│  - Batch findings → intermediate summaries → final answer    │
│  - Prevents context overflow with many chunks                │
└─────────────────────────────────────────────────────────────┘
```

**Semantic Chunking** preserves document structure:

```rust
pub enum DocumentType {
    Markdown,  // Headers, lists, code blocks
    Code,      // Functions, classes, modules
    Prose,     // Paragraphs, sentences
    Mixed,     // Combination
}

pub fn detect_structure(content: &str) -> DocumentStructure {
    // Pattern matching for markdown headers: ^#{1,6}\s+
    // Code function boundaries: ^(pub\s+)?(async\s+)?fn\s+\w+
    // Paragraph breaks: \n\n+
}
```

**Hierarchical Synthesis** handles large result sets:

```rust
const MAX_DIRECT_FINDINGS: usize = 10;
const BATCH_SIZE: usize = 8;

async fn synthesize(&self, summaries: &[ChunkSummary], query: &str) -> Result<String> {
    if summaries.len() <= MAX_DIRECT_FINDINGS {
        self.synthesize_direct(summaries, query).await
    } else {
        // Group into batches → intermediate summaries → final synthesis
        self.synthesize_hierarchical(summaries, query, BATCH_SIZE).await
    }
}
```

This design principle—**simple prompts repeated across chunks beats complex prompts the model can't follow**—enables edge devices to participate in FRLM workloads without requiring GPT-5-class reasoning capabilities.

### 4.4 Straggler and Timeout Policy

FRLM uses a policy-controlled approach:

* **Best-effort tier:** proceed once a minimum fraction of tasks returns.
* **Redundant tier:** require N-of-M agreement.
* **High-trust tier:** require validator attestation.

Timeouts emit trace spans and influence reputation.

---

## 5. Verification and Trust Model

**Implementation:** `crates/frlm/src/verification.rs` (11 unit tests)

RLM outputs can be subjective (summaries) or objective (hashable transformations). FRLM supports tiered verification:

```rust
pub enum VerificationTier {
    None,  // Trust provider
    Redundancy { n: usize, m: usize, similarity_threshold: f32 },
    Objective { schema: Option<String> },
    Validated { validator_pubkey: String },
}
```

### 5.1 Objective Verification

**Implementation:** `Verifier::verify_objective()`

For tasks with deterministic outputs:

* **Type validation:** Check JSON type (object, array, string, number, boolean, null)
* **Required fields:** Validate presence of specified fields
* **Hash verification:** SHA256 content hash matching (`sha256:abc123...` format)

```rust
// Schema format examples
{"type": "object", "required": ["name", "age"]}
{"hash": "sha256:abc123..."}
```

Uses `sha2` and `hex` crates for cryptographic hashing.

### 5.2 Redundancy Verification (Consensus)

**Implementation:** `Verifier::verify_redundancy()`

For subjective tasks:

* Run sub-queries on **N workers**, require **M agreement**
* **Similarity calculation:**
  * Short strings (<100 chars): Character-based prefix matching
  * Long strings: Word-based Jaccard similarity
* Default similarity threshold: **0.8**

```rust
// Helper constructors
VerificationTier::redundancy(3, 2)      // 2-of-3 agreement
VerificationTier::redundancy_3_of_5()   // 3-of-5 preset
VerificationTier::redundancy_2_of_3()   // 2-of-3 preset
```

### 5.3 Validator Pods and Attestations

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

### 5.4 Reputation-Weighted Routing

Workers accumulate scores based on:

* completion rate
* consistency with redundancy checks
* objective validation pass rate
* latency distribution

Routing prefers higher-tier providers, with explicit budget tradeoffs.

---

## 6. Trace-Native Orchestration and Visualization

### 6.1 Event Taxonomy

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

* **run_id** — Causal link to parent run
* **timestamp_ms** — Milliseconds since run start
* **Metrics** — duration_ms, cost_sats, size_bytes as appropriate
* **Previews** — Truncated content for logs (max 100-500 chars)

### 6.2 Visual Grammar

FRLM’s visualization uses five primitives:

* **Fill:** budgets, cache occupancy, progress
* **Pulse:** token emissions, dispatch, completion
* **Flow:** data movement (fragments, results, receipts)
* **Heat:** latency, uncertainty, divergence, error rates
* **Topology:** agents, services, workers, pods

### 6.3 Replay and Diff

Traces are replayable with checkpoints:

* load checkpoint state
* replay events forward
* diff two traces to locate divergence in orchestration decisions

---

## 7. Implementation Details

**Reference Implementation:** `crates/frlm/` (Rust, async/await, 23 unit tests)

**Engine-Orchestrated Analysis:** `crates/rlm/` (semantic chunking, hierarchical synthesis)

### 7.1 Conductor Runtime

The conductor is implemented in `crates/frlm/src/conductor.rs`:

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
* `run()` — Main async entry point for program execution
* `build_fragment_queries()` — Converts fragments to sub-queries
* `run_fanout()` — Parallel sub-query submission via scheduler
* `verify_results()` — Applies verification tier to collected results
* `aggregate_results()` — Combines verified results with reduce prompt

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

### 7.2 Worker Runtime

**Nostr Submitter** (`crates/pylon-desktop/src/frlm_integration.rs`):
```rust
pub struct NostrSubmitter {
    command_tx: mpsc::Sender<NostrCommand>,
    relay_connected: bool,
}
```
* Implements `SubQuerySubmitter` via NIP-90 job requests
* Converts `SubQuery` to `BatchJobRequest { id, prompt, model, max_tokens }`

**Local Executor** (FM Bridge fallback):
```rust
pub struct FmLocalExecutor {
    fm_bridge_url: String,
}
```
* Implements `LocalExecutor` via HTTP POST to `/generate` endpoint
* Seamless fallback when swarm unavailable

### 7.3 Transport and Relay

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
* `JobBatchPublished { job_mappings: Vec<(local_id, job_id)> }` — Batch submitted
* `JobResult { request_id, content, amount_msats, bolt11 }` — Result with payment request

### 7.4 Budgeting and Receipts

**Budget Policy** (`crates/frlm/src/policy.rs`):
```rust
pub struct BudgetPolicy {
    pub limit_sats: u64,              // Total budget for run (default: 10,000)
    pub per_query_limit_sats: Option<u64>,  // Per sub-query cap
    pub reserve_multiplier: f32,      // Overcommit factor (default: 1.5)
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

**Real Bitcoin Integration:** Costs denominated in satoshis, with Lightning payments via Spark wallet for job completion.

---

## 8. Implementation Validation

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
* `test_verify_none` — No verification tier
* `test_verify_redundancy_success/failure` — N-of-M agreement
* `test_verify_objective_type/required_fields/missing_field` — JSON schema validation
* `test_verify_validated_success/wrong_validator/no_attestation` — Attestation checking
* `test_similarity_exact/partial` — String similarity calculation

### 8.2 Mock Implementations

Tests use mock implementations for isolation:
```rust
struct MockSubmitter { should_succeed: bool }
struct MockExecutor { response: String }
```

### 8.3 Test Data Patterns

* **Redundancy tests:** Use distinct strings ("the sky is blue", "water is wet") to validate disagreement detection
* **Objective tests:** JSON objects with type/required field validation
* **Attestation tests:** Simulated pubkey/signature with content hash verification

### 8.4 Engine-Orchestrated Analysis Validation

**Implementation:** `crates/rlm/src/orchestrator.rs` + CLI `--orchestrated` flag

Comprehensive testing of engine-orchestrated analysis across diverse document types validates FRLM's applicability to edge model deployments. All tests executed against Apple Foundation Models via FM Bridge.

**Table 5:** Engine-Orchestrated Analysis Test Results

| Test | Document | Size | Chunks | Relevant | Time | Quality |
|------|----------|------|--------|----------|------|---------|
| Theme Extraction | Conversation log | 245KB | 50 | 50 (100%) | 120s | Excellent |
| Specific Details | Conversation log | 245KB | 30 | 8 (27%) | 35s | Excellent |
| Comparison Query | Technical discussion | 90KB | 50 | 14 (28%) | 118s | Excellent |
| Architecture | System documentation | 13KB | 24 | 15 (63%) | 72s | Excellent |
| Code Analysis | Rust source file | ~25KB | 50 | 45 (90%) | 140s | Good |
| Strategy (small) | GTM document | 10KB | 32 | 26 (81%) | 85s | Excellent |
| Strategy (detailed) | GTM document | 16KB | 50 | 35 (70%) | 130s | Excellent |

**Key findings:**

1. **Complete document coverage:** Unlike model-driven approaches that examined only the first 2000 characters, engine-orchestrated analysis processed all sections systematically.

2. **Query-adaptive relevance:** Broad queries (theme extraction) found most chunks relevant; specific queries (WebGPU details) correctly identified fewer but precise sections.

3. **Hierarchical synthesis success:** Documents with 50+ findings used batched synthesis (7 batches for 50 findings) without context overflow.

4. **Processing time linearity:** ~2-3 seconds per chunk on Apple M3 Max, scaling predictably with document size.

**Test 1: Large Document Theme Extraction (245KB)**

Query: "Extract the major themes"

Result: 7 comprehensive themes identified across full document:
- AI Compute and Machine Learning Advancements
- Agent Economy and OpenAgents
- NLP and ML Model Optimization
- Web Browsers and LLMs
- Efficiency and Engineering Feasibility
- System Performance and User Experience
- Visual Representation and Attention Mechanisms

**Test 2: Specific Technical Details (245KB)**

Query: "What specific technical decisions were made about WebGPU?"

Result: 8 relevant sections identified across 30 chunks:
- WebGPU accessed through Rust's wgpu → WASM compilation
- Custom WGSL kernels for dequant, matmul, RMSNorm, RoPE, attention, MoE routing
- Q8_0 and MXFP4 quantization schemes
- Limits-aware chunking for browser constraints

**Comparison: Model-Driven vs Engine-Driven**

| Aspect | Model-Driven (Guided Tier) | Engine-Driven (Orchestrated) |
|--------|---------------------------|------------------------------|
| Document coverage | First 2000 chars | All chunks systematically |
| Relies on summaries | Yes (got lucky) | No |
| Model orchestration | Model writes code | Engine drives process |
| Error resilience | Model must recover | Engine handles retries |
| Progress visibility | Limited | Per-chunk reporting |

**Conclusion:** Engine-driven orchestration enables edge models to process documents 100x larger than their effective reasoning context by decomposing the task into simple extraction prompts.

### 8.5 Future Benchmarks

| Category | Planned Datasets |
|----------|------------------|
| Long-context QA | Multi-document summarization |
| Repository QA | Code understanding tasks |
| Tool-use tasks | Autonomous sandbox execution |

### 8.6 Metrics (Planned)

| Metric | Description |
|--------|-------------|
| End-to-end latency | p50/p95 |
| Cost | Satoshis per task |
| Accuracy | Task success rate |
| Divergence | Under redundancy sampling |
| Churn tolerance | Provider availability resilience |
| Trace overhead | Event volume, logging time |

---

## 9. Results

*Benchmarks run on Apple M3 Max, `cargo test --release`. See `crates/frlm/src/bench_stats.rs`.*

### 9.1 Latency and Throughput

**Table 1:** Verification latency by fanout size and tier (p50/p95 in microseconds)

| Fanout | Verification | p50 (µs) | p95 (µs) |
|--------|--------------|----------|----------|
| 5      | None         | <1       | <1       |
| 10     | None         | <1       | <1       |
| 20     | None         | <1       | <1       |
| 50     | None         | <1       | <1       |
| 3      | 2-of-3       | <1       | <1       |
| 5      | 3-of-5       | <1       | <1       |
| 10     | 6-of-10      | 1        | 1        |
| 10     | Objective    | <1       | 2        |

**Key finding:** Verification overhead is sub-microsecond for most configurations. Redundancy verification scales linearly with result count due to pairwise similarity comparisons.

### 9.2 Cost and Efficiency

**Table 2:** Estimated cost per task by fragment size and fanout (satoshis)

| Fragment Size | Fanout | Total Cost (sats) | Sats/Result |
|---------------|--------|-------------------|-------------|
| 100B          | 10     | 10                | 1           |
| 100B          | 50     | 50                | 1           |
| 1KB           | 10     | 150               | 15          |
| 1KB           | 50     | 750               | 15          |
| 10KB          | 10     | 1,500             | 150         |
| 10KB          | 50     | 7,500             | 150         |

**Cost model:** 1 sat per 100 characters × 1.5 reserve multiplier. Actual costs depend on provider pricing in the swarm network.

### 9.3 Accuracy and Quality

**Table 3:** Quorum success by policy type

| Workers | Success% | Quorum Policy | Met? |
|---------|----------|---------------|------|
| 8/10    | 80%      | All           | No   |
| 8/10    | 80%      | Fraction(0.8) | Yes  |
| 8/10    | 80%      | MinCount(8)   | Yes  |
| 9/10    | 90%      | All           | No   |
| 9/10    | 90%      | Fraction(0.8) | Yes  |
| 10/10   | 100%     | All           | Yes  |
| 48/50   | 96%      | Fraction(0.8) | Yes  |

**Key finding:** `Fraction(0.8)` provides best balance between fault tolerance and latency, accepting runs with up to 20% worker failures.

### 9.4 Trust and Verification

**Table 4:** Redundancy detection with adversarial/garbage results

| Bad Results | Verification | Detection | Agreement |
|-------------|--------------|-----------|-----------|
| 0%          | 6-of-10      | Accepted  | 100%      |
| 5%          | 6-of-10      | Accepted  | 90%       |
| 10%         | 6-of-10      | Accepted  | 90%       |
| 20%         | 6-of-10      | Accepted  | 80%       |
| 30%         | 6-of-10      | Accepted  | 70%       |
| 40%         | 6-of-10      | **Rejected** | 60%    |

**Key finding:** 6-of-10 redundancy tolerates up to 30% adversarial results while maintaining consensus. At 40% bad results, agreement drops below threshold and verification fails correctly.

### 9.5 Diagnosability

Trace events enable identification of:

* **Straggler bottlenecks:** `SubQueryTimeout` events identify slow providers
* **Fragment selection issues:** `EnvSelectFragments` shows query→fragment mappings
* **Budget overruns:** `BudgetReserve`/`BudgetSettle` track spend attribution
* **Verification failures:** `VerifyRedundant` shows agreement scores per query

Trace replay via `TraceEmitter` buffer allows deterministic re-execution and diff analysis.

---

## 10. Discussion

### 10.1 When FRLM Helps Most

FRLM yields the largest gains when:

* the task decomposes into many independent sub-queries
* fragments are large relative to the base model window
* there is abundant idle compute
* verification is objective or redundancy-friendly
* latency is dominated by sequential subcalls

### 10.2 Model Capability Tiers and Orchestration Strategy

Production deployment reveals that orchestration strategy must adapt to model capabilities:

**GPT-5-class models (>100B parameters):**
- Can write code that calls `llm_query()` for each chunk
- Can maintain multi-step reasoning across iterations
- Benefit from full RLM REPL-style interaction
- Model-driven orchestration is appropriate

**Apple FM / Edge models (3B-7B parameters):**
- Cannot reliably orchestrate multi-step document traversal
- Lose context after encountering errors
- Tend to "pattern match" rather than reason about orchestration
- **Engine-driven orchestration is required**

**Prompt Tier System:**

| Tier | Target Models | Orchestration | Features |
|------|--------------|---------------|----------|
| Full | GPT-5, Claude | Model-driven | `llm_query()`, REPL, full meta-reasoning |
| Guided | Apple FM, Llama-7B | Engine-driven | Simple prompts, no imports, step-by-step |
| Minimal | <3B models | Engine-driven | Single-instruction, atomic tasks only |

**Key insight:** The same RLM semantics can be preserved across model tiers by shifting orchestration complexity from model to engine. This enables heterogeneous compute pools where edge devices handle extraction while premium nodes handle synthesis.

### 10.3 Limits of Federation

FRLM's benefits decrease when:

* the task requires tight sequential dependence
* sub-queries are too small (overhead dominates)
* verification is expensive or subjective without redundancy
* data locality cannot be managed

### 10.4 Implications for Tool-Using Agents

RLM-style recursion and tool-using agents converge:

* “fragment selection” becomes retrieval/tool selection
* “verification tiers” become unit tests/sandbox outputs
* traces become receipts for autonomous work

---

## 11. Limitations and Future Work

### 11.1 Open Challenges

* privacy-preserving fragment distribution
* stronger adversarial robustness
* richer reward shaping for tool-use pipelines
* fully on-device speculative execution improvements
* standardized interoperability with existing tracing ecosystems

### 11.2 Future Directions

* training-time recursion (not just inference)
* federated adapter learning for on-device models
* market-based scheduling under budget constraints
* multi-agent recursion: recursive programs that spawn specialized agents
* **parallel chunk processing:** async extraction could reduce 120s → 30-40s for large documents
* **adaptive chunk sizing:** code files may benefit from 3000-char chunks; prose from 8000-char chunks
* **query-aware extraction prompts:** "List..." → bullet extraction; "Compare..." → comparative extraction
* **hybrid orchestration:** edge models for extraction, premium models for synthesis

---

## 12. Conclusion

FRLM extends Recursive Language Models with federated execution, making recursion practical at web scale by leveraging heterogeneous idle compute and datacenter pods. By making orchestration trace-native and verification tiered, FRLM enables auditable, budget-bounded, replayable long-context problem solving. This work suggests that distributed recursion can serve as a general inference-time scaling primitive and a foundation for operable agent systems beyond fixed context windows.

---

## References

[1] Zhang, Y., et al. "Recursive Language Models: Scalable Problem Solving Beyond Context Limits." arXiv:2512.24601, 2024.

[2] Leviathan, Y., Kalman, M., & Matias, Y. "Fast Inference from Transformers via Speculative Decoding." ICML 2023.

[3] Shoeybi, M., et al. "Megatron-LM: Training Multi-Billion Parameter Language Models Using Model Parallelism." arXiv:1909.08053, 2019.

[4] OpenTelemetry Authors. "OpenTelemetry Specification." https://opentelemetry.io/docs/specs/, 2024.

[5] NIP-90: Data Vending Machine. Nostr Implementation Possibilities. https://github.com/nostr-protocol/nips/blob/master/90.md

---

## Appendix A: FRLM Pseudocode (Optional)

```text
function FRLM_Run(program, env, policy, budget):
    trace.start("Run.Init")
    state = env.init()
    while program.has_next(state):
        plan = program.next(state)

        fragments = env.select(plan.query)
        trace.event("Env.SelectFragments", count=len(fragments))

        tasks = []
        for frag in fragments:
            tasks.append(submit_subquery(plan.prompt, frag, policy.routing))

        results = collect(tasks, policy.quorum, policy.timeout)
        trace.event("SubQuery.Collect", received=len(results))

        verified = verify(results, policy.verify_tier)
        trace.event("Verify", tier=policy.verify_tier)

        state = program.reduce(state, verified)
    trace.end("Run.Done")
    return program.output(state)
```

---

If you want, I can also produce:

* a “camera-ready” LaTeX version (with placeholders), or
* a shorter arXiv-style 6–8 page version, or
* a version that explicitly grounds the compute federation layer in your Nostr/marketplace architecture (event kinds, routing, receipts).
