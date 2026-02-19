# RLM Dashboard: OpenAgents Research Workflow Tool

**Status:** The web dashboard and sync services described here are archived out of the current
workspace. This document is retained as a design reference.

A W&B-style experiment tracking and visualization dashboard for Recursive Language Models, built into openagents.com.

## Why This Matters for OpenAgents

**The wedge:** Autopilot is our coding agent. It's compelling because it ships results.

**The moat:** The RLM Dashboard captures the *research workflow* around agent execution—making experiments reproducible, comparable, and shareable. This is what W&B does for ML training; we do it for agent swarm execution.

**The network effect:** Every RLM run through Pylon generates data. That data feeds the dashboard. The dashboard makes the data useful. Users stay in our ecosystem.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      OPENAGENTS PRODUCT SUITE                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌───────────────────┐     ┌───────────────────┐     ┌───────────────────┐  │
│  │     Autopilot     │────▶│    RLM Swarm      │────▶│   RLM Dashboard   │  │
│  │  (coding agent)   │     │  (distributed)    │     │  (visualization)  │  │
│  └───────────────────┘     └───────────────────┘     └───────────────────┘  │
│           │                         │                         │              │
│           │                         │                         │              │
│  ┌────────▼────────┐       ┌────────▼────────┐       ┌────────▼────────┐   │
│  │  Codex SDK     │       │  Pylon/Nexus    │       │  openagents.com │   │
│  │  (sessions)     │       │  (infra)        │       │  (web UI)       │   │
│  └─────────────────┘       └─────────────────┘       └─────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## What W&B Gets Right (and Where We Improve)

### W&B's Core Value

| Capability | What it does | Why researchers love it |
|------------|--------------|-------------------------|
| Run ledger | Metrics + hyperparams + code version | "I ran stuff" → "we can all see what happened" |
| Comparison | Tables/plots across runs | Side-by-side experiments |
| Team memory | Reports, dashboards, links in PRs | Institutional knowledge |
| Artifacts | Model checkpoints, datasets | Reproducibility |

### Where W&B Falls Short

| Pain Point | User Complaint | Our Solution |
|------------|----------------|--------------|
| **Reliability** | Network errors crash training jobs | Local-first: SQLite first, sync later |
| **Pricing** | Per-user/usage scaling friction | Pay with sats, usage-based micropayments |
| **Bloat** | "I just want tracking" | Minimal core, optional features |
| **Lock-in** | Migration is painful | Open format: folders + SQLite + JSON |

### Our Differentiation

| W&B | OpenAgents RLM Dashboard |
|-----|--------------------------|
| Training runs | Agent execution runs |
| GPU metrics | Swarm fanout / provider metrics |
| Model checkpoints | Trajectory logs + SpanRefs |
| Team dashboards | Public HUDs (opt-in) |
| Credit card billing | Lightning micropayments |

---

## What We Visualize

RLM execution is fundamentally different from model training. We're tracking:

### 1. Swarm Fanout Execution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RLM RUN: doc-analysis-2024-01-08                                   ◉ LIVE  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  QUERY: "Summarize the key architectural decisions in this codebase"        │
│                                                                              │
│  ┌─ FANOUT TIMELINE ────────────────────────────────────────────────────┐   │
│  │                                                                       │   │
│  │  t=0ms   ████ chunk-001 → provider-a (Apple FM)  ✓ 234ms   12 sats   │   │
│  │  t=0ms   ████ chunk-002 → provider-b (Ollama)    ✓ 456ms   8 sats    │   │
│  │  t=0ms   ████ chunk-003 → provider-c (swarm)     ✓ 312ms   15 sats   │   │
│  │  t=0ms   ████ chunk-004 → provider-a (Apple FM)  ✓ 198ms   12 sats   │   │
│  │  t=0ms   ████ chunk-005 → provider-d (swarm)     ⏱ timeout           │   │
│  │                                                                       │   │
│  │  t=500ms ████ aggregate → local (FM)             ✓ 89ms    0 sats    │   │
│  │                                                                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─ BUDGET ──────┐  ┌─ QUORUM ───────┐  ┌─ LATENCY ───────────────────┐    │
│  │ ████████░░░░░ │  │ 4/5 collected  │  │ p50: 256ms  p99: 456ms      │    │
│  │ 47/100 sats   │  │ 80% threshold  │  │ ▁▃▅▇█▅▃▁                    │    │
│  └───────────────┘  └────────────────┘  └─────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2. Recursive Depth Visualization

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RECURSION TREE                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  root-query ─────────────────────────────────────────────────────────────   │
│      │                                                                       │
│      ├── router (select fragments)                                          │
│      │      ├── chunk-001 ── extractor ── SpanRef[file.rs:10-50]           │
│      │      ├── chunk-002 ── extractor ── SpanRef[mod.rs:1-30]             │
│      │      └── chunk-003 ── extractor ── (no evidence)                     │
│      │                                                                       │
│      ├── reducer (aggregate)                                                │
│      │      └── combined evidence ── 3 SpanRefs                            │
│      │                                                                       │
│      └── verifier                                                           │
│             └── ✓ passed (0 missing citations)                              │
│                                                                              │
│  Depth: 2    Branches: 4    SpanRefs: 3    Cost: 47 sats                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3. Provider Performance Comparison

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROVIDER LEADERBOARD (last 24h)                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Provider          Latency    Success   Cost/1k    Jobs                     │
│  ─────────────────────────────────────────────────────────                  │
│  provider-a        ███░░  234ms   98.2%    12 sats   1,247                  │
│  provider-b        ████░  456ms   94.1%     8 sats   2,891                  │
│  provider-c        ██░░░  312ms   97.8%    15 sats     456                  │
│  local-fm          █░░░░   89ms  100.0%     0 sats     892                  │
│                                                                              │
│  [Filter: model] [Sort: latency ▼] [Time range: 24h]                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4. Experiment Comparison Table

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  EXPERIMENTS: doc-analysis                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Run ID         Method      Accuracy   F1     Latency   Cost    Tokens      │
│  ─────────────────────────────────────────────────────────────────────────  │
│  run-001        base        0.72       0.68   1.2s      0       12,456      │
│  run-002        rlm-1       0.84       0.81   3.4s      47      8,901       │
│  run-003        rlm-2       0.86       0.83   2.8s      52      7,234       │
│  run-004 ★      rlm-2+      0.89       0.87   2.1s      38      6,102       │
│                                                                              │
│  ▲ Best: run-004 (rlm-2+ with chunk-size=512, quorum=0.9)                   │
│                                                                              │
│  [Compare selected] [Export CSV] [Share link]                               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Model

