# Relay backend plan: coder on `relay.openagents.com`

Follow-on, 2026-09-26: the [agent labor integration plan](../../agents/market-infrastructure.md)
brings market coordination back into the active roadmap. The exclusions below
record the original relay extraction; they do not defer agent labor today.
The current repository still needs a reviewed market specification, provider
order lifecycle, and payment integration before it can advertise that support.

Status: proposal. This document supersedes the HTTP-service direction in
[`service-spec.md`](service-spec.md) — the backend is now our own Nostr
relay, not a private HTTP service.

The terminal connects to exactly one endpoint: `wss://relay.openagents.com`.
Everything it needs — identity, requests, streaming replies, usage records —
is Nostr events. The relay is deployed from this repository.

## The model

Block's Buzz showed the pattern: *the relay is the workspace*. Application
logic lives in three places and nowhere else:

1. **Event kinds and tags**, specified as NIPs.
2. **Relay-side policy**: validation, admission, rate limits, relay-signed
   projections.
3. **Clients**, including privileged ones: the fulfillment worker that
   holds provider keys is itself a relay client with a known `npub`.

Buzz specifies its whole app this way — agent personas (NIP-AP), owner
attestation (NIP-OA), agent relay admission (NIP-AA), encrypted agent
memory (NIP-AE), turn metrics (NIP-AM), live telemetry (NIP-AO). We adopt
the same posture for coder: request, judgment, streamed generation, and
usage accounting are Nostr events with a spec, not HTTP endpoints.

```
coder terminal                       relay.openagents.com
+------------------+   wss NIP-42   +-------------------------+
| crates/coder     | -------------> | crates/nostr-relay      |
|   keypair,       |  EVENT/REQ     |   store + policy + fanout|
|   classify+gen   |                +-------------------------+
|   over events    |                         ^
+------------------+                         | wss, same protocol
                                    +-------------------------+
                                    | fulfillment worker      |
                                    | (provider keys live here|
                                    |  deploy-time env only)  |
                                    +-------------------------+
```

The worker subscribes for job-request events, calls the provider door,
and publishes results back. Provider keys are never in this repository
and never touch the terminal — they are environment on the worker's
deployment.

## What moves from `immortal`

`~/work/immortal` is public (`OpenAgentsInc/immortal`, CC0), so the move
is code copying within our own public repos, not disclosure. Two crates:

### `crates/nostr` — protocol primitives

From `crates/immortal-core`, the relay-facing domain, fixture-tested and
I/O-free:

| Module | Contents |
| --- | --- |
| `event`, `tags`, `filter`, `replacement`, `deletion`, `timestamp`, `hex`, `error` | NIP-01 event, canonical id, BIP-340 verification, filter matching, replacement/address ordering, NIP-09 deletion, timestamp policy |
| `expanded` | `RelaySigner` (relay-signed events), NIP-98 `HttpAuth` parse/verify, NIP-29 group metadata/actions |
| `agent` | NIP-OA owner attestation, NIP-AO/AM kinds and routing checks, NIP-44 envelope validation |
| `block` | the Block kind constants and ingest validation (NIP-AE/AP/ER/MP/CW/DV/WP) |
| `nip19` | `npub`/`nsec` bech32 |
| `nip44` | NIP-44 v2 encryption |

This crate also serves the client side: the terminal builds events, signs
them, answers NIP-42, and encrypts to the worker with the same code the
relay verifies.

### `crates/nostr-relay` — the server

From `crates/immortal-relay`:

- `gateway/` — HTTP+WebSocket server: NIP-42 auth, NIP-86 management,
  COUNT/search, Blossom media (config-gated, ships disabled), rate
  limits, subscription index, EOSE handoff, ephemeral lane, wire fuzz
  corpus.
- `store/` — Postgres: admission transaction, `ingest_seq`, indexes,
  `LISTEN`/`NOTIFY` fanout.
- `main.rs`, `operations.rs`, `bulk_import.rs` (archive/migration import
  from an external `events` table — needed if we carry the live
  relay.openagents.com database forward).
