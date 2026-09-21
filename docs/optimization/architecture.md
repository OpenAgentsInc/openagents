# Architecture and opportunity map

Status: target architecture. [NIP-OPT](../../nips/openagents/NIP-OPT.md) defines
the corresponding records. [Implementation proposals](proposed-issues.md)
remain unfiled.

## Stable meaning and replaceable implementation

Use four distinct layers:

| Layer | Owns | Changes through |
| --- | --- | --- |
| Application contract | Task meaning, typed fields, required evidence, domain acceptance, and protected control constraints. | Explicit task/schema revision. |
| Implementation | Prompts, demonstrations, decision questions, generation settings, retrieval policies, inference strategy, supported internal topology, and model bindings. | A new candidate and immutable implementation. |
| Measurement and optimization | Dataset partitions, objectives, proposal/search procedure, whole-task evaluation, and confirmation. | A new frozen study and retained results. |
| Host and deployment | Grants, credentials, privacy, effect enforcement, reservations, build isolation, activation, and rollback. | Independently authorized host/operator policy. |

Code protects essential control flow: validate inputs, check authority, consume
an exact approval, dispatch, confirm effects, and retain unknown outcomes.
Code need not permanently freeze every relevance threshold or model-call
decomposition. A search may replace a bounded internal inference graph while
preserving those protected transitions. Failure to prove the declared
invariants refuses the candidate; a higher reward cannot waive them.

A study states which parts can change. Whole-program optimization is possible
only for admitted surfaces, with all candidates passing the same external
contract and independent checks. Source-editing candidates require isolated
builds and normal execution admission. Code that defines the benchmark,
permission checks, or acceptance gate is outside the candidate's writable set.

## Fit with programs and extensions

A NIP-PRG program remains the host-interpreted workflow. A semantic AI signature
describes one operation or the intended behavior of a larger composition.
An AI implementation binds that signature to an existing decision function,
program, or host-supported operation. An EXT package can distribute these
components with their immutable dependencies and provenance.

The host resolves a selected implementation once at admission. Optimizer
output cannot replace a dependency during a run. The lock includes prompt
and demonstration bytes, renderers, adapters, consuming policy, model target,
and inference settings as well as executable code. Installing those bytes
does not execute them.

A specialized decision function still uses the existing System One contract.
A generated answer or richer composition can use an admitted operation or
program instead. Do not add arbitrary generation fields to Noul/Choice/Score
or pretend every DSPy signature is a decision API call. Optimized selection
still proposes a program or operation; eligibility and admission remain
mechanical host checks.

## Map the TypeSafe opportunities to learnable behavior

The [founder's proposal](../coder/design/thoughts-on-a-typesafe-coding-agent.md)
remains the source of the coding opportunities. This table defines how to realize them, including the appendices.

| Opportunity | Stable requirement | Candidate implementation choices | Evidence for adoption |
| --- | --- | --- | --- |
| Basic classification and tool judgments | Typed result, explicit abstention, unchanged meaning. | Jev/Kev/Lev questions; examples; admitted alternative model-backed implementation. | Harmful error directions, calibration where used, and downstream task results. |
| Meta-attention and context pruning | Supply sufficient attributable evidence and mandatory instructions within disclosure limits. | Retrieval/reranking, joint selection, per-item decisions, context budget allocation, bounded expansion. | Retrieval recall versus selection recall, missed constraints, expansion, complete task cost and quality. |
| Explicit task state and topic transitions | Preserve user intent, sources, scope, and unresolved obligations. | State extraction, update proposals, topic clustering, summarization. | Lost or invented constraints, stale state, correction cost, and task completion. |
| Tool selection and progressive loading | Select only from mechanically eligible operations; validate actual arguments. | Descriptor wording, shortlisting, selection strategy, loading policy. | Missed needed tools, false activation, argument failures, and complete task overhead. |
| Skills and instructions | Mandatory precedence and scope are fixed. | Optional guidance retrieval and rendering, task-scoped examples. | Required instruction retention, irrelevant guidance cost, and downstream results. |
| Model routing and cache economics | Permitted recipients, budget ceilings, and known output contracts. | Routing, escalation, batching, generation settings, context rebuilding. | End-to-end latency/cost, actual cache observations, disclosure refusals, and quality. |
| Parallel agents and shared context | Dependency order, writer fencing, evidence versioning, and shared reservations. | Task decomposition, independence proposals, allocation, assimilation strategy. | Total work, contention, duplicate work, missed dependencies, and independent final checks. |
| Arbitrage and reusable micro-operations | Same semantic task and domain acceptance. | Direct generation versus specialized operation, smaller model, compiled composition. | Target-specific workload quality and full cost including preparation and escalation. |
| Context compression and operation outputs | Originals remain attributable and expandable; incompleteness stays visible. | Structured representations, summary strategies, parsing, compression selection. | Sufficiency failures, bytes saved, expansion cost, and final correctness. |
| Long history, hierarchy, and memory | Scoped retention, provenance, freshness, and bounded access. | Indexing, clustering, retrieval, representation granularity. | Recall and staleness across history size; no assumed logarithmic semantic recall. |
| Multimodal material | Typed resource identity, permitted disclosure, and supported media adapters. | Captioning/extraction, media-aware retrieval, model/strategy selection. | Domain-appropriate content and task checks; unsupported media remains refused. |
| Background processing and changing user needs | Explicit subscription scope, budget, freshness, and foreground authority. | Finding generation, ranking, explanation, timing recommendations. | Useful versus noisy/stale findings, interruption burden, spend, and acceptance. |
| Self-improvement | Frozen task and protected controls; separately admitted experiments. | Prompt/example search, strategy selection, internal composition, source edits, separately authorized weight training. | Exact candidate activation, partition integrity, fresh confirmation, and reversible adoption where supported. |

Deterministic fixes remain appropriate when the problem is a broken parser,
incorrect state transition, or missing precondition. Optimization is a way to
improve uncertain behavior, not a substitute for repairing known correctness
defects. A measured hand-authored implementation is a legitimate baseline or
winner. The architecture does not require a learned component to win.

## Two examples across domains

For coding, define “produce a cited explanation of the failing test from the
admitted evidence.” Define a simple retrieval and answer baseline. Compare
a typed selector, a joint selector, and bounded iterative retrieval against
the same cases. Preserve the original evidence, citation checks, and maximum
disclosure throughout. A successful explanation does not grant a code edit.

For document work, define “extract these fields with source references or
abstain.” Compare direct extraction, a two-stage pipeline, and example-tuned
generation. Keep schema checks, source attribution, and human review of any
subsequent record update outside the optimizer. Measure new documents from
held-out sources, not only reworded cases from documents used in search.

Both use the same signature, implementation, study, and report contracts.
The input schemas, adapters, and verifiers differ. Neither requires a
repository, and a coding score does not admit a document workflow.

## Runtime and protocol boundaries

Rust remains the product implementation language. Initial DSPy/GEPA integration
can run as pinned offline Python infrastructure that exchanges inert artifacts
with Rust runners and Gym. An external optimizer or inference service needs
explicit capability admission, disclosure, accounting, and supported output
contracts. Do not add a Python product runtime or couple the protocol to a private backend.

Nostr carries identity and records; the host materializes candidates and
enforces permissions; Gym and domain evaluators measure them. The client shows
the active version, study provenance, workload scope, unknowns, and an
understandable adoption result. It need not expose compiler internals in
ordinary task flows.
