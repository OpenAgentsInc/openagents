# NIP-ENV — Environment leases and runtime attachment

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile defines admitted environment allocation, exact
materialization, bounded leases, runtime attachment, and cleanup. It introduces
no new event kinds. Private `3188` artifacts and [RUN](NIP-RUN.md) records carry
its requests and receipts; [CAP](NIP-CAP.md) operations over [CJ](NIP-CJ.md)
perform admitted effects.

A placement is not permission. A VM, container, worktree, remote host, or
provider session does not acquire authority from its name or apparent
availability. This draft is a target contract, not a claim that Coder already
supports a portable fleet allocator or these operations.

## Scope and roles

The **owner** authorizes resource use. An admitted **allocator** controls the
lease and provider lifecycle. The **worker** executes an attached operation.
A **participant** is another admitted runtime or inference/tool provider that
receives scoped inputs or performs effects. A provider account/resource and
its controlling credentials remain host-owned. These roles may be colocated,
but their signatures and assurances are not interchangeable.

ENV controls resource lifetime. [WS](NIP-WS.md) identifies workspace snapshots,
worktrees, and conditional mutations. [SESS](NIP-SESS.md) owns provider-session
admission and queue/resume semantics. [RUN](NIP-RUN.md) owns task-controller
handoff and dispatcher fencing; allocating another machine does not transfer
that controller. [COORD](NIP-COORD.md) claims resources and reserves shared
budgets. [POL](NIP-POL.md) governs effects, credentials, approvals, and disclosure.

CAP bindings advertise these closed operation roles:

| Operation | Input schema | Output schema |
| --- | --- | --- |
| Allocate or adopt an environment | `openagents.environment-request.v1` | `openagents.environment-result.v1` |
| Renew, release, or inspect a lease | `openagents.environment-control.v1` | `openagents.environment-result.v1` |
| Reserve an execution attachment | `openagents.environment-attach.v1` | `openagents.environment-result.v1` |
| Admit a participant to that attachment | `openagents.environment-participant-input.v1` | `openagents.environment-participant-admission.v1` |
| Activate or drain an attachment | `openagents.environment-attachment-control.v1` | `openagents.environment-result.v1` |

Each binding pins its input/output SchemaRefs, lifecycle implementation,
provider effects, external idempotency/reconciliation contract, and enforceable
limits. Discovery, a successful ping, or relay authentication alone cannot
admit it. Local calls use the same artifacts and authenticated provenance.

## Encoding and signed evidence

Every artifact has required `v`, `requires: []`, optional inert `meta`, and
exactly the fields listed below. IDs, references, effects, bounds, timestamps,
and byte ceilings follow the shared contracts. Unknown semantic fields,
profiles, or required enforcement refuse. Timestamps use a trusted host clock;
an event's self-declared time cannot renew an expired lease.

An attributable artifact MUST resolve to its expected signer through an
accessible signed `3188` declaration or authenticated CJ result binding those
exact bytes. A bare reference is not a signature. Local evidence requires the
equivalent durably retained host provenance. An allocator receipt establishes
what that allocator claims; it is not remote attestation, proof of isolation,
or proof of a provider's model weights.

## Environment definition and request

An environment definition has `v: "openagents.environment-definition.v1"`:

| Field | Type and meaning |
| --- | --- |
| `components` | Nonempty array of `{id, role, artifact}`: unique slug, `base`, `runtime`, `model`, `toolchain`, or `data`, and exact ArtifactRef. |
| `workspace` | Exact WS snapshot/binding ArtifactRef or null for work without a workspace. |
| `bootstrap` | Null, or `{target, lock, input, context, requirements, effects, bounds}`: DefinitionRef, four ArtifactRefs, common effects, and common bounds. |
| `runtime`, `configuration` | Exact runtime DefinitionRef and configuration ArtifactRef. |
| `model` | Requested model descriptor ArtifactRef or null when no model is used. |
| `requirements` | Exact enforcement/disclosure requirement ArtifactRef, including the required assurance and admitted recipients. |

