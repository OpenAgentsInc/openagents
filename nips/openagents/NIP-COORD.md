# NIP-COORD — Shared tasks and background work

`draft` `optional` — v1, 2026-09-21. The [shared contracts](contracts.md)
are normative. [NIP-RUN](NIP-RUN.md) supplies the authoritative journal and
controller fencing; [NIP-CJ](NIP-CJ.md) supplies authenticated invocation.
This NIP defines task, claim, and background-result artifacts exchanged by
participants. Relay event replacement is not a lock, transaction, or scheduler.

All artifacts include `v`, `requires`, and optional inert `meta`. Private
`3188` envelopes carry separately signed proposals/receipts. RUN records refer
to accepted transitions. No additional event kind or consensus algorithm is
introduced. One admitted coordinator owns each scope; hosts can use this
contract locally without a relay or additional model calls.

## Coordinator admission and operations

A coordinator descriptor has `v: "openagents.coordinator.v1"`, `scope`
(opaque ID), `controller` (pubkey), `epoch` (monotone integer), `run`
(RUN identity), `capability` (DefinitionRef), `policy` (ArtifactRef), and
`resources` (ArtifactRef to the exact resource namespace). The owner admits
this descriptor independently. A self-published descriptor cannot acquire
control over another workspace. Restart cannot reset the epoch or claim counter.

CAP bindings expose operations using typed input and output SchemaRefs. Remote
operations use CJ execution v1, so signer, deadline, idempotency, replay,
cancellation, and durable acceptance are inherited rather than reinvented.

| Role | Input schema | Success schema |
| --- | --- | --- |
| Propose task | `openagents.task-proposal.v1` | `openagents.coordination-result.v1` |
| Claim task/resources | `openagents.claim-request.v1` | `openagents.coordination-result.v1` |
| Renew, release, cancel, or read status | `openagents.coordination-control.v1` | `openagents.coordination-result.v1` |
| Submit finding or proposed integration | `openagents.finding.v1` | `openagents.coordination-result.v1` |

An operation result has `request` (exact input ArtifactRef), `coordinator`
(descriptor ArtifactRef), `revision` (monotone durable scope revision),
`status` (`accepted`, `duplicate`, `conflict`, `refused`, or `unknown`),
`record` (exact encrypted RUN EventRef or retained record ArtifactRef),
`value` (typed ArtifactRef or null), and
`reason` (common refusal code or null). Conflicts/refusals require a reason.
Response authority comes from the configured coordinator signer, not from a
participant echoing these fields. A CJ `completed` result can contain a
coordination conflict; it means the operation answered, not the task succeeded.

## Task identity, dependencies, and reuse

A proposal has `v: "openagents.task-proposal.v1"`, `task` (random ID),
`coordinator` (ArtifactRef), `frame` (CTX task-frame ArtifactRef), `parent`
(task ID or null), `target` (DefinitionRef), `lock`, `input`, `context`
(ArtifactRefs), `dependencies` (task IDs), `effects`, `bounds`, `acceptance`
(ArtifactRef), and `reuse` (`never` or `exact`). The coordinator resolves all
dependencies within the admitted scope and rejects cycles and absent owners.
Task frames distinguish user intent from inferred subgoals. Corrected frames
create a new proposal identity and invalidate affected unstarted work.

The exact reuse key is SHA-256 of JCS of `{scope, frame, target, lock, input,
context, effects, bounds, acceptance}`, with all references resolved to their
exact identities. Request IDs and display wording outside those artifacts do
not establish equivalence. Reuse additionally requires compatible grants,
recipient/disclosure, source freshness, and retained result/verification data.
Effectful work defaults to `never`; repeated requests for an effect are not
duplicates merely because arguments match. In-flight read coalescing requires
an immutable snapshot and a binding with a compatible idempotency contract.

A duplicate suggestion has `v: "openagents.duplicate-suggestion.v1"`,
`proposed` (proposal ArtifactRef), `candidate` (existing proposal ArtifactRef),
`decision` (receipt ArtifactRef), and `differences` (ArtifactRef). It is
advisory. Only the coordinator's deterministic equivalence/admission check or
an explicit owner decision may omit work. A high semantic similarity score
cannot close a requirement, change the objective, or return another tenant's
result. Record the reuse source and its freshness checks in RUN.

## Claims, fencing, and shared state

A claim request has `v: "openagents.claim-request.v1"`, `proposal`
(ArtifactRef), `worker` (pubkey), `resources`, `expected_revision`,
`requested_until`, and `reservation` (parent reservation ArtifactRef).
Resources are entries `{id, mode, snapshot}`: exact coordinator resource ID,
`read` or `write`, and snapshot ArtifactRef or null. Immutable snapshot reads
can share; unsnapshotted live reads conflict with writes. Multiple writes to
one resource conflict. Canonical paths, symlink/case aliases, repository-wide
locks, and logical resource overlap are resolved by the host namespace before
claiming. A model's claim that edits are independent cannot override overlap.
Validate requested expiry against the coordinator's trusted clock and maximum
lease duration; stale or excessively future requests refuse. Published event
timestamps cannot extend a lease.

