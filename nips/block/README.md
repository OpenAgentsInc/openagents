# Block (Buzz) extension NIPs

This README is maintained by OpenAgents. The sync script preserves it because
upstream does not publish a README for this directory. The **17 specification
files** beside it are exact copies from
[block/buzz](https://github.com/block/buzz/tree/781d39510cf23cfe224e8f521ae06a23377e06de/docs/nips)
at commit `781d39510cf23cfe224e8f521ae06a23377e06de`, recorded in the
[manifest](../manifest.json).

These drafts are implementation targets; PMA is explicitly a reservation
that must remain rejected until its deployment gates are met. Retaining a
specification does not establish a working or conformant role. The
[relay implementation status](../../docs/protocol/block-nips.md),
[fixture ledger](../../docs/protocol/block-nip-ledger.md), and
[2026-09-26 sync assessment](../../docs/protocol/2026-09-26-upstream-nip-sync.md)
separate current code, older evidence, and remaining work. In particular, the
existing CW, RS, and PL code implements only parts of their specifications;
FI and PMA have no implementation.

| Spec | Scope | Kinds | Current implementation boundary |
| --- | --- | --- | --- |
| [NIP-AA](NIP-AA.md) | Agent authentication | NIP-42 `22242` | Owner-attested relay admission. |
| [NIP-AE](NIP-AE.md) | Agent engrams | `30174` | Envelope validation and private storage/read gates. |
| [NIP-AM](NIP-AM.md) | Agent turn metrics | `44200` | Durable owner-private metrics envelopes. |
| [NIP-AO](NIP-AO.md) | Agent observability | `24200` | Ephemeral recipient-scoped routing. |
| [NIP-AP](NIP-AP.md) | Agent personas | `30175`, `30178` | Envelope/sharing ACLs; no persona adoption or ACP command consumer. |
| [NIP-CW](NIP-CW.md) | Channel and thread windows | Relay-signed `39005`, `39006`, `39007` | HTTP channel-window subset; no thread mode or `39007` client-publication guard. |
| [NIP-DV](NIP-DV.md) | DM visibility | Relay-signed `30622` | Recipient-private hidden-set projection. |
| [NIP-ER](NIP-ER.md) | Event reminders | `30300` | Private reminders with lazy due delivery. |
| [NIP-FI](NIP-FI.md) | Federated identity | No new kind | Not implemented or advertised. |
| [NIP-GS](NIP-GS.md) | Git object signing | No event kind | Git signing/verifying client primitives. |
| [NIP-IA](NIP-IA.md) | Identity archival | `9035`/`9036`, `8002`/`8003`, `13535` | Authenticated commands and signed archival state. |
| [NIP-MP](NIP-MP.md) | Multi-repository projects | `30621` | Envelope validation and addressable storage. |
| [NIP-OA](NIP-OA.md) | Owner attestation | `auth` tag | Verification primitive consumed by agent admission and commands. |
| [NIP-PL](NIP-PL.md) | Push leases | `30350` | Configured HTTP prototype; not a conformant public push executor or gateway. |
| [NIP-PMA](NIP-PMA.md) | Private managed-agent aggregate | Reserved `30179` | No implementation; required rejection is still missing. |
| [NIP-RS](NIP-RS.md) | Cross-device read state | Reuses `30078` | Addressable storage only; no RS client or atomic snapshot. |
| [NIP-WP](NIP-WP.md) | Workspace profile | `9033` | Management command and NIP-11 icon. |

## Specification summaries

The following summaries describe the pinned contracts, not a claim that every
role is implemented. Use the status table above and linked evidence for that
distinction.

### NIP-OA — Owner attestation

The root credential of the set. An owner key authorizes an agent key
through an optional 4-element `auth` tag
`["auth", owner_pubkey, conditions, sig]` whose signature covers
`SHA256("nostr:agent-auth:" || event.pubkey || ":" || conditions)`. The
conditions grammar allows `kind=` and `created_at` clauses joined by
`&`. The event stays authored solely by the agent key: the tag is
provenance evidence, never identity override. It deliberately reuses the
NIP-26 tag shape without NIP-26 delegation semantics. NIP-AA, NIP-AP,
NIP-GS, NIP-IA, and NIP-MP all consume it.

### NIP-AA — Agent authentication

Relay admission for owned agents on a NIP-43 membership relay. An agent
whose owner is an active member satisfies NIP-42 AUTH by carrying a
valid NIP-OA credential inside its kind 22242 AUTH event and receives
virtual membership without separate enrollment, with event rates
aggregated against the owner. Removal of the owner revokes the agent on
its next connection. Condition `kind=` clauses are not enforced at
connection admission.

### NIP-AE — Agent engrams

Persistent structured agent memory: addressable kind 30174 events signed
by the agent key and NIP-44-encrypted with the agent-owner conversation
key, so the owner can always read everything the agent remembers.
Defines `core` and `mem/...` records with HMAC-blinded `d` tags (the
slug never leaks), head selection with monotonic `created_at`,
tombstones, and an optional `[[wiki-link]]` reachability graph. Reads
require the authenticated agent author or exact owner; content never
enters full-text search.

### NIP-AM — Agent turn metrics

Durable encrypted accounting: one kind 44200 event per completed agent
turn, NIP-44-encrypted to the owner, carrying harness and model
identifiers, per-turn and session-cumulative token counts, estimated
cost, and `(sessionId, turnSeq)` ordering. The relay verifies the
agent-owner relation, stores durably, excludes the content from search,
and gates every read (including id lookups) to the tagged owner. The
durable sibling of NIP-AO: metrics here, never conversation content.

### NIP-AO — Agent observability

Ephemeral encrypted streaming of live agent session telemetry (protocol
frames, turn start/end) from agent to owner, and control commands such
as `cancel_turn` from owner to agent, on kind 24200. Relays must not
persist, index, or log these events; fanout is in-memory only, routed
solely to the tagged authenticated recipient after verifying the
agent-owner relation. Unknown frame types receive a successful silent
drop.

### NIP-AP — Agent personas

Addressable agent blueprints on kind 30175: display name, system prompt,
model, provider, runtime, avatar, ACP transport selection, session policy,
and behavioral defaults, with plaintext `d` slugs for discovery. Secrets
belong in the NIP-AE `mem/persona` engram instead. Kind 30178 projects a shared team
catalog embedding sanitized member definitions. Relays enforce
author-only reads unless a head is explicitly shared, across every read
surface, before ordering, limits, COUNT, and live fanout. The shared
catalog uses portable ACP aliases and strips machine-local commands; the
client must preserve redacted local commands on owner replay and keep local
definition hashes separate from catalog projection hashes. These client
behaviors are not implemented here.

### NIP-CW — Channel window

Relay-computed channel and thread views. Channel pagination uses top-level
filtering and a composite timestamp/ID cursor, with signed thread summaries
(`39005`) and channel bounds (`39006`). The new thread mode adds signed
`39007` bounds, strict query binding, newest-first reply pagination, bounded
batches, access revalidation, and deleted-reply root-summary recovery.
Clients must distinguish explicit unsupported mode from authorization,
signature, and resource failures; those failures do not permit fallback.
The current relay serves a channel-window subset and rejects client
`39005`/`39006`, but has no thread mode or `39007` rejection yet.

### NIP-DV — DM visibility

A relay-signed per-viewer projection of DM hide state (kind 30622,
`d` = viewer pubkey, one `h` tag per hidden DM channel) derived as a
side effect of the existing Buzz DM open/hide commands (41010/41012).
Hidden DMs still receive messages and can be re-opened; the snapshot
only drives sidebar filtering. Reads are strictly owner-gated and the
relay-only kind cannot be client-forged.

### NIP-ER — Event reminders

Encrypted author-only reminders: addressable kind 30300 events with a
public `not_before` due-time tag while the target, note, and status
stay NIP-44 self-encrypted — the relay learns when a reminder is due
but never what it is. Covers pending/done/cancelled transitions through
replacement, snooze, push versus lazy due-signal delivery, NIP-42
author-only read gating, and NIP-11 `supported_extensions`
advertisement.

### NIP-FI — Federated identity

An optional identity-admission layer pairing issuer-qualified `(iss, sub)`
claims with fresh Nostr key possession. Host/community policy selects allowed
issuers and audiences; bounded JWT/JWKS validation, session deadlines,
protected HTTP admission, and issuer-scoped disconnects govern access.
Neither an email address nor NIP-42 alone establishes this identity. No new
event kind is allocated. This repository has no FI implementation or
advertisement.

### NIP-GS — Git object signing with Nostr keys

Signing git commits and tags with Nostr secp256k1 keys through git's
pluggable signing-program interface: an armored JSON envelope with a
domain-separated signing hash (`nostr:git:v1:`), strict canonical-JSON
anti-malleability rules, and GPG status-line emulation. The optional
`oa` field embeds a NIP-OA owner attestation inside the signed hash so
owner authorization cannot be stripped or injected. Defines no event
kind and no relay behavior; signatures live in git objects.

### NIP-IA — Identity archival

Relay-scoped archiving of retired or stale pubkeys: hide them from
active-member surfaces while preserving history. Explicitly not a ban
and not global reputation. User-signed archive/unarchive requests
(9035/9036) produce relay-signed deltas (8002/8003) with a recorded
consent path and a relay-signed authoritative snapshot (13535).
Self-unarchive is mandatory as the anti-shadowban path, and an owner
may archive a "zombie agent" by proving the NIP-OA relation.

### NIP-MP — Multi-repository projects

An addressable kind 30621 project event grouping NIP-34 repository
announcements by coordinate, across owners, as metadata only — it
grants zero authority over member repositories and is never consulted
by push policy. Specifies strict ingest validation (member cap,
coordinate grammar, duplicate rejection), a deterministic client-side
fold with claim authority resting on repo owners/maintainers, and
exhaustive-pagination requirements.

### NIP-PL — Push leases

Mobile push without a shadow feed: a stored expiring installation-scoped
authorization (kind 30350, random `d`, mandatory NIP-40 expiration,
NIP-44-encrypted descriptor) asking an executor to keep a constrained
filter alive and wake the app through APNs/FCM/UnifiedPush. The wake
payload is a fixed reconnect constant — no event content transits the
platform push services. Requires strict filter narrowing (self-`#p`
only, allow-listed kinds), generation-watermarked replacement, and
tenant binding; includes a normative public Buzz APNs gateway profile with
App Attest enrollment, recoverable enrollment results, delegated delivery,
and signed requests. The existing local HTTP stub does not implement that
profile or the complete executor lifecycle.

### NIP-PMA — Private managed-agent aggregate

Reserves owner-private kind `30179` for an encrypted managed-agent aggregate,
with preserved agent identity and exact signed public projection references.
The pinned document explicitly requires rejection until the privacy, atomic
compare-and-swap, backup, revocation, and migration prerequisites exist.
Ordinary addressable replacement is insufficient. The current relay lacks
both that implementation and the required rejection; generic admission can
accept the kind, so it must not be used for private agent state here.

### NIP-RS — Cross-device read state sync

Syncs a user's own per-context read positions across devices as
self-encrypted NIP-78 kind 30078 blobs tagged `read-state` — explicitly
not read receipts. Uses a grow-only max-register CRDT merge,
per-installation slot coordinates, optional hierarchical thread
frontiers, and a formally model-checked manual-unread override layer
whose durability rules require a fenced full-state load with a delivery
barrier and no GC horizon for tombstones. The optional atomic snapshot
profile returns all retained own-author `30078` heads at one writer-database
cut, with a community/reader-bound digest and whole-request failure on
incompleteness. Neither that snapshot nor an RS merge client exists here;
ordinary storage and EOSE are not proof of a complete read-state load.

### NIP-WP — Workspace profile

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
