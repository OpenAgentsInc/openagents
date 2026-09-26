# NIP-AUTO — Bounded automation and continuation

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile defines durable scheduled work, source-triggered
work, and bounded continuation toward a pinned objective. It introduces no
new event kinds. Its artifacts use private `3188` envelopes; accepted
transitions belong to a [RUN](NIP-RUN.md) journal.

An automation can ask an agent to investigate a changed record, continue a
bounded objective, or run a periodic check. A timer, reminder, model statement,
idle session, reconnect, or webhook is not execution authority. Every occurrence
requires current [POL](NIP-POL.md) authority and [COORD](NIP-COORD.md) admission.
This is a target contract; the current Coder runtime does not implement it.

## Roles, operations, and encoding

The owner authorizes the immutable plan. One admitted controller serializes
its state. A source observes a resource; a worker executes an occurrence; a
checker evaluates continuation. These roles may share a process, but their
outputs and authority remain distinct. An owner field, relay membership, or
Block owner attestation cannot appoint a controller or grant spending.

CAP operations use pinned SchemaRefs and CJ execution admission:

| Operation | Input schema | Output schema |
| --- | --- | --- |
| Admit plan | `openagents.automation-plan.v1` | `openagents.automation-result.v1` |
| Pause, resume, cancel, or read | `openagents.automation-control.v1` | `openagents.automation-result.v1` |
| Observe a configured source | Pinned source input schema | `openagents.automation-observation.v1` |
| Evaluate continuation | `openagents.automation-evaluation-input.v1` | `openagents.automation-evaluation.v1` |
| Execute an occurrence | `openagents.automation-input.v1` | Pinned target output schema |

All artifacts below have required `v` and `requires: []`, optional inert `meta`,
and exactly the listed fields. Referenced structured artifacts require their
exact schema and complete supported closure. Unknown semantics refuse. IDs are
random common IDs, public keys and references follow the shared contracts,
and timestamps are Unix seconds. A declared duration is a positive integer.
The controller uses a trusted clock; signed event times cannot extend validity.

Every statement attributed to an owner, controller, source, worker, or checker
MUST have that principal's authenticated CJ provenance or an accessible signed
`3188` declaration binding the exact bytes. A copied ArtifactRef is neither a
signature nor authority to retrieve private content. Local records need the
corresponding authenticated, durably retained host provenance.

## Immutable plan

A plan has `v: "openagents.automation-plan.v1"` and these fields:

| Field | Type and meaning |
| --- | --- |
| `automation`, `run` | Fresh plan identity and scheduler RUN identity. |
| `owner`, `controller` | Owner and initially admitted controller public keys. |
| `coordinator`, `frame`, `policy`, `reservation` | Exact COORD descriptor, CTX task frame, POL policy, and aggregate parent reservation ArtifactRefs. |
| `task` | Existing task ID in the coordinator's scope. |
| `target` | DefinitionRef accepting `openagents.automation-input.v1`, including a supported adapter if needed. |
| `lock`, `payload`, `context`, `requirements`, `acceptance` | Exact ArtifactRefs for target closure, domain input, CTX context, enforcement requirements, and task acceptance policy. |
| `effects` | Common effects ceiling for the entire plan. |
| `per_occurrence`, `aggregate` | Common bounds; aggregate includes observation, execution, checking, children, and recovery. |
| `max_occurrences`, `max_observations` | Positive finite admission limits. An unused observation limit does not authorize observing a source. |
| `starts_at`, `expires_at`, `retain_until` | `starts_at < expires_at < retain_until`. No new dispatch after expiry. |
| `trigger` | One of the closed variants below. |
| `completion` | Null, or the checker binding below; required only for `continuation`. |

The owner signs the plan. The controller validates its identity and all grants,
source rights, effect scopes, required enforcement, disclosure recipients,
reservation limits, and retention obligations before activation. Plans are inert
before this durable admission. Unsupported bounds refuse; estimated prices
cannot satisfy an unenforceable hard monetary ceiling.