### Run Record

Every RLM execution produces a `Run`:

```rust
pub struct Run {
    pub id: String,                    // Unique run ID
    pub created_at: DateTime<Utc>,
    pub status: RunStatus,             // Running, Completed, Failed

    // Configuration
    pub config: RunConfig,             // Method, hyperparams, policy
    pub git_sha: Option<String>,       // Code version
    pub dataset: Option<String>,       // Dataset ID if benchmark

    // Results
    pub metrics: HashMap<String, f64>, // accuracy, f1, latency, cost
    pub output: Option<String>,        // Final output

    // Provenance
    pub span_refs: Vec<SpanRef>,       // Evidence citations
    pub trace: Vec<TraceEvent>,        // Full execution trace

    // Economics
    pub cost_sats: u64,
    pub tokens_used: u64,
}

pub struct RunConfig {
    pub method: String,                // "base", "rlm", "summary-agent"
    pub model: String,                 // "codex-3", "llama-3"
    pub chunk_size: usize,
    pub max_depth: usize,
    pub quorum: f32,
    pub budget_sats: u64,
    pub verification: VerificationTier,
}
```

### Trace Events (from `frlm/src/trace.rs`)

Already implemented:
- `RunInit`, `RunDone` — Lifecycle
- `SubQuerySubmit`, `SubQueryReturn`, `SubQueryTimeout` — Fanout
- `BudgetReserve`, `BudgetSettle` — Economics
- `VerifyRedundant`, `VerifyObjective` — Verification
- `Aggregate` — Result combination

### SpanRefs (from `rlm/src/span.rs`)

Provenance tracking:
- File path, git SHA, line ranges
- Content hash for verification
- JSON-serializable for DSPy signatures

---

## Storage Architecture

### Local-First Design

```
~/.openagents/rlm/
├── runs/
│   ├── run-001/
│   │   ├── config.json      # RunConfig
│   │   ├── metrics.json     # Final metrics
│   │   ├── trace.jsonl      # TraceEvent stream
│   │   ├── output.txt       # Final output
│   │   └── artifacts/       # SpanRefs, intermediate results
│   └── run-002/
│       └── ...
├── experiments/
│   └── doc-analysis.json    # Experiment grouping
└── rlm.db                   # SQLite index
```

**Why SQLite first:**
- Runs never fail because sync failed
- Full offline capability
- Fast queries for comparison
- Easy backup (copy the file)

### Cloud Sync (Optional)

```
Local SQLite ──sync──▶ openagents.com/api/rlm/runs
                              │
                              ▼
                       D1 (Cloudflare)
                              │
                              ▼
                       Dashboard UI
```

Sync is:
- Opt-in per experiment
- Incremental (delta updates)
- Resumable (idempotent)
- Privacy-respecting (redact before sync)

---

## Integration Points

### Pylon Integration

Pylon already runs RLM queries:

```bash
pylon rlm "Summarize this codebase" --budget 100
```

**Current flow:**
1. Pylon chunks input
2. Fans out to swarm (NIP-90)
3. Collects results
4. Aggregates

**New flow with dashboard:**
1. Same as above
2. TraceEmitter logs to SQLite
3. Background sync to cloud (if enabled)
4. Dashboard shows real-time progress

### FRLM Conductor

