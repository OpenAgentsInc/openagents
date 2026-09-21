# Roadmap for a TypeSafe-native Coder

Status: proposed delivery order, 2026-09-21. This roadmap applies the
[founder's document](thoughts-on-a-typesafe-coding-agent.md) and the
[architecture analysis](typesafe-agent-analysis.md) to the existing Coder
consumer work. It supersedes the earlier four-release ordering in the
[consumer contract](coder-as-decision-router-consumer.md). It does not
change the status of an issue or claim these features are implemented.

Use [project 16](https://github.com/orgs/OpenAgentsInc/projects/16) for Coder
delivery and [project 15](https://github.com/orgs/OpenAgentsInc/projects/15/views/1)
for shared Decision Router capabilities. The
[dated snapshot](2026-09-21-project-roadmap-snapshot.md) records every issue
on those boards, current discrepancies, and recent implementation slices.
Retain [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501) as
the consumer tracker and [#9481](https://github.com/OpenAgentsInc/openagents/issues/9481)
as the service tracker.

## Delivery strategy

Build one complete repository workflow around reusable evidence before
expanding the agent's breadth. Implement the initial store and deterministic
context builder while the shared decision client is being unified. Add a
small typed relevance function, inspect its context selection in the
terminal, and compare the resulting task outcomes. This produces a useful
product increment without waiting for every commercial or transport feature.

The target is explicit semantic decisions throughout the workflow, each
justified by its result. Deterministic parsing, source freshness, path
conflicts, authority, limits, and process supervision stay in Rust. Do not
replace those operations with a question, or turn retired questions back on
to make the agent appear more TypeSafe-driven.

The proposed phases are ordered by dependency, not promised dates. An issue
may span phases; delivering one slice does not complete its full acceptance
contract. The local proposal IDs below are planning labels, not filed GitHub
issues.

## Phase 0: establish a reliable consumer baseline

**Outcome:** every existing decision site has an explicit identity and
failure policy, and a useful baseline workflow can be measured end to end.

Owners: [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502),
[#9503](https://github.com/OpenAgentsInc/openagents/issues/9503),
[#9505](https://github.com/OpenAgentsInc/openagents/issues/9505), and a thin
slice of [#9506](https://github.com/OpenAgentsInc/openagents/issues/9506).

1. Unify `Agent` and `Runtime` calls behind the shared Rust decision client.
   Make missing, disabled, invalid, and unsupported configuration distinct.
   Integrate the shipped direct-loopback configuration; retain explicit
   hosted and own-provider profiles. Relay decisions follow their transport
   implementation, not the existing generation relay path.
2. Register the current decision functions with state/question/policy
   versions, allowed model artifacts, input limits, and consuming outcomes.
   Fix or rename the weaker `requires_calibration` check. Record `none`,
   missing answers, refusals, and optional degradation distinctly.
3. Join session/turn/function IDs to request/attempt IDs and available
   execution receipts. Keep decision, generation, and executor costs
   separate. Unknown provider or subscription costs remain unknown.
4. Retain a small development task set with repository revisions, required
   evidence, expected artifacts, and independent checks. Include repository
   questions, failing tests, multi-file edits, and irrelevant/no-op requests.
   Record the unchanged agent and a deterministic retrieval baseline.

**Completion evidence:** terminal/headless parity on existing decisions;
configuration failures are visible; local-only profiles make no hidden
hosted calls; receipt absence is visible; the task set records complete
outcomes and current shell-bound failures. Do not require a universal
positive model improvement to record an honest baseline.

**Code boundary:** `crates/coder` agent/runtime configuration and calls,
`crates/jev`, existing receipt types, ATIF references, and consumer fixtures.
Reuse shipped gateway/auth/CLI foundations rather than rebuilding them.

## Phase 1: preserve evidence and answer repository questions

**Outcome:** Coder can collect an observation once, select a bounded view,
answer a repository question, and show where the answer came from.

Owners: [#9513](https://github.com/OpenAgentsInc/openagents/issues/9513),
[#9505](https://github.com/OpenAgentsInc/openagents/issues/9505),
[#9506](https://github.com/OpenAgentsInc/openagents/issues/9506).
Proposals: `CTX-1`, `CTX-2`, and `UI-1` below.

1. Add Rust types for evidence items, immutable snapshots, source/span
   references, derived artifacts, capture limits, and task frames. Start
   with repository files, user constraints, command results, and explicit
   summaries. Keep storage local and bounded; define retention and index
   rebuilding before adding persistent cross-session retrieval.
2. Build deterministic candidate retrieval from paths, symbols, exact
   search, and recent task observations. Preserve related evidence bundles.
   Record unavailable, truncated, and excluded candidates separately.
3. Implement a deterministic context builder that always includes binding
   instructions and required evidence. Produce a context manifest with
   item versions, selected representations, omissions, and coverage.
4. Add one bounded relevance function over those candidates. Compare Noul
   filtering or Score ranking where appropriate; use Choice only when a
   single alternative is intended. Keep the deterministic path as the
   baseline and fallback allowed by the function's policy.
5. Connect `answer-question` and ordinary repository answers to the same
   evidence/context path. Show source links and a compact evidence summary
   in the terminal, with identical structured references in headless output.

**Completion evidence:** a needed file absent from the shortlist is counted
as a retrieval miss; missing decision results never become silent
exclusions; editing a source invalidates derived context; a restarted
read-only investigation can retrieve still-valid observations; summaries
expand to retained source. Report answer quality, evidence recall, latency,
disclosure, and full cost against deterministic retrieval.

**Dependency limit:** small bounded experiments can use the native
`/v1/systemone` contract. The partial
[`/v1/classify` route](../decision-models/classification-http.md) in
[#9482](https://github.com/OpenAgentsInc/openagents/issues/9482) now supports
ordered Choice, multi-label Noul, and named dimensions with versioned policy;
it forwards serial native requests. Binary helpers and Score ranking remain
open. Packing in [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483)
enables scale and need not block a small first slice. Preserve the shared
service semantics when using either entry point.

## Phase 2: close the native coding loop

**Outcome:** the evidence path supports a useful code repair from failure
to independently checked artifact, with generation consuming a task-specific
context rather than an undifferentiated transcript.

Owners: [#9513](https://github.com/OpenAgentsInc/openagents/issues/9513),
[#9509](https://github.com/OpenAgentsInc/openagents/issues/9509),
[#9503](https://github.com/OpenAgentsInc/openagents/issues/9503).
Proposals: `CTX-2`, `OPS-1`.

1. Add bounded native read, search, version-anchored edit, and test operations.
   Keep the ordinary shell path available under its existing authority.
   Use actual candidates for paths and anchors, schema-check proposals, and
   reject stale edits before writing.
2. Retain bounded command-output artifacts and extract useful diagnostics
   from them, including errors after a long compiler preamble. Selection
   must point to exact source spans and state when capture itself truncated.
3. Define an execution policy suitable for coding tasks. The current
   15-second command deadline, three rounds, and small output heads are
   material limits. Choose workload-specific time/output/attempt budgets
   through the shared supervisor; do not remove hard bounds to make a demo
   pass. Read the [subprocess contract](subprocesses.md) before changing it.
4. Give the generator a context manifest and structured task frame. Retain
   stable instructions and useful recent exchange where appropriate. Rebuild
   views when the task changes, an earlier approach fails, or evidence drifts.
5. Connect the shipped artifact verifier and typed `run-suite` path to a
   reviewed real suite adapter. Finish structured findings for
   `review-changes`. Keep semantic review alongside mechanical results.

**Completion evidence:** repair a pinned failing-test task and a multi-file
task; show the selected evidence, attempted patch, independent test result,
and retained artifact. Include stale anchors, missing diagnostics, a failing
check, and unavailable judgments. Compare success and repair effort with
the same generator and deterministic context. A delegate's self-report is
not the final verification result.

This phase is the first strong demonstration of the founder's architecture.
It does not require automatic merging or an unattended backlog.

## Phase 3: make context and operation selection economical

**Outcome:** Coder can use a large capability catalog and different
generation profiles without consuming their savings in irrelevant context
or repeated cache loading.

Owners: [#9503](https://github.com/OpenAgentsInc/openagents/issues/9503),
[#9513](https://github.com/OpenAgentsInc/openagents/issues/9513),
[#9512](https://github.com/OpenAgentsInc/openagents/issues/9512).
Proposals: `CAT-1`, `ROUTE-1`.

- Add compact operation and optional-skill descriptors. Filter for installed,
  authorized, compatible capabilities, retrieve likely candidates, and load
  complete schemas only when needed. Track activation lifetime and invalidation.
- Resolve mandatory instructions by scope and precedence before relevance
  selection. Give optional structured skills typed inputs/outputs and bounded
  hooks. Discovery, import, activation, and execution remain separate host
  operations with the authority already supplied by the session.
- Add hierarchical retrieval with multiple candidate branches and a bounded
  fallback. Compare it with flat search on cross-cutting repository tasks.
- Record cold/warm input usage and available cache evidence. Compare stable
  prefixes plus incremental context with full rebuilds; include summaries
  and decision calls in the cost.
- Offer operator-pinned generation profiles first. Add automatic routing only
  for task families with measured quality and full-task cost benefits, including
  return-to-strong-model failures. Keep decision backend, generator, and
  executor selection as separate policies.

**Completion evidence:** increasing catalog size does not hide the needed
operation; irrelevant optional guidance leaves the prompt; binding
instructions remain; cost claims use observed usage and declared unknowns;
switching models is compared with staying on the original one. Retain the
simpler policy where routing fails to improve the workload.

Remote discovery and package publication depend on the corresponding service
work. A local catalog can deliver progressive context before an MCP server,
public directory, or portable registry is complete.

## Phase 4: share context across parallel and background work

**Outcome:** independent tasks and one useful background feature reuse the
same evidence without losing foreground responsiveness or artifact identity.

Owners: [#9508](https://github.com/OpenAgentsInc/openagents/issues/9508),
[#9514](https://github.com/OpenAgentsInc/openagents/issues/9514),
[#9506](https://github.com/OpenAgentsInc/openagents/issues/9506),
[#9509](https://github.com/OpenAgentsInc/openagents/issues/9509).
Proposals: `SHARE-1`, `BG-1`.

1. Dispatch task-specific context manifests over pinned snapshots. Extend
   prepared read/write footprints and existing conflict accounting with
   references to the evidence each task actually used.
2. Return structured findings, patches/artifacts, verification, and unresolved
   questions. Retrieve the needed child results instead of merging all text.
   Recheck stale bases and conflicts before accepting an artifact.
3. Add duplicate-task suggestions over objective, base, inputs, and acceptance.
   Exact identity can reuse a valid result; uncertain semantic similarity
   cannot silently discard requested work.
4. Start one background diff explanation from existing observations. Add
   debounce, cancellation, supersession, a separate resource allowance, and
   priority below interactive work. Key findings to the revision they describe.
5. Extend to review and test proposals only when they improve outcomes.
   Candidate tests are artifacts to inspect, not arbitrary commands to run.

**Completion evidence:** shared reads reuse one valid observation; conflicting
writes never run as independent work; stale findings are marked; a new user
turn preempts expendable background work; cancelled work preserves known
costs and partial artifacts; increased concurrency improves complete-task
latency without increasing integration failures. Do not infer a larger local
Devin capacity from synthetic scheduling timings.

## Phase 5: durable programs and a reusable ecosystem

**Outcome:** useful measured workflows survive interruption, compose with
bounded effects, and can be distributed as inspectable packages.

Owners: [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510),
[#9511](https://github.com/OpenAgentsInc/openagents/issues/9511),
[#9512](https://github.com/OpenAgentsInc/openagents/issues/9512),
[#9514](https://github.com/OpenAgentsInc/openagents/issues/9514).

- Extend the shipped durable claim ledger into program/step/task/attempt
  recovery, with pinned evidence and context references. Reconcile ambiguous
  effects and reservations before dispatch; do not replay uncertain writes.
- Account for the whole program's decisions, reviews, generation, delegates,
  background work, time, and known money. Integrate service money enforcement
  when available; an accounting library alone does not enforce a Coder budget.
- Implement typed parent/child inputs and outputs, bounded depth and call
  counts, cancellation propagation, and narrowing authority. Wasm execution
  is a separate extension and need not block composition.
- Package programs, questions, policies, and optional skills with pinned
  references, compatibility, provenance, import/update/rollback, and offline
  resolution. Keep host paths and credentials out of portable packages.
- Publish repeatable repository-answer, code-repair, and prepared-backlog
  examples through [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475)
  and [#9496](https://github.com/OpenAgentsInc/openagents/issues/9496).

**Completion evidence:** restart at each meaningful transition, retain
unknown effects, reuse only valid context, prevent duplicate dispatch, and
reconcile cancellations and spend. Demonstrate the same program through
terminal and headless interfaces. Full hosted multi-user release also needs
the relevant service account, quota, money, and operations capabilities;
local dogfooding does not require waiting for commercial plans or a dashboard.

## Proposed backlog additions

These are suggested sub-issues or explicit acceptance additions under the
existing owners. They are not newly created GitHub issues. File narrowly
scoped implementation work as capacity opens; keep the existing trackers.

| Proposal | Scope and proposed implementation location | Existing owner | Done when |
| --- | --- | --- | --- |
| `CTX-1` Evidence store and task frames | New evidence/task modules in `crates/coder`; additive ATIF references; local bounded storage | #9513, #9505 | Sources and derived items have stable identity, scope, retention, and invalidation; corrected intent survives context rebuilding |
| `CTX-2` Context requests and manifests | New context module consumed by `agent`, `runtime`, and generation | #9513, #9503 | Required instructions survive selection; omissions and unknowns are recorded; the same input can be inspected offline |
| `OPS-1` Native coding operations | Rust host adapters using `supervise`, anchored edits, retained diagnostic artifacts, independent suite adapter | #9509, #9513 | A measured repair completes with source-linked context and independent artifact evidence under suitable workload bounds |
| `CAT-1` Progressive tools and instructions | Capability/program/skill descriptors and host-resolved instruction scope | #9512, #9503 | Large catalogs load only needed optional material without hiding required operations or weakening instructions |
| `ROUTE-1` Context-aware generation policy | Generation profiles, context reuse accounting, task-family routing evaluations | #9503, #9505 | Full-task benefit survives cache-transfer and failed-escalation costs on held-out tasks |
| `SHARE-1` Shared evidence for delegated tasks | Context bindings and structured results in `delegate`/`runtime`; scheduler conflict integration | #9508, #9514 | Independent tasks reuse observations, return attributable findings, and reject stale integration |
| `BG-1` Background evidence consumers | Low-priority controller work and one revision-bound diff explanation | #9514, #9506 | Foreground latency, cancellation, disclosure, and total cost remain visible and bounded |
| `UI-1` Evidence and task views | Extend `coder-terminal` frame/rail/intensity and shared Coder events | #9506, #9505 | A developer can inspect selected/omitted evidence, verification, and stale background findings in terminal and headless mode |

The model-function registry in #9503 should cover each newly introduced
function rather than creating another generic decision framework. Candidate
functions are evidence relevance, operation relevance, summary sufficiency,
task duplicate suggestion, and semantic review of remaining requirements.
Introduce one at a time against a deterministic baseline. Executor selection
and generation routing need separate outcome data.

## Model rollout and evaluation

Use Jev as a hosted reference where permitted, with an explicit model
identity. Evaluate Kev and Lev per function, preserving refusals and input
limits in the denominator. The existing Kev shell-outcome result is a
candidate lead; it does not admit Kev for program selection. Lev's existing
family-specific calibration does not transfer to repository relevance.
Optional workload training belongs under
[#9472](https://github.com/OpenAgentsInc/openagents/issues/9472) and admission
under [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473), after
the new task data shows a useful gap.

| Comparison | Hold fixed | Measure |
| --- | --- | --- |
| Deterministic retrieval versus typed reranking | Repository/task set, candidates, generator, execution policy | Candidate recall, selected-evidence recall, task success, added latency and cost |
| Transcript-oriented versus task-specific context | Task set, generator, available observations | Missing constraints, diagnostic coverage, stale claims, repair attempts, disclosed tokens |
| Fixed generator versus routed generator | Eligible providers, task quality criteria, budget | Success, cold/warm usage, failed escalation, full-task time and money |
| Isolated versus shared task context | Task graph, executor capacity, artifact checks | Duplicate reads, context preparation, conflict/staleness rate, integration effort |
| Foreground only versus background assistance | Foreground tasks and resource ceiling | Useful findings, stale/noisy findings, interactive latency, additional disclosure and spend |

Use development data to choose questions and policies, then held-out tasks
to evaluate the selected candidate. Label missing required evidence and
harmful error directions, not only average accuracy. Record complete
coverage, environment, task/base digests, model artifacts, context policy,
attempts, and unknown costs. Do not reuse a spent locked partition to tune
the next policy. No live paid runs are required to land this documentation.

## What to defer

Image judgments, confidential hosted inference, arbitrary model-controlled
state programs, line-level attention visualizations, quizzes, and automatic
model training can follow evidence of demand. Fully automatic integration
across arbitrary backlogs is also a later claim than the prepared-work
supervisor currently supports. A marketplace and subscription plans help
distribution but do not prove that typed context improves coding.

The next implementation increment should combine Phase 0's explicit client
outcomes with Phase 1's evidence/context path and an inspectable repository
answer. Then invest in the native repair loop. That order puts the founder's
main hypothesis in front of real coding tasks early, while making later
parallelism, background work, and routing reuse the same foundation.