All executable/bootstrap dependencies, resolved packages, images, and runtime
artifacts belong to the pinned closure. A tag such as `latest`, an ambient
`PATH` result, or a recipe hash omitting downloaded bytes is insufficient.
A permitted opaque provider model identity must be declared as such in its
supported descriptor; the host cannot pretend it verified unavailable weights.
Actual identity and assurance are recorded during materialization. Changed
requirements or runtime/configuration bytes require a new definition.

Bootstrap runs as its own admitted bounded operation before readiness. A
repository setup file is code, not trusted policy. Pin the workspace base and
any dirty/untracked input manifest through WS. Credential values, ambient user
homes, model authentication stores, and inherited process environments are not
part of an environment artifact. Host-owned credential bindings narrow to
actual account, destination, purpose, recipient, and lifetime; a lower-trust
repository or plugin cannot redirect a higher-trust credential.

A request has `v: "openagents.environment-request.v1"` and these fields:

| Field | Type and meaning |
| --- | --- |
| `request`, `run` | Fresh request and allocation RUN IDs. CJ transport retries preserve them. |
| `owner`, `allocator` | Exact public keys, independently authorized by the receiving host. |
| `definition`, `frame`, `policy`, `reservation` | Exact definition, CTX frame, POL policy, and parent reservation ArtifactRefs. |
| `mode` | `create` or `adopt`. |
| `resource` | Null for create; an authenticated existing-resource descriptor ArtifactRef for adopt. |
| `effects`, `bounds` | Common ceilings covering provision, bootstrap, operation lifetime, retention, and cleanup. |
| `ttl_seconds`, `idle_seconds` | Positive lease-renewal and idle limits. |
| `not_after`, `retain_until` | Absolute hard resource-use deadline, then evidence-retention deadline. |
| `cleanup` | `destroy` for a created resource or `detach` for an adopted resource. |

An existing-resource descriptor has `v: "openagents.environment-resource.v1"`,
`owner`, `allocator`, `provider` (DefinitionRef), `account` (host logical account
ID), `resource` (opaque provider resource ID), `generation` (opaque exact
provider generation), and `ownership` (ArtifactRef). Ownership evidence must
be validated by the provider binding against the independently authorized
account and current resource generation. A label, hostname, path, or copied
resource ID cannot prove ownership or authorize adoption/deletion.

V1 does not delete adopted resources, silently adopt unknown orphan machines,
or retarget a lease to a replacement instance. Those require a separate
explicitly admitted action/profile. A new instance gets a new resource and
lease identity. Pool selection is host scheduling; reuse additionally needs
an admitted same-owner scope and exact reset/materialization evidence. A
warm VM or retained session is not clean merely because its former client left.

## Lease admission and materialization

The allocator validates the request, atomically reserves its worst-case
required allowance, creates the durable allocation run, and records provider
dispatch intent before provisioning. Admission produces a lease with
`v: "openagents.environment-lease.v1"`, `lease` (fresh ID), `request`
(ArtifactRef), `allocator` (pubkey), `created_at`, `not_after`, `retain_until`,
and `reservation` (ArtifactRef). These bounds cannot exceed the request or
current host policy. The allocator signs it. The lease is not yet a ready
runtime or a transferable bearer capability.

Provider creation uses the binding's exact downstream idempotency identity.
If a create response is lost, record unknown allocation and reconcile it;
do not blindly create a second resource or infer failure from an empty list.
The resource ownership identifier must survive that reconciliation. If the
provider cannot support the required guarantee, refuse or use the explicitly
admitted weaker assurance in the binding. Never describe an uncertain cleanup
or expired request as confirmation that billing stopped.

A materialization has `v: "openagents.environment-materialization.v1"`:

