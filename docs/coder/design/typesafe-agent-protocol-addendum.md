# TypeSafe agent: protocol, host, and client responsibilities

Status: target protocol, host, and client responsibilities. Coding is the first
specialization of general agent infrastructure. The [source proposal](thoughts-on-a-typesafe-coding-agent.md)
identifies opportunities; the [architecture](../../optimization/README.md)
defines how semantic programming and bounded learning realize them.

## Semantic programming and optimization responsibilities

[NIP-OPT](../../../nips/openagents/NIP-OPT.md) defines task signatures,
immutable implementations, search/data plans, candidates, materialization,
trials, accounting, and study results. EVAL supplies measurement and admission;
EXT distributes eligible components; CAP/PRG/CJ/RUN/CTX/POL/COORD preserve the
same authority, evidence, execution, and coordination boundaries.

Nostr exchanges these contracts and attributable records. Hosts isolate
proposers and graders, enforce data rights, load the exact candidate, and
account for every model/tool call. DSPy/GEPA bridges provide supported authoring
and search; Gym and domain evaluators measure the complete task. Clients show
the active pin, evidence scope, limitations, and adoption outcome.

The [opportunity map](../../optimization/architecture.md#map-the-typesafe-opportunities-to-learnable-behavior)
defines what can be learned for every recommendation. Protected authority,
privacy, and verification are not optimization parameters. The
[proposed-issues document](../../optimization/proposed-issues.md) is the single
unfiled backlog for full integration.

## Recommendation-by-recommendation coverage

This is a coverage map, not a claim that every speculation deserves a default
feature. “Host” includes Coder, a trusted execution worker, or the configured
coordinator. A relay stores and routes signed envelopes under explicit policy.

| Source recommendation | Nostr contract | What hosts and clients must implement |
| --- | --- | --- |
| Reuse the best generation models and useful UI components | CAP binding identity; POL recipient/routing records; CJ transport. | Provider adapters, credentials, compatible model input/output, and native terminal/headless presentation. Model/API reuse does not require protocol-level UI or provider auth. |
| Design beyond one cached transcript | CTX task frames, snapshots, views, and retained source references. | Evidence storage, collection bounds, dependency invalidation, and a context builder shared by every decision/generation path. |
| Make model switching cost-aware | POL route estimates and actual usage; EVAL matched complete-task comparisons. | Current configured prices, observed cache billing, expected remaining work, escalation accounting, and provider-specific prefix reuse. Never hardcode the post's old prices. |
| Avoid a giant fixed tool catalog | EXT small operation descriptors and pinned full schemas; CAP admissibility. | Inert indexing, mechanical filtering, bounded shortlist retrieval, optional semantic selection, and schema/manual loading only when needed. |
| Replace universal compaction with query-aware views | CTX representation modes, source anchors, omissions, and expansion. | Preserve bounded originals, select coherent evidence bundles, generate or extract representations, and refuse when mandatory context cannot fit. |
| Prepare and assimilate subagents cheaply | CTX recipient-specific contexts; COORD tasks/findings; CJ execution; RUN provenance. | Snapshot sharing, scoped materialization, bounded execution, evidence import, independent verification, and integration. |
| Restart using relevant history | CTX frames/history indexes; RUN replay and unresolved effects. | Rebuild current constraints and retrieve fresh relevant evidence; reconcile work before redispatch. Restarting a UI must not reset budgets or silently lose user corrections. |
| Include many batteries without permanent prompt cost | EXT packages/descriptors and PRG native/module operations. | Installation/trust/configuration, lazy discovery, and measured adapter usefulness. “Available” does not mean zero lookup, maintenance, or model cost. |
| Programmable permissions and command review | POL exact action/review/approval records; CAP enforcement plans. | Permit evaluation, script/interpreter/argument identity checks, outside-checkout approval storage, atomic approval consumption, and OS/process boundaries. |
| Route a high-level request to the best tool or top few | EXT descriptors and decision functions; PRG invocation; CTX selection provenance. | Retrieve candidates first, validate arguments, include abstention, measure omitted useful operations, then admit the chosen binding. |
| Judge each chunk's relevance and choose full/short/long views | CTX evidence, representations, selection receipt, and original anchors. | Typed relevance/sufficiency functions plus deterministic budget allocation. A missing judgment stays unknown; a Score value is not automatically a summary length. |
| Route easier tasks to cheaper/faster models | POL eligible recipients and route records; EVAL per-workload quality. | Separate decision, generation, reviewer, and executor policies; enforce disclosure before ranking; compare complete-task costs and quality. |
| Offer spend/speed/quality controls | POL pinned preferences and common resource ceilings. | User controls, actual shared reservations, uncertain price handling, and visible refusals. A UI slider cannot relax a hard disclosure or permission rule. |
| Combine skills, tools, and progressive manuals | EXT components and bounded hooks; POL optional activation and mandatory instruction sets. | Load small descriptions, then selected schemas/manuals; keep untrusted package text out of host authority; expire optional activation. |
| Make unfamiliar compression/search tools useful natively | EXT guidance, plugins, source/operation definitions; CTX derivation evidence. | Native prompts/adapters and workload-specific evaluation. Proposed transformations do not inherit permission to execute their suggested commands. |
| Load conditional AGENTS.md sections and gotchas | POL source/scope/precedence and mandatory entries; CTX frame persistence. | Resolve directory ancestry, canonical paths, user corrections, and exact instruction revisions before optional semantic filtering. |
| Make structured skills change behavior with scoped hooks | EXT supported event bindings and allowed operations; POL activation lifetime; COORD optional background plans. | Bounded registered hooks, cleanup on closure/cancellation, reentrancy prevention, and explicit session lifetime. No arbitrary script hooks from a package. |
| Use explicit variables and recursive investigation | CTX typed variables and expansion; PRG bounded dataflow and child programs. | Store intermediates and implement admitted iterative exploration under call/depth/budget bounds. No serialized arbitrary interpreter environment or unlimited recursion. |
| Show relevant regions of grep/tool output | CTX byte-anchored excerpts, representation choices, and decision receipts. | Retain diagnostic captures and render expandable spans/heatmaps. Label relevance scores as judgments, not internal model attention. |
| Run many parallel tasks with shared state and locks | COORD task dependencies, exact claims, fencing, and findings; RUN recovery. | Atomic durable claims, canonical conflict scopes, shared budget accounting, isolated writers, enforced fencing, and integration checks. Nostr replacement is not compare-and-swap. |
| Route by security and likely data access | POL recipient/purpose/classification policy; CAP actual read/network assurance; CTX scoped views. | Classify data, restrict future reads, approve recipients, and apply the same rules to selectors, reviewers, fallback, logs, and background work. Predictions cannot replace confinement. |
| Compress output with headroom/rtk-style techniques | EXT bounded transform components; CTX originals/derivatives; EVAL sufficiency. | Compare deterministic parsing, excerpts, summaries, and optional adapters; verify retained meaning and downstream success. No dependency is mandated by an example link. |
| Generate candidate structural searches and select useful ones | EXT schemas/manuals and registered sources; PRG bounded invocations; CTX provenance. | Learn/load the selected tool interface, generate a bounded candidate set, validate syntax/effects, and rank before admitted execution. |
| Replace repeated exploration with structure, outlines, and a fast index | CTX snapshots/indexes/expansion; CAP sources. | AST/symbol/import indexes, filesystem watchers, typo-tolerant or lexical lookup, invalidation, and measured retrieval recall. Private indexes need not be published. |
| Deduplicate goals and subgoals | COORD exact identity/reuse and advisory duplicate suggestions. | Maintain an authoritative task graph, protect differing acceptance criteria, recheck freshness, and never close a goal on similarity alone. |
| Target the input-heavy subtask breakdown | EVAL declared populations and complete-run metrics; AM/ATIF source receipts. | Measure this agent's file reads, search, tool output, schema overhead, history replay, generation, and explanation. The source image's percentages are illustrative. |
| Background progress pages, explainers, quizzes, and micro-worlds | COORD revision-bound views/findings; CTX shared evidence; RUN attribution. | Low-priority generation/rendering, stale markers, local preview, and explicit publication. A view is not permission to host a public page. |
| Background evaluation and traffic mirroring | EVAL suite/report/partition and promotion records; COORD resource limits. | Separate consent and data policy, held-out evaluation, shadow accounting, retention, and training/export admission. Generated evals are proposals until reviewed and pinned. |
| Cross-model review | POL reviewer identity/disclosure; CTX reviewer context; COORD review findings; EVAL reviewer quality. | Admit a reviewer, give it exact artifact/source identities, preserve disagreement, and verify requirements independently of generation success. |
| Share read-only observations among background consumers | CTX immutable snapshots and representations; COORD coalescing and invalidation. | Capture once within limits, derive scoped views, prioritize foreground work, and track incremental cost and disclosures. Read-only does not mean cost-free. |
| Hierarchical history and tree search across tasks | CTX acyclic indexes, visited/unexpanded branches, bounded expansion, and scoped references. | Tree/graph construction, beam or fallback search, cross-task authority checks, and recall/latency measurement. No guaranteed logarithmic useful retrieval. |

## What belongs at each layer

### Relay and pure protocol library

The pure Rust protocol library validates shapes, references, signatures,
envelopes, and deterministic consistency. The relay validates public records
and private envelope syntax, authenticates connections, applies recipient ACLs
before results/counts, and performs configured retention and bounded retrieval.
Private plaintext is validated by authorized endpoints. The relay must not
pretend that accepting an event means a worker executed it or that a signed
evaluation is true.

Use the existing official and Block protocols where they fit: NIP-42 for
connection authentication, NIP-44 for encrypted payloads, NIP-94 for artifact
locators, AE for mutable private memory, AO for live observability, AM for
accounting, ER for reminders, and PL for mobile wakeups. A reminder or push
lease can wake a client but cannot grant a task, lock a file, or resume unknown
work. No pinned upstream NIP is rewritten by this design.

### Host and coordinator

The host owns permissions, credentials, interpretation of local resource
scopes, OS boundaries, process supervision, package enablement, immutable
snapshots, storage, context construction, and actual provider access. It
resolves mandatory instructions before semantic relevance and applies disclosure
rules to every call, including calls made only to judge relevance.

The coordinator owns durable task state, compare-and-swap claims, reservations,
fencing generations, priorities, cancellation, deduplication, and integration.
It can be the local host. Moving that role to a service does not move authority
into relay replacement rules. A multi-writer deployment needs a trusted
transaction/fencing service with tested failure semantics; event signatures
alone cannot implement it. Unknown effects remain unresolved after lease expiry.

TypeSafe functions are useful at uncertain boundaries: evidence or operation
relevance, representation sufficiency, workload routing, duplicate suggestions,
and semantic requirement review. Implement the question, state builder,
abstention policy, evaluation, and consumer together. Exact lookup, arithmetic,
authorization, serialization, and known conflict checks stay in code. Keep raw
judgments separate from the policy consuming them; batch independent questions
only when they share permitted state and backend limits.

### Client and user experience

Terminal and headless modes consume one host event/receipt model. Users and
agents should be able to inspect the current objective, mandatory instructions,
selected and omitted context, expandable originals, exact operation/recipient,
approval subject, estimated/observed spend, verification, and pending integration.
Expose stale and unknown states rather than smoothing them into success.

UI controls, summaries, heatmaps, quizzes, background HTML, and interactive
explainers are views over those records. They need no dedicated Nostr kinds.
Export/publish is a separate operation. Clients should not expose private source
names through notification payloads or assume an encrypted relay hides all
traffic metadata. Model cache implementation and provider auth also stay outside
Nostr; record enough identity and measured usage to explain their effects.

## Implementation gaps and a practical order

| Increment | Extend | Completion evidence |
| --- | --- | --- |
| 1. Protocol and artifact foundation | Protocol validators, scoped storage, schemas/fixtures, relay privacy, CJ/RUN integration. | Typed references survive local/remote round trips; forged issuers, oversized payloads, inaccessible content, and query ACL leaks refuse. |
| 2. Evidence and task state | Shared evidence and trajectory services, source adapters, task-frame storage. | A user correction survives restart; bounded originals expand; changed source/configuration invalidates stale conclusions. |
| 3. Context and progressive operations | Shared context builder, decision-function registry, CAP/EXT catalog, native operations. | The same repair works with deterministic retrieval and typed reranking; required instructions survive both; candidate misses and selection failures are measured separately. |
| 4. Policy and routing | Host permits/admission, recipient policy, generation/reviewer adapters, usage ledger. | Script changes invalidate approval; no fallback leaks restricted inputs; total routed cost includes rebuilds and failed attempts; uncertainty stays explicit. |
| 5. Parallel and background work | Coordinator, claims, host runtime, protected integration. | Concurrent writers cannot bypass fencing; exact reads can coalesce; stale views are marked; foreground latency and total reservations remain bounded. |
| 6. Evaluation and promotion | Gym, workload suites, matched comparisons, optional signed publication. | Held-out complete-task benefit survives overhead and adverse cases; only exact scoped versions become defaults; publication leaks no private closure. |
| 7. User and agent inspection | Interactive views, structured events, source-linked views. | Both interfaces explain selected evidence, unresolved outcomes, changed approvals, rejected integration, and current budget without a second runtime. |
| 8. Bounded learning and adoption | Semantic signatures, DSPy/GEPA bridges, study/data plans, materialization, and operator adoption. | The exact candidate runs under protected evaluation; all costs and exposure are recorded; active work keeps its pin. |

The [consolidated proposed issues](../../optimization/proposed-issues.md)
define full integration. These are planning records, not filed issues or
permission to execute paid experiments.

## A complete example

1. A user requests a Rust parser repair. The host creates a task frame with
   the exact objective, source snapshot, applicable instructions, and acceptance
   checks. A correction creates a new frame revision.
2. Bounded native search and test operations capture evidence. The context
   builder retrieves related symbols and failure spans, then optionally asks
   an admitted relevance function to rank them. The result pins the supplied
   bytes and records what was omitted or unavailable.
3. The host filters generator/executor candidates by authority and disclosure,
   then selects using measured workload quality, current cost, and cache
   uncertainty. It records the route and retains the actual usage later.
4. A remote worker, when useful, receives a CJ execution request naming the
   pinned program/operation, lock, frame/context inputs, and bounds. It accepts
   durably under RUN. The coordinator grants an isolated writing scope or
   enforced shared claim; a relay acknowledgment grants neither.
5. A background explanation and an independently admitted reviewer consume
   the same snapshot through separate scoped contexts. Their findings remain
   proposals bound to that revision. A changed base makes affected findings
   stale and cancels or revalidates pending work.
6. Protected checks validate the exact proposed artifact. Integration checks
   current base, authority, and fencing before applying it. Completion,
   verification, and integration stay distinct in the result.
7. The client shows the patch, source-linked explanation, verification, and
   measured cost. An explicitly admitted evaluation may compare the complete
   run with a deterministic baseline; it does not export the session by default.

This flow uses the shared execution, evidence, and optimization contracts.
It remains useful entirely locally. Nostr makes the same identities, evidence,
and authority boundaries usable across hosts when remote execution or sharing
is warranted.

## Evaluation requirements

Treat architectural benefits as hypotheses until measured on their workload.
Compare whole tasks with controlled inputs, explicit partitions, and a frozen
acceptance policy. Track missing evidence, harmful errors, uncertainty, and
complete costs. Operator adoption requires scoped evidence; a type-correct
result or attributable receipt alone does not establish quality.
