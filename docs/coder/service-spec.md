# Coder service: Nostr auth, free usage, and deployment

Status: proposal. Nothing in this document is implemented in this
repository yet. The server side describes work in the private `coder`
repository, which stays private.

This document proposes how `crates/coder` in this repository works without
user-supplied provider API keys: the terminal authenticates to a deployed
Coder service with a Nostr key, and the service holds the provider
credentials, applies quotas, and bills its own accounts.

## Goals and non-goals

Goals:

- A user with no API keys can run `cargo run -p coder` and hold a real
  conversation.
- Authentication uses Nostr signed events (NIP-98 and NIP-42), not GitHub.
- Anonymous usage is a first-class tier, controlled by the server: a small
  free allowance per Nostr public key, rate-limited.
- Users who authenticate to the service get a larger allowance under their
  account.
- Users who already hold a provider key can keep using it directly; that
  path is unchanged.
- A local development server reproduces the production API so contributors
  can run the full flow without production credentials.

Non-goals:

- Moving any part of the private backend into this repository.
- GitHub or OAuth sign-in for the anonymous tier.
- Paid billing in this repository. Accounts, ledgers, and grants are
  server-side concerns; this document only specifies the client-visible
  surface.
- Relaying or storing Nostr events. Every event is ephemeral
  (`20000`–`29999` kinds) and travels only in HTTP headers.

## Architecture

```
coder terminal (this repo)          Coder service (private repo)
+------------------------+          +---------------------------------+
| crates/coder           |  HTTPS   | admission: NIP-98 / NIP-42 /    |
|   classify → generate  | -------> |   key / token                   |
|   NIP-98 signer        |          | anonymous_people: npub → person |
| key under ~/.openagents|          | quotas, ledger, free allowance  |
+------------------------+          | door → Vercel AI Gateway → LLM  |
                                    +---------------------------------+
```

The terminal never sees the provider key. The service terminates the
client request, checks its credential, applies quota, and calls the door
with credentials it owns.

## Caller classes

The service distinguishes three caller classes, in this precedence order:

1. **Anonymous Nostr key.** The client generates a secp256k1 keypair on
   first run, keeps the secret key on the device, and signs every request.
   The server maps the public key (`npub`) onto an `anonymous_people` row
   and its synthetic person and account. The account receives the service's
   configured opening credit and per-minute free quota.
2. **Authenticated account.** The client presents a kind-`27241` identity
   token from a trusted issuer inside its request signature, or another
   credential the service accepts (API key, session, grant). The caller's
   own account pays, at the account's quota.
3. **Own provider key.** The client bypasses the service entirely and
   calls a door directly with `CODER_DOOR_KEY` or
   `CODER_AI_GATEWAY_KEY`, as the current `ResponsesDoor` does.

Class 1 is the new work on both sides. Classes 2 and 3 exist today: the
service's admission chain already tries session, API key, Nostr header,
and token bearer in that order, and the client's env-var door already
exists.

## Client identity

### Key custody

- On first run, the terminal generates a keypair and writes it under
  `~/.openagents/` (proposed: `identity.json`, mode `0600`), alongside the
  existing session layout the private clients already use.
- The `npub` is the user's identity on the wire and the handle the service
  bills. The terminal displays it on request.
- The secret key never leaves the device. The service stores only the
  `npub` in `anonymous_people`.
- Proposed, optional: derive the key from a BIP-39 mnemonic (NIP-06) so a
  user can back up or move an identity by writing down words. The private
  `coder-auth` crate already carries a NIP-06 module; this repository
  would implement or vendor an equivalent derivation.

### Signing dependency

The private repo signs with its `coder-auth` crate, which is not public.
The cryptography underneath is the public `immortal-core` crate
(`OpenAgentsInc/immortal`, CC0), which carries NIP-01 events, BIP-340
Schnorr signing, and NIP-19 `npub`/`nsec` encoding. Proposed: add a small
`auth` module in `crates/coder` that builds and signs the two event shapes
below on `immortal-core` (or `secp256k1` + `sha2` + `base64` directly).
This is roughly 150 lines, all of it specified here and in NIP-98.

## Request authentication: NIP-98

Every HTTP request to the service carries a kind-`27235` event in its
`Authorization` header:

```
Authorization: Nostr <base64url-or-base64(json event)>
```

The event:

```jsonc
{
  "kind": 27235,
  "pubkey": "<caller x-only public key, hex>",
  "created_at": 1760000000,           // within the server's skew window
  "tags": [
    ["u", "https://<service>/v1/responses"],   // exact absolute URL
    ["method", "POST"],                       // exact HTTP method
    ["payload", "<sha256 hex of request body>"], // required on POST
    ["token", "<base64 kind-27241 event>"]      // only when bound
  ],
  "content": "",
  "sig": "<BIP-340 schnorr signature>"
}
```

Verification rules, as the private `coder-auth` verifier implements them:

- Kind must be `27235`; `created_at` within the configured skew (60 s
  today); `u` and `method` match the request exactly; `payload` matches
  the body hash when present.
- The event id is claimed in a nonce store until its window closes, so a
  captured header replays at most zero more times per instance.
