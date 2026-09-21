# NIP-CJ — Coder Jobs

`draft` `optional`

A family of request/response protocols carried over Nostr. Conversation jobs
retain their existing integer versions, decision jobs retain the System One
contract, and the new execution family is `openagents.execution.v1`. The
families have distinct kinds and MUST NOT reinterpret each other's payloads.

The relay transports encrypted traffic and does not authorize execution.
Conversation/decision traffic remains ephemeral. Execution jobs require durable
worker admission and [NIP-RUN](NIP-RUN.md) records; a relay may retain those
opaque records under that separate profile without becoming the scheduler.
The [shared contracts](contracts.md) apply to the new execution family, not
as a silent rewrite of deployed conversation or decision payloads.

NIP-90 reserves a similar shape (`5xxx`/`6xxx`/`7000`) but upstream marks it
unrecommended, and its plaintext `i`/`output` tags and payment flow do not
fit encrypted terminal sessions. This NIP defines its own kind family.

## Kinds

All three kinds are ephemeral (`20000`–`29999`): relays fan them out to
open subscriptions and never persist them. Delivery depends on the connected
sockets. Closing a socket abandons observation; it does not prove that remote
generation or an executor stopped.

| Kind | Name | Direction |
| --- | --- | --- |
| `25900` | Job request | Terminal → worker |
| `26900` | Job result | Worker → terminal |
| `27000` | Job feedback | Worker → terminal |

A second, proposed family carries decision jobs on kinds `25910`,
`26910`, and `27010`; see Decision jobs below. The families never share a
kind, so a worker that speaks only one can never receive the other's
payloads.

## Job request — kind `25900`

Published by the terminal to ask the worker for one turn of conversation.

```jsonc
{
  "kind": 25900,
  "pubkey": "<customer x-only public key, hex>",
  "content": "<NIP-44 payload encrypted to the worker pubkey>",
  "tags": [
    ["p", "<worker x-only public key, hex>"],
    ["expiration", "<unix seconds>"]   // optional NIP-40 guard
  ]
}
```

The `p` tag names exactly one worker. The decrypted content is a JSON
object:

```jsonc
{
  "v": 2,
  "task": "the user's draft text",
  "transcript": [
    { "role": "user", "content": "earlier turn" },
    { "role": "assistant", "content": "earlier reply" }
  ],
  "instructions": "the system prompt for the turn",
  "client": "coder 0.1.0"
}
```

- `v` is the payload version; see Versions below.
- `transcript` is the bounded conversation so far, oldest first. The
  terminal bounds it; the worker bounds it again.
- `instructions` is the system prompt for the turn; it may be absent.
- `client` is informational.

## Job feedback — kind `27000`

Published by the worker, `e`-tagged to the request it answers, `p`-tagged
to the customer. Any number may precede the result.

```jsonc
{
  "kind": 27000,
  "pubkey": "<worker pubkey>",
  "content": "<NIP-44 payload encrypted to the customer pubkey>",
  "tags": [
    ["e", "<job request event id>"],
    ["p", "<customer pubkey>"],
    ["status", "processing"]          // optional coarse marker
  ]
}
```

The decrypted content is a JSON object with a `type` discriminator:

```jsonc
{ "v": 2, "type": "judgment", "verdict": "respond",
  "line": "respond 1.00 · conf 1.00 · risk 0.0 · prog 0.8 · code 0.03" }

{ "v": 2, "type": "partial", "seq": 0, "delta": "The borrow checker" }

{ "v": 2, "type": "status", "status": "queued" }

{ "v": 2, "type": "status", "status": "error",
  "code": "quota_exhausted", "message": "free allowance used" }
```

- `judgment` — the classification verdict the worker computed for the
  turn. `verdict` is one of `respond`, `clarify`, `end_conversation`,
  `unrouted`. `line` is a display-ready one-line summary; terminals render
  it verbatim.
- `partial` — one streaming text delta of the answer. Under version 2 it
  carries `seq`: the count of partial events the worker published before
  this one in the same job, starting at `0`. A terminal displays a delta
  only when `seq` equals the count it has already accepted, and the first
  delta that is not next — early, late, or repeated — ends the stream:
  nothing is buffered and nothing after it is rendered. A repeated delivery
  of the same event ID is ignored before checking the sequence. The result
  carries the full answer regardless of what the stream dropped.
