# Arwes first-principles audit and Khala UI architecture

## Anti-pattern verdict

Arwes is visually distinctive rather than generic, but importing it wholesale
would turn a focused product into a science-fiction skin: framed everything,
constant glow, decorative motion, illegible text tricks, and optional sound
competing with work. The right move is to preserve its _grammar_—precise edge
geometry, restrained luminous hierarchy, explicit enter/exit choreography, and
ambient depth—while rejecting its React runtime, product styling assumptions,
and “effect on every surface” temptation.

- Date: 2026-07-15
- Snapshot: OpenAgents `ac287fc22d2aaa76e3783e592d695ad54a84ec47`; Effect Native `ec04d1a066d6f3ed0c67735ba451cfc90a343aa8`; Arwes `bdbaa0324900ee978d42036d1304a053c1fe54b5`
- Class: architecture and dependency audit
- Status: recommendation; no implementation or deployment authority
- Dispatch: no; create and claim bounded implementation issues before coding
- Owner: OpenAgents UI architecture
- Final disposition: retain until Khala UI is implemented, rejected, or superseded
- Decision: build an owned Khala UI visual language inside Effect Native from
  first principles; never add Arwes as a production dependency

## Executive decision

Build **Khala UI** as the OpenAgents visual and interaction language on top of
the existing Effect Native component contract. “Khala UI” should be the product
and design-system name, not a second component runtime, state system, token
authority, or React library. Its implementation belongs in the owned
[`OpenAgentsInc/effect-native`](https://github.com/OpenAgentsInc/effect-native)
repository across the packages that already own each concern:

| Concern                                                 | Existing owner                                     | Khala UI addition                                                             |
| ------------------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------------------- |
| color, space, type, radius, motion, breakpoints         | `@effect-native/tokens`                            | named Khala geometry, luminance, edge, density, and ambient-quality roles     |
| portable component vocabulary and typed intent boundary | `@effect-native/core`                              | a small set of semantic-neutral decoration nodes and choreography descriptors |
| web and Electron lowering                               | `@effect-native/render-dom`                        | React 19-safe semantic DOM plus inert CSS/SVG decorative layers               |
| native lowering                                         | `@effect-native/render-rn`                         | explicit equivalents, degradations, or unsupported capabilities               |
| canvas lifecycle                                        | `@effect-native/render-canvas`                     | one Effect-scoped scheduler and bounded ambient primitives                    |
| examples and conformance                                | `@effect-native/gallery`, `@effect-native/testkit` | the Khala golden gallery, visual/a11y/performance matrices, and fixtures      |

Do **not** start with an `@arwes/*` fork or even an
`@effect-native/khala-ui` package. The current Effect Native catalog already
contains `Frame`, `Glow`, `BackgroundGradient`, `Wallpaper`, `Spotlight`, and a
central `MotionPreferenceService`. A parallel package would recreate the
catalog, theme, and lifecycle split this architecture exists to prevent. Add a
new package only later if a measured heavyweight optional subsystem—most
plausibly audio or advanced canvas effects—needs an independently lazy-loaded
dependency boundary.

Arwes should be treated as pinned MIT reference material, not an upstream to
track. Reproduce useful behavior from an OpenAgents specification and original
tests; do not copy its API shape, React wrappers, sound assets, or implementation
by default. Record Arwes provenance in third-party notices for any source that
is actually adapted. The Arwes website sounds have a separate website-only
license and must not be copied.

The implementation sequence is:

1. convert the standalone Effect Native repository from its remaining Bun
   scripts to the selected Node, pnpm, and Vite Plus authority;
2. freeze a Khala UI contract, capability matrix, invariants, and golden
   gallery before adding new product decoration;
3. implement deterministic geometry and static CSS/SVG frames first;
4. pilot one restrained static treatment in Desktop Project Home, then one
   retained `/forum` surface;
5. add an Effect-scoped choreography service only after static semantics,
   server output, reduced motion, and cleanup are proven;
6. add bounded canvas ambience only after measurement and lifecycle gates;
7. leave decipher text, pointer illumination, and audio out until separate
   opt-in product decisions justify them.

## Route authority correction after public-site unification

This section originally captured the short-lived split in which `apps/astro`
owned the landing candidate and `/tanstack` was its frozen Start comparison.
That topology changed after this audit and after #8848 was filed. Commit
`abf4eaa311` deleted `apps/openagents.com/apps/astro`, moved the public routes
into `apps/start`, made `/astro` the Desktop MVP landing, and reduced
`/tanstack` to a compatibility redirect to `/astro`.

Current repository law is [`apps/openagents.com/INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md):

- `apps/start` owns `/`, `/astro`, `/install`, and `/app`;
- `/tanstack` has no independent product or comparison authority; and
- the apex root keeps its holding-page contract until a separate owner decision.

Therefore:

| Surface                  | Current decision                                                                                                                               |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `/tanstack`              | Preserve the compatibility redirect. Do not create a second Khala landing or revive the removed comparison.                                   |
| `/astro`                 | The Start-owned Desktop MVP landing is authoritative. Any future visual change must be a newly scoped Start issue with current SSR/runtime budgets. |
| retired `apps/astro`     | Do not recreate it. #8848 is superseded because its implementation home and static-runtime premise no longer exist.                            |
| retained web product UI  | The bounded `/forum` board/status treatment remains the accepted Effect Native web pilot.                                                       |
| Desktop                  | Project Home and Settings remain the accepted first static product pilots.                                                                      |

The [#8848 disposition](./2026-07-15-khala-ui-astro-pilot-superseded-disposition.md)
records the repository evidence. It does not authorize a root cutover, deploy,
or Start landing redesign.

## What was audited

### Arwes

The audit covered the repository history, license and asset notices, workspace
topology, package exports, theme and CSS helpers, animator state graph, Motion
One bridge, dynamic SVG frame engine, Canvas backgrounds, pointer illumination,
text effects, Web Audio manager, React/Solid wrappers, documentation guidance,
CI, unit tests, and manual performance examples.

Important sources are pinned to the audited commit:

- [project status and React constraints](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/README.md)
- [closed contribution policy](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/.github/CONTRIBUTING.md)
- [MIT license](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/LICENSE)
- [workspace manifest](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/package.json)
- [animator types](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/types.ts)
- [animation scheduler](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/createAnimation/createAnimation.ts)
- [dynamic frame dimension evaluation](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/formatFrameDimension.ts)
- [dynamic frame lifecycle](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrame/createFrame.ts)
- [moving-lines Canvas implementation](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundMovingLines/createBackgroundMovingLines.ts)
- [HTML pointer illuminator](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/effects/src/createEffectIlluminator/createEffectIlluminator.ts)
- [decipher text implementation](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/animateTextDecipher/animateTextDecipher.ts)
- [Web Audio manager](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/createBleepsManager/createBleepsManager.ts)
- [sound asset license notice](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/static/assets/sounds/README.md)
- [CI workflow](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/.github/workflows/ci.yml)

### OpenAgents and Effect Native

The destination audit covered:

- current web route ownership, the `/tanstack` compatibility redirect,
  Start-owned `/astro` destination, Forum mount, and route budget;
- the Effect Native catalog, theme, motion preference, direct DOM and React DOM
  renderer seam, Canvas host, gallery, testkit, and vendoring boundary;
- Desktop's Electron security policy, React 19 Strict Mode workbench, CSS/token
  bridge, Effect state stream, host-boundary tests, startup measurements, and
  candidate pilot surfaces; and
- the Vite Plus conversion contract and current exact `0.2.4` build-core pin.

The architectural authorities are:

- [Effect Native one-substrate analysis](../effect-native/2026-07-08-effect-native-one-ui-substrate-analysis.md)
- [React web renderer harmonization gap analysis](../effect-native/2026-07-14-react-web-renderer-harmonization-gap-analysis.md)
- [three-effect / canvas boundary](../effect-native/2026-07-08-three-effect-vs-effect-native.md)
- [Node, pnpm, and Vite Plus conversion contract](./2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md)
- [Desktop product architecture](./2026-07-10-openagents-desktop-product-architecture.md)
- [Effect Native vendor manifest](../../apps/openagents.com/packages/effect-native-vendor.json)

### Evidence boundary

The Arwes checkout contains no installed dependency tree. A local attempt to
invoke its test script stopped before test discovery because `vitest` was not
installed; the machine's Node 25/npm 11 runtime also differs from Arwes's Node
20/npm 10 contract. The reference repository was deliberately left unchanged.
This audit is source, history, and configuration analysis; it does not claim an
independent passing Arwes build.

The upstream test corpus has 40 test files, but no meaningful coverage was
found for React Strict Mode, hydration, RSC, accessibility, reduced motion,
keyboard behavior, or the React animator/frame/effect wrappers. The performance
app records manual React Profiler/console results without enforceable budgets.

## First-principles model of Arwes

Arwes v1 is not a complete application component library. Its own button and
card examples live in the docs app, not in the published component catalog. It
is better understood as a layered visual-effects toolkit:

```text
pure tools + color/style helpers
            │
            ▼
hierarchical animator state graph
            │
            ▼
Motion One transition bridge
            │
     ┌──────┼────────┬──────────┬────────┐
     ▼      ▼        ▼          ▼        ▼
   frames  canvas   pointer     text     audio
           bgs      effects    effects   bleeps
     └──────┴────────┴──────────┴────────┘
            │
            ▼
thin React 18 / Solid wrappers and aggregate barrels
```

Its durable ideas are smaller than its package graph:

1. **Geometry is the brand carrier.** Chamfers, clipped corners, segmented
   strokes, headers, underlines, and sparse accents establish a visual
   language before motion or sound exists.
2. **Animation is a state transition, not an arbitrary timeout.** An element is
   `entering`, `entered`, `exiting`, or `exited`; parent managers coordinate
   parallel, sequence, stagger, reverse, and switch behavior.
3. **Assembly motion follows geometry.** Lines draw, segments reveal, and
   opacity/transform reinforce structure instead of merely fading a box.
4. **Ambience and content are separate layers.** Canvas and pointer effects
   sit behind or around content rather than defining application semantics.
5. **Sound is categorized by intent.** Background, transition, interaction,
   notification, and voice channels are conceptually useful even though the
   current implementation and assets are unsuitable.

Those are behaviors to specify. They are not reasons to inherit Arwes's
framework choices.

## Package-by-package disposition

| Arwes subsystem                    | What it teaches                              | Khala UI destination                                     | Disposition                                                                 |
| ---------------------------------- | -------------------------------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------- |
| `tools`                            | small numeric, DOM, and random helpers       | local pure functions next to their owning subsystem      | Learn; do not create a miscellaneous utility package                        |
| `theme`                            | derived color ramps and named breakpoints    | `@effect-native/tokens`                                  | Reject as authority; extend `khalaTheme` only                               |
| `styles`                           | stepped separators and clip-path geometry    | tokens + DOM/RN renderer lowerings                       | Reimplement selected geometry; reject repeating stripe effects              |
| `animator`                         | explicit transition states and manager modes | Effect-scoped choreography service in core/runtime       | Re-specify and model; do not port the mutable graph                         |
| `animated`                         | cancellation and geometry-aware presets      | renderer motion driver using canonical motion tokens     | Reimplement after static semantics                                          |
| `frames`                           | the strongest reusable visual corpus         | pure geometry kernel + typed `Frame` variants            | Reimplement from scratch; no `eval`, `innerHTML`, or imperative SVG rebuild |
| `bgs`                              | ambient dots/grid/puffs/lines                | static CSS/SVG first; later `render-canvas`              | Rewrite after measurement                                                   |
| `effects`                          | local light can reveal surface depth         | renderer-local optional pointer capability               | Defer; use CSS hover first                                                  |
| `text`                             | branded reveal can mark a rare milestone     | stable semantic text + optional `aria-hidden` decoration | Reject for task content; reconsider only for short brand labels             |
| `bleeps`                           | event categories and mix controls            | separate opt-in sound service/package, if ever           | Defer; create original licensed assets                                      |
| React wrappers                     | ergonomic adapter around vanilla primitives  | existing Effect Native React DOM renderer                | Reject entirely                                                             |
| Solid wrappers                     | alternate framework bridge                   | none                                                     | Ignore                                                                      |
| aggregate `arwes` / `@arwes/react` | one install surface                          | none                                                     | Reject; it unnecessarily pulls the complete graph                           |

## Integration-readiness score

This score evaluates Arwes as source material for the current OpenAgents web
and Electron products. It does not score artistic originality.

| Dimension                |     Score | Evidence                                                                                                                                                                                             |
| ------------------------ | --------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Accessibility            |       1/4 | reduced motion is consumer advice, decipher mutates cloned visible text, pointer effects are mouse-only, meaningful content can begin hidden, and audio lacks a complete opt-in accessibility policy |
| Performance              |       2/4 | cancellation, rAF, ResizeObserver, and DPR caps are useful; global pointer layout reads, per-instance loops, Canvas coordinate errors, and missing visibility/power gates are not                    |
| Responsive behavior      |       2/4 | responsive SVG redraw and ResizeObserver are good foundations, but touch/coarse-pointer, forced-colors, zoom, SSR, and several DPR/resize behaviors are unproven or defective                        |
| Theming                  |       3/4 | low-level color/style helpers and CSS variables are flexible, but there is no complete product theme and they must not replace `khalaTheme`                                                          |
| Product anti-pattern fit |       2/4 | the aesthetic is intentional, but broad use creates framed-card excess, decoration overload, animation gating, and reduced information density                                                       |
| **Total**                | **10/20** | **Acceptable only as reference material; substantial redesign is required**                                                                                                                          |

### Severity summary

- P0 — 0: no issue requires emergency remediation because Arwes is not a
  production dependency.
- P1 — 6: direct adoption blockers involving React/Strict Mode, CSP, SSR,
  accessibility, audio, and semantic text/pointer behavior.
- P2 — 6: architecture, lifecycle, Canvas, packaging, route ownership, and
  verification gaps that constrain an owned implementation.
- P3 — 2: maintenance/CI and broad responsive/forced-color maturity gaps.

The three highest-priority conclusions are:

1. never install the Arwes React runtime into React 19/Strict Mode/TanStack or
   the hardened Electron renderer;
2. define Khala UI inside Effect Native instead of creating a parallel design
   system; and
3. ship static geometry before animation, Canvas, text effects, or audio.

## Detailed findings

### P1-1 — The Arwes React model is incompatible with the destination

- **Location:** Arwes root README, React development guide,
  `packages/react/package.json`, `react-animator`, and `react-bleeps`.
- **Category:** compatibility, lifecycle, architecture.
- **Impact:** Arwes requires React 18, tells consumers to disable Strict Mode,
  does not support RSC, and creates animator/audio resources from render-time
  `useMemo`. OpenAgents web and Desktop use React 19; Desktop and the Effect
  Native React DOM host intentionally use Strict Mode. Direct adoption invites
  duplicate setup, leaked resources, and concurrent-render defects.
- **Standard:** React render must be pure; resource acquisition belongs to a
  committed, idempotent Effect Scope owned below the component contract.
- **Recommendation:** do not install `arwes` or `@arwes/react`. Keep React an
  implementation detail of `@effect-native/render-dom` and express Khala
  decoration as typed Effect Native nodes.
- **Suggested command:** `$impeccable harden the Khala UI renderer lifecycle for React 19 Strict Mode`

### P1-2 — Dynamic frames violate the Electron CSP

- **Location:** Arwes
  [`formatFrameDimension.ts`](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/formatFrameDimension.ts)
  and [`renderFrameElements.ts`](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/renderFrameElements.ts);
  [`apps/openagents-desktop/index.html`](../../apps/openagents-desktop/index.html).
- **Category:** security, CSP, implementation correctness.
- **Impact:** Arwes evaluates frame dimension strings with `eval` and permits
  string elements through `innerHTML`. Electron correctly allows only
  `script-src 'self'`, has `connect-src 'none'`, and does not allow
  `unsafe-eval`. Relaxing that policy for decoration would be a security
  regression.
- **Standard:** no `eval`, `new Function`, HTML string injection, or policy
  relaxation for visual effects.
- **Recommendation:** build a closed `DimensionExpr` algebra—literal, percent,
  add, subtract, multiply, divide, min, max, clamp—and evaluate it as pure
  typed data. Render stable SVG nodes through the renderer.
- **Suggested command:** `$impeccable harden frame geometry for CSP-safe Electron rendering`

### P1-3 — Arwes's SSR behavior can hide meaningful content

- **Location:** animator server branch, React `Animated`, React `NoSSR`, and the
  docs app's static-export configuration.
- **Category:** accessibility, SSR, resilience.
- **Impact:** the server animator remains `exited`, `Animated` defaults to
  hidden in that state, and `NoSSR` renders nothing until an effect. This makes
  “does not throw during SSR” look like SSR support while no-JS and pre-hydration
  users can receive hidden or absent content.
- **Standard:** semantic content and primary controls must exist in server HTML
  and remain visible without animation or JavaScript; decoration may upgrade
  after hydration without changing semantic order.
- **Recommendation:** render final static content first. Hydration may attach an
  inert decorative layer and optional non-gating motion after commit.
- **Suggested command:** `$impeccable adapt Khala UI frames for semantic SSR and stable hydration`

### P1-4 — Motion accessibility is advisory rather than systemic

- **Location:** Arwes motion documentation and animator provider; current
  Effect Native `MotionPreferenceService` and renderer motion resolution.
- **Category:** accessibility, motion, user preference.
- **Impact:** Arwes asks consumers to consult `prefers-reduced-motion`, but the
  runtime does not centrally prevent animations. A CSS duration override alone
  still creates loops, timers, subscriptions, and intermediate states.
- **Standard:** WCAG motion accommodations and the repository's single motion
  authority require the preference to be resolved before starting work. An
  explicit user choice may reduce more motion, never override the OS request to
  force more.
- **Recommendation:** extend the existing Effect Native service. Reduced mode
  must select a static end state and skip scheduler/resource construction.
- **Suggested command:** `$impeccable clarify Khala UI motion preferences and static fallbacks`

### P1-5 — Bleeps are unsuitable technically, legally, and product-wise

- **Location:** Arwes Bleeps manager/loader/provider and sound asset notice.
- **Category:** accessibility, licensing, network/CSP, resource lifecycle.
- **Impact:** the manager constructs an `AudioContext`, defaults toward preload,
  fetches URL assets without aborting in-flight work, and does not fully close
  the graph on teardown. Desktop forbids renderer network connections. The
  repository's sound files are licensed for the Arwes website rather than
  redistribution. Default sound would also be disruptive in a work tool.
- **Standard:** sound is off by default, gesture-unlocked, visibly mutable,
  original/licensed, abortable, category-scoped, and fully disposed.
- **Recommendation:** omit audio from initial Khala UI. If later approved, make
  it a separately lazy-loaded Effect service with original assets and a
  product-level accessibility review.
- **Suggested command:** `$impeccable adapt optional Khala UI audio controls for accessibility`

### P1-6 — Text and pointer effects damage semantics as written

- **Location:** Arwes decipher/sequence and HTML/SVG illuminator packages.
- **Category:** accessibility, input modality, performance.
- **Impact:** text effects clone a DOM subtree, hide the real content, and
  rewrite the clone every frame. Assistive technology can encounter gibberish,
  interactive descendants lose application behavior, and meaningful content is
  gated. Each illuminator installs document-level mouse listeners and combines
  `getBoundingClientRect()` with style writes per event; it excludes touch and
  coarse pointers.
- **Standard:** accessible text is stable; decorative copies are `aria-hidden`;
  decoration is never focusable or interactive; pointer behavior is local,
  optional, coalesced, and modality-aware.
- **Recommendation:** omit decipher from product workflows. If a rare brand
  label needs a reveal, keep the complete label in the accessibility tree and
  animate only an inert duplicate. Prefer CSS hover/focus lighting before a JS
  pointer driver.
- **Suggested command:** `$impeccable harden Khala UI decorative text and pointer semantics`

### P2-7 — The useful animator concept must become an Effect service, not a second state graph

- **Location:** Arwes `createAnimatorSystem`, manager modes, scheduler, and
  React registration lifecycle; Effect Native runtime and view stream.
- **Category:** architecture, state ownership.
- **Impact:** copying the animator would put a mutable visual state graph beside
  Effect Native's existing Effect state, scope, view, and intent authority.
  Parent/child registration and random node IDs add lifecycle ambiguity under
  concurrent rendering.
- **Standard:** product state stays in Effect. Visual choreography may observe
  resolved view transitions but cannot become application/domain authority.
- **Recommendation:** define a bounded Effect service with explicit states and
  manager semantics, injected Clock and deterministic IDs/randomness, and
  Scope-owned fibers. Model its small state space before implementation.
- **Suggested command:** `$impeccable animate Khala UI state transitions through one Effect-owned choreography service`

### P2-8 — Canvas implementations contain correctness and efficiency defects

- **Location:** Arwes Dots, GridLines, Puffs, and MovingLines backgrounds.
- **Category:** performance, responsive behavior, correctness.
- **Impact:** backing-store dimensions are multiplied by DPR, then some drawing
  loops treat those physical pixels as logical extents, performing excess work
  and drifting geometry. MovingLines compares dimensions captured from the
  canvas to the same unchanged properties, so its resize reseed condition is
  ineffective; its random range helper also omits the minimum offset. Infinite
  loops lack IntersectionObserver, Page Visibility, window-focus, or power
  lifecycle gates.
- **Standard:** one logical coordinate system, capped DPR, one scheduler per
  surface, deterministic seeded fixtures, offscreen/hidden pause, and explicit
  quality budgets.
- **Recommendation:** begin with static CSS/SVG dots or grid. Later implement
  Canvas through `@effect-native/render-canvas`, never as one loop per component.
- **Suggested command:** `$impeccable optimize Khala UI canvas ambience for visibility, DPR, and power`

### P2-9 — Arwes packaging would import the wrong dependency authority

- **Location:** 23 Arwes packages, aggregate React barrel, Motion One 10 peer,
  root Node/npm/Turbo/Lerna toolchain, and missing `sideEffects` declarations.
- **Category:** dependency and build architecture.
- **Impact:** the aggregate package installs visual, animation, text, effect,
  and audio subsystems, pins old peers, and introduces a second build/runtime
  culture. Even narrow packages would create external theme and animation
  authorities beside Effect Native and Vite Plus.
- **Standard:** one Node/pnpm/Vite Plus task graph, exact build-core identity,
  renderer-owned optional code, and measurable route/startup budgets.
- **Recommendation:** add no Arwes package. Implement in existing Effect Native
  modules and tree-shake/lazy-load only capabilities that are genuinely
  optional.
- **Suggested command:** `$impeccable optimize Khala UI package and bundle boundaries`

### P2-10 — The standalone Effect Native repo still has outgoing toolchain authority

- **Location:** Effect Native root `package.json` at the audited commit.
- **Category:** build architecture, requested modern stack.
- **Impact:** Effect Native already has the correct React 19, Effect v4, typed
  catalog, renderers, Canvas kernel, gallery, and testkit, but its root scripts
  still use Bun for tests, examples, generation, and builds. Building Khala UI
  there before conversion would grow precisely the split toolchain the
  OpenAgents Vite Plus program removed.
- **Standard:** Node, pnpm, and the same exact Vite Plus build-core identity as
  OpenAgents; one checked-in lockfile and root task graph.
- **Recommendation:** make the Effect Native toolchain conversion KU-0. Preserve
  behavioral tests and baselines while replacing Bun-specific runners/builds;
  do not combine that mechanical authority change with new visual behavior.
- **Suggested command:** `$impeccable harden the Khala UI workspace on Node, pnpm, and Vite Plus`

### P2-11 — Route authority changes can invalidate a queued visual pilot

- **Location:** web invariants, removed `apps/astro`, Start `/astro` and
  `/tanstack`, and Forum mount.
- **Category:** information architecture, repository ownership.
- **Impact:** #8848's app path and static-runtime premise disappeared during
  public-site unification. Recreating the deleted app would violate current
  route authority; treating `/tanstack` as a comparison would revive retired
  behavior; silently moving the work into Start would violate the issue's own
  non-goals and measurements.
- **Standard:** validate the owning app and route invariant immediately before
  each pilot. Supersede stale work instead of porting it across an authority
  boundary by implication.
- **Recommendation:** retain the accepted Desktop and Forum pilots. Any future
  `/astro` visual change needs a fresh Start-owned issue and current SSR,
  runtime, route-output, and public-copy proof.
- **Suggested command:** `$impeccable audit the current /astro landing before proposing a new Khala UI pilot`

### P2-12 — Neither upstream nor destination has the complete visual proof gate yet

- **Location:** Arwes unit/CI/perf coverage; OpenAgents route budget and Desktop
  visual tests; Effect Native gallery/testkit.
- **Category:** verification, performance, accessibility.
- **Impact:** Arwes lacks relevant concurrency/hydration/a11y coverage. The web
  route budget is not a full deploy gate, and the checked visual regression
  emphasis is currently Desktop-oriented. Without a cross-renderer fixture
  matrix, frame variants will drift and decorative behavior will bypass SSR,
  forced-color, or reduced-motion constraints.
- **Standard:** one fixture must prove schema, headless resolution, DOM server
  markup/hydration, React Strict Mode, RN disposition, visual baselines,
  keyboard/focus, reduced motion, forced colors, and budgets.
- **Recommendation:** build those gates into the golden gallery before the
  first product pilot and fail catalog bumps that omit a renderer disposition.
- **Suggested command:** `$impeccable audit the Khala UI golden gallery across renderers and preferences`

### P3-13 — Arwes is a frozen reference, not a maintained dependency

- **Location:** README, contribution policy, branch/CI configuration, and Git
  history.
- **Category:** maintenance and supply chain.
- **Impact:** upstream explicitly says it is unmaintained/outdated and asks
  users to fork for personal use; contributions are closed. Current work lives
  on `next`, while CI branch triggers do not represent a dependable supported
  release line.
- **Standard:** production UI dependencies need an active owner, compatible
  release process, upgrade path, and security response.
- **Recommendation:** pin only the audited commit as evidence. Khala UI owns its
  behavior, tests, maintenance, and security from day one.
- **Suggested command:** `$impeccable distill Arwes references into an owned Khala UI specification`

### P3-14 — Responsive and high-contrast behavior is under-specified

- **Location:** frame/background resize logic and docs examples.
- **Category:** responsive design and accessibility.
- **Impact:** ResizeObserver provides fluid geometry, but there is no complete
  evidence for 200% zoom, forced colors, high contrast, narrow containers,
  text expansion, right-to-left layouts, coarse pointers, or RN parity.
- **Standard:** decoration must not reduce readable space, clip focus rings, or
  carry the only state distinction at any supported viewport or user setting.
- **Recommendation:** define responsive frame collapse rules and forced-color
  fallbacks in tokens/core rather than allowing each renderer to improvise.
- **Suggested command:** `$impeccable adapt Khala UI geometry for zoom, forced colors, and narrow containers`

## Positive findings worth preserving

- The low-level decomposition separates geometry, animation, backgrounds,
  effects, text, and sound more clearly than most aesthetic UI libraries.
- The four transition states and manager vocabulary provide a comprehensible
  choreography model.
- Frame CSS variables offer a clean conceptual bridge between geometry and a
  consuming theme.
- Many primitives expose cancellation and tear down observers/subscriptions.
- Decorative React canvases and SVG frames generally use presentation roles.
- Canvas DPR is capped at two, even though logical-coordinate handling needs a
  rewrite.
- Responsive frame definitions contain a rich corpus of geometry ideas.
- MIT licensing permits careful source adaptation, subject to attribution; the
  separately licensed sound assets are clearly disclosed.
- The aesthetic has a recognizable point of view. Khala UI should retain that
  confidence while becoming calmer, denser, and more legible for sustained
  work.

## Khala UI architecture

### One language inside one UI substrate

The intended flow is:

```text
Khala theme roles + typed Khala decoration descriptors
                          │
                          ▼
                Effect Native View tree
          semantic nodes + inert decoration nodes
                          │
                          ▼
            Effect resolution and host services
       viewport / motion / visibility / clock / quality
                          │
          ┌───────────────┼────────────────┐
          ▼               ▼                ▼
    DOM/React 19      React Native       Canvas region
  semantic HTML +     native semantics   Effect-scoped
   CSS/SVG layer      + degradation      scheduler/backend
```

The semantic node remains primary. A frame is not a new button, card, dialog,
or status control. It is a decorative/layout treatment around an existing
semantic child. Domain state and user intent do not enter the visual runtime.

### Proposed vocabulary

Keep the first vocabulary intentionally small:

| Primitive        | Purpose                                       | Initial renderer technique                 |
| ---------------- | --------------------------------------------- | ------------------------------------------ |
| `Frame`          | restrained surface edge geometry              | CSS clip/path or stable SVG overlay        |
| `EdgeAccent`     | corner, header, underline, or segmented line  | CSS pseudo-element/SVG                     |
| `SignalGrid`     | static sparse ambient grid/dots               | CSS/SVG background                         |
| `Glow`           | tokenized luminance emphasis                  | border/background color; no blur blanket   |
| `Reveal`         | optional enter/exit descriptor for decoration | no-op/static first, motion driver later    |
| `AmbientSurface` | one bounded host for optional Canvas ambience | `Host(kind: "canvas")`/render-canvas later |

Prefer extending the existing `Frame` variants from `square | rounded |
arcade` with a few Khala names such as `cut`, `header`, and `signal`, rather
than importing Arwes's fictional names or adding nine frames at once. Names
describe design behavior, not source-library lineage.

### Geometry kernel

The geometry kernel should be pure and renderer-neutral:

```ts
type DimensionExpr =
  | { readonly _tag: "Px"; readonly value: number }
  | { readonly _tag: "Percent"; readonly value: number }
  | { readonly _tag: "Add"; readonly left: DimensionExpr; readonly right: DimensionExpr }
  | { readonly _tag: "Subtract"; readonly left: DimensionExpr; readonly right: DimensionExpr }
  | { readonly _tag: "Multiply"; readonly value: DimensionExpr; readonly factor: number }
  | { readonly _tag: "Min"; readonly values: ReadonlyArray<DimensionExpr> }
  | { readonly _tag: "Max"; readonly values: ReadonlyArray<DimensionExpr> }
  | {
      readonly _tag: "Clamp";
      readonly min: DimensionExpr;
      readonly value: DimensionExpr;
      readonly max: DimensionExpr;
    };
```

This is illustrative, not a checked-in API. The production schema should use
Effect Schema, bounded numeric ranges, deterministic evaluation, explicit
division-by-zero errors if division is needed, and property tests. Built-in
definitions should compile to renderer-friendly geometry ahead of time where
possible. No parser for arbitrary strings is needed unless a real authoring
case appears; a typed AST is safer and simpler.

### Choreography service

The useful Arwes state vocabulary can be retained without copying its runtime:

```text
exited ── enter ──▶ entering ── complete ──▶ entered
  ▲                                                │
  └──────────── complete ◀── exiting ◀── exit ─────┘
```

Required properties:

- states and manager modes are closed schemas;
- Clock, scheduler, deterministic ID, and seeded random services are injected;
- every running transition belongs to an Effect Scope;
- interruption has a specified result rather than relying on incidental DOM
  state;
- `parallel`, `sequence`, `stagger`, and `switch` have bounded semantics;
- reverse order is data, not mutation of a shared child array;
- reduced motion resolves immediately to the stable target state and allocates
  no timer, animation, listener, or frame loop;
- hidden/offscreen/power-constrained hosts may suspend ambient animation without
  suspending semantic state;
- server rendering always emits stable end-state content and deterministic
  identifiers; and
- React sees one resolved external store and never owns choreography state.

The state machine and manager scheduler are good candidates for a small TLA+
model. Prove at least: no child is permanently stranded in an intermediate
state; disposal leaves no scheduled work; a switch has at most one entered
branch; reduced motion reaches the target in zero ticks; and repeated enter/exit
commands converge according to the documented interruption rule. Convert model
counterexamples into regression tests.

### Canvas and ambient effects

Canvas is a renderer capability, not a default background implementation. Use
the existing plan to fold the three-effect renderer kernel into
`@effect-native/render-canvas`; do not create another independent Canvas
lifecycle for Khala UI.

One `AmbientSurface` should own:

- one scheduler for all primitives on that surface;
- logical CSS dimensions separate from backing-store dimensions;
- capped DPR and explicit quality tiers;
- ResizeObserver coalescing;
- IntersectionObserver, Page Visibility, Desktop window-focus, and host power
  signals;
- static rendering for reduced motion and constrained quality;
- seeded deterministic gallery/test mode;
- a maximum active-canvas and frame-time budget; and
- exact Scope teardown of observers, listeners, scheduled frames, GPU/Canvas
  resources, and workers.

Start with dots/grid rendered once. Moving lines, puffs, particles, WebGL, and
OffscreenCanvas are later capabilities, not prerequisites for a recognizable
Khala UI.

### Accessibility contract

1. Semantic content is complete, readable, and correctly ordered without
   decoration, motion, Canvas, audio, or JavaScript.
2. Decorative layers are `aria-hidden`, unfocusable, `pointer-events: none`,
   and absent from the intent graph.
3. Focus rings render above frames and are never clipped by a decorative
   wrapper.
4. Color and glow never carry the only state distinction.
5. Reduced motion prevents work from starting; it does not merely shorten CSS
   duration.
6. Forced-colors mode receives visible borders/separators and no dependency on
   translucent luminance.
7. Text effects never rewrite the accessible label or gate task content.
8. Sound, if it ever ships, is off by default with a visible persistent mute
   and per-category control.

### Brand restraint contract

Khala UI should feel like OpenAgents, not an Arwes clone:

- use the canonical near-black `#05070d` background and Protoss blue/cyan
  semantic roles from `khalaTheme`;
- reserve luminous edges for hierarchy, focus, live state, and high-value
  moments;
- use one signature frame per region, not a frame around every nested card;
- prefer precise solid color and edge contrast over large blurred glows;
- preserve the compact density required by coding sessions, transcripts,
  settings, and status surfaces;
- do not use repeating diagonal stripes, constant flicker, typewriter gating,
  decipher gibberish, or ornamental rounded SaaS cards;
- keep task copy direct and ordinary; the visual system supplies atmosphere
  without turning every label into lore; and
- make absence of motion/Canvas/audio look intentional, not degraded.

## Non-negotiable invariants

1. Khala UI never becomes a second application state, theme, intent, or
   component authority beside Effect Native.
2. No Arwes runtime package is a production dependency.
3. No `eval`, `new Function`, HTML string insertion, or Electron CSP weakening.
4. Semantic content exists before and independently of decoration.
5. Decoration is inert and can be removed without changing behavior.
6. `khalaTheme` and `@effect-native/tokens` remain the only theme authority.
7. Every new catalog node declares support, degradation, or unavailability in
   DOM, React DOM, RN, headless, and Canvas contexts.
8. Motion and ambient work are Scope-owned, deterministic under test, and
   exactly disposable.
9. Reduced motion skips scheduler creation and reaches a stable state.
10. SSR markup, keys, IDs, and initial geometry are deterministic and hydrate
    without changing semantic order.
11. There is at most one active visual scheduler per rendered surface.
12. No audio asset is copied from Arwes; any future sound has documented rights
    and an explicit opt-in product contract.
13. Product controls keep their existing semantic primitive; Khala UI wraps or
    lowers them but does not replace them with decorative lookalikes.
14. `/tanstack` remains only the current compatibility redirect to `/astro`; it
    does not regain independent product, comparison, or Khala UI authority.

If implementation changes one of these statements, update the owning
`INVARIANTS.md` and add the corresponding test, model note, or explicit
renderer-boundary exception in the same change.

## Recommended implementation order

### KU-0 — Toolchain and ownership baseline

**Goal:** make the implementation home match the selected modern stack before
adding behavior.

- Convert `OpenAgentsInc/effect-native` from Bun-rooted scripts to exact Node,
  pnpm, and Vite Plus authority in a dedicated change.
- Preserve Effect v4, React 19, the existing typed catalog, renderer boundaries,
  gallery, testkit, and baseline behavior.
- Add root `AGENTS.md`/`INVARIANTS.md` in that repository if still absent, so
  catalog and renderer policy is owned next to implementation.
- Record exact build-core pins, a pnpm lockfile, declared task inputs/outputs,
  and clean-machine CI.
- Keep the OpenAgents vendored snapshot unchanged until a complete upstream
  commit passes the vendoring guard.

**Exit:** the same gallery/test/typecheck behavior passes through Node/pnpm/Vite
Plus with no Bun authority and no Khala UI feature delta.

### KU-1 — Language contract and golden gallery

**Goal:** specify the system before styling product surfaces.

- Name the working language Khala UI and define its non-goals, vocabulary,
  token roles, frame density rules, accessibility contract, renderer capability
  matrix, and performance budgets.
- Add gallery fixtures for semantic content with and without every decoration.
- Capture viewport, zoom, forced-color, light/dark policy, reduced-motion,
  keyboard/focus, server-markup, hydration, RN, and headless expectations.
- Add a provenance ledger mapping each Arwes idea used, source commit, license,
  and whether behavior or code was adapted.

**Exit:** a reviewed static spec and failing/empty fixtures define the target;
no application depends on new nodes.

### KU-2 — Deterministic geometry and tokens

**Goal:** build the visual alphabet without DOM or animation coupling.

- Extend canonical tokens with bounded edge widths, cut sizes, accent lengths,
  luminance roles, density, and ambient quality.
- Implement pure typed geometry with property tests and deterministic snapshots.
- Add only three initial frame motifs: cut-corner surface, restrained header
  line, and underline/signal separator.
- Define responsive collapse: decoration simplifies before it reduces content
  width or clips focus.

**Exit:** the same geometry inputs yield deterministic logical outputs; invalid
dimensions fail explicitly; no string parser, DOM, Canvas, or motion exists.

### KU-3 — Static renderer lowerings

**Goal:** make the language real and useful without motion.

- Extend existing `Frame`/`Glow` vocabulary in Effect Native core rather than
  adding parallel controls.
- Lower DOM frames to semantic container + inert CSS/SVG layer with stable
  nodes and no runtime HTML strings.
- Add explicit RN equivalents or documented degradation.
- Render static dots/grid through CSS/SVG only if they meet contrast and bundle
  budgets.
- Prove server-visible content, hydration stability, Strict Mode cleanup,
  keyboard/focus, forced colors, 200% zoom, and reduced-motion equality.

**Exit:** the golden gallery passes across supported renderers and the static
bundle delta is within an agreed budget.

### KU-4 — Product pilots

**Goal:** validate the grammar in real work without touching critical flows.

Pilot in this order:

1. **Desktop Project Home coding-session surface.** Apply one frame treatment
   and one status/header accent. Do not touch boot, sidebar, composer,
   transcript/timeline, terminal, review/decision/update flows, or IPC.
2. **Desktop Settings status articles.** Validate dense text, keyboard, zoom,
   and state colors without additional motion.
3. **Retained web `/forum`.** Apply the same Effect Native definitions to a
   bounded board card/breadcrumb/status band and at most one static backdrop.
4. **Retired Astro landing pilot.** #8848 is superseded: `apps/astro` was
   deleted and `/astro` moved into Start before activation. Do not recreate the
   app or silently transfer the static-runtime scope. Open a new Start-owned
   issue if the authoritative landing later needs Khala UI work.

Measure Desktop renderer bytes and startup against the current checked baseline
(approximately 401 ms first paint and 444 ms shell mounted on the recorded
machine), plus web route output and interaction/paint metrics. Treat those
numbers as point-in-time baselines, not universal budgets.

**Exit:** each pilot passes visual, accessibility, startup/bundle, keyboard, and
headed smoke review; owner acceptance is recorded before wider rollout.

**Pilot 1 implementation receipt (2026-07-15):** Desktop Project Home is
implemented on [#8845](https://github.com/OpenAgentsInc/openagents/issues/8845)
with Effect Native `effect-native/v42`. The bounded static frame, continuous
cut-corner perimeter and header accent, exact startup/bundle A/B,
security/accessibility boundaries, and before/after images are preserved in the
[Project Home pilot receipt](./2026-07-15-khala-ui-desktop-project-home-pilot-receipt.md).
Owner review corrected the shared header join in effect-native#95 and removed
the competing top stroke in effect-native#96; neither fix is an app-local CSS
shim. The ordered #8846 Settings pilot remains gated on recorded owner visual
acceptance of the corrected artifact.

### KU-5 — Effect-owned choreography

**Goal:** add intentional motion after static correctness.

- Model transition and manager semantics.
- Implement Clock/Fiber/Scope-owned choreography with deterministic IDs and
  tests for interruption/disposal.
- Use canonical 150–350 ms motion roles; default to opacity/transform/stroke
  progression, not layout animation.
- Keep server content in its final visible state. Attach motion only after a
  committed client mount.
- Begin with a single frame assembly or status transition in the gallery, then
  the pilot surfaces. No page-load obstacle course.

**Exit:** model properties and regression tests pass; Strict Mode creates no
duplicate subscription/timer; reduced motion creates zero scheduled work; no
content waits for animation.

### KU-6 — Bounded Canvas ambience

**Goal:** add depth where CSS/SVG cannot meet the accepted design.

- Use `@effect-native/render-canvas` and its Effect lifecycle.
- Start with one dots/grid primitive, one canvas per surface, static reduced
  mode, deterministic seeds, and quality tiers.
- Enforce resize/DPR correctness, visibility/focus/power pause, frame-time and
  memory budgets, and exact teardown.
- Pilot only in a spacious hero, empty state, or shell background—not behind a
  dense transcript/editor for the entire session.

**Exit:** no startup regression outside the agreed budget, no background work
while hidden/offscreen, no renderer security-boundary change, and a stable
fallback for unsupported hosts.

### KU-7 — Experimental effects, text, and sound

**Goal:** explicitly decide whether these features belong in the product.

- Pointer illumination requires local Pointer Events, cached geometry, rAF
  coalescing, coarse-pointer/reduced-motion disablement, and CSS fallback.
- Decorative text reveal requires complete stable accessible text, an inert
  duplicate, a short bounded label, and no workflow gating.
- Audio requires a separate decision, original licensed assets, default-off
  preference, gesture unlock, category volumes, visible mute, abortable load,
  complete disposal, and Desktop CSP/protocol integration review.

**Exit:** each feature has a standalone product justification and proof packet.
“Arwes has it” is not justification. Omitting all three is a valid final state.

## Verification matrix

| Layer             | Required proof                                                                                                       |
| ----------------- | -------------------------------------------------------------------------------------------------------------------- |
| geometry          | unit + property tests, bounded numeric inputs, deterministic snapshots, no parser/eval                               |
| core catalog      | schema round-trip, exhaustive tag/capability disposition, no domain state                                            |
| SSR/hydration     | meaningful server HTML, stable IDs/keys/styles, no hydration warnings, no content gating                             |
| React 19          | Strict Mode double-mount/replay tests, one store, no leaked Scope/listener/timer/observer                            |
| DOM accessibility | axe/semantic assertions, keyboard/focus, 200% zoom, forced colors, reduced motion, no-JS                             |
| RN                | explicit visual/semantic equivalent or named degradation; screen-reader and dynamic-type fixtures                    |
| Canvas            | logical/DPR correctness, resize, deterministic seed, offscreen/hidden pause, resource teardown, frame/memory budgets |
| Electron          | unchanged CSP/sandbox/protocol boundary, no IPC expansion, build + `smoke:react`, headed keyboard/a11y review        |
| web               | route budget wired to the deploy gate, SSR/browser visual snapshots, `/tanstack` redirect contract unchanged          |
| vendoring         | complete Effect Native upstream commit, catalog bump, all package commit fields, guard/freshness checks              |
| brand             | golden-gallery review at all densities; restrained use; no glow/stripe/frame proliferation                           |

For Desktop pilots, the proportional command set is the focused
adapter/workbench tests, accessibility and design-conformance tests, Electron
boundary tests, typecheck, full tests, build, React smoke, and startup A/B. For
web pilots, add Start/Forum tests, server output and hydration assertions,
browser visual snapshots, route budget, and monolith routing smoke.

## Systemic risks

### Aesthetic overreach

The largest product risk is not technical failure; it is successful
over-application. Once frames and glow are easy, every card can acquire them.
The gallery must include density and nesting limits, and design review must ask
what can be removed. Signature geometry belongs on shells, live status, major
regions, and rare moments—not ordinary paragraphs and controls.

### Renderer drift

DOM can reproduce complex edges cheaply while RN or native renderers may need
different primitives. “One component set” does not mean pixel-identical
techniques; it means one semantic/visual contract with explicit equivalents.
Catalog changes must fail when a renderer has no support/degrade/unavailable
entry.

### Lifecycle duplication

React, Effect, CSS animations, Web Animations, Canvas, and Electron can each
schedule work. Khala UI must choose one owner per capability: Effect resolves
state and lifecycle; React commits DOM; CSS/Web Animations execute bounded DOM
transitions; render-canvas owns frame clocks. No component creates its own
permanent global loop.

### Toolchain split

The destination repository currently has the right architecture but an
outgoing Bun-rooted task graph. Adding visual work before Vite Plus conversion
would make later mechanical migration riskier and undercut the user's explicit
modern-stack goal. KU-0 is a real dependency, not housekeeping to postpone.

### Vendoring lag

OpenAgents currently vendors Effect Native at the audited upstream tip and
guards against partial commit/catalog bumps. Khala UI must land upstream first,
then be re-vendored atomically. Never hand-edit only the vendored core or DOM
renderer to make a pilot work.

## Recommended command sequence for future implementation agents

These are intent prompts, not dispatch authority. Create and claim bounded
issues first.

1. `$impeccable distill the Khala UI language contract and golden gallery`
2. `$impeccable harden the Effect Native workspace on Node, pnpm, and Vite Plus`
3. `$impeccable colorize Khala UI geometry with canonical khalaTheme roles`
4. `$impeccable adapt static Khala UI frames across DOM and React Native`
5. `$impeccable audit the static gallery for accessibility, responsiveness, and renderer parity`
6. `$impeccable animate the accepted frame transitions through Effect Scope`
7. `$impeccable optimize bounded Canvas ambience against startup and frame budgets`
8. `$impeccable polish the accepted Khala UI pilots`

After each implementation tranche, rerun the applicable audit and compare the
same gallery fixtures and product baselines. The design language is ready to
expand only when the static, reduced-motion, and unsupported-capability states
feel as deliberate as the full treatment.

## Final recommendation

Arwes is valuable because it exposes a coherent set of first principles, not
because its packages are reusable in this stack. Its strongest contribution is
the realization that a few geometric rules can carry an identity across
surfaces and that motion should assemble those rules rather than decorate them
arbitrarily.

Khala UI should take that lesson and build a smaller, stricter system:

- owned by Effect Native;
- branded by `khalaTheme`;
- compiled with Node/pnpm/Vite Plus;
- semantic and static before decorative and animated;
- secure under the existing Electron CSP;
- deterministic under SSR, Strict Mode, and tests;
- respectful of reduced motion, forced colors, density, and sustained work;
- measured in real Desktop and Forum pilots; and
- free to omit Canvas, text tricks, pointer light, and sound when they do not
  improve the product.

That creates an original OpenAgents design system rather than a modernized
Arwes clone—and gives web, Desktop, mobile, and future Canvas renderers one
language they can honestly share.
