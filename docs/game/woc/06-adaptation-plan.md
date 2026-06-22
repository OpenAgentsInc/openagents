# WoC Adaptation Plan for the Verse

Date: 2026-06-22
Consolidates docs 01-05 into a prioritized "what we adapt, where it lands, what we drop".

## Principle first

The biggest thing to take from World of ClaudeCraft is not any one system; it is the
**discipline** that makes its systems portable:

1. One deterministic core behind a single read-only seam (`IWorld`).
2. Pure domain/presentation logic split out of DOM and renderer so it is unit-testable.
3. The server owns every outcome; the client renders.
4. Almost nothing is a shipped asset; the world is generated from a shared seed.

Our Verse already reaches for all four (SpacetimeDB + Worker authority split, `three-effect`
primitives, evidence-bound projection, owned procedural assets). This plan borrows the
concrete modules that fall out of that discipline, mapped to our repos and epics. Where
WoC keeps one authority, we keep the Verse's deliberate split: **SpacetimeDB owns presence
and local interaction; the Worker/D1 owns run/proof/business truth; the desktop renders.**
No borrowed pattern may let presence fabricate run or settlement state.

## Where adapted code lands

- **`three-effect`** (`/Users/christopherdavid/work/three-effect`): camera follow/collision,
  pointer-pick, the procedural icon system, nameplate/label projection, minimap rendering
  primitives, and the procedural-world patterns (terrain/sky/water/foliage/weather/animation).
  This honors the workspace rule to build Verse visuals in `three-effect` first.