A checker binding is exactly `{target, lock, context, requirements, bounds}`:
DefinitionRef, three ArtifactRefs, and common bounds. It accepts the evaluation
input below and returns the evaluation schema. It is pinned independently of
the worker's output and cannot be replaced by the working model's self-claim.
A supported semantic checker is still an attributable judgment; the acceptance
policy specifies the required evidence and assurance, including deterministic
checks where required. A checker result cannot rewrite that policy.

The trigger is exactly one of:

| Variant | Fields and semantics |
| --- | --- |
| Once | `{kind: "once", at}`. One due slot at `at`, within the plan's active interval. |
| Interval | `{kind: "interval", first_at, every_seconds, missed}`. Slot `n` is `first_at + n × every_seconds`; `n` begins at zero. `missed` is `skip` or `latest`. |
| Source | `{kind: "source", first_at, every_seconds, source, resource, lock, input, context, requirements, bounds, max_age_seconds}`. The source is a pinned read-only CAP DefinitionRef; resource is the exact host-resolved logical resource ID; lock/input/context/requirements are ArtifactRefs and bounds are common bounds. Polls follow the interval slots and use `latest` missed-slot handling. |
| Continuation | `{kind: "continuation", minimum_interval_seconds}`. The first occurrence is due at `starts_at`; subsequent occurrences require the evaluation and settlement below. |

Interval and source `first_at` is within the active interval. Arithmetic MUST
be checked for overflow. This profile defines fixed UTC intervals, not cron,
time-zone calendars, DST recovery, unlimited subscriptions, or recursive
self-created plans. A later profile must define those semantics explicitly.
A source may use a webhook or relay subscription to wake its poller early,
but an untrusted wake neither creates an occurrence nor changes its slot.

Only one occurrence may be active per plan in v1. Child concurrency still
follows COORD reservations. A due slot cannot interrupt a running occurrence.
For `skip`, missed or occupied slots are durably skipped. For `latest`, the
controller coalesces missed slots and considers only the latest currently due
slot after the active occurrence settles. It records the full skipped slot
range, including due times, without creating paid work for each missed slot.
A `once` slot observed late is eligible only before plan expiry. Suspended
machines do not acquire a right to replay a backlog.

A skipped-range artifact has `v: "openagents.automation-skipped.v1"`, `plan`
(ArtifactRef), `first_slot`, `last_slot`, `first_due_at`, `last_due_at`, and
`reason`. Slots are nonnegative inclusive integers with first no greater than
last; due times must equal the interval/source formula for those slots. Reason
is `missed`, `occupied`, `paused`, `expired`, or `bound`. The controller signs
and journals it. Ranges may not overlap admitted interval occurrences, admitted source polls,
or previously recorded skips; they account for scheduling gaps without inventing successful runs.

## Observations, occurrences, and exact input

An observation has `v: "openagents.automation-observation.v1"`, `source`
(DefinitionRef), `resource` (host-resolved logical resource ID), `revision`
(nonempty opaque source version string or null when unavailable), `observed_at`, `available` (boolean),
`evidence` (ArtifactRef or null), and `reason` (common refusal code or null).
The configured source signs it. A true `available` requires a nonempty revision, evidence, and null reason;
false requires a reason, null evidence, and null revision. An unavailable
source cannot invent a version merely to satisfy the response schema. The source's pinned binding
MUST define resource identity, authoritative revision semantics, and how an
observation is validated or refreshed. The controller validates observation
time against its trusted clock and the binding's admitted clock uncertainty;
unverifiable freshness refuses. A source without these guarantees
cannot use this profile. Source output is data, never a new instruction. A [WORK](NIP-WORK.md) source
retains the exact WorkRevisionRef inside its evidence; display status or item
assignment alone cannot authorize an occurrence.

