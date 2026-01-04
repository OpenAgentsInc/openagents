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

RLM outputs can be subjective (summaries) or objective (hashable transformations). FRLM supports **tiered verification**:

### 4.1 Objective Verification (§5.1)
For deterministic outputs:
- Validate by hash, schema, or replayable computation
- Release payments only after verification ("pay-after-verify")

### 4.2 Redundancy Verification / Consensus (§5.2)
For subjective tasks:
- Run sub-queries on **multiple workers**
- Compare outputs using similarity measures or higher-model grading
- Accept consensus and pay accordingly

### 4.3 Validator Pods and Attestations (§5.3)
Validator pods can:
- Re-run a sample
- Score divergence
- Sign an attestation
- Update provider reputation

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

FRLM defines canonical spans across categories:

| Category | Events |
|----------|--------|
| **Run** | `Run.Init`, `Run.Decode`, `Run.Done` |
| **Environment** | `Env.LoadFragment`, `Env.SelectFragments` |
| **Sub-queries** | `SubQuery.Submit`, `SubQuery.Execute`, `SubQuery.Return` |
| **Verification** | `Verify.Redundant`, `Verify.Objective`, `Verify.Attest` |
| **Resources** | `Memory.CacheHit/Miss`, `Weights.Fetch`, `GPU.Dispatch` |
| **Economics** | `Budget.Reserve`, `Budget.Settle`, `Receipt.Emit` |

Each event includes:
- **Causal links** (parent span, cause)
- **Metrics** (ms, bytes, cost)
- **Payload references** (artifact IDs, blob refs)

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
- Stores environment state (documents, indexes, buffers)
- Schedules sub-queries
- Merges results
- Enforces policy and budget constraints
- Emits trace events
- **Implementation:** Rust + async [TBD]

### 6.2 Worker Runtime (§7.2)
Workers expose:
- One or more model backends (local, datacenter endpoint)
- Optional tool execution capabilities
- Job acceptance policy
- Trace emission

### 6.3 Transport and Relay (§7.3)
Supported transports:
- Direct WebSocket/SSE
- Message relays (e.g., pub/sub)
- Optional encrypted payload transport
- **Exact relay protocol:** [TBD]

### 6.4 Budgeting and Receipts (§7.4)

**Pricing models:**
- Per-token
- Per-job
- Per-second
- Per-byte

**Budget levels:**
- Run budget (total for entire execution)
- Subquery budget (per sub-call)
- Provider-specific caps

Receipts tie spend to trace spans for full attribution.

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

## 8. Experimental Setup (§8) [PLACEHOLDERS]

### 8.1 Benchmarks

| Category | Datasets |
|----------|----------|
| Long-context QA | [Dataset A], [Dataset B] - TBD |
| Repository QA | [Repo QA benchmark] - TBD |
| Tool-use tasks | [Autonomous run / sandbox verify] - TBD |

### 8.2 Baselines

1. RLM baseline (sequential sub-calls)
2. Local-only FRLM (no federation)
3. Federated best-effort
4. Federated redundancy
5. Federated validator tier
6. Premium datacenter lane for selected steps

### 8.3 Metrics

| Metric | Description |
|--------|-------------|
| End-to-end latency | p50/p95 |
| Cost | Tokens, compute cost units |
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

From the chatgpt.md analysis, the concrete implementation path:

### Phase 1: Basic Swarm Query
```
llm_query(chunk) → swarm_query(chunk)
  └─ Nostr NIP-90 job: {chunk_id, prompt, schema}
  └─ Route to best Mac provider
  └─ Return structured result
```

### Phase 2: Async Fanout
- Issue 10-200 chunk jobs simultaneously
- Budget-bounded parallelism
- Reduce as results arrive

### Phase 3: Trace Integration
- Each subcall is a HUD span (queued → running → done)
- Buffers visible as state objects
- Real-time "execution movie"

### Phase 4: Verification Tiers
- Redundancy for untrusted workers
- Validator pods for high-stakes steps
- Reputation-weighted routing

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
