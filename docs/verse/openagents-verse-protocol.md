# `openagents.verse`: the Verse protocol specification and plan

## Post-reset scope and authority

This is the defining protocol plan for **every OpenAgents surface**, including
web, mobile, desktop, coding agents, cloud hosts, decisions, payments and Verse.
**Coax is the implementation target. Metareal is the engine; Verse is the
experience built on it.** Shared engine contracts stay under
`openagents.engine.*`; world/product contracts stay under `openagents.verse.*`.

Recovered `.proto` files and frozen descriptors define published shapes.
Prose sketches define planned coverage, not already generated or deployed APIs.
The historical inventories below describe the pre-reset implementation: an
"Implemented" label does not mean a deleted crate or service is installed in
this checkout. References to old crates/NIPs/ADRs are pinned historical evidence,
not a requirement to rebuild them or introduce new Rust product code. Read them
from the pre-reset revision recorded in `proto/recovery.json`.

The root [protocol guide](../../proto/README.md) identifies what is actually
restored and executable. New alpha files cover experimental options and the
first Verse world/presence projection; they do not amend Nostr, REACH or realm
transports. Coax's `#import_proto` and production decoders are not implemented.

The [Metareal migration](../../proto/migrations/README.md) names old IDs, new
canonical IDs and the required reader/writer transition. `legacy/<key>` in
this catalog refers to that registry, never to a literal wire ID. Frozen
snapshots and original signed records retain their bytes. Only the typed
asset-profile catalog migration is automated here; other contract, bridge
and signature transitions require actual consumer gates.

Status: **design and migration plan**, reconciled 2026-10-27. The root tree now includes alpha Verse common/presence foundations; other
package sketches are planned. Existing engine, common, HOST, and
REACH packages were previously shipped and remain the baseline in the master spec §2.6.
Other `.proto` blocks are incomplete proposals. Their `.v1` names indicate
targets; new representations remain alpha until they pass the conformance gate.
Never overwrite a published message or change its wire types to fit a sketch.

This is **Chapter V** of [the OpenAgents Protocol](../protocol/openagents-protocol-spec.md),
the specification for every OpenAgents project. That document owns the
principles, descriptor options, limits, Nostr mirrors and the whole package
tree. Where this chapter's §4 sketches the tree outside `verse`, the master
spec's §4 replaces it.

The master spec also owns the single options vocabulary, shared refs/errors,
consumer profiles, transport compatibility, and publication process. This
chapter does not redefine them. Accepted ADRs and NIP text remain authoritative;
future design sketches do not amend them.

This chapter is the specification and build plan for `openagents.verse`, the
Protobuf namespace for everything 3D that OpenAgents makes: the Verse
metaverse, its worlds, zones, planets and the galaxy; avatars, agents and
townsfolk; presence, chat and social; the authoritative realm service;
simulation, physics, combat and game rules; authored world documents and
their edits, publishing and offers; assets, private assets, presentation and
device profiles; quests, XP projection and games; Agent Studio, the Gym
building and in-world terminals; replays and film; and playtest telemetry.

It covers what is built today, in this repository (`crates/verse*`,
`crates/metareal*`, `apps/metareal`, the NIP-MV client) and in
`Metareal source archive`. It also covers what the glossary, the NIPs and
the design documents say we intend to build. The aim is that one versioned,
public schema tree describes the whole 3D vision. Anyone, inside or outside
OpenAgents, should be able to implement a compatible client, host, tool or
agent from `proto/` and this document alone.

## Contents