- Identity resolution: a `token` tag carries a bound identity token and
  must name the signing key as its subject; an event signed by a trusted
  issuer resolves identity from its own tags; anything else is currently
  refused as `Unknown`.

### Proposed server change: admit the bare key

Today a validly signed request from an unknown key gets `Refusal::Unknown`.
The anonymous tier requires one change in the private service: when a
NIP-98 event verifies and resolves no identity, map `auth.pubkey` through
`people::anonymous(npub)` and admit it as an anonymous caller. The
plumbing exists — `anonymous_people` maps an `npub` to a person with a
negative synthetic GitHub id and its own account — the forum delegation
path already writes those rows. Admission policy (`Admits::Anyone`) and
the chat surface already differ from run-ordering policy, so anonymous
callers can reach generation without reaching the fleet.

### Authenticated requests

A user who holds an account presents its token bound to the same key:
`POST /v1/token` on the service mints a kind-`27241` identity token
(optionally bound with a `p` tag naming the caller key), and the client
carries that token in the `token` tag of every request auth. Binding is
what keeps a leaked token useless without the key. Unbound tokens also
work as `Authorization: Bearer <base64 event>` for clients that sign
nothing, at bearer-token risk.

## WebSocket authentication: NIP-42

The service's `/v1/responses` also answers a websocket upgrade for
persistent sessions. Proposed client flow, matching the server's existing
challenge path:

1. Client sends a `challenge` frame on the open socket.
2. Server answers `session.challenge` with a fresh nonce.
3. Client signs a kind-`22242` event naming the challenge and the service
   URL, and sends it as an `authorize` frame, with a `token` tag when the
   key is account-bound.
4. Server verifies signature, challenge, skew, and token, then sends
   `session.authorized` naming the caller.

For the anonymous tier the same extension applies: a verified kind-`22242`
with no token maps the signing `npub` to `anonymous_people`. The initial
client can ship POST-only; the socket is a later optimization and this
document does not require it.

## Free usage and quotas

All quota decisions are server-side. The proposal wires existing knobs:

- **Opening credit.** `CODER_BALANCE_CENTS` already grants credit on first
  recognition. Anonymous npub-mapped accounts get a small configured
  opening credit so a new user can have a real conversation.
- **Rate limit.** The service already enforces a process-local rolling
  quota of 20 free generations per account per minute. Anonymous callers
  key on the synthetic account the npub maps to.
- **Ledger.** Every generation already reads a balance and writes a ledger
  row against the account; anonymous accounts behave identically.
- **Abuse controls to add.** Per-IP request limits at the edge, a daily
  cap per anonymous account, and a cap on anonymous account creation per
  IP (the npub is free to mint, so the bound must live on IP, not key).
  Sized so legitimate anonymous use is comfortable and farming the
  opening credit is not.

Exact numbers are operator configuration, not protocol. The service
refuses with typed errors (`insufficient_credit`, `rate_limited`) that
the client renders as plain text.

## Endpoints the client uses

| Endpoint | Method | Purpose |
| --- | --- | --- |
| `/v1/responses` | `POST` | One generation, SSE stream. The wire the client's `ResponsesDoor` already speaks. |
| `/v1/responses` | `GET` (upgrade) | Persistent socket, later. |
| `/v1/credit` | `GET` | Caller reads its own balance, for the token rail. |
| `/v1/token` | `POST` | Mint an identity token (authenticated users). |
| `/.well-known/coder-issuer` | `GET` | Publish the issuer `npub`, so a client can check a minted token's issuer. |
| `/v1/models` | `GET` | Model catalog, if the client ever lists lanes. |

The client needs no new wire format for generation: the service answers
the same Open Responses `POST` the current `ResponsesDoor` sends to the
Vercel gateway. Only the `Authorization` header changes.

## Service discovery and client configuration

Proposed environment and defaults, mirroring the private terminal's
conventions:

- `CODER_CLOUD` — service base URL. Default: the production deployment
  (a dedicated host, for example `https://coder.openagents.com`; exact
  host is an operator decision). Local dev: `http://127.0.0.1:4300`.
- The client builds request URLs as `{CODER_CLOUD}/v1/responses` and
  signs the exact absolute URL in the `u` tag, so a base-URL change needs
  no other work — the signature binds the URL it was made for.
- `CODER_DOOR_KEY` / `CODER_AI_GATEWAY_KEY` keep their current meaning
  and take precedence when set: own-key callers skip the service.

## Classify with no keys

Classification is a second provider call today (`jev` → TypeSafe API,
`TYPESAFE_API_KEY`). Proposed precedence in the client:

1. `TYPESAFE_API_KEY` set → local classify through `jev`, unchanged.
2. No key, service configured → `POST {CODER_CLOUD}/v1/classify` (new
   endpoint), same NIP-98 auth, same quota accounting. The service runs
   the question set with its own TypeSafe credential and answers the Jev
   response shape.
3. Neither → the current unrouted fallback (generate without judgment).

The `/v1/classify` endpoint is server work in the private repo; the
request and response mirror the Jev `/v1/answers` shape so `crates/coder`
reuses its decode path. Treating classify as billable service work also
keeps the question set on the server, where its tuning is not public.

