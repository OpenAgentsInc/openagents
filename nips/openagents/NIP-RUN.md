# NIP-RUN — Durable runs and evidence

`draft` `optional` — v1.
The [shared contracts](contracts.md) and [NIP-CJ](NIP-CJ.md) define payload
identity and remote admission. Nostr transports attributable records; it does
not provide a transactional job lock or exactly-once effects.

Runs are domain-independent. A task/base identity names the admitted input
state or external observations; it need not be a Git commit. Domain adapters
define confirmation and reconciliation evidence. Receipt of an API response,
acceptance of a proposal, verification, and the actual external effect remain
distinct facts, including when the effect is irreversible.

## Kinds and privacy

| Kind | Class | Record |
| --- | --- | --- |
| `3187` | Regular | One encrypted durable run record for one recipient. |
| `30186` | Addressable | Encrypted current-head hint for that recipient. |

Each has exactly one `p` recipient, one `h` random 64-hex mailbox, and
`t: oa:run:v1`. A head has `d` equal to the mailbox. The mailbox is generated
per run/recipient; it is not a repository name, path, or plaintext run ID.
Content uses NIP-44 v2 to that recipient. These tags reveal timing, traffic
volume, and correlation; encryption does not conceal that metadata.

A conforming relay MUST restrict all reads, ID lookups, COUNT, and fanout to
the authenticated author or exact recipient and exclude content from search.
It validates envelope and ciphertext bounds without claiming to validate the
encrypted record. Publication authenticates the author through NIP-42;
application authority follows the verified event signer and pinned run policy.

Clients MAY maintain the same records locally without publishing them. A
remote durable profile requires an explicitly agreed retention service or
worker-side store, not a hope that an arbitrary relay retains events forever.

## Record identity

Decrypted content is `{v: "openagents.run-record.v1", requires: [], record,
digest}`. Digest is SHA-256 of the UTF-8 schema identifier
`openagents.run-record.v1`, one LF byte, then JCS(record), with the common
Digest prefix. The record
contains `run`, `seq`, `previous`, `controller`, `generation`, `type`,
`subject`, `time`, and `data`.

- `seq` starts at zero and increases by one; `previous` is null at zero,
  otherwise the previous record Digest.
- `controller` is the exact author pubkey, `generation` begins at zero,
  and event signer MUST match the controller authorized for that sequence.
- `subject` is `{step, iteration, attempt}` with null step/iteration for a
  run-level record. Attempts begin at 1; null attempt means run-level state.
- `time` is observational Unix seconds. Sequence and chain determine order;
  timestamp order does not establish authority.

The same logical record can be encrypted separately to authorized recipients.
Its Digest is stable while Nostr event IDs differ. Record digests remain inside
ciphertext. Deduplicate signed transport events and logical records separately.
Two different records at one sequence or predecessor form a fork: report
`conflict`, halt automatic recovery/dispatch, and retain both. Never select a
winner by relay arrival or `created_at`.

## Record types

| Type | Required data |
| --- | --- |
| `created` | Owner pubkey, admitted request identity, task/base identity, program/operation DefinitionRef, lock and context ArtifactRefs, policy/grant references, parent run reference or null, and recipient set. |
| `admitted` | Effective enforcement plan, shared reservation identities, deadlines, retention policy, and admitted input digest. |
| `dispatched` | Exact binding/component, attempt identity, input/context digests, effect identity, and fencing generation. |
| `observed` | Bounded evidence descriptor or ArtifactRef, source versions, capture completeness, and provenance. |
| `resolved` | Common outcome, dispatched boolean, output/artifact references, receipt references, known/unknown usage, verification, and integration. |
| `cancel_requested` | Authorized requester reference, target scope, and reason. |
| `unknown` | The unresolved attempt/reservation and last known evidence; no fabricated success or rollback. |
| `reconciled` | Previous unknown record, supporting evidence, resolved outcome, and accounting disposition. |
| `handoff` | New controller, next generation, and dispatcher fencing acknowledgment reference. |
| `settled` | Final run outcome, outstanding unknowns, verification/integration, result references, and retained reservation disposition. |

Data objects use the named fields above; versioned artifact schemas define
their contained evidence. Unsupported required data semantics refuse. A
record may reference an immutable payload artifact when it exceeds wire size;
its identity, access scope, and retention are part of the run's closure.

`created` is sequence zero and establishes the owner/controller relationship
only when admitted under independently verified local policy or a CJ request
signed by that owner. A self-published record cannot appoint itself controller
of another principal's run. The owner/authorized recipients are fixed at
creation; sharing later creates explicit scoped copies under owner policy,
not permission inherited from a guessed mailbox.

## State transitions

Per subject, `created/pending -> admitted -> dispatched -> resolved` is the
normal path. A pre-dispatch refusal/cancellation can resolve with
`dispatched: false`. Once dispatched, failure to observe a result is unknown,
not evidence that no effect occurred. `cancel_requested` does not itself
resolve work. Unknown can become reconciled only with attributable evidence.
No subject is dispatched after a terminal resolution without a new admitted
attempt, and no new work is appended after `settled`.

