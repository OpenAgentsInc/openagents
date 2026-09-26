# Block NIP implementation status

The [Block lane](../../nips/block/README.md) contains **17 specifications**
pinned at Buzz commit `781d39510cf23cfe224e8f521ae06a23377e06de`. These are
implementation targets. The 2026-09-26 sync changed AP, CW, PL, and RS and
added FI and PMA; it did not change Rust behavior or establish conformance.

Subsequent Rust changes add scoped AP, CW, FI, and RS primitives, an opt-in
atomic RS HTTP snapshot, mandatory reserved-kind refusals, and narrower
advertisement. The source inventory now covers all 17 files separately from
the original 15-fixture baseline at
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. Neither source synchronization nor
that inventory proves complete conformance. See the
[sync assessment](2026-09-26-upstream-nip-sync.md) for the original findings and
the [fixture ledger](block-nip-ledger.md) for current evidence.

The current pure `nostr` suite passes 276 tests and strict all-target Clippy.
The dedicated RS HTTP suite passes against a fresh disposable Postgres database;
remaining live relay changes are pending the current manual verification gate.

## Agent identity and turns

- **NIP-OA:** strict owner-attestation grammar, BIP-340 signature, conditions,
  and event-bound preimage validation are owned domain primitives. The first
  accepted owner for an agent becomes its immutable main-owner relation.
- **NIP-AA:** on a closed relay, a non-member agent can satisfy NIP-42 with a
  valid owner attestation only when the owner is an active member. That
  connection gets virtual membership for the agent and owner-aggregated event
  rate accounting. Direct members retain ordinary NIP-42 behavior.
- **NIP-AO:** kind 24200 is authenticated, signature-checked, NIP-44-shaped,
  restricted to the materialized owner/agent pair, and limited independently
  per IP and agent. It remains ephemeral and routes only to the tagged
  authenticated recipient. Unknown frame types receive a successful silent
  drop as the specification requires.
- **NIP-AM:** kind 44200 is authenticated as the agent, checked against the
  immutable owner relation, stored outside full-text search, and returned only
  to a query/live recipient authenticated as its `p`-tagged owner.

## Stored private and shared data

- **NIP-AE:** kind 30174 requires one 64-hex `d`, one valid owner `p`, and a
  NIP-44 v2 envelope. Addressable replacement remains scoped to the agent
  author. Reads require either that authenticated agent author or exact
  authenticated owner scope; content never enters FTS.
- **NIP-AP:** kinds 30175 and 30178 enforce their `d` and exact sharing-tag
  grammar while keeping plaintext content opaque to the relay. Unshared heads
  are author-only; explicitly shared heads can be read publicly. ACL checks
  happen before ordering, limits, COUNT, and live fanout. The new portable
  `acp_command` and session-policy client rules now have typed helpers in
  [`nostr::agent_persona`](../../crates/nostr/src/agent_persona.rs). They validate
  known fields, redact machine-local commands from shared catalogs, restrict
  foreign adoption to portable aliases, preserve an owner's existing custom
  command during redacted catalog replay, and identify session-boundary changes
  that require restart. They do not implement local executable discovery, an
  ACP launcher, catalog UI, or permission to execute a command. Keeping content
  opaque remains the relay role.
- **NIP-ER:** kind 30300 validates the address, NIP-44 envelope,
  `not_before`, and expiration ordering. It is author-private and delivered
  lazily when a normal authenticated REQ is made; NIP-11 advertises that lazy
  due mode and the enforced one-year horizon.
- **NIP-MP:** kind 30621 validates its non-empty address, bounded unique
  repository coordinates, and bounded metadata. It uses ordinary NIP-01
  addressable storage and reads.

## Authenticated relay commands

- **NIP-IA:** fresh protected 9035/9036 requests accept self, configured relay
  administrator, or request-borne verified owner consent. With relay signing
  configured, one transaction updates the archive registry and inserts the
  relay-signed 8002/8003 delta plus current 13535 snapshot. Those generated
  kinds are rejected from clients.
- **NIP-DV:** authenticated NIP-29 group members may send 41010/41012 for an
  exact `h` channel. One transaction changes their hidden set and replaces the
  relay-signed, recipient-private 30622 snapshot; clients cannot forge that
  snapshot kind.
- **NIP-WP:** a fresh authenticated 9033 command from
  `NOSTR_RELAY_MANAGEMENT_PUBKEY` atomically changes the singleton workspace
  icon. Every process reads it from Postgres when serving NIP-11, so no cache
  or restart is needed.

