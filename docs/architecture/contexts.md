# Bounded contexts and context map

This page names the repository's bounded contexts, the folder each owns, its
classification (core, supporting or generic), what each may depend on, and the
relationship kind on every edge between them. Derived from section 2 of the
[domain-driven design audit](../audits/2026-11-11-domain-driven-design-audit.md)
(tracker #13311, under #13257).

For shared domain words and their owning contexts, see the
[glossary](../books/glossary.md). When `ARCHITECTURE.md` lands (#13273), it
links here.

## Bounded contexts

Every context is classified as **Core** (why someone uses the product),
**Supporting** (needed for our product, not sold on its own) or **Generic** (a
solved problem or external standard). Each row names both where the code lives
today and the folder it owns as the layout settles (#13270, DDD.6).

| Context | Folder owned (target) | Where it lives today | Class | What it may depend on | Notes |
| --- | --- | --- | --- | --- | --- |
| **Combat rules (SRD)** | `rules/srd/` (today `aethyr/rules/srd/`) | `aethyr/rules/srd/` (27,889 lines of `.coax`) | Core | Engine core (`aethyr/` value types and math), `coax/lib/` | Everglade's pitch is "the SRD's rules decide the outcome" (`everglade/README.md`). Shared across Everglade II, Experimental and 5E Lab; language tied to SRD 5.2.1 ([compliance ledger](../dnd/srd-compliance-ledger.md)). |
| **Characters and progression** | `everglade/model/` | Scattered: room seat state, `aethyr/session/character-checkpoint.coax`, `coax/web/room/room-checkpoint.coax`, `coax/web/room/room-events.coax`, SQL `verse.character_*` | Core | Combat rules, Items (by ID only), Engine, Protocol (`proto/`) | Workstream P4 ("a character worth keeping", #13290). Rooted at `CharacterId` with one `#authority` owner of level-from-XP and slot limits. |
| **World and zones** | `everglade/world/` (plus `everglade/server/`, `everglade/client/`) | `verse/everglade2/`, `everglade/world/`, `aethyr/world/` | Core | Engine, Realm authoring (`aethyr/forge/`), Room session, `components/` | The place the player is in (#13280). |
| **Creatures and encounters** | `everglade/model/` and `everglade/server/` | `coax/web/room/room-e2-monsters.coax`, `coax/web/room/room-e2-guard.coax`, `coax/web/room/room-events.coax`, `aethyr/actors/` | Core | Combat rules, World and zones, Engine | Creature templates and encounter logic (#13293). |
| **Items and economy** | `everglade/model/` and `aethyr/session/ledger.coax` | `aethyr/session/ledger.coax`, SQL `verse.items`, `verse.item_*` | Core (dormant) | Characters (by `CharacterId` only), Protocol (`proto/`), `coax/lib/` | Built in Coax and SQL; dormant outside tests until wired to gameplay (#13291, DDD.7). Items are never money. |
| **Quests** | `everglade/model/` | Page code and browser save today | Core (missing) | Characters, World and zones, Items, Combat rules | Server-owned quest state planned in #13292. |
| **Room session and replication** | `aethyr/session/`, `aethyr/net/`, `coax/web/room/` | `coax/web/room/`, `aethyr/net/`, `aethyr/session/` (control, travel, party, region, compat) | Supporting | Engine, Protocol (`proto/`); Everglade rooms additionally depend on Combat rules, Characters, World, Creatures, Items, Identity (seat ticket ACL) | Authoritative simulation hosting, seat admission, checkpoints and travel sagas. |
| **Realm authoring** | `aethyr/forge/` (adapters in `apps/aethyrforge-desktop/`, `apps/openagents-web/worldbuilder*.coax`, `coax/web/api/worldbuilder.c`, `coax/web/api/realms.c`) | Same, plus SQL `verse.realm_*`, `worldbuilder.*` | Supporting | Engine, Protocol (`proto/`), `components/` | WorldBuilder edits through `aethyr/forge/edit.coax` (#13226); `aethyr/forge/doc.coax` owns the realm document format (#13249). |
| **Identity and auth** | `coax/web/api/auth*` (`auth*.c`, `auth-*-policy.coax`) | `coax/web/api/auth*.c`, `coax/web/api/auth-*-policy.coax`, SQL `auth.*` and legacy `signin.*`, `docs/auth/` | Generic (built in-house) | `coax/lib/` (crypto, web), Nostr (NIP-42/key auth) | Accounts, authenticators and sessions (`0142_auth_accounts.sql`), built in-house with no phone or third-party identity provider. |
| **Payments and wallet** | `pay/` (or `coax/lib/pay/`, consolidating `verse/wallet.coax` and pay policies; DDD.9) | `verse/wallet.coax`, `coax/web/api/lnaddr*.c`, `coax/web/api/makerpay-policy.coax`, SQL `pay.*`, `makers.*`, `coax/lib/bitcoin/`, `coax/lib/lightning/`, `coax/lib/lnaddr/` | Supporting | `coax/lib/bitcoin/`, `coax/lib/lightning/`, `coax/lib/lnaddr/`, Nostr, Identity | Bitcoin-only rails ([`docs/lightning/`](../lightning/README.md)); money policy files are `#authority`. |
| **Coder and agents** | `coder/`, `coax/lib/agent/`, `coax/lib/microcoder/`, `coax/lib/gateway/` | Same, plus `apps/everglade-desktop/` (Golemancy view) | Core | `coax/lib/gateway/`, Psionic (via ACL), Nostr, Protocol (`proto/`), `coax/lib/` | Coding agent harness, session logs (`openagents.session.v1`), Router and Workers (`coder/book/`). |
| **Chat** | `components/zeron/`, `apps/openagents-web/chat.coax`, `coax/web/api/chat.c` | Same, plus SQL `chat.*` | Supporting | Coder, Identity, Protocol (`proto/`), `components/` | Web and desktop chat surfaces over agent and user threads. |
| **Projects** | `coax/web/api/` (`private.c`, `drops.c`) | `coax/web/api/private.c`, `coax/web/api/drops.c`, SQL `projects.*`, `drops.*` | Supporting | Identity, Payments, `coax/lib/` | Private project pages and drops. |
| **Nostr** | `nostr/`, `coax/lib/nostr/` | `coax/lib/nostr/`, `nostr/` | Generic | `coax/lib/` (crypto, net, TLS) | External protocol implementation and OpenAgents NIPs (`nostr/nips/`). |
| **Psionic and inference** | `psionic/`, `coax/lib/systemone/`, `coax/web/api/inference.c` | Same, plus SQL `inference.*`, `mesh.*` | Core (separate product) | `coax/lib/`, Nostr (Pylon announcements), Protocol (`proto/`) | Tensors, decision models, `/v1/systemone`, Pylon and Mesh serving (`psionic/`). |
| **Engine (Aethyr)** | `aethyr/` (minus `aethyr/rules/`) | `aethyr/` minus `aethyr/rules/` | Supporting kernel | `coax/lib/` only (strictly downward; zero loads into `verse/`, `everglade/`, `apps/` or `coax/web/` once #13270 burns down `aethyr/layer-allowlist.txt`) | Frame loop, ECS world, rendering, physics and deterministic replay shared by every realm. |
| **Protocol (published language)** | `proto/` | `proto/` (`proto/registry.json`, `.proto` schemas, wire profiles) | Supporting (published language) | None (schemas and catalogue; codecs live in their owning contexts, DDD.5) | Canonical protobuf schemas and wire shape registry ([`proto/README.md`](../../proto/README.md)), gated by `coax/web/tool.sh wire-check`. |
| **Coax toolchain** | `coax/` | `coax/` (`coax/bin/`, `coax/lib/`, `coax/web/`) | Supporting | Host C11 toolchain | Compiler, standard libraries, mutation tester, compat checker and Coaxial web runtime. |
| **Analytics, email, jobs, access** | `coax/web/api/` (`analytics.c`, `email.c`, `jobs.c`, `access.c`) | Same, plus SQL `analytics.*`, `email.*`, `jobs.*`, `access.*` | Generic | `coax/lib/`, Identity (where route access applies) | Operational plumbing routes and background queues. |

Three contexts are the product's core for the game and agent lines today:
**Combat rules**, **Characters and progression**, and **Coder** (with
**Psionic** as a separate core product line). **Engine (Aethyr)** is a
supporting shared kernel, not the game's domain model.

## Context map

Arrows point from downstream to upstream (`DOWNSTREAM --> UPSTREAM`), except
where labeled to show a leak being removed.

```mermaid
flowchart LR
  subgraph Everglade[Everglade game]
    RULES[Combat rules SRD]
    CHAR[Characters and progression]
    WORLD[World and zones]
    CREAT[Creatures and encounters]
    ITEMS[Items ledger]
    QUEST[Quests]
  end
  ROOM[Room session and replication]
  ENGINE[Aethyr engine]
  AUTHOR[Realm authoring]
  AUTH[Identity and auth]
  PAY[Payments and wallet]
  CODER[Coder and agents]
  GOLEM[Golemancy desktop view]
  CHAT[Chat]
  NOSTR[Nostr]
  PSI[Psionic and inference]
  PROTO[(proto registry: published language)]

  ROOM -- conformist today; target: customer/supplier --> RULES
  ROOM -- owns today; target: customer/supplier --> CHAR
  CHAR -- customer/supplier --> RULES
  CREAT -- customer/supplier --> RULES
  QUEST -- customer/supplier --> CHAR
  ITEMS -- id only --> CHAR
  ROOM -- ACL: seat ticket --> AUTH
  AUTH -- ACL: verse_account --> CHAR
  AUTHOR -- published language: realm doc --> WORLD
  ENGINE -- shared kernel --> ROOM
  ENGINE -. upward loads, 57 allowlisted .-> WORLD
  PAY -- open host: LNURL / zaps --> NOSTR
  CODER -- ACL: external rows --> PSI
  GOLEM -- ACL: golems over transcripts --> CODER
  CHAT -- conformist --> CODER
  CODER -- published language --> PROTO
  ROOM -- published language --> PROTO
```

## Relationships by edge

DDD relationship kinds used here:

- **Shared kernel:** two contexts compile the same small, checked module and
  change it together; dependencies point strictly downward into the kernel.
- **Customer / supplier:** downstream depends on an explicit `#export` surface
  owned by upstream; upstream evolves its internals freely behind that surface.
- **Anticorruption layer (ACL):** one boundary module translates an external,
  legacy or foreign-context representation into the local context's own types.
- **Published language ( / Open host service):** contexts share a versioned,
  registered schema (`proto/registry.json`, `coax compat` or realm document
  version) rather than internal structs.
- **Conformist:** downstream uses upstream's model as-is with no translation
  layer because both belong to the same product line and upstream's model fits.

| Edge (between contexts) | Relationship today | Kind it should be | Notes and tracking |
| --- | --- | --- | --- |
| **Combat rules -> Room session** | Conformist / unsealed: `coax/web/room/room-everglade.coax` calls `rs_*` and reads `RsCreature` fields directly (`v[0].eg[k].rules[0].b[0].c[...]`) | **Customer / supplier** (with **shared kernel** execution in client and room) | Expose a sealed `#module` / `#export` surface and `world-profile.coax` so #13285 can rewrite `RsCreature` into `Ability` and `Aura` records without breaking room callers. |
| **Combat rules -> Characters** | No standalone character context yet; the room holds both creature state and checkpoint arrays | **Customer / supplier** | `everglade/model/` (#13290) consumes the rules' level-from-XP, class and slot tables from one `#authority` source instead of duplicating XP tables. |
| **Combat rules -> Creatures and encounters** | Monster and guard scripts in `coax/web/room/room-e2-*.coax` construct `RsCreature` slots by index | **Customer / supplier** | Creature templates reference content IDs (#13286, #13293) and call the sealed rules surface. |
| **Characters -> Room session** | Room owns live fields and emits positional `n[17]` checkpoint captures (`aethyr/session/character-checkpoint.coax`) | **Customer / supplier** (via `#persisted` aggregate at seat admission, checkpoint and travel) | Room steps columns during the tick (#13274); capture and restore in `everglade/model/` (#13290) own the persistent `Character` record. |
| **Characters <- Items and economy** | `aethyr/session/ledger.coax` and `verse.items` key inventory by player/character ID string; dormant in rooms | **ID-only reference + published domain events** | Items reference `CharacterId` by nominal ID only and publish `items.item.granted.v1` through an outbox (#13291, DDD.8). |
| **Characters <- Quests** | Quests live in page state and browser storage only | **Customer / supplier** | Server-owned quests (#13292) reference `CharacterId` and content IDs (#13286). |
| **Identity and auth -> Game (Characters)** | `auth.accounts.verse_account` maps login accounts to `verse.accounts.id` (`0142_auth_accounts.sql`) | **Anticorruption layer (ACL)** (keep) | Keeps login authenticators and sessions (`auth.*`) decoupled from game accounts (`verse.*`). |
| **Identity and auth -> Room session** | Seat ticket proves seat identity, while the client hello still supplies class and level | **Anticorruption layer (ACL)** | The signed seat ticket (#13248, #13259) carries identity plus the character context's class and level; the room never trusts the hello for rules facts. |
| **Engine (Aethyr) -> World, Room and apps** | Shared code, but with 57 `aethyr-up` upward loads in `aethyr/layer-allowlist.txt` into `apps/openagents-web/` and `verse/` | **Shared kernel** (strictly downward) | `coax/web/tool.sh layer-check` (#13264) blocks new upward loads; #13270 removes the remaining allowlisted ones. |
| **Realm authoring -> World and zones** | Versioned realm document read by `aethyr/forge/doc.coax` and written through `aethyr/forge/edit.coax` (`realms.c` still holds separate SQL rules) | **Published language** | The versioned realm document (#13249) is the contract between authoring tools and world loaders. |
| **Payments and wallet -> Nostr and callers** | Wallet logic in `verse/wallet.coax`, maker pay in `coax/web/api/makerpay-policy.coax`, LNURL in `coax/lib/lnaddr/` | **Open host service + published language** | Consolidate into one payments context folder (DDD.9) exposing an `#authority` policy API internally and LNURL / Nostr zaps externally. |
| **Coder <- Psionic and third-party agents** | `proto/registry.json` holds 12 `external` rows (Claude Code, Codex, Cursor, ACP, MCP, LNURL, Nostr NIPs) alongside `coax/lib/gateway/` and `psionic.*` | **Anticorruption layer (ACL)** (keep and name) | Each `external` row's codec folder is the sole parser for that third-party format and outputs our own record types. |
| **Coder -> Chat** | `apps/openagents-web/chat.coax` and `components/zeron/` render Coder threads and turns directly | **Conformist** | Same product family; Chat conforms to Coder's thread and turn model rather than translating it. |
| **Coder -> Golemancy (`apps/everglade-desktop/`)** | `apps/everglade-desktop/golems.coax` reads Coder session transcripts and presents agents as golems | **Anticorruption layer (ACL)** | Game terminology ("golem") over agent session records (`openagents.session.v1`) is an explicit view/translation layer. |
| **Coder, Room session, Psionic -> Protocol (`proto/`)** | 98 rows in `proto/registry.json` checked by `coax/web/tool.sh wire-check`, with allowlist burn-down in progress | **Published language** | Every wire shape, persisted save and cross-context domain event has one registry row, an `owner` context (DDD.5) and one codec file. |

## Dependency direction rules

1. **Kernel and libraries at the bottom.** `coax/lib/` depends on no product
   context. `aethyr/` (minus `rules/`) depends only on `coax/lib/`. `proto/`
   holds schemas and metadata only.
2. **Rules depend on no room, page or database.** `rules/srd/` declares pure
   procedures (`#effects(reads, writes, traps)` or narrower, DDD.3) and loads
   nothing from `everglade/`, `coax/web/` or `apps/`.
3. **Game models sit above rules and below rooms/pages.** `everglade/` may
   depend on `rules/srd/`, `aethyr/`, `aethyr/session/`, `aethyr/forge/`,
   `proto/` and `components/`, never on `apps/` (`everglade-up` in
   `layer-check`).
4. **Cross-context communication uses sealed exports, nominal IDs or registered
   shapes.** Contexts do not reach into another context's internal structs, SQL
   tables or `tests/` fixtures (DDD.4, DDD.6).

## Open decisions

The audit leaves three context-boundary questions to decide explicitly as the
AAA layout (#13257) lands:

1. **Are SRD rules their own root context at `rules/srd/`, or owned by
   Everglade?**
   - *Audit recommendation:* a separate root context at `rules/srd/` (#13270,
     D7), because Experimental and 5E Lab also use the SRD engine and the
     rules are not part of the rendering/simulation engine (`aethyr/`).
   - *Alternative:* move SRD rules under `everglade/` once non-Everglade
     consumers leave the build (#13269), and keep the Everglade book's current
     statement without moving the root folder.
2. **Is Golemancy its own bounded context or a view (ACL) over Coder?**
   - *Audit recommendation:* a view and **anticorruption layer** over Coder in
     `apps/everglade-desktop/golems.coax`, translating `openagents.session.v1`
     transcripts and agent tasks into golem words without introducing a second
     agent state store.
   - *Alternative:* promote Golemancy to a separate game context if golems
     acquire durable game-world state independent of Coder transcripts.
3. **What does "zone" mean?**
   - Today "zone" is used four ways in the codebase and audits: (a) a named
     area of a realm's world (`docs/books/glossary.md`), (b) a separately
     compiled Wasm page (`EfZone` in `everglade/features.coax`), (c) a server
     simulation process in the AAA audit ("150 players in one zone process"),
     and (d) a cloud availability zone.
   - *Audit recommendation:* reserve **zone** for (a) a named area of one
     realm's world loaded as cells; call (b) a **page**, (c) a **room** (or
     **shard** for the process hosting rooms), and qualify (d) as **cloud
     zone** / **GCP zone**.

## Tracking notes

- **#13273 (`ARCHITECTURE.md`):** when root `ARCHITECTURE.md` lands, link
  `docs/architecture/contexts.md` from its architecture map section (this page
  does not block on #13273, and `ARCHITECTURE.md` is not created here to avoid
  conflicting with #13273).
- **DDD.1 (`docs/books/glossary.md`):** links to this page.
- **DDD.5 and DDD.6:** enforce context ownership in `proto/registry.json`
  (`wire-check`) and per-context `#load` rules (`contexts.txt` in
  `layer-check`).
