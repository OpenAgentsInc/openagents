# Valenfeld

Date: 2026-11-11. Sources: the author's PDF "Valenfeld: give agents a world
they can understand" (a snapshot from 2026-11-10 near commit `77e9d91cc`), his
World DB write-up, his post looking for a cofounder, and his public GitHub
([rondorkerin](https://github.com/rondorkerin)).

**Goal:** Valenfeld is playable in the OpenAgents Metaverse: reachable from the Grid, using
Metaverse identity and characters, and linked to other worlds by gates. It comes
in whatever engine it already runs on. Moving to the OpenAgents Metaverse Engine is an option for
later, never a condition.

## What Valenfeld is

Valenfeld is an open-world fantasy RPG in the Elder Scrolls mold. Agents
author it as data, and a person sets the direction. The author calls it a
testbed for "generative world authoring": the hard part of making AI games
bigger than demos. He's content-first and is looking for a partner who cares
about graphics and game feel.

It has two authoring systems over one existing game engine.

### 1. World DB: what the world means

Typed YAML records, one file per entity (`content/npc/<id>.yml`,
`content/faction/<id>.yml`, `content/quest/<id>.yml`). Every entity has a
stable ID, and other records refer to that ID instead of keeping their own
copy. Each record holds structured facts that software checks, and prose that
carries motive, history and voice.

- **Scale (Nov 10):** 62 record types. These include 734 NPCs, 1,191 quests,
  47 settlements, 143 sites, 1,724 items, 336 lore entries, 320 dialogue
  topics, 285 rumours, 235 encounters, 40 factions, 11 Houses, 78
  relationships and 68 presence records. About a dozen newer types exist as
  schema only. The map is about 9.8 km across.
- **People and offices are separate.** An `institution-office` (duties,
  jurisdiction, exclusive) is distinct from the `appointment` that names who
  holds it. A steward can change while the office, its duties and old
  references stay correct.
- **Truth is separate from knowledge.** A `claim` has a statement, an
  authority (established, attributed or unresolved) and a visibility. A
  `knowledge` record says who believes, doubts or has heard it, on what
  basis, and when they'll say so. NPCs aren't omniscient, and books that
  disagree don't break the world.
- **Power has more than one dimension.** Claimed ownership, administration,
  garrisons and trade control are separate records (`presence`,
  `relationship`), not one "owner" field.
- **One situation feeds many kinds of content.** A `conflict` (participants,
  stakes, competing claims) supports a guild contract, a merchant's side of
  the story, a witness, a ledger, ambient talk and service access, all
  pointing at the same records.
- **Checks:** typed references, required fields, uniqueness, exclusive
  offices, acyclic history, overlapping intervals, source pointers, component
  property contracts and golden render rows. Quest checks use the engine's
  actual semantics, for example a required quest that can't yet start. The
  checker names the record and the missing target, so an agent can fix it.
- **Status and sources:** each record is draft, review or canon, and facts
  carry where they came from.
- **Compiled, not mirrored.** YAML compiles into the engine's runtime tables,
  which are outputs, never edited. Stable IDs protect save games: one check
  kept 28 runtime tables and 6,607 existing ID mappings intact.
- **Agents work on part of the world.** Module manifests, source packets and
  dependency queries give an agent the slice it needs (a city, its cast, its
  politics) instead of the whole world in context.

### 2. JSX world components: how places take shape

React-style components build physical places, for example:

```jsx
<Fort id="fort-greywatch" name="Fort Greywatch" at={[-199, 167]} seed={1} />
```

- `Fort` expands to `Frame`, `Tokens`/`Style`, `OuterWard`, `InnerWard`,
  `HallRow`, `DrillCourt` and `SiteMarker`. It takes size, wall height,
  storeys, keep, barracks, seed, style and children, and has standing and
  ruined states.
- **There are 146 components:** frames (`Frame`, `Offset`, `Ground`),
  relationship layout (`Row`, `Grid`, `Ring`, `Corners`, `Sides`, `Mirror`,
  `Along`), camp props, stronghold parts, 108 town parts and districts, and
  fort and expedition compositions.
- **A separate dungeon library** (`Plan`, `Level`, `Space`, `Flex`, crypts,
  caves, mines) has 13 `.ddl.jsx` dungeon programs. The Barrow encodes the
  adventure itself: three tiers, a goal you see before you can reach it, an
  urn puzzle that opens a door, and a robbers' shortcut that opens only from
  below.
- **The shared contract:** a stable ID links a place's lore record, its
  component instance and its game state.

### The engine

The PDF says only "existing game engine" and never names it. Evidence from
his GitHub:

- [`gamestack`](https://github.com/rondorkerin/gamestack) (34 stars, active
  November 2026) is his Claude Code skill pack for game design. It has a
  worked Godot example of headless, testable architecture: input becomes a
  verb on a seam, every system has a bench, `--fixed-fps`, and a playtest API.
- [`valenfeld-noray`](https://github.com/rondorkerin/valenfeld-noray) is a
  fork of foxssake's noray, the NAT punch-through and UDP relay used with
  Godot's networking, "for Valenfeld co-op" on Fly.io.
- [`morrowindlike`](https://github.com/rondorkerin/morrowindlike) (TypeScript,
  February 2026) is an earlier Morrowind-like experiment.
- [`pixelforge`](https://github.com/rondorkerin/pixelforge) is a YAML-defined
  asset generation pipeline with Godot TileSet export.

So Valenfeld is most likely a Godot game with peer-to-peer co-op. Confirm
this with him. Record types such as `birthsign`, `barter` and `topic` follow
Morrowind's data model on purpose.

### The author

Nicholas Steele, [`rondorkerin`](https://github.com/rondorkerin), of
Metatransformer (metatransformer.com, `@metatransformr` on X). He has 56
public repos, including agent tooling (`openclaw-cluster`, `corellm`),
genetic-algorithm and robotics work, and gamestack.

## How it comes into the Metaverse

Three layers. Each one works without the next.

### Layer 1: Valenfeld as a Metaverse realm, in its own engine

The path is guest content, as designed for Unreal in
[web-games-in-the-verse.md](../unreal/web-games-in-the-verse.md). Godot fits
it better than Unreal:

- **Web build.** Godot 4 exports to the web (WebAssembly, the Compatibility
  renderer on WebGL2). Since 4.3 it can export without threads, so it needs no
  cross-origin isolation headers. A threaded build needs them, and the guest
  domain can send them.
- **Hosting.** A static stage in the guest bucket, under
  `g/valenfeld/<version>/` on the separate guest domain, admitted by
  `coax/web/guest/admit.coax`. The world page is `/worlds/valenfeld`.
- **The bridge.** `verse-bridge.js` is the same file for every engine. Godot
  reaches it through its `JavaScriptBridge`, so the Godot side is one small
  addon (`addons/verse/verse.gd`) that wraps `who()`, `character()`,
  `achieve()`, `presence()`, `friends()`, `pay()`, `gate()` and `worlds()`.
  We write and publish that addon.
- **Characters.** The Quaternius humans and Fantasy Source wardrobe are CC0
  glTF, which Godot imports natively. The character pack's mapping from
  `aethyr.player.appearance.v1` to meshes works the same way. His game builds
  your Metaverse character and runs it on its own animations.
- **Co-op.** Browsers can't use noray's raw UDP. Godot's web build has
  `WebSocketMultiplayerPeer` and `WebRTCMultiplayerPeer`, so web co-op needs a
  signalling or relay server. We can host that next to our room servers. His
  native builds keep noray.
- **Native.** Desktop Godot builds stay his. The Everglade apps list the
  world and open its web build in the page host. Later, the launcher can
  start his native build when it's installed.
- **Gates.** A portal on the Grid opens `/worlds/valenfeld`;
  `verse.gate("the-grid")` comes back.
- **Money.** `pay()` opens our Bitcoin pay sheet, if he ever charges. Tips go
  straight to his own Lightning Address
  ([lightning-addresses.md](../auth/lightning-addresses.md)).

**Status on our side:** the guest path is a design, not built. Nothing exists
yet for `coax/web/guest/`, `verse.guest_games`, the guest bucket or the
bridge. Valenfeld is the reason to build it, engine-neutral from the start
instead of Unreal-first.

### Layer 2: World DB in the Metaverse

World DB solves a problem we have. Our NPC chat (`verse.npc_request`,
migration 0005) gives the model only a character's name, role, greeting and
what players taught it, so every answer is improvised. World DB's records are
what that prompt is missing:

- **Grounded NPCs.** An NPC's prompt is built from their own record, their
  appointments and relationships, and only the claims their `knowledge`
  records say they hold, with each record's disclosure condition. Characters
  can't leak what they don't know.
- **Shared schema.** His record types become messages in `proto/` (an
  `openagents.verse.lore.v1alpha1` package: entity, claim, knowledge, office,
  appointment, conflict, presence, relationship). They use the generated Coax
  codec, by the rule in
  [the protocol review](../protocol/2026-11-11-protocol-review.md). His YAML
  stays his source. A converter reads it and checks it against the schema.
- **His checker, our checks.** His typed-reference and world-rule checks map
  onto Coax `#authority` checks over those messages. Our checks don't replace
  his; one runs his records through the other.
- **Every Metaverse realm gets it.** Everglade (the SRD MMO), made realms and
  WorldBuilder could all keep their lore in the same records. A player's
  chat with an Everglade NPC would then work the way his characters do.

### Layer 3: his places in the Metaverse Engine (optional)

The Metaverse Engine builds worlds with typed edit operations (`metaverse-engine/editor/edit.coax`)
and lets you place things by chat (WorldBuilder). It has nothing like
`<Fort>`: a component that expands one intent into a coherent, checked place.
If he wants the Metaverse Engine's renderer, netcode or character pipeline, the path is:

- a compiler from his component tree, after expansion, into Metaverse Engine zone data:
  props, colliders and markers that keep their stable IDs. His JSX stays the
  source;
- a first target of one place, Fort Greywatch or the Barrow, walkable on web,
  Mac and iPhone with a Metaverse character, compared against his own build;
- WorldBuilder reusing the idea: "a royal fort near the town" expands through
  components instead of placing props one by one.

This comes only after layer 1, and only if he wants it.

## What each side gets

| He gets | We get |
| --- | --- |
| Players reach Valenfeld from the Grid and the Metaverse apps | A large, authored RPG in the Metaverse |
| Accounts, characters, friends, presence and Bitcoin payments without building them | A reason to build the guest path, engine-neutral |
| Hosting and a web co-op relay | World DB's model for lore, knowledge and offices, to ground NPCs in every world |
| Our renderer, netcode and character pipeline, if he wants them | A component approach for WorldBuilder |

His world, content and IP stay his. The CC0 character pack needs nothing from
us.

## What we build first

1. The engine-neutral guest path: admission, the bucket, the domain,
   `verse.guest_games`, `verse-bridge.js` and the `/worlds/<slug>` guest page.
   This generalises the Unreal design.
2. A Godot addon for the bridge, plus a sample Godot web build admitted end to
   end as the test.
3. A Godot import guide for the character pack, plus a sample scene.
4. A web co-op relay for Godot's WebSocket or WebRTC peers next to the room
   servers, when he asks for it.
5. The `openagents.verse.lore.v1alpha1` schema and NPC prompts grounded in it,
   tried first on one Everglade NPC.

## Questions for him

- Which engine and version? Is a web export possible today, and how big is
  it?
- Will he publish World DB's schema? Can our converter read his records, and
  under what licence?
- Does co-op need to work in the browser, or only native?
- Which of his places would he like to see in Metaverse Engine first, if any?

## The invite

> Read the Valenfeld PDF and the World DB write-up. This is the hard part
> almost nobody does, and separating claims from knowledge is the sharpest
> idea in it.
>
> We'd like Valenfeld in the Metaverse, in your engine as it is. The Metaverse is the
> set of worlds on openagents.com and our Mac and iPhone apps. Your web build
> gets a world page and a gate from the Grid, and players bring their
> accounts, characters and friends. We host it, and we write the engine
> bridge. Your world and IP stay yours.
>
> If you ever want our engine for rendering, netcode or characters, we'd
> compile your components into it. That's optional. We also want to ground
> our NPCs in records like yours.
>
> Which engine is it on, and does it export to the web yet?
