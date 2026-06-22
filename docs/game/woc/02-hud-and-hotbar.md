# WoC HUD, Hotbar, and Icon System

Date: 2026-06-22
Scope: `src/ui/` (HUD), `src/ui/hotbar.ts`, `src/ui/icons.ts`, unit frames, meters, tooltips, perf overlay.

## HUD architecture

The HUD is a single large coordinator, `src/ui/hud.ts` (~6,280 lines, ~338 fields, ~172
methods), implemented in **raw imperative DOM with no UI framework**. It is a read-only
renderer of `IWorld` state, called once per frame after the sim tick. Three properties
make it fast and worth studying:

- **`IWorld`-only.** It never imports `Sim` or `ClientWorld`; it reads through the
  interface (`src/world_api.ts`). Runs identically offline and online.
- **Hot-write dedup cache.** A `Map<HTMLElement,string>` gates every DOM write through
  `setText` / `setDisplay` / `setTransform` / `setWidth`. A write is skipped if the value
  is unchanged. The skip rate is a guarded perf invariant (target > 80%): per-frame DOM
  is the dominant cost in a browser MMO HUD, and elision is what keeps it cheap.
- **Three throttle tiers.** Per-frame core (HP/resource/cast/auras/action-bar/nameplates),
  ~100 ms (minimap), ~250 ms (party/quest/trade/arena/map), ~500 ms (social/market). The
  per-frame core stays imperative with no allocations; windows recompute only on a changed
  "signature" string.

There is an active modularization program (`docs/ui-architecture-hud-modularization/`,
`docs/hud-program-roadmap.md`) extracting the ten on-demand windows (spellbook, talents,
character, bags, market, social, trade, options, quest log, arena) into per-window modules
behind a shared `HudContext` (one hot-write gate, a window manager, an icon service,
formatters, the `IWorld`, keybinds, `t`). The locked decision is to keep the game HUD
framework-free; the admin SPA is the one place a framework may be piloted.

### Relevance to us

We use Foldkit, not raw DOM, so we **diverge on the framework choice** and do not adopt
`hud.ts` literally. What we take is the *discipline*: read-only projection, a single
state seam, recompute-on-signature for panels, and the pure-logic-split that makes every
sub-system below testable without a DOM. Our `verse-scene-graph-vs-react-three-fiber`
audit already argues for an Effect-native frame clock and scoped resources; WoC's
hot-write gate and signature pattern are the DOM-side analog of the same on-demand idea.

## Hotbar / action bar

`src/ui/hotbar.ts` (~170 lines) is **pure domain logic, fully unit tested, no DOM.** The
model:

```ts
type HotbarAction = { type: 'ability'; id: string } | { type: 'item'; id: string } | null;
```

A 12-slot bar keyed by index, mapped to keys `1`-`9`, `0`, `-`, `=`, serialized to
`localStorage`. Pure functions:

- `parseHotbarAction()` validates against known ability/item ids; **each ability appears at
  most once per bar** (dedup via a `seenAbilities` set).
- `placeAbilityOnSlot()` / `swapHotbarSlots()` return new arrays (no mutation): place into
  empty, swap when occupied.
- `buildDefaultFormBar()` / `shouldSeedFormBar()` seed a context bar (WoC uses druid forms)
  and detect un-customized bars to migrate.
- `syncHotbarActions()` removes now-invalid ids on ability change and auto-places new
  high-priority abilities into empty slots.

GCD is **not** in the hotbar; it is a sim-level constraint. The HUD reads
`player.gcdRemaining` and per-frame computes a cooldown sweep via
`remainingCooldown(now, started, duration)` to paint the slot overlay. Drag/drop between
spellbook, bags, and bar is HUD plumbing on top of the pure model.

### Relevance to us

**Adopt the pure model wholesale.** The Verse hotbar should map agent/Pylon actions
(assignment, inspect-proof, zap, focus-target, open-run-board, emote) into the same
slot/dedup/sync model, persisted per user. The only things that change are slot count and
the action taxonomy. The cooldown-sweep idea is reusable for any rate-limited or
owner-gated action (for example a tip/zap action that is gated and shows a cooldown).

## Procedural canvas icon system

`src/ui/icons.ts` (~1,523 lines) composes every spell/item/aura/crest icon at runtime on a
128x128 canvas from layered parts, cached as PNG data URLs. **No image files for icons.**

- An `IconRecipe = { bg, pal, prims[], fx? }`: ~16 backgrounds (fire/frost/arcane/holy/
  steel/food/parchment...), ~18 palettes (steel/gold/blood/ice/venom/holyGold...), ~25
  hand-authored vector primitives (sword/flame/skull/shield/heart/book/gem/staff...),
  optional FX (glow/sparkle).
