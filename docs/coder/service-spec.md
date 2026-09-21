# Coder service: Nostr auth, free usage, and deployment

Status: historical service proposal. The current
[Coder consumer vision](coder-as-decision-router-consumer.md) and
[Decision API specification](../decision-models/decision-api.md) supersede
this proposal's product-wide relay-only and no-billing scope. The target
supports explicit HTTP, relay, own-provider, and local decision profiles.
The text below preserves the earlier design; implemented wire behavior is
documented in [relay transport](relay-transport.md).

The earlier relay design proposed that the terminal talks only
Nostr to `wss://relay.openagents.com`, deployed from this repository's
`crates/nostr-relay`. The earlier version of this document proposed a
private HTTP service with NIP-98 headers; that direction is dropped. The
extraction and deployment plan for the relay itself lives in
[`relay-backend-plan.md`](relay-backend-plan.md).

This document proposes how `crates/coder` in this repository works without
user-supplied provider API keys: the terminal connects to our relay with a
Nostr key, and a fulfillment worker — itself a relay client — holds the
provider credentials, applies quotas, and answers.

## Goals and non-goals

Goals:

- A user with no API keys can run `cargo run -p coder` and hold a real
  conversation.
- Authentication is Nostr-native: a keypair on the device, NIP-42 AUTH on
  the socket. No GitHub, no bearer tokens, no HTTP.
- Anonymous usage is a first-class tier: a small free allowance per Nostr
  public key, enforced server-side and rate-limited.
- Users with accounts get a larger allowance under their `npub`.
- Users who already hold a provider key can keep using it directly; that
  path is unchanged.
- A local relay plus a dev-mode worker reproduces the production flow so
  contributors can run everything without production credentials.

Non-goals:

- Any private backend. The relay comes from the public `immortal`
  repository (CC0) and deploys from this repo; the worker is plumbing
  that can also live here.
- GitHub or OAuth sign-in for the anonymous tier.
- Paid billing in this repository. Quotas and usage records are
  worker-side concerns; this document only specifies the client-visible
  surface.
- Coder-specific relay features. The relay stays a general Nostr relay;
  the product logic lives in event kinds and the worker.

## Architecture

```
coder terminal                       relay.openagents.com
+------------------+   wss NIP-42   +-------------------------+
| crates/coder     | -------------> | crates/nostr-relay      |
|   keypair,       |  EVENT/REQ     |   store + policy + fanout|
|   job events     |                +-------------------------+
+------------------+                         ^
                                             | wss, same protocol
                                    +-------------------------+
                                    | fulfillment worker      |
                                    | classify + generate     |
                                    | provider keys = env     |
                                    | quota ledger per npub   |
                                    +-------------------------+
```

The terminal never sees a provider key and never sees a plain HTTP
endpoint. The worker never sees the client's secret key — only its `npub`
and the events it publishes.

## Caller classes

Three tiers, all on the same socket:

1. **Anonymous Nostr key.** The client generates a secp256k1 keypair on
   first run, keeps the secret key on the device, and answers the relay's
   NIP-42 challenge. The `npub` is the account. The worker grants a small
   configured free allowance per `npub` per window.
2. **Authenticated account.** An `npub` the worker recognizes — linked
   to an account through an attestation or a minted credential. The
   account's quota applies. The mechanism for linking is an open
   question; the identity on the wire is still only the key.
3. **Own provider key.** The client bypasses the relay entirely and
   calls a door directly with `CODER_DOOR_KEY` or
   `CODER_AI_GATEWAY_KEY`, as the current `ResponsesDoor` does.

Class 1 is the new work. Class 3 exists today.

## Client identity

- On first run, the terminal generates a keypair and writes it under
  `~/.openagents/` (proposed: `identity.json`, mode `0600`).
- The `npub` is the user's identity on the wire and the handle the
  worker bills. The terminal displays it on request.
- The secret key never leaves the device. Nothing server-side ever sees
  or stores an `nsec`.
- Optional: derive the key from a BIP-39 mnemonic (NIP-06) so a user can
  back up or move an identity by writing down words.