- `migrations/` — the store, NIP-expansion, media, agent-identity, and
  block-handler migrations; renumbered into a fresh sequence (see open
  questions).
- `tests/fixtures/` — the `nip*` fixture dirs for every NIP the moved
  relay supports (official + block lanes), plus `fuzz-corpus`.
- `deploy/` — systemd unit, nginx config, env example, root `Dockerfile`;
  renamed (below).
- `contract/` + `scripts/export-contract.sh` — the machine contract: a
  deterministic descriptor of relay-observable behavior pinned to the
  crate version and the NIP source commits, with a `--check` mode. This
  is the parity gate; it gets trimmed of market sections.
- `scripts/sync-nips.sh` — the upstream sync/parity checker, reduced to
  the two lanes we keep.

## What stays behind

- **Market/Boltz/Liquid**: `domain/mkt.rs` (~7,900 lines), `market.rs`,
  `ark.rs`, `liquid.rs`, `boltz_compat.rs`, `mkt_swp_verify.rs`,
  `boltz_facade.rs`, `dev_market.rs`, `dev_work.rs`,
  `mkt_swp_coordination.rs`, `gateway/boltz.rs`, the `mkt_*` migrations
  and `nipmkt`/`bip327`/provider/lab fixtures, the market sections of
  `contract.rs`. The store touches these at known seams
  (`decide_mkt_immutable_admission`, `is_mkt_private_kind`,
  `mkt_swp_*` tables, `MktSwpCoordination*` types, the boltz HTTP
  branch in `handle_socket`) — a bounded excision, not a rewrite.
- **OpenAgents-lane NIPs**: `nips/openagents/` (46 files) and the domain
  modules that implement them (`openagents.rs`, `allwork.rs` — NIP-OT
  project kinds and NIP-WK/PI work records), plus `nipwk`/`nipotpg`
  fixtures and the `wk_work_tag` index migration. Not moved now; they can
  land later as a third lane if a spec is adopted.
- **Other immortal crates**: client, client-web, provider, lab,
  regtest gateway, adapters, scripts outside the two named above.

## The `nips/` tree

This is the original extraction snapshot. The current repository has three
lanes, including OpenAgents-authored contracts. The
[September 26 source review](../../protocol/2026-09-26-upstream-nip-sync.md)
records 100 official Markdown files and 17 Block specifications, with current
implementation gaps kept separate from source coverage.

```
nips/
  README.md          adapted: two lanes, sync contract, review rule
  manifest.json      two sources: official + block
  official/          all 99 files, pinned commit
  block/             all 15 Buzz NIPs + the repo-owned README
  openagents/        absent for now
```

`scripts/sync-nips.sh` keeps its contract — sparse-clone each upstream,
replace the lane, record commit + count in `manifest.json`, preserve a
repo-owned lane README — minus the openagents source. Spec changes are
normative only after review plus fixture update; that rule carries over
verbatim.

The block lane is load-bearing, not reference-only: NIP-OA/AA/AE/AM/AO are
the substrate for the coder protocol (see below), and the relay already
implements them.

## Renames

| Immortal | Proposed here |
| --- | --- |
| crates `immortal-core`, `immortal-relay` | `nostr`, `nostr-relay` |
| binary `immortal` | `nostr-relay` |
| `IMMORTAL_*` env vars | `NOSTR_RELAY_*` (e.g. `DATABASE_URL` stays, `NOSTR_RELAY_URL`, `NOSTR_RELAY_SECRET_KEY`, `NOSTR_RELAY_MANAGEMENT_PUBKEY`) |
| `LISTEN`/`NOTIFY` channels `immortal_event`, `immortal_ephemeral` | `nostr_event`, `nostr_ephemeral` |
| NIP-11 `name`/`software` | `nostr-relay` identity |

`NOSTR_RELAY_SECRET_KEY` is the relay signer key (NIP-29 group metadata,
relay-signed projections); it moves to the renamed env unchanged in
meaning.

## How coder works over the relay