The coordinator atomically compares expected revision, reserves the shared
budget, and grants all resources or none. The claim artifact has
`v: "openagents.claim.v1"`, `proposal`, `coordinator` (ArtifactRefs),
`worker`, `resources`, `token` (strictly increasing integer within coordinator
epoch), `epoch`, `expires_at`, `reservation`, and `revision`. A dispatcher
accepts work only from the named worker, for the exact task/resource grant,
and under the latest token registered in its trusted coordinator connection.
A signed claim forwarded by another principal is not transferable authority.

Every effect dispatcher, including integration, must enforce epoch/token
fencing. The coordinator cannot release resources for a conflicting writer
until old dispatchers are fenced and unresolved prior effects are reconciled.
Lease expiry stops new dispatch but does not prove old code stopped; it never
automatically frees unknown reservations. Unsupported fencing confines writers
to isolated workspaces with a protected integration gate, or refuses shared
writing. Host clocks, transactional storage, and process termination implement
these guarantees; relay timestamps and addressable heads cannot do so.

A control has `v: "openagents.coordination-control.v1"`, `coordinator`,
`proposal`, `claim` (ArtifactRef or null), `action` (`renew`, `release`,
`cancel`, or `status`), `expected_revision`, `requested_until` (Unix seconds
for renewal, otherwise null), and `evidence` (receipt ArtifactRefs).
Renew/release require the named worker or authorized owner; cancel requires
the owner or a separately admitted controller; status requires scoped read
authority. Release acknowledges reconciliation and dispatcher fencing, not
merely a request to free resources. The result records pending unknown work.
Status is observational and cannot authorize dispatch without a current claim.

## Background subscriptions

A background plan has `v: "openagents.background-plan.v1"`, `owner`,
`coordinator`, `task_frame`, `operation`, `lock` (references), `trigger`,
`inputs`, `bounds`, `priority`, `lifetime`, `expires_at`, `disclosure`,
`destination`, and `stale_policy`. Owner is a pubkey; coordinator/frame/lock/
inputs/disclosure are ArtifactRefs; operation is a DefinitionRef. Priority
is `background`. Trigger is `snapshot_changed`, `operation_completed`, or
`task_closed`. Lifetime is `operation`, `task`, or explicitly admitted
`session`; destination is `evidence` or `view`; stale policy is `cancel`
or `retain_as_stale`. Bounds narrow a separately reserved allowance under the
parent budget. A plan is inert until an authorized host activates it.

The host observes typed trigger events, debounces/coalesces repeated input
digests, checks the plan's lifetime, and admits one bounded proposal per
effective input snapshot. Exact duplicate delivery cannot create another job.
These are scheduler events, not new EXT skill-hook names. A skill activates
only its supported EXT hooks; any background plan requires its own admission.
Task closure disables new background triggers after its one admitted closure
notification. Derived background results never recursively trigger themselves.

Read-only source access can still cost money and disclose data. Background
plans cannot write the primary workspace, change policy, publish externally,
train a model, or mirror live traffic merely by selecting `view`. Such effects
require a separately admitted operation and explicit scope. Store proposed
tests/patches as artifacts; execution, integration, and publishing remain
separate. Apply foreground priority, cancellation, total concurrency/spend,
and fair resource accounting at the host scheduler.

## Findings and integration

A finding has `v: "openagents.finding.v1"`, `proposal`, `frame`, `snapshot`,
`context` (ArtifactRefs), `producer` (pubkey), `kind` (`explanation`, `review`,
`test_proposal`, `patch_proposal`, `evaluation`, or `fact`), `content`
(ArtifactRef), `evidence` (descriptor ArtifactRefs), `receipts`, `valid_until`,
`verification`, and `integration`. Receipts are ArtifactRefs; the last two
fields use common states. Authenticate producer and admitted attempt through
the original CJ result or its signed envelope. A free-standing finding is an
attributable proposal, never authority to merge or proof of task completion.

Before using a finding, compare task revision, snapshot, policy, and acceptance
identities. Changed inputs mark it stale; use for history or explicitly
revalidate. Integration requires current resource claims, exact proposed/base
artifact identity, independent checks, and an authorized atomic application.
Parallel successful children do not establish parent success. Show pending,
rejected, stale, and unknown findings separately from accepted results.

## Conformance and existing protocols

Required cases include two racing claims, duplicate CJ delivery, cyclic tasks,
aliasing resources, stale frames, semantic false duplicates, lost renewals,
crashed coordinators, unfenced workers after expiry, unknown spend, changed
background inputs, hook feedback loops, and stale integration. Recovery follows
RUN handoff and requires trusted fencing before a new controller dispatches.

NIP-AO can carry live telemetry, NIP-AM accounting, NIP-ER reminders, and
NIP-PL mobile wakeups. None supplies these task/resource claims; a push lease
is not a workspace lock. NIP-AE mutable memory can index retained task frames
but does not replace immutable history. Advertise `nip-coord-v1` only for the
tested coordinator/client role. A relay forwarding CJ or `3188` alone cannot
claim to schedule tasks, prevent conflicting writes, or enforce budgets.
