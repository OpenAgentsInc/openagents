# Block NIP implementation status

The [Block lane](../../nips/block/README.md) contains **17 specifications**
pinned at Buzz commit `781d39510cf23cfe224e8f521ae06a23377e06de`. These are
implementation targets. The 2026-09-26 sync changed AP, CW, PL, and RS and
added FI and PMA; it did not change Rust behavior or establish conformance.

This page describes code inspected at that sync. Existing fixtures cover
selected behavior developed against the earlier 15-file pin
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`; they do not cover all requirements
of either pin. See the [sync assessment](2026-09-26-upstream-nip-sync.md) for
the source-level review and remaining work, and the
[fixture ledger](block-nip-ledger.md) for the known stale pin/count test.
The two source-ledger guards were run and failed at their old-pin assertions;
no full Rust gate or live Postgres acceptance was run.

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
  `acp_command`, session-policy, shared-catalog projection, and local adoption
  rules require client behavior that is not implemented here. Keeping
  content opaque is consistent with the relay role.
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

### NIP-CW: channel-window subset

`POST /query` serves a channel window when `top_level` is true. The page
contains top-level rows in `(created_at DESC, id ASC)` order and a relay-signed
kind `39006` bounds event. A closed group the reader is not in returns an
empty array without bounds. WebSocket `REQ` ignores the extension fields.
Clients cannot publish kinds `39005` or `39006`.

The new thread-window mode, signed `39007` bounds, query-wide batch budgets,
access-refresh barrier, and deleted-reply root-summary recovery are absent.
The relay-only kind guard also omits `39007`, so generic client admission can
accept it. That guard is required implementation work, not an available
protection. The documented legacy thread-pagination mode is also absent.

Existing channel behavior has narrower evidence than full CW conformance:
summaries and auxiliary events are derived from a bounded fetched slice, and
a nonexistent channel currently receives empty bounds, contrary to the
pinned contract. The live fixture tests that current behavior; its presence
does not resolve the mismatch. NIP-11 currently advertises `nip-cw` when
`NOSTR_RELAY_URL` and `NOSTR_RELAY_SECRET_KEY` are set. That coarse label does
not establish support for the new thread mode or complete channel semantics.

### NIP-RS: storage foundation

Kind `30078` uses ordinary NIP-01 addressable replacement. There is no RS
merge/manual-unread client and no optional atomic snapshot implementation:
no discovery descriptor, strict snapshot request parser, writer-database
snapshot envelope, or client completeness verifier.

The previous version of this page claimed that ordinary history high-water
and buffered live handoff established the required full-state EOSE barrier
across processes. That claim was too broad. History completion and durable
notification delivery run independently, and no dedicated fixture establishes
the cross-subscription barrier needed for a complete RS load. Static review
identified a delayed-notification/replacement race to test; this assessment
did not reproduce a live failure. Ordinary query arrays or EOSE must not be
presented as verified complete read-state snapshots. NIP-11 still advertises
`nip-rs`; the current evidence supports only the storage foundation.

### NIP-PL: configured HTTP prototype

Kind `30350` is accepted only when `NOSTR_RELAY_PUSH_SECRET`,
`NOSTR_RELAY_PUSH_GATEWAY`, and `NOSTR_RELAY_URL` are set. The executor
validates and decrypts the lease, checks origin, generation, and filter
narrowing, then stores the event. A later match can post the fixed APNs
reconnect constant to a configured `http://` stub. The body does not contain
the triggering event. Without that configuration, the relay refuses the
lease and does not advertise `nip-pl`. FCM and UnifiedPush are refused.

This is a partial prototype, not a conformant public executor or Buzz APNs
gateway. Important preexisting gaps remain:

- Endpoint uniqueness, generation checks, and event replacement occur across
  separate operations, rather than one admission transaction. Prior state
  comes from a capped list of unexpired events rather than a durable complete
  lease/watermark book.
- Match-time filtering does not recheck current group membership. A constant
  wake body still exposes unauthorized wake timing after membership loss.
- Dispatch uses a non-durable spawned task, without a durable deduplicated
  outbox, retry state, or a current-generation check immediately before send.
- The request uses `X-Push-Endpoint` and a constant body; it has no NIP-98
  signature, stable job ID, endpoint grant, HTTPS support, or provider-response
  handling. There is no public App Attest/enrollment/delegation service or
  executor-key retirement lifecycle.

The newly pinned public gateway changes add requirements for enrollment
recovery, limits, conflict handling, renewal, and signed delivery path
binding. They were not implemented by this sync. NIP-11 currently advertises
`nip-pl` whenever push configuration exists; that behavior overstates the
available role. Close or disable the incomplete paths before relying on a
conformant service. The [assessment](2026-09-26-upstream-nip-sync.md) separates
these older gaps from the new upstream changes.