## Partial and missing roles

### NIP-CW: channel service and thread primitives

With a relay URL and signing key, `POST /query` serves the existing channel
window when `top_level` is true. It returns top-level rows in
`(created_at DESC, id ASC)` order and a relay-signed kind `39006` bounds event.
A nonexistent or inaccessible channel returns an empty array without bounds;
access is checked again after the read. Clients cannot publish relay-owned
kinds `39005`, `39006`, or `39007`. Unsupported `thread_window` and
`resolve_thread_roots` HTTP modes refuse rather than becoming ordinary filters.

[`nostr::thread_window`](../../crates/nostr/src/thread_window.rs) now supplies
strict normalized thread batches, request bindings, signed `39007` bounds
verification, and shared query-budget accounting. It binds the exact reader,
community, channel, root, selection fields, and cursor. These are pure helpers:
there is no thread-mode database adapter, full auxiliary reconstruction,
batch-wide access refresh, or deleted-reply root recovery in the running relay.
The existing channel service also derives summaries and auxiliary events from
a bounded fetched slice; it is not proof of every current CW guarantee.

NIP-11 therefore does **not** advertise `nip-cw`. The explicit channel endpoint
remains available; broad CW discovery must wait for the missing server roles.

### NIP-RS: private storage and optional atomic snapshot

Kind `30078` has addressable replacement plus author-only reads. The relay
retains the base `nip-rs` storage declaration. Setting
`NOSTR_RELAY_READ_STATE_COMMUNITY` to a UUID with `NOSTR_RELAY_URL` enables the
optional snapshot path. NIP-11 includes `read_state_snapshot` only when the
request Host matches the configured origin; a relay signing key is unnecessary.

`POST /query` accepts exactly one raw filter with `read_state_snapshot: 1`,
`kinds: [30078]`, and the NIP-98 signing key as its sole author. The writer
transaction consumes the authorization event, checks membership, block/allow
policy, and writer status, and reads one complete retained cut. It includes
unrelated application coordinates and applies no ordinary query cap, `#t`
filter, or age window.
The hard ceilings are 4,096 events and 8 MiB separately for stored content/tag
bytes and the compact event array. Oversize, corrupt, ambiguous, unauthorized,
and replayed requests never receive a partial `complete` envelope.

If the same database cut contains an expired but retained own-author coordinate,
the whole snapshot returns HTTP 503 until that row is physically removed by the
expiration sweep. It neither returns an expired event nor silently omits retained
state from a `complete` result. Other authors' expired rows do not block the cut.

[`nostr::read_state_snapshot`](../../crates/nostr/src/read_state_snapshot.rs)
provides strict raw request and response parsing, signature and coordinate
checks, and the community/author/event-set digest. This authenticates the supplied
cut; the hash alone does not prove server completeness, act as a compare-and-swap
token, or establish freshness after the statement. The dedicated
[`read_state_snapshot_postgres`](../../crates/nostr-relay/tests/read_state_snapshot_postgres.rs)
fixture passes against actual HTTP and a fresh Postgres database. It covers
origin-bound discovery without a signing key, complete results above the
ordinary limit, strict malformed and duplicate fields, replacement/deletion,
reader isolation, changing membership, replay after restart, retained expiration,
recovery after physical sweeping, corruption, and
both hard byte limits plus count overflow. Corrupt stored-event decoding also
triggers the relay's existing process-wide fail-closed behavior.

The RS merge/manual-unread application client is still absent. Ordinary history
and EOSE do not establish the required cross-subscription complete-load barrier;
that remains separate from this atomic HTTP alternative.

### NIP-PL: disabled delivery prototype

The previous configured HTTP stub did not implement transactional lease
admission, complete generation/endpoint authority, current membership checks,
or a durable delivery outbox. Configuration now rejects any push executor, and
NIP-11 never advertises `nip-pl` or a push descriptor. The gateway refuses
lease publication without an enabled executor. Retained parser, encryption,
filter, and constant-body fixtures are narrow evidence, not a live delivery
service.

A public executor still needs atomic lease/replacement and endpoint accounting,
durable deduplicated dispatch and retry state, generation and membership
rechecks before send, authenticated HTTPS delivery with stable job identity,
and provider outcomes. The public gateway additionally requires enrollment,
App Attest/delegation, renewal, recovery, limits, and key retirement. Disabling
the incomplete prototype does not implement those roles.

### NIP-FI: offline policy primitives