`frlm::TraceEmitter` already emits structured events:

```rust
// Already exists in frlm/src/trace.rs
emitter.run_init("query", 5);
emitter.subquery_submit("q-1", "prompt", Some("frag-1"));
emitter.subquery_return("q-1", "result", 100, 10, true);
emitter.run_done("output", 3, 50);
```

We need:
1. SQLite sink for TraceEmitter
2. Web API for sync
3. Dashboard UI components

### bench-harness Integration (Archived)

`bench-harness` is archived out of the current workspace. Historically it defined:
- `TaskInstance` — Benchmark tasks
- `Method` — Solution methods
- `Trajectory` — Execution traces
- `Metric` — Evaluation metrics

Connect to dashboard:
1. ExperimentRunner logs runs to SQLite
2. Metrics flow to comparison table
3. Trajectories available for replay

---

## Dashboard UI (openagents.com)

### Routes

| Route | Purpose |
|-------|---------|
| `/rlm` | Dashboard home, recent runs |
| `/rlm/runs` | Run list with filters |
| `/rlm/runs/:id` | Single run detail |
| `/rlm/experiments` | Experiment groups |
| `/rlm/experiments/:id` | Comparison table |
| `/rlm/providers` | Provider leaderboard |

### Components (WGPUI)

Reuse existing components:
- `SignalMeter` — Budget, latency bars
- `Frame` — Sci-fi borders
- `VirtualList` — Scrolling run lists
- `StatusBadge` — Run status

New components needed:
- `FanoutTimeline` — Parallel execution visualization
- `RecursionTree` — Depth visualization
- `ComparisonTable` — Experiment comparison
- `ProviderLeaderboard` — Provider stats

### Real-Time Updates

WebSocket from Pylon → Dashboard:

```typescript
// Browser receives
{ type: "trace_event", run_id: "...", event: TraceEvent }

// Dashboard updates
- FanoutTimeline adds new lane
- Budget meter updates
- Latency distribution shifts
```

---

## Implementation Phases

### Phase 1: Local Storage (Week 1)

| Task | Files |
|------|-------|
| SQLite schema for runs | `crates/rlm/src/db/` |
| TraceEmitter SQLite sink | `crates/frlm/src/trace_db.rs` |
| CLI: `pylon rlm --log` | `crates/pylon/src/cli/rlm.rs` |
| CLI: `pylon rlm history` | Same |

### Phase 2: Web API (Week 2)

| Task | Files |
|------|-------|
| Sync endpoint | Archived (former `crates/web/worker/src/routes/rlm.rs`) |
| D1 schema | Archived (former `crates/web/worker/migrations/`) |
| Auth (link to Pylon identity) | Archived (former `crates/web/worker/src/auth/`) |

### Phase 3: Dashboard UI (Week 3)

| Task | Files |
|------|-------|
| Run list view | Archived (former `crates/web/client/src/views/rlm/`) |
| Run detail view | Same |
| FanoutTimeline component | `crates/wgpui/src/components/rlm/` |

### Phase 4: Experiment Comparison (Week 4)

| Task | Files |
|------|-------|
| Experiment grouping | `crates/rlm/src/experiment.rs` |
| Comparison table | `crates/wgpui/src/components/rlm/comparison.rs` |
| Export (CSV, JSON) | Archived (former `crates/web/worker/src/routes/export.rs`) |

---

## Differentiation from Existing Work

### vs. ml-inference-visualization.md

That doc covers **single-model inference**:
- Attention weights
- Token probabilities
- KV cache
- Layer activations

This doc covers **distributed RLM execution**:
- Swarm fanout
- Recursive depth
- Provider comparison
- Experiment tracking

Both use WGPUI, both use telemetry hooks, but different data and different visualizations.

### vs. W&B

| W&B | RLM Dashboard |
|-----|---------------|
| Training epochs | RLM iterations |
| GPU utilization | Provider latency |
| Loss curves | Accuracy/cost curves |
| Model artifacts | SpanRef provenance |
| Team workspaces | Public HUDs |

---

## Open Questions

1. **Pricing:** How do we charge for dashboard access? Per-run? Subscription? Free tier?

2. **Privacy:** What's the default for sync? Opt-in? What gets redacted?

3. **Sharing:** Public links? Embeddable reports? Export formats?

4. **Integration:** Should this be a separate product or baked into openagents.com?

5. **Mobile:** Do we need mobile-friendly views for monitoring runs?

---

## Summary

The RLM Dashboard is a W&B-style research workflow tool that:

1. **Tracks** every RLM execution with structured traces
2. **Compares** experiments with side-by-side tables
3. **Visualizes** swarm fanout and recursive depth
4. **Persists** locally first, syncs optionally
5. **Integrates** with Pylon, FRLM, and archived bench-harness flows

It's the "research taste test" that demonstrates we think like researchers—building the tool that makes experiments reproducible, comparable, and shareable.

**Next step:** Implement Phase 1 (SQLite storage) and prove the concept with a single run visualization.
