# What we adapt, which kernel, and the build order

Researched 2026-10-14. Part of [CAD research](README.md). Sources are in
[classcad.md](classcad.md) and [ecosystem.md](ecosystem.md); facts about
our own code come from `origin/main` on 2026-10-14. **[V]** verified,
**[I]** inferred.

## The first target: a rover in our own stack (#11627)

The owner wants an equivalent of
[solidcraft-rover v1.0.0](https://github.com/0xlillith/solidcraft-rover/releases/tag/v1.0.0)
built in our stack: a scientific rover built entirely by an agent through
typed CAD commands, shipped as an editable model plus STEP and 3MF, renders
and a build video, under MIT OR Apache-2.0. Theirs has 1,395 bodies, 576
sketches, 36 components, 17 revolute joints and 3,324 history entries [V].

What the rover needs, and who can do it today in Rust or WASM:

| Need | SolidCraft crates (Rust, truck) | Truck alone (Rust) | smlibwasm / USD BRep (SMLib) | opencascade.js (OCCT) | Manifold / csgrs | ClassCAD WASM |
| --- | --- | --- | --- | --- | --- | --- |
| Sketch + extrude/revolve | Yes [V] | Yes, low level [V] | Yes [V] | Yes [V] | Extrude only, meshes [I] | Yes [V] |
| Booleans | Yes, with fixes and provenance [V] | Yes, fragile [V from SolidCraft's patches] | Yes [V] | Yes [V] | Mesh only [V] | Yes [V] |
| Fillets, chamfers | Yes, variable radius, corners [V] | `fillet` module on master, not in released crates [V/I] | Yes, G2/G3 [V] | Yes [V] | No [I] | Yes [V] |
| Patterns | Rectangular, circular, path [V] | By copying transforms (our code) [I] | Grid, polar, hex locations [V] | By transforms [I] | By transforms [I] | Yes [V] |
| Components and mates/joints | Yes, 11 joint commands, limits, drive [V] | No [I] | No constrained assemblies yet [V] | No assembly solver (XCAF structure only) [I] | No | Yes, full set [V] |
| STEP export | Yes, validated and read back in CI [V] | Yes (`truck-stepio`, partial) [V] | Yes [V] | Yes [V] | No [V] | Yes, AP203/214/242 [V] |
| 3MF export | Yes [V] | No [I] | No (STL, glTF) [V] | No [V] | Manifold yes [V] | No [V] |
| Mass properties | Tessellation-based, with density [V] | Mesh analyzers [I] | Exact surface integrals, inertia matrix [V] | Exact (GProp) [I] | Mesh [I] | Volume and centre of gravity [V] |
| URDF export | Yes, mass and inertia per link [V] | No | No | No | No | No |
| wasm32 in CI | Yes [V] | Yes [V] | Yes, ~1.9 MB brotli [V] | ~2.4-9 MB brotli [V] | Yes [V] | Yes, ~55 MB [V] |
| Licence for us | MIT OR Apache-2.0 [V] | Apache-2.0 [V] | Apache-2.0 + MIT [V] | LGPL-2.1 + exception [V] | Apache-2.0 / MIT [V] | Paid distributor licence [V] |

Only SolidCraft's crates and ClassCAD cover every column; only SolidCraft's
are open, Rust, and on our wgpu version [V for each fact, I for the
conclusion].

## Kernel recommendation

**Build on SolidCraft's Rust crates (and through them, Truck), pinned to a
git commit, behind a thin `cad` boundary of our own. Keep SMLib (USD BRep,
Apache-2.0) as the precision and fallback kernel. Do not license ClassCAD
now. Do not use OCCT.** [I, reasoning below]

Why:

1. **It already does the rover.** Booleans, fillets, patterns, joints, STEP,
   3MF and URDF are working and tested, and the rover itself proves the
   agent-through-commands workflow at 3,324 steps [V].
2. **It is our stack.** Pure Rust, wgpu 30 like `crates/verse` and
   `crates/verse-pbr`, wasm32 green in its CI, WebGPU with WebGL2 fallback
   like our `web` feature [V both sides]. No C++ toolchain enters our wasm
   build [I].
3. **The licence is the one we want to ship the rover under** (MIT OR
   Apache-2.0) [V].
4. **Fornjot is dead and CADmium is archived** [V]; writing our own kernel
   on raw Truck would repeat SolidCraft's week of blend, boolean-guard and
   provenance work [I].

Risks and what we do about them [I]:

- **Churn.** It is one week old and changes by the minute. Pin a commit,
  depend only on `geom`, `sketch`, `kernel`, `doc` and `engine`, wrap them in
  our own `cad` crate so callers never see SolidCraft types, and bump the pin
  deliberately with our tests as the gate.
- **Robustness.** Truck booleans are the weak spot. Our checks catch
  failures per edit; persistent ones become upstream issues or patches (we
  contribute back; they accept contributions [V, CONTRIBUTING.md exists]).
- **Inexact mass properties.** SolidCraft measures from tessellation [V].
  For Lagrange 2 latch loads and inertia we cross-check against SMLib's exact
  integrals (smlibwasm or native USD BRep), and fail the check when they
  disagree beyond a tolerance. Known SMLib caveat: tighten its accuracy below
  the 1e-3 default for circle-trimmed faces [V, KNOWN_ISSUES].
- **Upstream disappears or diverges.** Both licences let us fork. If Truck
  hits a wall, SMLib is a mature exact kernel that is now Apache-2.0 [V];
  the `cad` boundary is what makes swapping possible.

Why not the others [I]:

- **ClassCAD.** The best history and assembly layer we saw, but closed, a
  55 MB WASM engine, a key per origin, and a distributor licence (order of
  CHF 5,000 a year) because openagents.com users would reach it [V for the
  facts]. Revisit only if we need what it has and SolidCraft does not (for
  example drawings with hidden lines) and the owner wants a partner.
- **OCCT / opencascade.js.** Capable, but LGPL obligations, a C++ build, 2.4
  to 9 MB of wasm, and still no assembly solver [V for the facts].
- **Manifold.** Excellent mesh booleans, Apache-2.0; keep it in mind for
  imported game meshes and printing, not for parts [I].

## What to learn and adapt

### 1. Part (history) vs Solid (direct)

ClassCAD's split is right [V for the split, I for the judgement]:

- A **history** part is a list of features that replays; every number can
  change later and face references survive. This is what an agent edits by
  talking.
- A **direct** solid is code that rebuilds from scratch; fast, no
  bookkeeping. This is what generators (a truss of N bays, a tank of volume
  V) want.
- They meet in one place: a direct block runs as one feature inside a
  history (ClassCAD's `EntityInjection`). We should make that block
  recompute when its inputs change, which ClassCAD does not do yet [V].

SolidCraft's timeline is the history side; a generator is a function that
emits commands, so both live in one document [I].

### 2. Typed, versioned documents in the Lab pattern

Our Labs (Video, Game, Robot) share one shape: a typed JSON document
`<kind>.v1` with a schema generated from Rust, immutable versions with
parent, author, one-line summary, digest and measured stats, stored
privately, and a single write path `*_edit(doc, patch, summary)` that checks
before saving [V, `docs/game-lab/README.md`, `docs/lab/README.md`,
`crates/robot-lab`].

Proposed kinds [I]:

- **`part.v1`:** parameters (name, value, unit, bounds, plain description),
  material and density, and a feature list. Each feature is one SolidCraft
  command id plus its JSON parameters (`solid.extrude`, `solid.fillet`, …),
  or a `generator` block (direct code by name and inputs). Derived and
  stored with each version: volume, area, mass, centre of mass, inertia
  tensor, bounding box, a mesh digest.
- **`assembly.v1`:** components (each a `part.v1` reference pinned to a
  version, as ClassCAD PDM pins references [V]), occurrences with
  transforms, joints (rigid, revolute, slider, cylindrical, planar, ball,
  with limits), and for Lagrange 2, ports (frame, type, rated latch load) as
  `docs/spacecraft-design/sim-mapping.md` already specifies [V].

The document is ours; `.solidcraft`, STEP, 3MF, glTF and URDF are exports.
Feature ids in the document are SolidCraft's command ids, so the mapping is
one to one and an agent's MCP habits carry over [I].

### 3. A CAD Lab: change a part by talking

Chat tools with a `cad_` prefix, offered to the same Labs turn as the
others (`crates/openagents-web/src/labs.rs`), routed by typed tool choice,
never keywords [V for the pattern]: `cad_list`, `cad_open`, `cad_edit`,
`cad_versions`, `cad_compare`, `cad_export`, `cad_notes` [I].

`cad_edit` checks every edit before it saves, in order [I]:

1. **Schema:** values exist and are in bounds.
2. **Rebuild:** the whole history replays with no failed feature.
3. **Valid solid:** each body is closed and manifold, no
   self-intersections, STEP export reads back (SolidCraft's oracle does this
   [V]).
4. **Numbers:** mass properties are computed and stored; where the part
   states expectations (a mass budget, a centre-of-mass box), they hold. For
   flagged parts, SMLib's exact mass properties agree with SolidCraft's.
5. **Interference:** no two occurrences overlap unless the joint says so
   (SolidCraft has `inspect.interference` [V]).
6. **Pictures:** deterministic renders from fixed views, stored with the
   version, so Compare shows before and after (classcad.ai's approach [V]).

A refused edit returns the reason in plain words and saves nothing, as Game
Lab does [V].

### 4. Rendering in Verse

Tessellate in the `cad` crate, hand triangle meshes with per-face ids to
`verse-pbr` [I]. We already import glTF but have no glTF writer [V]; add one
for exports (the `gltf` crate's `gltf-json` can write) [I]. Keep face ids
through tessellation so a click selects a face, which is ClassCAD's
"tessellation that keeps topology identity" [V for ClassCAD]. SolidCraft's
own wgpu renderer is a reference, not something we import [I].

### 5. Exports

STEP (exact, for other CAD), 3MF (printing), glTF (web and Verse), URDF
(Robot Lab, MuJoCo), `.solidcraft` (open in SolidCraft) [I]. All but glTF
come from the SolidCraft crates [V].

### 6. Feeding Robot Lab and Lagrange 2

- **Robot Lab** imports a checked subset of URDF with STL meshes today
  (`crates/robot-description`) [V]. SolidCraft's URDF export writes exactly
  that: per-link STL, mass and inertia, joints [V]. So an `assembly.v1` with
  joints becomes a `robot.v1` with no new importer [I].
- **Lagrange 2** needs parts with real mass, centre of mass, full inertia
  tensors (our `physics` body uses diagonal inertia today [V]), and ports for
  latching [V, sim-mapping.md]. CAD gives the tensor; we diagonalize into
  principal axes for `physics`, or extend it [I]. The spacecraft document's
  derived values (parallel-axis sums, thrust-line offset) become CAD Lab
  checks on an `assembly.v1` [I].

## Build order, smallest first

Sizes: S is up to half a day of agent time, M up to two days, L up to a
week [I].

1. **Spike (S).** New crate `crates/cad` with SolidCraft's crates as a git
   dependency pinned to a commit. Native and `wasm32-unknown-unknown`
   builds. Headless test: build a filleted plate with a hole pattern through
   the command engine, assert volume against the analytic value, export STEP
   and 3MF, read the STEP back. Run a handful of SolidCraft's oracle cases
   in our CI. *Proves the dependency works in our tree.*
2. **`part.v1` and `assembly.v1` (M).** Schemas generated from Rust,
   private versioned store reused from `robot-lab`'s store, checks 1 to 5
   above, a CLI `openagents cad …` that opens, edits by JSON Patch, lists
   versions and exports. *The documents the rover is written in.*
3. **The rover, #11627 (L).** An agent builds a scientific rover through
   `cad` commands only: chassis, rocker-bogie suspension with revolute
   joints, wheels by circular pattern, mast and camera head, arm, solar
   wings. Ship the `assembly.v1` history, `.solidcraft`, STEP, 3MF, URDF,
   renders and a build video under MIT OR Apache-2.0. Size it below theirs
   first (tens of components, not 36), then grow. *Equivalent of the
   SolidCraft rover in our stack.*
4. **Verse rendering (M).** CAD meshes in `verse-pbr` with face ids, on web
   (WebGPU and WebGL2) and native; build-video playback by replaying
   history. *Lets the rover drive in Verse.*
5. **CAD Lab in chat (M).** `cad_*` tools in the Labs turn, inline card with
   renders, Compare and notes. *"Make the wheels 20% bigger" by talking.*
6. **Robot Lab bridge (S).** URDF export → `robot-description` import →
   `robot.v1`; the rover's joints drive in Robot Lab and MuJoCo. *Rover
   physics.*
7. **Exact mass cross-check (M).** SMLib (smlibwasm or native USD BRep)
   computes exact mass properties for flagged parts; disagreement fails the
   check. *Needed before Lagrange 2 trusts latch loads.*
8. **Lagrange 2 parts (M, after #11625 lands its document).** Ports and
   latch loads on `assembly.v1`; the factory reads assemblies made in CAD
   Lab.

The first three steps are what #11627's builder needs.

## Open questions for the owner

1. **Partner with AWV?** We recommend no licence now. ClassCAD's closed
   layer (history, references, solvers, drawings) is the best we saw, but a
   distributor licence and a 55 MB keyed engine cut against our open, Rust,
   ship-it-ourselves direction. A conversation with AWV about their
   now-Apache SMLib work (smlibpy, smlibwasm) costs nothing.
2. **Depend on SolidCraft, fork it, or contribute upstream?** We recommend a
   pinned dependency plus upstream contributions, with the right to fork
   kept in reserve.
3. **Scope.** Parts and assemblies for our own worlds (rover, spaceship,
   robots), or a general CAD product for users? The build order above is the
   first; a user-facing CAD app is a much larger commitment.
4. **Exactness bar.** Are tessellation-based mass properties good enough for
   games, with SMLib exactness only for Lagrange 2, or exact everywhere?
5. **Rover size.** Match their 36 components and 17 joints, or a smaller
   first rover that is fully simulated in Robot Lab?
