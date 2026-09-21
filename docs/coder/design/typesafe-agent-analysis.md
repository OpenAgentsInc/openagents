# A TypeSafe-native Coder

Status: target architecture. Coder is the first specialization of
[general agent infrastructure](../../agents/README.md). The
[founder's proposal](thoughts-on-a-typesafe-coding-agent.md) identifies
opportunities for explicit state and economical semantic decisions. The
[AI programming design](../../optimization/README.md) defines how to express,
evaluate, and improve their implementations.

## The main opportunity

Organize the agent around attributable evidence and explicit task state.
Different operations should be able to ask different questions about the
same observations without inheriting one ever-growing generation transcript.
Context selection, routing, progressive tools, parallel tasks, and background
views are applications of this architecture.

Define each useful semantic operation by its purpose, inputs, outputs,
abstention, required evidence, and protected constraints. Its implementation
can use a typed judgment, generation, deterministic retrieval, or a supported
bounded composition. A fixed list of handcrafted Jev calls is not the
application's permanent abstraction.

DSPy and GEPA can help search model-facing instructions, examples, inference
strategies, parameters, and permitted internal decompositions. Gym and domain
evaluators measure whether a candidate improves the complete task. Rust owns
authority, disclosure, mandatory constraints, scheduling, budgets, effects,
and independent verification throughout.

The [complete opportunity map](../../optimization/architecture.md#map-the-typesafe-opportunities-to-learnable-behavior)
covers the proposal and its appendices. Benefits remain hypotheses until
measured on an appropriate workload.

## Evidence store and task frame

Capture repository state, user turns, tool outputs, diagnostics, and external
observations under stable identities. Record source versions, scope,
completeness, retention, and permitted recipients. Preserve original captures;
derived summaries, outlines, and facts reference their source evidence.

A task frame holds the objective, explicit corrections, binding constraints,
acceptance criteria, attempted approaches, unresolved questions, and relevant
artifact versions. Distinguish user instructions from extracted observations
and inferred subgoals. A model may propose an update; the host validates its
source and authority before adopting it.

Changed source state invalidates conclusions tied to its version. Incomplete
capture remains incomplete through every derivative. A local digest does not
establish that a live external service still has the observed value.

## Context as a reproducible artifact

A context request identifies the task, recipient, purpose, required evidence,
and bounds. Mechanical scope and disclosure filtering comes first. Mandatory
constraints are resolved independently of relevance. Retrieval proposes
candidate evidence; optional semantic selection chooses among it.

The resulting manifest names exact included representations, order, omissions,
coverage, and the implementation used to select them. Each model, delegate,
reviewer, reflector, and judge receives its own admitted view. Access granted
to one participant does not authorize another.

Define the semantic goal as sufficient evidence for the operation. Compare
per-item judgments, joint selection, retrieval, summaries, and bounded expansion
under that contract. Measure retrieval recall separately from selection recall.
A perfect selector cannot recover a source the retriever never offered.

Retain expansion to originals under scope and budget. A representation that
cannot preserve required evidence must refuse or use a declared fallback.
Fewer tokens do not establish a more correct answer.

## Native tools, programs, and extensions

Native operations provide bounded reading, search, version-anchored edits,
test execution, and retained diagnostics. Generation proposes open-ended
content; typed decisions help where uncertainty remains. Validate the actual
arguments, source preconditions, and authority before any effect.

A progressive catalog exposes small descriptors before loading complete schemas
and optional guidance. Mechanical eligibility removes unavailable or forbidden
operations. Semantic ranking can improve a shortlist without authorizing it.
Record omissions and support bounded expansion.

Programs compose typed operations with exact dependency pins and shared
reservations. Selection proposes a workflow or `none`. Explicit structured
requests need no redundant selection call. Wasm plugins transform data within
their admitted profiles; they do not gain ambient model or network access.

An AI implementation realizes a semantic signature through a decision function,
program, or operation. Search may replace a bounded internal inference graph,
but cannot remove approval consumption, effect checks, protected verification,
or the task's required output meaning. Every candidate is an immutable closure.

## Routing and cache economics

First determine permitted recipients and supported output contracts. Then
compare candidates using measured quality, expected task cost, remaining work,
and observed cache behavior. A matching context digest is not a provider
cache-hit guarantee.

Include context rebuilding, fallback, escalation, failed attempts, and review
in total cost and latency. Pin model targets and record observed identities.
An alias-only target cannot claim immutable weights or automatic calibration.

User-facing choices should describe quality, time, cost, and disclosure.
Do not require users to tune many unrelated probability thresholds. A
schema-compatible model replacement still needs scoped evaluation.

## Parallel and background work

Independent tasks share scoped immutable evidence. Each receives a task frame,
context manifest, declared resources, and narrowed budget. Known dependencies
and write conflicts are mechanical constraints; semantic judgments can only
address uncertainty that remains.

A parent assimilates relevant findings and checks final acceptance independently.
Successful children do not establish a successful parent. Exact task/source/
operation identity can permit declared reuse; semantic similarity alone cannot
prove equivalence. Independent evaluation repetitions must never be coalesced
as duplicate work.

Background explanations, reviews, indexes, and candidate studies require an
explicit plan, recipients, data rights, freshness, and resource allowance.
Findings are proposals with provenance and staleness, not changes to foreground
authority. Background learning cannot rewrite a running task or activate a
candidate.

## Permissions, privacy, and reliability

Separate selection from permission. A confident judgment cannot waive scope,
a budget, a source precondition, a protected check, or required approval.
Disclose to admitted destinations only, including fallbacks and reflectors.
Local inference for one step does not make the rest of a workflow local.

If a policy consumes probabilities, evaluate calibration on the relevant
workload and preserve abstention. Review records retain original answers,
review outputs, and the rule that selects what to consume. Model judgment
is evidence for a decision, not the authority to execute it.

Durable runs record intent before effects and observations afterward.
Cancellation and timeout preserve uncertain effects and spending. Reconcile
before retry; do not treat a lost response as nonexecution. Verification and
integration remain separate from execution completion.

## Learning and measured adoption

Freeze task semantics, protected constraints, data rights, partitions,
objective, grader, and acceptance before searching. Permit changes only in
declared surfaces. Materialize and verify the exact candidate before attributing
a score. Track proposal, reflection, student, judge, tool, build, storage, and
cleanup work.

Selection data is development evidence. Confirm a committed candidate under a
bounded protected allowance, then let independent operator policy determine
adoption. A study may find no gain or insufficient evidence. Publication and
installation do not activate a candidate, and active tasks retain their pins.

The [experiment lifecycle](../../optimization/experiments.md) and
[NIP-OPT](../../../nips/openagents/NIP-OPT.md) define these boundaries.
Prompt/example optimization and weight training require their own data rights;
retained user traces are not an implicit training set.

## Product experience and evaluation

Show task intent, evidence, omissions, active implementation, approvals, known
cost, and unresolved outcomes. Offer exact provenance in an inspection view.
Distinguish provisional output, completed work, verified acceptance, and
accepted integration. A relevance visualization does not measure internal
model attention.

Evaluate complete tasks: success, harmful errors, source coverage, false
exclusion, unnecessary activation, stale edits, escalation, latency, total
cost, and human correction effort. Use a workload capable of detecting the
intended gain and report uncertainty. Module scores guide development but
cannot replace domain outcomes.

The [roadmap](typesafe-agent-roadmap.md) and
[consolidated unfiled proposals](../../optimization/proposed-issues.md)
define delivery. A non-coding reference workflow must exercise the same
contracts without requiring repository, shell, or terminal state.
