# NIP-CJ — Agent jobs

`draft` `optional` — v1. The [shared contracts](contracts.md) are normative.

This NIP defines encrypted conversation, typed-decision, and recoverable
execution jobs. Tasks are domain-independent: document, research, coding,
and business operations use the same transport with domain schemas and
host admission. A relay transports requests; it grants no execution authority.

## Families and transport

| Family | Request/control | Result/control answer | Feedback | Payload version |
| --- | --- | --- | --- | --- |
| Conversation | `25900` | `26900` | `27000` | Integer `1`. |
| Decision | `25910` | `26910` | `27010` | `openagents.systemone.v1`. |
| Execution | `25920` | `26920` | `27020` | `openagents.execution.v1`. |

All kinds are ephemeral. Relays fan them out to matching subscriptions and
MUST NOT retain them as durable job state. Execution recovery uses durable
worker state and [RUN](NIP-RUN.md). Each family has its own parser; a handler
MUST reject another family's kind or payload rather than reinterpret it.

Every request has exactly one `p` naming the worker. Every response has
exactly one `p` naming the caller and one `e` naming the exact request or
control event it answers. Content is NIP-44 v2 encrypted to that recipient.
Bodies contain required `v` and `requires`, plus optional inert `meta`.
The caller subscribes before publishing. An optional NIP-40 `expiration`
limits delivery; where a body declares a deadline they MUST agree.

Verify NIP-01 event ID/signature, kind, signer, recipient, and request binding
before accepting a payload. Deduplicate verified event IDs. An unsigned
subscription label cannot identify a job. NIP-42 authenticates a relay
connection, not a forwarded event's execution authority. The verified request
signer maps to host principal/tenant policy outside the payload. Payloads
contain no bearer credential or self-asserted grant.

Workers bound input bytes, output bytes, active jobs, spend, and elapsed time.
They validate request freshness under a declared skew/window policy. Bodies
with missing/unknown versions or features refuse. Display strings are data;
clients must escape terminal controls and active markup.

## Conversation jobs

A request contains `v: 1`, `requires`, `task` (string), `transcript` (ordered
array of `{role, content}`), and optional `instructions` and `client` strings.
Role is `user` or `assistant`; content is a string. Both caller and worker
bound the transcript. Instructions are caller-supplied guidance and cannot
override host policy. Client is informational and conveys no authority.

Feedback has `v: 1`, `requires`, `type`, and fields for that type:

| Type | Fields |
| --- | --- |
| `judgment` | `verdict`: `respond`, `clarify`, `end_conversation`, or `unrouted`; `line`: bounded display string. It is an optional observation, not permission. |
| `partial` | `seq`: nonnegative integer starting at zero; `delta`: string. |
| `status` | `status`: `queued`, `processing`, or `error`; error requires `code` and `message`, with optional nonnegative `retry_after_ms`. |

Workers increment `seq` once per emitted partial. After event-ID deduplication,
a client renders only the next contiguous sequence number. A gap, repeat with
a different event ID, or out-of-order partial ends incremental rendering;
the client waits for the complete result. Feedback never proves completion.

A result has `v: 1`, `requires`, `type: "result"`, nonempty `text`, optional
`usage: {input, output}` (nonnegative token counts), and optional `model`
(nonempty identifier). Missing usage is unknown, not zero. A model name is
an attribution claim, not proof of immutable weights. The first valid result
or terminal error ends observation; subsequent events do not change it.

Conversation jobs have no durable retransmission identity. Another request is
a new invocation. A dropped socket does not stop remote work. Effects requiring
recovery, cancellation, or exact implementation attribution use execution jobs.

## Typed decision jobs

A request has `v: "openagents.systemone.v1"`, `requires`, `type: "systemone"`,
`request` (common random ID), `attempt` (positive integer), `model` (nonempty
host-admitted target identifier), `state` (string), `questions`, and `deadline`
(Unix seconds). Questions is a nonempty object keyed by unique nonempty IDs.
Each question has `type`, `instructions` (string), and the following fields:

