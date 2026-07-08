# Three.js Graphics Skills Audit — porting `Threejs-Awesome-Graphics-Agent-Skills` into the Verse

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-22

Reference repo (read-only): `projects/repos/Threejs-Awesome-Graphics-Agent-Skills`
(`scottstts/Threejs-Awesome-Graphics-Agent-Skills`) in the workspace
`projects/` lane.

Our graphics stack:
- `three-effect` sibling repo, `@openagentsinc/three-effect`
  (`/Users/christopherdavid/work/three-effect`). Effect-first Three.js + Foldkit.
- Verse primitives:
  `packages/core/src/inferenceGatewayPrimitives.ts`
  (`createCracklingArc`, `createGatewayPortal`) and
  `packages/core/src/trainingRun.ts` (the dark Verse scene renderer,
  background `0x050505`, ACES tone mapping).
- Post-processing infra already present:
  `packages/core/src/renderPrimitives.ts`
  (`createEffectComposerResources` wrapping `EffectComposer` + `UnrealBloomPass`
  + `OutputPass`).
- Consumed by `openagents/apps/autopilot-desktop`
  (`src/ui/pylon-network-visualization.ts` feeds `trainingRun` /
  `TrainingRunVisualizationOptions`).

> Workspace rule: `projects/repos/*` is **read-only reference**. This audit
> ports *ideas* into our owned `three-effect` code. It does not vendor or copy
> large chunks of the reference repo.

---

## 1. What the reference repo actually contains

It is an **agent-skill pack**, not a library. Each skill is a `SKILL.md` +
`references/*.md` (and sometimes an `examples/*.js`) describing a graphics
*system* with named perceptual parameters, a no-post baseline, and diagnostics.
The pack's own thesis (README): it teaches the *vocabulary* of sophisticated
graphics by showing exact implementation, not a summary.

Skill catalog (`skills/`):

| Skill | What it is | Relevance to our dark Verse |
| --- | --- | --- |
| `threejs-skill-router` | Decomposes a visual target into the smallest relevant skills. | Meta. Useful as an internal authoring checklist. |
| `threejs-bloom` | HDR bloom: signal ordering, threshold/radius/strength in HDR, single-node vs dual selective bloom, `toneMapped=false` emitters, material-substitution restoration. | **Highest.** Directly fixes "arcs/portals invisible in the dark." |
| `threejs-exposure-color-grading` | Scene-referred exposure: luminance meter, asymmetric eye adaptation, one tone-map owner, generated 32³ display-domain 3D LUT. | High — disciplines the HDR range bloom needs and gives a Verse "look". |
| `threejs-image-pipeline` | Pass ownership: one producer per buffer (HDR/depth/normal/bloom/exposure), DPR/pixel-budget policy, disable paths. | High — the contract our composer wiring should follow. |
| `threejs-procedural-vfx` | Instanced spark pools (12k sprites, per-instance position/velocity/accel/spawn), additive HDR emitters, dense-swap pooling, layered emission hierarchy (spark 80 > projectile 30 > laser 10). | **High.** Better portal sparks / arc-strike particles. |
| `threejs-procedural-fields` | Shared scalar/vector field bundles; domain-warp coordinates not results; different frequency bands per purpose; reuse one field across color/roughness/normal/emission. | Medium — coherent noise for arc jitter, beam flow, portal turbulence. |
| `threejs-screen-space-ambient-occlusion` | GTAO horizon sampling, bent normals, bilateral/temporal reconstruction; AO modulates *indirect* light only. | Medium — grounds pylon stations/avatars in the scene. |
| `threejs-shadow-systems` | Stable cascades + cached clipmap shadows, update budgets, manual invalidation. | Low–medium for our mostly-emissive scene. |
| `threejs-procedural-materials` | Atlas filtering, specular AA, frame PBR, per-instance dissolve. | Medium — PBR for pylon stations / structural meshes. |
| `threejs-procedural-animation` | Analytic timelines, springs, quaternion alignment, staging, debris motion. | Medium — arc strike staging, portal open/close. |
| `threejs-camera-direction` | Authored lenses/shots, chase/orbit/side rigs, body-relative frames, handoffs, floating origins. | Medium — Verse camera rigs / cinematics. |
| `threejs-temporal-surfaces` | Persistent touch/interaction history (ping-pong), reduced-res blur, refraction composite. | Low — niche (frost/wake trails). |
| `threejs-procedural-geometry` | Profile-swept rails, semantic mesh writers, material groups, UV density. | Low–medium — structural Verse geometry. |
| `threejs-procedural-vegetation` / `-architecture` / `-planets` | Growth hierarchies / massing+façade grammars / spherical terrain+biomes. | Low for the Verse (no foliage/terrain/cities today). |
| `threejs-spectral-ocean` / `threejs-water-optics` | FFT ocean / analytic wave optics + refraction. | Not applicable. |
| `threejs-atmosphere-aerial-perspective` / `threejs-volumetric-clouds` | Rayleigh/Mie sky + depth scattering / raymarched clouds. | Not applicable (no sky/atmosphere). |
| `threejs-raymarched-space-effects` | Curved-ray black holes, accretion disks, wormholes, bounded raymarch quality. | Aspirational — striking but heavy; see "don't bother for now". |
| `threejs-visual-validation` | Fixed-view captures, diagnostic mosaics, seed/scale sweeps, GPU evidence. | High as *process* — pairs with our screenshot/gallery tooling. |