### Identity and auth

Same client key custody as the previous spec: the terminal generates a
secp256k1 keypair under `~/.openagents/` on first run; the `npub` is the
identity. On the socket it answers the relay's NIP-42 challenge with a
kind-`22242` AUTH event. No GitHub, no bearer tokens — the `npub` *is*
the account.

### Requests and replies

NIP-90-shaped job flow (the reserved `5xxx`/`6xxx`/`7000` pattern; exact
kinds are an open question because upstream marks NIP-90 unrecommended —
the alternative is our own kind family, which is the Buzz approach):

1. Terminal publishes a job-request event, NIP-44-encrypted to the
   worker's `npub`, carrying task + transcript + lane hint.
2. The relay stores it (or keeps it ephemeral — decision below) and fans
   it out to the worker's subscription.
3. The worker emits feedback events `e`-tagged to the request:
   `judgment` (the classify verdict), `partial` (stream deltas),
   `status` (processing/queued), then a result event with the final text
   and usage.
4. The terminal subscribes `#e: [<request-id>]` and renders exactly what
   it renders today: judgment line, streaming text, token rail.

Classify folds into the worker: it owns `TYPESAFE_API_KEY`, runs the
question set, and publishes the verdict as the first feedback event. No
provider credential ever sits on the client; a user who *wants* local
classify keeps `TYPESAFE_API_KEY` and it takes precedence.

### Anonymous vs authenticated

- **Anonymous** = a freshly generated `npub`. The worker grants a small
  free allowance per `npub` per window, tracked in its own ledger. The
  relay's existing per-pubkey/per-IP rate limits bound the plumbing side.
- **Authenticated** = an `npub` the worker recognizes as an account
  (linked later through an attestation or a minted credential) with a
  larger allowance. Identity is still just the key — no OAuth anywhere.
- **Own-key users** keep the direct-door env path, unchanged, and never
  touch the relay.

Abuse: `npub`s are free to mint, so per-`npub` allowance alone is
farmsable. The worker's defenses, in order: small anonymous allowance,
per-`npub` daily cap, optional NIP-13 proof-of-work requirement on
anonymous job requests (weight is a tag the relay already validates), and
the relay's per-IP connection/event rates. Numbers are worker config,
not protocol.

### Usage accounting

NIP-AM is the Buzz primitive built for exactly this: kind `44200`
turn-metric events, NIP-44-encrypted to the owner, one per completed
turn, carrying token counts and cost. The worker publishes one per turn;
the user's client (and our dashboards, where entitled) reads them. The
relay already enforces AM's storage and owner-gated read rules.

### Relay-signed state where it helps

Buzz's pattern of relay-signed projections (NIP-CW window bounds, NIP-DV
visibility, NIP-WP workspace profile) is available if coder needs
server-computed views later — e.g. a relay-signed per-`npub` usage
snapshot. Not required for v1.

## The fulfillment worker

A new deploy unit — proposed as `crates/coder-agent` or a binary in the
coder repo initially; it is plumbing (a relay client + classify + door
call), so its code can be public here even though its deployment is
private:

- Connects to `wss://relay.openagents.com`, NIP-42s as its own `npub`,
  subscribes to the job-request kind tagged to it.
- Holds `CODER_DOOR_*` / `TYPESAFE_API_KEY` / `CODER_AI_GATEWAY_KEY` as
  deploy env — the only place provider keys exist.
- Enforces quota: its own store keys usage by `npub`; config grants the
  anonymous allowance.
- Publishes feedback/result/metric events; replies nothing to requests
  over quota beyond a typed `error`/`status` event.

The relay needs no coder-specific logic: it is a general Nostr relay.
Job-request admission policy (who may publish the kind, size bounds) is
ordinary relay config.

## Deployment

- `relay.openagents.com` redeploys from this repo's `crates/nostr-relay`.
  The live host currently runs a *pre-MKT* immortal build, so this is a
  binary swap, not a schema surprise — but the DB decision (reuse the
  existing Postgres + renamed migration ledger, vs fresh) is open below.