| Type | Input | Answer |
| --- | --- | --- |
| `noul` | No additional fields. | `{type: "noul", noul}` with a finite probability in `[0,1]`. |
| `choice` | `criteria`: nonempty map of unique option IDs to description strings. | `{type: "choice", choice, confidence, probabilities}`; selected option belongs to criteria, and probabilities names exactly those options. |
| `score` | `criteria`: ordered nonempty array of level description strings. | `{type: "score", score, confidence, legend, probabilities}`; legend maps zero-based decimal-string indexes to the requested descriptions; probabilities maps zero-based level indexes, encoded as decimal strings, to probabilities; score is their probability-weighted index. |

All probabilities and confidence values are finite in `[0,1]`. Categorical
probabilities sum to one within absolute tolerance `0.000001`. Choice confidence
is the selected option's probability; select a maximum-probability option and
break ties by lexicographic option ID. Score confidence is the largest level
probability; score is within `0.000001` of the weighted index. This transport
does not certify calibration or turn a probability into permission. Consumers
pin any abstention, threshold, and interpretation policy separately.

The same state supplies every question; one question does not consume another
answer from the same request. Dependent questions require separate calls.
A result contains `v`, `requires`, `type: "result"`, request/attempt, common
`outcome`, `dispatched`, `response`, `receipt`, and `code`. Receipt is an
ArtifactRef to the shared execution receipt. On `completed`, response contains
`model`, `answers` keyed exactly as requested, and `usage` (ArtifactRef or null),
and code is null. On other outcomes, response is null and code is a bounded
cause string or null when no more specific cause is known. Answer types must
match the questions. Refusals, transport failure, and model answers are distinct.

Progress is `type: "status"` with request/attempt and status `queued`,
`processing`, or `error`. Error includes code/message and optional
`retry_after_ms`; it is not evidence that an admitted call incurred no cost.
The caller obtains the final receipt or retains an unknown outcome.

The idempotency key is `(worker, principal, request, attempt)`. A fingerprint
covers JCS of the complete request body. Retransmission with the same key and
fingerprint retrieves recorded state without a second charge or model call;
changed content is `idempotency_conflict`. Reserve quota and persist admission
before dispatch. A new permitted attempt increments attempt under the same
request, after preceding uncertain spend/execution is reconciled. The worker
publishes its supported retention horizon; outside it, absence of state cannot
justify automatic replay. Use execution jobs when a required recovery horizon
or model-call composition is part of the task.

Cancellation uses `type: "cancel"`, the same version/features, request/attempt,
and an `e` tag naming the request. Require the original signer. Before dispatch,
resolve cancelled with `dispatched: false`; after dispatch, propagate a stop
request and preserve unknown effects or usage until reconciliation. Receipt
of a cancel control is not proof of stop.

Refusal causes include shared codes and `busy`, `quota_exhausted`,
`rate_limited`, and `uncalibrated`. Workers admit model targets, recipients,
capacity, and budgets under policy; a client naming a model does not authorize
its use. Only supported families and roles may be advertised.

## Execution jobs

This family carries an admitted operation or
program, typed task input, context references, and durable outcomes. It does
not use conversation text as an executable command or require an LLM to select
a program. CAP/PRG/EXT/RUN and the shared contracts are normative for this
family. Each handler MUST validate its own kind and schema before admission.

| Kind | Name | Direction |
| --- | --- | --- |
| `25920` | Execution request or control | Caller → worker |
| `26920` | Execution result or control answer | Worker → caller |
| `27020` | Execution admission/progress | Worker → caller |

These are draft OpenAgents assignments and are ephemeral. Durable state lives
in the worker and optional NIP-RUN retention service. A client subscribes before
publishing. Requests have exactly one `p` worker; responses have exactly one
`p` caller and one `e` for the request/control event they answer. Payloads are
NIP-44 v2 encrypted. Verify kind, signer, recipient, and exact request binding
before interpreting content; NIP-42 connection identity is not a forwarded grant.

### Execute payload

The body contains `v: "openagents.execution.v1"`, `requires`, `type: "execute"`,
`request`, `attempt`, `run`, `target`, `lock`, `input`, `context`,
`requirements`, `bounds`, `deadline`, `retain_until`, and optional `parent`.