### NIP-FI: not implemented

Existing NIP-42 and NIP-98 verification are foundations, not federated
identity. The relay has no issuer/community policy, assertion/JWKS validator,
FI session deadlines, protected-route FI admission, or issuer-disconnect
implementation. FI has no fixtures and is not advertised. It requires a
separate deployment design and tests across every protected transport.

### NIP-PMA: required rejection missing

Kind `30179` is reserved for private managed-agent state. The upstream draft
requires relays to reject it until private access, transactional updates,
backup, revocation, and migration prerequisites exist. None of those roles
is implemented here, and the current generic admission path does not reject
it. Ordinary historical/live read paths also lack a PMA-specific privacy
gate. Do not publish private agent state in this kind to this implementation.

The first required change is explicit rejection across public write paths,
with an audit of imported or restored rows and their read visibility. Keeping
ciphertext in ordinary addressable storage is not PMA support. PMA is not
advertised.

### NIP-GS: client primitives

`nostr::git_sign` signs and verifies the armored Git-object envelope,
including an optional owner attestation bound into the hash. NIP-GS defines
no event kind and is not advertised by the relay.

## Current advertisement behavior

`POST /query` and the push prototype run inside the relay binary. NIP-11
always advertises `nip-mp`, `nip-oa`, and `nip-rs`. With NIP-42 configured it
also advertises `nip-aa`, `nip-ae`, `nip-am`, `nip-ao`, `nip-ap`, and `nip-er`.
Relay signing additionally enables `nip-dv` and `nip-ia`, and a configured
management pubkey enables `nip-wp`. CW requires the relay URL and signing key;
PL requires push configuration. GS, FI, and PMA are not advertised.

These are observations of current code, not a conformance certificate. CW,
RS, and PL claims need to be narrowed or completed as described above. A RUN
journal can cite AO kind `24200` only as non-durable telemetry; it can cite
AM kind `44200` and AE kind `30174` by ID. Such citations do not change the
upstream kinds or prove an unimplemented host role.

## Existing evidence and its limits

The first 15 specifications have retained fixtures under `tests/fixtures/`,
consumed by relay fixture suites and protocol unit tests. The live Postgres
contracts are `gateway_postgres` and `block_lane_postgres`, invoked by
[`scripts/test-postgres.sh`](../../scripts/test-postgres.sh). This table names
existing tests; it does not report a fresh run or complete current-pin
conformance. FI and PMA have no corresponding fixtures.

| Specification | Fixtures | Existing evidence |
| --- | --- | --- |
| NIP-OA | `tests/fixtures/nipoa/` | `gateway_postgres` owner-attestation admission. |
| NIP-AA | `tests/fixtures/nipaa/` | `gateway_postgres` virtual membership. |
| NIP-AO | `tests/fixtures/nipao/` | `gateway_postgres` ephemeral routing. |
| NIP-AM | `tests/fixtures/nipam/` | `gateway_postgres` owner-scoped reads. |
| NIP-AE | `tests/fixtures/nipae/` | `gateway_postgres` owner/agent reads. |
| NIP-AP | `tests/fixtures/nipap/` | Shared-head ACLs; no ACP catalog/adoption consumer. |
| NIP-ER | `tests/fixtures/niper/` | `gateway_postgres` lazy due delivery. |
| NIP-MP | `tests/fixtures/nipmp/` | `gateway_postgres` addressable storage. |
| NIP-IA | `tests/fixtures/nipia/` | `gateway_postgres` archive transaction. |
| NIP-DV | `tests/fixtures/nipdv/` | `gateway_postgres` hidden-set snapshot. |
| NIP-WP | `tests/fixtures/nipwp/` | `gateway_postgres` workspace icon. |
| NIP-CW | `tests/fixtures/nipcw/` | `block_lane_postgres` channel windows, not new thread/batch/recovery modes. |
| NIP-RS | `tests/fixtures/niprs/` | Addressable storage; no dedicated complete-load barrier or atomic snapshot proof. |
| NIP-GS | `tests/fixtures/nipgs/` | `nostr::git_sign` unit tests. |
| NIP-PL | `tests/fixtures/nippl/` | Encrypted lease to constant-body local stub; no public gateway or full executor proof. |
| NIP-FI | None | Not implemented or advertised. |
| NIP-PMA | None | No implementation or mandatory rejection guard. |

The [in-process ledger](block-nip-ledger.md) still pins the old commit and
15-file count in Rust. Its inventory assertion failed against the newly
synced manifest in the [focused run](2026-09-26-upstream-nip-sync.md#verification-and-limits). That mismatch is intentionally disclosed; this documentation
update does not change constants and present unimplemented roles as covered.