The signing and protocol code comes from `crates/nostr` — the moved
`immortal-core` domain, which carries NIP-01 events, BIP-340 Schnorr,
NIP-19 `npub`/`nsec`, and NIP-44 encryption. The same crate serves the
relay's verifier and the terminal's signer.

## Connection authentication: NIP-42

The relay implements NIP-42. On connect it sends an `AUTH` message with
a challenge; the client answers with a kind-`22242` event whose tags
name the relay URL and the challenge:

```jsonc
["AUTH", {
  "kind": 22242,
  "pubkey": "<caller x-only public key, hex>",
  "created_at": 1760000000,
  "tags": [
    ["relay", "wss://relay.openagents.com"],
    ["challenge", "<challenge string from the relay>"]
  ],
  "content": "",
  "sig": "<BIP-340 schnorr signature>"
}]
```

The relay verifies the signature, the challenge, and the timestamp
window (the moved implementation uses 600 s), then treats the socket as
that `npub`. Every event the socket publishes is already signed, so
per-message auth is unnecessary — unlike the HTTP design, there is no
per-request signing at all.

## Requests and replies

The job flow, NIP-90-shaped (kind family is an open question in the
relay plan; the shape is what matters here):

1. The terminal publishes a job-request event: kind `5xxx` or a private
   kind, NIP-44-encrypted to the worker's `npub`, carrying the task and
   transcript.
2. The relay fans it out to the worker's subscription (and stores it if
   the chosen kind is non-ephemeral).
3. The worker publishes feedback events `e`-tagged to the request id:
   `judgment` (the classify verdict), `partial` (stream deltas),
   `status` (queued/processing/failed).
4. It finishes with a result event carrying the final text and usage.
5. The terminal subscribes `#e: [<request-id>]` before publishing, and
   renders the same judgment line, streaming text, and token rail it
   draws today.

The worker discovers requests by subscribing to the job-request kind
with its own `npub` in `#p`. The terminal discovers the worker's `npub`
from configuration or the relay's NIP-11 document.

## Classify with no keys

Classification is a second provider call today (`jev` → TypeSafe API,
`TYPESAFE_API_KEY`). Precedence in the client:

1. `TYPESAFE_API_KEY` set → local classify through `jev`, unchanged.
2. No key → the worker classifies server-side with its own TypeSafe
   credential and publishes the verdict as the first feedback event. No
   separate endpoint; classify is a step in the same job.
3. No key and no worker → the current unrouted fallback (generate
   without judgment).

## Free usage and quotas

All quota decisions are worker-side. The relay contributes transport
limits only:

- **Free allowance.** The worker keeps a ledger keyed by `npub` and
  grants each new `npub` a configured allowance — enough for a real
  conversation, small enough to limit farming.
- **Rate limits.** The moved relay already rate-limits per pubkey and
  per IP. The worker adds its own per-`npub` concurrency and daily caps.
- **Abuse.** `npub`s are free to mint, so the allowance is small and the
  defenses stack: per-IP socket/event rates at the relay, per-`npub`
  caps at the worker, and optionally a NIP-13 proof-of-work `nonce`
  requirement on anonymous job requests (weight is a tag the relay
  already validates).
- **Accounting.** One NIP-AM kind-`44200` turn-metric event per turn,
  NIP-44-encrypted to the owner, gives durable per-turn token/cost
  records the relay already stores and gates correctly.

Exact numbers are operator configuration, not protocol. Over-quota
requests get a typed `status`/`error` feedback event naming the reason
(`quota_exhausted`, `rate_limited`), which the client renders as text.

## Configuration