| Field | Type and meaning |
| --- | --- |
| `lease`, `definition`, `resource` | Exact lease, requested definition, and realized resource descriptor ArtifactRefs. |
| `generation` | Fresh random runtime-generation ID. A process replacement uses a new generation even on the same resource. |
| `components` | Actual `{id, role, artifact}` entries, matched against the full required definition closure. |
| `runtime`, `configuration`, `model` | Effective runtime DefinitionRef, configuration ArtifactRef, and model descriptor ArtifactRef or null. |
| `workspace` | Realized WS binding/snapshot ArtifactRef or null. |
| `participants` | Nonempty array of the closed participant objects below, with unique keys. |
| `bootstrap`, `enforcement`, `evidence` | Bootstrap RUN result ArtifactRef or null, enforcement-plan ArtifactRef, and evidence ArtifactRefs. |
| `observed_at` | Host observation time. |

A participant is exactly `{principal, role, runtime, model, configuration,
assurance}`: public key; `executor`, `inference`, or `tool`; runtime
DefinitionRef; model descriptor ArtifactRef or null; configuration ArtifactRef;
and enforcement/provenance evidence ArtifactRef. Exactly one participant has
role executor and is the primary worker. All input recipients and effectful
participants must be enumerated and admitted by policy. Provider/account
binding evidence must establish what each key represents; an arbitrary proxy
key cannot conceal that another provider receives the content.

Requested/effective identities and disclosures must satisfy the pinned
requirements. Missing, incompatible, unknown-required, or substituted components
cannot produce readiness. A claim of opaque serving identity remains labeled
as such; requirements that demand stronger evidence refuse it. Dynamic mesh
routing to an undisclosed node is unsupported in this profile. Adding a node,
changing a model/configuration, or replacing the runtime requires a fresh
materialization generation and admission before further inputs/effects.
Discovery and peer liveness do not establish compatible capacity or grants.

The allocator retains bootstrap failures and partial resources as well as
successful materializations. Readiness means the required closure, bootstrap,
bindings, and enforcement were validated. It does not establish task success,
model quality, approval to publish, or acceptance of a market deliverable.

## Lease state and expiry

A state has `v: "openagents.environment-state.v1"`, `lease`, `revision`,
`previous`, `status`, `expires_at`, `last_activity_at`, `materialization`,
`attachment`, `accounting`, `cleanup`, and `record`. Lease is an ArtifactRef;
revision is a nonnegative monotone integer; previous is the preceding state
ArtifactRef, null only at revision zero. Status is `provisioning`, `ready`,
`reserved`, `attached`, `draining`, `released`, `failed`, or `unknown`.
Materialization and attachment are exact ArtifactRefs or null. Accounting is
the current reservation/usage receipt ArtifactRef. Cleanup is a cleanup receipt
ArtifactRef or null. Record is the exact allocator RUN transition from which
this state is derived; that transition does not contain a forward reference
to this state artifact. Initial materialization, attachment, and cleanup are
null. The allocator signs state; event replacement is only a retrieval hint.

The current effective use expiry is the minimum of `expires_at`,
`last_activity_at + idle_seconds`, and lease `not_after`. Expiry is enforced
before every new dispatch, and the host cancels/fences remaining execution as
specified by the admitted enforcement plan. Expiry transitions to draining,
not released. It does not prove that old processes stopped or that external
side effects reversed. Revision forks halt new attachment/dispatch and retain
both histories; they are not resolved by timestamp or relay arrival.

A control has `v: "openagents.environment-control.v1"`, `lease`, `command`,
`expected_revision`, `action`, `requested_until`, and `evidence`: lease
ArtifactRef, common command ID, current revision, `renew`, `release`, or
`status`, Unix seconds for renewal and otherwise null, and receipt ArtifactRefs.
The authenticated owner or an independently admitted controller may renew or
release; read authority permits status only. Renewal must arrive before the
current effective expiry, cannot pass `not_after` or `now + ttl_seconds`, and
must renew the required shared reservation. It updates last_activity_at to
the trusted admission time. An expired lease cannot be resurrected.

Lease admission initializes last_activity_at to created_at and expires_at to
at most `created_at + ttl_seconds`. New attachment activation or a valid
renewal updates activity; ordinary telemetry, observer traffic, an open socket,
or unauthenticated heartbeats do not. Hosts may impose stricter limits but
must record them. Explicit renewable use may keep an intentionally idle
resource alive within its allowance and hard deadline; it cannot be free.