A poll admission has `v: "openagents.automation-poll.v1"`, `plan`, `poll`,
`phase`, `slot`, `occurrence`, `request`, `run`, and `deadline`. Plan is its
ArtifactRef; poll/request/run are fresh common IDs; phase is `observe` or
`revalidate`. Observe uses its nonnegative source slot and null occurrence;
revalidate uses null slot and the exact unstarted occurrence ArtifactRef.
Deadline is bounded by source/aggregate allowance and plan expiry. Persist
and reserve this artifact before CJ dispatch; deduplicate observations by
`(plan, slot)` and revalidation by `(plan, occurrence)`. The controller retains
every poll admission and its exact CJ/RUN outcome, including unavailable,
failed, or unknown polls. Only available observations of the configured
resource can trigger work. Deduplicate by `(plan, source definition digest,
resource, revision)` for the plan's retained lifetime. Do not order opaque
revision strings or treat a different webhook delivery ID as a new version.
Immediately before dispatch, the source revalidates that exact version under
its pinned binding and `max_age_seconds`. Stale evidence cancels the unstarted
occurrence; a subsequent valid revision can create another. Revalidation calls
consume observation and aggregate budgets. No freshness check can lock out
independent external writers; the eventual effect still requires its own
version precondition and external confirmation.

An occurrence has `v: "openagents.automation-occurrence.v1"` and exactly
`plan`, `occurrence`, `ordinal`, `key`, `due_at`, `deadline`, `observation`,
`prior`, `request`, `run`, and `task`. Plan is an ArtifactRef; occurrence,
request, run, and task are fresh common IDs; ordinal is a positive monotone
integer. Observation is its ArtifactRef or null. Prior is a previous occurrence
outcome ArtifactRef or null. Deadline is at most plan expiry and the applicable
per-occurrence wall bound. `key` is one of these closed objects:

- `{kind: "once"}`.
- `{kind: "interval", slot}` with nonnegative integer slot.
- `{kind: "source", source, resource, revision}` with exact source DefinitionRef
  and the authenticated observation's resource and revision.
- `{kind: "continuation", predecessor}` with a previous occurrence ArtifactRef,
  or null for the first occurrence.

The controller atomically reserves budget, deduplicates this key, allocates
identities, and retains the occurrence before acknowledging or scheduling it.
Ordinals count admitted occurrences, including later refusal, cancellation,
and unknown outcomes. Counters never reset after pause, recovery, or handoff.
Duplicate delivery returns the same occurrence; conflicting bytes for its
identity refuse. A retransmission keeps the same CJ request, attempt, and
fingerprint. A new experimental repeat is a new occurrence and trial identity,
not cached evidence from an earlier attempt.

The worker input has `v: "openagents.automation-input.v1"` and exactly
`plan`, `occurrence`, `frame`, `payload`, `context`, and `prior`. The first five
are exact ArtifactRefs; prior is the occurrence's prior outcome or null. This
wrapper identifies the scheduler input without changing the target's schema
by convention. An ordinary agent or [SESS](NIP-SESS.md) session operation requires an explicitly
pinned adapter accepting this wrapper; hosts MUST NOT prepend it to arbitrary
prompts or hide it in `meta`. The occurrence does not contain its future input
or CJ event digest, so constructing the wrapper creates no digest cycle.

A session adapter must await the queued intent's terminal disposition and any
promoted turn, managed children, and ENV/effect reconciliation within the
occurrence's bounds. A successful enqueue/control operation alone is not the
occurrence outcome. The occurrence stays active while the intent is queued
or its turn runs; a bounded timeout records unknown or confirmed cancellation
according to retained evidence. Cancellation propagates to the pending intent
or active turn. This prevents interval/source plans from accumulating queued
turns and prevents a continuation checker from grading queue admission as work.

The controller creates a fresh COORD proposal and claim for the occurrence.
The proposal uses the occurrence task ID, the plan task as parent, and the
pinned frame, operation closure, acceptance, and narrowed effects and bounds. Current grants and source/resource state are checked again. An old
plan signature cannot override revoked access or a changed task frame. A
frame change blocks unstarted work pending a new explicit plan; it does not
silently change the objective or protected acceptance criteria.

