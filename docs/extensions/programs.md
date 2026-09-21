# Programs and decisions

Status: target specification for typed workflows and AI implementations.

## What the program decision decides

Program selection answers: which admissible workflow does the user's request
ask to run, or `none`? It does not choose a plugin to install, choose an
arbitrary next command, decide whether execution is permitted, or require
that every turn use a workflow.

The target entry path is:

1. Resolve an explicit workflow request directly when the caller supplies a
   structured program ID. Otherwise construct candidates from the current
   task and installed program descriptors.
2. Remove programs with unsupported steps, unresolved references, unmet
   prerequisites, or missing authority. Record why an explicitly requested
   program is unavailable. Do not run executable probes without approval.
3. If interpretation is needed and eligible candidates remain, ask the
   versioned selection function with `none`. Pin candidate order and wording.
4. Treat the answer as a proposal. Check the actual task, effect scope,
   required inputs, budgets, and host grants before the first effect.
5. On `none`, use the ordinary shared turn. On an unknown/unavailable answer,
   follow the declared failure policy; never map a transport failure to
   `none` or dispatch a guessed program.

A program grant permits eligible work; it does not establish that the user
requested this particular task. Validate request scope separately. Include
ordinary bullet lists, quotations of commands, and questions about programs
among negative activation cases. A selected fan-out with no work refuses.

An explicit request should not pay a redundant semantic classification cost.
An ambiguous request may need clarification; an optional selector may degrade
to ordinary response under existing authority. Repeatedly asking the same
selection question over unchanged task/candidate/policy identities is wasted
work. Reuse only a still-valid recorded answer and preserve its origin.

## Program definitions and bindings

NIP-PRG defines a program's identity, named steps, inputs, outputs, and bounds.
A program names sources and question sets rather than carrying commands or
question wording. Keep questions, generation guidance, optional skills, and
host adapter definitions as separate digested assets.

The binding contract defines these interfaces:

| Binding | Required semantics |
| --- | --- |
| Program input | Named schema and version; required/optional fields; maximum size; artifact/evidence references with scope and freshness. |
| Step input | References only to program inputs or declared preceding outputs; field selection is a bounded typed projection, never executable text. |
| Step output | Named schema, completion status, source/artifact identities, limits, and explicit partial/unknown fields. |
| Decision result | Raw typed answers and model/request identities plus the separate policy outcome used by the runtime. |
| Child binding | Exact child definition and dependency lock, typed input projection, output mapping, and parent context/authority references. |
| Terminal output | Declared result references, verification results, unresolved requirements, known costs, and retained artifact locations. |

Resolve and type-check the entire dependency graph before the first effect.
Refuse duplicate names, missing required inputs, impossible type bindings,
unsupported semantics, unresolved child identities, and dependency cycles.
Missing data is an error or an explicit schema-defined optional value, never
an empty string inserted to make the next step run.

These bindings are specified by [NIP-PRG](../../nips/openagents/NIP-PRG.md).
The executor runs an acyclic graph. Bounded retries and repair rounds are explicit host
policies with attempt limits; they do not permit recursive program cycles.

## Step kinds and extension points

| Kind | Contract |
| --- | --- |
| `query` | Resolve a named host source into ordered, bounded, attributable data. |
| `check` | Evaluate a deterministic predicate or a protected host verification plan. |
| `decide` | Ask a separately identified question set through the shared decision client. |
| `delegate` | Give a bounded task to a host-approved executor with explicit context and expected outputs. |
| `program` | Execute a fully resolved child program with typed dataflow and narrowed bounds. |
| `module` | Invoke a pinned Wasm module through the plugin host with typed packets. |
| `invoke` | Invoke a registered native or approved adapter operation under its typed effect contract. |

Native reading, editing, testing, and generation use registered host bindings;
a package cannot add an arbitrary `shell` step by supplying a string. A native
agent executor can perform an open-ended repair under `delegate`, while Rust
owns its admitted operations, context construction, and verification. If a
new independently serialized step kind becomes necessary, specify it before
use. The `invoke` kind supplies this native-operation path. Do not disguise a side effect as a
deterministic `query`.

Plugins can serve two positions. An explicit `module` step transforms typed
input into typed output. A host role can automatically derive evidence after
a supported event. Both use the same guest validation, invocation, authority,
and receipt boundary. A plugin's installation never changes a program's graph.

## Model-independent AI operations

A program may invoke an [AI implementation](../../nips/openagents/NIP-OPT.md)
whose signature defines semantic behavior. The implementation resolves to
a supported decision function, child program, or operation. The host validates
the full closure and matching schemas before execution.

A typed decision is one useful realization. A cited answer, extraction, or
context selection may use another supported strategy. Search can change
bounded internal inference composition while preserving the program's protected
authority and verification transitions. Every candidate is a new immutable
definition; running programs keep their admitted pins.

Use whole-task evaluation as well as module metrics. The
[experiment lifecycle](../optimization/experiments.md) governs selection,
confirmation, and adoption. Program discovery and semantic selection cannot
activate an optimizer or replace an implementation automatically.