A run may settle with unknown outcomes only when it declares them and retains
unresolved reservation disposition; it cannot report verified success. Later
investigation of a settled unknown run is a separate linked reconciliation
record set/run, not a rewritten terminal record. Child completion does not
establish parent verification or integration acceptance.

The controller MUST durably record admission/reservation before acknowledging
acceptance, and dispatch intent before the effect. It durably records observed
outcomes before advertising them as recoverable. Relay acknowledgment alone
does not prove that the controller persisted state or that a worker accepted
work. A crash between dispatch intent and observation produces unknown work
for reconciliation; this contract makes no exactly-once execution claim.

## Controller handoff and fencing

One controller serializes a run's authoritative journal. Workers and reviewers
return signed results/receipts that it references; they do not concurrently
append independent authoritative branches. Local trajectory output and worker
telemetry may have different writers and are linked rather than merged by time.
A record may cite Block events in `data.block_refs` without copying their
content or changing their kinds. Kind 24200 agent telemetry is ephemeral, so
a citation sets `durable` to false and is not execution state. Kind 44200
turn metrics and kind 30174 engrams may be cited as durable references by
event id. A durable citation of kind 24200 is refused.

Handoff requires the old controller to stop new dispatch, obtain a fencing
acknowledgment from every effect dispatcher that can still act, then append
`handoff`. The next record is signed by the new controller with incremented
generation and the handoff digest as predecessor. Dispatchers persist the
generation and reject older generations. A signed handoff without actual
dispatcher fencing is insufficient. Unreachable unfenced workers stay unknown.

If the old controller cannot participate, automatic takeover requires a
separately trusted shared fencing authority already bound at admission.
Absent that authority, require explicit reconciliation; do not elect a new
controller from whichever signed event arrives first. Revocation and
owner-agent attestations do not by themselves terminate old processes.

## Heads, replay, and retention

A `30186` head body is `{v: "openagents.run-head.v1", requires: [], run,
seq, digest, generation, record: EventRef, retained_from, retain_until}`.
It is a retrieval hint, not the journal. The client verifies the chain,
controller transitions, and any cached higher sequence before accepting it.
A head from an old generation cannot erase later history. Under handoff,
the new signer has a different address; the verified handoff identifies it.

Reconnect retrieves stored records using the private mailbox and requests a
worker status snapshot through CJ. Missing sequences require bounded replay
or an explicit incomplete result. EOSE, an empty response, or the absence of
a newer head cannot prove that no later record exists. A retention service
states its horizon; after expiry clients report unavailability rather than
infer completion or safely repeat an effect.

Keep exact definitions, locks, evidence references, and receipts under explicit
retention limits. Evidence deletion can leave a valid record whose payload is
unavailable. Record this condition. Deletion from one relay is not proof of
global erasure. Sensitive artifacts use separate scoped access; a hash or URL
is not a bearer grant.

Offline replay verifies retained records and reconstructs supplied context
without running tools or inference. Re-execution is a new authorized attempt.
CTX task frames/context builds, POL approvals/routes, COORD claims/findings,
and EVAL reports are typed artifacts linked by these records. A participant's
separately signed private artifact uses the shared `3188` envelope; receiving
it does not append an authoritative transition or activate a background job.
The controller validates its authority and freshness before recording adoption.

## Optimization lineage and immutable execution

An [OPT](NIP-OPT.md) study uses admitted runs for proposing, building,
materializing, evaluating, and confirming candidates. Their typed input and
observed artifacts bind study, candidate, trial, implementation, target,
and materialization identities. The controller validates those records before
adopting them into its journal. An optimizer cannot declare itself the
controller of another principal's evaluation or deployment.

Proposal completion, successful materialization, evaluation outcome, candidate
selection, and operator promotion are different observations. They MUST NOT
be collapsed into a single successful run. A candidate that builds but fails
evaluation remains a completed build with failed quality evidence.

Recovery preserves frozen study inputs, exposure/confirmation consumption,
candidate pins, and outstanding reservations. Resume does not rerun unknown
effects, release uncertain spending, reset a confirmation allowance, or select
new bytes under the same attempt. Active runtime work and optimization work
have separate locks; adopting a new implementation affects only new admissions.

## Conformance

Required cases include duplicate/out-of-order delivery, chain gaps, forged
owners/controllers, recipient isolation across all query surfaces, forks,
stale heads, handoff with missing fences, crash at every effect boundary,
unknown reservations, late cancellation/results, replay after retention expiry,
and verified output separated from integration. Backing storage must enforce
atomic claims and durable transitions; a collection of signed events alone
is insufficient.

Advertise `nip-run-v1` only when configured durable retention, envelope
validation, privacy, retrieval, and client journal verification are proven for
the advertised role. Relay storage does not claim to implement a controller.