- `status` — coarse state or a terminal error. `error` ends the job;
  `code` is a machine-readable reason (`quota_exhausted`,
  `rate_limited`, `offline`, `not_admitted`, `unsupported_version`,
  `internal`), `message` is display text. `not_admitted` means the worker
  does not answer requests from the customer's pubkey; it is sent rather
  than withheld so that a customer can tell a refusing worker from an
  absent one.

## Job result — kind `26900`

The terminal event of the job: the finished answer and its accounting.

```jsonc
{
  "kind": 26900,
  "pubkey": "<worker pubkey>",
  "content": "<NIP-44 payload encrypted to the customer pubkey>",
  "tags": [
    ["e", "<job request event id>"],
    ["p", "<customer pubkey>"]
  ]
}
```

```jsonc
{
  "v": 2,
  "type": "result",
  "text": "the complete answer",
  "usage": { "input": 554, "output": 61 },
  "model": "google/gemini-3.8-flash"
}
```

`usage` is optional; when absent the terminal shows no token count.
`model` is optional too, and a worker that can name the model it used
should: the terminal's own session record cannot name it, because the
terminal does not choose it, so the name the worker sends is the only
evidence of what answered. A job ends on the first result or
`status: error` the terminal accepts; later events for the same `e` tag
are ignored.

A result must carry nonempty `text`. Partial feedback is a preview and cannot
replace a missing final answer.

## Binding

The relay is transport, not authority. The subscription label a relay
delivers an event under is an unsigned routing hint, so it never
identifies the job. Before reading a payload the terminal checks what the
signature covers: the kind is `26900` or `27000`, the signer is the
worker's key, an `e` tag names the request this turn published, and a
`p` tag names the terminal's own key. A correctly signed answer to an
older job — relabeled onto the current subscription — fails the `e` tag
check, and an answer meant for another customer fails the `p` tag check.
Delivered event ids are deduplicated after those checks, so a forged
event cannot claim a genuine event's id and suppress it.

## Versions

`v` names the protocol revision a payload conforms to. This document
defines `2`; revision `1` differs only in that `partial` feedback carried
no `seq`, so its order is the relay's word rather than the worker's.

- A terminal accepts `v: 1` and `v: 2` worker payloads and rejects the
  field's absence and every other value.
- A worker answers at the version the request named: a `v: 1` request
  gets `v: 1` feedback, whose partials carry no `seq` because the
  terminal cannot check one. A request whose `v` is absent or names
  anything else is declined with `status: error`, code
  `unsupported_version`, rather than generated against a schema the
  worker cannot read.
- Under `1`, partial deltas are a liveness signal: they prove the worker
  answered but are never rendered as text, because nothing signed their
  order. A `v: 1` result still completes the job.
- Under `2`, a `partial` without an integer `seq` and a string `delta` is
  malformed and establishes nothing.

## Flow

1. The terminal connects to the relay and answers the NIP-42 challenge
   with a kind-`22242` event (required when the relay demands auth;
   recommended always — the `npub` is the billing identity).
2. The terminal builds and signs the kind-`25900` request, opens a
   subscription `{ "#e": [<request id>] }`, then publishes the request.
   Subscribing before publishing avoids the race where fast feedback is
   missed.
3. The worker — itself an authenticated relay client subscribed to
   `{ "kinds": [25900], "#p": [<its pubkey>] }` — receives the request,
   decrypts it, and publishes `judgment`, `partial`, and `status`
   feedback followed by exactly one result, all `e`-tagged to the
   request and `p`-tagged to the customer.
4. The terminal renders the judgment line, streams partial deltas, and
   folds the result into the transcript.

A terminal that sees no feedback within its deadline reports an unknown or
unavailable outcome according to observed contact. This conversation family
has no durable retry identity: another request is a new run, and MUST NOT be
used to recover an uncertain effectful task automatically. Use the execution
family for effectful work requiring retry identity or recovery.

## Encryption

