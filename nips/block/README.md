# Block (Buzz) Extension NIPs

This README is owned by the Immortal repository. It is not synced from
upstream; `scripts/sync-nips.sh` preserves it because the upstream Buzz
tree does not publish a README for this directory. The 15 specification
files beside it are exact copies from
[block/buzz](https://github.com/block/buzz/tree/main/docs/nips) at the
commit pinned in `../manifest.json`.

Buzz is Block's relay-as-workspace team chat and agent platform ("the
relay is the workspace"). All 15 specs are `draft` `optional`. Owner
adoption on 2026-08-03 pulled the full set into Immortal; the exact
server contract, advertisement rules, and deliberate non-advertisement
cases live in `../../docs/protocol/block-nips.md`.

| Spec | Title | Kinds | Relay behavior |
| --- | --- | --- | --- |
| NIP-AA | Agent Authentication | none new (NIP-42 22242) | yes |
| NIP-AE | Agent Engrams | 30174 | yes |
| NIP-AM | Agent Turn Metrics | 44200 | yes |
| NIP-AO | Agent Observability | 24200 | yes |
| NIP-AP | Agent Personas | 30175, 30178 | yes |
| NIP-CW | Channel Window | 39005, 39006 (relay-signed) | yes |
| NIP-DV | DM Visibility | 30622 (relay-signed) | yes |
| NIP-ER | Event Reminders | 30300 | yes |
| NIP-GS | Git Object Signing | none (git objects) | no |
| NIP-IA | Identity Archival | 9035/9036, 8002/8003, 13535 | yes |
| NIP-MP | Multi-Repository Projects | 30621 | yes |
| NIP-OA | Owner Attestation | none (an `auth` tag) | no |
| NIP-PL | Push Leases | 30350 | yes |
| NIP-RS | Read State Sync | reuses 30078 | no |
| NIP-WP | Workspace Profile | 9033 | yes |

## Per-spec summaries

### NIP-OA — Owner Attestation

The root credential of the set. An owner key authorizes an agent key
through an optional 4-element `auth` tag
`["auth", owner_pubkey, conditions, sig]` whose signature covers
`SHA256("nostr:agent-auth:" || event.pubkey || ":" || conditions)`. The
conditions grammar allows `kind=` and `created_at` clauses joined by
`&`. The event stays authored solely by the agent key: the tag is
provenance evidence, never identity override. It deliberately reuses the
NIP-26 tag shape without NIP-26 delegation semantics. NIP-AA, NIP-AP,
NIP-GS, NIP-IA, and NIP-MP all consume it.

### NIP-AA — Agent Authentication

Relay admission for owned agents on a NIP-43 membership relay. An agent
whose owner is an active member satisfies NIP-42 AUTH by carrying a
valid NIP-OA credential inside its kind 22242 AUTH event and receives
virtual membership without separate enrollment, with event rates
aggregated against the owner. Removal of the owner revokes the agent on
its next connection. Condition `kind=` clauses are not enforced at
connection admission.

### NIP-AE — Agent Engrams

Persistent structured agent memory: addressable kind 30174 events signed
by the agent key and NIP-44-encrypted with the agent-owner conversation
key, so the owner can always read everything the agent remembers.
Defines `core` and `mem/...` records with HMAC-blinded `d` tags (the
slug never leaks), head selection with monotonic `created_at`,
tombstones, and an optional `[[wiki-link]]` reachability graph. Reads
require the authenticated agent author or exact owner; content never
enters full-text search.

### NIP-AM — Agent Turn Metrics

Durable encrypted accounting: one kind 44200 event per completed agent
turn, NIP-44-encrypted to the owner, carrying harness and model
identifiers, per-turn and session-cumulative token counts, estimated
cost, and `(sessionId, turnSeq)` ordering. The relay verifies the
agent-owner relation, stores durably, excludes the content from search,
and gates every read (including id lookups) to the tagged owner. The
durable sibling of NIP-AO: metrics here, never conversation content.

### NIP-AO — Agent Observability

Ephemeral encrypted streaming of live agent session telemetry (protocol
frames, turn start/end) from agent to owner, and control commands such
as `cancel_turn` from owner to agent, on kind 24200. Relays must not
persist, index, or log these events; fanout is in-memory only, routed
solely to the tagged authenticated recipient after verifying the
agent-owner relation. Unknown frame types receive a successful silent
drop.

### NIP-AP — Agent Personas

Public addressable agent blueprints on kind 30175: display name, system
prompt, model, provider, runtime, avatar, and reserved behavioral
defaults, with plaintext `d` slugs for discovery (secrets belong in the
NIP-AE `mem/persona` engram instead). Kind 30178 projects a shared team
catalog embedding sanitized member definitions. Relays enforce
author-only reads unless a head is explicitly shared, across every read
surface, before ordering, limits, COUNT, and live fanout.

### NIP-CW — Channel Window

A relay-computed cursor-paged view of a channel's top-level timeline
through extended NIP-01 filter fields (`top_level`, a composite
`until` plus `before_id` cursor), fixing tag-absence filtering and
same-second pagination. Two relay-signed overlay families report thread
summaries (39005) and window bounds (39006, the pagination authority);
clients cannot publish them. On relays that do not implement the
extension the fields degrade safely to a standard NIP-01 filter.

### NIP-DV — DM Visibility

A relay-signed per-viewer projection of DM hide state (kind 30622,
`d` = viewer pubkey, one `h` tag per hidden DM channel) derived as a
side effect of the existing Buzz DM open/hide commands (41010/41012).
Hidden DMs still receive messages and can be re-opened; the snapshot
only drives sidebar filtering. Reads are strictly owner-gated and the
relay-only kind cannot be client-forged.

### NIP-ER — Event Reminders

Encrypted author-only reminders: addressable kind 30300 events with a
public `not_before` due-time tag while the target, note, and status
stay NIP-44 self-encrypted — the relay learns when a reminder is due
but never what it is. Covers pending/done/cancelled transitions through
replacement, snooze, push versus lazy due-signal delivery, NIP-42
author-only read gating, and NIP-11 `supported_extensions`
advertisement.

### NIP-GS — Git Object Signing with Nostr Keys

Signing git commits and tags with Nostr secp256k1 keys through git's
pluggable signing-program interface: an armored JSON envelope with a
domain-separated signing hash (`nostr:git:v1:`), strict canonical-JSON
anti-malleability rules, and GPG status-line emulation. The optional
`oa` field embeds a NIP-OA owner attestation inside the signed hash so
owner authorization cannot be stripped or injected. Defines no event
kind and no relay behavior; signatures live in git objects.

### NIP-IA — Identity Archival

Relay-scoped archiving of retired or stale pubkeys: hide them from
active-member surfaces while preserving history. Explicitly not a ban
and not global reputation. User-signed archive/unarchive requests
(9035/9036) produce relay-signed deltas (8002/8003) with a recorded
consent path and a relay-signed authoritative snapshot (13535).
Self-unarchive is mandatory as the anti-shadowban path, and an owner
may archive a "zombie agent" by proving the NIP-OA relation.

### NIP-MP — Multi-Repository Projects

An addressable kind 30621 project event grouping NIP-34 repository
announcements by coordinate, across owners, as metadata only — it
grants zero authority over member repositories and is never consulted
by push policy. Specifies strict ingest validation (member cap,
coordinate grammar, duplicate rejection), a deterministic client-side
fold with claim authority resting on repo owners/maintainers, and
exhaustive-pagination requirements.

### NIP-PL — Push Leases

Mobile push without a shadow feed: a stored expiring installation-scoped
authorization (kind 30350, random `d`, mandatory NIP-40 expiration,
NIP-44-encrypted descriptor) asking an executor to keep a constrained
filter alive and wake the app through APNs/FCM/UnifiedPush. The wake
payload is a fixed reconnect constant — no event content transits the
platform push services. Requires strict filter narrowing (self-`#p`
only, allow-listed kinds), generation-watermarked replacement, and
tenant binding; includes a normative public Buzz APNs gateway profile.

### NIP-RS — Cross-Device Read State Sync

Syncs a user's own per-context read positions across devices as
self-encrypted NIP-78 kind 30078 blobs tagged `read-state` — explicitly
not read receipts. Uses a grow-only max-register CRDT merge,
per-installation slot coordinates, optional hierarchical thread
frontiers, and a formally model-checked manual-unread override layer
whose durability rules require a fenced full-state load with a delivery
barrier and no GC horizon for tombstones.

### NIP-WP — Workspace Profile

Sets the relay-scoped workspace icon: an admin/owner-signed kind 9033
command carrying one validated `icon` tag (scheme-allow-listed,
size-capped), checked against NIP-43 role state, then served to every
client through the standard unauthenticated NIP-11 `icon` field, so the
read path needs no custom code.

## How the specs fit together

NIP-OA is the root credential the others consume in different
verification contexts (relay admission, git envelopes, archival
authority, spawn attestation, deletion). The agent lifecycle stack runs
NIP-AP (definition) -> NIP-OA (attestation) -> NIP-AA (relay access) ->
NIP-AE (private memory) -> NIP-AO (live telemetry) alongside NIP-AM
(durable metrics) -> NIP-IA (retirement) -> NIP-GS (signed code
output). NIP-IA, NIP-DV, NIP-CW, and NIP-WP share the relay-signed
workspace-state pattern anchored to the NIP-11 relay identity, and the
pre-numbering drafts advertise through NIP-11 `supported_extensions`
rather than `supported_nips`.
