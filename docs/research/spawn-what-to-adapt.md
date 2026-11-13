# Spawn: what to adapt

Status: analysis, 2026-11-12. This reads [spawn.md](spawn.md) against the
Metaverse Engine as it is on `main` today, the browser editor spec
([docs/editor/2026-11-12-browser-editor.md](../editor/2026-11-12-browser-editor.md),
issues #13410-#13423) and the AAA program (#13257).

## The short version

Spawn has built what we are building: an engine that agents operate, with a
browser editor and live worlds. It is about four months ahead on the product
loop and behind us on rigor (determinism, verification, server authority).
Its most valuable idea isn't a technology. It is a stance: **the running world
is the center, and every tool (Savi, external agents, the editor) works
against it, sees the result and gets told why something went wrong.**

Our editor spec already has the same shape (one World, one edit log, a live
channel, the agent in the editor). What Spawn adds, and we lack, is the
quality of **what the engine tells an agent**: typed diagnostics,
version-matched docs and UI hit testing. It also shows the boundaries we need
before strangers' code runs on our site: separate kinds of state, separate
kinds of authority, and the game sandboxed from the site.

Their multiplayer model (clients simulate, relays order) is the one part we
should not adopt for Everglade.

## Where we stand on each Spawn idea

| Spawn idea | Ours today | Verdict |
|---|---|---|
| A running, inspectable world at the center | `coax drive` (`tree`, `state`, `screenshot`, `logs`, `perf`); the editor spec puts the World at the center | **Have the shape.** The editor issues finish it |
| Generate → apply → observe → diagnose → revise | Apply and observe are covered by E6 (`apply` returns a frame and the refusal) | **Diagnose is the gap. Adapt now** (N1) |
| Typed diagnostics: "in state but not rendered", "texture still loading", "script failed at this field", "GPU device lost" | Refusals in words exist for edits (Splice's checker, content validator). Nothing answers "why can't I see entity 42?" across state → render → asset → collider | **Adapt now** (N1) |
| Budgets explained: runaway scripts paused and the reason given | Budgets exist (frame, compile, wasm, "no hidden limits"), but a cap the engine applies at runtime isn't reported to the agent | **Adapt now**, inside N1 |
| UI inspection: element bounds, visibility, an overlay intercepting clicks | `coax drive tree` and `layout` give widgets by id; nothing reports what actually receives a tap at a point | **Adapt now** (N5) |
| Version-matched agent docs (`llms.txt`, `llms-full.txt`, an OpenAPI spec, a reference per engine version) | Site-level `llms.txt` exists (`apps/openagents-web/llms.txt`), and `coax vocab`; nothing ties the engine's API reference to the build a world runs (we already pin builds per published realm, migration 0107) | **Adapt now** (N2) |
| A world as a constrained, learnable vocabulary (YAML objects + behaviour scripts) | Content tables (`aethyr.content.v1`), typed edits and Coax for rules. Same idea, stricter types | **Have it.** Keep Coax for rules; don't add YAML |
| Three kinds of state: authored, running, durable | All three exist (content tables and edits; the World and E2's state keeping; the character record and item ledger) but no rule says an authored rollback never touches durable data | **Adapt now** as an invariant (N3) |
| Three kinds of authority: content, simulation, durability | Server-authoritative simulation (AAA P0); admin writes for content; the ledger for durability. Not written per realm | **Adapt now**, inside N3 |
| The game in a separate origin (`play.spawn.co`), sandboxed, cross-origin isolated | Pages are cross-origin isolated (COOP/COEP in `serve.c`) but served from the site's own origin | **Adapt soon:** required before anyone's realm code runs for other people (E12) (N4) |
| Git as a world's source of truth; publish moves a ref; an update applies to sessions now or at next session | Code is in the monorepo; worlds will be documents with revisions (E3); a publish pins the build (0107); rooms must accept the previous wire (#13408) | **Adapt the policy, not Git:** each world gets a `dev` and a `live` revision, and an update says "now" or "next session". Added to E3 |
| Content-addressed assets, CDN, direct upload for large files | Packs are content-named (`pack.coax`, `publish.coax`) | **Adapt with E11:** a signed direct upload to storage, so big files skip the API |
| Workers: simulation, transport, rendering, jobs, shader compile, texture decode | The offline room runs in a Web Worker; GPU device loss is handled (`host.js` `deviceLost`); otherwise one thread | **Soon, measured first:** move pack parsing and texture decode off the frame (with P6.2 #13304), then decide on a render worker |
| A renderer restart that keeps simulation state | `deviceLost` recovery, plus E2's state keeping for code swaps | **Have most of it** |
| Mantle: deterministic custom physics; GJK/EPA; internal triangle edges giving false wall normals | Deterministic engine (graded A); "no invisible blockers"; the owner's open complaint that mounts get stuck on ground colliders | **Check soon:** test the character controller on triangle-mesh seams for false wall normals (N6) |
| Proximity-based script execution; idle objects don't tick; empty areas do less work | The World's ordered system list; no sleep or wake policy | **Soon**, with P2.2's 150-player room (#13275) |
| Places, rooms, instances; portals that carry objects | Zones, rooms of 16 seats, cells planned (P2.7 #13280); travel carries auras and cooldowns (P2.9 #13282) | **Have the plan.** Borrow the word "instance" for copies of a zone |
| A per-game SQLite-compatible data store, dev and live, 10 GiB | Postgres; the character record and ledger for Everglade; nothing for a user-made realm | **Later:** each made realm will need its own durable store, with dev and live kept apart |
| Multiplayer by client simulation and elected hosts; a relay that orders but doesn't validate | Server-authoritative rooms, prediction and reconciliation (AAA) | **Don't adopt for Everglade.** It could suit small casual realms later, with the authority table (N3) stating what a client may decide |
| Import by AI-assisted porting (Unity, Godot, Phaser, Three.js) | The Unreal exporter (#13399-#13402); import from Unreal sketched | **Same stance as ours:** content moves, logic gets rewritten. Nothing new to take |
| Savi on Claude Opus 5.5, with Wisps working on parts of a build | Coder with Claude Code, Codex, Antigravity, Microcoder; subagents | **Have it.** Their builder seat staying across areas matches E9's agent strip |

## Adapt now

These cost little next to the editor work and make every agent loop better.
Each is an issue on project 22 (#13429-#13434).

- **N1 (#13429). Typed diagnostics: why an entity isn't seen, reached or working.**
  - The engine answers `why ID` with the first link that failed in the chain
    state → spawned → in view → drawn → asset loaded → collider → script or
    system ran. The answer names the field, the expected and actual values,
    and whether a retry can help.
  - Runtime caps (budgets hit, a system paused, a pack refused) are reported
    on the same channel.
  - It is exposed through `coax drive why ID`, `world why`, the editor's
    inspector, and the result of `world apply`.
- **N2 (#13430). A version-matched reference for agents.**
  - Each engine build writes a reference pack: the world and edit schemas,
    the `world_*` tools, content table schemas, limits, diagnostic codes and
    examples.
  - The pack carries the build's identity and is served beside the build a
    realm is pinned to (`/worlds/NAME/llms.txt`, linked from the site's
    `llms.txt`). `world show` names it.
  - An agent working on a published realm reads the reference for that
    realm's build, not for `main`.
- **N3 (#13431). Three kinds of state and three kinds of authority, written down and
  checked.**
  - An `INVARIANTS.md` entry and a check: undo, content rollback and
    `world_load` never write the character record or the ledger.
  - Each realm states who decides content, simulation and durability. Splice:
    owner and admins / the page / nothing durable. Everglade: admins / the
    room / the ledger.
- **N5 (#13433). UI hit testing for agents.** `coax drive hit X Y` names the element
  that receives a tap at a point and anything covering it. The drive tool
  reports overlaps and tap targets under 44 px for the HUD and the editor
  shell.

## Adapt soon

- **N4 (#13432). Realms that run others' code are served from their own origin.** This
  is a sandboxed frame on a separate origin, keeping cross-origin isolation.
  The site, the editor shell and the chat talk to it only by typed messages.
  It is required before E12 (code from the browser) opens to anyone but the
  owner.
- **N6 (#13434). Character controller on triangle-mesh seams.** A check that walking
  and riding over the seams between triangles never produces a wall normal on
  flat ground (Spawn's v5.2.20 bug), tied to the open complaint that mounts
  get stuck.
- **Dev and live revisions per world, and an update policy** of "now" or
  "next session". Added to E3 (#13412).
- **Direct upload for large assets.** Added to E11 (#13420).
- **Work off the frame.** Measure first, then move pack parsing and texture
  decode into a worker (with P6.2).
- **Idle systems sleep:** a wake and sleep policy in the World's system list,
  with the 150-player room (P2.2).

## Don't adopt

- **Client-run simulation with elected hosts for Everglade.** Combat, loot and
  currency stay server-decided (AAA P0, P4). If a casual realm wants it later,
  N3's authority table decides what a client may own.
- **YAML world files and JavaScript behaviours.** Content tables and Coax give
  the same learnable surface, with types, determinism and mutation-tested
  rules.
- **A WebGPU-only runtime.** Our hosts already run Metal and Vulkan natively.
  Keep that.

## What would change our minds

Spawn publishes no scene or hardware benchmarks, no per-area player counts,
and no ownership-transfer timings. If they publish numbers for dense,
interacting areas that beat a server-authoritative room at equal cost, look
again at hybrid authority for casual realms.
