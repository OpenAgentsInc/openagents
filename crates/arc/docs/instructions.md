Below is a **condensed implementation guide** designed specifically for coding agents.
It extracts only the **actionable implementation requirements** from the ARC and Psionic specs so agents can start building without parsing the full documentation.

This is intentionally **procedural and minimal**, not explanatory.

---

# ARC Solver Implementation Guide (Coding Agents)

This guide describes the **minimum implementation order and contracts** for building the ARC solver stack.

Agents must follow the order strictly.

Do **not skip phases**.

---

# 0. Repository Structure

Create the following crate layout.

```
crates/
  arc/
    arc-core/
    arc-datasets/
    arc-engine/
    arc-client/
    arc-benchmark/
    arc-solvers/
    arc-ml/
```

Ownership rules:

| crate         | purpose                               |
| ------------- | ------------------------------------- |
| arc-core      | ARC domain types and canonicalization |
| arc-datasets  | dataset loading and normalization     |
| arc-engine    | deterministic task execution          |
| arc-client    | remote/local runner                   |
| arc-benchmark | scoring and run truth                 |
| arc-solvers   | solver implementation                 |
| arc-ml        | model integration                     |

No crate may violate this ownership.

---

# 1. Core Domain Types (`arc-core`)

Implement the canonical ARC types.

### ArcGrid

```rust
pub struct ArcGrid {
    pub height: u8,
    pub width: u8,
    pub cells: Vec<u8>,
}
```

Constraints:

```
height ∈ 1..=30
width ∈ 1..=30
cells length = height * width
color values ∈ 0..=9
```

Memory layout:

```
row-major

index = row * width + column
```

---

### ArcExample

```rust
pub struct ArcExample {
    pub input: ArcGrid,
    pub output: ArcGrid,
}
```

---

### ArcTask

```rust
pub struct ArcTask {
    pub id: TaskId,
    pub train: Vec<ArcExample>,
    pub test: Vec<ArcGrid>,
}
```

---

# 2. Canonicalization (`arc-core`)

Implement deterministic object extraction.

Objects must include:

```
ArcObject {
  id
  bounding box
  color histogram
  mask
}
```

Object ordering must be deterministic using:

```
1. top-left position
2. area
3. color histogram
4. scan order
```

---

# 3. Candidate Representation (`arc-solvers`)

Implement solver candidate types.

### HypothesisKind

```
Program
DirectGrid
InteractivePlan
```

---

### Hypothesis

```
Hypothesis {
  id
  kind
  program_ast?
  output_grid?
  action_plan?
}
```

Hypotheses must be **immutable once verified**.

---

### CandidateIdentity

Used for deduplication.

Canonical signature:

```
hash(
  hypothesis_kind
  normalized_program_ast
  normalized_output_grid
  normalized_action_plan
)
```

Equivalent signatures must be deduplicated.

---

# 4. Budget System

Implement `TaskBudget`.

```
TaskBudget {
  max_candidates
  max_program_exec
  max_model_calls
  max_wall_time_ms
}
```

Budget rules:

```
budget must be checked before work
budget counters never decrease
run must refuse if exceeding limits
```

Budget usage tracked with:

```
BudgetCounterDelta
```

---

# 5. Solver Lanes

Implement lanes in `arc-solvers`.

Each lane must implement:

```
trait Lane {
    fn propose(&self, state) -> LaneProposalBatch
}
```

---

### LaneProposalBatch

```
LaneProposalBatch {
  lane_name
  proposals
  status
}
```

Status values:

```
Proposed
Empty
Refused
BudgetExhausted
```

---

# 6. Verifier

Verifier evaluates hypotheses.

Inputs:

```
ArcTask
Hypothesis
```

Outputs:

```
VerificationReport {
  verifier_pass
  exact_fit
  score_metrics
}
```

Verifier must be deterministic.

---

# 7. Arbiter

The arbiter ranks verified hypotheses.

Ranking criteria:

1. exact train fit
2. MDL simplicity
3. program complexity
4. stability under augmentation

Output:

```
ArbiterDecision {
  selected_candidate
  ranking
}
```

---

# 8. Solver Pipeline

Solver execution pipeline:

```
lane propose
→ deduplicate
→ verify
→ rank
→ select
```

Repeat until:

```
solution found
or
budget exhausted
```

---

# 9. DSL Interpreter

DSL Tier A operations only.

Required primitives:

```
select objects
crop region
translate
rotate
reflect
recolor
connected components
map/filter over objects
grid composition
conditionals
```

DSL programs must enforce:

```
grid bounds
max grid size
```

---

# 10. Trace Bundles

Every solver run must produce a trace bundle.

Required contents:

```
seed bundle
candidate proposals
verification results
arbiter ranking
budget usage
final result
```

Trace bundles must allow full replay.

---

# 11. Benchmark Integration (`arc-benchmark`)

`arc-benchmark` owns scoring truth.

Scoring includes:

```
exact-match scoring
RHAE scoring (interactive tasks)
attempt counting
```

Rules:

```
score logic must not exist outside arc-benchmark
```

---

# 12. Engine (`arc-engine`)

The engine performs deterministic task execution.

Responsibilities:

```
apply actions
step environment
return next state
record trajectory
```

Engine must be deterministic under seed.

---

# 13. Client (`arc-client`)

Responsibilities:

```
remote server interaction
session handling
retry logic
recording ingestion
```

The client must not compute scores.

---

# 14. Dataset Loader (`arc-datasets`)

Implement dataset loaders for:

```
ARC-AGI-2
ARC-AGI-3
```

Tasks must be normalized to `ArcTask`.

---

# 15. Determinism Rules

All components must be deterministic under:

```
(task_id, solver_manifest_digest, seed_bundle, budget)
```

Parallelism is allowed but final outputs must match replay.

---

# 16. v1 Completion Criteria

ARC v1 requires:

```
arc-core implemented
arc-engine deterministic
arc-benchmark scoring parity
arc-solvers symbolic lane working
trace bundles produced
```

v1 explicitly excludes:

```
large model solvers
HRM implementations
distributed training
advanced learned search
```

---

# 17. Implementation Order

Agents must implement crates in this order.

### Phase 1

```
arc-core
arc-datasets
```

---

### Phase 2

```
arc-engine
arc-client
```

---

### Phase 3

```
arc-benchmark
```

---

### Phase 4

```
arc-solvers (symbolic lane)
```

---

### Phase 5

```
arc-ml integration
```

---

# 18. Acceptance Checks

Each phase must pass these checks.

### Core

```
grid canonicalization deterministic
dataset loads correctly
```

### Engine

```
replay identical under seed
```

### Benchmark

```
score parity with reference scripts
```

### Solver

```
trace bundle reproducible
candidate dedup working
```

---

# Final Rule

Coding agents must **not extend DSL scope or solver architecture beyond this guide** unless the ARC spec is updated.

This prevents architectural drift.
