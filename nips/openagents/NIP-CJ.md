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
  "v": 1,
  "task": "the user's draft text",
  "transcript": [
    { "role": "user", "content": "earlier turn" },
    { "role": "assistant", "content": "earlier reply" }
  ],
  "instructions": "the system prompt for the turn",
  "client": "coder 0.1.0"
}
```

- `v` is the payload version; this document defines `1`.
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
{ "v": 1, "type": "judgment", "verdict": "respond",
  "line": "respond 1.00 · conf 1.00 · risk 0.0 · prog 0.8 · code 0.03" }

{ "v": 1, "type": "partial", "delta": "The borrow checker" }

{ "v": 1, "type": "status", "status": "queued" }

{ "v": 1, "type": "status", "status": "error",
  "code": "quota_exhausted", "message": "free allowance used" }
```

- `judgment` — the classification verdict the worker computed for the
  turn. `verdict` is one of `respond`, `clarify`, `end_conversation`,
  `unrouted`. `line` is a display-ready one-line summary; terminals render
  it verbatim.
- `partial` — one streaming text delta of the answer, in order.
- `status` — coarse state or a terminal error. `error` ends the job;
  `code` is a machine-readable reason (`quota_exhausted`,
  `rate_limited`, `offline`, `internal`), `message` is display text.

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
  "v": 1,
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
