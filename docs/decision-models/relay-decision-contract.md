# The relay decision contract

`proposed` — the pure protocol layer is implemented; the network service
is not. No decision worker or relay-side caller exists; this document is the contract a worker implementation
follows, written spec-first under
[#9469](https://github.com/OpenAgentsInc/openagents/issues/9469). The wire
shapes are defined in [NIP-CJ](../../nips/openagents/NIP-CJ.md) ("Decision
jobs"); this document defines what the payloads mean and how the service
semantics of the HTTP lane carry to the relay lane.

A relay decision job is one `POST /v1/systemone` call carried as a NIP-CJ
job instead of an HTTP request. The caller is a Nostr client; the door is
a decision worker subscribed to the relay. The relay remains transport
only: it holds no job state, sees only ciphertext, and never stores a job
artifact. The contract goal is that the same admission path the gateway
runs — authenticate, authorize, bound, reserve, verify, settle, receipt —
has a relay-lane equivalent with the same outcome vocabulary, so an answer
means the same thing on either transport.

## Kinds and flow

The decision family uses its own kinds, all ephemeral:

| Kind | Name | Direction |
| --- | --- | --- |
| `25910` | Decision job request | Caller → worker |
| `26910` | Decision job result | Worker → caller |
| `27010` | Decision job feedback | Worker → caller |

The flow mirrors the conversation family:

1. The worker connects, answers the NIP-42 challenge, and subscribes
   `{"kinds": [25910], "#p": [<worker pubkey>]}`.
2. The caller connects, answers AUTH, signs a kind-`25910` request
   `p`-tagged to the worker with NIP-44 content encrypted to the worker's
   key, subscribes `{"kinds": [26910, 27010], "#e": [<request id>]}`, and
   only then publishes the request.
3. The worker decrypts, admits, runs the decision call, and publishes
   zero or more feedback events and exactly one result, each `e`-tagged
   to the request and `p`-tagged to the caller, encrypted to the caller's
   key.
4. The job ends on the first result or `status: error` the caller
   accepts. The caller closes the job's subscription; the socket is
   reusable.

All payloads use NIP-44 version-2 encryption over the sender/recipient
conversation key, as the conversation family does. The relay sees the
routing metadata only — kind, `e`, `p`, `expiration` — and the NIP-42
authentication controls each connection to the relay. The worker derives
the request principal from the verified event signer, not from a claim about
another connection's authentication.

## The envelope

Every payload in the family carries two fields before any other field is
read:

- `v` — the schema tag, the string `"openagents.systemone.v1"`. A string,
  not the integer `v` of the conversation family, so the two payload
  grammars cannot share a version check.
- `type` — the payload discriminator. On requests: `systemone` (a
  decision call) or `cancel` (a best-effort cancellation). On feedback:
  `status`. On results: `result`.

A reader that finds `v` absent, or any other value, refuses the payload
as `unsupported_version` — or ignores it, when the event was never
deliverable to it in the first place. A reader that finds `v` correct but
`type` unknown refuses `malformed`. Unknown fields beyond these are
ignored, so later revisions can add fields without a version bump; a
revision that changes the meaning of a field bumps the schema tag.

### Request payload — kind `25910`

```json
{
  "v": "openagents.systemone.v1",
  "type": "systemone",
  "request": "req-9f4c2a",
  "attempt": 1,
  "model": "shared-kev",
  "state": "I was charged twice on the March invoice.",
  "questions": {
    "refund": {
      "type": "noul",
      "instructions": "Does the customer ask for money back?"
    }
  },
  "deadline": 1784599800
}
```

- `request` — the logical request identity, caller-chosen and stable
  across retries. This is the same role `Idempotency-Key` plays on the
  HTTP lane.
- `attempt` — the one-based attempt number. A retry keeps `request` and
  bumps `attempt`, exactly as `X-Attempt` does on the HTTP lane.
- `model`, `state`, `questions` — the `POST /v1/systemone` envelope,
  verbatim. `model` names the door the call is authorized against.
- `deadline` — optional unix seconds; the latest time an answer is
  useful. The worker refuses work that cannot start before it and may
  abandon work that outlives it. The event SHOULD carry the same value as
  a NIP-40 `expiration` tag so the relay can drop it too.

The event's `id` is the attempt's transport identity — signed, unique per
published attempt, and already deduplicated by both ends — and the receipt
records it as `attempt_id`. A retry is a new event with a new `id`,
the same `request`, and the next `attempt`.

### Cancellation — kind `25910`, `type: "cancel"`

```json
{
  "v": "openagents.systemone.v1",
  "type": "cancel",
  "request": "req-9f4c2a"
}
```

A cancel event is `e`-tagged to the request event it cancels and names
the logical `request`. The worker requires the cancel event signer to equal
the original request signer and resolves the event reference within that
principal and tenant. Neither a guessed request ID nor an `e` tag grants
cancellation authority. Cancellation is best-effort, never guaranteed:

- The job was admitted but not dispatched: the worker answers
  a terminal result with outcome `unattempted` and cause `cancelled`,
  releases the reservation, and settles without a spend.
- The job is running: the worker may abort it and settle `unavailable`
  with cause `cancelled`, or let it finish — the caller ignores a late
  result for a job it cancelled.
- The job already resolved: the cancel is a duplicate of nothing and is
  ignored.
- The cancel never arrives — the worker's socket dropped, the relay lost
  it: the job runs to its normal end. A caller that cannot tolerate the
  work sets a `deadline` instead of relying on cancellation.

### Feedback payload — kind `27010`

```json
{
  "v": "openagents.systemone.v1",
  "type": "status",
  "request": "req-9f4c2a",
  "attempt": 1,
  "status": "processing"
}
```

`status` is `queued`, `processing`, or `error`. The first two are
progress; `error` is terminal and carries the typed refusal:

```json
{
  "v": "openagents.systemone.v1",
  "type": "status",
  "request": "req-9f4c2a",
  "attempt": 1,
  "status": "error",
  "code": "quota_exhausted",
  "message": "daily input budget spent",
  "retry_after_ms": 3600000
}
```

`code` is the refusal vocabulary of the HTTP lane plus the relay lane's
own causes. `message` is display text. `retry_after_ms` accompanies the
congestion codes, the same role `Retry-After` plays on HTTP.

| Code | Meaning | Retryable |
| --- | --- | --- |
| `malformed` | The payload does not decrypt, parse, or fit the schema. | no |
| `unsupported_version` | `v` names a schema this worker does not serve. | no |
| `invalid_request` | The envelope is missing `model`, `state`, or `questions`, or fails validation. | no |
| `too_many_questions`, `too_many_options` | The envelope exceeds the door's bounds. | no |
| `unauthenticated` | The signer maps to no tenant and the door is not shared. | no |
| `not_admitted` | This worker does not answer requests from the caller's pubkey. | no |
| `door_not_bound` | The tenant holds no binding for the named door. | no |
| `idempotency_conflict` | The `(request, attempt)` pair is taken by different execution-affecting content. | no |
| `stale` | `created_at` is older than the worker's request window, or `deadline` passed before admission. | no |
| `rate_limited`, `busy`, `overloaded` | Capacity pressure; `retry_after_ms` says when. | yes |
| `quota_exhausted` | The tenant's budget is spent; retrying cannot fix it. | no |
| `door_unavailable`, `identity_mismatch`, `unavailable` | No backend, a card that disagrees with the binding, or a failed forward. | yes |
| `internal` | Anything else. | no |

### Result payload — kind `26910`

```json
{
  "v": "openagents.systemone.v1",
  "type": "result",
  "request": "req-9f4c2a",
  "attempt": 1,
  "outcome": "answered",
  "response": {
    "model": "shared-kev",
    "answers": {
      "refund": {"type": "noul", "noul": 0.91}
    },
    "usage": {"input_tokens": 412, "output_tokens": 2}
  },
  "receipt": {
    "v": "openagents.receipt.execution.v1",
    "request": "req-9f4c2a",
    "attempt": 1,
    "attempt_id": "b3f1c2d4e5a60718b3f1c2d4e5a60718b3f1c2d4e5a60718b3f1c2d4e5a60718",
    "transport": "relay",
    "tenant": "key-ref:acme/ops",
    "requested": {"model": "shared-kev"},
    "served": {
      "model": "shared-kev",
      "artifact_signature": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
    },
    "outcome": "answered",
    "timing": {"queued_ms": 34, "latency_ms": 281,
               "resolved_at": "2026-09-20T12:00:11Z"},
    "request_digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "result_digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    "usage": "resv-1042",
    "digest": "sha256:4444444444444444444444444444444444444444444444444444444444444444"
  }
}
```

- `outcome` — `answered`, `refused`, `unattempted`, `unavailable`, or `unknown`,
  the receipt's vocabulary. Every non-answered result carries `error`
  shaped like the feedback refusal instead of `response`. An `unknown`
  outcome is an explicit inability to establish completion, never a
  successful answer or evidence that retrying is free.
- `response` — the `POST /v1/systemone` response body verbatim when the
  outcome is `answered`: model identity, named typed answers, usage.
- `receipt` — the sealed `ExecutionReceipt` (`openagents.receipt.execution.v1`)
  with `transport: "relay"`. Its `digest` is the identity a caller quotes
  for the call, the same role `x-receipt` plays on HTTP. `request_digest`
  is the canonical digest of `{model, state, questions, request, attempt, deadline}`
  with an absent deadline represented as null, so caller and worker digest
  the same execution-affecting envelope. `tenant` is the resolved
  tenant reference — never a credential.

## What a signature must cover

The relay is transport, not authority, and the subscription label is a
routing hint. Before reading a payload, each end checks what the
signature covers.

The worker accepts a request only when all of these hold:

- The kind is `25910`.
- The Schnorr signature verifies over the event.
- A `p` tag names the worker's pubkey.
- `created_at` is within the worker's request window — a replayed event
  inside the window carries the same `id` and is deduplicated; one
  outside it is refused `stale`.
- The decrypted payload's `v` is `openagents.systemone.v1` and its `type`
  is known.

The caller accepts feedback or a result only when all of these hold:

- The kind is `26910` or `27010`.
- The signer is the worker's pubkey and the signature verifies.
- An `e` tag names the request event this attempt published.
- A `p` tag names the caller's pubkey.
- The decrypted `request` and `attempt` match the job in flight — a
  correctly signed answer to an older attempt of the same request fails
  the `e` tag check, and a relabeled one fails the payload check.

## Principals, authorization, and quota

The verified request event's signing pubkey is the principal. NIP-42
authenticates a connection to the relay; a forwarded event does not prove
which connection published it. The worker —
or the admission front it runs behind — maps that pubkey to a tenant
through an operator-provisioned npub-to-tenant binding, so one binding
decides the door and the budget on either transport. A signer that maps
to no tenant reaches only shared bindings, exactly as an anonymous HTTP
call does.

The payload carries no credential. Bearer secrets (`oak_<id>.<secret>`)
never appear in a request, a tag, or a log; a field claiming to be one
does not authorize anything and a worker SHOULD refuse a payload that
carries one, because a caller that pastes its key into job content has
already leaked it to the worker. Authorization is the registry's
`authorize` against the named `model` door; quota is `tenancy::quota`'s
durable reservation settled once by `(principal, tenant, request, attempt)`:
The following pair comparisons always occur within that principal and tenant.

- The same pair reserved again with the same request digest returns the
  in-flight or settled reservation — a retry is not a second spend. If
  the job already resolved, the worker republishes the recorded result
  rather than re-running it, so delivery is at-least-once while execution
  stays once.
- The same pair with a different request digest is refused
  `idempotency_conflict`.
- A crash after `reserved` leaves the recovery path's `orphaned`/`unknown`
  mark; the relay lane inherits it unchanged because settlement happens
  in the ledger, not on the socket.

Admission order is the HTTP lane's order: authenticate, authorize, bound,
reserve, verify the backend's card, then dispatch. A `busy` refusal
happens before a slot is held — a congestion refusal never holds quota.

## Deadlines, capacity, and unknown completion

- The caller's contact wait ends at the first bound feedback; silence
  past it is `worker_absent`, and on this family it also means no
  decision worker is listening — the correct reading, because a
  Coder-only worker cannot see the request at all.
- `deadline` bounds the whole job; work that outlives it settles
  `unavailable` with cause `timeout` or is abandoned, never answered
  late as if on time.
- Concurrency and rate bounds refuse `busy`, `rate_limited`, or
  `overloaded` at admission with `retry_after_ms`.
- A caller that loses the socket mid-job cannot know whether the attempt
  settled. That outcome is `unknown` — counted, never scored as free or
  as an error. The retry's next `attempt` settles against its own
  reservation, and the ledger reconciles the orphaned one.

## Replay, retry, and durable ownership

Every kind in this family is ephemeral: the relay fans an event out to
open subscriptions and never stores it. A relay event is therefore
neither a durable queue nor evidence of exactly-once completion — a
delivered event proves at most that the relay saw it, and an
undelivered one proves nothing. Exactly-once is a property of the
worker's settlement, not of the transport.

The `(request, attempt)` pair is the idempotency identity, scoped by the
principal and tenant. A retry keeps `request`, bumps `attempt`, and
publishes a new event with a new `id`; the event `id` is the attempt's
transport identity and settles nothing on its own.

- A replayed or duplicated request event — the same `(request, attempt)`
  delivered again, whether under the same event `id` or a fresh one —
  must not settle twice. The worker resolves the pair against its
  ledger once: a second delivery with the same request digest gets the
  recorded result republished, and one with a different digest is
  refused `idempotency_conflict`.
- Durable ownership of request and attempt state lives in the worker's
  settlement ledger, not in relay history. Ephemeral kinds leave no
  relay history to consult; the ledger holds the reservation, the
  settled outcome, and the crash-recovery marks, and it is the only
  state a later delivery is resolved against.
- An interrupted connection does not make an attempt's outcome unknown
  to the protocol. The worker settles every admitted attempt
  deterministically — `answered`, `refused`, `unattempted`,
  `unavailable`, or a stated `unknown` — whether or not the caller's
  socket lived to receive the result. The terminal result stays
  retrievable by `(request, attempt)`: the worker republishes the
  recorded result event for a settled pair, and a `cancel` resolves an
  in-flight one. `unknown` remains an explicit settlement outcome — the
  worker's stated inability to establish completion — never a synonym
  for a dropped socket; what the socket's loss changes is the caller's
  observation until a later delivery resolves it.

## Why the family does not share kind `25900`

A shared-kind envelope was considered and rejected; the reasoning is
recorded so the decision does not have to be re-derived.

- `v: "openagents.systemone.v1"` on kind `25900` would be refused
  `unsupported_version` by the current `coder-worker`, because its
  version check runs before any other payload field is read. That path
  is reliable today, but it is one line of check ordering away from not
  being, and it spends a signature verification, a decryption, and an
  admission slot on every misdirected job.
- `v: 2` with `type: "systemone"` on kind `25900` is not refused at all:
  the current worker reads `task`, `transcript`, and `instructions`,
  finds them absent, and generates an answer to an empty prompt — the
  silent reinterpretation this contract exists to prevent.
- On kind `25910`, an old worker's subscription (`kinds: [25900]`) never
  receives the event, and its address check would reject the kind even if
  the event arrived. Non-delivery is the rejection; the caller's contact
  deadline reports `worker_absent`. A caller that needs to know whether a
  worker speaks this family probes it, rather than inferring it from a
  refusal meant for another protocol revision.

The symmetric rule holds for the decision worker: it subscribes
`kinds: [25910]` and never sees a conversation request; one delivered
anyway fails the `v` check.

## Compatibility and test matrix

The matrix a worker implementation is tested against. "Old worker" is
`coder-worker` as implemented; "new worker" is the proposed decision
worker.

| Case | Expected behavior |
| --- | --- |
| New request `25910`/`systemone` → new worker | Admitted; status feedback, one result, sealed receipt with `transport: "relay"` |
| New request `25910` → old worker | Never delivered (filter); if delivered, rejected at the kind check, no response; caller reads `worker_absent` |
| `25900` conversation request `v: 2` → new worker | Never delivered (filter); if delivered, `v` is the wrong type — refused or ignored |
| `25900` payload `v: 2`, `type: "systemone"` → old worker | Misinterpreted as an empty conversation job. The contract forbids this combination; the kind split is what enforces it |
| Same `request`, `attempt` 1 and 2 | Two attempts, two receipts; the settled pair is spent once each |
| Same `(request, attempt)`, same digest, delivered twice | Second delivery gets the recorded result republished; no re-execution |
| Same `(request, attempt)`, different digest | `idempotency_conflict` refusal |
| Replayed request event inside the window | Deduplicated by event `id`; answered once |
| `created_at` outside the window | `stale` refusal |
| `deadline` already passed | `stale` refusal before admission |
| Concurrency bound reached | `busy` refusal with `retry_after_ms`; no slot held, no reservation |
| Unknown signer, door not shared | `unauthenticated` or `not_admitted` refusal |
| Tenant not bound to `model` | `door_not_bound` refusal |
| Quota spent | `quota_exhausted` refusal |
| `cancel` before dispatch | Terminal result with outcome `unattempted` and cause `cancelled`; reservation released |
| Cancel signed by another principal | Refused; original job remains active |
| `cancel` mid-run | Best-effort abort or normal completion; late result ignored |
| Socket lost mid-job | Caller outcome `unknown`; ledger reconciles the reservation |
| Payload carrying a credential field | Refused `malformed`; the credential is treated as leaked |

## What this document does not establish

- No worker, caller, or relay change is implemented. This file and the
  NIP-CJ section are the specification an implementation is checked
  against.
- The npub-to-tenant binding is the registry's business and is tracked
  under [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470);
  this contract only requires that the mapping exist before keyed doors
  are served.
- A relay event is not durable job storage; work that must outlive the
  sockets belongs to the durable job API under
  [#9484](https://github.com/OpenAgentsInc/openagents/issues/9484).
- The receipt is an attributable claim by the serving process, not remote
  attestation — the same standing the HTTP lane's receipts have.

## Implemented protocol layer

`nostr::decision` constructs and checks signed, encrypted requests, statuses,
results, and cancellations for this family. It binds a result to the configured
worker, recipient, request event, logical request, attempt, and request digest.
The embedded receipt must have a valid canonical digest, relay transport, and a
matching outcome. Contradictory response/error fields refuse parsing. Consumers
must still decode the full execution receipt and verify model identity and typed
answer semantics; a valid signature does not prove inference correctness.

The layer is pure protocol code. It does not connect to a relay, map principals
to tenants, reserve quota, persist idempotency state, dispatch inference, or
settle billing. Its tests use signed local fixtures, not a live decision worker.
The worker and caller paths, service admission, cancellation settlement, and
HTTP/relay parity measurements remain required for #9469.
