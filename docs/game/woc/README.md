# World of ClaudeCraft (WoC) Audit

Date: 2026-06-22

This folder audits the open-source MMO **World of ClaudeCraft** (`levy-street/world-of-claudecraft`,
local reference clone at `projects/repos/world-of-claudecraft/`) as a system-by-system
reference for our own 3D world, **the Verse** (Autopilot Desktop + `three-effect` +
the `openagents-world` SpacetimeDB module).

WoC is a complete classic-era browser MMO built on one deterministic TypeScript sim
core, a Three.js renderer with zero hand-imperative UI framework, and an authoritative
Postgres-backed server. It is MIT licensed. It is close enough to what the Verse wants
to be (walkable third-person world, hotbar, chat, minimap, nameplates, real multiplayer
presence) that it is the single best end-to-end reference we have. It is also different
enough (it is a fantasy game; we are an agentic-work world bound to real run/proof
evidence) that we adapt patterns and pure logic, not gameplay.

## Why this matters for the Verse

Transcript [`240.md`](../../transcripts/240.md) is the product intent: a walkable
Tassadar run board with Pylon bases, assignment markers, training metrics, a refs
ticker, avatar movement, jump/sprint, tab-target, and early multiplayer. The Verse epics
(#5887 SpacetimeDB multiplayer, #5819/#5822/#5883 default world, #5897 forum reflection,
#5943 keybindings, VCODE-* code mode) are all building the same surface WoC already
ships. WoC shows us what "done" looks like for the MMO-shaped parts, and where the
landmines are (per-frame DOM cost, interest scoping, chat moderation, save cadence).

## Read order

1. [`01-overview.md`](01-overview.md) - what WoC is, the "one sim, three hosts"
   architecture, the `IWorld` seam, the repo map, and how its three load-bearing ideas
   map onto our Worker + SpacetimeDB + `three-effect` split.
2. [`02-hud-and-hotbar.md`](02-hud-and-hotbar.md) - HUD composition, the hotbar/action-bar
   model, the procedural canvas icon system, unit frames, cast bars, resource meters,
   tooltips, performance overlay.
3. [`03-input-camera-targeting.md`](03-input-camera-targeting.md) - keybind registry and
   remapping, third-person camera (follow/orbit/zoom/collision), tab-target and pointer
   picking, click-to-move, F-interactions, mobile/touch controls.
4. [`04-multiplayer-netcode.md`](04-multiplayer-netcode.md) - the 20 Hz authority loop,
   interest-scoped delta snapshots, persistence, social systems (party/trade/duel),
   auth, and moderation, and how each maps to SpacetimeDB + Worker.
5. [`05-chat-minimap-world.md`](05-chat-minimap-world.md) - chat channels, minimap +
   compass + coords + subzone, nameplate projection, player context menu / card, and the
   procedural world (terrain, sky, water, foliage, weather, rigged characters).
6. [`06-adaptation-plan.md`](06-adaptation-plan.md) - the consolidated, prioritized
   recommendation: which systems we adapt, where they land in our repos, what we drop,
   and the candidate issue lanes.

## TL;DR adaptation verdict

The fastest, highest-leverage wins are the **pure, host-agnostic logic modules** WoC
already isolated from DOM and Three.js. They port almost verbatim into `three-effect`,
`packages/`, or the desktop HUD and come with unit tests we can mirror:

| System | WoC verdict | Verse priority |
|---|---|---|
| Procedural canvas icon system | Adopt wholesale | High |
| Hotbar action model (slots, dedup, sync) | Adopt the pure model | High |
| Keybind registry + remap | Adopt; merges with #5943 | High |
| Camera follow / collision / pointer-pick | Adopt pure math | High |
| Chat channels + timestamp + profanity | Adopt the pure model | High |
| Compass / minimap-zoom / coords / subzone | Adopt pure cores | High |
| Nameplate projection + threat + combo | Adopt pure math | High |
| Interest-scoped delta snapshots | Adapt pattern onto SpacetimeDB | High |
| Chat moderation (two-tier, escalation) | Adapt pattern into Worker | Medium |
| Unit frames / cast bar / resource meters | Adapt | Medium |
| Tooltips + item compare | Adapt if we have inspectable entities | Medium |
| Click-to-move, mobile joysticks | Adopt if/when we ship those surfaces | Low |
| Procedural terrain/sky/water/foliage | Adapt architecture, not code 1:1 | Low/Medium |
| Talents, arena, loot rolls, dungeon scoring | Drop (game-specific) | Drop |
| Raw imperative DOM HUD framework choice | Diverge (we use Foldkit) | Diverge |

The cross-cutting lesson is WoC's discipline, not its content: a **single deterministic
core behind one seam (`IWorld`)**, **pure logic split from rendering so it is unit
testable**, **the server owns every outcome**, and **almost nothing is a shipped asset**.
Those four are exactly the invariants our Verse + SpacetimeDB + Worker design is already
reaching for, and they are the real thing to copy.
