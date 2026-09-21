# NIP-CJ — Coder Jobs

`draft` `optional`

A request/response protocol between a Coder terminal and a fulfillment
worker, carried entirely over Nostr relay traffic. The relay is transport
only: it holds no job state, sees only ciphertext, and never stores a job
artifact. This NIP is the OpenAgents-owned analogue of the pattern the
Block lane uses for Buzz: application logic expressed as event kinds and
tags rather than a private API.

NIP-90 reserves a similar shape (`5xxx`/`6xxx`/`7000`) but upstream marks it
unrecommended, and its plaintext `i`/`output` tags and payment flow do not
fit encrypted terminal sessions. This NIP defines its own kind family.

## Kinds

All three kinds are ephemeral (`20000`–`29999`): relays fan them out to
open subscriptions and never persist them. A job lives only as long as the
two connected sockets that speak it.

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

A terminal that sees no feedback within its deadline reports the worker
unavailable and may retry; because every kind is ephemeral, a retry is
simply a new request — there is no stored queue to drain.

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
- `cancel` exists because a decision call is worth aborting; a
  conversation turn ends by dropping the socket.