An occurrence outcome has `v: "openagents.automation-outcome.v1"`, `occurrence`,
`proposal`, `claim`, `result`, `settled`, `evidence`, and `usage`. Occurrence,
proposal, result, and settled are ArtifactRefs; claim is an ArtifactRef or
null for a pre-claim refusal; evidence and usage are arrays of exact receipt
ArtifactRefs. Result uses the admitted target's output/outcome contract and
settled references its authenticated RUN terminal record. Null result is
permitted only for a documented refusal before target dispatch; its RUN record
still supplies the refusal. Unknown outcomes stay unknown and retain holds.

## Continuation and completion

After each continuation occurrence settles, the controller may admit the pinned
checker. Its input has `v: "openagents.automation-evaluation-input.v1"` and
`plan`, `occurrence`, `outcome`, `frame`, and `acceptance` (ArtifactRefs).
The result has `v: "openagents.automation-evaluation.v1"`, `input`
(exact input ArtifactRef), `decision` (`continue`, `complete`, `blocked`, or
`unknown`), `evidence` (receipt ArtifactRefs), and `reason` (bounded text).
The controller authenticates the checker and its exact admission/implementation,
retains the complete result, and validates it against the pinned policy.
Missing, malformed, timed-out, unavailable, or contradictory checks are
`unknown`; they cannot confirm completion.

Only a policy-valid `continue` can create the next occurrence. The preceding
run and all managed children must be settled with no unresolved effects or
usage holds; the next due time is no earlier than prior settlement plus the
minimum interval. An `unknown` or `blocked` evaluation blocks the plan. A
cancelled occurrence pauses it. Budget or expiry exhaustion stops it without
claiming the objective succeeded. The controller MUST NOT infer continuation
from a quiet process, empty stream, disconnected client, session idle state,
model finish token, or incomplete transcript.

`complete` means only that this plan's pinned completion rule accepted its
retained evidence. It does not accept a LAB deliverable, close an external
issue, merge code, release payment, or declare every work item complete.
Those are separately admitted domain actions. A model-proposed completion
claim is evidence for the checker, not the final state transition.

## Controls, journal, and recovery

A control has `v: "openagents.automation-control.v1"`, `plan`, `command`,
`expected_revision`, `action`, and `reason`: exact plan ArtifactRef, common
command ID, current nonnegative revision, `pause`, `resume`, `cancel`, or
`status`, and bounded text. It is signed by the owner or a separately admitted
principal for exactly that action and plan. Generic CTRL observation or
steering rights do not authorize automation creation or budget increases.
Identical command retries return the retained result; conflicting command
reuse refuses. Mutation admission uses compare-and-swap on the current revision.

A state has `v: "openagents.automation-state.v1"`, `plan`, `revision`,
`previous`, `controller`, `generation`, `status`, `occurrences`, `observations`,
`active`, `last_outcome`, `evaluation`, `accounting`, and `record`.
Previous is the preceding state ArtifactRef, null only at revision zero.
Controller/generation follow RUN; counters are nonnegative. Active is an
occurrence ArtifactRef or null; last_outcome and evaluation are ArtifactRefs
or null. Accounting is the current reservation/usage receipt ArtifactRef;
record references the exact RUN transition from which this state is derived;
that transition does not reference the future state artifact. Status is `active`, `paused`,
`blocked`, `exhausted`, `complete`, `cancelled`, or `expired`.

A result has `v: "openagents.automation-result.v1"`, `input`, `status`,
`state`, and `reason`: input ArtifactRef, `accepted`, `duplicate`, `conflict`,
`refused`, or `unknown`, state ArtifactRef or null, and common refusal code
or null. Accepted and duplicate results require the authoritative state;
conflict/refusal require a reason. The expected controller signs results.

Admission starts at revision zero with active status. Every accepted control,
occurrence admission/outcome, observation accounting change, skipped-slot
range, and completion decision is durable before acknowledgment. RUN records
retain those typed artifacts and coalescing evidence; the latest state is a
projection, not a replacement for the journal. Competing states at one revision
halt automatic dispatch. Status reads do not advance revision or activate work.