[NIP-LAB](NIP-LAB.md) defines the required feature
`openagents.labor-binding.v1` for a commercially bound execution. A worker
that does not implement that feature refuses it as `unsupported_feature`.
A LAB worker validates and durably binds the separately authenticated order
linkage before dispatch. The marker grants no authority and does not add a
new job family, executable prompt convention, or implicit order field.

[NIP-ENV](NIP-ENV.md) similarly defines required feature
`openagents.environment-binding.v1`: an exact execution attachment must be
reserved, admitted by every required participant, and activated before
dispatch. A generic CJ worker refuses this feature if unsupported. The
attachment references an already signed execute request; the request need
not refer to its future attachment. LAB and ENV checks are independent, and
an execution requiring both must satisfy both before any effects.

| Field | Meaning |
| --- | --- |
| `request` | Random logical request ID stable across retries. |
| `attempt` | Positive integer; retransmission preserves it, a permitted new attempt increments it. |
| `run` | Logical remote run ID, distinct from parent run ID. |
| `target` | Exact operation, program, or OPT AI implementation DefinitionRef. |
| `lock` | ArtifactRef of the complete dependency lock. |
| `input` | Schema-valid bounded typed value, or `{artifact: ArtifactRef}` when the target schema specifies artifact input. |
| `context` | ArtifactRef of a recipient-specific context manifest. |
| `requirements` | ArtifactRef of requested effects, assurance, and disclosure constraints; not a grant. |
| `bounds` | Whole-attempt ceilings under the caller's parent reservation and worker policy. |
| `deadline` | Required Unix-second latest completion time; mirror in `expiration`. |
| `retain_until` | Required recovery horizon beyond deadline, subject to worker admission. |
| `parent` | Optional `{run, step, iteration, attempt}` attribution; no inherited authority by assertion. |

The request signer maps to the worker's principal/tenant policy independently
of payload claims. Admission resolves the target/lock, validates context
recipient and source scope, intersects authority, verifies enforceability,
reserves quota, and durably claims the pair before any effect. Missing or
unavailable referenced content refuses under bounded fetch policy. Credentials
and local absolute paths are never supplied as portable authority.

The worker validates `created_at` against its documented freshness/skew window
and requires an unexpired deadline. An `expiration` tag that differs from the
deadline is malformed. Retention must extend beyond the deadline. A deadline
is enforced by the worker as well as checked by the relay; NIP-40 expiration
does not terminate a subprocess. Valid retransmissions retrieve known state
without dispatching again, even if execution's deadline has since passed.

### Identity, admission, and retransmission

The idempotency key is `(worker, principal, request, attempt)`. Its fingerprint
is SHA-256 of JCS(the complete execute body), including target, input, lock,
context, requirements, bounds, deadline, and retention horizon. Different
transport events with the same key and fingerprint are retransmissions:
return the recorded admission/result and do not reserve or execute again.
Changed content under the same key is `idempotency_conflict`.

The worker binds a request to one run and monotone attempt sequence. A new
attempt is admitted only after reconciling the preceding outcome and applying
the target's retry contract. No automatic retry of an unknown effect is
permitted. Resending to a different worker has no cross-worker deduplication
guarantee and requires explicit reconciliation/admission.

Before a `27020` `type: "accepted"` response, persist the admitted claim,
enforcement/reservation plan, and NIP-RUN root. Accepted includes `request`,
`attempt`, `run`, `input_digest`, `lock_digest`, `record` (exact encrypted
NIP-RUN EventRef or retained record ArtifactRef), `mailbox`, `retain_until`,
and `controller`. It binds to the execute event with `e`. The worker may refuse
an unsupported retention request; it MUST NOT silently promise a shorter one.

A relay `OK` is delivery admission only, not worker acceptance. Missing
accepted feedback does not prove that the worker did nothing. The worker
records dispatch intent before dispatch; crash after intent is unknown until
reconciled. Claim/dispatch storage and fencing enforce at-most-once dispatch
for a known attempt where supported; the protocol does not promise exactly
once effects across crashes or external systems.