All payloads use NIP-44 version-2 encryption over the sender/recipient
conversation key, exactly as gift wraps do. On a shared relay this keeps
tasks, transcripts, judgments, and answers visible only to the two
parties; tags carry only routing metadata (kind, `e`, `p`).

## Identity and quotas

- The customer's `npub` — the NIP-42 identity — is the billing identity.
  Anonymous usage is simply a freshly generated keypair.
- Quotas, allowances, and rate decisions are worker policy, not relay or
  protocol rules. The worker communicates refusal through
  `status: error` feedback with a typed `code`.
- An operator MAY require NIP-13 proof of work (`nonce` tag) on job
  requests; relays already validate the tag.
- Per-turn accounting SHOULD be emitted by the worker as a NIP-AM
  kind-`44200` metric event encrypted to the owner (stored kind — the
  durable ledger), separate from this ephemeral protocol.

## Decision jobs (proposed)

`proposed` — specified, not implemented. No decision worker or
relay-side decision caller exists yet. The wire shapes are defined here;
the service contract — principal mapping, admission order, quota
settlement, deadlines, cancellation, and the test matrix — is
[docs/decision-models/api/relay-decision-contract.md](../../docs/decision-models/api/relay-decision-contract.md).

A decision job carries one `POST /v1/systemone` call — `state` plus typed
`questions` — instead of a conversation turn. The family has its own
kinds, all ephemeral:

| Kind | Name | Direction |
| --- | --- | --- |
| `25910` | Decision job request | Caller → decision worker |
| `26910` | Decision job result | Decision worker → caller |
| `27010` | Decision job feedback | Decision worker → caller |

### The envelope

Every payload in the family leads with two fields:

- `v` — the schema tag, the string `"openagents.systemone.v1"`. It is a
  string, never the integer `v` of the conversation family, so the two
  payload grammars cannot share a version check.
- `type` — the discriminator. Requests are `systemone` or `cancel`;
  feedback is `status`; the result is `result`.

A missing `v`, any other value, or a `type` the reader does not know is a
refusal — `unsupported_version` or `malformed` — never a guess.

```jsonc
// kind 25910, decrypted content — a decision request
{
  "v": "openagents.systemone.v1",
  "type": "systemone",
  "request": "req-9f4c2a",      // logical request id, stable across retries
  "attempt": 1,                 // one-based; a retry bumps it
  "model": "shared-kev",        // the door, as in POST /v1/systemone
  "state": "I was charged twice on the March invoice.",
  "questions": {
    "refund": {"type": "noul", "instructions": "Does the customer ask for money back?"}
  },
  "deadline": 1784599800        // unix seconds, optional; mirror it in an expiration tag
}

// kind 25910, type "cancel" — best-effort cancellation, e-tagged to the request
{ "v": "openagents.systemone.v1", "type": "cancel", "request": "req-9f4c2a" }

// kind 27010 — progress or a terminal typed refusal
{ "v": "openagents.systemone.v1", "type": "status", "request": "req-9f4c2a",
  "attempt": 1, "status": "processing" }

{ "v": "openagents.systemone.v1", "type": "status", "request": "req-9f4c2a",
  "attempt": 1, "status": "error", "code": "quota_exhausted",
  "message": "daily input budget spent", "retry_after_ms": 3600000 }

// kind 26910 — the terminal event: outcome, the verbatim systemone
// response, and the sealed execution receipt (transport "relay")
{ "v": "openagents.systemone.v1", "type": "result", "request": "req-9f4c2a",
  "attempt": 1, "outcome": "answered", "response": { "model": "shared-kev",
  "answers": {"refund": {"type": "noul", "noul": 0.91}},
  "usage": {"input_tokens": 412, "output_tokens": 2} },
  "receipt": { "v": "openagents.receipt.execution.v1",
    "transport": "relay", "digest": "sha256:…" } }
```

`request`/`attempt` are the idempotency pair `Idempotency-Key` and
`X-Attempt` are on the HTTP lane; the request event's `id` is the
attempt's transport identity and the receipt's `attempt_id`. The refusal
`code` vocabulary is the gateway's plus the relay lane's own (`stale`,
`not_admitted`, `unsupported_version`); the result embeds a sealed
`openagents.receipt.execution.v1` receipt with `transport: "relay"`.