- **`apps/autopilot-desktop`**: the Foldkit HUD that consumes the above (hotbar, chat,
  minimap, unit/run frames, perf overlay), plus the keybindings work (#5943).
- **The SpacetimeDB module + desktop projection** (`apps/openagents-world-spacetimedb`,
  `apps/autopilot-desktop/src/shared/chat-world-*.ts`): interest scoping, distance-tiering,
  delta-as-unchanged on presence rows (#5888/#5889/#5892).
- **The public Worker** (`apps/openagents.com`): chat moderation (two-tier + escalation),
  per-account login throttle, and the evidence-bound run/proof projection that feeds the
  Verse.

Most adapted code is **pure logic we re-author to our taxonomy**, not vendored files
(consistent with the workspace "do not vendor large chunks of external code" rule). WoC is
MIT, so direct reuse is permitted, but the cleanest path is to port the pure modules and
mirror their tests.

## Priority tiers

### Tier 1 (high ROI, low risk, mostly pure modules)

| System | Source | Lands in | Notes |
|---|---|---|---|
| Procedural canvas icon system | `src/ui/icons.ts`, `ui_icons.ts` | `three-effect` / `packages/` | Re-skin recipes to Pylon/agent/run/proof taxonomy; keep keyword fallback so new agent actions always render. Matches owned-asset policy. |
| Hotbar action model | `src/ui/hotbar.ts` | desktop HUD | Slots + dedup + sync, persisted per user. Maps Verse actions (assignment, inspect-proof, focus, zap, emote). |
| Keybind registry + remap | `src/game/keybinds.ts`, `input.ts` | #5943 lane | Folds directly into the custom-keybindings audit; adopt `canUseGameKeys` modal/chat gate as the baseline input context. |
| Camera follow + collision | `src/game/camera_follow.ts`, `src/render/camera_collision.ts` | `three-effect` controller | Auto-settle-behind + occlusion easing; the "feel" our controller lacks. Mouselook-first, so skip orbit/Mouse-Camera mode. |
| Pointer pick | `src/game/pointer_pick.ts` | `three-effect` input | Click-vs-drag disambiguation when world entities become clickable. |
| Chat channels/timestamp/profanity | `src/ui/chat_channels.ts`, `chat_timestamp.ts`, `profanity.ts` | desktop HUD | Pure models; pair client mask with server filter (Tier 2). |
| Compass/minimap-zoom/coords/subzone | `src/ui/compass.ts`, `minimap_zoom.ts`, `coords.ts`, `subzone.ts` | `three-effect` + HUD | Minimap reads the same world the 3D scene reads; blips = Pylons, run core, assignments, avatars. |
| Nameplate projection/threat/combo | `src/render/nameplate_projection.ts` (+ threat/combo) | `three-effect` label primitive | World->screen anchor for entity labels + status bars. |
| Interest-scoped + distance-tiered + delta presence | `server/game.ts` snapshot logic | SpacetimeDB projection (#5888/89/92) | Subscribe-by-radius with hysteresis; tier publish rate; treat absent field as unchanged; settle-row on stop. |

### Tier 2 (medium, adapt the pattern)

| System | Source | Lands in | Notes |
|---|---|---|---|
| Two-tier chat moderation + escalation | `server/chat_filter.ts` | Worker | Empty hard list seeded privately; whole-token + confusable-fold matching; warn->timed-mute ladder. Gate Verse chat/forum bubbles (#5906) before delivery. |
| Per-account login throttle | `server/ratelimit.ts` | Worker auth | Orthogonal to per-IP; defeats distributed credential stuffing. |
| Unit frame / cast-progress / resource meters | `src/ui/unit_portrait*.ts`, `cast_bar.ts`, `absorb_bar.ts`, `xp_bar.ts` | desktop HUD | Portrait HiDPI+overscan for agent face chips; cast/progress bar maps onto Tassadar run-step lifecycle (#5822). |
| Perf overlay model | `src/ui/perf_overlay_model.ts` | `three-effect`/HUD | Frame-time sparkline + draw-call counter for the WebGL Verse; model/consumer split ports without DOM. |
| Interaction dispatch pattern | `src/game/interactions.ts` | desktop | Injected-interface F-to-interact (inspect Pylon, open run board), range-check + "too far" toast, unit-testable. |
| Player context-menu action model | `src/ui/player_context_menu.ts` | desktop | Right-click a Pylon/avatar: inspect/message/tip/view-run/report. |
| Atomic staged-confirm | `server/` trade flow | Worker | Template for owner-gated two-party Verse actions (confirmed tip/zap, co-sign). |
| Tooltip pure-core pattern | `src/ui/stat_tooltip.ts`, `item_compare.ts` | desktop | Only if Verse exposes inspectable comparable entities; copy the core+thin-view split. |

### Tier 3 (low / when a surface ships)

| System | Source | Notes |
|---|---|---|
| Procedural terrain/sky/water/foliage/weather/animation | `src/render/*` | Adapt architecture into `three-effect`, not code 1:1. Keep: shared canonical sun, shared seed for placement, chunked LOD + skirts, splat-from-slope, instanced deterministic foliage, pooled camera-relative weather, velocity-driven animation state machine. |
| Click-to-move | `src/game/click_move.ts` | Optional accessibility/traversal; keep turning-cone + latency stop-distance tricks. |
| Mobile/touch controls | `src/game/mobile_controls.ts` | Only if the Verse ships a touch client; reuse joystick/pinch pure helpers, long-press-vs-tap, double-tap recenter. |
| Player card (shareable) | `src/ui/player_card.ts` | Shareable Pylon/agent/run OG cards if wanted. |

### Drop (game-specific or superseded)

- Talents, arena ladders, loot rolls (need/greed), dungeon scoring, party/duel/tap-rights
  mechanics: WoW-shaped gameplay, not Verse work.
- WoW biomes/zones and the GLB rig dispatch: different assets and world.
- Raw imperative DOM HUD as a framework choice: we use Foldkit. Borrow the discipline
  (read-only, signature-recompute, hot-write elision idea), not the framework.

## Cross-cutting carry-overs (apply everywhere)

- **Pure-logic split + mirror the tests.** Every adapted system is a DOM/renderer-free
  module with a Vitest, exactly as WoC ships it. This is the cheapest quality win.
- **One world, one source.** The minimap, nameplates, and 3D scene must all read the same
  projected world so they cannot drift. WoC enforces this via `IWorld`; our analog is the
  SpacetimeDB-rows + Worker-projection the desktop already mirrors.
- **Absent means unchanged.** Adopt the delta invariant in the presence projection so a
  stalled subscription never blanks an avatar.
- **Empty hard-list, seeded privately.** Never commit a slur list; operators curate. This
  is the correct open-repo posture for our moderation too.
- **Evidence-bound first.** Unlike WoC, the Verse may not animate motion without public
  refs or a measured live transition (#5822). Borrowed netcode/visual patterns live inside
  the presence/render layer; run/proof authority stays on the Worker.

## Suggested issue lanes

These map onto the existing Verse epics rather than inventing a parallel program:

1. `three-effect`: procedural icon primitive (Tier 1) - re-skinned recipe system + keyword
   fallback, with a Vitest mirror of `icons` behavior.
2. `three-effect`: camera follow + collision + pointer-pick primitives (Tier 1) - improves
   the existing controller feel.
3. `three-effect` + HUD: minimap primitive + compass/coords/subzone pure cores (Tier 1) -
   blips from SpacetimeDB avatars + Worker run projection.
4. `three-effect`: nameplate/label projection primitive (Tier 1) - world-anchored entity
   labels with status bars.
5. desktop HUD: hotbar action model + chat channel model (Tier 1) - folds chat into the
   forum-reflection lane (#5904-#5907).
6. SpacetimeDB projection: interest scoping + distance tiering + delta-as-unchanged
   (Tier 1) - serves #5888/#5889/#5892.
7. #5943: adopt the keybind registry + input-context gate (Tier 1).
8. Worker: two-tier chat moderation + per-account login throttle (Tier 2) - gate Verse and
   forum-reflection chat (#5906).
9. desktop HUD: run-step cast/progress + agent portrait chip + perf overlay (Tier 2) -
   ties to #5822 Tassadar-in-scene.

Each lane is a pure-module-first port with its own test, consistent with both repos'
"leave it cleaner, test-backed" rules.