## Binding an execution before dispatch

This section defines required CJ feature `openagents.environment-binding.v1`.
A worker that does not implement it MUST refuse the execute request. A worker
that does MUST authenticate a currently active exact attachment before dispatch.
Opaque `meta`, a workspace path, or a relay message arriving first is not a
substitute. LAB order binding and other required features remain independent
and must all be satisfied when composed.

An attachment request has `v: "openagents.environment-attach.v1"`, `lease`,
`expected_revision`, `materialization`, `worker`, `controller`, `generation`,
`execute`, `input_digest`, `claim`, and `reservation`. References are exact
ArtifactRefs except worker/controller public keys, RUN controller generation,
current lease revision, and Digest input_digest. Execute is the signed
execution declaration defined below. The CJ request includes the
environment-binding feature. Worker matches the primary
executor and CJ recipient; controller/generation match the current RUN/COORD
owner; input digest matches the referenced CJ input bytes. Claim/reservation
are current exact COORD artifacts with no scope or budget amplification.

An execution declaration has `v: "openagents.environment-execute.v1"`,
`caller`, `event`, and `body`: caller public key, exact CJ EventRef, and
ArtifactRef of the `openagents.execution.v1` execute plaintext. The caller
signs its declaration, and must equal the verified event signer and the
independently admitted task caller. The allocator validates that declaration
and plaintext under its own explicit disclosure grant. The primary worker
also decrypts the signed CJ event and verifies byte identity with the declared
body before admitting or dispatching it. An allocator without the worker
decryption key relies on the caller declaration and worker validation; it
cannot claim to have independently verified ciphertext/plaintext agreement.
Undisclosed participants need only the authenticated root identity and their
authorized projection, not permission to fetch the complete body.

An attachment grant has `v: "openagents.environment-attachment.v1"`,
`attachment` (fresh ID), `request`, `lease`, `materialization`, `execute`,
`claim`, `reservation` (ArtifactRefs), and `expires_at`. The allocator signs
it and reserves the lease's one execution slot atomically with the expected
revision check. The allocator deduplicates attachment reservation by `(lease, execute signer,
CJ request, attempt)` and retains both the complete execute fingerprint and
SHA-256 of JCS of the complete attachment request. An exact retry returns the
original grant even after later state changes. Changed materialization, claim,
reservation, expected revision, or other request bytes under that key refuse
as an identity conflict rather than reserving another slot. Grant expiry cannot outlast
effective lease expiry or CJ deadline. V1 permits one active or reserved attachment per lease; independently
owned simultaneous sessions require separate leases or a later partition profile.

The bootstrap order avoids a digest cycle and unbound work:

1. Construct and sign the exact CJ execute locally, then sign its execution
   declaration. The execute request need
   not contain its future attachment digest. Its required feature prevents a
   generic worker from executing it without that binding.
2. Authenticate the attachment request at the allocator. Durably reserve the
   exact materialization and execution slot, and obtain its signed grant.
3. Deliver the grant and signed execute to every listed participant through
   the registered participant-admission operation. This admission grants no
   execution yet; it verifies the exact closure, recipient projection, lease,
   current claims, enforcement, and input identity.
4. The allocator validates all required admissions and durably activates the
   attachment. Only then may the execute be dispatched to the primary worker.
   Every participating dispatcher checks the active attachment and current
   expiry/generation before its first and subsequent effects. It uses its
   trusted allocator/fencing connection and persisted monotone state, not an
   arbitrary relay head. If current required admission cannot be established,
   it stops new dispatch rather than inferring that no revocation exists.

Participant input has `v: "openagents.environment-participant-input.v1"`,
`attachment`, `participant`, and `projection`: grant ArtifactRef, exact listed
public key, and POL-authorized CTX input projection ArtifactRef. Projection
must preserve mandatory input and instructions for the participant's role;
it cannot change the root operation or disclose another participant's private
credentials. Each participant gets only its admitted data projection.