1. [Why one namespace](#1-why-one-namespace)
2. [What we have today](#2-what-we-have-today)
3. [Principles](#3-principles)
4. [Where `openagents.verse` sits in the whole protocol tree](#4-where-openagentsverse-sits-in-the-whole-protocol-tree)
5. [Layout, naming and versioning](#5-layout-naming-and-versioning)
6. [Message classes, authority and unknown fields](#6-message-classes-authority-and-unknown-fields)
7. [Limits, canonical encodings and JSON](#7-limits-canonical-encodings-and-json)
8. [Nostr, signed bytes and other formats proto does not replace](#8-nostr-signed-bytes-and-other-formats-proto-does-not-replace)
9. [Package catalog](#9-package-catalog)
10. [Package specifications](#10-package-specifications)
11. [Services](#11-services)
12. [Migration map: every current format and where it goes](#12-migration-map-every-current-format-and-where-it-goes)
13. [Tooling, generation and consumers](#13-tooling-generation-and-consumers)
14. [Publishing the protocol](#14-publishing-the-protocol)
15. [Conformance and verification](#15-conformance-and-verification)
16. [The build plan](#16-the-build-plan)
17. [Risks](#17-risks)
18. [Planned scope and constraints](#18-planned-scope-and-constraints)
19. [Open questions for the owner](#19-open-questions-for-the-owner)
20. [Sources](#20-sources)

---

## 1. Why one namespace

The Verse is currently spread over about forty wire formats. They come in
four families:

- **Nostr events.** NIP-MV kinds `33300`, `33301`, `23300`, `23301` and
  `23302`; NIP-C7 chat; NIP-29 rooms; NIP-17 private messages; NIP-XP;
  NIP-EVAL; NIP-HOST and REACH artifacts. Their bodies are JSON with
  one-letter keys.
- **Length-prefixed JSON on the chamber wire.** `verse-world::service`, wire
  version 34, carries realm login, commands, movement confirmations and
  spatial deltas.
- **Ad-hoc JSON over HTTP and `postMessage`.** In Metareal engine: the
  `verse.world.v1` world document, `/api/world`, `/api/planets`,
  `/api/change`, `/api/events`, `/api/presence` and `/api/npc/ask`, plus the
  iOS `legacy/save-bridge-v1`, `verseShell` and `verseWallet` bridges. In this
  repository: the Metareal tools host protocol (versions 1 to 8), `ritual.json`
  and `chamber.json`.
- **Named JSON documents.** `openagents.verse-world-tree.v1`,
  `openagents.verse-npc.v1`, `openagents.verse.private-asset.v1`,
  `haunt.v1`, `bunny.*.v1`, `snap.*.v1`, `verse.input-replay.v1`,
  `verse.game-replay.v1`, `legacy/command-v1`, `legacy/release-v1`
  and others.

Every one of these is reasonable on its own terms. Together they have three
costs:

1. **Each language writes its own reader.** Rust, C++, JavaScript, Swift,
   Kotlin and Python each re-implement the same shapes. The Metareal engine HTTP
   presence route was inventoried with a 7-number look while `LookSpec` has 12;
   verify the current route before migrating it.
   Mismatches like that are what independent hand-written readers produce.
2. **Nobody outside can build against us.** There is no single place an
   outside developer can read "this is a Verse pose", "this is a world
   document" or "this is how you publish a planet".
3. **Product truth outruns contract truth.** Planets, offers, townsfolk,
   rumors, shared bodies and the realm wire all ship without a schema that
   says what they are.

`openagents.engine.v1` (historical engine epic #113, six phases, closed) has already
shown the fix at engine level. Checked-in schemas, generated bindings for
Rust, C, C++, Wasm, Python and Swift, immutable descriptor snapshots and
shared fixtures replaced hand-written readers for scenes, HUD, controls,
world saves, asset profiles and phone-agent tasks. `openagents.verse` takes
the same discipline from the engine up to the whole metaverse.

## 2. What we have today

### 2.1 The engine package (`openagents.engine.v1`, implemented)

`proto/openagents/engine/v1/` holds six schemas, each with Nanopb `.options`
limits:

| File | Contract |
| --- | --- |
| `scene.proto` | `SceneMetadata`, `CoordinateConvention` (Y up, meters, +Z forward), `Vec3`, `Landmark`, `InteractionRange`, `AxisAlignedBox`, `AssetManifest`, `Artifact`, `BufferDescriptor`, `CompiledScene`, `ScenePlacement`, `SceneLight`, `SceneCamera` |
| `agent.proto` | Phone-agent `AgentTask`, `AgentEvent`, `ToolActivity`, `AgentOperation`, `AgentDispatch`, `AgentProgress` |
| `hud.proto` | `ConversationSnapshot`/`Delta`, `HudFrame` and its parts, `HudInvocation` |
| `control.proto` | `EngineRequest`/`Reply`/`Event`, `EngineCapabilities`, `CameraControl`, `EntityEdit`, `InputIntent`, `WorldLoad` |
| `state.proto` | `RegisteredFieldKind`, `ComponentSchema`, `WorldSnapshot`, `WorldDelta` (revision-fenced, explicit entity order), `AuthoredSource` |
| `assets.proto` | `ConsumerAssetProfile`, `AssetProfileCatalog`, compiled from `profiles.json` |

`crates/engine-proto` owns generation (protoc 29.3, Prost 0.13.5, Nanopb
0.4.9.1, Python protobuf 5.29.3), validation, compatibility snapshots
(`compatibility/*.descriptor.pb`), cross-language fixtures and the
hash-pinned export bundle. Metareal engine imports that bundle into
`third_party/openagents-engine-proto`, and Swift uses the C++ codec through a
C ABI. The proto README sets the rules this document inherits:

- reserve retired numbers and names;
- published descriptors are immutable;
- durable readers reject unknown fields;
- display-only readers accept additive fields;
- no generic `Any`/JSON fallback;
- 16 MiB / 64 levels / 250,000 values;
- integers exact within 2^53−1 at JSON boundaries;
- Nostr, Loro, signed bytes and provider protocols keep their own formats.

The current implementation target is Coax in this repository. The engine is
Metareal; the Verse is its product/world layer. The scalar bootstrap and its
[spec](../../coax/spec.md) do not yet implement aggregate types or protobuf
decoders. Current root contracts and the Metareal rename migration are listed
in [the protocol guide](../../proto/README.md).

Preserve native Metal, host-owned storage, the C ABI and GPU buffer layouts
while adopting Coax. Proto is the boundary schema, not the native frame layout.
The old app/engine inventories below are pre-reset evidence. They are not
claims that the deleted application crates have been restored.

### 2.2 What the Verse uses that is not yet proto

[§12](#12-migration-map-every-current-format-and-where-it-goes) lists every
format with its destination. In short:

- **Presence and social.** NIP-MV, world chat, rooms, private messages, Gym
  notes, the NOSTR tab, Metareal engine HTTP presence and chat.
- **Authoritative play.** The chamber/realm wire (v34), login challenge,
  character roles, commands, movement, spatial deltas, transfers, parties,
  guilds, trades, persistence and checkpoints (`verse-chamber-owned-v24`),
  input replay.
- **Worlds and authoring.**
  - World and zone documents: the `verse.world.v1` world document and its
    house delta, `haunt.v1`, the zone catalog (`openagents.zones.v1`), zone
    atmosphere and portals.
  - The world model: the world tree, `legacy/world-v1` and
    `legacy/command-v1`.
  - Hosting and publishing: the Metareal tools host protocol v1–v8, prop
    worlds, publish, offers, edit log, world settings, planets, the galaxy
    and awards.
- **Characters and agents.** Accounts, sessions, player keys, `LookSpec`,
  outfits, inventory, NPCs, brains, routines, rumors, the town clock,
  `npc/ask`, workshop-agent records, Agent Studio snapshots and updates,
  crew verdicts.
- **Content.**
  - Assets: VTP3 packs, private assets and placements, source manifests,
    fantasy catalogs, the `asset-packs.js` manifest, device profiles.
  - Presentation and simulation: the atmosphere/`look`/`env` records,
    particles, water, weather, destruction events.
- **Games.** `haunt.v1`, `game.v1` (Ice Cream Man), `bunny.*`, `snap.*`,
  `verse.game-replay.v1`, the proposed mobile game bridge, playables.
- **Measurement.** Analytics events, survey answers, remark kinds, playtest
  sessions and automation marking.
- **Media.** Film take wire, shots and cuts.

## 3. Principles

These are migration constraints derived from existing sources. Shipped
contracts, accepted ADRs, and NIP text take precedence. Proposed behavior
changes need a separate reviewed amendment before implementation.

1. **A schema describes; it never authorizes.** Successful decoding grants no
   permission. A capability list, a profile ID, a release `capabilities`
   field, a signature or a digest each states something, and none of them
   is a grant. Authority is the host's separate check of tenant, project,
   role, grant, scope, expiry and revocation (SEC-001, INV-01).
2. **Four data classes never share a writable field.** These are the
   *authored document*, *runtime state*, *presence* and *release identity*
   (Metareal INV-05). Every message declares its class (§6). Promoting runtime
   state into a document is an explicit capture or bake command, never a
   field copy.
3. **Clients send intent, never results.** Commands carry intent and aim.
   They never carry trusted position, damage, rolls, cost, rewards or XP.
   Results come from the authority that owns them (`docs/verse/engine/architecture.md`,
   `combat-model.md`).
4. **Signed and content-addressed bytes keep their exact form.** Nostr event
   IDs are computed over NIP-01 JSON. Release, channel and signature
   statements are JCS JSON. Asset bytes are hashed as stored. Proto may
   *mirror* these or *carry them as bytes*, but it never re-encodes them in
   a way that changes what was signed or hashed (§8).
5. **Closed vocabularies; no escape hatch.** There is no `google.protobuf.Any`,
   `Struct` or free JSON map for component data, commands or rules. An
   existing authored format that has not been modeled yet travels as a
   named `AuthoredSource{contract, bytes}` with a declared contract ID, and
   that is a migration debt, not a design. Unknown operations, kinds and
   required profiles fail closed.
6. **Bounded everything.** Every string, bytes field and repeated field has
   a maximum. Every decoder enforces it before allocating, including in C
   and Wasm.
7. **Profiles are pinned, not negotiated.** A world pins its scene, asset,
   presentation, physics, rules and authority profiles by exact ID and
   revision. An unknown required profile refuses entry. Saves and replays
   record the exact profiles they were made under (`zone-rules.md`).
8. **Revisions fence mutations; request IDs deduplicate retries.** Mutating
   requests carry an expected revision and an idempotency key. Undo is a
   new compensating command. Revisions never decrease (INV-02).
9. **One schema, many renderers.** Nothing in `openagents.verse` encodes a
   GPU layout, shader, script, screen layout or color literal.
   Presentation is tokens, profiles and data. Per-frame input and GPU
   buffers keep their direct native paths.
10. **Public by default; private data stays off the wire.** The schemas are
    public. Licensed asset bytes, MetaHuman sources, owner keys, emails,
    tokens and free text written without consent never appear in a public
    record. Fields that can carry personal data are annotated (§7.4).

## 4. Where `openagents.verse` sits in the whole protocol tree

The master spec §4 owns the complete namespace catalog. Shipped shared refs
and errors stay in `openagents.common.v1`; there is no parallel `type.v1`
rewrite. Verse-specific geometry, world IDs, and profiles live in
`openagents.verse.common`, and engine-neutral runtime contracts stay in
`openagents.engine.v1`. Verse never becomes an engine dependency.

Verse can import shared common/options/engine types, the UI types it actually
uses, and selected pinned `nostr.*` mirrors. Other owners remain independent:
XP awards, agent identity/control, evaluation, host authority, and payment
settlement are referenced by their existing identities and verified at their
own boundaries. `verse.host` is a world/client bridge, not the native
`openagents.host` authority. These two namespaces must not be conflated.

Lightning Labs RPC modules keep upstream packages, bytes, include roots, and
licenses. A Verse consumer imports payment views/refs, not LND RPC trees.
Consumer profiles select roots and transitive dependencies; a phone or Wasm
bundle is not the whole protocol catalog. Optional private-record and webhook
aggregation lives outside core common/API types and is selected only by the
carriage code that needs it.

## 5. Layout, naming and versioning

### 5.1 Directory and package layout

Each domain is its own package with its own version, so presence can reach
`v2` without forcing a new world-document version:

```
proto/openagents/verse/
  README.md                  index of packages, status, owners, consumers
  common/v1/common.proto     WorldId, EntityRef, Pose, Cell, Stamp, ProfileRef, ContentPin
  world/v1/                  world definitions, zones, portals, profiles, catalog
  doc/v1/                    authored world documents (areas, objects, npcs, triggers, foliage)
  authoring/v1/              commands, revisions, prefabs, structural model, merge
  publish/v1/                releases, channels, publication, offers, edit log, settings
  presence/v1/               NIP-MV bodies: entity state, frames, gestures, bodies, objects
  social/v1/                 chat lines, scopes, rooms, mutes, buddies, parties, guilds
  realm/v1/                  authoritative session wire: login, commands, deltas, transfer
  sim/v1/                    ticks, physics profiles, replays, checkpoints, determinism
  rules/v1/                  rules profiles, abilities, effects, conditions, dice, combat events
  character/v1/              accounts, characters, appearance, outfits, inventory, progression
  npc/v1/                    townsfolk, brains, routines, rumors, town clock, dialogue
  place/v1/                  world tree, places, affordances, object states, known subgraphs
  agent/v1/                  agents in the world: seats, studio, crew, workshop agents
  quest/v1/                  quests, steps, facts, triggers, rewards (projections of XP)
  galaxy/v1/                 planets, galaxy, visits, invitations, the universe picker
  asset/v1/                  packs, catalogs, private assets, placements, provenance, budgets
  presentation/v1/           atmosphere, environment, lighting, look grades, quality, particles
  environment/v1/            water, weather, destruction, terrain events
  host/v1/                   client↔host bridges: Metareal tools host ops, mobile shell, game bridge
  gym/v1/                    Gym building projections: boards, notes, recipes (views only)
  terminal/v1/               in-world screen bindings to NIP-TERM terminals
  media/v1/                  in-world voice/video placement (LIVE references), film takes and cuts
  telemetry/v1/              playtest events, survey, remarks, consent, automation marking
  economy/v1/                in-world prices, offers, settlement views, wallet bridge (refs only)
  game/
    haunt/v1/                The Haunted House rules and documents
    icecream/v1/             Ice Cream Man
    bunny/v1/                Bunny garden, progress, tutorial, run receipts
    snap/v1/                 Snap Station
    duel/v1/                 Prussian duel, hand duel
    arena/v1/                arena encounter rules
    deadgears/v1/            Dead Gears
  space/v1/                  Lagrange 1/2: station, spacecraft, assembly, orbital profiles
  lab/v1/                    Physics Lab, robot lab, render lab scenario contracts
```

Each package directory holds `*.proto` files and, until §13's
options-generation lands, `*.options`. Files are named by subject
(`frame.proto`, `body.proto`), not by message.

### 5.2 Naming

- **Packages:** `openagents.verse.<domain>.v<N>`, and for games and
  scenarios `openagents.verse.game.<game>.v<N>`. Lowercase, no underscores
  in domain names. Games use their product slug.
- **Messages:** `PascalCase` nouns (`EntityState`, `PoseFrame`). Requests
  and replies are `VerbNounRequest` and `VerbNounReply` (`PublishWorldRequest`),
  matching `EngineRequest`/`EngineReply`. Events are past-tense nouns
  (`PieceBroken`, `QuestCompleted`).
- **Fields:** `snake_case`. Units go in the name when they are not obvious:
  `_m`, `_mm`, `_ms`, `_s`, `_deg`, `_rad`, `_mps`, `_msat`, `_bytes`. Keep
  the suffixes engine.v1 already uses (`label_height_m`, `vertical_fov_degrees`).
- **Enums:** `ENUM_NAME_VALUE`. Zero is always `*_UNSPECIFIED` and always
  invalid on a durable or intent message. Open vocabularies that third
  parties extend (gestures, roles, object kinds) are `string` slugs with a
  documented registry, not enums. A new enum value would otherwise require
  every reader to upgrade before it can accept a record.
- **IDs:** `string` with the grammar written in the field comment and
  enforced by the validator. Shared grammars live in `common.v1`: slug
  `^[a-z0-9][a-z0-9_-]{0,63}$`, entity ID `^[a-z0-9_-]{1,64}$`, scene ID
  `^[A-Za-z0-9_-]{1,48}$`, UUID canonical lowercase, digest
  `sha256:<64 hex>` as text or 32 raw bytes as `bytes`.
- **Wire names that must match NIP JSON keys** (`p`, `q`, `k`, `s`, `n`,
  `e`, `b`, `o`) are *not* used as proto field names. Proto uses readable
  names (`position`, `orientation`, `stamp`). The NIP JSON mapping is a
  separate, tested table (§8.3). `json_name` is never used to imitate NIP
  keys, because protobuf's JSON mapping is not the NIP canonical form.

### 5.3 Versioning

There are four independent version axes. The proto README already
separates them, and this spec keeps them separate:

| Axis | Example | Changes when |
| --- | --- | --- |
| Package version | `openagents.verse.presence.v1` | A field's meaning changes, a required field is added, or a field's type changes |
| Message `schema` field | `WorldDocument.schema = 1` | A durable record's content rules change within a package, read by an explicit migration |
| Profile ID | `verse-bare.bodies.v1`, `ruins.wizard-woods.v1` | A rules, physics, asset or presentation profile's behavior changes |
| Container/wire version | AESV v2, chamber wire 34, host protocol 8 | The framing or envelope changes |

Rules:

- **Additive changes stay in the package:** new optional fields, new
  messages, new enum values on display-only messages.
- **Frozen descriptors.** Every published package version gets a
  descriptor snapshot in `compatibility/`, and CI fails on any incompatible
  change against all frozen snapshots (`buf breaking` plus the existing
  engine-proto checker).
- **Never reuse a retired field.** Reserve both the number and the name.
- **New package versions.** A `v2` package is a new directory. Its
  messages are new types, and an explicit converter in the owning crate
  maps `v1` to `v2`. Readers keep the `v1` reader for as long as any
  durable `v1` record exists. For world documents and saves that means
  forever.
- **Stability labels.** Packages under `v1alpha1` can break between
  commits and are never imported by a durable store. They become `v1` only
  through the §15 gate. Every new Verse representation starts at `v1alpha1`; stable source JSON
  still needs field parity, consumer evidence, and the conformance gate.

## 6. Message classes, authority and unknown fields

### 6.1 Classes and shared descriptor options

Use the master spec §6 proposal for `openagents.options`; this chapter has no
second `FieldLimits`, `Sensitivity`, or option-number definition. In particular,
`CONFIDENTIAL` and `SECRET` must have the same meaning everywhere, including
nested oneofs and upstream extension containers. Option annotations do not by
themselves implement bounds, redaction, or admission checks.

Shipped `.options`, generated descriptors, and validators retain their existing
behavior. A shared options generator must first reproduce that output and
prove semantic parity. New fields whose omitted/default/null states differ
use explicit presence plus semantic validation under the master spec §7.5.
Required scalar zero/false is distinct from missing; repeated omission needs
a wrapper if the source contract distinguishes it from an empty list.

### 6.2 What each class means for readers

| Class | Unknown fields | Missing required | Who may write | Persisted by |
| --- | --- | --- | --- | --- |
| Document | Reject on read-for-edit; keep as opaque preserved block only where a package declares a preservation rule | Reject | The authoring service, after command admission | Project store, saves |
| Runtime | Reject | Reject | The simulation authority only | Checkpoints, realm persistence |
| Presence | Ignore | Drop the record | Anyone, about their own entities | Relays (last-known), nowhere else |
| Release | Reject | Reject | Publisher, through the publish transaction | Release store, CDN |
| Intent | Reject | Refuse with `malformed` | A client with a session | Idempotency log (result, not request) |
| Receipt | Reject | Reject | The authority that performed the effect | Authority's ledger |
| View | Ignore | Show what is present | A projector of someone else's records | Not durable |
| Telemetry | Ignore | Drop the field | Instrumented pages and tools | Append-only event files |

One rule is non-negotiable: **a reader that rewrites a durable record never
silently drops fields it does not understand.** Protobuf's default
behavior (keep unknown fields in Rust and C++, drop them in some JSON paths)
is not relied on. A strict reader refuses the record. A preserving reader
uses an explicit `preserved` block a package declares.

### 6.3 Authority model per domain

| Domain | Authority today | Authority in this spec |
| --- | --- | --- |
| Presence (poses, entity state) | Each publisher about its own keys; no validation | Unchanged. Marked `PRESENCE`; never accepted as proof of position by any authority |
| Shared bodies (`verse-bare`, `verse-coast`) | Last toucher by stamp; a toy with no stakes | Unchanged. The profile declares `AUTHORITY_SERVERLESS_STAMPED`; any stakes need `AUTHORITY_HOSTED` |
| Realm/chamber play | `verse-world` host, 30 Hz | Unchanged. `realm.v1` intents in, receipts and deltas out |
| Metareal multiplayer (designed) | 60 Hz authoritative tick, session pins release | `realm.v1` with `sim.v1` tick profile; release pinned in session open |
| World documents | Metareal engine server owner check (`may_own`); Metareal tools project service | `authoring.v1` commands with revision fences; the project service is the only writer |
| Publication | Owner POST; designed signed channel CAS | `publish.v1`; Published only after channel CAS (PUB-003) |
| NPC routines and rumors | Pure functions of seed and clock; owner admits villagers | Unchanged; documents are owner-written, runtime is deterministic |
| XP, awards | Referee key signs NIP-XP | Verse only reads; `quest.v1` mirrors references |
| Payments | Wallet and settlement services | Verse only references; `economy.v1` is views and intents to the payment owner |
| Agent Studio | Host owns all state; client is a view | `agent.v1` snapshots are `VIEW`; operations go through NIP-HOST grants |

## 7. Limits, canonical encodings and JSON

### 7.1 Limits

- Each message declares `max_encoded_bytes`, and each bounded field
  declares `FieldLimits`. Future generators turn these into Nanopb
  `.options` (`max_length`, `max_count`, `FT_CALLBACK` for large
  collections), the Rust validator's constants, the C++ field tables
  (`contracts.inc`) and the Python limit module. That replaces today's
  hand-written `.options` files as the source of truth (§13.2). The
  generated `.options` are still checked in.
- Global ceilings follow engine.v1: 16 MiB per message, 64 nesting levels,
  250,000 visited values. Most Verse messages are far smaller, and each
  package states its own. For example, a pose frame fits under 4 KiB.
- The limits that exist today are kept exactly. They are listed with each
  package in §10. Example: 16 entities per pose frame, 64 bodies per body
  snapshot, 64-byte status, 150-character ALL chat, 24-character display
  names, 524,288-byte host envelope, 64 KiB mobile game bridge message.

### 7.2 Numbers and coordinates

- **Coordinates.** Use `openagents.engine.v1.CoordinateConvention`
  (`Y_UP_METERS_Z_FORWARD`), which NIP-MV shares: right-handed, +Y up,
  meters, rest facing +Z. Metareal tools's authored model states forward as
  −Z. That is a view convention of its editor, recorded per document as a
  convention enum, never inferred.
- **Floating point.** Runtime poses use `float` (`engine.v1.Vec3`). Global
  world positions that exceed float precision use a `double` `GlobalVec3`
  in `common.v1`, matching the kernel ABI's f64 world pose. Lagrange
  orbital frames use `double` with explicit frame IDs.
- **Structural coordinates.** Authored structural coordinates (walls,
  rooms, openings) are integer millimeters (`sint64 *_mm`), as
  `world-model.md` requires. Sub-millimeter input is refused, never
  rounded.
- **Quaternions** are `Quat{x,y,z,w}` floats. Readers normalize and
  refuse near-zero length.
- **Large integers.** Every integer that crosses a JSON presentation
  boundary stays within 2^53−1. Money is a decimal `string` of millisats,
  and counters that can exceed 2^53 are `string` with a grammar, as in the
  release schema.
- **Time.** Wall time is `int64 unix_ms` (`_ms` suffix). Simulation time
  is `uint64 tick` under a declared tick profile. `google.protobuf.Timestamp`
  is not used, because Nanopb and the C ABI are simpler with integers and
  NIP-MV already uses ms.

### 7.3 Deterministic encoding and hashing

Follow the master spec §7.4. Existing JCS and NIP identities retain their exact
hashing rules; blobs, packs, saves, and replay payloads are hashed as stored.
Verify original bytes before decoding a projection. Re-encoding a typed view
cannot establish the identity of a signed original.

A generic `openagents-proto-canon` encoder is not implemented or approved here.
House deltas and documents must preserve their existing identity rules during
migration. If independent producers later need a new logical fingerprint,
write a separate versioned format with field-specific semantics, explicit
presence/default and floating-point rules, and cross-language vectors first.
Standard deterministic protobuf output is insufficient for that purpose.
No Verse implementation phase depends on a new canonical encoder.

### 7.4 JSON

- **Proto3 JSON mapping** is supported for tools, logs and debugging, with
  `int64`/`uint64` as strings per the standard mapping.
- **Signed records are never protobuf JSON.** A NIP-MV body is NIP-MV
  JSON, a JCS statement is JCS, and both are produced by the explicit
  mapping layer (§8).
- **Personal fields** (`SENSITIVITY_PERSONAL`: chat text, survey free
  text, remarks, emails, display names tied to accounts, location trails)
  are redacted from `Debug`, logs and crash reports by generated
  formatting helpers. Telemetry records carry them only with a `consent`
  flag, matching the owner's 2026-10-27 rule for playtests: save
  everything, and say so in Settings.

## 8. Nostr, signed bytes and other formats proto does not replace

### 8.1 What stays outside proto as the authoritative form

| Format | Why | What proto does |
| --- | --- | --- |
| NIP-01 events and their JSON content | Event ID and signature cover the JSON serialization | Mirror types in `nostr.event.v1`; typed bodies in owning packages; tested two-way mapping |
| JCS statements (release, channel, signature) | Ed25519 over the original domain bytes identified by `legacy/signature-domain-v1` + JCS bytes | `publish.v1` mirrors fields; signatures carried as `bytes` alongside the exact JCS bytes |
| Content-addressed blobs (packs, textures, scene.bin, TWN2, AGR1, remarks.bin) | Digest covers stored bytes; custom GPU-ready layouts | `asset.v1` describes them (digest, size, format, buffers, budgets), never re-encodes them |
| Kernel ABI slabs (`AFB1`), behavior bytecode (`AFBCv1`) | Fixed-stride binary hot paths | `sim.v1` references their versions and digests |
| Loro CRDT updates and frontiers | Loro's own format | `authoring.v1` records frontier IDs and materialized digests |
| Per-frame input to the engine | Direct path, no allocation | `engine.v1.InputIntent` for remote/agent input only |
| Codex/Claude/provider protocols, Lightning RPC | Owned by others | Referenced by ID only |

### 8.2 Nostr mirror packages

A new `proto/nostr/` tree mirrors the pinned NIPs the way `nips/official/`
mirrors their text. It is outside `openagents.*`, because Nostr is not ours.
Its package is `nostr.*`, which signals a mirror. If a community schema for
Nostr appears, we adopt or align with it.

```proto
// proto/nostr/event/v1/event.proto (sketch)
package nostr.event.v1;
message Tag { repeated string values = 1; }      // ["w", "33300:<pk>:plaza"]
message Event {
  optional bytes id = 1;          // 32 bytes; sha256 of the NIP-01 serialization
  optional bytes pubkey = 2;      // 32 bytes x-only
  optional int64 created_at = 3;  // unix seconds
  optional uint32 kind = 4;
  repeated Tag tags = 5;
  optional string content = 6;    // exact content string; never re-serialized
  optional bytes sig = 7;         // 64 bytes Schnorr
}
message Filter { repeated bytes ids = 1; repeated bytes authors = 2; repeated uint32 kinds = 3;
  repeated TagFilter tags = 4; optional int64 since = 5; optional int64 until = 6; optional uint32 limit = 7; }
message TagFilter { optional string letter = 1; repeated string values = 2; }
```

Rules:

- `Event` carries `content` as the exact string. A typed body such as
  `openagents.verse.presence.v1.EntityState` is the *parsed* content.
  Converting the typed body back to `content` goes through the NIP's JSON
  mapping, and the resulting event is re-signed. Never verify a signature
  against re-serialized content.
- Each official NIP that defines a structured body gets
  `nostr.nip<NN>.v1`. Examples: `nip29` group metadata, `nip94` file
  metadata, `nip53` live activities, `nip32` labels. External NIPs we pin get
  `nostr.block.<id>.v1`: `oa` owner attestation, `ae` engrams, `ap`
  personas. Pins come from `nips/manifest.json`.
- The Verse uses these official NIPs: 01, 02 (planned buddies), 17, 24,
  29, 32, 42, 44, 53, 59, 65, 67, 94, C7. It also uses external OA, AA, AE,
  AP, AM, IA, MP and RS. Those packages are built first (§16, phase V1).

### 8.3 NIP-MV mapping table

`openagents.verse.presence.v1` is the typed body of NIP-MV. The mapping is
normative and tested both ways against the NIP's examples:

| NIP-MV JSON | Proto |
| --- | --- |
| `v` | `content_version` (must be 1) |
| `p` `[x,y,z]` | `position: engine.v1.Vec3` |
| `q` `[x,y,z,w]` | `orientation: common.v1.Quat` |
| `v` (in a frame entry) | `velocity: Vec3` |
| `w` (body) | `angular_velocity: Vec3` (rad/s) |
| `r` | `at_rest: bool` |
| `k` `[epoch, rev]` + owner | `stamp: common.v1.Stamp{epoch, rev, owner}` |
| `s`, `n`, `t` | `session`, `sequence`, `time_ms` |
| `e` | `entries: repeated FrameEntry` (≤16) |
| `b`, `set`, `o` | `bodies: repeated BodyRest` (≤64), `body_set`, `owner` |
| `a` | `animation: string` slug (`idle`, `walk`, `run`, `jump`) |
| `look{rig,shade}` | `look: AvatarLook{rig, shade}` |
| `g`, `d`, `at`, `to` | `gesture`, `duration_s`, `at: repeated Vec3`, `to: EntityRef` |
| Tags `w`, `c`, `d`, `role`, `z`, `p` | Event-envelope fields in `presence.v1.Envelope`, mapped to tags by the mapper |

The mapper lives in `crates/verse-net` (Rust) and is generated for C++ and
JS from the same table. Nothing writes NIP-MV JSON by hand any more.

## 9. Package catalog

### Current alpha foundation

The root tree currently compiles `openagents.verse.common.v1alpha1.WorldRef`
(world ID and optional revision) and
`openagents.verse.presence.v1alpha1.PeerState`/`PresenceSnapshot`. Required
IDs are nonempty and bounded to 128 UTF-8 bytes without NUL. Positions use
engine meters/Y-up/+Z-forward and finite components. Heading is optional,
finite radians within [-pi, pi]. Peer sequence and observation time require
explicit presence while allowing zero. Snapshots permit at most 256 unique
actors and 256 KiB. Unknown fields remain transient presence data.

Those are new projections, not full NIP-MV/realm mirrors or authority grants.
The reference fixtures check these limits and presence semantics; no Coax
production decoder is claimed. The larger catalog below remains the target.


### 9.1 The packages

| Package | Covers | Primary classes | Today's sources | Status |
| --- | --- | --- | --- | --- |
| `verse.common.v1` | Shared IDs, poses, cells, stamps, profile refs, content pins, refusals | (types) | NIP-MV, engine.v1, contracts.md | Designed |
| `verse.world.v1` | World definitions, zone catalog, portals, arrivals, atmosphere refs, profile tuples, instances | Release, View | NIP-MV `33300`, `ZoneId`, `openagents.zones.v1`, `zone-rules.md` | Partial (implemented as JSON/Rust) |
| `verse.doc.v1` | Authored world document: areas, doors, objects, NPC placements, voices, triggers, foliage, settings, planets | Document | `verse.world.v1` (Metareal engine), `haunt.v1`, `legacy/world-v1` | Implemented as JSON |
| `verse.authoring.v1` | Commands, operations, preconditions, revisions, house deltas, prefabs, structural building model, merges, presence of editors | Intent, Receipt, Document | `legacy/command-v1`, house delta, host `propose` ops, `collaboration.md` | Partial |
| `verse.publish.v1` | Releases, channels, signature statements (mirrors), publication state, offers, edit log, world settings, ownership | Release, Intent, Receipt | `legacy/release-v1`, `legacy/channel-v1`, `legacy/signature-v1`, `/api/world`, `/api/change`, `/api/log` | Partial |
| `verse.presence.v1` | NIP-MV bodies: entity state, pose frames, gestures, zone commands, shared bodies and snapshots, object states, looks | Presence | NIP-MV, `crates/verse-net/src/mv.rs`, `web/grid-session.js` | Implemented as JSON |
| `verse.social.v1` | World chat lines and scopes, rooms, private-message views, mutes, buddies, parties, guilds, moderation, hosted visits | Presence, Intent, View | NIP-C7/29/17 usage, `chat.md`, `/api/chat`, visits qualification | Implemented as JSON (parts designed) |
| `verse.realm.v1` | Authoritative session wire: hello, login challenge, character select, commands, confirmations, spatial deltas, acks, transfers, instances, errors | Intent, Runtime, Receipt | `verse-world::service` wire 34, `chamber.json` | Implemented as JSON |
| `verse.sim.v1` | Tick profiles, physics profiles, determinism scope, input replay, game replay, checkpoints index | Runtime, Release | `verse.input-replay.v1`, `verse.game-replay.v1`, `physics` crate, kernel ABI | Partial |
| `verse.rules.v1` | Rules profiles, abilities, spells, effects, conditions, dice reveal, combat events, encounter phases | Release, Runtime | `combat-model.md`, `verse-world/src/content.rs` | Implemented in Rust |
| `verse.character.v1` | Accounts (views), characters, roles, appearance (`LookSpec`, rigs, shades), outfits, equipment, inventory, progression, settings | Runtime, Document, View | Realm accounts, `LookSpec`, `verse-content` outfits, Metareal engine players | Partial |
| `verse.npc.v1` | Townsfolk, towns, brains, routines, rumors, the town clock, dialogue lines, ask requests, knowledge | Document, Runtime, Intent | `openagents.verse-npc.v1`, `verse-town.v1`, `verse-rumor.v1`, `/api/npc/ask`, host v8 `talk` | Implemented as JSON |
| `verse.place.v1` | World tree: districts, buildings, rooms, objects, standing points, affordances, object state, known subgraphs, place choice | Document, Presence | `openagents.verse-world-tree.v1` (`crates/world-tree`) | Implemented as JSON |
| `verse.agent.v1` | Agents in the world: seats, stations, studio snapshots/updates, crew, workshop agent views, greetings, scans | View, Presence | `agent-studio.md`, `workshop-agent.md`, `crew.md` | Implemented (views) |
| `verse.quest.v1` | Quests, steps, facts, trigger-driven progress, tutorial quests, XP projections | Document, Runtime, View | `haunt_quest.h`, `quest-*` commands, NIP-XP `30193` references | Partial |
| `verse.galaxy.v1` | Planets, planet setup, galaxy listing and polling, visits, invitations, universe picker entries | Document, View, Intent | `/api/planets`, `/api/award`, galaxy pages | Implemented as JSON |
| `verse.asset.v1` | Pack descriptors, catalogs, private assets, placements, source manifests, licenses, provenance, decode budgets, cache policy | Release, Document | VTP3, `openagents.verse.private-asset.v1`, source manifests, `asset-packs.js` | Implemented as JSON/binary |
| `verse.presentation.v1` | Atmosphere, environment (`env`), grades, lighting profiles, quality tiers, device-profile refs, particles, art styles | Document, Release | `look{}`, `env`, zone atmosphere, `particles.md`, `legacy/device-profile-v1` | Implemented as JSON |
| `verse.environment.v1` | Water and sea state, weather schedules, ice, destruction hierarchy and events, terrain edits | Runtime, Document | `water.md`, `coast.md`, `destructible-buildings.md` | Partial/Designed |
| `verse.host.v1` | Client↔host bridges: Metareal tools host ops (v1–v8 consolidated), mobile shell bridge, mobile game bridge, native save bridge, playables | Intent, Receipt, View | `metareal-host-protocol.md`, iOS `verseShell`/`legacy/save-bridge-v1`, `mobile-games.md` | Implemented as JSON |
| `verse.gym.v1` | Gym building views: board, run observations, recipes (refs), EVALS board rows, Gym notes | View | `gym.md`, `gym_evals.rs`, `gym_notes.rs` | Implemented (views) |
| `verse.terminal.v1` | In-world screens bound to NIP-TERM terminals; layout; share views | Intent, View | `in-world-terminal.md` | Designed |
| `verse.media.v1` | In-world voice/video placement (refs to LIVE), spatial audio zones, film takes, shots, cuts | View, Release | `film.md`, `crates/verse-film`, LIVE | Partial |
| `verse.telemetry.v1` | Playtest events, sessions, funnel, survey prompts and answers, remark classifications, automation marking, consent | Telemetry | `/api/events`, `web/survey.js`, `/api/survey/answer`, NIP-XP playtest `3196`/`3197` | Implemented as JSON |
| `verse.economy.v1` | Prices shown in the world, change offers, settlement views, wallet bridge envelopes, reward links | Intent, View | `/api/change`, `/api/settlement`, `verseWallet`, reward links | Implemented as JSON |
| `verse.game.*.v1` | Per-game documents, rules, progress, run receipts | Document, Runtime, Receipt | `haunt.v1`, `game.v1`, `bunny.*`, `snap.*`, duel, arena, Dead Gears | Mixed |
| `verse.space.v1` | Station state, spacecraft, assembly plans, CAD mass feed, orbital frames, EVA | Document, Runtime | Lagrange 1/2 crates and docs | Implemented locally |
| `verse.lab.v1` | Lab scenarios and knobs (Physics Lab, robot lab, render lab) | Document, View | `physics-lab.md`, render lab | Implemented locally |

### 9.2 Dependency order

These are proposed dependency cohorts. Compile the actual descriptors and
check their imports before starting a cutover. The shared/core engine graph
stays the one already shipped. The Verse graph must include every real edge,
including data types mentioned inside a sketch:

| Package | Verse prerequisites beyond common/engine |
| --- | --- |
| `npc`, `presentation`, `presence`, `economy`, `quest`, `lab` | None for the initial primitives. Shared `ArtStyle` and `GrantRef` stay in Verse common; errors reuse global common. |
| `doc` | `npc`, `presentation` |
| `authoring` | `doc` |
| `asset` | `presentation` (catalog light templates); no reverse import from presentation to doc |
| `world` | `presentation`, `asset` |
| `sim` | `world` (`ExecutionProfile.profiles`); it is not a leaf |
| `rules`, `environment` | `sim` |
| `character` | `doc` |
| `publish` | `doc`, `authoring` |
| `galaxy` | `doc`, `publish`, `world` |
| `place` | `npc` |
| `social`, `agent` | `presence` |
| `realm` | `world`, `sim`, `rules`, `environment`, `character`, `quest`; its grant reference is a common primitive, not a dependency on the full host bridge |
| `host` | `authoring`, `publish`, `character`, `npc`, `economy` and actual UI types used by bridge results |
| `gym`, `terminal`, `media`, `telemetry` | Their actual presence/social/host dependencies; no product-owner record embeddings |
| `game.*`, `space` | Actual used document/rules/sim/quest/presence prerequisites; nothing foundational imports game packages |

`presentation` must not import `doc` merely for an art-style enum while `doc`
imports presentation. Catalog light templates likewise use presentation-owned
light data rather than forcing asset→doc→presentation cycles. Replay command
bytes retain a named/versioned realm contract and exact-byte identity so that
sim does not import realm. A new dependency needs both this table and §16
updated; prose claims of acyclicity are not a compiler check.

### 9.3 What the Verse does not own, and only references

| Concept | Owner | How Verse refers |
| --- | --- | --- |
| XP awards, quests as NIP-XP records, trainer cards | `openagents.xp.v1` | `openagents.common.v1.EventRef` + projected `quest.v1.QuestView` |
| Eval results, Gym leaderboards | `openagents.eval.v1` | `EventRef`; `gym.v1` rows are views |
| Agent identity, definitions, sessions, control | `openagents.agent` | Pubkey + `EventRef` to `30187`; studio ops through NIP-HOST |
| Payments, wallets, settlement | `openagents.payment` (Bitcoin, Lightning, Taproot Assets only) | Payment/settlement IDs and msat strings; never invoices or keys in Verse records |
| Labor, work orders, deploy reports | `openagents.market` | `EventRef` for billboards and projections |
| Host grants, REACH channels, vault | `openagents.host` | Grant IDs and epochs inside realm session open |
| UI view tree | `openagents.ui.v1` | `host.v1` carries `ui.v1` views for world panels |

## 10. Package specifications

Each subsection gives the package's purpose, its key messages as a sketch,
its rules and limits, and what it replaces. The sketches leave out the
`openagents.options` annotations where they would only repeat the limits
stated underneath.

### 10.1 `openagents.verse.common.v1`

```proto
package openagents.verse.common.v1;
import "openagents/engine/v1/scene.proto";

message Quat { optional float x = 1; optional float y = 2; optional float z = 3; optional float w = 4; }
message GlobalVec3 { optional double x = 1; optional double y = 2; optional double z = 3; }      // f64 world pose
message Pose { engine.v1.Vec3 position = 1; Quat orientation = 2; engine.v1.Vec3 velocity = 3; }

// A world is either a 33300 address or an opaque ≤128-byte identifier agreed out of band.
message WorldId {
  oneof id {
    WorldAddress address = 1;
    string opaque = 2;        // e.g. "verse-plaza", "verse-bare", "physics-lab-v1"
  }
}
message WorldAddress { optional bytes pubkey = 1; optional string d = 2; }               // 33300:<pubkey>:<d>

message EntityRef { optional bytes publisher = 1; optional string entity_id = 2; }        // ^[a-z0-9_-]{1,64}$
message Cell { optional sint32 x = 1; optional sint32 z = 2; }                           // floor(x/cell), floor(z/cell)
message Stamp { optional uint64 epoch = 1; optional uint64 rev = 2; optional bytes owner = 3; }    // order: epoch, rev, owner hex

// Exact identity of bytes. sha256 is 32 raw bytes; byte_length is exact.
message ContentPin { optional bytes sha256 = 1; optional uint64 byte_length = 2; optional string media_type = 3;
                     optional string schema = 4; repeated string urls = 5; }  // urls locate; never identify

// A host-supported profile: rules, physics, assets, presentation, authority, tick...
message ProfileRef { optional string id = 1; optional uint32 revision = 2; }

enum AuthorityModel {
  AUTHORITY_MODEL_UNSPECIFIED = 0;
  AUTHORITY_MODEL_LOCAL = 1;              // single client simulates; nothing shared is authoritative
  AUTHORITY_MODEL_SERVERLESS_STAMPED = 2; // NIP-MV shared bodies; stakes-free only
  AUTHORITY_MODEL_HOSTED = 3;             // one host owns simulation (realm/chamber)
  AUTHORITY_MODEL_DETERMINISTIC = 4;      // pure function of seed and clock (towns, sea state)
}

// Errors reuse the shipped openagents.common.v1.Refusal; no duplicate
// Refusal or shared-code enum is defined in this Verse package. Domain "verse"
// preserves its own registered codes and the common error's 512-byte limit.

message GrantRef { optional string id = 1; optional uint64 epoch = 2; optional uint64 generation = 3; }
enum ArtStyle { ART_STYLE_UNSPECIFIED = 0; ART_STYLE_LOW_POLY = 1; ART_STYLE_PAINTED = 2;
  ART_STYLE_PIXEL = 3; ART_STYLE_INK = 4; }

// Appearance primitives shared by presence and character (kept here to keep imports acyclic).
message AvatarLook { optional string rig = 1; optional uint32 shade = 2; }                // bunny-v1, shades 0–20; unknown rig ignored
// LookSpec: 12 small integers, one canonical layout for every client (fixes the 7-vs-12 mismatch).
message LookSpec { optional uint32 body = 1; optional uint32 dressed = 2; optional uint32 arms = 3; optional uint32 top = 4; optional uint32 legs = 5; optional uint32 feet = 6;
  optional uint32 hood = 7; optional uint32 pauldrons = 8; optional uint32 hair = 9; optional uint32 beard = 10; optional uint32 variant = 11; optional uint32 hair_color = 12; }
message AppearanceRef { optional uint32 seed = 1; LookSpec look = 2; }          // the small form presence carries
```

Rules:

- Errors preserve their source domain and code in the shipped common
  refusal. Verse-specific codes (`stale_target`, `invalid_topology`,
  `rate_limited`, `storage_busy`, and others) use an explicit registry;
  they do not renumber or overwrite HOST/REACH/shared codes.
- `common.v1.Cell` uses the world's declared cell size, default 64 m.
- ID grammars are named constants (`ENTITY_ID`, `SLUG`, `SCENE_ID`,
  `WORLD_OPAQUE`) that each language's validator implements identically.

### 10.2 `openagents.verse.world.v1`

Purpose: what a world *is*. This covers its identity, bounds, spawn,
cells, the pinned profile tuple, portals, exits and instances. It
implements the NIP-MV scene manifest profile, which is Designed today. A
world definition is `RELEASE`-class: it is the content of a signed `33300`
plus everything it pins.

```proto
package openagents.verse.world.v1;

message WorldDefinition {                       // NIP-MV 33300 content
  optional uint32 content_version = 1;                   // 1
  common.v1.WorldId id = 2;
  optional string name = 3;                              // ≤ 64 bytes, display only; never identity
  optional float cell_m = 4;                             // default 64
  engine.v1.AxisAlignedBox bounds = 5;
  SpawnDisc spawn = 6;
  SceneBinding scene = 7;                       // absent for presence-only worlds
  engine.v1.CoordinateConvention coordinates = 8;
}
message SpawnDisc { engine.v1.Vec3 center = 1; optional float radius_m = 2; optional float keep_clear_m = 3; }

// The scene manifest profile: exact bytes of a ZoneManifest plus where to find them.
message SceneBinding { optional string profile = 1;      // "openagents.mv.scene.v1"
                       common.v1.ContentPin manifest = 2; }

// The manifest the binding pins. Every concern is explicit (NIP-MV list).
message ZoneManifest {
  optional uint32 schema = 1;                            // 1
  common.v1.WorldId world = 2;                  // must equal the signed definition's world
  ProfileTuple profiles = 3;
  repeated common.v1.ContentPin assets = 4;     // asset.v1 pack descriptors, ≤ 64
  engine.v1.SceneMetadata metadata = 5;         // landmarks, colliders
  repeated Arrival arrivals = 6;                // ≤ 16
  repeated Exit exits = 7;                      // ≤ 32
  presentation.v1.Atmosphere atmosphere = 8;
  Budgets budgets = 9;                          // decode/GPU ceilings the client enforces anyway
  repeated Notice notices = 10;                 // license and provenance notices
}

message ProfileTuple {                          // zone-rules.md: pinned into saves and replays
  common.v1.ProfileRef scene = 1;
  common.v1.ProfileRef assets = 2;
  common.v1.ProfileRef presentation = 3;
  common.v1.ProfileRef physics = 4;             // e.g. "verse.physics.default", "verse.physics.cr3bp-l1"
  common.v1.ProfileRef rules = 5;               // e.g. "verse.rules.social", "ruins.wizard-woods.v1" (retired)
  common.v1.AuthorityModel authority = 6;
  common.v1.ProfileRef tick = 7;                // sim.v1 tick profile
}

message Arrival { optional string id = 1; engine.v1.Vec3 position = 2; optional float yaw_deg = 3; optional float radius_m = 4; }
message Exit { optional string id = 1; engine.v1.AxisAlignedBox volume = 2; common.v1.WorldId to = 3;
               optional string arrival_id = 4; optional string label = 5; optional bool hidden = 6; }   // GRID_PORTAL_OPEN-style gates
message Budgets { optional uint64 max_transfer_bytes = 1; optional uint64 max_decoded_texture_bytes = 2;
                  optional uint64 max_triangles = 3; optional uint64 max_drawn_triangles = 4; optional uint64 max_gpu_bytes = 5; }
message Notice { optional string asset_id = 1; optional string license = 2; optional string attribution = 3; }

// The zone catalog a host serves (today openagents.zones.v1 at GET /v1/zones; ≤ 32 zones, ≤ 64 KiB).
message ZoneCatalog { optional uint32 schema = 1; repeated ZoneEntry zones = 2; }
message ZoneEntry {
  optional string zone = 1;                              // ZoneId slug: plaza, bare, lagrange1, physics_lab, everglade, coast...
  common.v1.WorldId world = 2;                  // verse-plaza, verse-bare, verse-lagrange-1 ...
  optional string label = 3; optional string sign = 4;            // sign: A–Z 0–9 space / .
  optional float half_extent_m = 5;
  repeated Exit portals = 6;
  SceneBinding scene = 7;                       // compiled-in zones may omit and use a host profile
  ZoneAvailability availability = 8;
}
enum ZoneAvailability { ZONE_AVAILABILITY_UNSPECIFIED = 0; ZONE_AVAILABILITY_OPEN = 1;
  ZONE_AVAILABILITY_HIDDEN = 2; ZONE_AVAILABILITY_DESKTOP_ONLY = 3; ZONE_AVAILABILITY_RETIRED = 4; }

// A running copy of a world (realm instance, hosted planet, REACH chamber).
message WorldInstance { common.v1.WorldId world = 1; optional string instance = 2; optional uint32 wire = 3;
  common.v1.ContentPin content = 4; optional uint32 population = 5; optional uint32 capacity = 6; }

// Zone transition: the client-side state machine NIP-MV requires, as data for hosts and tests.
enum TransitionPhase { TRANSITION_PHASE_UNSPECIFIED = 0; TRANSITION_PHASE_DISCOVERED = 1;
  TRANSITION_PHASE_ADMITTING = 2; TRANSITION_PHASE_LOADING = 3; TRANSITION_PHASE_READY = 4;
  TRANSITION_PHASE_ACTIVE = 5; TRANSITION_PHASE_FAILED = 6; TRANSITION_PHASE_RETURNED = 7; }
message ZoneTransition { optional uint64 generation = 1; optional string from_zone = 2; optional string to_zone = 3;
  TransitionPhase phase = 4; openagents.common.v1.Refusal refusal = 5; }
```

Rules carried from NIP-MV and `zones.md`:

- Seeing a portal or receiving a pose never starts a heavy download.
  Loading starts on explicit entry.
- A newer `33300` never silently replaces a scene in use.
- A cancelled generation's late work never replaces the current scene.
- On leave, the client publishes offline state, stops its stream and
  clears remote entities.
- Display names are not identity.

Replaces: the `33300` JSON content, `ZoneId` serialization, the
`openagents.zones.v1` catalog, and the retired `verse.zone.v1` (a new
`ZoneManifest` replaces it; the old schema stays readable only for
history). Fixes: today's local zone names are not published `33300`
addresses. `WorldId.opaque` keeps them valid while `address` becomes the
target for creator worlds.

### 10.3 `openagents.verse.doc.v1`

Purpose: one typed authored world document for every editable place,
including the Haunted House, every planet, the Sandbox, personal worlds,
hosted social zones and future creator worlds. It unifies Metareal engine's
`verse.world.v1` JSON, the frozen `haunt.v1` data, and the structural
parts of `legacy/world-v1`. `DOCUMENT` class: strict and
revision-carrying.

```proto
package openagents.verse.doc.v1;

message WorldDocument {
  optional uint32 schema = 1;                            // 1; migrations from legacy/native-save-v2 schema 0/1/2 are explicit readers
  optional string id = 2;                                // SAFE ^[A-Za-z0-9_-]{1,64}$
  optional string title = 3;                             // ≤ 80 bytes
  optional string owner = 4;                             // account id; display via character.v1
  optional uint64 revision = 5;
  optional bytes base = 6;                               // fingerprint of the built-in document this was derived from
  Arrival arrival = 7;
  presentation.v1.Look look = 8;
  repeated Area areas = 9;                      // ≤ 64
  repeated Door doors = 10;                     // ≤ 128
  repeated PlacedObject objects = 11;           // ≤ 4096 (Haunted House has 688)
  repeated NpcPlacement npcs = 12;              // ≤ 40
  repeated Voice voices = 13;                   // ≤ 16
  repeated TriggerZone zones = 14;              // ≤ 256
  repeated FoliageLayer foliage = 15;           // ≤ 32
  WorldSettings settings = 16;
  PlanetSetup planet = 17;                      // present for planets; galaxy.v1 lists them
  repeated Building buildings = 18;             // structural model (authoring.v1 validates topology)
  DocumentKind kind = 19;
}
enum DocumentKind { DOCUMENT_KIND_UNSPECIFIED = 0; DOCUMENT_KIND_HOUSE = 1; DOCUMENT_KIND_PLANET = 2;
  DOCUMENT_KIND_SANDBOX = 3; DOCUMENT_KIND_PERSONAL = 4; DOCUMENT_KIND_ZONE = 5; DOCUMENT_KIND_TOWN = 6; }

message Arrival { optional float x = 1; optional float z = 2; optional float yaw_deg = 3; optional string area = 4; optional string cue = 5; }
message Area { optional string id = 1; optional string name = 2; optional string kind = 3;          // outdoor, room, hall...
  optional sint32 x0 = 4; optional sint32 z0 = 5; optional sint32 x1 = 6; optional sint32 z1 = 7;            // 2 m grid cells
  optional float ceiling_m = 8; optional string light = 9; optional string sound = 10; optional string intent = 11; }
message Door { optional string id = 1; optional sint32 cell_i = 2; optional sint32 cell_j = 3; Side side = 4; optional string leaf = 5; }
enum Side { SIDE_UNSPECIFIED = 0; SIDE_N = 1; SIDE_S = 2; SIDE_E = 3; SIDE_W = 4; }

message PlacedObject {
  optional string id = 1; optional string name = 2;
  optional string mesh = 3;                              // catalog key; asset.v1 resolves it
  optional float x = 4; optional float y = 5; optional float z = 6; optional float yaw_deg = 7; optional float scale = 8;
  repeated float tint = 9;                      // 0 or 3
  optional float glow = 10;
  optional bool fixed = 11;
  repeated string tags = 12;                    // ≤ 8
  optional bool block = 13;
  presentation.v1.ObjectLight light = 14;
  optional bool flames = 15;
  optional string group = 16;                            // a building made of pieces shares a group
  ObjectKind kind = 17;
  optional string state = 18;                            // initial place.v1 state slug, if stateful
}
enum ObjectKind { OBJECT_KIND_UNSPECIFIED = 0; OBJECT_KIND_PROP = 1; OBJECT_KIND_CREATURE = 2;
  OBJECT_KIND_BUILDING = 3; OBJECT_KIND_ART = 4; }


message NpcPlacement { optional string id = 1; optional string npc = 2;                  // npc.v1 Villager id or inline
  optional float x = 3; optional float z = 4; optional float yaw_deg = 5; optional string area = 6; optional string clip = 7;
  npc.v1.Villager inline = 8; }                 // for single-world characters (the house's three NPCs)
message Voice { optional string id = 1; optional string name = 2; npc.v1.LineSet lines = 3; }

message TriggerZone {                           // today: an ECS bundle {Transform, Trigger}
  optional string id = 1; optional string name = 2;
  engine.v1.Vec3 position = 3;
  TriggerShape shape = 4; engine.v1.Vec3 half = 5; optional float radius_m = 6; optional float height_m = 7;
  repeated string who = 8;                      // player, npc, prop, projectile, anyone
  TriggerWhen when = 9; optional float stay_s = 10; optional float cone_deg = 11; optional string verb = 12;
  optional bool once = 13; optional float cooldown_s = 14; optional string need = 15;               // "flag" or "!flag"
  repeated TriggerAction actions = 16;          // ≤ 16
}
enum TriggerShape { TRIGGER_SHAPE_UNSPECIFIED = 0; TRIGGER_SHAPE_BOX = 1; TRIGGER_SHAPE_SPHERE = 2; TRIGGER_SHAPE_CAPSULE = 3; }
enum TriggerWhen { TRIGGER_WHEN_UNSPECIFIED = 0; TRIGGER_WHEN_ENTER = 1; TRIGGER_WHEN_EXIT = 2;
  TRIGGER_WHEN_STAY = 3; TRIGGER_WHEN_LOOK = 4; TRIGGER_WHEN_INTERACT = 5; }
message TriggerAction {
  oneof op {                                    // closed set; no "call arbitrary code"
    Show show = 1; Hide hide = 2; Morph morph = 3; Play play = 4; LightChange light = 5; Fx fx = 6;
    Spawn spawn = 7; Say say = 8; Toast toast = 9; Teleport teleport = 10; SetFlag set = 11;
    QuestStep quest = 12; NamedCall call = 13;
  }
  optional float delay_s = 20;
}
// NamedCall invokes a host-registered behavior by name; the registry is compiled in, never authored.
message NamedCall { optional string name = 1; repeated string args = 2; }
// (Show, Hide, Morph, Play, LightChange, Fx, Spawn, Say, Toast, Teleport, SetFlag, QuestStep:
//  small messages with target/name/at/value/size/duration/text exactly as docs/triggers.md.)

message FoliageLayer { optional string name = 1; optional uint32 seed = 2; optional string species = 3; optional float density = 4;
  optional float scale = 5; optional bool random_yaw = 6; optional string align = 7; optional float spacing_m = 8; optional float jitter = 9;
  repeated string areas = 10; repeated string exclude = 11; optional string avoid_mask = 12; optional float avoid_radius_m = 13;
  optional bool collide = 14; repeated float tint = 15; optional float tint_jitter = 16; optional float draw_distance_m = 17;
  optional string density_tier = 18; optional string role = 19; optional string style = 20; optional float impostor_distance_m = 21; }

message WorldSettings { ViewSettings view = 1; optional bool open = 2; }        // open: anyone may edit
message ViewSettings { CameraView view = 1; optional bool allow_toggle = 2; }
enum CameraView { CAMERA_VIEW_UNSPECIFIED = 0; CAMERA_VIEW_THIRD = 1; CAMERA_VIEW_FIRST = 2; CAMERA_VIEW_TOP = 3; }

message PlanetSetup { optional string id = 1; optional string owner = 2; PlanetEnvironment env = 3; optional string kind = 4;
  optional string name = 5; common.v1.ArtStyle art = 6; CameraView camera = 7; DayCycle cycle = 8; }
enum PlanetEnvironment { PLANET_ENVIRONMENT_UNSPECIFIED = 0; PLANET_ENVIRONMENT_VILLAGE = 1;
  PLANET_ENVIRONMENT_FOREST = 2; PLANET_ENVIRONMENT_CEMETERY = 3; }

enum DayCycle { DAY_CYCLE_UNSPECIFIED = 0; DAY_CYCLE_CYCLE = 1; DAY_CYCLE_ALWAYS_DAY = 2; DAY_CYCLE_ALWAYS_NIGHT = 3; }

// Structural buildings (legacy/world-v1): integer millimeters, stable UUIDs, one owner per edge.
message Building { optional string id = 1;                // UUID
  BuildingTransform transform = 2; repeated Storey storeys = 3; optional string prefab = 4; }
message BuildingTransform { optional sint64 x_mm = 1; optional sint64 y_mm = 2; optional sint64 z_mm = 3; optional sint32 yaw_millideg = 4; optional uint32 scale_ppm = 5; }
message Storey { optional string id = 1; optional sint64 elevation_mm = 2; repeated Room rooms = 3; repeated WallEdge walls = 4; }
message Room { optional string id = 1; repeated Point2Mm polygon = 2; optional string name = 3; }        // ≤ 64 vertices
message WallEdge { optional string id = 1; Point2Mm a = 2; Point2Mm b = 3; optional string policy = 4;
  repeated Opening openings = 5; optional string parent = 6; }                                 // split lineage
message Opening { optional string id = 1; optional string kind = 2; optional sint64 offset_mm = 3; optional sint64 width_mm = 4; optional sint64 height_mm = 5; optional sint64 sill_mm = 6; }
message Point2Mm { optional sint64 x_mm = 1; optional sint64 z_mm = 2; }

// One list item, typed, for deltas and prefab overrides.
message WorldDocumentItem { oneof item { PlacedObject object = 1; TriggerZone zone = 2; NpcPlacement npc = 3;
  FoliageLayer foliage = 4; Area area = 5; Door door = 6; WorldSettings settings = 7; Building building = 8; } }
message FieldValue { oneof value { bool flag = 1; sint64 integer = 2; double number = 3; string text = 4;
  engine.v1.Vec3 vec3 = 5; WorldDocumentItem item = 6; } }      // typed override values; no free JSON
```

Rules:

- Limits follow the current house (688 objects, 16 areas, 21 doors, 3
  NPCs) with headroom, and `world-model.md`'s structural guards: 128 rooms
  per building, 2,048 wall edges, 64 polygon vertices, 16 openings per
  edge.
- `TriggerAction.call` is the only extension point, and it names a
  compiled-in host behavior. It never carries code.
- Structural IDs are allocated by the authoring validator. Rebuilding
  derived outputs never allocates authoritative IDs.
- The `legacy/native-save-v2` wrapper (`{format, schema, savedAt, data:{doc, base}}`)
  becomes the `publish.v1.PublishedWorld` and AESV envelopes. The
  document is the payload.

Replaces: `verse.world.v1` JSON, the `haunt.v1` document shape, prop
worlds `{props, zones}`, `verse.<world>.props`/`.zones`/`.look`, and the
structural part of `legacy/world-v1`. The ECS `AuthoredSource`
JSON carrying triggers in `state.proto` goes away once `doc.v1` is the
document.

### 10.4 `openagents.verse.authoring.v1`

Purpose: the only way an authored document changes. Typed operations in a
command envelope, with revision fences, preconditions, idempotency and
receipts. This is the proto form of `legacy/command-v1` and the
Metareal engine house delta. It also covers the Metareal tools host `propose`,
`undo` and `redo` ops and the in-world edit mode.

```proto
package openagents.verse.authoring.v1;

message Command {                               // INTENT; ≤ 64 KiB encoded (instant profile)
  optional uint32 schema = 1;
  optional string command_id = 2;                        // UUID; idempotency key scoped (project, branch, principal)
  optional string project_id = 3; optional string branch_id = 4;
  optional string actor_id = 5;                          // set by the server from the session, never trusted from body
  optional string grant_ref = 6;                         // capability reference; checked independently
  optional uint64 base_revision = 7;
  repeated Operation operations = 8;            // 1–8
  repeated Precondition preconditions = 9;      // ≤ 32
  CommandContext context = 10;                  // origin (editor, voice, agent, model proposal), summary
}
message Operation {
  oneof op {                                    // closed; unknown op fails closed
    PlaceObject place_object = 1; SetTransform set_transform = 2; RemoveEntity remove = 3;
    SetObjectFields set_fields = 4; PlaceBuilding place_building = 5; AddRoom add_room = 6;
    SetWallPolicy set_wall_policy = 7; AddOpening add_opening = 8; SetLook set_look = 9;
    UpsertTriggerZone upsert_trigger = 10; UpsertNpc upsert_npc = 11; UpsertFoliage upsert_foliage = 12;
    SetArea set_area = 13; SetDoor set_door = 14; SetSettings set_settings = 15; ApplyPrefab apply_prefab = 16;
    SetFaceMaterial set_face_material = 17;     // host v6
  }
  repeated string target_ids = 30;              // explicit; ≤ 32 distinct per command incl. new IDs
}
message Precondition { optional string target_id = 1; optional uint64 entity_revision = 2; optional bool must_exist = 3; }
message CommandContext { CommandOrigin origin = 1; optional string summary = 2;    // ≤ 300 bytes
  optional string proposal_id = 3; }                     // model proposals are inert until a person or policy admits
enum CommandOrigin { COMMAND_ORIGIN_UNSPECIFIED = 0; COMMAND_ORIGIN_EDITOR = 1; COMMAND_ORIGIN_VOICE = 2;
  COMMAND_ORIGIN_AGENT = 3; COMMAND_ORIGIN_MODEL_PROPOSAL = 4; COMMAND_ORIGIN_IMPORT = 5; COMMAND_ORIGIN_UNDO = 6; }

message CommandReceipt {                        // RECEIPT
  optional string command_id = 1;
  CommandStatus status = 2;
  optional uint64 revision = 3; optional bytes world_digest = 4;
  repeated string changed_ids = 5;
  openagents.common.v1.Refusal refusal = 6;
  optional string build_identity = 7;
}
enum CommandStatus { COMMAND_STATUS_UNSPECIFIED = 0; COMMAND_STATUS_COMMITTED = 1;
  COMMAND_STATUS_REJECTED = 2; COMMAND_STATUS_UNSUPPORTED = 3; COMMAND_STATUS_CANCELLED = 4; COMMAND_STATUS_UNKNOWN = 5; }

message Revision { optional string project_id = 1; optional string branch_id = 2; optional uint64 revision = 3; optional uint64 parent = 4;
  optional bytes world_digest = 5; optional string command_id = 6; optional string actor_id = 7; optional string validator_version = 8;
  MergeSource merge = 9; repeated SourceFrontier frontiers = 10; }
message MergeSource { optional string branch_id = 1; optional uint64 revision = 2; }
message SourceFrontier { optional string doc = 1; optional bytes loro_frontier = 2; optional bytes materialized_sha256 = 3; }  // COL-004

// The house delta: id-keyed changes; preserve the current JSON/JCS identity
// through an explicit adapter. No generic canonical-proto hash is introduced.
message DocumentDelta { optional uint64 base_revision = 1; optional uint64 revision = 2;
  repeated ListChange added = 3; repeated ListChange changed = 4; repeated ListRemoval removed = 5; }
message ListChange { DocList list = 1; doc.v1.WorldDocumentItem item = 2; }
message ListRemoval { DocList list = 1; optional string id = 2; }
enum DocList { DOC_LIST_UNSPECIFIED = 0; DOC_LIST_OBJECTS = 1; DOC_LIST_ZONES = 2; DOC_LIST_NPCS = 3;
  DOC_LIST_FOLIAGE = 4; DOC_LIST_AREAS = 5; DOC_LIST_DOORS = 6; DOC_LIST_SETTINGS = 7; }

// Editor presence (COL-003): ephemeral, rate-limited, never authority.
message EditorPresence { optional string project_id = 1; optional string branch_id = 2; optional string actor_id = 3;
  repeated string selection = 4; common.v1.Pose camera = 5; repeated Reservation reservations = 6; }
message Reservation { optional string target_id = 1; optional int64 expires_ms = 2; }    // advisory only

message Prefab { optional string id = 1; optional uint32 version = 2; doc.v1.WorldDocument body = 3; }      // immutable
message PrefabInstance { optional string prefab = 1; optional uint32 version = 2; repeated Override overrides = 3; }
message Override { optional string path = 1; doc.v1.FieldValue value = 2; }      // stable semantic path
```

Rules, carried from `contracts.md` and `collaboration.md`:

- Commit has seven steps: strict decode and authenticate, then read
  permission and idempotency lookup, then the independent grant check,
  then revision and preconditions, then stage and whole-document
  validation, then an atomic commit with the outbox, then the receipt.
- Undo is a compensating command with a new ID. It never decrements the
  revision.
- Retention: full results for 30 days. Tombstones live for the
  project's lifetime.
- Offline work becomes a branch, merged three-way on reconnect. Queued
  commands are never replayed blindly.
- Loro covers only source text, comments and designated metadata.
  Structural records stay out of CRDT merge.
- Model output is a `MODEL_PROPOSAL` and carries no authority (INV-01).
- `doc.v1.WorldDocumentItem` is a oneof over the list item types, so
  deltas stay typed.

### 10.5 `openagents.verse.publish.v1`

Purpose: how a document becomes something others can visit, and how
others propose changes to it. This covers release identity, channels,
publication state, the Metareal engine publish/offer/edit-log flow, world
settings and ownership.

```proto
package openagents.verse.publish.v1;

// Mirror of legacy/release-v1. The authoritative signed form is the JCS JSON; this message
// is decoded from it and re-checked against release_jcs's digest. Never hash this message.
message Release {
  optional bytes release_id = 1;                         // sha256(JCS body)
  optional bytes release_jcs = 2;                        // the exact signed body bytes
  optional string project_id = 3; optional string branch_id = 4; optional uint64 revision = 5; optional bytes world_digest = 6;
  EngineIdentity engine = 7; optional string target_profile = 8; optional bytes rights_digest = 9;
  repeated ReleaseArtifact artifacts = 10;      // 1–100,000
  repeated string entry_points = 11;            // 1–32
  optional bytes compiler_digest = 12; optional string world_schema = 13; optional string simulation_protocol = 14;   // af.simulation.1
  repeated string capabilities = 15;            // requirements, never grants; ≤ 64
  repeated bytes startup_digests = 16;
  OfflinePolicy offline_policy = 17;
}
message EngineIdentity { optional string abi = 1; optional bytes runtime_digest = 2; optional string vm_version = 3; optional bytes vm_digest = 4; }
message ReleaseArtifact { optional bytes digest = 1; optional uint64 byte_length = 2; optional string media_type = 3; optional string role = 4; }
enum OfflinePolicy { OFFLINE_POLICY_UNSPECIFIED = 0; OFFLINE_POLICY_CACHED_TRUST = 1; OFFLINE_POLICY_FRESH_AUTHORIZATION = 2; }

message ChannelRef {                            // mirror of legacy/channel-v1 + detached signature
  optional bytes channel_jcs = 1; optional bytes signature = 2;   // Ed25519 over original domain bytes (registry key signature-domain-v1), never the key text
  optional string project_id = 3; optional string channel = 4; optional uint64 sequence = 5; optional bytes previous_ref_digest = 6;
  optional bytes release_id = 7; optional string key_id = 8; optional int64 issued_ms = 9; optional int64 expires_ms = 10; optional uint64 trust_version = 11; }

enum PublicationState { PUBLICATION_STATE_UNSPECIFIED = 0; PUBLICATION_STATE_BUILDING = 1;
  PUBLICATION_STATE_UPLOADING = 2; PUBLICATION_STATE_READY = 3; PUBLICATION_STATE_PUBLISHED = 4; PUBLICATION_STATE_FAILED = 5; }

// The lightweight per-world publish used by planets and the Haunted House today.
message PublishWorldRequest { optional string world = 1; doc.v1.WorldDocument doc = 2; optional string summary = 3;
  optional uint64 expected_version = 4; optional string request_id = 5; }
message PublishedWorld { doc.v1.WorldDocument doc = 1; optional uint64 version = 2; optional string published_by = 3;
  optional int64 published_ms = 4; }                     // version 0 + no doc = nothing published

message ChangeOffer {                           // someone else's proposed change, priced or free
  optional string id = 1;                                // 12 hex
  optional string world = 2; optional string summary = 3;
  optional string price_msat = 4;                        // decimal string; 0–1,000,000 sats today
  authoring.v1.DocumentDelta delta = 5;
  OfferState state = 6;
  Actor by = 7; optional bool guest = 8; optional int64 at_ms = 9; optional int64 decided_ms = 10; optional string decided_by = 11;
  optional string settlement_ref = 12;                   // economy.v1 / payments owner id once paid
}
enum OfferState { OFFER_STATE_UNSPECIFIED = 0; OFFER_STATE_WAITING = 1; OFFER_STATE_ACCEPTED = 2; OFFER_STATE_DECLINED = 3; }
message Actor { optional string account = 1; optional string player = 2; }

message EditLogEntry { optional string id = 1; optional uint64 version = 2; optional string who = 3; optional string summary = 4; optional int64 at_ms = 5;
  optional string amount_msat = 6; EditLogState state = 7; authoring.v1.DocumentDelta delta = 8; Actor by = 9;
  optional bool guest = 10; optional int64 decided_ms = 11; optional string decided_by = 12; optional bool paid = 13; }
enum EditLogState { EDIT_LOG_STATE_UNSPECIFIED = 0; EDIT_LOG_STATE_PUBLISHED = 1; EDIT_LOG_STATE_RESET = 2;
  EDIT_LOG_STATE_WAITING = 3; EDIT_LOG_STATE_ACCEPTED = 4; EDIT_LOG_STATE_DECLINED = 5; }
message EditLog { repeated EditLogEntry entries = 1; }                  // ≤ 200, newest first

message WorldOwnership { optional bool claimed = 1; optional bool you = 2; optional bool site = 3; optional bool open = 4; optional string name = 5; }
```

Rules:

- **Only the channel CAS publishes.** In the full Metareal tools flow,
  *Published* means the channel compare-and-swap succeeded (PUB-003).
  Uploading or signing is not enough.
- **Channel references.** Clients persist `(sequence, ref digest, trust version)`
  and refuse older ones. A different digest at the same sequence is a
  fork. Rollback is a higher sequence pointing at an older release.
- **Sessions pin a release** (INV-07).
- **Lightweight publish** (planets, the house). The owner check is the
  server's. A non-owner receives `NOT_ADMITTED` together with an offer
  hint. They never receive a silent failure.
- **Money fields** are msat decimal strings and are views of the payments
  owner's records. An offer's `price_msat` is a request, and payment state
  comes only from `settlement_ref`.

### 10.6 `openagents.verse.presence.v1`

Purpose: the typed body of NIP-MV, and the only presence vocabulary.
`PRESENCE` class: self-asserted, never authoritative, ignore-unknown.

```proto
package openagents.verse.presence.v1;

message Envelope {                              // what the mapper turns into tags
  common.v1.WorldId world = 1;                  // w
  repeated common.v1.Cell cells = 2;            // c, one per occupied cell
  optional string district = 3;                          // z (chat districts and zone commands)
  optional bytes addressed_to = 4;                       // p
  optional string d = 5;                                 // addressable key, e.g. "<world>/<entity id>"
  optional string role = 6;                              // role tag
  repeated bytes owner_auth = 7;                // NIP-OA auth tag bytes (avatar state only)
}

message EntityState {                           // kind 33301
  optional uint32 content_version = 1;                   // 1
  optional string id = 2; optional string role = 3;               // avatar, agent, object, body, bodies; unknown accepted
  engine.v1.Vec3 position = 4; common.v1.Quat orientation = 5;
  optional int64 time_ms = 6; optional bool online = 7;
  optional string follows = 8;                           // entity id
  optional string name = 9;                              // ≤ 24 chars display; not identity
  common.v1.AvatarLook look = 10;
  optional string status = 11;                           // ≤ 64 bytes, no controls
  ObjectState object = 12;                      // role=object
  BodySnapshot bodies = 13;                     // role=bodies
  common.v1.AppearanceRef appearance = 14;   // richer appearance (metareal seed/look), optional
}
message PoseFrame {                             // kind 23300; ≤ 4 KiB
  optional uint32 content_version = 1;
  optional string session = 2;                           // 1–16 bytes
  optional uint64 sequence = 3; optional int64 time_ms = 4;
  repeated FrameEntry entries = 5;              // 1–16
}
message FrameEntry { optional string id = 1; optional string role = 2; engine.v1.Vec3 position = 3; common.v1.Quat orientation = 4;
  engine.v1.Vec3 velocity = 5; optional string follows = 6; optional string animation = 7;
  common.v1.Stamp stamp = 8;                    // required for role=body, forbidden otherwise
  engine.v1.Vec3 angular_velocity = 9; optional bool at_rest = 10; }

message BodySnapshot { optional string body_set = 1;     // verse-bare.bodies.v1, verse-coast.bodies.v1; ≤ 64 bytes
  repeated BodyRest bodies = 2; }               // 0–64; unmoved omitted
message BodyRest { optional string id = 1; engine.v1.Vec3 position = 2; common.v1.Quat orientation = 3;
  common.v1.Stamp stamp = 4; optional bytes owner = 5; }

message Gesture {                               // kind 23301
  optional uint32 content_version = 1; optional string id = 2;
  optional string gesture = 3;                           // ^[a-z0-9-]{1,32}$: look-around, greet, zone-ok, zone-refused, ...
  optional int64 time_ms = 4; optional float duration_s = 5;
  repeated engine.v1.Vec3 at = 6; common.v1.EntityRef to = 7;
  ZoneCommandResult result = 8;                 // for zone-ok / zone-refused
}

message ZoneCommand {                           // kind 23302
  optional uint32 content_version = 1;
  optional string zone = 2; optional string command = 3;          // ^[a-z0-9-]{1,32}$
  repeated CommandArg args = 4;                 // ≤ 16
  optional int64 time_ms = 5; optional string id = 6;             // 1–16 bytes
}
message CommandArg { oneof value { double number = 1; string text = 2; } } // text ≤ 128 bytes
message ZoneCommandResult { optional string command_id = 1; optional bool ok = 2; optional string reason = 3;
  repeated CommandArg values = 4; }             // e.g. Lagrange 1 status snapshot as named values (≤ 16)

message ObjectState {                           // world-tree projection
  optional string node = 1; optional bytes tree = 2;              // node id; world-tree digest
  oneof state {
    LampState lamp = 10; DoorState door = 11; WorkstationState workstation = 12; TaskWallState task_wall = 13;
    PylonState pylon = 14; WellspringState wellspring = 15;
  }
}
message LampState { optional bool lit = 1; }
message DoorState { optional bool open = 1; }
message WorkstationState { optional bool busy = 1; common.v1.EntityRef by = 2; }
message TaskWallState { repeated TaskColumn columns = 1; }
message TaskColumn { optional string name = 1; optional uint32 count = 2; }
message PylonState { optional string pool = 1; optional uint32 online = 2; }             // projection of NIP-PYLON; refs only
message WellspringState { optional uint32 level = 1; }
```

Rules (NIP-MV, normative):

- **Frames.** Order frames by `(session, sequence)`. Render 100–200 ms in
  the past. Treat a publisher as offline after about 10 s without frames.
- **Cadence.** About 10 Hz while moving. Durable state is published on
  join, on rest, every few seconds while moving, and on leave. Mobile
  budget is 54 events per minute.
- **Shared bodies.**
  - Order stamps by epoch, then rev, then owner.
  - The last toucher claims with `rev+1`. No claim-back within 0.3 s.
  - Orphans can be claimed after 6 s. Reports above 60 m/s or 60 rad/s
    are refused.
  - A reset raises the epoch, at most once every 5 s.
- **Statuses** longer than 64 bytes are refused, not truncated.
- **Object state.** The entity ID is `obj-` plus the first 20 hex digits
  of SHA-256(node).

New vocabulary is registered, not invented ad hoc. Gesture names, role
names, rig names, animation names and object-state kinds live in
`presence/v1/REGISTRY.md`. Each entry has an owner, a status and the
readers that support it. Unknown values are always ignored.

### 10.7 `openagents.verse.social.v1`

Purpose: the typed bodies and views for world chat, rooms, private
messages, mutes, buddies, parties, guilds and hosted visits. The Nostr
records stay NIP-C7, NIP-29 and NIP-17. This package defines their parsed
form and the client and host state around them.

```proto
package openagents.verse.social.v1;

enum Scope { SCOPE_UNSPECIFIED = 0; SCOPE_ALL = 1; SCOPE_ADS = 2; SCOPE_ZONE = 3;
  SCOPE_NEAR = 4; SCOPE_HERE = 5; SCOPE_ROOM = 6; SCOPE_PRIVATE = 7; SCOPE_AGENT = 8; SCOPE_PARTY = 9; SCOPE_GUILD = 10; }

message ChatLine {                              // kind 9 body + tags, parsed
  optional bytes event_id = 1; optional bytes author = 2; optional int64 created_s = 3;
  Scope scope = 4;
  common.v1.WorldId world = 5;                  // absent for rooms
  optional string district = 6;                          // z, for SCOPE_ZONE
  engine.v1.Vec3 position = 7;                  // pos, for NEAR and HERE
  optional string room = 8;                              // NIP-29 h
  optional string text = 9;                              // PERSONAL; ALL/ADS ≤ 150 chars, others ≤ 500
  optional bytes reply_to = 10;                          // q
  GymNoteRef gym_note = 11;                     // NIP-MV Gym notes
}
message GymNoteRef { optional bytes result_event = 1; optional bytes answers = 2; }      // cites one 3189; answers one opener

message ScopeRule { Scope scope = 1; optional float radius_m = 2; optional uint32 max_chars = 3; optional float min_interval_s = 4;
  optional uint32 budget = 5; optional float refill_s = 6; }      // ALL: budget 15 refill 20 s; ADS 1/min; NEAR 40 m; HERE 3 m

message MuteSet { repeated MuteTarget targets = 1; repeated bytes pubkeys = 2; }
enum MuteTarget { MUTE_TARGET_UNSPECIFIED = 0; MUTE_TARGET_ALL = 1; MUTE_TARGET_ADS = 2; MUTE_TARGET_ZONE = 3;
  MUTE_TARGET_NEAR = 4; MUTE_TARGET_HERE = 5; MUTE_TARGET_ROOMS = 6; MUTE_TARGET_PM = 7; MUTE_TARGET_LOGINS = 8; MUTE_TARGET_GESTURES = 9; }

message Room { optional string id = 1; optional string name = 2; optional string about = 3; optional bool closed = 4; repeated bytes members = 5; }
message Buddy { optional bytes pubkey = 1; optional bool mutual = 2; optional bool online = 3; common.v1.WorldId world = 4; }   // NIP-02 mutual follows

message Party { optional string id = 1; optional bytes leader = 2; repeated bytes members = 3; optional uint32 capacity = 4; }  // ≤ 64 (realm)
message Guild { optional string id = 1; optional string name = 2; optional string room = 3;       // backed by a NIP-29 closed group
  repeated bytes members = 4; optional string ledger_ref = 5; }                 // arena guild ledger / future purse

// Hosted visits (Loro document chat today): invitations and their limits.
message VisitInvitation { optional string id = 1; optional string world = 2; optional int64 expires_ms = 3;    // 1 h, 1 d, 7 d
  optional uint32 max_participants = 4; }                // ≤ 64
message VisitMessage { optional string invitation = 1; optional string from = 2; optional string text = 3; optional int64 at_ms = 4; }  // ≤ 1000 B

message ModerationAction { optional string room = 1; optional uint32 kind = 2; optional bytes target = 3; optional string reason = 4; }   // NIP-29 9000–9010
```

Rules:

- **Scope is a display filter, not privacy.** Only `SCOPE_PRIVATE`
  (NIP-17 gift wraps) is private. `SCOPE_AGENT` never leaves the device or
  the user's own agent door.
- **Existing limits stand:** 5+-letter all-caps refusal, 7 s bubbles,
  name tags within 60 m, at most 24 shown.

Replaces: the hand-built tag logic in `verse-net::chat`,
`web/grid-session.js`, Metareal engine `/api/chat/<room>` (≤500 lines kept,
name ≤40, text ≤500), and the visits qualification's message shapes.

### 10.8 `openagents.verse.realm.v1`

Purpose: the authoritative session wire between a Verse client and a world
host. It covers the chamber over TLS and REACH today and Metareal
multiplayer next. This is the largest proto-native win: wire version 34 is
length-prefixed JSON today, and proto makes it compact, typed and the same
in every client.

```proto
package openagents.verse.realm.v1;

// Framing: each frame on the stream is varint length + ClientFrame/ServerFrame bytes. ≤ 64 KiB.
message ClientFrame { optional uint64 seq = 1; oneof body {
  Hello hello = 10; LoginProof login = 11; SelectCharacter select = 12; Command command = 13;
  Ack ack = 14; TransferAccept transfer_accept = 15; Ping ping = 16; ReadRequest read = 17; Bye bye = 18; } }

message ServerFrame { optional uint64 seq = 1; optional uint64 tick = 2; oneof body {
  Welcome welcome = 10; LoginChallenge challenge = 11; CharacterList characters = 12; Joined joined = 13;
  CommandReceipt receipt = 14; MovementConfirm movement = 15; SpatialDelta delta = 16; Snapshot snapshot = 17;
  TransferOffer transfer = 18; Pong pong = 19; ReadReply read = 20; Kick kick = 21; HudUpdate hud = 22; } }

message Hello { optional uint32 wire = 1;                // today 34; becomes the proto wire's own version
  optional string client = 2; common.v1.WorldId world = 3; optional string instance = 4;
  optional bytes content_digest = 5;                     // scene/asset identity the client loaded (wire v8)
  common.v1.GrantRef grant = 6; }                 // NIP-HOST world right: id, epoch, generation
message Welcome { optional uint32 wire = 1; optional string rules_revision = 2;          // verse-chamber-owned-v24
  world.v1.ProfileTuple profiles = 3; optional uint32 tick_hz = 4; optional uint32 substeps = 5; optional float replication_radius_m = 6; }
message LoginChallenge { optional bytes nonce = 1; optional bytes content_digest = 2; optional int64 expires_ms = 3; }
message LoginProof { optional bytes pubkey = 1; optional bytes signature = 2; optional string role = 3; }   // content-bound Schnorr
message CharacterList { repeated character.v1.CharacterSummary characters = 1; }
message SelectCharacter { optional string character_id = 1; }
message Joined { optional string character_id = 1; optional uint32 actor = 2; optional uint64 life = 3; Snapshot snapshot = 4; }

// Intent and aim only. Never position, damage, cost or reward (architecture.md).
message Command {
  optional uint64 control_epoch = 1; optional uint64 life = 2; optional uint32 actor = 3; optional uint64 client_tick = 4; optional string request_id = 5;
  oneof intent {
    Move move = 10; Jump jump = 11; Interact interact = 12; UseAbility ability = 13; CancelCast cancel = 14;
    Target target = 15; InventoryAction inventory = 16; EquipAction equip = 17; QuestAction quest = 18;
    PartyAction party = 19; TradeAction trade = 20; Emote emote = 21; Respawn respawn = 22; StudioPose studio = 23;
  }
}
message Move { optional float x = 1; optional float z = 2; optional float yaw_deg = 3; optional bool run = 4; }              // stick, not position
message UseAbility { optional string ability = 1; optional uint32 target_actor = 2; engine.v1.Vec3 aim = 3; }
message QuestAction { optional string quest = 1; QuestVerb verb = 2; }           // accept / claim (wire v15/16)
enum QuestVerb { QUEST_VERB_UNSPECIFIED = 0; QUEST_VERB_ACCEPT = 1; QUEST_VERB_CLAIM = 2; QUEST_VERB_ABANDON = 3; }
// (Jump, Interact, CancelCast, Target, InventoryAction, EquipAction, PartyAction, TradeAction, Emote,
//  Respawn, StudioPose: small closed messages matching verse-world::service commands.)

message CommandReceipt { optional string request_id = 1; optional bool accepted = 2; openagents.common.v1.Refusal refusal = 3;
  optional uint64 tick = 4; optional uint32 retry_after_ms = 5; }  // rate_limited / storage_busy: back off ≥100 ms, stop at 10 s
message MovementConfirm { optional uint64 client_tick = 1; common.v1.GlobalVec3 position = 2; optional float yaw_deg = 3;
  optional bool corrected = 4; }

// Acknowledged spatial replication within the replication radius (64 m today).
message SpatialDelta { optional uint64 base = 1; optional uint64 revision = 2;
  repeated ActorAppear appear = 3; repeated uint32 vanish = 4; repeated ActorUpdate update = 5;
  repeated rules.v1.CombatEvent events = 6; repeated PropPose props = 7; repeated Blocker blockers = 8;
  repeated environment.v1.DestructionEvent destruction = 9; }
message ActorAppear { optional uint32 actor = 1; optional uint64 life = 2; optional string kind = 3; character.v1.Appearance appearance = 4;
  optional string name = 5; common.v1.GlobalVec3 position = 6; optional float yaw_deg = 7; }
message ActorUpdate { optional uint32 actor = 1; common.v1.GlobalVec3 position = 2; optional float yaw_deg = 3; optional string animation = 4;
  rules.v1.Vitals vitals = 5; repeated rules.v1.StatusEffect statuses = 6; rules.v1.CastBar cast = 7; }
message PropPose { optional string prop = 1; common.v1.Pose pose = 2; }          // wire v6
message Blocker { optional string id = 1; engine.v1.AxisAlignedBox box = 2; optional bool on = 3; }   // wire v7
message Snapshot { optional uint64 revision = 1; repeated ActorAppear actors = 2; repeated PropPose props = 3;
  repeated Blocker blockers = 4; environment.v1.DestructionBaseline destruction = 5; }
message Ack { optional uint64 revision = 1; }

// Atomic transfer between instances (portal.world.v1 prepare/commit).
message TransferOffer { optional string transfer_id = 1; world.v1.WorldInstance to = 2; optional string arrival = 3;
  optional bytes ticket = 4; optional int64 expires_ms = 5; }
message TransferAccept { optional string transfer_id = 1; }

message HudUpdate { engine.v1.HudFrame frame = 1; }                    // wire v5 player HUD
message ReadRequest { oneof what { bool inventory = 1; bool progress = 2; bool quests = 3; } }   // wire v10+
message ReadReply { character.v1.Inventory inventory = 1; character.v1.Progress progress = 2;
  repeated quest.v1.QuestView quests = 3; }
message Kick { openagents.common.v1.Refusal reason = 1; }
message Ping { optional int64 t_ms = 1; } message Pong { optional int64 t_ms = 1; } message Bye {}
```

Rules:

- **Admission.** Every request rechecks the NIP-HOST `world` right, grant
  epoch and generation, and a timer rechecks them too. A walking avatar
  gains no Studio panel rights.
- **Fencing.** Commands are fenced by actor ownership, life generation,
  control epoch, sequence and tick. Clients never replay a command after
  `rate_limited`.
- **Tick and scale.** Today: 30 Hz with four 120 Hz physics substeps, a
  64 m replication radius and up to 64 cooperative players per simulation.
  Metareal multiplayer declares 60 Hz through the tick profile. Neither
  value is hard-coded in the schema.
- **One wire version.** The proposed realm proto wire starts at `wire = 35` and carries
  every feature that versions 2 to 34 added (presentation, telegraphs,
  HUD, props, blockers, content identity, inventory, names, rewards,
  gear, quests). Hosts keep the JSON v34 reader until clients are
  migrated (§16, V4).

### 10.9 `openagents.verse.sim.v1`

Purpose: what makes a simulation reproducible and checkable. This covers
tick profiles, physics profiles, the determinism scope, replays and
checkpoint indexes.

```proto
package openagents.verse.sim.v1;

message TickProfile { optional string id = 1; optional uint32 revision = 2; optional uint32 tick_hz = 3; optional uint32 physics_substeps = 4;
  optional uint32 max_steps_per_frame = 5; }             // physics crate: 1/120 s, ≤ 12 per frame (48 in the Lab)

message PhysicsProfile { optional string id = 1; optional uint32 revision = 2; optional float gravity_mps2 = 3; optional string integrator = 4;
  optional string collision = 5; optional float max_linear_mps = 6; optional float max_angular_radps = 7; optional string frame = 8; // "local", "cr3bp-sun-earth-l1"
  repeated string features = 9; }               // declared coverage; selecting a profile implies nothing else

// Exact-profile determinism: the guarantee holds only for this tuple.
message ExecutionProfile { optional string executable_digest = 1; optional string target = 2; optional string rules_revision = 3;
  optional bytes content_digest = 4; world.v1.ProfileTuple profiles = 5; }

message InputReplay {                           // verse.input-replay.v1
  optional uint32 schema = 1; ExecutionProfile execution = 2; optional uint64 start_tick = 3; optional uint64 rng_seed = 4;
  repeated TimedInput inputs = 5; optional bytes result_digest = 6; }
message TimedInput { optional uint64 tick = 1; optional uint32 order = 2; optional uint32 actor = 3;
  optional bytes command = 4; }                          // an encoded realm.v1.Command, hashed as stored (keeps sim below realm)

message GameReplay {                            // verse.game-replay.v1: game-neutral
  optional uint32 schema = 1; optional string game = 2; optional bytes rules_digest = 3; optional bytes level_digest = 4; optional uint64 seed = 5;
  optional bytes inputs = 6;                             // game-defined packed inputs, hashed as stored
  optional bytes result_digest = 7; ExecutionProfile execution = 8; }

message CheckpointIndex { optional string world = 1; optional uint64 tick = 2; optional string schema = 3;   // verse-chamber-owned-v24
  optional bytes digest = 4; optional uint64 byte_length = 5; }   // checkpoint bodies stay in their owned format

message KernelIdentity { optional string abi = 1;        // af.kernel.1
  optional uint32 abi_version = 2;                       // 0x00010000
  optional string behavior_vm = 3; }                     // af.behavior.1
```

Rules:

- A replay's guarantee holds only for its exact `ExecutionProfile`, never
  across builds or architectures, and cross-machine float determinism is
  not assumed.
- Checkpoint bodies and kernel slabs stay in their owned binary formats
  (§8.1). `sim.v1` indexes them.

### 10.10 `openagents.verse.rules.v1`

Purpose: game rules as versioned, pinned data. This covers real-time MMO
combat with SRD dice resolved by the authority, abilities and spells,
effects and conditions, encounter phases, and what clients are shown
(telegraphs, cast bars, rolls revealed after resolution).

```proto
package openagents.verse.rules.v1;

message RulesProfile { optional string id = 1; optional uint32 revision = 2; repeated string features = 3;
  optional float gcd_s = 4; optional float round_s = 5; optional float feet_to_m = 6;    // 1.0 s, 6 s, 0.3 (5 ft = 1.5 m)
  DiminishingReturns dr = 7; repeated AbilityDef abilities = 8; repeated ConditionDef conditions = 9; }
message DiminishingReturns { optional float window_s = 1; repeated float factors = 2; }   // 15 s; 1, ½, immune

message AbilityDef { optional string id = 1; optional string name = 2; optional uint32 tier = 3; optional uint32 mana = 4; optional float cooldown_s = 5;
  optional float cast_s = 6; optional float range_m = 7; Targeting targeting = 8; repeated EffectDef effects = 9;
  optional bool bonus_action = 10; optional uint32 charges = 11; optional bool concentration = 12; optional string dice = 13; }   // "2d6+3"
enum Targeting { TARGETING_UNSPECIFIED = 0; TARGETING_SELF = 1; TARGETING_ACTOR = 2; TARGETING_POINT = 3;
  TARGETING_CONE = 4; TARGETING_LINE = 5; TARGETING_AREA = 6; }
message EffectDef { optional string kind = 1; optional string damage_type = 2; optional string dice = 3; optional string condition = 4; optional float duration_s = 5; }
message ConditionDef { optional string id = 1; optional string name = 2; repeated string modifiers = 3; }

message Vitals { optional uint32 hp = 1; optional uint32 hp_max = 2; optional uint32 mana = 3; optional uint32 mana_max = 4; optional uint32 shield = 5; }
message StatusEffect { optional string condition = 1; optional float remaining_s = 2; optional uint32 stacks = 3; optional uint32 source_actor = 4; }
message CastBar { optional string ability = 1; optional float elapsed_s = 2; optional float total_s = 3; optional bool interruptible = 4; }

message CombatEvent { optional uint64 tick = 1; oneof event {
  Telegraph telegraph = 10; Projectile projectile = 11; Hit hit = 12; Miss miss = 13; Death death = 14;
  ConditionApplied applied = 15; ConditionRemoved removed = 16; PhaseChanged phase = 17; Reward reward = 18; } }
message Hit { optional uint32 source = 1; optional uint32 target = 2; optional string ability = 3; optional uint32 amount = 4; optional string damage_type = 5;
  DiceReveal roll = 6; optional float multiplier = 7; }  // resistance 0.5 / vulnerability 2 / immunity 0
message DiceReveal { optional string expression = 1; repeated uint32 faces = 2; optional int32 total = 3; }   // revealed after resolution
message Reward { optional uint32 actor = 1; optional string item = 2; optional uint32 count = 3; optional uint64 xp = 4; }      // game XP, not NIP-XP
// (Telegraph, Projectile, Miss, Death, ConditionApplied/Removed, PhaseChanged: small closed messages.)
```

Rules:

- Clients never send rolls or results. Dice resolve on the authority and
  are revealed afterwards.
- `Reward.xp` is game progression inside a realm and has no relation to
  NIP-XP trainer XP. The two are never summed.
- A rules profile declares its feature coverage. Selecting
  `fifth-edition-encounter` implies nothing beyond the features it lists.

### 10.11 `openagents.verse.character.v1`

Purpose: who you are in the Verse. This covers account and player views,
characters (realm), appearance across all clients, outfits and equipment,
inventory, progression and personal settings.

```proto
package openagents.verse.character.v1;

message PlayerView { optional string id = 1;              // "p-…"; never the key
  optional string account = 2; repeated string worlds = 3; }
message AccountView { optional string id = 1; optional string name = 2; optional string via = 3; optional bool guest = 4; }  // emails never here

message CharacterSummary { optional string id = 1; optional string name = 2; optional string role = 3; optional uint32 level = 4; Appearance appearance = 5; }

message Appearance {                            // uses common.v1.LookSpec / AvatarLook
  optional uint32 seed = 1;
  common.v1.LookSpec look = 2;                  // metareal figure
  common.v1.AvatarLook rig = 3;                 // NIP-MV rig/shade
  repeated EquipmentSlot equipment = 4;
  repeated float tint = 5;
  optional string private_asset = 6;                     // owner-local private character name; never public
}
message EquipmentSlot { optional string slot = 1; optional string item = 2; }
message Item { optional string id = 1; optional string kind = 2; optional string name = 3; optional uint32 count = 4; repeated string tags = 5; }
message Inventory { repeated Item items = 1; optional uint32 capacity = 2; }
message Progress { optional uint32 level = 1; optional uint64 xp = 2; repeated string unlocked = 3; }

message PlayerSettings {                        // verse/settings
  map<string, optional float> bus_volume = 1;            // allowed: not hashed
  optional bool mute = 2; optional string quality = 3; optional float look_sensitivity = 4; CameraPrefs camera = 5;
  optional bool motion_look = 6; repeated string hotbar = 7; }                  // ≤ 9 slots
message CameraPrefs { doc.v1.CameraView view = 1; optional float zoom = 2; }

message DisplayName { optional string text = 1; }        // ≤ 24 chars: printable ASCII, Latin-1, ·—…•
```

Rules:

- **Player keys.** A player key is a secret. Only its derived `p-` ID
  appears.
- **Desktop and phone world keys** stay in OS key storage. Recovery is a
  local operator action that preserves character IDs and inventory.
- **Private assets.** `Appearance.private_asset` names an owner-local
  private character. It is meaningful only on the owner's devices and is
  never published (§10.15).

### 10.12 `openagents.verse.npc.v1`

Purpose: townsfolk and every other non-player character. This covers
character cards, brains, daily routines, towns, rumors, the town clock,
dialogue lines and the request to ask a character something.

```proto
package openagents.verse.npc.v1;

message Villager {                              // openagents.verse-npc.v1
  optional string id = 1;                                // 1–40
  optional string name = 2;                              // ≤ 24
  Card card = 3; Tint look = 4; optional string home = 5; optional string workplace = 6;          // place.v1 node ids
  repeated RoutineRow routine = 7;              // ≤ 24
  LineSet lines = 8;                            // ≤ 12 lines total
  optional string brain = 9;                             // brain definition name
  optional string knowledge = 10;                        // taught text, ≤ 4000 bytes
  optional uint32 seed = 11;
}
message Card { optional string character = 1; optional string role = 2; optional string about = 3; }   // role ≤ 40, about ≤ 280
message Tint { repeated float rgb = 1; }
message RoutineRow { optional uint32 minute = 1; optional string node = 2; optional string activity = 3; }   // town-day minute
message LineSet { repeated string greet = 1; repeated string hint = 2; repeated string ask = 3;
  repeated string answer = 4; }                 // each ≤ 160

message Town {                                  // openagents.verse-town.v1
  optional uint32 schema = 1; repeated Villager villagers = 2;      // ≤ 40
  TownBudgets budgets = 3; repeated bytes admitted = 4; }  // admitted villager/rumor digests (owner only)
message TownBudgets { optional uint32 rumors_in_flight = 1; optional uint32 replies_per_player_day = 2; optional float draw_radius_m = 3; }  // 32, 50, 90
message TownProposal { optional string id = 1; oneof change { Villager add = 2; string remove = 3; Rumor rumor = 4; } }

message Rumor {                                 // openagents.verse-rumor.v1
  optional string id = 1; optional string fact = 2;               // ≤ 200
  optional string source = 3; optional string node = 4; optional uint32 day = 5; optional uint32 at_minute = 6; optional uint32 days = 7;   // 1–7
  optional uint32 step = 8; optional bool repeat = 9; }

message TownClock { optional int64 epoch_ms = 1;         // 2026-10-01
  optional uint32 real_seconds_per_day = 2; }            // 3600: one town day per real hour

message Brain { optional string name = 1; repeated BrainState states = 2; }    // critter, townsfolk, follower, guard...
message BrainState { optional string name = 1; optional string move = 2; optional float radius_m = 3; repeated BrainTransition on = 4; }
message BrainTransition { optional string event = 1; optional string to = 2; }

message AskRequest {                            // /api/npc/ask, host v8 talk; INTENT
  optional string request_id = 1; optional string world = 2;
  NpcBrief npc = 3; repeated Turn history = 4;  // ≤ 10
  optional string question = 5; }                        // PERSONAL; ≤ 500
message NpcBrief { optional string name = 1; optional string role = 2; optional string knowledge = 3; optional string greet = 4; }
message Turn { optional string from = 1; optional string text = 2; }                      // ≤ 500
message AskReply { optional string text = 1;             // ≤ 600
  optional string model = 2; AnswerSource source = 3; openagents.common.v1.Refusal refusal = 4; }
enum AnswerSource { ANSWER_SOURCE_UNSPECIFIED = 0; ANSWER_SOURCE_TAUGHT = 1;   // nearest taught sentence
  ANSWER_SOURCE_MODEL = 2; ANSWER_SOURCE_FALLBACK = 3; }

message DialogueLine { optional uint64 sequence = 1; optional string npc = 2; optional string name = 3; LineKind kind = 4;
  optional string text = 5; optional string emote = 6; }          // host v8: greet|hint|reply|emote; wave|hop|spin|bow
enum LineKind { LINE_KIND_UNSPECIFIED = 0; LINE_KIND_GREET = 1; LINE_KIND_HINT = 2; LINE_KIND_REPLY = 3; LINE_KIND_EMOTE = 4; }
```

Rules:

- **Determinism.** Routines and rumor spread are pure functions of seed
  and clock, and must give identical results on every device.
- **Owner-only admission.** Only the owner admits or removes villagers
  and rumors.
- **Model answers go through the decision and inference doors.** An ask
  reply that came from a model is labelled `MODEL`. Taught-sentence
  matching uses the semantic selector, never keyword routing.

### 10.13 `openagents.verse.place.v1`

Purpose: the world tree. Places form a containment tree of districts,
buildings, rooms and objects, each with stable IDs, standing points and a
closed set of affordances. The tree is what lets agents ground actions in
places.

```proto
package openagents.verse.place.v1;

message WorldTree { optional uint32 schema = 1; optional string world = 2; repeated Node nodes = 3; optional bytes digest = 4; }
message Node {
  optional string id = 1;                                // everglade/knowledge-district/owners-house/great-room/workstation
  NodeKind kind = 2; optional string district = 3; optional string parent = 4;
  engine.v1.Vec3 stand = 5; optional float heading_deg = 6; engine.v1.Vec3 entry = 7;
  repeated Affordance affordances = 8; optional bool exclusive = 9; optional bool open = 10; optional string label = 11;
}
enum NodeKind { NODE_KIND_UNSPECIFIED = 0; NODE_KIND_DISTRICT = 1; NODE_KIND_BUILDING = 2; NODE_KIND_ROOM = 3; NODE_KIND_OBJECT = 4; }
enum Affordance { AFFORDANCE_UNSPECIFIED = 0; AFFORDANCE_SIT = 1; AFFORDANCE_WORK = 2; AFFORDANCE_READ = 3;
  AFFORDANCE_TALK = 4; AFFORDANCE_OPEN = 5; AFFORDANCE_LIGHT = 6; AFFORDANCE_SLEEP = 7; AFFORDANCE_EAT = 8;
  AFFORDANCE_POST = 9; AFFORDANCE_PLAN = 10; AFFORDANCE_REVIEW = 11; AFFORDANCE_MERGE = 12; }
message KnownSubgraph { optional bytes agent = 1; repeated string nodes = 2; optional bytes tree = 3; }    // per-agent knowledge
message PlaceChoice { optional string node = 1; Affordance affordance = 2; optional string reason = 3; }  // decision output, grounded
```

Rules:

- The affordance set is closed. Adding one is a schema change.
- `PlaceChoice` is the typed output of a decision call (Clef first, via
  `/v1/decisions`). It is a proposal, and the host checks it against the
  agent's known subgraph and the node's `exclusive` flag before moving the
  agent.

### 10.14 `openagents.verse.agent.v1`

Purpose: agents as residents of the Verse. This covers seats, stations,
Agent Studio views, crew, workshop agents, greetings and scans. Agent
identity and control belong to `openagents.agent`, so these messages are
the in-world projection.

```proto
package openagents.verse.agent.v1;

message Seat { optional string id = 1; optional bytes agent = 2; optional string station = 3; optional string activity = 4; optional string task = 5;
  repeated engine.v1.Vec3 route = 6; common.v1.AvatarLook look = 7; }
enum Station { STATION_UNSPECIFIED = 0; STATION_GOAL_ATRIUM = 1; STATION_TASK_WALL = 2; STATION_DESK = 3;
  STATION_WORKBENCH = 4; STATION_LIBRARY = 5; STATION_ORACLE = 6; STATION_PROVING_GROUND = 7; STATION_PODIUM = 8;
  STATION_MERGE = 9; STATION_LOUNGE = 10; }

message StudioSnapshot { optional uint64 sequence = 1; repeated Goal goals = 2; repeated Seat seats = 3; repeated Task tasks = 4;
  repeated Decision decisions = 5; repeated RepoSummary repos = 6; repeated string log_tail = 7; }    // VIEW
message StudioUpdate { optional uint64 sequence = 1; repeated Seat seats = 2; repeated Task tasks = 3; repeated Decision decisions = 4;
  repeated string log_append = 5; repeated string removed = 6; }
message Goal { optional string id = 1; optional string text = 2; optional string state = 3; }
message Task { optional string id = 1; optional string title = 2; optional string status = 3; repeated string depends_on = 4; optional string assignee = 5; }
message Decision { optional string id = 1; optional string question = 2; repeated string options = 3; optional string answer = 4; }
message RepoSummary { optional string repo = 1; optional string head = 2; optional uint32 open_tasks = 3; }

message Greeting { common.v1.EntityRef from = 1; common.v1.EntityRef to = 2; optional int64 at_ms = 3; }   // 7 m, 45 s per pair
message Scan { common.v1.EntityRef by = 1; repeated common.v1.EntityRef seen = 2; optional float radius_m = 3; }
message CrewMember { optional string id = 1; optional bytes agent = 2; optional string role = 3; optional string verdict = 4; }       // crew-verdict.v1 view
message WorkshopAgentView { optional string id = 1; optional bytes agent = 2; optional string standing_job = 3; optional uint32 memory_entries = 4;
  optional string day_plan_digest = 5; }
```

Rules:

- **The host owns all Studio state.** Operations (`studio.goal.submit`,
  `seat.pause`, `merge.decide` and the rest) are NIP-HOST operations under
  their rights. This package never defines an operation that bypasses
  them.
- **Seats on the plaza** (agents as NIP-MV `agent` entities) use
  `presence.v1` with `role=agent`. This package adds only what the seat
  is doing.

### 10.15 `openagents.verse.quest.v1`

Purpose: quests a world runs. This covers the Haunted House's "Make it
yours" chain, tutorial quests, realm quests with Accept and Claim, and
trigger-driven steps. It also covers the in-world view of NIP-XP quests
and awards.

```proto
package openagents.verse.quest.v1;

message QuestDef { optional string id = 1; optional string name = 2; repeated StepDef steps = 3; optional string xp_quest = 4; }  // 30193 coordinate, optional
message StepDef { optional string id = 1; optional string label = 2; StepWhen when = 3; optional string guide_target = 4; optional string guide_text = 5; }
message StepWhen { oneof cond { string fact = 1; string trigger = 2; string event = 3; } }   // closed: named facts only

message QuestFacts {                            // quest-facts; RUNTIME, local to the page or host
  optional bool editing = 1; optional bool touch = 2; optional bool planet = 3; optional bool name_empty = 4; optional bool survey_open = 5; optional bool theirs = 6;
  optional string pane = 7; optional bool holding = 8; optional bool planet_made = 9; optional bool invited = 10; optional bool survey_answered = 11; }
message QuestProgress { optional string quest = 1; optional uint32 at = 2; repeated string done = 3; optional int64 updated_ms = 4; }   // haunt/quest {at}
message QuestView { optional string quest = 1; optional string name = 2; optional uint32 step = 3; repeated string steps = 4; optional string edge = 5;
  optional bool accepted = 6; optional bool claimable = 7; }

// Read-only projection of NIP-XP for boards and name tags.
message XpProjection { optional bytes trainer = 1; optional uint64 total = 2; optional uint32 level = 3; optional string curve = 4;   // trainer-curve-v1
  repeated bytes award_events = 5; optional bool shown = 6; }
```

Rules:

- The Verse never publishes XP. A referee signs NIP-XP, and the Verse
  reads it. Level n+1 needs ceil(100·n^1.5) cumulative XP under
  `trainer-curve-v1`.
- Playtest XP is never summed into the trainer level.
- Quest progress on a page is local (AESV v1 today, AESV v2 with
  `QuestProgress` after migration). Realm quest state belongs to the
  realm authority.

The designed time-and-quests model in Metareal engine
`docs/time-and-quests.md` (clock, events, schedules, items, rules,
`/api/time`, `/api/state`) lands here as `quest.v1` `Rule` and `Schedule`
messages when it is built. Those messages are reserved, not sketched,
until that design is accepted.

### 10.16 `openagents.verse.galaxy.v1`

Purpose: the universe picker and players' planets. This covers the
galaxy's worlds and live planets, planet setup, visits and invitations.

```proto
package openagents.verse.galaxy.v1;

message Planet { optional string id = 1;                 // "planet-" + 12 alnum + 4 hex, one per player
  optional uint64 n = 2; optional string name = 3;                // ≤ 40
  optional string owner = 4;                             // display name ≤ 40; owner id stays server-side
  doc.v1.PlanetEnvironment env = 5; common.v1.ArtStyle art = 6; doc.v1.CameraView camera = 7; doc.v1.DayCycle cycle = 8;
  optional int64 made_ms = 9; optional int64 at_ms = 10; }
message PlanetList { repeated Planet planets = 1; optional uint64 n = 2; }      // GET ?after=N; galaxy polls every 3 s
message CreatePlanetRequest { optional string request_id = 1; doc.v1.PlanetSetup settings = 2; }
message MyPlanet { doc.v1.WorldDocument doc = 1; repeated string invited = 2; optional int64 saved_ms = 3; }

message GalaxyWorld { optional string id = 1; optional string label = 2; optional uint32 orbit = 3; world.v1.ZoneEntry zone = 4; optional string href = 5; }
message Galaxy { repeated GalaxyWorld worlds = 1; PlanetList planets = 2;
  optional uint32 full_planets = 3; }                    // newest 48 drawn full; older become belt dust

message VisitLink { optional string planet = 1; doc.v1.PlanetSetup setup = 2; }  // haunt.html?visit=<id>&planet=<kind>&…
message Invitation { optional string planet = 1; optional string to = 2; optional int64 at_ms = 3; }
```

Rules:

- **No locked or placeholder destinations.** The galaxy's worlds come from
  the host's `ZoneCatalog` filtered to `OPEN`.
- **The galaxy keeps its own worlds without a server.** That is the iOS
  bundle case. Planets need the planets service.
- **A visit link must work when shared.** The setup travels in the link,
  and the published document comes from `publish.v1`.

### 10.17 `openagents.verse.asset.v1`

Purpose: describing every asset the Verse loads, without carrying asset
bytes. This covers pack descriptors, catalogs, sources and licenses,
private assets and their placements, decode budgets and cache policy. It
builds on `engine.v1.AssetManifest` and `ConsumerAssetProfile`.

```proto
package openagents.verse.asset.v1;

message PackDescriptor {                        // one content-addressed pack file
  optional string id = 1; optional string group = 2;              // core, chars, skins, far, sounds, props, village, nature, anims, grid, town...
  common.v1.ContentPin pin = 3;                 // exact bytes; redirects refused
  optional string format = 4;                            // VTP3, scene.bin, anims.bin, AGR1, TWN2, VersePack-v1, KTX2, GLB...
  optional uint32 format_version = 5;
  repeated engine.v1.BufferDescriptor buffers = 6;
  DecodeLimits limits = 7;
  repeated string depends_on = 8;
  Provenance provenance = 9;
}
message DecodeLimits { optional uint64 max_transfer_bytes = 1; optional uint64 max_decoded_texture_bytes = 2; optional uint32 max_texture_px = 3;
  optional uint64 max_triangles = 4; optional uint64 max_triangles_per_model = 5; optional uint64 max_gpu_bytes = 6; }
message Provenance { optional string author = 1; optional string source_revision = 2; optional string compiler_revision = 3; optional string license = 4;
  optional string source = 5;                            // fab listing, original, procedural, AI-assisted (flagged)
  optional bool ai_generated = 6; optional bool no_training = 7; }  // no_training: MetaHuman/Draconia rule

message WorldPacks { optional string world = 1; repeated string first = 2; repeated string later = 3; }   // asset-packs.js
message PackIndex { optional uint32 schema = 1; repeated PackDescriptor packs = 2; repeated WorldPacks worlds = 3;
  optional bytes scene_bounds_digest = 4; }

message CatalogEntry { optional string key = 1; optional string name = 2; optional string group = 3;   // Furniture, Decor, Lights, Spooky, Nature...
  optional string mesh = 4; ObjectTemplate template = 5; repeated string variants = 6; }
message ObjectTemplate { presentation.v1.ObjectLight light = 1; optional string shape = 2; optional float default_scale = 3; optional bool building = 4; }
message Catalog { optional uint32 schema = 1; optional string id = 2;              // openagents.verse.fantasy-catalog.v1 ...
  repeated CatalogEntry entries = 3; }

message SourceManifest { optional uint32 schema = 1; repeated SourceEntry sources = 2; }   // openagents.verse.source-manifest.v1
message SourceEntry { optional string id = 1; common.v1.ContentPin pin = 2; Provenance provenance = 3; }

// Private assets: owner-local; never public; readers are explicit pubkeys.
message PrivateAsset { optional string name = 1; optional string title = 2; optional string kind = 3;       // character
  Provenance source = 4; optional string license = 5; repeated bytes readers = 6;
  PrivatePack pack = 7; optional string vendor = 8; }
message PrivatePack { optional bytes sha256 = 1; optional uint64 byte_length = 2; optional string format = 3; repeated string forms = 4;
  optional uint64 triangles = 5; optional string rig = 6; repeated string clips = 7; }
message PrivatePlacements { optional string broker = 1; optional string profile = 2; repeated PrivatePlacement placements = 3; }  // ≤ 64 KiB
message PrivatePlacement { optional string asset = 1; optional bytes sha256 = 2; optional uint64 byte_length = 3; optional string zone = 4;
  optional float x = 5; optional float z = 6; optional float yaw_deg = 7; optional float scale = 8; optional string seat = 9; }
message PrivateUrlRequest { optional string name = 1; optional bytes sha256 = 2; }       // NIP-98 27235-authenticated
message PrivateUrlReply { optional string url = 1; optional int64 expires_ms = 2; }      // 300 s; every refusal is a bare 403
```

Rules:

- **Packs are data, never code.** A pack never carries scripts or shaders
  ([NIP-MV loading rules](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/nips/openagents/NIP-MV.md), Metareal INV-09).
- **Clients enforce their own decode limits.** The publisher's
  `DecodeLimits` can only make a client stricter.
- **Licensed and private bytes never go in Git or public records.**
  Committed code never names a private asset. `PrivatePlacements` stay on
  the owner's devices. `no_training` provenance is binding on every
  pipeline that reads the asset.
- **The current limits stand:**
  - Everglade: pack 12 MiB, 48 MiB textures, 540k triangles, 600k drawn,
    160 MiB GPU.
  - Private assets: 8 MiB transfer, 1024 px, 16 MiB decoded, 30k
    triangles.

### 10.18 `openagents.verse.presentation.v1`

Purpose: how a world looks, as data and tokens only. This covers
atmosphere, environment and post-processing values, art styles, lighting
profiles, quality tiers, device-profile references and particle effects.
It contains no shaders and no layouts.

```proto
package openagents.verse.presentation.v1;

message Atmosphere { repeated float sky = 1; repeated float fog = 2; optional float fog_density = 3; optional float far_m = 4;  // colors 0–1; fog ≤ 2 km
  repeated float ambient_sky = 5; repeated float ambient_ground = 6; }
message ObjectLight { optional string kind = 1; repeated float color = 2; optional float radius_m = 3; optional float flicker = 4;
  engine.v1.Vec3 at = 5; }
message Look {                                  // the world document's look{}
  Atmosphere atmosphere = 1; optional bool moon = 2; engine.v1.Vec3 moon_dir = 3;
  repeated float grade = 4; repeated float grade_dark = 5; repeated float grade_mid = 6; repeated float grade_light = 7;
  optional float vignette = 8; optional float exposure = 9; optional uint32 character_lod = 10; optional float wind = 11; optional string roofs = 12; }
message Environment {                           // env command
  engine.v1.Vec3 sun_dir = 1; repeated float sun_color = 2; optional float ambient = 3; Atmosphere fog = 4; optional float ground_fog = 5;
  repeated float grade = 6; optional string post = 7; optional float wind = 8; optional bool toon = 9; optional bool ink = 10; optional float hour = 11; optional string sky = 12; }
message LightingProfile { optional string id = 1; LightUnits units = 2; optional float exposure_ev100 = 3; }
enum LightUnits { LIGHT_UNITS_UNSPECIFIED = 0; LIGHT_UNITS_CHAMBER = 1; LIGHT_UNITS_PHYSICAL = 2; }  // candela/lux/EV100
enum QualityTier { QUALITY_TIER_UNSPECIFIED = 0; QUALITY_TIER_LOW = 1; QUALITY_TIER_MEDIUM = 2; QUALITY_TIER_HIGH = 3; }
message TierBudgets { QualityTier tier = 1; optional uint32 lamps = 2; optional uint32 shadow_views = 3; optional uint32 particles = 4;
  optional float render_scale_min = 5; }                 // lamps 8/16/32, shadows 6/12/24, particles 160 on low
message DeviceProfileRef { optional string profile = 1;  // legacy/device-profile-v1 id; the profile itself is openagents.ui / metareal-profile
  QualityTier tier = 2; }

message ParticleEffect { optional string id = 1; optional uint32 seed = 2; optional uint32 max_particles = 3; optional float priority = 4;
  optional float stretch = 5; optional float stretch_max = 6; optional string emitter = 7; }      // ≤ 4096 particles, ≤ 256 effects
message ArtStyleProfile { common.v1.ArtStyle style = 1; optional string presentation_profile = 2; }   // npr.outline-gray.v1 ...
```

Rules:

- Colors here are world appearance values that the world document
  authors. App chrome never takes colors from here. App chrome uses
  `oa-tokens` through `openagents.ui.v1`.
- A shared app palette does not constrain the appearance of a world. That
  rule comes from NIP-MV.

### 10.19 `openagents.verse.environment.v1`

Purpose: simulated environment. This covers water and sea state, weather,
ice, destruction and terrain edits.

```proto
package openagents.verse.environment.v1;

message SeaState { optional uint64 seed = 1; optional uint64 tick = 2; optional uint32 period_s = 3; }   // deterministic; folds every 8,192 s
message WeatherSchedule { optional string zone = 1; repeated WeatherSpan spans = 2; }
message WeatherSpan { optional uint64 from_tick = 1; optional uint64 to_tick = 2; optional string kind = 3; optional float intensity = 4; }
message IceCell { optional sint32 x = 1; optional sint32 z = 2; optional float tolerance_kg = 3; }  // 3 m cells; 3d10×4.5 kg

// Destruction (designed D4): host owns structure; cosmetic chunks local from pattern + seed.
message DestructionEvent { oneof event { PieceDamaged damaged = 1; PieceBroken broken = 2; SeamBroken seam = 3; BuildingSettled settled = 4; } }
message PieceDamaged { optional string building = 1; optional uint32 piece = 2; optional uint32 hp = 3; }
message PieceBroken { optional string building = 1; optional uint32 piece = 2; optional string pattern = 3; optional uint64 seed = 4; }
message SeamBroken { optional string building = 1; optional uint32 seam = 2; }
message BuildingSettled { optional string building = 1; optional uint64 revision = 2; }
message DestructionBaseline { repeated BuildingBaseline buildings = 1; }  // late joiners
message BuildingBaseline { optional string building = 1; optional uint64 revision = 2; optional bytes broken_bitset = 3;
  repeated PieceDamaged damaged = 4; repeated LeaningPiece leaning = 5; optional bytes rubble_digest = 6; }
message LeaningPiece { optional uint32 piece = 1; common.v1.Pose pose = 2; }

message TerrainEdit { optional string tile = 1; repeated sint32 heights_mm = 2; optional uint32 brush = 3; }   // terrain.heightfield.v1; ≤ 4 tiles
message Medium { optional string kind = 1; optional float depth_m = 2; }                   // swimmer medium field (proposed)
```

Rules:

- Destruction never relies on lockstep, and cross-architecture float
  equality is not promised. The host replicates at most 64 bodies per
  viewer.
- Gameplay water state is authoritative. Visual detail is local.

### 10.20 `openagents.verse.host.v1`

Purpose: the bridges between a Verse surface and the thing that hosts it.
This covers the Metareal tools host protocol (one consolidated vocabulary
replacing v1 to v8), the mobile shell bridge (iOS `verseShell` and
`legacy/save-bridge-v1`, and Android equivalents), the mobile game bridge, and
playables.

```proto
package openagents.verse.host.v1;

// Metareal tools host protocol: envelope with session, per-direction sequence, request id.
message HostEnvelope { optional string session = 1; optional uint64 seq = 2; optional string request_id = 3; oneof body {
  HostHello hello = 10; HostReady ready = 11; HostRequest request = 12; HostResult result = 13; HostStatus status = 14; } }
message HostHello { optional uint32 protocol = 1; repeated string ops = 2; }    // ops actually advertised (private adapter: inspect, observe)
message HostReady { optional string world = 1; optional uint64 revision = 2; }
message HostRequest { oneof op {
  Open open = 1; Inspect inspect = 2; Observe observe = 3; Close close = 4;
  authoring.v1.Command propose = 5; Draft draft = 6; Compile compile = 7; Undo undo = 8; Redo redo = 9;
  Save save = 10; publish.v1.PublishWorldRequest publish = 11; Share share = 12; Navigate navigate = 13; Capture capture = 14;
  Lookup lookup = 15; Cancel cancel = 16; View view = 17; ExportProject export_project = 18; RestoreProject restore_project = 19;
  npc.v1.AskRequest talk = 20; } GrantUse grant = 30; }
message GrantUse { optional string grant_id = 1; optional bytes action_sha256 = 2; optional int64 expires_ms = 3; }   // 5 min, bound to action hash
message HostResult { HostResultKind kind = 1; openagents.common.v1.Refusal refusal = 2; SaveStatus save = 3; optional bytes payload = 4; }
enum HostResultKind { HOST_RESULT_KIND_UNSPECIFIED = 0; HOST_RESULT_KIND_APPLIED = 1; HOST_RESULT_KIND_REJECTED = 2;
  HOST_RESULT_KIND_UNSUPPORTED = 3; HOST_RESULT_KIND_CANCELLED = 4; HOST_RESULT_KIND_UNKNOWN = 5; }
enum SaveStatus { SAVE_STATUS_UNSPECIFIED = 0; SAVE_STATUS_UNSAVED = 1; SAVE_STATUS_DEVICE_ONLY = 2;
  SAVE_STATUS_PROJECT_SAVED = 3; SAVE_STATUS_FAILED = 4; }
message HostStatus { repeated npc.v1.DialogueLine lines = 1; repeated string npcs = 2; repeated string nearby = 3; }   // ≤ 10/s
// (Open..RestoreProject: small closed messages matching metareal-host-protocol.md.)

// Mobile shell bridge (WKWebView / Android WebView): typed replacement for postMessage JSON.
message ShellRequest { optional string id = 1; optional uint64 generation = 2; oneof op {
  CloudCall cloud = 10; WorldFocus world = 11; MotionControl motion = 12; SharePatch share = 13; ConsoleLine console = 14;
  SaveGet save_get = 15; SavePut save_put = 16; SaveDelete save_delete = 17; economy.v1.WalletRequest wallet = 18; } }
message ShellReply { optional string id = 1; optional uint64 generation = 2; optional bool ok = 3; openagents.common.v1.Refusal error = 4; optional bytes value = 5; }
message CloudCall { optional string path = 1; optional string method = 2; optional bytes body = 3; }   // ≤ 180-byte path; proxied to the API base
message SavePut { optional string key = 1; optional bytes value = 2; }                    // AESV bytes or a record
// (WorldFocus, MotionControl{on|off|center}, SharePatch ≤ 300 KB, ConsoleLine ≤ 2000, SaveGet, SaveDelete.)

// Mobile game bridge (mobile-games.md, proposed). Every message has schema and session generation; ≤ 64 KiB.
message GameHostMessage { optional uint32 schema = 1; optional uint64 generation = 2; oneof m {
  Start start = 10; Pause pause = 11; Resume resume = 12; Resize resize = 13; SetSettings settings = 14; Stop stop = 15; } }
message GamePlayerMessage { optional uint32 schema = 1; optional uint64 generation = 2; oneof m {
  Ready ready = 10; Paused paused = 11; MenuRequested menu = 12; SettingsChanged settings = 13; SaveChanged save = 14;
  RunEnded ended = 15; Failed failed = 16; } }

message Playable { optional uint32 schema = 1;           // openagents.verse.playable.v2 keeps the recipe
  optional string game = 2; publish.v1.ReleaseArtifact entry = 3; optional bytes recipe_digest = 4; }
```

Rules:

- **Capabilities are not permission.** The host checks identity and
  membership per request, and grants are bound to the exact action hash.
- **Envelope limits:** 524,288 bytes (frozen v1 to v5: 65,536), 16
  pending requests, 64 retained results, a 15 s timeout (120 s for the
  initial load).
- **The bridges have no generic pass-through.** `CloudCall` reaches the
  configured API base only. The wallet goes through `economy.v1`, not raw
  RPC.

### 10.21 `openagents.verse.gym.v1`

Purpose: the Gym building's views, meaning what the boards show while the
player is inside. Eval records belong to `openagents.eval.v1`.

```proto
package openagents.verse.gym.v1;
message GymBoard { optional uint32 schema = 1; repeated RunObservation runs = 2; repeated RecipeRef recipes = 3; optional int64 at_ms = 4; }  // ≤ 64 runs, 16 recipes, ≤ 128 KiB
message RunObservation { optional string run = 1; optional string suite = 2; optional string state = 3; optional uint32 passed = 4; optional uint32 total = 5;
  optional string landmark = 6; }                        // workbench, oracle, library, proving ground, plaza
message RecipeRef { optional string id = 1; optional uint32 revision = 2; optional string label = 3; }   // launched by identity + revision only
message EvalsRow { optional string suite = 1; optional string capability = 2; optional bytes result_event = 3; optional uint32 confirmed_by = 4; optional uint32 disputed_by = 5; }
message GymNote { optional bytes event = 1; optional bytes author = 2; optional bytes cites = 3; optional bytes answers = 4; optional string text = 5; }   // ≤ 500 chars
```

Rules:

- A board shows host observations. It is not a verified leaderboard.
- A lost launch response is `unknown`. Retrying it reuses the same ID.
- Notes are rate-limited (4 per hour, 1 opener per 15 min) and off by
  default.

### 10.22 `openagents.verse.terminal.v1`

Purpose: screens in the world that show NIP-TERM terminals. Terminal
bytes never enter world events.

```proto
package openagents.verse.terminal.v1;
message ScreenBinding { optional string screen = 1; optional string node = 2; optional string terminal = 3; optional uint64 generation = 4;
  TermMode mode = 5; common.v1.GrantRef grant = 6; }
enum TermMode { TERM_MODE_UNSPECIFIED = 0; TERM_MODE_INTERACT = 1; TERM_MODE_OBSERVE = 2; }
message MuxLayout { oneof node { Split split = 1; string pane = 2; } }
message Split { optional bool vertical = 1; optional float ratio = 2; MuxLayout a = 3; MuxLayout b = 4; }   // 0.1–0.9
message TerminalShareView { optional string terminal = 1; optional bytes grantee = 2; TermMode mode = 3; optional uint64 first_seq = 4; optional int64 expires_ms = 5; }
```

### 10.23 `openagents.verse.media.v1`

Purpose: media inside the world, plus the film pipeline. This covers voice
and video placement (references to LIVE sessions), spatial audio regions,
film takes, shots and cuts.

```proto
package openagents.verse.media.v1;
message SpatialAudioZone { optional string id = 1; engine.v1.AxisAlignedBox volume = 2; optional string live_session = 3; optional float falloff_m = 4; }
message Screen { optional string id = 1; optional string node = 2; optional string media_slot = 3; }    // never a URL; host resolves the slot

message Shot { optional string name = 1; ShotScene scene = 2; optional float duration_s = 3; }
message ShotScene { optional string kind = 1; common.v1.WorldId world = 2; repeated Walker walkers = 3; optional string portal = 4; }
message Walker { optional string route = 1; optional float speed_mps = 2; optional float start_s = 3; optional string agent = 4; }
message Cut { optional uint32 fps = 1; optional uint32 width = 2; optional uint32 height = 3; optional float bpm = 4; repeated string shots = 5; }  // 1080p until approved
message TakeIndex { optional string take = 1; optional bytes digest = 2; optional uint64 byte_length = 3; optional uint32 wire = 4; }  // take wire v1 stays binary (84-byte records)
```

Rules:

- **Film output.** Cuts render at 1080p until the owner approves, and the
  voiceover voice is configured outside the schema.
- **Licensed sources.** MetaHuman and Draconia sources are private assets
  (§10.17).

### 10.24 `openagents.verse.telemetry.v1`

Purpose: playtest measurement across the Verse. This covers events,
sessions, funnels, survey prompts and answers, remark classifications,
consent and automation marking. It is append-only and privacy-annotated.

```proto
package openagents.verse.telemetry.v1;

message EventBatch { optional string sid = 1;            // ^[A-Za-z0-9_:-]{8,80}$
  repeated Event events = 2;                    // ≤ 50 per batch, ≤ 5000 per session
  optional bool automated = 3; optional string automation_tool = 4; }
message Event { optional string name = 1;                // ^[a-z][a-z0-9-]{0,31}$ from REGISTRY: landing, galaxy, world-enter, place, publish, prompt...
  optional string page = 2;                              // ≤ 32
  optional int64 client_ms = 3;
  repeated Property props = 4; }                // ≤ 16
message Property { optional string key = 1;              // ^[A-Za-z][A-Za-z0-9_]{0,23}$
  oneof value { string text = 2; double number = 3; bool flag = 4; }   // text ≤ 80
  optional bool personal = 5; }                          // free text ≤ 500, kept only with consent
message StoredEvent { optional int64 server_ms = 1; optional string sid = 2; optional string account = 3; Event event = 4; optional bool automated = 5; }

message Consent { optional bool save_text = 1; optional int64 at_ms = 2; optional string notice_revision = 3; }

message SurveyQuestion { optional string id = 1; optional string text = 2; repeated string choices = 3; optional bool free_text = 4; SurveyMoment moment = 5; }
enum SurveyMoment { SURVEY_MOMENT_UNSPECIFIED = 0; SURVEY_MOMENT_FIRST_CHANGE = 1; SURVEY_MOMENT_PUBLISH = 2;
  SURVEY_MOMENT_PLANET_SETUP = 3; SURVEY_MOMENT_WRAP_UP = 4; }
message SurveyAnswer { optional string rid = 1; optional string question = 2; optional string text = 3;   // ≤ 1000
  repeated string choices = 4; optional bool skipped = 5; optional uint32 n = 6; optional uint32 of = 7; optional string player = 8; optional string sid = 9; }

enum RemarkKind { REMARK_KIND_UNSPECIFIED = 0; REMARK_KIND_TALK = 1; REMARK_KIND_CONFUSED = 2; REMARK_KIND_LOVE = 3;
  REMARK_KIND_WISH = 4; REMARK_KIND_BROKEN = 5; REMARK_KIND_FRUSTRATED = 6; }
message Remark { RemarkKind kind = 1; optional float score = 2; optional string route = 3; optional string text = 4; optional string model = 5; }  // text personal

message PlaytestSessionRef { optional bytes xp_session_event = 1; optional bytes xp_report_event = 2; }   // NIP-XP 3196 / 3197
```

Rules:

- Event names and property keys come from `telemetry/v1/REGISTRY.md`.
  Adding one is a registry change, not free-form logging.
- Our own tools mark themselves. That is the `verse-automation` cookie,
  the `X-Verse-Automation` header or WebDriver. Marked sessions are kept
  and excluded from funnels and exports by default.
- The remark classifier is the semantic selector, an embedding plus small
  networks, never keyword matching. This follows the workspace's
  semantic-routing rule.
- Stored files are append-only and are never rewritten.

### 10.25 `openagents.verse.economy.v1`

Purpose: what the world shows and asks for about money, without owning
money. This covers prices on offers, settlement views, reward links and
the wallet bridge envelope.

```proto
package openagents.verse.economy.v1;
message Price { optional string amount_msat = 1; optional string unit = 2; }              // "sats" display; msat decimal string on the wire
message SettlementView { optional string id = 1; optional string world = 2; optional string offer = 3; optional string state = 4; optional string amount_msat = 5;
  optional string payments_ref = 6; }                    // openagents.payment record id
message RewardLink { optional string contribution = 1; optional string credit = 2; optional string recorded_by = 3; }   // ≤ 1024 links, ≤ 8 per contribution
message WalletRequest { optional string id = 1; optional uint64 generation = 2; oneof op {
  bool status = 10; bool open = 11; bool refresh = 12; Receive receive = 13; PrepareSend prepare_send = 14;
  bool present_send_review = 15; PaymentsPage payments = 16; bool deposits = 17; bool present_claim_review = 18;
  bool present_refund_review = 19; bool present_backup = 20; bool present_restore = 21; bool recovery_status = 22; } }
message Receive { ReceiveRail rail = 1; optional string amount_msat = 2; }
enum ReceiveRail { RECEIVE_RAIL_UNSPECIFIED = 0; RECEIVE_RAIL_SPARK = 1; RECEIVE_RAIL_LIGHTNING = 2; }
message PrepareSend { optional string destination = 1; optional string amount_msat = 2; }
message PaymentsPage { optional uint32 offset = 1; optional uint32 limit = 2; }           // ≤ 50
```

Rules:

- **The rails are Bitcoin only:** Lightning, Spark and Taproot Assets.
  Nothing else is ever added.
- **Money movement is never automatic.** Every `present_*` operation
  opens a native review the person confirms. Keys and mnemonics never
  appear in any Verse message.
- **Payment state is owned elsewhere.** It is read from
  `openagents.payment` through `payments_ref`. The Verse never derives
  "paid" from its own records.

### 10.26 Games: `openagents.verse.game.<game>.v1`

Each game gets its own package for its document, rules profile, progress
and run receipts. All of them reuse `doc.v1`, `rules.v1`, `sim.v1` and
`quest.v1` where they can, and each game package adds only what is
specific to that game.

| Package | Contents | Replaces |
| --- | --- | --- |
| `game.haunt.v1` | House-specific area intents, scare cues, arrival cue, exit, audio beds; the "Make it yours" quest definition | `haunt.v1` (frozen Kiki keeps its own reader) |
| `game.icecream.v1` | `GameDocument{weapons, minions, look}`, live tenant pointer (kind `30440`), designed host-mirrored multiplayer state, 60 s slot hold | `game.v1` |
| `game.bunny.v1` | `Garden` level and rules profile (60 Hz integer math), `Progress` (also an addressable event), `Tutorial`, `Key`, `RunReceipt` | `bunny.garden/progress/tutorial/key/run-receipt.v1` |
| `game.snap.v1` | `Level`, `Progress`, `Tutorial`, `RunReceipt`, `SharedBuild` | `snap.*.v1` |
| `game.duel.v1` | Prussian duel (240 Hz, hit zones, stamina) and hand duel | local Rust structs |
| `game.arena.v1` | Arena encounter rules, guild ledger view, deposits, quest chain | `voyager`-adjacent arena records (the ledger itself stays Voyager's) |
| `game.deadgears.v1` | Dead Gears progress and settings | AESV v2 already; adds typed components |

The shared run receipt shape is
`RunReceipt{game, rules_digest, level_digest, seed, result_digest, replay: sim.v1.GameReplay}`.
A receipt is verified by re-simulating it under the exact execution
profile.

### 10.27 `openagents.verse.space.v1`

Purpose: Lagrange 1 and 2, and any future orbital zones. This covers
station state, EVA, the assembly plan, spacecraft, the CAD mass feed and
orbital frames.

```proto
package openagents.verse.space.v1;
message OrbitalFrame { optional string id = 1; optional string model = 2; optional double mu = 3; }   // "cr3bp-sun-earth", L1 linearized local field
message StationState { optional uint32 schema = 1; optional uint64 tick = 2; repeated PartState parts = 3; repeated Latch latches = 4;
  EvaState eva = 5; }
message PartState { optional string id = 1; common.v1.GlobalVec3 position = 2; common.v1.Quat orientation = 3;
  engine.v1.Vec3 velocity = 4; engine.v1.Vec3 angular_velocity = 5; optional bool installed = 6; }
message Latch { optional string part = 1; optional string port = 2; }   // within 1.6 m, 15°, < 0.35 m/s, < 0.05 rad/s
message EvaState { optional float thrust_n = 1; optional float propellant_kg = 2; optional bool tethered = 3; optional string holding = 4; }   // 40 N; clip 3 m
message StationStatus { optional uint32 installed = 1; optional uint32 total = 2; optional string holding = 3; }
message StationJournal { repeated JournalEntry entries = 1; }
message JournalEntry { optional uint64 tick = 1; presence.v1.ZoneCommand command = 2; }   // grab, release, fly, stop, clip...
message Spacecraft { optional uint32 schema = 1; optional string id = 2; optional double mass_kg = 3; repeated Module modules = 4; }  // spacecraft.v1; ≤ 115 t
message Module { optional string id = 1; optional string kind = 2; optional double mass_kg = 3; }
message AssemblyPlan { optional uint32 schema = 1; repeated AssemblyStep steps = 2; }   // assembly-plan.v1
message AssemblyStep { optional string part = 1; optional string attach_to = 2; optional string port = 3; }
message CadMassFeed { optional uint32 schema = 1; repeated Module modules = 2; optional bytes source_digest = 3; }   // cad-mass-feed.v1
```

### 10.28 `openagents.verse.lab.v1`

Purpose: teaching and debugging labs. This covers Physics Lab's nine
scenarios and their knobs, the robot lab and the render lab's scenario
pins.

```proto
package openagents.verse.lab.v1;
message LabScenario { optional string id = 1; optional string label = 2; repeated Knob knobs = 3; optional uint64 seed = 4; }
message Knob { optional string id = 1; optional string label = 2; optional double min = 3; optional double max = 4; optional double value = 5; optional double step = 6; }
message LabState { optional string scenario = 1; optional double time_scale = 2; optional double gravity = 3; optional bool overlay = 4; repeated Knob values = 5; }
```

Reset must replay bit for bit under the same execution profile.

## 11. Services

Proto services are defined where a request/reply boundary already exists
over HTTP or a bridge. Transport is a deployment choice, so each service
may be served as:

- **gRPC** for native and host-to-host;
- **Connect** (HTTP/1.1 and HTTP/2, binary or JSON) for browsers and
  WebViews, which keeps the existing `fetch` callers simple;
- the **existing JSON routes**, kept as adapters until callers migrate.

| Service | Package | Methods (sketch) | Today |
| --- | --- | --- | --- |
| `WorldService` | `world.v1` | `GetZoneCatalog`, `GetWorldDefinition`, `ListInstances` | `GET /v1/zones`, compiled catalogs |
| `PublishService` | `publish.v1` | `GetPublished`, `Publish`, `Reset`, `SetOpen`, `GetOwnership`, `GetSettings`, `SetSettings`, `GetEditLog` | `/api/world/*`, `/api/log/*` |
| `OfferService` | `publish.v1` | `Offer`, `Accept`, `Decline`, `ListMine` | `/api/change/*`, `/api/wallet` |
| `AuthoringService` | `authoring.v1` | `Submit(Command) → CommandReceipt`, `GetRevision`, `WatchPresence` (stream) | Metareal tools project service, host `propose` |
| `GalaxyService` | `galaxy.v1` | `ListPlanets(after)`, `GetPlanet`, `CreatePlanet`, `GetMine`, `SaveMine`, `Invite` | `/api/planets*`, `/api/award` |
| `ProgressService` | `character.v1` | `GetProgress`, `PutProgress(AESV bytes, at)`, `Erase` | `/api/progress/*` |
| `NpcService` | `npc.v1` | `Ask` | `/api/npc/ask` |
| `TelemetryService` | `telemetry.v1` | `Record(EventBatch)`, `Answer(SurveyAnswer)`; owner-only `Query`, `Export` | `/api/events`, `/api/survey/answer`, `/api/analytics*` |
| `PresenceService` | `presence.v1` | `Update`, `Watch` (stream), for no-relay rooms only | `/api/presence/*` |
| `PrivateAssetService` | `asset.v1` | `GetUrl(PrivateUrlRequest)` | `POST /v1/private/url` |
| `RealmService` | `realm.v1` | `Session(stream ClientFrame) returns (stream ServerFrame)` | chamber TLS/REACH socket |

Realm sessions over REACH or raw TLS keep length-prefixed proto frames
rather than gRPC, because REACH channels are not HTTP/2. The realm payload change is an
explicit new wire version, not a replacement for the REACH handshake or HOST
packet format. Negotiate it through an old-peer-compatible bootstrap or a
separate endpoint; test mixed old/new peers before cutover.

The account and sign-in routes (`/api/auth/*`, `/api/me`, `/api/session`)
belong to the account owner (`openagents.agent` or `openagents.host`), not
the Verse. They are listed in §12 for completeness and get their schema in
that subtree.

## 12. Migration map: every current format and where it goes

The *Keeps* column says what keeps working after migration. *Reader* means
we keep a reader for old records for as long as such records exist.

### 12.1 Nostr and presence

| Current | Where | Becomes | Keeps |
| --- | --- | --- | --- |
| NIP-MV `33300` world definition | `nips/openagents/NIP-MV.md` | `world.v1.WorldDefinition` + `SceneBinding` | NIP-MV JSON on relays (mapped) |
| NIP-MV `33301` entity state (avatar, agent, object, bodies) | `verse-net/src/mv.rs`, `web/grid-session.js` | `presence.v1.EntityState` | NIP-MV JSON (mapped) |
| NIP-MV `23300` pose frame | same | `presence.v1.PoseFrame` | same |
| NIP-MV `23301` gesture | same | `presence.v1.Gesture` | same |
| NIP-MV `23302` zone command | same, `openagents zone` CLI | `presence.v1.ZoneCommand` | same |
| Shared bodies `verse-bare.bodies.v1`, `verse-coast.bodies.v1` | `crates/verse/src/shared.rs` | `presence.v1.BodySnapshot` + profile registry | profile IDs unchanged |
| Object states (lamp, door, workstation, task wall) | `verse_net::mv::object` | `presence.v1.ObjectState` | entity ID rule unchanged |
| Pylon/wellspring projection | `crates/world-tree` | `presence.v1.ObjectState.pylon/wellspring` | NIP-PYLON authority unchanged |
| World chat kind 9, scopes, districts | `verse-net/src/chat.rs` | `social.v1.ChatLine` | NIP-C7 events |
| NIP-29 rooms, NIP-17 PMs | same | `social.v1.Room`, `nostr.nip29.v1`, `nostr.nip17.v1` | upstream NIPs |
| Gym notes | `verse-gym/src/gym_notes.rs` | `social.v1.ChatLine.gym_note`, `gym.v1.GymNote` | kind 9 + NIP-32 label |
| NOSTR tab (kind 1 read-only) | `verse` | `nostr.event.v1.Event` view | — |
| HTTP presence `/api/presence/<room>` (7-number look) | Metareal engine `tools/serve.py` | `presence.v1` via `PresenceService` with `character.v1.LookSpec` (12) | fixes the mismatch |
| HTTP chat `/api/chat/<room>` | Metareal engine | `social.v1.ChatLine` via a room service, or retire for NIP-C7 | last 500 lines |

### 12.2 Realm and simulation

| Current | Becomes | Keeps |
| --- | --- | --- |
| Chamber wire v34 (length-prefixed JSON) | `realm.v1` frames, wire 35 | v34 JSON reader on hosts until clients migrate |
| `ritual.json`, browser `chamber.json`, mobile `chamber_bindings` | `realm.v1.Hello` inputs + `host.v1` shell config | file formats as adapters |
| `verse.input-replay.v1` | `sim.v1.InputReplay` | old replays: reader |
| `verse.game-replay.v1` | `sim.v1.GameReplay` | reader |
| `verse-chamber-owned-v24` checkpoints | `sim.v1.CheckpointIndex` (bodies unchanged) | owned format |
| Combat content (`verse-world/src/content.rs`) | `rules.v1.RulesProfile` | compiled in; exported as data |
| `StationState`, station journal | `space.v1` | reader |
| `spacecraft.v1`, `assembly-plan.v1`, `cad-mass-feed.v1` | `space.v1` | reader |

### 12.3 Worlds, documents, publishing (Metareal engine first)

| Current | Becomes | Keeps |
| --- | --- | --- |
| `verse.world.v1` world document (house, planets, sandbox) | `doc.v1.WorldDocument` | JSON reader (schema 0/1/2 migrations); Kiki's frozen tree keeps its own |
| `{format:"legacy/native-save-v2", schema:2, data:{doc, base}}` | AESV v3 container with `doc.v1` payload; `publish.v1.PublishedWorld` on the wire | reader |
| House delta | `authoring.v1.DocumentDelta` (existing identity rule preserved) | JS/C++ JSON delta reader during migration |
| Prop world `{props, zones}` and `verse.<world>.props/zones/look/view/hotbar` | `doc.v1` + `character.v1.PlayerSettings` | reader |
| Trigger zones as `AuthoredSource` JSON in `state.proto` | `doc.v1.TriggerZone` | `AuthoredSource` contract kept for old saves |
| `haunt.v1` | `doc.v1` + `game.haunt.v1` | frozen Kiki |
| `legacy/world-v1` structural model | `doc.v1.Building` etc. | JSON Schema retained as historical design |
| `legacy/command-v1` | `authoring.v1.Command` | design JSON Schema retired after cutover |
| `legacy/release-v1`, `legacy/channel-v1`, `legacy/signature-v1` | `publish.v1.Release`, `ChannelRef` (mirrors; JCS stays authoritative) | JCS signing unchanged |
| `legacy/player-artifact-v1`, `legacy/stream-package-v1` | `publish.v1.ReleaseArtifact` + `asset.v1.PackIndex` | reader |
| `/api/world/*`, `/api/log/*`, `/api/change/*`, world settings, owner, open | `PublishService`, `OfferService` | JSON routes as adapters |
| `/api/planets*`, `/api/award`, `planets.json`, `awards.json` | `GalaxyService`, `galaxy.v1` | stored JSON read once and rewritten as proto |
| `/api/progress/*` (AESV in JSON) | `ProgressService` (AESV bytes) | AESV container unchanged |
| `openagents.zones.v1` (`GET /v1/zones`) | `world.v1.ZoneCatalog` | JSON route as adapter |
| `ZoneId` serialization, zone atmosphere, portals | `world.v1.ZoneEntry` | — |
| Metareal tools host protocol v1–v8 JSON schemas | `host.v1.HostEnvelope` (protocol 9) | frozen v1–v5 releases keep their SDKs |
| `openagents.verse.playable.v1/.v2` | `host.v1.Playable` | reader |

### 12.4 Characters, NPCs, places, agents

| Current | Becomes |
| --- | --- |
| Player key / `players.json` / `X-Verse-Player` | `character.v1.PlayerView` (key never on wire beyond the header) |
| `LookSpec` (12 ints), `set-look`, metareal seed/look in MV content | `character.v1.LookSpec`, `Appearance`, `presence.v1.EntityState.appearance` |
| Realm accounts, characters, inventory, gear, progress | `character.v1` + `realm.v1.ReadReply` |
| `verse/settings` | `character.v1.PlayerSettings` |
| `openagents.verse-npc.v1`, `-town.v1`, `-town-proposal.v1`, `-rumor.v1` | `npc.v1` |
| Brains JSON | `npc.v1.Brain` |
| `/api/npc/ask`, host v8 `talk`, `legacy/npcs-v1` | `npc.v1.AskRequest/AskReply`, `DialogueLine` |
| `openagents.verse-world-tree.v1` | `place.v1.WorldTree` |
| Studio `studio.snapshot`/`studio.update` | `agent.v1.StudioSnapshot/Update` |
| Workshop agent records (`workshop-agent.v1`, memory, journal, day plan, crew verdict) | stay owned by `openagents.agent`; `agent.v1` views |

### 12.5 Assets and presentation

| Current | Becomes |
| --- | --- |
| VTP3 packs, `scene.bin`, `anims.bin`, `AGR1`, `TWN2`, textures, audio | `asset.v1.PackDescriptor` (bytes unchanged) |
| `asset-packs.js`, `scene-bounds.bin` | `asset.v1.PackIndex` |
| `openagents.verse.private-asset.v1`, `.private-placements.v1` | `asset.v1.PrivateAsset`, `PrivatePlacements` |
| `openagents.verse.source-manifest.v1`, `.fantasy-catalog.v1`, `.character-sources.v1` | `asset.v1.SourceManifest`, `Catalog` |
| Hotbar catalog groups, `web/prefabs.js` templates | `asset.v1.Catalog` |
| `look{}`, `env`, zone atmosphere | `presentation.v1.Look`, `Environment`, `Atmosphere` |
| Particle effects | `presentation.v1.ParticleEffect` |
| `legacy/device-profile-v1`, `legacy/ui-v1` | `openagents.ui.v1` (outside this spec); referenced |

### 12.6 Telemetry, economy, games, media

| Current | Becomes |
| --- | --- |
| `/api/events`, JSONL event files, funnel | `telemetry.v1.EventBatch`, `StoredEvent` (append-only files of length-delimited records; CSV/JSON exports unchanged) |
| `/api/survey/answer`, survey steps, `/api/survey` legacy | `telemetry.v1.SurveyAnswer`, `SurveyQuestion` |
| Remark kinds | `telemetry.v1.Remark` |
| `/api/playtest/join` | `telemetry.v1` + account owner |
| `/api/settlement/*`, `/api/wallet`, `verseWallet` bridge, reward links | `economy.v1` (views and bridge envelopes) |
| `game.v1`, `bunny.*`, `snap.*`, duel, arena, Dead Gears | `game.*.v1` |
| Film take wire v1, shots, cuts | `media.v1.TakeIndex`, `Shot`, `Cut` (take bytes unchanged) |
| Qualification receipts `openagents.verse.*-qualification.v1` | stay JSON acceptance records; not wire formats |

## 13. Tooling, generation and consumers

### 13.1 Standalone modules and consumer profiles

The master §33 and root `proto/README.md` describe the executable tooling.
The former Cargo generator and app crates are historical evidence, not present
build prerequisites. Pinned protoc 29.3 and Python protobuf 5.29.3 compile the
owned/vendor modules and four actual consumer descriptor closures.

```sh
scripts/proto.sh list
scripts/proto.sh generate
scripts/proto.sh check
```

These commands do not export a runtime SDK or implement Coax `#import_proto`.
Current descriptors are regenerated reproducibly; immutable compatibility
snapshots are never regenerated. Profile roots and upstream include paths
come from `proto/modules.json`.

### 13.2 Coax consumers and bootstrap bridges

Coax is the target for Metareal, Verse clients, hosts, tools and agent surfaces.
The root `metareal`, `verse` and `phone-agent` profiles select actual descriptor
closures. The Coax bootstrap does not ingest them yet. Follow master P1 for
aggregates, explicit presence/oneofs, bounded codecs and domain validation.

C/C++ generated bindings and native platform shims are migration bridges, not
a second long-term schema or a reason to rebuild deleted product crates.
Original signatures, asset pins and the current C ABI remain the compatibility
reference until a real consumer proves parity.

### 13.3 Implementation and interop

Coax is the primary target for every product implementation. C++ and Python
currently bootstrap the compiler and schema tests. Existing native C ABI and
platform shims can bridge engine adoption; generated external-language clients
are optional interop outputs for real consumers, not independent product logic.

A profile selects a single canonical schema closure. The external native
bundle and its existing consumers remain pinned until their explicit migration;
this checkout does not pretend those runtimes were restored. Future bundle
formats need hash manifests and old/new consumer checks before replacing one.

### 13.4 Where code lives

- `proto/openagents/verse/` owns source schemas. Current alpha common/presence
  inputs do not claim the entire catalog has been implemented.
- `proto/descriptors/` contains current selected-profile descriptors.
- `proto/compatibility/` contains immutable legacy descriptor/catalog evidence.
- `proto/tools/` contains standalone reference checks, not product admission.
- `coax/` owns the compiler, future ingestion/codec generation and its tests.
- Runtime fixtures/adapters must accompany each actual Coax consumer. No new
  `verse-proto` Rust/npm product workspace is mandated by this plan.

## 14. Publishing the protocol

The goal is for anyone to consume the protocol without asking us.

1. **Source.** `proto/` in `OpenAgentsInc/openagents` under Apache-2.0,
   the repository's license. `proto/third_party/` keeps each upstream's
   license beside its files.
2. **Docs site.** Generated reference docs for every package, published at
   `openagents.com/protocol/verse/<package>`. Each message page links the
   prose section of this spec, its status, its limits and its fixtures.
   They are built from descriptors (`protoc-gen-doc` or Buf's docs), never
   hand-written.
3. **Buf Schema Registry (owner decision pending).** A possible target is `buf.build/openagents/openagents`; publish only reviewed modules/profiles
   after the §15 gate, if that hosting option is selected. The BSR gives outsiders generated SDKs for Go, TypeScript,
   Swift, Kotlin and Python without a toolchain.
4. **Packages.**
   - selected descriptors and Coax consumer artifacts after real ingestion;
   - generated foreign clients only for tested external consumers;
   - a hash-pinned release bundle with source/version/compatibility manifests.
5. **Descriptor sets.** Every release attaches the full
   `FileDescriptorSet` and the frozen compatibility snapshots, so a
   consumer can check compatibility offline.
6. **Extension number registry.** Register the `openagents.options.v1`
   option range in the protobuf global extension registry, so that our
   custom options never collide with another organization's.
7. **NIP cross-links.** NIP-MV gets a "Typed body" section pointing at
   `openagents.verse.presence.v1` and the mapping table. Other OpenAgents
   NIPs get the same treatment as their subtrees land.
8. **Versioned announcements.** Each package reaching `v1` gets a
   changelog entry in `proto/CHANGELOG.md` and a Nostr long-form note from
   the OpenAgents key, with the descriptor digest.

## 15. Conformance and verification

A package moves from `v1alpha1` to `v1` only when the applicable master §35 checks and the following consumer checks
hold. That move freezes its descriptor.

1. **Fixtures:** positive and negative cases, at-limits, over-limits,
   NaN/Inf, invalid UTF-8, NUL, truncated input and unknown fields per
   class, all in `fixtures/<package>/cases.json`.
2. **Cross-language round trip:** Rust, C++ (native and Wasm) and
   TypeScript (plus Python where a tool consumes it) decode every fixture
   to the same verdict, and preserve exact original fixtures and existing identity rules. A semantic
   round trip is not an assertion that standard proto encodings are canonical.
3. **Mapping tests** where the package mirrors JSON (NIP-MV, JCS,
   Metareal engine routes): every NIP example and every stored production
   record sample maps both ways losslessly, or fails with a named reason.
4. **A real consumer:** at least one shipped surface reads and writes the
   package in production, alongside the legacy reader where one exists.
5. **A limits audit:** every bounded field has a limit, and each limit
   matches the current enforced value (§10) or has a recorded reason for
   the change.
6. **Invariants:** if the package touches an invariant-bearing surface,
   the relevant `INVARIANTS.md` is updated in the same change with a
   test. Candidates include Metareal INV-01 to INV-13, the Kiki pin and
   Metareal engine's "Registered world saves preserve schema and iteration
   order".
7. **A formal model where it fits.** Shared-body stamp ordering, realm
   command fencing, channel CAS and transfer prepare/commit are bounded
   state machines. Each gets a small model (TLA+ or a Rust model checker
   such as `stateright`) with its counterexamples turned into regression
   fixtures, following the workspace's formal-verification guidance.

## 16. The build plan

Follow the master spec P0–P8 prerequisites and consumer profiles. Existing
engine/common/HOST/REACH contracts retain v1. New Verse representations begin
as alpha and reach v1 only after §15, even when the source JSON protocol is
stable. Land a package subset only when its imported descriptors and checked
adapters are ready; do not create placeholders to satisfy a phase label.

| Phase | Packages and prerequisites | Exit evidence |
| --- | --- | --- |
| **V0 — Baseline and selected tooling** | Master P0/P1: current engine/common/HOST/REACH baselines, selected-profile generator compatibility, proposed Verse common primitives and a selected Nostr envelope. | Existing outputs and bundle pins remain valid. Native Metal, C ABI, registered saves, and frozen Kiki behavior unchanged. |
| **V1 — Presence consumer** | Verse presence plus only the Nostr/external mirrors its existing reader needs, after master P2 foundations. | Actual NIP-MV fixtures and bounded captured traffic round-trip through existing validation; a real Grid/mobile consumer works. Alpha until the gate; no automatic v1. |
| **V2 — Document prerequisites and authoring** | Presentation primitives and NPC placement/card/line types first, then doc→authoring. Shared ArtStyle/ObjectLight ownership follows §9.2. | Stored house/planet/sandbox JSON and deltas convert losslessly with explicit old readers. Identity rules unchanged; frozen Kiki goldens remain valid. |
| **V3 — Assets, worlds and zones** | Presentation→asset→world after their actual types exist. | Pinned pack/catalog/ZoneManifest admission, old asset profiles and loading limits preserved. No claim that the scene profile is implemented until the live proof passes. |
| **V4 — Publish, galaxy and progress views** | Doc/authoring/world prerequisites from V2/V3; basic character/progress views where consumed. | Existing publishing, galaxy, planet setup, old JSON routes, and playtest flow work with checked adapters. No mandatory Connect deployment. |
| **V5 — Simulation prerequisites** | World→sim→rules/environment primitives, doc→character, and quest state needed by realm. | Pinned execution profiles, current checkpoint/replay identities, and original readers preserved; native layouts stay native. |
| **V6 — Realm wire** | V3/V5 prerequisites plus common grant references; supported HOST world authority remains the existing owner. | Explicit proposed wire 35 and dual-reader/bootstrap tests with JSON v34 peers; command fencing, actual replication fields, replay and soak evidence before cutover. |
| **V7 — Remaining NPC/place and host bridges** | V2 placement types can expand into full NPC/place/quest views; full world host bridge after authoring/publish/character/NPC/economy prerequisites. | Deterministic town behavior and labeled dialogue; versioned bridge transition with old SDKs and native wallet reviews preserved. |
| **V8 — Telemetry** | Selected event/survey records and existing consent/admission paths. | CSV/JSON export parity and automation marking. Core telemetry and Verse telemetry share mapping rules instead of competing definitions. |
| **V9 — Studio, Gym, terminal and media views** | Actual presence/social/host prerequisites and references to existing product owners. | Real studio/tool views and in-world reads; no host or payment authority inferred from a rendered view. |
| **V10 — Games, space, labs and remaining environment** | Each game's actual doc/rules/sim/quest prerequisites. | Old saves/replays still read; per-profile run receipts and resets verified. No new language or GPU-layout serialization. |
| **V11 — Full releases and channels** | Existing signed JCS release/channel models and V2/V4 authoring/publishing. | Exact original signature verification, CAS/fork/rollback fixtures, mixed old/new consumers. |
| **V12 — Consumer publication** | Each selected slice can publish after its gate; hosting follows the master's owner decision. | An independent sample client consumes published artifacts, joins, chats, and publishes through actual supported authority paths. |

Presence can proceed beside document work after shared foundations. Documents
need presentation/NPC primitives first; realm needs world/asset, simulation,
character, quest, and environment primitives first. These dependencies are
explicit in the sequence rather than deferred to later phases. Publication
and unused mirror coverage do not block one another. No phase introduces a
generic canonical encoder or rewrites checked domain logic as generated DTOs.

## 17. Risks

| Risk | Mitigation |
| --- | --- |
| Proto mirrors drift from signed JSON (NIP-MV, JCS) | Mapping table is generated code with fixtures; signatures always verified against original bytes |
| Too many packages too early; churn | All new Verse mirrors start as alpha; shipped core packages retain v1; add packages for actual consumers and pass the gate before freezing |
| Nanopb memory on Wasm and phones | Limits from options; `FT_CALLBACK` for large repeated fields, as `CompiledScene` already does |
| Unknown-field loss on edit | Class rule: durable readers refuse; no reader silently rewrites a newer record |
| Kiki frozen flow broken by migration | Kiki never imports new packages; its server routes keep their JSON adapters; goldens gate every server change |
| Outsiders read "schema exists" as "feature works" | Every package and message carries the glossary's status label in its docs page; NIP-MV keeps its Designed/Implemented markers |
| Proto JSON mistaken for canonical | §7.3/§7.4 rules; existing JCS identities and stored-byte digests remain unchanged |
| Private data leaks through public schemas | Recursive carriage/redaction checks and private-asset rules; credentials stay in their owning authenticated flows, never Verse views |
| Two engines (Rust Metareal, C++ Metareal engine) diverge | Both consume the same export bundle and the same fixtures; cross-language tests in CI |

## 18. Planned scope and constraints

1. The 3D namespace is `openagents.verse`, with one package per domain,
   each `.vN`, under `proto/openagents/verse/<domain>/vN/`. Games are
   `openagents.verse.game.<game>.vN`.
2. `openagents.engine.v1` remains the engine layer. Verse depends on it,
   and it never depends on Verse.
3. Shared primitives across all OpenAgents subtrees go in
   `openagents.common.v1`, and descriptor options in `openagents.options.v1`.
4. Nostr mirrors live under `proto/nostr/` (`nostr.*` packages).
   Lightning Labs protos are vendored byte-exact under
   `proto/third_party/lightninglabs/` with their own package names.
5. Every top-level message declares a data class, and readers follow the
   class's unknown-field rule.
6. Limits are authored as descriptor options and generated into
   `.options` and validators.
7. Signed and content-addressed bytes keep their exact form. Proto
   mirrors or carries them, and never replaces them.
8. Payment views retain existing msat strings or permitted payment references.
   Accounting units retain their source meaning; Verse never owns settlement.
9. Published packages keep their versions. New Verse mirrors start as alpha
   and graduate only after field parity, a real consumer, and conformance.
10. Existing transports keep their wire encodings. gRPC/Connect are optional
    new bindings; realm proto needs an explicit negotiated version and old reader.
11. Owned protocol packages target Apache-2.0 publication, with generated
    docs, selected language packages and descriptor sets. BSR hosting is
    an open owner decision, not a prerequisite; retain upstream licenses.

## 19. Open questions for the owner

These change the plan if answered differently. The defaults above stand
until answered.

1. **BSR.** Is publishing on the Buf Schema Registry acceptable, with the
   GitHub release bundle as the always-available fallback? Or should it
   be self-hosted only?
2. **Package scoping.** Keep the `openagents.` prefix for packages, or
   register a shorter public prefix such as `oa.`? This document keeps
   `openagents.`, because `openagents.engine.v1` is already published.
3. **Account and sign-in schemas.** Do these belong under
   `openagents.agent` or under a separate `openagents.account`? This
   affects §11's account routes only.

## 20. Sources

**Engine schemas and tooling.**

- `proto/README.md` and `proto/openagents/engine/v1/*.proto`
- `crates/engine-proto/README.md`

**Glossary and NIPs.**

- `docs/glossary.md`, in particular Verse, Voyager, and Nostr and shared
  protocols
- `nips/README.md`, `nips/manifest.json`
- `nips/openagents/README.md` (kind registry) and
  `nips/openagents/contracts.md`
- `nips/openagents/NIP-MV.md`, NIP-XP, NIP-EVAL, NIP-PYLON, NIP-REACH,
  NIP-HOST, NIP-AGENT, NIP-LIVE, NIP-TERM

**Verse documentation.** The Verse README, status and runtime contract:

- `docs/verse/README.md`, `status.md`, `runtime-contract.json`

Worlds, zones and networking:

- `zones.md`, `zone-rules.md`, `networking.md`, `chat.md`, `mobile.md`,
  `platform-clients.md`

Agents and identity:

- `gym.md`, `agent-studio.md`, `workshop-agent.md`, `generative-agents.md`,
  `agent-identity-and-engrams.md`, `agent-trainer-leveling.md`

Simulation, environment and combat:

- `combat-model.md`, `destructible-buildings.md`, `water.md`, `coast.md`,
  `particles.md`

Zones and labs:

- `lagrange-1.md`, `lagrange-2.md`, `physics-lab.md`

Assets, interactions and hosting:

- `private-assets.md`, `in-world-terminal.md`, `world-interactions.md`,
  `mobile-games.md`, `metareal-host-protocol.md`

Games and the engine:

- `games/`
- `engine/architecture.md`, `engine/roadmap.md`

**Metareal engine documentation.**

- `docs/metareal/engine/` (README, architecture, contracts and
  `contracts/*.schema.json`, world-model, runtime, runtime-abi,
  behavior-bytecode, cell-streaming, collaboration, publishing,
  asset-compilation, game-systems, security, INVARIANTS)
- `docs/metareal/ui-protocol.md`, `docs/metareal/merge.md`

**Metareal engine.**

- README.md, INVARIANTS.md
- `docs/save.md`, `docs/worlds.md`, `docs/triggers.md`, `docs/ai.md`,
  `docs/time-and-quests.md`, `docs/realms.md`
- `tools/serve.py` and its helpers
- `web/haunt-world.js`, `web/analytics.js`, `web/survey.js`
- `src/save/`, `src/proto/`, `src/ecs/state_wire.cpp`
- `third_party/openagents-engine-proto/manifest.json`
- Historical engine epic #113

**Lightning reference.**

- `crates/lnd-sdk/proto/` and
  `docs/payments/2026-10-15-lnd-sdk-plan.md`