- `CODER_RELAY` — relay URL. Default `wss://relay.openagents.com`;
  local dev `ws://127.0.0.1:8080` (the moved relay's default port).
- `CODER_WORKER` — the worker `npub` job requests are encrypted to, if
  it is not advertised in NIP-11.
- `CODER_DOOR_KEY` / `CODER_AI_GATEWAY_KEY` keep their current meaning
  and take precedence when set: own-key callers skip the relay.
- `TYPESAFE_API_KEY` keeps its current meaning for local classify.

## Errors and refusals

There are no HTTP statuses on this path. Failures arrive as typed
events or socket-level conditions:

| Condition | Surface | Client action |
| --- | --- | --- |
| NIP-42 answer invalid or missing | relay `CLOSED`/notice | Re-answer once, then surface. |
| Job request rejected at ingest | relay `OK` false + reason | Show the reason verbatim. |
| Worker offline | no feedback events within a deadline | Report worker unavailable; retry. |
| Quota or rate exhausted | `status`/`error` feedback, typed | Show allowance hint or own-key pointer. |
| Malformed result/feedback | unparseable event | Ignore the event; flag the turn failed. |

An anonymous user who exhausts the allowance sees the refusal and a
pointer to authenticate or set a provider key.

## Deployment

Two deploy units, both reachable only through the relay:

- **Relay** — `crates/nostr-relay` at `relay.openagents.com`, one binary
  and one Postgres behind TLS termination. Config: database URL, public
  relay URL, relay signer key (NIP-29/relay-signed kinds), management
  pubkey, rate limits. No provider keys, no issuer keys, no billing
  config — it is a general Nostr relay.
- **Worker** — a second process with a private deployment holding
  `CODER_DOOR_*` / `TYPESAFE_API_KEY` / `CODER_AI_GATEWAY_KEY` as env,
  its quota config, and its own small store for the `npub` ledger. It
  only needs outbound wss to the relay.

Observability without content: count connections, AUTH answers,
job-request volume, quota refusals, and worker latency. Do not log
prompts, ciphertext payloads, event ids beyond their window, or any
secret — the worker sees plaintext (NIP-44 decrypts to it) and must not
log it.

## Local development

The whole loop runs on a laptop:

- `cargo run -p nostr-relay` against a local Postgres gives the relay;
  the worker runs against the same relay with a dev `npub`, a dev door
  key or stub, and generous quota.
- The terminal points at it with `CODER_RELAY=ws://127.0.0.1:8080`; no
  `CODER_DOOR_KEY` or `TYPESAFE_API_KEY` needed.
- A dev worker must never hold production provider keys or answer on
  the production relay (its `npub` is not the configured worker there).

Because both units live in this repository, local dev needs no private
code at all — the first fully public path to a working terminal.

## Migration

1. Extract `crates/nostr` + `crates/nostr-relay` per the relay plan;
   deploy `relay.openagents.com` from this repo.
2. Add key custody + NIP-42 to `crates/coder` on top of `crates/nostr`.
3. Stand up the worker (kind subscription, classify, door, quota
   ledger).
4. Add `RelayDoor` beside `ResponsesDoor`: connect, AUTH, publish
   encrypted job request, render feedback events.
5. Env-key behavior stays as the own-key tier; `StubGenerate` remains
   the last fallback.

## Open questions

- **Job kind family** — NIP-90 `5xxx/6xxx/7000` vs our own kinds; see
  the relay plan.
- **Persistence** — stored job events (free multi-device history) vs
  ephemeral (no retention).
- **Worker home** — `crates/coder-agent` here vs a binary in `crates/coder`
  vs private repo.
- **Account linking** — how an `npub` becomes a recognized account:
  NIP-OA attestation, a minted credential, or a payment rail.
- **Free-allowance size** — per-`npub` and per-IP numbers, PoW weight if
  used.
- **Worker discovery** — NIP-11 field, well-known document, or config.
- **NIP-29 sessions** — whether conversations become group channels
  later; affects kind design.

## References

- Relay move and deployment plan:
  [`relay-backend-plan.md`](relay-backend-plan.md).
- NIP-42 auth and relay behavior: `nips/official/42.md` (moved tree),
  `~/work/immortal/crates/immortal-relay/src/gateway/auth.rs`.
- NIP-44 encryption: `nips/official/44.md`,
  `~/work/immortal/crates/immortal-core/src/nip44.rs`.
- Job-request prior art: `nips/official/90.md` (NIP-90 DVM; marked
  unrecommended upstream, shape still applies).
- Buzz agent NIPs this design reuses: `nips/block/NIP-OA.md`,
  `NIP-AM.md`, `NIP-AO.md`, `NIP-AE.md` (moved tree).