Participant admission has `v: "openagents.environment-participant-admission.v1"`,
`input`, `materialization`, `participant`, `status`, `root_input_digest`,
`projection_digest`, `enforcement`, and `reason`. Input and materialization
are ArtifactRefs; participant is its signer; status
is `admitted` or `refused`. An admitted result requires non-null enforcement
ArtifactRef, `root_input_digest` (the exact root CJ input Digest), and
`projection_digest` (the exact participant projection Digest), with null reason.
A refusal requires a common refusal code; any of those three evidence fields
that could not be validated must be null, not invented. An unreadable or
malformed grant can refuse at the CJ boundary before producing this result.
The primary worker verifies the original CJ input bytes and their digest.
Other participants authenticate the signed root identity, verify their own
projection bytes, and validate its authorized source linkage; they do not claim
to have inspected undisclosed root content. Observers without access to the
original ciphertext rely on these attributed validations, not a claim of
direct decryption.

Attachment control has `v: "openagents.environment-attachment-control.v1"`,
`attachment`, `command`, `expected_revision`, `action`, and `evidence`:
grant ArtifactRef, common command ID, current lease revision, `activate` or
`drain`, and receipt ArtifactRefs. Activation requires one authenticated
admitted result for every participant, current compatible grants, and an
unexpired reserved lease. Drain requires the owner, admitted controller, or
allocator under its pinned lifecycle policy. Both use durable compare-and-swap.
The same input retry returns its original receipt; conflicting reuse refuses.

Lost acknowledgments are reconciled by exact IDs. A caller cannot reserve a
second attachment because it failed to observe the first. A participant or
allocator crash after possible dispatch leaves unknown effects and capacity
holds. Any change to the execute fingerprint, including a new CJ attempt,
requires a new attachment after the previous one is fenced and reconciled.

## Detachment, cleanup, and accounting

An attachment authorizes only its exact admitted CJ execution. A persistent
SESS engine may outlive several sequential execution attachments, but its
lease/materialization identity is not authority for the next turn. A new native turn or queue promotion
requires its own current operation admission and execution binding. Session opening inputs and pre-execution WS snapshots
may reference a ready lease/materialization or prior independent evidence;
they MUST NOT reference their own future attachment. Post-activation admission
receipts or new workspace revisions may retain that attachment without changing
the already-signed CJ input.

The execution slot does not serialize host control behind the work it must
control. Separately admitted allocator/session-host lifecycle, observation,
queue admission, and exact active-turn steer/cancel operations need no second
compute attachment. They name the current session/turn/binding and cannot
start another turn, add recipients, widen effects, or increase allowance. The
worker applies supported steering at its admitted safe boundary under the
existing active execution; SESS/POL retain the supplemental input and authority
without rewriting the original signed execute. A control that actually starts
new execution requires a fresh attachment.

Client disconnection is observational. It neither drains the attachment nor
extends the lease. Transcript restoration, a provider resume token, or a
replay cursor does not prove that an interrupted effect can safely rerun.
SESS governs provider-session continuity; RUN governs task authority and
unknown effects. A stopped process is not automatically a fully restored turn.

Draining blocks new dispatch, requests cancellation of managed work, and
obtains fencing/termination evidence from every participating effect dispatcher.
Only reconciled, fenced attachments release their execution slot. If required
participants are unreachable, retain unknown attachment and resource holds.
A lease may return to ready only while still unexpired, under the same exact
materialization, with compatible resource state. Mutation or changed runtime
identity requires a fresh materialization before attachment. Handoff to another
task controller additionally follows RUN; an allocator cannot appoint it.

A cleanup receipt has `v: "openagents.environment-cleanup.v1"`, `lease`,
`resource`, `attachment`, `action`, `status`, `fencing`, `retained`, `provider`,
`accounting`, and `reason`. Lease/resource are exact ArtifactRefs; attachment
is an ArtifactRef or null; action is `destroy` or `detach`; status is
`pending`, `confirmed`, `failed`, or `unknown`; fencing and retained are arrays
of receipt/artifact references; provider is an external confirmation receipt
ArtifactRef or null; accounting is a receipt ArtifactRef; reason is bounded
text. It is signed by the allocator. Confirmed requires all applicable fencing,
retention, and provider-confirmation evidence; null provider is allowed for a
local detach that has no provider mutation and has complete local evidence.

