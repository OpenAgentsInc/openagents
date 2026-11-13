# Everglade and Aethyr against AAA MMORPG practice

Date: 2026-11-11 (Chicago). Code read at `24eab15ff3`.
Tracker: [#13257](https://github.com/OpenAgentsInc/openagents/issues/13257); it lists the issue for each
item below (P0.1 to P6.4, #13258 to #13306, and #13255).

This audit measures Everglade, its engine Aethyr and the services behind them
against how the best MMORPG studios build and run their games. It names what
is already at that standard, what isn't, and the order to fix it in. It
proposes refactors, rewrites, moves and deletions without regard to how much
work already went into the code they replace.

It builds on three earlier documents and doesn't repeat them:

- [MMO architecture gap analysis](../metaverse/2026-11-04-aethyr-verse-mmo-gap-analysis.md)
  (gaps G1 to G10, 2026-11-04)
- [Everglade multiplayer audit](2026-11-10-everglade-multiplayer-audit.md)
  (netcode measured, 2026-11-10)
- [Protocol compliance plan](../protocol/2026-11-11-compliance-plan.md)
  (wire shapes, steps E1 to E6, tracker #13250)

## How this was done

Five reviewers read the code in parallel, one each for: the room server and
netcode; backend services, persistence and operations; the engine; gameplay
systems and content; and the client, hosts, build and process. I then checked
the highest-stakes claims against the source myself. Every claim marked
**checked** below is one I read in the code. The others come from a reviewer's
report with a file and line.

Nothing was built, run or deployed. Performance numbers are quoted from
existing documents, not measured again. No live GCP state was read.

## Verdict

**The parts are better than the whole.** Aethyr's kernels are verified to a
standard most AAA engines don't reach. The netcode for rows is textbook. The
checkpoint and deploy-drain protocols are careful. But the pieces are not
assembled into one game on one engine:

- Everglade is four separately built clients and a server that is a mode flag
  inside the previous game's server.
- Aethyr has an entity store, a frame graph, a streaming world, a broadphase
  and an actor runtime. Everglade uses none of them.
- The live combat profile is deliberately "harmless and free": one target per
  cast, unlimited resources, no items.
- What we call an MMO is, today, 16-player instances on one virtual machine.

An AAA studio has the opposite shape: one client, one server, one world model,
one ability system, one content pipeline, and a build farm that keeps the
mainline green. We have more verified primitives than they do and fewer
integrated systems.

**The work is integration and deletion, not invention.** Most of what an AAA
MMORPG needs already exists somewhere in this repository in a tested form. The
plan below is mostly "adopt what we built, then delete the copies".

### Scorecard

Grades: **A** at or above AAA practice. **B** sound, with named gaps. **C**
works, but the design won't scale. **D** missing or structurally wrong.

| Area | Grade | One line |
| --- | --- | --- |
| Determinism, clocks, replay | A | Integer clock, strict math, seeded streams, per-step digests |
| Verification of kernels | A | About 5,000 checks in `metaverse-engine/`, mutation-tested policy files |
| Movement authority and prediction | A | Same step on page and room; input credit from the server's clock |
| Row replication | A− | Acked delta baselines, priority, per-link budget, hysteresis |
| Checkpoints, drain, version handshake | A− | Fenced owners, exact receipts, build pins that refuse a mismatch |
| Transport | B | WebTransport datagrams on shards; load-balancer path is TCP only |
| Combat fairness | C | Lag compensation for arrows against monsters only; no line of sight in Everglade II casts |
| Server trust boundary | D | Class and level come from the client's hello; dice seed is published |
| Room architecture and scale | C | 16 seats fixed in data shapes; all rooms on one thread |
| Hosting | D | Every production room on one 2-vCPU VM in one zone |
| Persistence of characters | D | Vitals only; no inventory; quests in the browser |
| Ability system | C | Spells are data; class features, resources and auras are hand-coded |
| Combat profile shipped | D | One target per cast, unlimited presses, no player harm |
| Server AI | C | Two hand-written state machines; engine AI unused |
| Content pipeline | D | Content is compiled-in Coax under the web app |
| Engine layering | D | 88 loads from `metaverse-engine/` up into `metaverse/` and `apps/` (checked) |
| Scene and entity model | C | A good entity store nobody in Everglade uses |
| Renderer | C | Three hand-written hosts restate the pass order; the frame graph runs nowhere |
| World model | C | Zones are separate pages; the streaming world is unadopted |
| Collision queries | C | Linear scans; the broadphase is loaded by tests only |
| Animation | C | Four runtimes; no blend graph, root motion or level of detail |
| Asset packs | C | Seven parsers of one format in six languages |
| Client structure | D | About 70 Wasm pages; game code lives in the website folder |
| Mainline health | D | No gate before `main`; `main-green` hours behind |
| Load and soak testing | C | Largest production check is 64 bots for 60 seconds |
| Observability and alerts | C | Good counters; no alerts, service levels or crash reports |
| Security of keys | D | One symmetric key signs everything, on every machine |
| Social and economy | Absent | By stated choice; the item ledger is built and unused |

## What is already at AAA standard

Keep these and build on them. Don't rewrite them.

- **Determinism.** `metaverse-engine/loop.coax` (integer-nanosecond clock, fixed step,
  bounded catch-up), `math.coax` (same bits natively and in the browser),
  `rand.coax`, and `replay.coax` with a digest for each step.
- **Verification.** About 5,000 `#test` and 469 `#forall` checks in `metaverse-engine/`;
  533 `#authority` files with mutation testing; a CPU reference rasterizer
  that checks the GPU path.
- **Shared simulation.** The page and the room compile the same movement step
  (`apps/openagents-web/everglade-step.coax`), rules and wire codec. The whole
  server also compiles to Wasm for offline play.
- **Movement authority.** The room steps clamped inputs through the same
  collision step. Input credit is earned from the server's clock, at most four
  steps a tick, so a speed hack gains nothing.
- **Row replication.** Delta against the newest acknowledged row, a priority
  accumulator, a byte budget for each link that adapts to congestion, enter
  and leave hysteresis at 110 m and 130 m, and reliable spawn and despawn.
- **Backpressure by class.** Replaceable state, lifecycle, outcomes and cues
  are treated differently when a link falls behind.
- **Checkpoints.** Owner generations fence stale writers. Receipts are exact.
  A replay is answered `duplicate`. Play pauses at the 30-second loss bound.
- **Deploys.** Shards drain one at a time, pages re-seat elsewhere, and build
  pins refuse a page whose engine, rules or content differ from the room's.
- **Network tests.** Real browser pairs under 120 ms, jitter and loss; a
  drift check; a loss matrix that found a real QUIC resend bug.
- **The SRD import.** A pinned source hash, byte-identical regeneration, and a
  ledger that marks nothing Verified without evidence.
- **The spell effect program.** Spells are records with a closed list of
  effect steps. This is the right shape; it stops too early (see G2).
- **The timing profile.** One sealed `#authority` layer maps SRD rounds to
  real time and names each deviation.
- **Shaders in Coax.** Written once, run on the CPU under test, emitted to
  WGSL, SPIR-V and Metal with no outside toolchain.
- **The host contract.** A versioned C face with conformance suites.

## Fix this week

These are defects, not design debates. Each is small.

### W1. The dice seed is public (checked)

The room's seed comes from the wall clock
(`coax/web/room/room-everglade.coax:377`). It seeds the rules' dice
(`apps/openagents-web/everglade5e-room-rules.coax:116`:
`dice[0].state = seed * 7919 + 3`). The same seed goes to every client in the
world header (`coax/web/room/room-sky.coax:22`). A modified client that counts
draws can know the next d20 before it presses.

**Fix:** seed the dice from the operating system's random source and never
send it. Send a separate seed for cosmetics. Keep the fixed-dice switch for
local rooms.

### W2. The client chooses its class and level (checked)

A hello's class string, such as `srd-fighter-5`, seats the player as that
class at that level (`room-everglade.coax:787-790`, `:859-873`). A second
hello re-seats at another level. XP is the server's, but nothing ties level to
XP on this path.

**Fix:** the API signs class and level into the seat ticket from the stored
character. The room ignores the hello's level. Character Create writes the
class to the API, once.

### W3. One key signs everything (checked)

`OA_ROOM_TICKET_KEY` signs seat tickets, including the admin bit, and also
every room-to-API write: vitals, checkpoints, items, directory, stats, drains
(`coax/web/api/player.c:361,708,741,817,873`, `rooms.c:112`). It is on every
shard, in the API and in the deploy shell. One compromised room process can
forge an admin ticket or write any character.

**Fix:** the API signs tickets with a private key and rooms verify with the
public key. Each shard gets its own key for writes, with the shard's name in
the signed bytes. Add a key ID and a two-key overlap so keys can rotate.
Compliance step E5 (#13248) rewrites the ticket writer anyway; do both
together.

### W4. NPC chat is open to anyone (checked)

`POST /api/playtest/npc/ask` takes the character's persona and history from
the client, and the player key is optional (`coax/web/api/npc.c:1-10`). The
only limit is 2,000 answers a day for everyone. One script can spend the day's
budget and silence every character, or use our model with its own prompt.

**Fix:** require a player key; limit by key and by address; take the persona
from server content by NPC ID. Compliance step E4 (#13247) is the place.

### W5. Everglade II casts skip line of sight

The Everglade II cast path checks slot, a 0.5-second gap and range plus 3 m
(`room-everglade.coax:914-923`, checked). The v1 path has a sight check
(`CR_SIGHT`). A cast goes through walls.

**Fix:** run the same sight query the arrows use.

### W6. The watcher that keeps `main` honest runs on one laptop

Issue #13255: `main-green` is hours behind, and the watcher is a launchd job
on the coordinator's Mac. Move it to an always-on build host today. The
structural fix is P0 below.

## The structural gaps

Twelve gaps, each with evidence, the AAA practice, and what to do. The program
after this section puts them in order.

### G1. Everglade is not one game yet

**Evidence.**

- `everglade/features.coax` names four "zones", each its own Wasm module:
  `everglade2.wasm`, `fivelab.wasm`, `archery-range.wasm` and v1's
  `everglade.wasm`. A zone change is a URL navigation.
- `apps/openagents-web/build.sh` builds about 70 pages.
- Game code by location: `apps/openagents-web/everglade*.coax` 57,557 lines;
  `verse/everglade/` 60,087; `coax/web/room/` 24,218; `everglade/` 7,612. The
  folder named for the game holds the least of it.
- Everglade II's server is a flag (`.e2`, 51 uses) inside the v1 room. The
  room binary loads 18 v1 files, borrows v1's battle layout, and keeps two
  tick functions (`everglade_room_tick`, `everglade2_room_tick`).
- v1 is "frozen", yet `everglade.coax` had 73 commits and `everglade-net.coax`
  104 in ten days.
- Three client controllers (`everglade/zone.coax`, `everglade2.coax`,
  `everglade.coax`) and two reconciliation implementations (`net_replay_*`,
  `e2n_replay_*`).

**AAA practice.** One client executable and one server executable. The world
is data that streams into them. Old versions live in version control, not in
the build.

**Do.**

1. Declare Everglade II the game. It gets the names `everglade/client/` and
   `everglade/server/`. The "II" goes away.
2. Extract `everglade/server/` with its own battle layout and frames, then cut
   the v1 room out of the binary.
3. Make zones data: terrain tiles, spawns, points, light. Archery Range and
   Character Create become places in the world or scenes of the one client.
4. Move v1, Experimental, the 5E Lab and the other labs out of the default
   build. The reviewers estimate 80,000 to 100,000 lines. Keep them buildable
   by name for reference; stop paying for them on every build.
5. When zones are data, delete `everglade-parity` and `features.coax`. That
   check matches names because the design lets copies exist. One client makes
   the copies impossible.

### G2. Abilities are code, and the shipped combat profile isn't a game

**Evidence.**

- Spells are data with about 33 effect-step kinds. But executable records stop
  at 90 of the 339 imported spells (`metaverse-engine/rules/srd/more.coax:47`), and
  about 150 `== SP_X` special cases sit outside the records.
- Class features are 36 hand-written kinds, each with its own procedure and
  its own fields on the creature (`engine.coax:212-300`: rage, focus,
  `flame_until`, `hex_free`, and more).
- A creature holds at most six lasting effects (`RS_EFFECTS :: 6`), a battle
  eight zones. There are no stacking rules and no generic triggers.
- Familiar, Healer's Kit, arrows and rest each have their own room file and
  their own frames.
- The live profile, `RS_EVERGLADE_WORLD`, sets `RS_UNLIMITED_PRESS`: every
  press refills slots, uses, Focus and Rage (`press-budget.coax:15-23`,
  checked). A harmful cast hits one creature near the aim point. Fireball on
  a pack hits one wolf. Per-seat booleans stand in for reagents.
- Range is measured in 5-foot squares with diagonal distance counted as
  straight, so diagonal reach is about 41% longer.
- The battle clock is `f32` with 0.01-second tolerances; precision drops below
  that after a day or two of uptime.
- A cast's wire ID is its position on the class bar
  (`class-kit.coax:8-9,369`). Reordering a bar changes the protocol.

**AAA practice.** Every MMORPG that shipped hundreds of abilities did it with
one data model: an ability record (cost, cast time, recharge, targeting,
effects) plus auras that carry modifiers, periodic effects and triggers. WoW's
spell, effect and aura tables and Unreal's ability system are the two public
references. Designers add an ability by adding rows.

**Do.**

1. Write the target combat profile first: real area resolution through
   `rs_area`, paid resources, recharge and cast time for each ability, and
   ranges in metres. Then delete the unlimited-press profile.
2. Define one `Ability` record and one `Aura` record. Move class features onto
   them. Replace the per-class fields on `RsCreature` with an aura list that
   has stacking rules and a real cap.
3. Give every ability, spell, creature template and spawn a stable content ID.
   Use those IDs on the wire and in saves. Register them like wire shapes.
4. Use an integer tick for the battle clock.
5. Split `battle.coax` (3,624 lines, not `#authority`) into gated units:
   damage, saves, casting, turn lifecycle.
6. Add a headless encounter simulator that reports time-to-kill for each
   class and level. Balance is a number we should be able to print.

Do this **before** importing the remaining 250 spells. Each one added now adds
special cases that this refactor then has to remove.

### G3. There is no character to persist

**Evidence.**

- A checkpoint holds HP, XP, WANTED, position, one class bit, slots and rest
  state (`room-checkpoint.coax:141-149`). No inventory, gold, equipment,
  quests or known spells.
- Quests are the page's, saved in the browser. The page save is a container
  the client builds, protected by a CRC.
- The item ledger is sound (`migrations/0130_item_ledger.sql`: operation IDs,
  version guard, owner fence, outbox) and nothing calls it.
- The XP table has three levels hard-coded.

**AAA practice.** The server owns one character record. Every change of value
is a transaction with an operation ID, so a retry can't duplicate an item.
Nothing the client stores is trusted.

**Do.**

1. Define the server character record: class, level, XP, abilities known,
   inventory, equipment, currency, quest state. Version it and write the
   migration rule now.
2. Route every reward through the item ledger with a room-side outbox, the way
   checkpoints already work. The hard part is built.
3. Move quest state to the server. The browser save becomes settings and
   cosmetics only.
4. Level comes from XP on the server (W2).

### G4. The room is a 16-seat instance, by construction

**Evidence (checked).** `EG_ROOMS :: 16; EG_SEATS :: 16`
(`room-everglade.coax:56-57`). The count is in the data shapes: one-integer
seat bitmasks, seat-by-seat arrays, IDs packed by 1,000, one battle "for the
16 seats". One `poll()` loop steps all 16 rooms in turn, so one slow raid tick
delays every room on the shard. Travel between rooms closes the socket and
drops auras and zones.

**AAA practice.** Zone servers hold hundreds of players. Interest management
is the only limit on fan-out. Zones run on separate threads or processes.
Crossing a boundary hands the character to the next owner with its state.

**Do.** This is a decision first, then work.

- **State the limit.** Sixteen players in an instance is a fine game (it is a
  dungeon group or a small raid). It isn't what "MMORPG" promises. Until the
  number changes, say "16 players an instance" wherever we describe the game.
  A cap we know about is shown, not implied.
- **Pick the target.** I recommend 150 players in one zone process as the
  design number, with zones as the unit of ownership. That is the size where
  a town feels alive and one core can still hold the tick. `repl-bench.coax`
  already pushes 1,000 entities through the interest grid, so the engine layer
  is ahead of the room.
- **Rebuild the room core for it.** Dynamic seat and entity tables. No seat
  count in any bitmask or packed integer. One room to a thread. Interest as
  the only fan-out limit. This lands with G1's server extraction and G6's
  world: build it once, on the entity store.
- **Then travel.** A hand-off that carries auras and cooldowns between zone
  owners. The 11-04 gap analysis already specifies the fencing it needs.

### G5. Two replication systems

**Evidence.** Rows are replicated properly. Everglade II's monsters (`h2`),
guard (`g2`) and familiars (`m2`) bypass rows: each sends full state by text,
deduplicated by hash, resent every second, with no delta, acknowledgement,
priority, budget or interest check. Frames are decimal text, with an opt-in
packed variant, so two encoders are live. Each outgoing frame is copied four
times, and the flush scans all 512 connections for each frame. Only monsters
and the guard are in the rewind history; players are not. Melee and spells are
judged at the present.

**AAA practice.** One replication layer. A new entity type declares its fields
and gets delta, priority and interest for free. Lag compensation covers every
hit test, with a bound.

**Do.**

1. Put monsters, the guard, familiars and arrows on rows. Delete their frames.
   This also completes compliance steps E2 and E3 by removal instead of by
   pinning frames we are about to delete. **Change E2 and E3 accordingly.**
2. Move prediction and reconciliation into `metaverse-engine/net/`, once.
3. Make the binary packed encoding the only one. Generate encoder, decoder and
   profile from one field table. Write straight into the connection's buffer.
4. Put players in the rewind history. Judge melee and spells with the same
   bounded rewind as arrows.
5. Fuzz the wire parser. No fuzzing exists today; a public socket needs it.

### G6. The engine has no layers, and Everglade doesn't use its integrating parts

**Evidence.**

- 88 `#load` lines in `metaverse-engine/`, outside tests, tools and examples, point up
  into `metaverse/`, `apps/` or `coax/web/` (checked by grep: 40 in `render/`, 30
  in `host/`). Examples: `metaverse-engine/net/latency.coax:5` loads the web app's
  Everglade wire; `metaverse-engine/ui/draw.coax:6-7` loads a Verse HUD frame and the
  web app's font; `metaverse-engine/character-appearance.coax:3` loads Character
  Create's parts.
- The shader library lives above the engine, in `metaverse/gpu/`.
- `metaverse-engine/walk.coax` loads the camera (`follow.coax`, line 23, checked), and
  `Player` carries camera fields. The server's player state includes orbit
  angles. It also loads three test files in production (lines 26, 31, 34).
- The game's rules (`metaverse-engine/rules/srd`, 39,256 lines) are the engine's largest
  directory. Realm tools (`valley-*`, `kokiri-*`) live in `metaverse-engine/tools`.
- 77 files sit flat at the engine's root. Eight of them are walking.
- `metaverse-engine/entity.coax` is a good store (generation-checked handles, deferred
  spawn). Everglade's files load none of it. Its world is a struct of page
  structs.
- No system scheduler. Each page calls its systems by hand. One thread.
- Unused by any product: `sim/broad.coax`, `world/cells.coax`,
  `render/passes.coax`, `actors/` (3,820 lines), `nav/grid.coax` outside one
  small game.

**AAA practice.** Strict layers with a build that fails on an upward include.
One world: entities, component columns, an ordered list of systems, a spatial
index. Everything in the game lives in it, on client and server.

**Do.**

1. Add `coax/web/tool.sh layer-check`. It fails on a new upward `#load` and
   carries an allowlist that only shrinks, like `wire-check`.
2. Reorganize to this layout. Move directories one commit at a time.

   | Target | Holds |
   | --- | --- |
   | `metaverse-engine/core/` | loop, ids, entity, math, rand, input, budget, replay, checkpoint, assets |
   | `metaverse-engine/gpu/` | the shader library from `metaverse/gpu/` |
   | `metaverse-engine/physics/` | query, ground, collide, `sim/` |
   | `metaverse-engine/move/` | one walker, one tuning record, one C face |
   | `metaverse-engine/world/` | cells, terrain tiles, streaming, navigation |
   | `metaverse-engine/net/` | transport, replication, prediction, rewind |
   | `metaverse-engine/render/`, `anim/`, `cam/`, `audio/`, `ui/`, `host/` | as now, with no upward loads |
   | `rules/srd/` at the root, or `everglade/rules/` | the SRD engine, out of `metaverse-engine/` |
   | `metaverse/tools/` | realm-specific tools |

3. Split `Player` into a body the server owns and a camera the client owns.
4. Define one `World` in `metaverse-engine/core/`: the entity store, fixed
   structure-of-arrays columns keyed by slot, an ordered system list with
   declared reads and writes, and a spatial index. Archetypes in the style of
   Flecs or Bevy are more than this game needs.
5. Move Everglade's players, monsters, projectiles and props onto it. This is
   the same work as G4's room core and G5's rows; do them as one design.
6. Put a grid or BVH behind `query_*` by adopting `sim/broad.coax`. Retire the
   legacy `rigid.coax` and `contact.coax`.
7. For each unused module, adopt it or delete it. Unused, tested code still
   costs build time and misleads the next reader.

### G7. Zones are pages; the streaming world sits unused

**Evidence.** Zone travel opens another page out of sight and hands over. The
Experimental pack is 300 MiB, too large for the paged reader. `world/cells.coax`
(64 m cells, resident, simulate and draw rings, floating origin) has no
consumer outside the engine. Terrain has three representations. Navigation is
a uniform grid with flow fields; monsters walk straight lines with no
collision.

**AAA practice.** One continuous world streamed by cell, with level of detail
for far terrain, and a navigation mesh built from the same colliders the
players use.

**Do.** Pick `cells.coax`, `terrain-tile.coax` and paged VPK3 as the one world
model. Convert the glade and the forest path to adjacent cells in one process
as the first proof. Delete the legacy heightfield when its 12 users move.
Build navigation from the zone's colliders and put the monsters on it. The
owner's rule from 2026-11-07 already says to build one world; this is how.

### G8. Three renderers

**Evidence.** `metaverse-engine/render/passes.coax` defines a bounded frame program that
only a test loads. The shipping pass order is hard-coded in `coax/web/host.js`
(4,050 lines), restated in `metaverse-engine/host/apple/PageHost.m` ("the native twin")
and again in the Vulkan host. The draw model is immediate: up to 250,000
vertices rebuilt on the CPU each frame. Uniforms are raw float offsets. The
cluster and Hi-Z culling exists and the web host has no indirect draws.
Materials and pipelines are per page: 69 shader modules in the web app.

**AAA practice.** One frame graph that every platform backend executes.
Retained GPU scene data. GPU-driven culling. A material system.

**Do.** Make `passes.coax` the only pass order and have each host interpret
it. Generate typed uniform blocks. Keep meshes on the GPU and send transforms.
Retire the built-in passes in `host.js` one page at a time, Everglade first.
Write one C reference host core (packs, stores, draw-list walk, input) and
compile it for every platform, leaving only the GPU and window shims per OS.

### G9. Animation and assets

**Evidence.** Four animation runtimes (`skin.coax`, `character/`,
`figure.coax`, `mannequin.coax`). No blend graph, root motion, clip
compression or level of detail. The VPK1 header is parsed in seven places in
six languages. Packs carry JPEG and PNG and every device gets RGBA8, though
the BC7 and ASTC encoder exists. No dependency graph or incremental cook.
`verse/everglade/preview-rig.coax` is 35,005 lines of rig data as source.

**Do.** One animation runtime with a data-driven blend graph and a lower
update rate for distant actors. Compile `forge/pack.coax` to C and call it
from every host; delete the hand parsers. Ship block-compressed textures. Move
rig data into binary assets.

### G10. Server AI and content are code

**Evidence.** Monsters are an if-chain over six states with one target and no
threat table (`room-e2-monsters.coax:213-294`). Spawns are 12 compiled-in
slots. Monsters are identified by index: under 8 is a wolf. The guard is a
second machine. Camp props, points and dialogue are Coax constants.
Aethyrforge has no authoring for items, abilities, quests or loot.

**AAA practice.** Creature templates, spawn tables, loot tables, quests and
encounters are validated data that designers edit without a programmer. The
server loads them at start.

**Do.** One server creature runtime: template, spawn table, threat list,
leash, navigation. Wolves, goblins and the guard become rows. Define versioned
content tables with validators, and have the room load them. Then give
Aethyrforge editors for them. Adopt `metaverse-engine/actors` for the runtime or delete
it.

### G11. Hosting, capacity and operations

**Evidence (checked).** All six production shards and staging run on
`oa-room-1`, one e2-standard-2 in us-central1-a
(`docs/deployment/room-vm.md`). The API is one poll loop with one blocking
database connection, at minimum instances zero. Placement takes one global
advisory lock. Page, room and API deploy by three separate commands. The
largest production check is 64 bots for 60 seconds; the 256-player run used
stand-in persistence. No alerts, no written service levels, no crash reports,
no symbol store (Wasm is stripped). 161 migrations with nine duplicate
numbers. Moderation has no permanent ban, mute or audit log. No feature flags,
maintenance mode or canary shard.

**AAA practice.** No single machine or zone can take the game down. Capacity
is a number proven nightly by bots at full scale. Alerts page someone. Every
crash lands in a bucket with symbols. One versioned release moves the whole
service, behind a canary.

**Do.**

1. Two room VMs in two zones. The directory already supports an endpoint for
   each shard.
2. Protocol bots, not browsers: hundreds of headless clients that speak the
   wire. Run a nightly soak of the full fleet for two hours against staging
   with the real database. Publish tick p99, checkpoint lag and API latency.
3. Alerts on shard down, tick p95, checkpoint failures and API errors. Three
   written service levels: tick time, join time, checkpoint age.
4. A crash handler with a backtrace in the room. A symbol sidecar for each
   Wasm build, keyed by content hash, so error beacons can be read.
5. One release command that orders API, rooms, page, and refuses a page whose
   pins don't match the live shards. A canary shard that takes the build
   first.
6. An API that doesn't block on one connection, with one warm instance.
7. Unique migration numbers enforced by a check, and a generated snapshot of
   the current schema.
8. Permanent bans, mutes and an append-only log of every moderator action.

### G12. The mainline and the process

**Evidence.** About 2,600 commits reached `main` in ten days, up to 415 in a
day. Nothing checks a commit before it lands; the integrator is "built and
optional" (`docs/metaverse/green-main-pipeline.md`, checked). `main-green` runs
hours behind with three of five recent runs red. Two commits built on stale
bases deleted other agents' work and had to be restored. Goldens, 60 FPS and
the pair check are reports, not gates, and the 60 FPS check targets v1, not
Everglade II. 39 files of 600 lines or more have two or fewer tests. Forty
`clamp` definitions. `docs/metaverse` holds 93 documents and no single
architecture page or decision log.

**AAA practice.** Nothing reaches the mainline without a pre-submit build and
a smoke bot. A build cop reverts within minutes. The mainline is always
playable.

**Do.** This has to fit the owner's standing rules: speed matters, releases
aren't blocked except by a regression they cause, and agents push to `main`.
The proposal keeps all three.

1. **A two-minute pre-push check, not a queue.** A tracked hook runs the
   layer, wire and parity checks plus the touched units' tests, and refuses a
   push that deletes a file changed upstream since its base. It runs on the
   pusher's machine or build host and costs about as long as the push.
2. **The watcher on an always-on host** (W6), with the 30-minute auto-revert
   it already has.
3. **Gate on regressions only.** The Everglade pair check and the 60 FPS
   check on Everglade II, compared with production, stop `main-green` when
   they get worse. That is the existing release rule applied one step earlier.
4. **Ratchets.** Publish the four allowlist counts and fail when one rises.
5. **One `ARCHITECTURE.md`** for Everglade and Aethyr, and `docs/decisions/`
   for decisions. Dated documents become history once the page exists.
6. **Owners.** Each directory in the target layout names its checks and its
   budget. An agent that changes it runs them.

## The program

Six workstreams. Each builds the real thing directly; none is a stopgap. The
order comes from dependencies: you can't put content tables on a server that
is a flag inside another server.

| # | Workstream | Gaps | Depends on | Done when |
| --- | --- | --- | --- | --- |
| P0 | **Stop the bleeding.** W1 to W6, the layer check, the pre-push check, the ratchets | W1–W6, G12 | — | No public seed, no client-chosen level, split keys, `main-green` under 30 minutes behind for a week |
| P1 | **One game.** Extract `everglade/server/` and `everglade/client/`; cut the v1 room; move v1, Experimental and labs out of the default build; reorganize `metaverse-engine/` | G1, G6 | P0 | One Everglade client module and one server binary with no v1 file loaded; zero upward loads |
| P2 | **One world.** The engine `World`; room core without a seat count; all entities on rows; broadphase; cells and tiles; navigation | G4, G5, G6, G7 | P1 | 150 protocol bots in one zone at tick p99 under 10 ms; the glade and forest path are cells in one process; the E2 and E3 frames are deleted |
| P3 | **Real combat.** Ability and aura records; the target combat profile; content IDs; the encounter simulator; full lag compensation | G2, G5 | P1 | A level-5 party clears a pack with area spells, paid slots and line of sight; time-to-kill prints for every class |
| P4 | **A character worth keeping.** The server character record; the item ledger wired; server quests; creature runtime and content tables | G3, G10 | P2, P3 | Kill, loot, equip, log out, crash the shard, log in: nothing lost, nothing doubled |
| P5 | **Run it like a service.** Two zones; nightly soak; alerts and service levels; crash buckets; one release; canary | G11 | P0 (can start at once) | A zone outage loses no character; the capacity number on the status page is last night's |
| P6 | **One renderer.** The frame graph on every host; retained scene; one pack reader; one animation runtime | G8, G9 | P1 | `host.js` has no built-in passes for Everglade; one pack parser; compressed textures on phones |

P5 and P6 run beside the others. P1 is the gate for everything else, and it is
mostly moves and deletions. Start there the day P0 lands.

### Changes to existing plans

- **Compliance E2 and E3 (#13245, #13246).** Don't build a shared codec and
  profile for the familiar, kit, guard and monster frames. P2 moves those
  entities onto rows and deletes the frames. Pin only what survives:
  projectiles, the WebTransport framing and the admin lines. Pinning frames a
  month before deleting them is wasted work.
- **Compliance E5 (#13248).** Add W3 to it: the one ticket writer signs with a
  private key.
- **Compliance E4 (#13247).** Add W4 to it.
- **The remaining SRD spells.** Hold the import past 90 until the ability
  record lands (P3).
- **The gap analysis's milestone 5** (reusable Aethyr multiplayer) is P2 here,
  and it moves earlier: extracting the engine contract is how the room core
  gets rebuilt, not a later cleanup.
- **`everglade-parity`.** Keep it until P1 lands, then delete it.

### What to delete

| What | Lines | When |
| --- | --- | --- |
| `coax/web/room/room-oldwire.coax`, the v4 profile | 243 | Now (E1) |
| `metaverse/rules/srd5*.coax` | 423 | P1 |
| v1 client and its room half (`everglade.coax`, `everglade-*.coax`, `everglade5e*`) | about 48,000 | P1, out of the build; deleted after P2 |
| Experimental, 5E Lab, `fivemmo*`, the labs | about 30,000 | P1, out of the default build |
| `verse/everglade/previous/` | 8 MB | P1 |
| The `h2 h3 g2 g3 m1–m8 M1–M7 U1 u1` frames and their files | about 1,500 | P2 |
| The text row encoder | part of 1,865 | P2 |
| `RS_UNLIMITED_PRESS` and the one-target adapter | small | P3 |
| Six of seven pack parsers; three of four animation runtimes; `rigid.coax`, `contact.coax`, the legacy heightfield | about 4,000 | P2, P6 |
| `everglade-parity`, `features.coax` | about 1,000 | After P1 |
| Engine modules with no consumer that P2 doesn't adopt | to be listed in P1 | P2 |

## What we chose not to build, and should say so

Trade, mail, guilds, an auction house and regional servers don't exist. The
gap analysis records that as a choice, and it is the right one today: each
needs the character record and the ledger first (P4). The risk isn't their
absence; it's describing the game as if they were present. Until P4 lands, the
accurate description is: a 16-player co-operative action RPG on the SRD, with
server-authoritative combat and no persistent possessions.

## What this audit didn't check

- No build, test, mutation run, deploy or load test was run. Frame rates and
  capacity figures are quoted from existing documents.
- Live GCP configuration, the Cloud SQL tier and which flags production has
  on.
- The internals of `wt.c`, `room-events.coax`, `room-casts.coax`,
  `room-ticket.coax`'s verification, `auth.c` and the auth migrations.
- Whether `e2_arrow_loose` clamps the client's launch point as its header
  says.
- How hard counting dice draws is in practice (W1). The seed is public either
  way.
- Whether re-seating at a new level also refills HP and slots (W2).
- `metaverse-engine/audio`, `ui`, `fx` and `forge`, beyond their sizes.
- The iOS app's internals, and Android beyond its host file.
- Line and commit counts are from `grep` and `git log` and are approximate.
  The reviewers' count of upward loads was 61; mine, with host conformance
  files included, is 88.
