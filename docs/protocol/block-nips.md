# Block NIP Server Contract

nostr-relay adopts all 15 specifications pinned under `nips/block/` at Buzz
commit `8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. This page states the relay
behavior. A role is advertised only when the process is configured to serve it.

## Main agent identity and turns

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
  happen before ordering, limits, COUNT, and live fanout.
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

## Relay semantics and current deployment boundaries

- **NIP-CW:** `POST /query` serves a window when `top_level` is true. The
  page is top-level rows in `(created_at DESC, id ASC)` order, with one
  relay-signed kind 39006 bounds event. A closed group the reader is not in
  returns an empty array and no bounds event. WebSocket `REQ` still ignores
  the extension fields. The relay advertises `nip-cw` when `NOSTR_RELAY_URL`
  and `NOSTR_RELAY_SECRET_KEY` are set, because overlays are signed by that
  key. Clients cannot publish kinds 39005 or 39006.
- **NIP-RS:** kind 30078 uses ordinary NIP-01 addressable replacement. The
  existing Postgres high-water boundary plus buffered live handoff provides
  the required full-state EOSE barrier across processes; no bespoke mutable
  read-state table is introduced. NIP-RS is advertised.
- **NIP-GS:** the pinned spec signs Git objects and defines no event kind.
  `nostr::git_sign` signs and verifies the armored envelope, including an
  optional owner attestation bound into the hash. The relay does not
  advertise `nip-gs`.
- **NIP-PL:** kind 30350 is accepted only when `NOSTR_RELAY_PUSH_SECRET`,
  `NOSTR_RELAY_PUSH_GATEWAY`, and `NOSTR_RELAY_URL` are set. The executor
  decrypts the lease, checks origin, generation, and filter narrowing, and
  stores the event. A later match posts the fixed APNs reconnect constant to
  the configured `http://` gateway. The body does not contain the event.
  Without that configuration the relay still answers `restricted: push
  executor is not configured or advertised` and does not advertise `nip-pl`.
  FCM and UnifiedPush stay refused because the pinned text does not register
  their constants.

`POST /query` and the push executor run inside the one relay binary. NIP-11
lists `nip-cw` only with a relay signing key and `nip-pl` only with an
executor key and gateway. NIP-GS remains a client. A RUN journal may cite an
AO kind 24200 event only as non-durable telemetry, and may cite AM kind 44200
and AE kind 30174 events by id. Those citations do not change the upstream
kinds.

NIP-11 always advertises `nip-mp`, `nip-oa`, and `nip-rs`. With NIP-42
configured it also advertises `nip-aa`, `nip-ae`, `nip-am`, `nip-ao`, `nip-ap`,
and `nip-er`; relay signing additionally enables `nip-dv` and `nip-ia`, while a
configured management pubkey enables `nip-wp`. Every protocol surface has a
committed fixture; the live Postgres gateway contract covers admission,
privacy, derived state, and cross-process visibility.
