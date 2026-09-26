---
name: nostr
description: >
  How Nostr works in this repository: the three NIP lanes under `nips/`, the
  wire protocol (NIP-01), relay authentication (NIP-42), payload encryption
  (NIP-44), the Block agent NIPs the relay serves, and the OpenAgents NIPs
  (NIP-CJ jobs, NIP-CAP capabilities, NIP-PRG programs) that carry a Coder
  turn to a worker. Read it before touching `crates/nostr`,
  `crates/nostr-relay`, `crates/coder/src/relay.rs`, `coder-worker`, or any
  file under `nips/`, and before debugging a relay handoff.
---

# Nostr in OpenAgents

Use this skill when you change or debug anything that touches a relay:
the protocol crates, the relay binary, the Coder relay door, the worker,
or a specification under `nips/`. The specifications are the source of
truth; this skill tells you where they are and how the pieces fit, so you
don't rediscover the flow from code.

## Where the specifications live

| Lane | Path | Source of truth | Synced |
| --- | --- | --- | --- |
| Official NIPs | `nips/official/` | [nostr-protocol/nips](https://github.com/nostr-protocol/nips) | yes, `./scripts/sync-nips.sh` |
| Block (Buzz) extension NIPs | `nips/block/` | [block/buzz](https://github.com/block/buzz/tree/main/docs/nips) | yes |
| OpenAgents NIPs | `nips/openagents/` | this repository | no — the files here are authoritative |

`nips/manifest.json` pins the upstream commit of each synced lane. A sync
never changes the implementation without review and a fixture update.
`nips/README.md` states the mandate: every pinned spec that applies to a
relay is an implementation target for `crates/nostr` and
`crates/nostr-relay`, optional features stay fail-closed and out of NIP-11
until they run, and client-only NIPs are implemented as fixture-backed
clients rather than pretended.

Read the [September 26 upstream review](../../../docs/protocol/2026-09-26-upstream-nip-sync.md)
before assuming current-pin support. The source collections are updated;
the code ledgers retain old pins and their guards fail. New PMA kind 30179
requires rejection until its gates exist; thread-bounds kind 39007 is
relay-only but lacks the corresponding client rejection. NIP-78 privacy,
RS snapshot/barrier guarantees, and PL delivery have pending work.

Read the spec before the code. The files you need most often:

- `nips/official/01.md` — the protocol: events, kinds, filters, and the
  client-relay messages.
- `nips/official/42.md` — client authentication to a relay.
- `nips/official/44.md` — encrypted payloads.
- `nips/official/11.md` — the relay information document.
- `nips/official/40.md` — the `expiration` tag.
- `nips/official/19.md` — `npub` and `nsec` encoding.
- `nips/block/README.md` — a per-spec summary of all 17 Block NIPs.
- `nips/openagents/NIP-CJ.md` — Coder jobs, the protocol between `coder`
  and `coder-worker`.
- `nips/openagents/NIP-CAP.md` — capability manifests and presence.
- `nips/openagents/NIP-PRG.md` — programs.
- `nips/openagents/NIP-EXT.md` — extension releases, discovery, revocation,
  foreign-format import, and compatible host component-set assessments.
- `nips/openagents/NIP-RUN.md` — encrypted durable journals and recovery.
- `nips/openagents/NIP-CTX.md` — task frames, evidence views, and expansion.
- `nips/openagents/NIP-POL.md` — instructions, approvals, disclosure, routing
  records, and governed activation of learned private preferences.
- `nips/openagents/NIP-COORD.md` — task claims, fencing, and background findings.
- `nips/openagents/NIP-EVAL.md` — attributable workload evaluation and promotion evidence.
- `nips/openagents/NIP-OPT.md` — semantic AI contracts, immutable implementations,
  bounded studies, actual candidate execution, and optimization evidence.
- `nips/openagents/NIP-KB.md` — shared knowledge entries, heads, withdrawals,
  and evidence; `crates/nostr` (`kb`) checks them.
- `nips/openagents/NIP-CTRL.md` — task-scoped client pairing, observation,
  steering, and cancellation; no approval or wallet authority from pairing.
- `nips/openagents/NIP-MKT.md` — negotiated offerings, bilateral orders,
  private records, and separately admitted fixed-price Lightning settlement.
- `nips/openagents/NIP-LAB.md` — bounded agent labor, exact execution linkage,
  deliverables, independent checks, rework, disputes, acceptance, and rights.
- `nips/openagents/NIP-X402.md` — Designed Lightning-paid operations before
  execution, standard x402 HTTP/MCP bindings, and an opt-in native Nostr profile.
- `nips/openagents/NIP-SESS.md` — engine sessions, actual adapter support,
  queues, turn control, pending interactions, and retained native history.
- `nips/openagents/NIP-WS.md` — workspace/resource identity, conditional edits,
  checkpoints, and bounded projection snapshots, deltas, and cuts.
- `nips/openagents/NIP-WORK.md` — tracked objectives, planning relationships,
  accountable ownership, delegation, exact revisions, and work disposition.
- `nips/openagents/NIP-AUTO.md` — durable schedules, source watches, bounded
  goal continuations, occurrence identity, and restart-safe accounting.
- `nips/openagents/NIP-ENV.md` — execution environment leases, actual
  materialization, attachment, cleanup, and uncertain resource accounting.
- `nips/openagents/NIP-LIVE.md` — admitted media/capture, participants and
  recipients, input and speaking floors, evidence, and scoped device input.
- `nips/openagents/NIP-MV.md` — Verse world presence, entity state, and
  gestures; independent of the shared agent-artifact contracts.
- `docs/optimization/README.md` — DSPy/GEPA concepts, Gym evaluation, host
  boundaries, and a consolidated unfiled integration backlog.
- `nips/openagents/contracts.md` — pinned identities, schemas, locks, evidence,
  context, effects, outcomes, and private artifact envelopes for the v1 contracts.
- `docs/coder/design/typesafe-agent-protocol-addendum.md` — what belongs in
  Nostr and what hosts/clients must implement; complete source-proposal coverage.
- `docs/protocol/implementation-plan.md` — implementation across all lanes.
- `docs/agents/README.md` and `docs/agents/roadmap.md` — general agent
  infrastructure, Coder's domain boundary, and remaining non-code contracts.
- `docs/protocol/block-nips.md` — what the relay does with each Block NIP,
  including what it deliberately doesn't advertise.

The OpenAgents lane contains 22 NIPs plus the shared contracts. X402, SESS, WS,
WORK, AUTO, ENV, and LIVE are **Designed** drafts, as are POL's learned
preference lifecycle and EXT's import/component-set assessment additions.
They allocate no new event kinds: private artifacts use `3188`, remote host
operations use admitted CAP/CJ profiles, and effects leave RUN evidence.
Keeping an encrypted envelope does not implement the contract inside it.
MKT/LAB and CTRL also remain designed host roles; existing event helpers do
not establish a running labor service, settlement adapter, or client-control
system. Check the [glossary](../../../docs/glossary.md#nostr-and-shared-protocols)
for the narrower implemented and partial roles.

Read the [x402 integration assessment](../../../docs/coder/design/x402-lightning-nostr-integration.md)
before adding a payment adapter. NIP-X402 preserves upstream `http:1` and
`mcp:1`; `nostr:openagents:1` is an opt-in OpenAgents extension that requires
explicit facilitator support. It pays before execution; MKT/LAB pays after
acceptance. NWC is wallet transport, not purchase authority. Zap invoices hash
the zap request, while x402 invoices hash the bound operation; do not reuse
one invoice as both. L402/LSAT is a distinct macaroon-based protocol. These
paths have no implemented OpenAgents payment adapter.

Read the [teardown integration plan](../../../docs/coder/design/teardown-nostr-integration.md)
and [file-by-file archive coverage](../../../docs/protocol/2026-09-26-teardown-coverage.md)
when carrying historical ideas forward. The archive supplies design evidence;
its old TypeScript, hosting, and product decisions are not current instructions.
A tracked work item, session, run, environment, view, and commercial order have
separate identities. Assignment does not dispatch; completion does not accept
labor; acceptance does not pay; a live view does not grant mutation. Hosts
must enforce these boundaries before advertising the corresponding role.

## The protocol in one page (NIP-01)

An **event** is a signed JSON object: `id` (SHA-256 of the serialized
fields), `pubkey` (the author's x-only secp256k1 key, hex), `created_at`
(unix seconds), `kind`, `tags` (arrays of strings; the first element is
the tag name), `content`, and `sig` (Schnorr). The `id` and `sig` are what
make an event evidence: a relay can relabel or drop an event, but it can't
forge one.

**Kind ranges** decide how a relay stores an event:

| Range | Class | Relay behavior |
| --- | --- | --- |
| everything not listed below | regular | stored, all kept |
| `0`, `3`, `10000`–`19999` | replaceable | latest per `pubkey`+`kind` kept |
| `20000`–`29999` | ephemeral | fanned out to open subscriptions, never stored |
| `30000`–`39999` | addressable | latest per `pubkey`+`kind`+`d` tag kept |

`crates/nostr` implements this as `EventClass::from_kind` in
`domain/replacement.rs`. Every NIP-CJ
job kind is ephemeral; NIP-CAP and NIP-PRG discovery kinds are addressable.
NIP-EXT and NIP-RUN add regular immutable records with separate addressable
heads. A discovery head is never an execution version pin.

**Messages**, client to relay:

- `["EVENT", <event>]` — publish.
- `["REQ", <sub_id>, <filter>, ...]` — subscribe; the relay sends stored
  matches, then `EOSE`, then live matches until `CLOSE`.
- `["CLOSE", <sub_id>]` — end a subscription.
- `["AUTH", <event>]` — answer a NIP-42 challenge.

Relay to client:

- `["EVENT", <sub_id>, <event>]` — a match.
- `["OK", <event_id>, true|false, <message>]` — the verdict on an
  `EVENT` or `AUTH`. A `false` message starts with a machine-readable
  prefix: `duplicate:`, `pow:`, `blocked:`, `rate-limited:`, `invalid:`,
  `restricted:`, `mute:`, `auth-required:`, or `error:`.
- `["EOSE", <sub_id>]` — end of stored events; what follows is live.
- `["CLOSED", <sub_id>, <message>]` — the relay ended the subscription,
  with the same prefixes.
- `["NOTICE", <message>]` — human-readable.
- `["AUTH", <challenge>]` — a NIP-42 challenge.

**Filters** select on `ids`, `authors`, `kinds`, `#<single-letter-tag>`,
`since`, `until`, and `limit`. Only single-letter tags are indexed, which
is why NIP-CJ routes on `e` and `p`.

## Authentication (NIP-42)

A relay may send `["AUTH", <challenge>]` at any time; this relay sends
one on connect whenever `NOSTR_RELAY_URL` is set. The client answers with
a kind-`22242` event carrying `["relay", <url>]` and
`["challenge", <challenge>]` tags, `created_at` within ten minutes of
now, and the relay replies `["OK", <auth_event_id>, true, ""]`. Kind
`22242` is never stored or broadcast.

An unauthenticated client isn't refused by default. Only what the relay
gates needs auth:

- Everything, when `NOSTR_RELAY_AUTH_REQUIRED=true` — the relay answers
  `auth-required:` on `EVENT` and `CLOSED ... auth-required:` on `REQ`.
- Reads of private Block kinds (gift wraps `1059`, engrams `30174`, turn
  metrics `44200`, and others listed in
  `event_visible_to_reader` in `crates/nostr-relay/src/gateway/subscription.rs`).
  A subscription's `read_pubkeys` is the set of keys the connection has
  authenticated as.
- Protected events (NIP-70 `-` tag), closed-membership relays, and every
  relay command in `docs/protocol/block-nips.md`.

The Coder client and worker always authenticate when challenged — the
`npub` is the account — and both refuse a connection whose challenge
gets no `OK`. NIP-AA lets an agent key satisfy AUTH with an owner
attestation (`nips/block/NIP-OA.md`) instead of its own membership.

## Encryption (NIP-44)

NIP-44 version 2 encrypts a payload under a **conversation key** derived
from the sender's secret and the recipient's public key (ECDH, then HKDF),
so either party can decrypt and nobody else can. It hides the content and
pads its length; it does not hide the tags, so `kind`, `e`, and `p` stay
visible to the relay as routing metadata. Gift wraps (NIP-59) and every
NIP-CJ payload use it. The implementation is `crates/nostr`'s `nip44`
module.

## Coder jobs (NIP-CJ)

`nips/openagents/NIP-CJ.md` is the contract between `coder` (the
customer) and `coder-worker` (the fulfiller). The relay is transport
only: it holds no job state and sees only ciphertext.

| Kind | Name | Direction | Payload `type` |
| --- | --- | --- | --- |
| `25900` | job request | terminal → worker | `task`, `transcript`, `instructions`, `client`, `v` |
| `27000` | job feedback | worker → terminal | `judgment`, `partial` (with `seq` under `v: 1`), `status` |
| `26900` | job result | worker → terminal | `result` with `text`, optional `usage` and `model` |

The flow, in the order the sockets speak it:

1. The worker connects, answers AUTH, and subscribes
   `{"kinds": [25900], "#p": [<worker pubkey>]}`.
2. The terminal connects, answers AUTH, signs a kind-`25900` request
   `p`-tagged to the worker with NIP-44 content encrypted to the
   worker's key, subscribes `{"kinds": [26900, 27000], "#e": [<request id>]}`,
   and only then publishes the request. Subscribing first closes the
   race where fast feedback would be missed.
3. The relay fans the request out to the worker's subscription. The
   worker decrypts, generates through its door, and publishes feedback
   and exactly one result, each `e`-tagged to the request and `p`-tagged
   to the customer, encrypted to the customer's key.
4. The terminal accepts an event only if the signature covers what it
   expects: the kind is `26900` or `27000`, the signer is the configured
   worker, an `e` tag names this request, and a `p` tag names this
   terminal. The subscription label is a routing hint, never identity.
5. The job ends on the first result or `status: error`. The terminal
   then sends `CLOSE` for the job's subscription and keeps the socket
   for the next turn. A relay `CLOSED` on that subscription ends the job
   as a relay error with the relay's reason. A socket that broke, or a
   wait that ran out with a subscription that might still deliver, is
   dropped, and the next turn opens a fresh one. A turn holds the socket
   for its duration, so a caller that cancels the turn drops the socket
   with it; the relay frees the subscription when the connection ends.

The conversation specification uses integer v1 with sequenced partials.
Decision and execution bodies use their named v1 schemas. Validate each family
against its own complete contract; do not infer support from a version alone.

Deadlines and failure words, on the terminal side
(`crates/coder/src/relay.rs`): if nothing bound to the request arrives
within the contact deadline, the turn fails with cause `worker_absent`
("no worker answered"). If a worker was heard but no result arrives
within the answer deadline, the cause names the worker as silent. A
relay `OK … false` on the request fails the turn with the relay's reason.
Opening a socket, handshake and AUTH together, is bounded by
`CONNECT_TIMEOUT`. A worker that fails upstream and publishes nothing
looks identical to an absent worker from the terminal — check the
worker's stderr before blaming the relay.

A worker whose door is a local executor (`CODER_EXECUTOR`) publishes one
encrypted kind-`27000` feedback event,
`{"type": "status", "status": "processing"}`, as soon as it admits a
delegation and before the executor starts. The terminal counts any
well-formed kind-`27000` status of `queued`, `processing`, or `error` as
contact, so its 30-second contact deadline ends at admission rather than
at the executor's answer.

Environment, terminal side: `CODER_WORKER` (worker `npub` or hex pubkey)
and `CODER_RELAY` (`ws://` or `wss://` URL). The identity is
`CODER_SECRET_KEY` or `CODER_NSEC`, or, when neither is set, the key at
`~/.openagents/nostr-secret`, created on first use with mode `0600`. Only
a missing file is a first use: an unreadable one is an error, so a
permissions accident cannot silently mint a new `npub`. Creation is
exclusive and atomic, and concurrent first runs agree on one key. A
fresh keypair is a valid anonymous account; never print, log, or commit
the secret.

Environment, worker side (`crates/coder/src/bin/coder-worker.rs`):
`CODER_WORKER_SECRET` (64 hex or `nsec`), `CODER_RELAY`, and the door:
`CODER_DOOR_KEY` with `CODER_DOOR_URL` and `CODER_WORKER_MODEL` for an
Open Responses door, or `CODER_EXECUTOR=<capability slug>` for a local
approved executor such as `devin-local` (see
`docs/coder/guides/worker-executor.md`). Setting both is refused. The
worker prints its pubkey on start; that value is what the terminal's
`CODER_WORKER` names. `--once` answers one job and exits; `--decline
<CODE>` refuses every job with a typed status error, for measuring the
refusal path. `CODER_WORKER_ALLOW` (comma-separated `npub` or hex keys)
limits which customers the worker answers; anyone else gets a typed
`not_admitted` status, never silence. Unset admits everyone, which is
right only on a local relay. `CODER_WORKER_JOBS` bounds how many jobs
the worker runs at once; unset, an executor door runs as many as its
manifest's `concurrent_max` and a model door runs four. A request past
the bound is refused before anything runs, with a typed `busy` status,
and the refusal releases the slot. The worker's key and the terminal's
key must differ.

`docs/coder/measurements/relay-transport.md` is the measured proof that both ends
meet, with per-transport latency and refusal causes.

## Capabilities and programs (NIP-CAP, NIP-PRG)

CAP describes portable interfaces, host bindings, presence, and operator
preferences. Reading a manifest is inert; probes and invocations have separate
admission. PRG defines typed workflows, seven step kinds, bounded composition,
and the Wasm packet ABI. EXT distributes immutable components and RUN records
durable execution.

OPT separates semantic AI signatures from their concrete implementations.
Programs can invoke a supported pinned implementation through the same host
boundary. Compiler, optimizer, evaluator, and inference capabilities remain
distinct. EVAL supplies workload evidence and scoped promotion; neither a score
nor a package signature grants execution.

Follow the normative NIPs for exact fields and validation. Implement only
supported roles and advertise them after conformance. Artifact identity,
actual materialization, protected evaluation, and operator adoption are
separate checks. Local operation need not publish an event for every action.

## The relay (`crates/nostr-relay`)

One binary, one Postgres database. Configuration is environment only;
`deploy/nostr-relay.env.example` and `docs/deployment/configuration.md`
list every variable. The ones you need for a local run:

```sh
DATABASE_URL=postgres://<user>:<password>@127.0.0.1:5432/<db>
NOSTR_RELAY_BIND_ADDR=127.0.0.1
NOSTR_RELAY_PORT=7447
NOSTR_RELAY_URL=ws://127.0.0.1:7447      # enables the NIP-42 challenge
NOSTR_RELAY_MEDIA_ROOT=$HOME/relay-media  # optional; absolute path
NOSTR_RELAY_AUTH_REQUIRED=false           # true to gate EVENT and REQ
NOSTR_RELAY_LOG_LEVEL=info
```

Migrations run on first boot and the start fails closed if the ledger
disagrees with the database. `GET /health` and a NIP-11 request
(`Accept: application/nostr+json` on `/`) confirm it's up.
`docs/deployment/runbook-local-dev.md` walks the same steps with a
disposable Postgres cluster and the `nak` client.

Live fan-out is in `gateway/subscription.rs`: a published event is
indexed by id, author, kind, and each single-letter tag; a subscription
is matched against its filters, then checked with
`event_visible_to_reader` for the private kinds; a subscription still
loading history buffers live matches until `EOSE`. Ephemeral events are
deduplicated by id over a short window and never reach storage.

The Block NIPs the relay serves, and how, are in
`docs/protocol/block-nips.md`. The relay-signed kinds (`39005`, `39006`,
`30622`) are refused from clients with `restricted:`.

## Debugging a handoff

When `coder -p` reports `worker_absent`:

1. Confirm the worker is connected and subscribed: its stderr shows
   `worker <pubkey>`, `relay <url>`, `door …`, and `waiting for jobs`.
   `CODER_WORKER` must equal that printed pubkey.
2. Confirm the relay fans out: open a second subscription with the
   worker's filter (a `nak req` or a scripted WebSocket client) and run
   the turn again. Seeing the kind-`25900` event there proves the relay
   and the terminal; not seeing it means the request was refused (read
   the `OK`) or the terminal's `CODER_RELAY` points elsewhere.
3. Read the worker's stderr for `job <id> failed: …`. A worker whose
   door fails upstream publishes nothing, and the terminal can't tell
   that from silence.
4. Run the relay with `NOSTR_RELAY_LOG_LEVEL=debug`. It then logs one
   line per admitted ephemeral event: kind, ID, author, `e` and `p`
   tags, and the content's byte length, never the content. A `25900`
   with no `26900` tagged `e` to it is a job the worker never answered.
   Ephemeral kinds are not stored, so the database holds nothing to
   query afterwards; the log is the only relay-side record.
5. Only then read relay code.

## Rules that don't bend

- Product code is Rust. `crates/nostr` has no storage, no network, and
  no third-party Nostr crate.
- Never print, log, commit, or paste a secret key, `nsec`, or door key.
  Test fixtures use throwaway keys generated in the test.
- The relay is transport, not authority. Anything a relay could relabel
  or replay must be checked against the signature — kind, signer, `e`,
  and `p` — before the payload is read.
- Unverifiable state is refused, not assumed: an unknown probe is not
  present, an unsigned answer is not an answer, and a feature that isn't
  configured isn't advertised in NIP-11.
- A NIP change goes spec first (`nips/openagents/` or a synced upstream
  commit), then validators and fixtures, then the applicable host, client, or
  relay role. A client-only or encrypted host contract does not become relay
  behavior merely because its envelope passes validation.