- Same shape as today: one binary, one Postgres, nginx/caddy in front
  terminating TLS, systemd unit or Cloud Run. `deploy/` moves over
  renamed; the root `Dockerfile` adapts to build `nostr-relay`.
- Worker deploys separately with its env keys; it only needs outbound
  wss to the relay.
- Local dev: `cargo run -p nostr-relay` with a local Postgres, plus a
  dev-mode worker (or the terminal's own-key/stub paths). No production
  keys or trusted issuers on a dev relay.

## Migration order

1. `crates/nostr` + `crates/nostr-relay`: copy, strip market/openagents
   lanes, rename, build, run fixture tests against Postgres.
2. `nips/` (two lanes) + `scripts/sync-nips.sh` + `contract/` +
   `export-contract.sh`, trimmed.
3. Deploy assets renamed; redeploy `relay.openagents.com`.
4. Coder-over-relay protocol: pick kinds, write the OpenAgents NIP draft
   (Buzz-style, lands in `nips/openagents/` when adopted — the lane
   returns then), implement the worker and the terminal's `RelayDoor`.
5. Terminal: key custody, NIP-42, publish/subscribe flow, quota-error
   rendering. `ResponsesDoor` remains as the own-key tier.

## Open questions

- **Job kind family**: reuse the NIP-90 `5xxx/6xxx/7000` reservation for
  interop, or define `2xxxx` ephemeral + addressable kinds of our own?
  (NIP-90 is marked unrecommended upstream; Buzz would have written its
  own microstandard.)
- **Persistence**: are job requests and results stored relay events
  (syncable history across devices — multi-device terminals get the
  transcript free) or ephemeral kinds (privacy, no retention)? A hybrid —
  ephemeral request, stored result pointer — is possible.
- **Encryption scope**: NIP-44 to the worker hides prompts from the
  public feed but the relay operator (us) still stores ciphertext; is
  plaintext-in-transit-to-own-relay acceptable for v1?
- **DB carry-forward**: reuse the live relay's Postgres (its data is
  pre-MKT, so it is all in the kept subset) or start clean?
- **Migrations**: renumber into a fresh `0001…` sequence on extraction,
  or keep upstream numbers with gaps as provenance?
- **Worker home**: `crates/coder-agent` in this repo (open plumbing,
  private deploy) vs the private coder repo vs in `crates/coder` as a
  second binary.
- **NIP-29 sessions**: model conversations as NIP-29 group channels
  (sync/moderation/share semantics free, this repo already ships a
  NIP-29 client skill) vs plain request/response kinds? Probably later,
  but the choice shapes the kind design.
- **Quota ledger location**: worker-private store (simple) vs
  relay-published encrypted usage events (auditable by the user, NIP-AM
  already does this for metrics).
- **Management ops**: keep NIP-86 management + Blossom media compiled in
  (config-gated off) or strip for a leaner first deploy?
- **`contract.rs`**: it is ~1,800 lines with heavy market sections —
  port it trimmed, or defer the machine-contract artifact until the
  extraction settles?

## References

- Relay source: `~/work/immortal/crates/immortal-relay` (gateway, store,
  migrations), `~/work/immortal/crates/immortal-core` (domain, nip19,
  nip44).
- Buzz NIPs + server contract: `~/work/immortal/nips/block/`,
  `~/work/immortal/docs/protocol/block-nips.md`.
- NIP sync + manifest: `~/work/immortal/scripts/sync-nips.sh`,
  `~/work/immortal/nips/manifest.json`, `~/work/immortal/nips/README.md`.
- Deploy assets: `~/work/immortal/deploy/`, `~/work/immortal/Dockerfile`,
  `~/work/immortal/docs/deployment/runbook-google-cloud.md`.
- Contract/fixture tooling: `~/work/immortal/contract/`,
  `~/work/immortal/scripts/export-contract.sh`,
  `~/work/immortal/tests/fixtures/`.
- Superseded HTTP-service proposal: [`service-spec.md`](service-spec.md).