Before destroying a resource, retain the required trace, input/lock closure,
outputs, dirty workspace evidence, and provider receipts according to the
pinned policy. The retained closure must remain accessible outside the resource
being destroyed; references to files that the same deletion removes do not
satisfy retention. If retention fails, cleanup remains pending or failed unless
the owner separately authorized an explicit loss policy; retain the loss
record. Never silently delete the only unpushed output or its provenance.
Use the exact account, resource, generation, and ownership evidence for every
cleanup operation. Broad label-based sweeps are not an ownership check.

Released requires confirmed cleanup. Failed provisioning can still have a
live chargeable resource and pending cleanup; report that obligation explicitly.
A destroy request, provider timeout, local process exit, stale PID file, or
lease expiry is insufficient confirmation. Retain unresolved resources until
reconciliation or an explicitly admitted residual-liability disposition.
A billing estimate, relay deletion, or artifact expiration does not settle it.

All allocation, bootstrap, active, idle, data transfer, retention, and cleanup
usage belongs to the admitted parent allowance, including failed and cancelled
work. Keep measured, provider-declared, estimated, and unknown costs distinct.
Account retries and distributed participants without double-counting one
receipt or omitting shared host costs. Reserve enforceable worst-case limits
before admitting effects; otherwise refuse the hard-bound request. A lease
is not a wallet authorization, commercial order, escrow, or automatic payout.
Market terms and Bitcoin settlement use supported [MKT](NIP-MKT.md) profiles.

An operation result has `v: "openagents.environment-result.v1"`, `input`,
`status`, `lease`, `state`, `value`, and `reason`. Input is the exact request
ArtifactRef; status is `accepted`, `duplicate`, `conflict`, `refused`, or
`unknown`; lease/state/value are ArtifactRefs or null. Accepted/duplicate
require lease and authoritative state; value carries an attachment grant for
attachment reservation and is otherwise null. Conflicts/refusals require a
common refusal reason. The configured allocator signs the result. CJ completion
means the lifecycle operation answered, not that provisioning or cleanup finished.

## Example and conformance

A coding agent requests a pinned runtime plus a WS repository snapshot. The
allocator records its reservation, creates the resource once, runs its exact
bootstrap, and signs the effective materialization. The controller signs an
execute requiring environment binding, reserves one attachment, obtains the
participants' admission receipts, and activates it before dispatch. A browser
client can disconnect while the bounded turn runs. On completion, WS captures
its output and RUN retains outcomes. After every dispatcher is fenced and
required evidence retained, provider confirmation can release the lease.
A provider timeout leaves unknown cleanup and charge exposure visible.

Required cases include:

1. Concurrent creates/retries preserve one logical request; a lost provider
   response creates an unknown allocation, not an automatic second machine.
2. Two attachment requests race for one lease: only one reserves the slot.
   A signed CJ execute arriving first refuses or waits without dispatch.
3. Renewal after expiry, stale revisions, unsigned heartbeats, and forged
   ownership labels cannot renew, adopt, delete, or retarget a resource.
4. A participant, model, runtime, configuration, workspace, or requested
   enforcement differs: readiness/attachment refuses until fresh admission.
5. Disconnect, replay, and session resume do not transfer task control or
   authorize re-execution of an unknown effect.
6. Bootstrap failure, missing artifacts, unreconciled charges, and unreachable
   dispatchers preserve resource/accounting obligations through expiry.
7. A changed mesh recipient cannot receive data under an old materialization;
   a provider compatibility claim without required evidence refuses.
8. Unknown cleanup stays distinct from released; adopted resource release
   detaches and never silently destroys its owner's machine.

Conformance names allocator, materializer, participant, worker, and observer
roles separately. Supporting `3188`, an OpenAI-compatible endpoint, ACP, or SSH
alone supplies none of these guarantees. Provider-specific allocation/reset
algorithms remain host implementations; the typed identities, authority,
receipts, and failure states are the interoperable contract.