## Errors and refusals

The server already returns typed refusals; the client should render them
verbatim plus a suggested action:

| Condition | Status | Client action |
| --- | --- | --- |
| No credential | `401`, `sign_in_required` | Offer sign-in or own-key setup. |
| Bad signature, stale `created_at`, URL/method/payload mismatch, replay | `401` + `WWW-Authenticate: Nostr`, typed `code` | Re-sign and retry once; then surface. |
| Valid key, unknown identity | `401`, `unknown` today | With the anonymous change this becomes an anonymous admit. |
| Quota or credit exhausted | `402`/`429`, typed | Show remaining-credit link or wait hint. |
| Server store unavailable | `503` | Retry; never treat as revoked credential. |

An anonymous user who exhausts the allowance sees the refusal and a
pointer to authenticate or set a provider key.

## Deployment

Proposed: a new deployment of the existing service binary — no new server
code beyond the anonymous-admission and `/v1/classify` changes:

- Dedicated host for this surface (for example `coder.openagents.com`),
  so anonymous traffic and quotas are isolated from the current
  `openagents.com` deployment's invite list.
- Config: `CODER_ISSUER_NSEC`/`CODER_ISSUER_NPUBS` for minting and trust,
  `CODER_BALANCE_CENTS` for the opening credit, `CODER_DOOR_*` for the
  provider credentials, `CODER_MODEL` for the served lane.
- Database: the existing schema already carries `anonymous_people`,
  accounts, and the ledger; no new tables are required for this proposal.
- Observability: count admissions, refusals by code, anonymous-account
  creation rate, and quota exhaustion. Do not log prompts, signatures,
  event ids beyond their nonce lifetime, or `nsec`s — the service never
  sees a secret key by design.

## Local development server

Before the public deployment exists, a contributor or the operator runs
the service locally in dev mode:

- The private `coder-serve` binary runs on `127.0.0.1:4300` (the port the
  private terminal already uses for local mode), with a dev issuer key, a
  local database, and either a real door key or a stub door.
- Dev mode should relax admission so a freshly generated client key is
  admitted anonymously with generous credit — that is the feature under
  test.
- The terminal points at it with `CODER_CLOUD=http://127.0.0.1:4300`; no
  `CODER_DOOR_KEY` or `TYPESAFE_API_KEY` needed.
- A dev instance must never hold production provider keys, production
  issuer keys, or a production database, and must not mint tokens a
  production instance would trust (a dev `CODER_ISSUER_NSEC` is not in
  production's `CODER_ISSUER_NPUBS`).

Because `coder-serve` is private, this path serves developers with repo
access today. If a fully public local mode is wanted later, the honest
version is a thin public dev server in this repository that implements
the same two endpoints against a caller-supplied key — listed as an open
question, not assumed.

## Migration

1. Add the `auth` module (key custody + NIP-98 signing) to `crates/coder`.
2. Extend `ResponsesDoor` with a service mode: base URL + Nostr header
   instead of a bearer key.
3. Server: admit verified-but-unknown keys as anonymous callers; add
   `/v1/classify`; deploy to the new host.
4. Terminal: first-run key generation, `npub` display, `CODER_CLOUD`
   wiring, refusal rendering.
5. Existing env-key behavior stays as the own-key tier; `StubGenerate`
   remains the last fallback.

## Open questions

- **Production host name** for the new deployment, and whether it shares
  the existing database or gets its own.
- **Free-allowance size**: opening credit in cents, per-minute and
  per-day caps for anonymous accounts.
- **NIP-06 mnemonic** on first run: convenience and backup, or a file
  key only? (The private crate already derives NIP-06 keys.)
- **Bound vs bearer tokens** for the authenticated tier's default:
  bound is safer, bearer is simpler for scripts.
- **Public local dev server**: worth building a thin OSS server that
  proxies `/v1/responses` and `/v1/classify` to a caller-supplied key, or
  is dev-mode `coder-serve` enough?
- **Classify placement**: server-side `/v1/classify` (proposed) vs. the
  terminal embedding the question set and the service proxying Jev raw.
- **WebSocket path**: adopt NIP-42 sessions now, or stay on per-request
  NIP-98 until multiplexing matters?
- **Account linking**: does an anonymous npub later merge into a GitHub
  account (the `p`-tag binding suggests yes), and what happens to its
  credit?

## References

- NIP-98, HTTP auth: `~/work/immortal/nips/official/98.md`.
- NIP-42, challenge auth: implemented in `crates/coder-auth/src/nip42.rs`
  (private); the kind-`22242` shape is standard.
- OpenAgents credential NIP draft (kinds `27240`–`27242`, tags):
  `~/work/coder/crates/coder-auth/docs/events.md`, mirrored as
  `~/work/immortal/nips/openagents/CA.md`.
- Existing admission, anonymous-people, free-limit, and mint paths:
  `~/work/coder/bins/coder-serve/src/` (`mcp/admission.rs`,
  `people.rs`, `responses_endpoint/`, `mint.rs`, `handshake.rs`).
- Public signing primitives: `~/work/immortal/crates/immortal-core`.
