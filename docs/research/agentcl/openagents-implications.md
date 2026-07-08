# AgentCL Implications for OpenAgents

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


## Why This Matters Here

OpenAgents already has several places where agent memory, task traces, study
packets, or workflow reuse could become product claims. AgentCL is a useful
warning label: "we stored prior work" is not the same as "we learned
continually."

The paper gives us a clean evaluation vocabulary:

- Did memory help on a later task with a known reusable dependency?
- Did the benefit persist after more tasks updated memory?
- Did memory remain harmless on unrelated or held-out tasks?

Those should be separate claims in docs, reports, and product promises.

## Evaluation Pattern to Borrow

For any OpenAgents memory lane, create four views of the same candidate system:

1. Memoryless baseline.
2. First pass over a relationship-controlled stream.
3. Frozen-memory second pass over the same stream.
4. Held-out stream from a different source or task family.

Then report:

- Plasticity Gain: first pass minus memoryless baseline.
- Stability Gain: frozen second pass minus first pass.
- Generalization Gain: held-out performance minus memoryless baseline.

Do not collapse these into one "memory improved accuracy" number.

## Candidate OpenAgents Streams

Coding and repo work:

- Source tasks: small bugs, lint fixes, docs repairs, fixture tests, or focused
  package changes.
- Complex tasks: later issues that compose those repairs, such as a feature that
  requires the same helper, schema, test harness, or command pattern.
- Naive stream: unrelated issues from the same repo area.
- Held-out stream: another package or app surface with the same broad tooling
  but no explicit source-target relationship.

StudyBench-style repository understanding:

- Source tasks: questions about one module, invariant, schema, or route.
- Complex tasks: questions requiring composition across those source modules.
- Held-out tasks: private validation questions whose gold answers and rubrics
  are never injected into study packets.

Pylon/Khala operational traces:

- Source tasks: single-account setup, heartbeat, assignment closeout, trace
  lookup, or token-row verification.
- Complex tasks: full own-capacity burn flow requiring those steps in sequence.
- Naive stream: adjacent operational tasks that share tooling but should not
  bias the agent toward incorrect assumptions.

## Memory Record Shape

AgentCL's MemProbe split maps cleanly onto OpenAgents trace and memory records.
For every reusable memory entry, keep typed fields like:

- `source_task_ref`
- `stream_ref`
- `relationship_ref`
- `memory_kind`: `interaction`, `insight`, or `skill`
- `consolidation_status`: `valid`, `uncertain`, or `invalid`
- `validation_ref`
- `retrieval_score`
- `reuse_mode`: `reference_only`

This fits the workspace invariant against ad hoc routing: select memories with
a typed semantic selector or modeled query planner, then apply deterministic
parsing only to bounded fields like IDs, enum values, and exact refs.

## Product-Claim Boundary

Suggested claim language discipline:

- "Improves compositional reuse" requires positive Plasticity Gain on a
  relationship-controlled stream.
- "Retains useful experience" requires non-negative or positive Stability Gain
  after memory is frozen.
- "Generalizes" requires held-out Generalization Gain, not only in-stream gains.
- "Safe memory" requires naive-stream or held-out evidence that memory does not
  degrade unrelated tasks.

This should feed `docs/promises/` before any broad product copy claims that
OpenAgents agents continually learn from prior work.

## Design Warnings

Retrieval is not authority.

Memories should be rendered as contextual evidence with provenance and caveats.
The solver still has to ground the answer in the current task, current repo
state, and current verifier.

High semantic similarity can be harmful.

The paper's negative case shows a memory that was topically close but
semantically wrong for the target. OpenAgents memory should prefer explicit
relationship metadata where available, and it should log when a solver rejects
retrieved memory.

Naive streams are not useless.

They are poor proof of learning, but good proof of harmlessness. Keep them in
the test set specifically to measure interference.

Held-out rows are necessary.

Any StudyBench, repo-memory, or Pylon-workflow benchmark should include private
or at least isolated held-out rows that do not feed memory construction.

## Concrete Follow-Ups

1. Add an `agentcl_eval.v0` draft contract for memory experiments: baseline,
   first pass, frozen second pass, held-out pass, and the three gain metrics.
2. Build a tiny public fixture stream over `openagents` docs where simple
   source tasks compose into later complex tasks.
3. Pair that with a private held-out stream in the workspace repo, referenced
   by checksum only from public docs.
4. Extend trace summaries so a memory entry can say whether it was used,
   ignored, contradicted, or harmful.
5. Keep product claims narrow until the same system passes compositional reuse,
   stability, and held-out harmlessness checks.
