# Game Direction

This folder collects OpenAgents game-world, HUD, MMO, and spatial run-page
direction.

Current transcript anchor: [`docs/transcripts/240.md`](../transcripts/240.md)
demos the live-facing shape this folder is aiming at — a walkable Autopilot
Tassadar run board with Pylon bases, assignment markers, training metrics, refs
ticker, avatar movement, and an initial multiplayer direction. Treat it as
visual/product intent; authority for real run state remains with the public
Worker/D1. Live Verse presence and local interaction are moving to the
Cloudflare Verse World Service (`apps/openagents-world`) with Region Durable
Objects, D1, `packages/world-contract`, and `packages/world-client`. Older
world-backend docs are historical source material unless a document explicitly
says otherwise.

## Reading Order

- `2026-06-16-spatial-hud-agentic-mmo-wow-direction.md` - archive synthesis for
  Commander, HUD, MMO, Tassadar living-run, and receipt-backed visual language.
- `2026-06-17-episode-189-agentic-mmorpg-run-page-analysis.md` - focused read of
  episode 189 and the pivot from gamified HUD to actual game/world.
- `2026-06-17-tassadar-wasd-mouselook-controller-plan.md` - implementation plan
  for adding a reusable `three-effect` WASD + mouselook controller and enabling a
  2.5D first-person `/tassadar` navigation mode.
- `2026-06-17-agent-avatar-proximity-chatter-world-plan.md` - brainstorm and
  implementation direction for making each pylon's agent visible as a world
  avatar that can move, notice nearby visitors, talk locally, and emit chat
  bubbles during a run.
- `2026-06-17-openagents-world-asset-catalog.md` - owned OpenAgents world asset
  catalog and provenance policy for avatars, stations, props, materials,
  adornments, and Quick 3D MMORPG reference-asset eligibility.
- `2026-06-17-proof-replay-theater-system-plan.md` - audit and implementation
  plan for turning public proof sets into deterministic 3D replays with agent
  avatars, proof gates, camera tracks, and receipt-backed sats zaps.
- `2026-06-22-effect-typescript-world-backend-replacement-audit.md` - audit of
  the decided fast path for the OpenAgents-owned Effect/TypeScript Cloudflare
  Verse World Service while
  preserving the Worker/D1 product authority split, subscription contract,
  multiplayer semantics, WoC-derived world-read seam, and outage behavior.
- `2026-06-21-verse-scene-graph-vs-react-three-fiber-audit.md` - deep audit of
  how the desktop Verse scene graph is built today (`three-effect` + Foldkit,
  full teardown+rebuild on every change) versus react-three-fiber's
  catalogue/reconciler/attach/on-demand-frameloop model, with the concepts to
  port into `three-effect` and how Effect (`Scope`, `Layer`, `SubscriptionRef`,
  fiber frame clock) is the better substrate for them.
- `2026-06-22-verse-custom-keybindings-audit.md` - audit of current Desktop
  shortcuts, Verse movement/controller bindings, `three-effect` input
  primitives, and the recommended MMO-style custom keybinding architecture.
- `woc/` - thorough system-by-system audit of the open-source MMO World of
  ClaudeCraft (`projects/repos/world-of-claudecraft/`) as a reference for the
  Verse: overview/architecture, HUD + hotbar + procedural icons, input/camera/
  targeting, multiplayer netcode + moderation, chat/minimap/nameplates/world,
  and a consolidated, prioritized adaptation plan mapped to the Cloudflare/Effect
  backend cutover, `three-effect`, desktop HUD, and follow-on issue lanes. Start
  at `woc/README.md`.

## Implementation Homes

- `apps/openagents-world/` owns the Cloudflare Worker + Region Durable
  Object Verse World Service: live presence, socket fanout, interest scoping,
  local world commands, chat moderation before fanout, expiry, and durable D1
  projection rows.
- `packages/world-contract/` will own Effect Schema row/command/delta contracts,
  branded refs, world-read projection schemas, interest plans, public-safety
  helpers, and test fixtures.
- `packages/world-client/` will own the Cloudflare Verse client and WoC-style
  read-only `ClientWorld` mirror consumed by desktop/web render and HUD code.
- `/Users/christopherdavid/work/three-effect` owns reusable spatial/visual
  primitives for the game world and proof replay theater. Add missing replay
  stages, avatar, zap, camera, particle, terrain, label, and interaction
  primitives there before consuming them from web or desktop.
- `packages/proof-replay/` owns replay bundle normalization, deterministic
  clocks, source gates, and timeline planning only. It is not a visual renderer.
- `apps/openagents.com/` and `apps/autopilot-desktop/` may adapt public replay
  data and render Foldkit HUD/inspector/accessibility chrome, but should consume
  `three-effect` for world visuals rather than adding app-local DOM/canvas
  replay renderers.