## Decision functions inside a program

A function record binds input schema, state builder, question-set digest,
output schema, policy version, model/artifact eligibility, evaluation scope,
limits, and refusal/review behavior. Define a semantic function inventory and
Decision API instead of creating a plugin-specific inference endpoint.

| Selection | Appropriate judgment | Host consumer |
| --- | --- | --- |
| Requested workflow | Choice among eligible programs and `none` | Select a workflow, then validate request and authority. |
| Useful operation | Choice for one alternative; independent relevance for several | Load bounded schemas or invoke a host-scheduled operation. |
| Relevant evidence | Noul or ordered Score under the same rubric | Allocate a context budget with mandatory evidence retained. |
| Representation sufficiency | Judgment against source and task | Use the proposed representation or expand retained evidence. |
| Semantic dependency | Judgment about uncertainty after mechanical checks | Narrow dispatch or request more evidence; never override a known conflict. |
| Requirement review | One attributable requirement and its evidence | Record met, unmet, or unresolved alongside mechanical verification. |

Independent questions over the same state may share a request. A dependent
question waits for its new evidence. Apply backend limits before sending;
unknown answers cannot silently remove required evidence. Question IDs bind
results in code but do not replace complete model-visible meaning.

A Score's weighted mean is not a representation enum. Choice probabilities
from different option sets are not global relevance scores. Confidence is
not permission, proof of correctness, or evidence of workload calibration.
`requires_scorable_answer` checks scoreability only. No new universal threshold
is specified here; promotion requires workload-specific evidence.

Decision backend, generator, and executor selection remain separate policies.
A local relevance call cannot authorize a hosted generator or reviewer.
Review/fallback records the original result, all attempts, actual destinations,
and the consuming policy; local-only policy has no hidden hosted fallback.

## Execution, scheduling, and budgets

Each admitted run pins the task revision, base, dependency lock, source and
question identities, operation bindings, context policy, and authority
snapshot. Recheck revocable grants and mutable source versions at dispatch.
A later catalog update cannot relabel a running attempt.

Limits apply at run, child, step, and attempt scope. Children share the
parent's reservations; they do not each receive a fresh copy of its total
budget. Intersect ceilings and access scopes, account for consumed amounts,
and refuse unsatisfied minimum requirements. Bound total calls, depth,
concurrency, wall time, output/storage bytes, and known monetary spend.
Unknown executor charges remain unknown. Refuse a required hard bound that
no component can enforce.

Extend the project scheduler's dependency and conflict admission. Shared reads
of an immutable snapshot can run together. Write/write and relevant write/read
conflicts, stale bases, and unmet dependencies block dispatch. Unknown effects
require a conservative host policy. Writing delegates retain isolated worktrees
and return artifact identities; integration rechecks the target base.

Background tasks use a separate allowance within the parent ceiling and yield
to foreground work. Debounce input changes, coalesce duplicate requests, cancel
obsolete work, and mark late results stale. Cache reuse keys include task,
source, policy, component, and recipient/disclosure identities. A semantic
near-duplicate is a suggestion; only compatible identity and acceptance permit
reuse of a completed result.

## Durable state and outcomes

The target controller records transitions before effects and observed results
afterward. ATIF explains the execution; the controller owns resumability; the
evidence store owns retrievable observations. None substitutes for the others.

The controller records these conceptual stages under the NIP-RUN journal:

```text
prepared -> admitted -> running -> completed
                    -> refused
running -> failed | cancelled | unknown
unknown -> reconciled completion/failure, or retained for operator resolution
```

Record per-step and per-attempt states as well as the run aggregate. Completed
execution does not imply verified output or accepted integration. Record
verification (`passed`, `failed`, `unverifiable`) and integration acceptance
separately. A required failed or unresolved requirement cannot become success
because every subprocess exited zero.

Before recovery, verify pinned definitions, outstanding reservations, artifact
identity, surviving executor sessions, and revocation state. Reconnect where
supported. Never automatically replay an ambiguous write, publication, paid
request, or delegate dispatch. A pure transformation can be retried only under
its declared deterministic/idempotent contract with the same input identity
and a new attempt record. Bound every retry, including review and fallback.

Cancellation stops queued work, propagates to children and supervised process
groups, and retains partial artifacts and cleanup failures. A disconnected
remote executor may remain unknown; report that limitation rather than claim
it stopped. Integrating or publishing a result remains a separately authorized
effect with fresh preconditions.

## Required execution record

A program record joins session/turn, task, program, child, step, and attempt IDs
to definition/dependency digests, authority and policy, evidence/context
manifests, decision request/result identities, adapter/plugin receipts,
artifacts, verification, and known/unknown costs. Record omitted, unattempted,
refused, cancelled, and unknown work as well as completed work.

Offline replay reconstructs the supplied inputs and host transitions without
executing commands or inference. Re-execution is a new authorized attempt.
A digest proves byte identity and local consistency, not remote execution
or a provider cache hit.