### Binding, on this family

The signature checks are the conversation family's checks with the kinds
and the payload fields swapped in. The worker accepts a request only when
the kind is `25910`, the signature verifies, a `p` tag names it, and
`created_at` is inside its request window. The caller accepts feedback or
a result only when the kind is `26910` or `27010`, the signer is the
worker's key, an `e` tag names this attempt's request event, a `p` tag
names the caller, and the decrypted `request` and `attempt` match the job
in flight.

The payload carries no credential: the verified event signer's pubkey is
the principal, mapped to a tenant by an operator-provisioned binding outside
the payload. NIP-42 controls the connection to the relay; the forwarded
event does not attest to that connection. A field claiming to be a bearer
secret authorizes nothing. Cancellation requires the original request signer
and resolves the referenced request within that principal and tenant. A
cancel before dispatch produces a terminal result with outcome `unattempted`
and cause `cancelled`; it does not remain a progress-only state.

### How an existing worker rejects this family

The kind split is the compatibility mechanism, and it is deliberate, not
cosmetic:

- A Coder-only worker subscribes `{"kinds": [25900], "#p": [<its key>]}`,
  so a kind-`25910` request never reaches it — the relay's filter is the
  first rejection. If such an event arrived anyway, the worker's address
  check rejects the kind before anything is decrypted. The rejection the
  caller sees is the contact deadline expiring — `worker_absent` — which
  is accurate: for a decision job, a worker that speaks only the
  conversation family is absent.
- Sharing kind `25900` would not be equivalent. A decision payload at any
  integer `v` the worker does not know, or at the string schema tag,
  would be refused `unsupported_version` — but only because the deployed
  worker happens to check `v` before reading anything else. A decision
  payload tagged `v: 2` is not refused at all: the worker reads `task`,
  `transcript`, and `instructions`, finds them absent, and generates an
  answer to an empty prompt. That is the silent reinterpretation the
  envelope must make impossible, so the boundary is the kind, not the
  payload version.
- Symmetrically, the decision worker subscribes `{"kinds": [25910], …}`
  and never sees a conversation request; one delivered anyway fails its
  `v` check, since `2` is not `"openagents.systemone.v1"`.

### Differences from the conversation family

- One job is one decision call: no transcript, no instructions, no
  streaming `partial` or `judgment` feedback. Progress is `status` only.
- The result carries a typed `outcome` and a sealed execution receipt —
  a relay call leaves the same evidence an HTTP call does.
- `request`/`attempt` make a retried job one logical request; the
  conversation family has no retry identity because a turn is not
  retried, it is re-run.
- `cancel` exists on the decision family. Dropping a conversation socket ends
  the caller's observation but does not establish remote cancellation.

## Execution jobs

`proposed` — v1, not implemented. This family carries an admitted operation or
program, typed task input, context references, and durable outcomes. It does
not use conversation text as an executable command or require an LLM to select
a program. CAP/PRG/EXT/RUN and the shared contracts are normative for this
family. Existing conversation/decision handlers MUST reject these kinds.

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

| Field | Meaning |
| --- | --- |
| `request` | Random logical request ID stable across retries. |
| `attempt` | Positive integer; retransmission preserves it, a permitted new attempt increments it. |
| `run` | Logical remote run ID, distinct from parent run ID. |
| `target` | Exact operation/program DefinitionRef. |
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
never zero. Decision subcalls retain their own sealed receipt schema.

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

### Compatibility and conformance

No existing family is renumbered or implicitly upgraded. CJ conversation v1/v2
remain their established payload versions; the decision family remains
`openagents.systemone.v1`; this new execution contract is v1. Core kinds,
version/type validation, and schemas provide separate rejection boundaries.

Required fixtures cover all three family cross-deliveries, signer/tenant
binding, repeated and conflicting fingerprints, lost admission/result traffic,
crashes before/after each effect boundary, restart/status/replay, retention
expiry, cancellation races, stale bases, budget settlement, duplicate/forked
results, and unauthorized controls. Demonstrate terminal/headless behavior
through the same host path. Do not advertise an execution worker until this
complete path runs; relay fanout alone is not execution support.