- Known ids get hand-assigned recipes (`ABILITY_RECIPES['fireball'] = r('fire','blood',
  ['flame','sphere'])`). **Unknown ids fall back procedurally** from school + name
  keywords, so every id always renders a valid icon, even abilities added after launch.
- Public API: `iconDataUrl(kind, id, size)`, `iconCanvas(kind, id, size)`, plus a
  `QUALITY_COLOR` table. Results are module-cached. `ui_icons.ts` does vector UI chrome.

### Relevance to us

**Adopt wholesale, highest ROI of the UI layer.** This is the single most reusable thing
in WoC for the Verse. It is procedural, asset-free, fallback-safe, extensible, and
canvas-only (so it can blit into a 3D texture atlas in `three-effect`, or into a Foldkit
HUD). It aligns directly with our asset policy in
`2026-06-17-openagents-world-asset-catalog.md` (owned, procedural, provenance-clean). We
would re-skin the recipe tables to our taxonomy (Pylon/agent/run/proof/training icons)
and keep the fallback-by-keyword behavior so new agent-defined actions always get a sane
icon with no new art. Home: a `three-effect` icon primitive or a `packages/` module.

## Unit frames, portraits, cast bar, meters, XP bar

These are split into **pure cores plus thin DOM consumers**, which is why they test
cleanly and port well:

- **Portrait** (`unit_portrait.ts` core + `unit_portrait_painter.ts` consumer): HiDPI
  backing-store sizing (`portraitBackingPx`, clamped DPR), crest overscan math so a square
  emblem's bevel lands outside a circular clip, and `crestIdForEntity(kind, family)` to
  resolve an entity to an emblem. One shared painter instance serves both player and
  target frames.
- **Absorb bar** (`absorb_bar.ts`, pure): models shields as absorb auras;
  `absorbBarView()` returns fill fraction + overshield flag.
- **XP / progress bar** (`xp_bar.ts`, pure, snapshot-tested): pre-cap fill with rested
  overlay, at-cap lifetime total, and a post-cap virtual-level overflow ("prestige") mode.
- **Resource meters** and **cast bar** are inline in `hud.ts`'s per-frame core, reading
  `castBarState()` from `src/render/cast_bar.ts` (`{ cast, progress, remaining }`).

### Relevance to us

**Adapt (medium priority).** The portrait HiDPI + overscan math is reusable for any
circular avatar chip (a Pylon's agent face in the Verse HUD). The cast/progress bar maps
naturally onto a Tassadar run's per-step lifecycle (assignment -> trace -> replay ->
verdict -> settle), which already wants a progress affordance per #5822. The absorb/
resource meter math is only worth it if the Verse gives entities depletable resources;
otherwise skip. Reuse the pure-core + thin-consumer split so our versions are Vitest-able.

## Tooltips, item compare, settings, perf overlay

- **Stat tooltip** (`stat_tooltip.ts` core + `_view.ts`): class-aware stat-effect lines
  whose coefficients mirror the sim's `recalcPlayerStats()`, cross-checked in tests. The
  view receives `t` and `formatNumber` injected, so it is DOM-testable.
- **Item compare** (`item_compare.ts`, ~33 lines pure): stat deltas vs equipped, thresholded
  so same-for-same swaps yield nothing; HUD colors upgrades/downgrades.
- **Settings controls** (`settings_controls.ts`): generic DOM builders (`settingRow`,
  `toggleControl`) that take plain callbacks + localized strings, no app coupling.
- **Performance overlay** (`perf_overlay_model.ts` pure + `perf_overlay.ts` consumer):
  frame-time sparkline, FPS, p95/max, draw calls, entity/memory counts, DPS/HPS meters;
  drag-to-place, persisted to localStorage. Metric math is DOM-free and testable.

### Relevance to us

**Adapt selectively.** Tooltips + item-compare are worth it only if the Verse exposes
inspectable entities with comparable stats (for example inspecting a Pylon's capabilities
or a run's parameters); the pure-core pattern is the part to copy. The **perf overlay is
genuinely valuable** for a `three-effect`/WebGL Verse: a frame-time sparkline + draw-call
counter we can toggle is exactly the kind of diagnostic the desktop world will want, and
the model/consumer split means the metric math ports without the DOM. Settings builders
are superseded by Foldkit, so skip those.

## Net for the adaptation plan

High priority: **procedural icon system**, **hotbar pure model**. Medium: **portrait /
cast-progress meters**, **perf overlay model**, **tooltip pure-core pattern**. Diverge:
the raw-DOM HUD framework (we use Foldkit) and WoW-specific windows (talents, arena,
loot rolls). The throughline is to keep copying WoC's pure-logic-split so every piece
arrives with a unit test, not a DOM dependency.