Concrete file pointers used below:
- `skills/threejs-bloom/references/hdr-bloom-system.md` — signal order, dual
  selective-bloom transaction, emissive hierarchy, `toneMapped=false`.
- `skills/threejs-procedural-vfx/references/procedural-vfx-system.md` and
  `skills/threejs-procedural-vfx/examples/reentry-plasma/reentry-plasma.js` —
  instanced spark pool, additive emitter shader, fbm noise, HDR multipliers.
- `skills/threejs-exposure-color-grading/references/scene-referred-color-pipeline.md`
  — luminance meter, adaptation, 32³ LUT recipe.
- `skills/threejs-image-pipeline/references/production-image-pipeline.md` —
  buffer/ownership table, DPR budget.
- `skills/threejs-procedural-fields/SKILL.md` — shared-field discipline.
- `skills/threejs-screen-space-ambient-occlusion/SKILL.md` — GTAO failure modes.

---

## 2. Where we are today (the real pain)

- **`createCracklingArc`** (`inferenceGatewayPrimitives.ts:172`) draws strands
  as `Three.LineBasicMaterial` — 1px GPU lines with `opacity ~0.72`,
  `depthWrite:false`, colors `0x93c5fd`/`0xf8fafc`. No emissive, no glow, no
  thickness. In a `0x050505` scene these are thin and faint — the documented
  pain.
- **`createGatewayPortal`** (`inferenceGatewayPrimitives.ts:307`) builds rings
  as `TorusGeometry` of tube radius `radius*0.018` with `MeshBasicMaterial`
  (unlit), a low-opacity core, and `PointsMaterial` sparks. Pretty but small
  and low-contrast; nothing "blooms."
- **`trainingRun.ts`** already sets `renderer.toneMapping =
  ACESFilmicToneMapping` and an exposure (`trainingRun.ts:3323-3324`), uses
  `MeshStandardMaterial` with *very* low emissive (`0x060708`, `0x30343a`) plus
  `MeshBasicMaterial`/`LineBasicMaterial`, and a 3-light rig
  (`AmbientLight 0.28`, `DirectionalLight 4.1` sun + `0.22` fill,
  `PointLight` accents). **It renders the scene directly and does not use the
  `EffectComposer`/`UnrealBloomPass` from `renderPrimitives.ts`.**

So the bloom *infrastructure* exists (`createEffectComposerResources`) but the
Verse scene path never wires it, and the energy primitives carry no HDR
emissive signal for bloom to pick up. That is the single biggest gap, and it is
exactly what `threejs-bloom` is about.

Also present and reusable:
- `advancedMaterialPrimitives.ts` — transmission/refraction/distort/wobble
  physical materials (no additive-glow or fat-line helper yet).
- `conditionalLinePrimitives.ts` — a `LineMaterial`/`ConditionalLineMaterial`
  with a real `linewidth` uniform (screen-space fat lines).

---

## 3. Ranked adopt list

Each item: (a) what it is, (b) where it plugs in, (c) visual payoff,
(d) effort/risk.

### A1 — Wire selective HDR bloom into the Verse render path  ★ most impactful
- **What:** `threejs-bloom` — establish HDR emissive signal first, then add a
  bloom pass *before* tone mapping; threshold/radius/strength tuned in HDR
  (`hdr-bloom-system.md`). For the Verse, the simplest correct version is a
  single global bloom pass; the dual *selective* model (energy on its own bloom
  layer, base scene on the final composer, with the `try/finally` material
  restoration invariant) is the upgrade once we want arcs/portals to bloom
  without making pylon stations and HUD text glow.