| Transition | Required behavior |
| --- | --- |
| Active → paused | Stop new occurrence and source/checker admission. Already dispatched work remains visible and bounded; pause alone does not claim to stop it. |
| Active/paused/blocked → cancelled | Stop all future admission; request cancellation of managed in-flight work and retain unresolved effects. Cancellation is terminal for this plan. |
| Paused/blocked → active | Require explicit owner-authorized resume, refreshed grants/frame/source checks, available budget, and reconciled prior unknowns. No automatic resume from a later timer. |
| Active → exhausted/expired | Stop new dispatch at a bound or trusted-clock expiry; cancel managed work according to its admitted deadline and retain unknowns. |
| Active → complete | Require the validated checker decision for continuation, or all admitted finite scheduled work settled after the last eligible slot with no unknowns. Scheduled completion does not imply a goal was achieved. |

Complete, cancelled, exhausted, and expired are terminal scheduling states.
They can retain an active cleanup/reconciliation obligation; that is shown
separately, never erased by the scheduling label. Plan replacement requires a
new owner-authorized identity and reservation, and cannot discard earlier
unknown liabilities or secretly reset a shared ancestor allowance.

Revocation of required authority blocks new dispatch immediately when the host
learns it; unsupported freshness guarantees refuse. A controller durably
serializes interactive task changes and automation dispatch, so a pending
accepted cancellation or frame correction cannot lose to an advisory timer.
Recovery resumes the exact ledger and occurrence identities. RUN fencing is
required before controller takeover. If takeover changes a frozen coordinator,
binding, or policy identity, stop this plan and require explicit replacement
with retained accounting; do not silently update its pins. A lost dispatch response is unknown;
reconnection queries and reconciles it rather than submitting another job.

## Accounting, privacy, and conformance

Preparation, source polling/revalidation, checker calls, children, retries,
and runtime reservation costs all draw from the aggregate allowance. Reserve
before dispatch. Unknown costs remain unknown and cannot replenish allowance.
Clock/slot metadata and source inventories can disclose activity; keep them
private and retain the signed inputs and decision evidence under the admitted
policy. Imported recipes, hooks, learned preferences, and work-item text are
inputs subject to POL, not permission to create another automation.

Block [ER](../block/NIP-ER.md) can show a reminder and
[PL](../block/NIP-PL.md) can wake a client. Neither schedules an admitted
occurrence. Block [AO](../block/NIP-AO.md) telemetry is ephemeral, and
[AM](../block/NIP-AM.md) usage summaries do not reserve a shared budget.
[NIP-40](../official/40.md) expiry concerns event visibility, not job fencing.
COORD's read-only background findings keep their existing smaller contract;
AUTO is for explicitly admitted execution and continuation lifetimes.

Required conformance cases include:

1. Duplicate timer/webhook delivery, changed bytes under one command ID, and
   two racing controllers produce at most one admitted occurrence per key.
2. Sleep across many interval slots records skipped ranges without a burst;
   pause/resume preserves counters and never replays cancelled work.
3. A source changes after observation, or its revision cannot be revalidated:
   unstarted work refuses; downstream effects still enforce their own versions.
4. A goal self-claim with a missing checker remains unknown, and a failed or
   cancelled child cannot disappear from the continuation decision.
5. Crash after durable dispatch intent but before response retains unknown
   effects and budget holds. Neither expiry nor controller restart clears them.
6. Revoked grants, changed frame, inaccessible evidence, unsupported effects,
   unenforceable costs, and exhausted aggregate budgets block new work.
7. A queued session adapter preserves its native queue admission and does not
   confuse session idle with permission to start another turn.
8. A plan completing successfully does not imply domain acceptance, publication,
   settlement, or an external work item's completion.

Conformance claims name the roles implemented: plan controller, source,
checker, worker adapter, or observer. Relay support for `3188` alone does not
implement automation. Required Rust validators, durable host state, dispatch
fencing, operation adapters, and crash fixtures remain implementation work.
