# WoC Overview and Architecture

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-22
Source: `projects/repos/world-of-claudecraft/` (MIT licensed; `levy-street/world-of-claudecraft`)

## What it is

World of ClaudeCraft is a complete classic-era MMO that runs in three places from one
codebase: an offline browser world, an authoritative Postgres-backed multiplayer server,
and a headless Gymnasium RL environment. It ships nine classes with real vanilla-style
kits and talent trees, three open-world zones, five dungeons, a ranked PvP arena, real
party/trade/duel multiplayer, and a procedural world where almost nothing is a shipped
asset (towns, terrain, water, weather, spell icons, and sound are generated at runtime;
only the rigged character GLBs and a handful of textures/HDRIs are real files).

Stack: TypeScript (ESM, `strict`), Three.js r165 renderer, `ws` WebSockets, Postgres
(`pg`), Vite + esbuild, Vitest. No UI framework. Tiny dependency set.

## The three load-bearing ideas

WoC's root `CLAUDE.md` states three invariants that hold the whole project together. They
are the most important thing to take from this audit, because they are the same shape the
Verse is reaching for.

### 1. One sim, three hosts

The exact same `src/sim/` code runs the offline browser world, the online server, and the
RL env. Behavior must be identical everywhere. The sim is a fixed 20 Hz tick
(`DT = 1/20`), all randomness flows through one seeded `Rng` (never `Math.random` /
`Date.now` / `performance.now`), and `src/sim/` carries zero DOM, browser, or Three.js
imports. An architecture test (`tests/architecture.test.ts`) scans every sim file to keep
it that way.

That purity is what lets the same code bundle into a Node env server, an authoritative
game loop, and a browser tab without changing a line. It is also what makes the headless
RL env a real test of the actual game rather than a reimplementation.

### 2. `IWorld` is the only seam

`src/world_api.ts` defines `IWorld` (a ~300-line read-only interface: player, entities
map, inventory, equipment, party, social, market, quests, etc., no setters). The offline
`Sim` satisfies it structurally; the online `ClientWorld` (`src/net/online.ts`)
implements it by mirroring server snapshots. **`src/render/` and `src/ui/` talk only to
`IWorld`**, never to a concrete world. A new render/UI feature extends `IWorld` first,
then is implemented in both worlds, then consumed through the interface. The HUD therefore
runs identically offline and online and never knows which world it is reading.

### 3. The server is authoritative

Clients stream movement intent and commands at 20 Hz; the server runs the one shared
`Sim` and returns interest-scoped (~90-120 yd) snapshots plus per-player events. Every
combat roll, loot drop, quest credit, and vendor transaction resolves server-side. The
client is a renderer; it never decides outcomes.

## Repo map

| Path | What it is |
|---|---|
| `src/sim/` | Deterministic game core, source of truth. No DOM/Three deps. |
| `src/sim/content/` | Data-as-code: classes, abilities, zones, dungeons, items, talents. |
| `src/render/` | Three.js renderer (procedural geometry/textures/VFX). Reads world, never mutates. |
| `src/game/` | Local input, camera, keybinds, mobile controls, procedural WebAudio. |
| `src/ui/` | Classic HUD (frames, windows, tooltips, map, FCT), procedural icons, i18n. |
| `src/net/` | Online client: REST auth + WebSocket world mirror (`ClientWorld`). |
| `src/admin/` | Admin dashboard SPA (separate `admin.html` entry). |
| `src/world_api.ts` | `IWorld`, the seam render/ui depend on. |
| `server/` | Authoritative server: HTTP+WS, world loop, Postgres, auth, social, moderation. |
| `headless/` + `python/` | RL env server (`env_server.ts`) + Python Gym bindings. |
| `tests/` | Vitest suite (formulas, combat, AI, quests, parties, duels, dungeons). |
| `scripts/` | Asset build + browser E2E/screenshot/integration `.mjs` scripts. |
| `public/` `docs/` | Static assets (GLB/textures/HDRIs) + design and PRD docs. |

Most directories carry their own `CLAUDE.md` with local conventions.

## Conventions worth noting

- ESM + TypeScript `strict` everywhere, 2-space indent, tiny dependency set.
- **Module-first for new code.** The big monoliths (`sim.ts`, `hud.ts`) are intentional,
  but new self-contained behavior goes in its own small module behind an existing seam,
  extracted on the rule of three. Pure presentation/domain logic (geometry, formatting,
  id/state resolution) is lifted out of DOM/render/sim into host-agnostic modules a
  Vitest imports directly, leaving the render side a thin consumer. This is exactly why
  so much of WoC ports cleanly: the hard logic is already DOM-free and tested.
- **i18n: every player-visible string is a `t()` key**, classified by render sink. The
  sim and server stay language-agnostic (emit stable keys + values, re-localized at the
  client boundary). 14 locales, contributors add English only.
- No `Math.random` / `Date.now` in sim. No `ALLOW_DEV_COMMANDS=1` in production. No
  secrets committed. No em dashes / emoji anywhere.

## How the three ideas map onto our stack

| WoC | Verse equivalent |
|---|---|
| Deterministic `Sim` (offline + RL + server) | Public Worker/D1 remains product authority; `apps/openagents-world` Region Durable Objects own live world presence/interaction |
| `IWorld` seam (render/ui read-only) | `packages/world-contract` + `packages/world-client` expose a read-only world projection consumed by `three-effect` + Foldkit HUD |
| Authoritative server, client renders | Same posture: Region DOs own world commands/presence; Worker/D1 owns run/proof/business truth; desktop/web render |
| `ClientWorld` mirrors server snapshots | `packages/world-client` mirrors Cloudflare `WorldDelta` snapshots/deltas into the Verse read model |
| 20 Hz tick, intent streaming | Controller pose/intent commands publish at bounded rates to Region DO command handlers with seq/ack diagnostics |

The key divergence: WoC's authority is one process running one `Sim`. Ours is split across
the Cloudflare Verse World Service (presence/local interaction) and the public Worker/D1
(run/proof/business truth), deliberately, because the Verse must never let presence or
multiplayer fabricate run or settlement state. WoC does not have that constraint, so it can
keep everything in one authority. When we borrow its netcode patterns (interest scoping,
delta snapshots, seq/ack receipts), we apply them inside the Region Durable Object
presence layer and keep the evidence-bound projection ("no anonymous motion without public
refs") on the Worker/D1 side.

The most important non-obvious carry-over: WoC proves that the discipline of **one seam +
pure-logic extraction + server authority** is what makes a world maintainable across
offline, online, and headless hosts at once. Our `three-effect`-first rule and the
Cloudflare-world-vs-Worker/D1 authority split are the same instinct; this audit is largely
about borrowing the concrete modules that fall out of that discipline.
