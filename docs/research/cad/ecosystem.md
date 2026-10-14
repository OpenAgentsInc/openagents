# The browser and programmable CAD ecosystem

Researched 2026-10-14. Part of [CAD research](README.md). ClassCAD and its
family are in [classcad.md](classcad.md).

Marks: **[V]** read on the linked page, repo, release or registry entry on
2026-10-14. **[I]** our inference. Stars and dates are as of that day.

## Comparison table

| Name | Kind | Language | In the browser | Licence | Exact B-rep | STEP | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [SolidCraft](https://github.com/bherbruck/solidcraft) | Parametric CAD app + MCP | Rust | Yes, wasm, wgpu (WebGPU, WebGL2 fallback) | MIT OR Apache-2.0 | Yes (truck, extended) | Read (own reader) and write | Created 2026-10-07; v0.10.1 on 2026-10-14; 2,270 stars; 350 commands; URDF export. **Closest thing to what we want.** [V] |
| [ClassCAD](https://classcad.ch) | Headless CAD engine | C++ runtime, own language | Yes, 55 MB WASM | Proprietary, keyed; distributor licence | Yes (SMLib) | R/W, AP203/214/242 | History, references, assemblies, solvers [V] |
| [USD BRep / SMLib](https://github.com/NVIDIA-Omniverse/usd-brep) | NURBS B-rep kernel | C++, Python | Via [smlibwasm](https://smlibpy.classcad.ch/docs/javascript), ~1.9 MB brotli | Apache-2.0 | Yes, exact integrals | R/W (STEP, IGES, JT fixtures) | Released by NVIDIA 2026-10-08; not taking contributions [V] |
| [smlibpy](https://github.com/awv-informatik/smlibpy) | Code-CAD library on SMLib | Python, JS (wasm) | Yes (smlibwasm) | MIT (+ Apache-2.0 kernel) | Yes | R/W | v0.1; no constrained assemblies [V] |
| [OCCT](https://github.com/Open-Cascade-SAS/OCCT) | B-rep kernel | C++ | Via opencascade.js | LGPL-2.1 + OCCT exception | Yes | R/W, plus IGES, glTF | V8.0.1, 2026-07-30; no 3MF [V] |
| [opencascade.js](https://github.com/donalffons/opencascade.js) | OCCT WASM port | C++ to wasm | Yes, 48.9 MB raw, ~9 MB brotli (custom ~2.4 MB) | LGPL-2.1-only | Yes | R/W | Stable line still OCCT 7.6.2 [V] |
| [replicad](https://github.com/sgenoud/replicad) | Code-CAD on OCCT | TypeScript | Yes | MIT (OCCT wasm is LGPL) | Yes | Yes | npm 1.1.0 [V] |
| [CadQuery](https://github.com/CadQuery/cadquery) | Code-CAD on OCCT | Python | No | Apache-2.0 | Yes | R/W, writes 3MF | 2.8.0, 2026-06 [V] |
| [build123d](https://github.com/gumyr/build123d) | Code-CAD on OCCT | Python | No | Apache-2.0 | Yes | R/W | 0.13.0, 2026-09; builder and algebra modes [V] |
| [OpenSCAD](https://github.com/openscad/openscad) | CSG language and app | C++ | Experimental wasm; [playground](https://github.com/openscad/openscad-playground) | GPL-2.0 + CGAL exception | No (mesh) | No | Stable still 2021.01; Manifold backend in nightlies [V] |
| [JSCAD](https://github.com/jscad/OpenJSCAD.org) | CSG library and app | JavaScript | Yes | MIT | No (mesh) | No | 3MF, STL, DXF and more [V] |
| [Manifold / manifold-3d](https://github.com/elalish/manifold) | Mesh booleans | C++, wasm | Yes, ~2.8 MB npm | Apache-2.0 | No (mesh) | No | "Guaranteed manifold output"; 3.5.4, 2026-09; used by Blender, OpenSCAD, Godot [V] |
| [Truck](https://github.com/ricosjp/truck) | NURBS B-rep kernel | Rust | Yes (`truck-js`, WebGPU) | Apache-2.0 | Yes | R/W (`truck-stepio`, partial) | crates last published 2024-09; master pushed 2026-09-28 and has a `fillet` module [V] |
| [Fornjot](https://github.com/hannobraun/fornjot) | B-rep kernel | Rust | n/a | 0BSD | Yes | No | **Shut down, archived 2026-06-19** [V] |
| [csgrs](https://github.com/timschmidt/csgrs) | CSG on meshes | Rust | wasm feature | MIT | No | No | STL, OBJ, glTF, DXF [V] |
| [opencascade-rs](https://github.com/bschwind/opencascade-rs) | OCCT bindings | Rust (cxx) | Planned | LGPL-2.1 | Yes | Partial | "A major work in progress" [V] |
| [CADmium](https://github.com/CADmium-Co/CADmium) | Browser CAD on Truck | Rust, Svelte | wasm | Elastic-2.0 | Yes | n/a | **Archived 2025-09-05** [V] |
| [Onshape](https://www.onshape.com) | Cloud CAD, FeatureScript | FeatureScript | Native web | Proprietary | Yes (Parasolid) | Yes [I] | $1,500 and $2,500 per user per year [V] |
| [Zoo Design Studio](https://github.com/KittyCAD/modeling-app) | App + KCL + Text-to-CAD | Rust, TS | Limited web client; engine streams video | App MIT; engine closed | Yes (GPU) | Export | KCL parser in Rust [V] |
| [Fusion API](https://help.autodesk.com/cloudhelp/ENU/Fusion-360-API/files/ReferenceManual_UM.htm) | Desktop add-in API | Python, C++ | No | Proprietary | Yes | Yes [I] | |
| Shapr3D | App | n/a | No [I] | Proprietary | Yes (Parasolid, D-Cubed) | Yes [I] | [Siemens](https://blogs.sw.siemens.com/plm-components/software-startup-reduces-risk-and-accelerates-time-to-market-with-plm-components/) [V] |
| [Parasolid](https://en.wikipedia.org/wiki/Parasolid) | Kernel | C | No | Proprietary (Siemens) | Yes | Via translators [I] | v39.0; Onshape, SolidWorks, Shapr3D, Plasticity [V] |
| [ACIS](https://en.wikipedia.org/wiki/ACIS) | Kernel | C++ | No | Proprietary (Spatial/Dassault) | Yes | Via InterOp [I] | [V] |
| [CADAM](https://github.com/Adam-CAD/CADAM) | AI text-to-CAD app | TS + OpenSCAD wasm | Yes | GPL-3.0 | No | No | An LLM writes OpenSCAD [V] |

## SolidCraft, in depth

Added at the owner's request. Repo
[bherbruck/solidcraft](https://github.com/bherbruck/solidcraft), site and
book at [bherbruck.github.io/solidcraft](https://bherbruck.github.io/solidcraft/),
browser app at [/app/](https://bherbruck.github.io/solidcraft/app/) [V].

**What it is.** "An open-source parametric 3D CAD application built in pure
Rust": sketch with constraints, a parametric feature timeline, parameters
and configurations, components and joints, on desktop (Windows, macOS,
Linux), in the browser, from a CLI, and through a built-in MCP server [V,
README].

**Activity.** Created 2026-10-07; v0.10.0 and v0.10.1 released on
2026-10-14; 2,270 stars and 59 contributors in its first week; commits
landing every few minutes on the day we looked [V, GitHub API]. The roadmap
budgets work in "agent hours" (≈82 to a useful alpha), so it is largely
agent-built [V, [ROADMAP](https://github.com/bherbruck/solidcraft/blob/main/ROADMAP.md)].
Status in its own words: "early but usable for simple parts" [V].

**Licence.** MIT OR Apache-2.0 for its own code. Truck crates are
Apache-2.0; `truck-shapeops` 0.4.0 and `truck-stepio` 0.3.0 are vendored
with marked fixes. Fonts are OFL-1.1 [V,
[NOTICE](https://github.com/bherbruck/solidcraft/blob/main/NOTICE)]. All
compatible with our use [I].

**Stack** [V, workspace Cargo.toml and crate manifests]:

| Crate | Job |
| --- | --- |
| `solidcraft-geom` | Vectors, planes, frames, meshes, measures; the portable B-rep data model |
| `solidcraft-kernel` | "B-rep kernel boundary: solids, extrude/revolve/booleans/fillets, tessellation, measures and STEP, behind our own API (implemented with truck)" |
| `solidcraft-sketch` | The 2D constraint solver |
| `solidcraft-doc` | Parameters and expressions, sketches and features on a timeline, incremental evaluation |
| `solidcraft-engine` | The session, the command registry ("everything is a command"), undo and selection |
| `solidcraft-io` | Files |
| `solidcraft-render`, `solidcraft-gpu`, `solidcraft-ui-egui` | wgpu 30 rendering and an egui UI |
| `solidcraft-mcp` | "JSON-RPC 2.0 over stdio, driving a headless session or the running app" |
| `apps/solidcraft-web` | eframe web runner, "WebGPU with WebGL2 fallback" |

The kernel crate wraps truck but adds a great deal of its own: blends
(variable radius, corners, vertex blends), boolean guards and diagnostics,
face and edge provenance, healing, offsets, sewing, its own STEP and IGES
readers [V, `crates/kernel/src` file list]. Its vendored truck boolean
records which operand face and edge every result face and edge came from;
that is the basis of its persistent naming [V,
[vendor/README.md](https://github.com/bherbruck/solidcraft/blob/main/vendor/README.md)].

**Commands.** 350 registered (6 planned), in the same shape from toolbar,
script, CLI `solidcraft-cli exec <id> '<json>'`, control channel and MCP
[V, [features.md](https://github.com/bherbruck/solidcraft/blob/main/docs/features.md)]:
sketch (99), solid modelling (35: extrude, revolve, sweep, loft, fillet,
chamfer, shell, combine, rectangular, circular and path patterns, …),
surfaces, construction geometry, sheet metal, plastic parts, components
(16), assembly joints (11: joint, as-built, rigid group, drive, motion link,
limits, solve), configurations, timeline (21), inspect (centre of mass,
interference), materials with densities.

**Files.** `.solidcraft` is a ZIP with `document.json` (the versioned
parametric document, `solidcraft/20`) and SHA-256 content-addressed blobs
[V, [design-container.md](https://github.com/bherbruck/solidcraft/blob/main/docs/design-container.md)].
Exports: STEP, IGES, 3MF, STL, OBJ, DXF, and **URDF** (one link per
component with mesh, mass and inertia; joints as URDF joints) [V,
[urdf.md](https://github.com/bherbruck/solidcraft/blob/main/docs/urdf.md)].

**How it checks itself.** An oracle suite: each public case is a
`recipe.json` replayed through the command engine and compared with
`expected.json` (body count, volume and area to 1e-3 relative, centre of
mass, bounding box, face count, Hausdorff distance to a reference STL,
principal moments). Expected values are analytic (61 cases) or from
FreeCAD as an independent kernel (21). Every rebuilt part is exported as
STEP, validated and read back. The roadmap reports 82 of 82 passing; the
README says 75 of 76, one a tracked bug [V,
[oracle.md](https://github.com/bherbruck/solidcraft/blob/main/docs/oracle.md),
ROADMAP, README]. CI also runs hostile-input fuzz over every command and a
wasm32 build [V].

**Weak spots** [V unless marked]:

- Assembly mass properties use "the kernel's tessellation-based body
  measurement", not exact surface integrals [V,
  [assembly-properties.md](https://github.com/bherbruck/solidcraft/blob/main/docs/assembly-properties.md)].
  Fine for a game, worth a cross-check for Lagrange 2's latch loads [I].
- Truck booleans needed several robustness patches; expect more [I].
- A week-old project moving this fast will break APIs; nothing is on
  crates.io (`solidcraft-kernel` "does not exist" there) [V], so we would
  pin a git commit [I].

### The rover

[0xlillith/solidcraft-rover v1.0.0](https://github.com/0xlillith/solidcraft-rover/releases/tag/v1.0.0)
(Apache-2.0 repo; assets "under MIT OR Apache-2.0"; 2026-10-13) [V]:
"an original scientific rover built entirely through native SolidCraft MCP
commands". The model has **1,395 bodies, 576 sketches, 36 components plus
root, 17 revolute joints and 3,324 history entries**, built with SolidCraft
0.7.0. Assets: `Rover-v7.solidcraft` (216 KB, the editable history),
`Rover-v7.step` (18 MB), `Rover-v7.3mf` (53 MB), 16 PNG renders, build and
showcase GIF/MP4, SHA-256 sums, and a benchmark protocol [V]. It is "an
illustrative assembly, not a manufacturing-certified design" [V].

The lesson: an agent driving a typed command API can build a large, fully
editable assembly, and the history file is tiny next to the exports [I].
That is our OpenAgents issue #11627.

### What we'd adapt from SolidCraft

1. **The crates themselves.** Same language, same wgpu major version (30)
   as Verse, wasm-clean, permissive licence. Depending on
   `solidcraft-kernel`, `-geom`, `-sketch`, `-doc` (and possibly `-engine`)
   saves us months [I]. See [adaptation.md](adaptation.md).
2. **"Everything is a command."** One registry serves UI, CLI, scripts and
   MCP. Our Lab chat tools map onto it directly [I].
3. **The oracle.** Recipes with analytic or second-kernel expected values,
   STEP round-trip on every case, in CI. We should adopt the pattern as our
   CAD Lab checks [I].
4. **Provenance through booleans** for stable face and edge names, the
   thing ClassCAD calls stable references [I].
5. **URDF export with mass and inertia** as the bridge to Robot Lab [I].
6. **Content-addressed blobs** in the design file, matching our Lab
   version store's digests [I].

## Other notes per family

### OpenCascade

OCCT is the only mature open-source B-rep kernel with a long track record
[I]. In the browser it costs about 9 MB brotli in full, 2.4 MB in a custom
build [V, [ocjs.org](https://ocjs.org/docs/getting-started/file-size)]. The
LGPL with OCCT's exception allows static linking with attribution [V,
[exception](https://raw.githubusercontent.com/Open-Cascade-SAS/OCCT/master/OCCT_LGPL_EXCEPTION.txt)],
but we would still have to ship source of any OCCT changes and keep a C++
toolchain in our wasm build [I]. CadQuery and build123d show the API style
agents write best: build123d's algebra mode, which smlibpy copies [V,
smlibpy README].

### Code CAD and meshes

OpenSCAD and JSCAD are CSG on meshes: easy to script, no exact faces, no
fillets beyond hacks, no STEP [I]. Manifold fixed the speed and robustness
of mesh booleans and is now everywhere [V]. For us Manifold is a tool for
meshes we import (game assets, scans), not the part kernel [I].

### Browser CAD apps

Onshape proved full parametric CAD runs in a browser tab, with the kernel
on its servers and FeatureScript as the feature language [V,
[FsDoc](https://cad.onshape.com/FsDoc/)]. Zoo puts the kernel on GPUs in its
cloud and streams video to the client; KCL is a Rust-parsed language and
Text-to-CAD writes KCL [V]. Both keep the kernel closed [V].

### Kernels

Parasolid and ACIS are the commercial standards, licensed per product [V].
In Rust: Truck is the only living exact B-rep kernel; Fornjot and CADmium
are archived [V]. New this month: SMLib open-sourced as USD BRep (C++,
Apache-2.0), with a 1.6 MB brotli wasm build via smlibwasm [V].

## Formats

- **STEP (ISO 10303).** AP242 merges AP203 and AP214 and adds PMI,
  kinematics and assembly constraints [V,
  [prostep](https://www.prostep.org/mediathek/fact-sheets/iso-10303-242)].
  The interchange format for exact solids. Writers: SolidCraft (truck
  writer, fixed), truck-stepio, SMLib, OCCT, ClassCAD [V]. Rust:
  `ruststep` 0.4.0 and `truck-stepio` 0.3.0, both Apache-2.0 [V].
- **IGES.** Legacy surfaces; SolidCraft reads and writes it [V].
- **glTF 2.0.** For rendering, not engineering. Our Verse crates use the
  `gltf` 1.4.1 crate to import; we have no glTF writer today (our own code
  survey) [V]. Manifold recommends glTF with `EXT_mesh_manifold` [V].
- **3MF.** ISO/IEC 25422:2025, the 3D-printing format [V,
  [3mf.io](https://3mf.io/spec/)]. Rust: `threemf` 0.8.0, 0BSD [V].
  SolidCraft writes it [V].
- **URDF.** Not a CAD format, but SolidCraft exports it from joints, which
  makes it one for us [V].

## AI CAD and how it is checked

| Work | What the model writes | How the result is checked |
| --- | --- | --- |
| [DeepCAD](https://arxiv.org/abs/2105.09492) (2021) | CAD operation sequences | Dataset of 178,238 models [V] |
| [Text2CAD](https://arxiv.org/abs/2409.17106) (NeurIPS 2024) | Parametric sequences from text | Visual quality, parametric precision, geometric accuracy [V] |
| [CAD-Recode](https://arxiv.org/html/2412.14042v2) | CadQuery Python from point clouds | Chamfer distance, IoU, invalidity ratio (code that fails to build) [V] |
| [CADCodeVerify](https://arxiv.org/html/2410.05340v2) (ICLR 2025) | CAD code | A VLM asks and answers yes/no questions about renders, then repairs [V] |
| [CAD-Assistant](https://arxiv.org/abs/2412.13810v3) (ICCV 2025) | FreeCAD Python | Renders, cross-sections, per-command geometry diffs [V] |
| [Zoo Text-to-CAD](https://zoo.dev/docs/faq) | KCL | Builds on their B-rep engine [V] |
| [CADAM](https://github.com/Adam-CAD/CADAM) | OpenSCAD | Renders in the browser; parameters become sliders [V] |
| [classcad.ai](https://classcad.ai) | ClassCAD JS | Deterministic renders, sections, pixel diffs, mass properties vs a hand calculation [V] |
| [SolidCraft](https://github.com/bherbruck/solidcraft) MCP | Commands | Oracle: analytic and FreeCAD values, STEP round-trip [V] |

Pattern [I]: the strong checks are (1) it builds without error, (2) it is a
valid closed solid, (3) numbers match an independent expectation (volume,
centre of mass, inertia, bounding box), (4) pictures look right to a VLM.
Most research stops at (1), (3) against a dataset, and (4). Nobody we found
gates every edit on all four. We can.