- **Where:** route the Verse render loop through
  `createEffectComposerResources(renderer, scene, camera, { bloom: {...},
  output: true })` (`renderPrimitives.ts:118`) instead of the direct
  `renderer.render` in `trainingRun.ts`. Keep `OutputPass` as the single
  tone-map/output owner (matching `threejs-image-pipeline` "one owner" rule);
  move `ACESFilmicToneMapping` ownership to that pass so we don't double
  tone-map.
- **Payoff:** the crackling arc and gateway portal finally *read* in the dark
  scene — energy looks like energy. This is the single change that makes the
  Verse "pop."
- **Effort/risk:** Medium. Infra exists; the work is render-loop rewiring +
  resize/DPR handling + ensuring HUD/text either live on a non-bloom layer or
  set `toneMapped`/`material` so they don't smear. Version-sensitive
  `UnrealBloomPass` params; validate base frame still reads with bloom
  disabled (the skill's acceptance test).

### A2 — Give energy primitives a scene-relative HDR emissive hierarchy
- **What:** `threejs-bloom` + `threejs-procedural-vfx` emissive hierarchy
  (`spark flash > projectile > laser > ordinary surface`; reference
  multipliers 80/30/10 are relative, not absolute). Push arc strands and portal
  cores into HDR (`color * multiplier`, `toneMapped = false` on pure emitters)
  so the bloom pass in A1 has signal to extract.
- **Where:** `createCracklingArc` strands and `createGatewayPortal`
  core/ring/spark materials in `inferenceGatewayPrimitives.ts`. Add an
  `emissiveStrength`/`hdrBoost` option; set `toneMapped=false` on the unlit
  emitters. Encode a hierarchy: arc strike-peak > portal core > steady ring >
  ambient station.
- **Payoff:** controlled, calibrated glow (bright cores, soft falloff) instead
  of uniform flat lines. Status (`working`/`online`/`offline`) can drive the
  multiplier so a busy gateway visibly burns brighter.
- **Effort/risk:** Low–medium. Pure material/option changes. Risk: multipliers
  are scene-relative — must be tuned against the actual exposure from A1, not
  copied raw.

### A3 — Thicken the crackling arc with screen-space fat lines
- **What:** Replace 1px `LineBasicMaterial` strands with screen-space-width
  lines. We already own the machinery: `conditionalLinePrimitives.ts` ships a
  `LineMaterial`/`ConditionalLineMaterial` with a `linewidth` uniform
  (`three/examples/jsm/lines/LineMaterial.js`). The reference's `procedural-vfx`
  filament shader (`reentry-plasma.js`) shows the complementary idea: a thin
  *core* plus a wider, softer additive *envelope* rather than one flat stroke.
- **Where:** `createCracklingArc` — render each strand as `Line2`/fat line for
  the bright core, optionally add a wider low-opacity additive tube/sheet for
  the glow envelope. Keep the existing seeded jitter/wobble.
- **Payoff:** arcs gain presence and a bright-core/soft-halo profile — lightning
  that reads at a distance instead of hairline scratches.
- **Effort/risk:** Medium. Fat lines need resolution updates on resize and have
  their own draw cost; the core+envelope pair doubles strand draws.

### A4 — Instanced additive spark pool for portal/arc-strike particles
- **What:** `threejs-procedural-vfx` instanced spark contract: a fixed-capacity
  pool with per-instance `startPosition/startVelocity/acceleration/spawnTime`,
  linear size falloff, circular additive sprite, HDR color fading to dark
  (`procedural-vfx-system.md`), plus the **dense-swap** pooling invariant (move
  last live instance into a freed slot, copying *all* custom attributes).
- **Where:** upgrade `createGatewayPortal` sparks (currently a static
  `Three.Points` ring) to an instanced, animated, additive emitter; emit a
  burst at arc-strike moments from `createCracklingArc`. Pair with the existing
  `entityPoolPrimitives.ts`/`mediaParticlePrimitives.ts` rather than a new pool
  type where possible.
- **Payoff:** portals "spit" sparks and arcs throw embers on contact — life and
  motion instead of a static dotted halo.
- **Effort/risk:** Medium. Instancing + per-instance attribute buffers + an
  Effect-friendly update loop. Note the reference's deliberate
  non-physical/non-deterministic quirks (`Math.random`, `a*t²`,
  re-integrated velocity) — use a seeded RNG for our deterministic-evidence
  rule.

### A5 — Exposure/color-grade owner + LUT for a consistent Verse "look"
- **What:** `threejs-exposure-color-grading` — one tone-map owner, optional
  eye-adaptation (asymmetric speed-up/down), and a generated 32³ display-domain
  3D LUT for grade (contrast/saturation/shadow-midtone-highlight tints). Apply
  *after* tone mapping, per the reference's domain warning.
- **Where:** sits right after A1's `OutputPass` in the composer chain
  (`image-pipeline` order: `…→ bloom → exposure → tone map → LUT → FXAA`). A
  fixed grade (no adaptation) is the low-risk first cut; adaptation is optional
  polish.
- **Payoff:** a deliberate, cohesive Verse mood (deep blacks, controlled
  highlights) and a single knob to art-direct the whole scene.
- **Effort/risk:** Medium. Start with a static LUT; skip the luminance
  meter/readback initially (it adds frame-cadence and double-exposure pitfalls
  the reference calls out).

Lower-priority but worth tracking:
- **A6 — GTAO contact grounding** (`threejs-screen-space-ambient-occlusion`):
  modulate *indirect* light only; grounds pylon stations/avatars. Medium
  effort, needs depth+normal MRT (coordinate via `image-pipeline`).
- **A7 — Shared procedural fields** (`threejs-procedural-fields`): one
  domain-warped noise bundle driving arc jitter + beam flow + portal turbulence
  instead of independent per-effect noise ("visual soup"). Low–medium; mostly a
  refactor discipline.
- **A8 — Visual-validation evidence sets** (`threejs-visual-validation`):
  fixed-view captures + diagnostic mosaics + seed/scale sweeps as acceptance
  gates. Pairs with our existing screenshot/gallery tooling; cheap, high
  process value — and the bloom skill *requires* a "base frame still reads with
  bloom off" check that this formalizes.

---

## 4. Don't bother / not applicable

- **Ocean / water optics / atmosphere / volumetric clouds / planets /
  vegetation / architecture** — no terrain, sky, sea, foliage, or cities in the
  Verse. Skip unless the world gains those biomes.
- **Raymarched space effects (black holes, wormholes)** — visually stunning but
  a heavy bounded-raymarch budget for a niche payoff; revisit only for a
  deliberate cinematic set piece, not the everyday Verse.
- **Temporal surfaces (frost/touch history)** — niche ping-pong interaction
  effect; not aligned with current arcs/portals/beams needs.
- **WebGPU/TSL/`RenderPipeline` specifics** — several references target the
  WebGPU `RenderPipeline`/`PostProcessing` node path. Our stack is the WebGL
  `EffectComposer` path; port the *ordering and ownership ideas*, not the
  node-API specifics.
- **Verbatim multipliers/params** — every reference number (bloom
  threshold/radius, spark multipliers 80/30/10, exposure min/max) is
  scene-relative. Treat them as *relationships* and re-tune against our actual
  exposure; do not hardcode.
- **Installing the pack as agent skills** — the repo ships an installer
  (`npx threejs-awesome-graphics-agent-skills install`). Not in scope; we mine
  ideas, we don't install third-party skills into our agents.

---

## 5. Recommended sequence

1. **A1** (wire the composer + bloom into the Verse render path) — unlocks
   everything else and is the biggest single visual win.
2. **A2** (HDR emissive hierarchy on arcs/portals) — gives A1 something to
   bloom; ship together.
3. **A3** (fat-line arcs with core+envelope) — makes lightning read.
4. **A4** (instanced additive sparks) — adds motion/life.
5. **A5 + A8** (grade/LUT owner + validation gates) — lock a consistent look
   and prevent regressions.

**Most impactful single change:** A1 — route the Verse scene through
`createEffectComposerResources` with a bloom pass (and let `OutputPass` own tone
mapping), paired with A2's HDR emissive on the arc/portal emitters. Today the
energy primitives carry no HDR signal and the scene never runs a bloom pass, so
the crackling arc and gateway portal stay thin and faint in the `0x050505`
scene. Bloom + HDR emissive is the difference between "hairline scratches" and
"energy that glows in the dark."