### Progress and results

`27020` progress has version/features, `type: "progress"`, request/attempt/run,
`seq`, and `status` (`queued`, `running`, or `reconciling`). Sequence is a
monotone progress counter, separate from the authoritative run journal.
Gaps stop incremental rendering until status/replay; progress never establishes
completion. Optional view/evidence ArtifactRefs remain recipient-scoped.

`26920` `type: "result"` includes request/attempt/run, common `outcome`,
`dispatched`, `output` (typed value or null), `artifacts`, `receipts`,
`verification`, `integration`, and latest `record` reference. A refusal before
dispatch also contains a typed `code` and `message`. Refusals distinguish
unsupported semantics, permission, stale inputs, limits, busy capacity,
revocation, unavailable content, and identity conflict. Unknown spend is null,
never zero. Decision subcalls retain separate execution receipts.

Persist results before reporting them as recoverable. One logical terminal
result can be delivered repeatedly, bound to each retransmission's event ID;
clients deduplicate by request/attempt and verified outcome identity. Conflicting
terminal results require reconciliation, not first-arrival selection. A result
may contain only artifacts and no prose; nonempty conversation text is not a
requirement here. Execution completion does not imply verified acceptance.

### Status, replay, and cancellation

Controls use kind `25920`, the same version/features, and `type` of `status`,
`replay`, or `cancel`. They contain request/attempt/run and one `e` referring
to an accepted execute event. `replay` additionally contains `after_seq` (null
for the root) and `max_records`; `cancel` contains a bounded reason.
Require the original caller or an independently authorized control principal;
knowledge of run ID or mailbox is not authority.

[NIP-CTRL](NIP-CTRL.md) defines one such independently admitted client role.
Its task rights do not transfer the original caller's execution or spending
authority. Apply its current grant and revision checks before control admission.

Controls never create new execution. The worker answers with `26920`, `e`
bound to the control event, type `status_result`, `replay_result`, or
`cancel_result`, and the same logical identities. Status includes the current
state and latest record/result reference. Replay includes ordered retained
record references, `next_seq`, and `complete`. Gaps, truncation, or expired
retention are explicit. A missing/expired record answers `unknown` or
`content_unavailable`, not proof of unattempted work.

Cancellation persists `cancel_requested`, prevents queued dispatch, propagates
to children/supervised processes, and reports confirmed outcome or unknown.
Before dispatch it may resolve `cancelled` with dispatched false; after
dispatch it MUST preserve evidence of effects and unresolved accounting.
`cancel_result` acknowledges the control, not guaranteed stop. Late outcomes
remain available for reconciliation even when the UI stops displaying them.

Workers retain idempotency state/results or tombstones through `retain_until`.
An expired/stale execute event is refused and MUST NOT recreate forgotten
work. A new attempt after expiry requires explicit reconciliation policy;
absence of a tombstone is not permission to repeat an effect.

### Optimization and compiled execution

An execution target may be an [OPT](NIP-OPT.md) AI implementation or a
registered optimization, materialization, or evaluation operation. Its typed
input pins study/candidate/trial references; its lock pins the complete
functional closure. The worker verifies those identities before measuring or
attributing a result and returns materialization and evaluation artifacts.
A generic conversation response cannot substitute for that evidence.

The same admission, disclosure, reservations, retries, and durable recovery
apply to proposal, reflection, student, and judge work. New candidate bytes
require a new candidate/trial identity; retransmission cannot hot-swap them.
No dedicated optimizer job family or arbitrary code payload is required.

## Conformance

Required cases cover family cross-delivery, malformed schemas, sequence gaps,
signer/principal binding, wrong answer keys/types, invalid probability mass,
repeated/conflicting fingerprints, lost admission/results, crash boundaries,
restart/status/replay, retention expiry, cancellation races, stale inputs,
unknown spend, duplicate/forked results, unauthorized controls, candidate
substitution, and unbound reflection calls. Relay fanout alone cannot establish
worker execution, materialization, or recovery conformance.