[`nostr::federated_identity`](../../crates/nostr/src/federated_identity.rs)
implements strict compact-token and header parsing, issuer/community policy,
accepted token classes, exact proven-key comparison, bounded session deadlines,
and issuer-scoped temporary deny-set updates. It passes the exact token to a
caller-supplied trusted `OfflineVerifier`; the crate supplies no JWT signature
algorithms or JWKS network client. Fixtures exercise policy, malformed evidence,
deadlines, denial classes, and issuer isolation with a test verifier.

The relay has no FI handshake integration, verified key distribution/cache,
session registry, protected-route enforcement, or active issuer disconnect.
It does not advertise FI. The helpers are not a deployed federation service.

### NIP-PMA: reserved kind refused

The relay rejects kind `30179` through `validate_block_ingest` and public write
admission, and hides it from ordinary history, live delivery, and counts.
This implements the upstream draft's mandatory first gate while the private
managed-agent runtime is unavailable. It does not implement private access,
transactional updates, backup, revocation, or migration. PMA is not advertised.
Historical imports and restored bytes remain retained data, not an authorized
managed-agent runtime; the read gates must still apply to them.

### NIP-GS: client primitives

`nostr::git_sign` signs and verifies the armored Git-object envelope,
including an optional owner attestation bound into the hash. NIP-GS defines
no event kind and is not advertised by the relay.

## Current advertisement behavior

NIP-11 always advertises `nip-mp`, `nip-oa`, and the `nip-rs` storage foundation.
With NIP-42 configured it adds `nip-aa`, `nip-ae`, `nip-am`, `nip-ao`, `nip-ap`,
and `nip-er`. Relay signing enables `nip-dv` and `nip-ia`, and a configured
management pubkey enables `nip-wp`. The optional RS snapshot has its own
Host-bound descriptor. CW, PL, GS, FI, and PMA are not advertised.

These declarations name relay roles, not every client or application behavior
in a specification. A RUN journal can cite AO kind `24200` only as non-durable
telemetry; it can cite AM kind `44200` and AE kind `30174` by ID. Such citations
do not implement a missing host role.

## Evidence and remaining work

The original 15 specifications retain fixtures under `tests/fixtures/`.
Additional pure tests live beside their owning modules, and the live Postgres
contracts run through [`scripts/test-postgres.sh`](../../scripts/test-postgres.sh)
on separate disposable databases. Existing fixture presence is distinct from
a fresh passing run; the current RS suite result is stated above.

| Specification | Evidence | Remaining role boundary |
| --- | --- | --- |
| NIP-OA | `tests/fixtures/nipoa/`, `gateway_postgres` | Owner-attestation admission |
| NIP-AA | `tests/fixtures/nipaa/`, `gateway_postgres` | Virtual membership, not FI |
| NIP-AO | `tests/fixtures/nipao/`, `gateway_postgres` | Ephemeral private routing |
| NIP-AM | `tests/fixtures/nipam/`, `gateway_postgres` | Owner-scoped metric reads |
| NIP-AE | `tests/fixtures/nipae/`, `gateway_postgres` | Owner/agent engram reads |
| NIP-AP | `agent_persona` tests, shared-head ACL fixtures | Host launcher, local resolution, and catalog UI absent |
| NIP-ER | `tests/fixtures/niper/`, `gateway_postgres` | Lazy due delivery |
| NIP-MP | `tests/fixtures/nipmp/`, `gateway_postgres` | Addressable project storage |
| NIP-IA | `tests/fixtures/nipia/`, `gateway_postgres` | Archive command transaction |
| NIP-DV | `tests/fixtures/nipdv/`, `gateway_postgres` | Hidden-set snapshot |
| NIP-WP | `tests/fixtures/nipwp/`, `gateway_postgres` | Workspace icon command |
| NIP-CW | `channel_window`, `thread_window`, `block_lane_postgres` | Thread helpers exist; corresponding server modes and complete recovery absent |
| NIP-RS | `read_state_snapshot` and passing `read_state_snapshot_postgres` | Atomic snapshot available; merge/manual-unread client and ordinary-EOSE barrier absent |
| NIP-GS | `nostr::git_sign` tests | Git-object client primitive |
| NIP-PL | `push_lease` tests; configuration rejection | Delivery disabled; no public executor/gateway |
| NIP-FI | `federated_identity` tests | Offline policy only; external crypto/JWKS and relay integration absent |
| NIP-PMA | Reserved-kind unit and gateway fixtures | Required refusal only; managed-agent state runtime absent |

The [source and fixture ledger](block-nip-ledger.md) preserves the old baseline
separately from the current 17-file inventory and these new scoped tests.
