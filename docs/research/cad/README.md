# CAD in the browser: ClassCAD, the ecosystem, and what we build

Researched 2026-10-14 at the owner's request: read ClassCAD's docs and the
family around it, survey browser and programmable CAD, and say what we
should adapt. Later additions from the owner: SolidCraft, and building an
equivalent of the SolidCraft rover in our stack (#11627).

| File | What's in it |
| --- | --- |
| [classcad.md](classcad.md) | ClassCAD and its six siblings in depth: engine, WASM vs native, API, history, assemblies, agents, kernel, licence and price |
| [ecosystem.md](ecosystem.md) | The comparison table; SolidCraft and its rover in depth; OCCT, code CAD, browser CAD apps, kernels, formats, AI CAD |
| [adaptation.md](adaptation.md) | What each tool can do for the rover, the kernel recommendation, what we adapt, the build order, open questions |

Marks: **[V]** verified on the linked source on 2026-10-14. **[I]** our
inference. Every claim in the supporting files carries one.

## Plain summary

- **ClassCAD** is a closed, paid, headless CAD engine from AWV Informatik
  in St. Gallen. It runs the same API in a browser tab (a 55 MB WebAssembly
  build in a Web Worker) or on a server [V,
  [docs](https://www.classcad.ch/docs/), [buerli.io](https://buerli.io)].
  Around it sit a JavaScript SDK (buerli), an MIT-licensed browser CAD app
  (Buerligons), a PDM in development, agent tools (classcad.ai: an MCP
  server and a chat panel), and smlibpy [V].
- **The big news under it:** its kernel, SMLib, was open-sourced by NVIDIA
  six days ago as **USD BRep, Apache-2.0**, and AWV ships it free as smlibpy
  (Python) and smlibwasm (about 1.9 MB brotli in the browser) [V,
  [usd-brep](https://github.com/NVIDIA-Omniverse/usd-brep),
  [smlibpy](https://github.com/awv-informatik/smlibpy)]. The kernel is now
  free; ClassCAD's layer above it (history, stable references, solvers,
  assemblies, drawings) stays closed [V].
- **The best fit for us is not ClassCAD but SolidCraft**: an open,
  pure-Rust parametric CAD app (MIT OR Apache-2.0) on the Truck kernel, with
  wgpu 30 like Verse, a wasm web build, 350 typed commands, an MCP server,
  joints, STEP, 3MF and URDF export, and an oracle test suite. It is one
  week old and moving very fast [V,
  [solidcraft](https://github.com/bherbruck/solidcraft)]. An agent built a
  1,395-body, 17-joint rover in it entirely through MCP commands [V,
  [rover](https://github.com/0xlillith/solidcraft-rover/releases/tag/v1.0.0)].
- **What we do:** depend on SolidCraft's kernel crates behind our own `cad`
  crate, write parts and assemblies as typed, versioned Lab documents
  (`part.v1`, `assembly.v1`), check every edit (rebuilds, valid solid, mass
  properties, interference, STEP round-trip, renders), render in Verse, and
  feed Robot Lab (through URDF) and the Lagrange 2 factory (parts with real
  mass and ports). The first target is our own rover (#11627) [I].

## What ClassCAD is, technically

Details and sources in [classcad.md](classcad.md).

- **Engine:** a C++ runtime that runs CAD logic written in ClassCAD's own
  language, compiled to bytecode; plugins for the kernel, 2D and 3D
  constraint solvers, and STEP, DXF and glTF [V].
- **Kernel:** SMLib, a NURBS B-rep kernel, now Apache-2.0 as NVIDIA's USD
  BRep [V].
- **WASM vs native:** the same program on both, only the connect line
  differs; WASM keys are bound to registered domains, native runs on Windows
  and Linux behind a Node Socket.IO server, one engine per session [V].
- **API:** `api.v1.<domain>.<method>`, 264 methods across part, assembly,
  sketch, common, curve, solid and drawing2d; one parameter object per call;
  generated typed clients for TypeScript, Python and .NET [V].
- **History model:** the Part API adds features to a replayable history and
  tracks face and edge references; the Solid API returns solids directly
  with no history; `EntityInjection` runs Solid code as one feature inside a
  part (not yet recomputed on change) [V].
- **Assemblies:** templates and instances, mates as coordinate systems,
  fastened, revolute, slider, cylindrical, planar, parallel, spherical and
  gear constraints with limits, built by API or from JSON [V].
- **Licence and price:** free for non-commercial use; CHF 100 a year for
  commercial use; CHF 5,000 a year to ship it in your own product, per AWV's
  pages; newer Free, Solo, Pro and Business plans whose prices we could not
  read [V]. Serving it to openagents.com users would make us a
  "Distributor" [V, [licence](https://awv-informatik.ch/license/)].
- **Open vs closed:** kernel, smlibpy, smlibwasm, Buerligons, buerli
  packages and classcad.ai tools are open; the engine that makes them work
  is not [V].

## The ecosystem in one table

The full table, with SolidCraft, OCCT, opencascade.js, replicad, CadQuery,
build123d, OpenSCAD, JSCAD, Manifold, Truck, Fornjot, csgrs,
opencascade-rs, CADmium, Onshape, Zoo, Fusion, Shapr3D, Parasolid, ACIS and
CADAM, is in [ecosystem.md](ecosystem.md#comparison-table). The short
version:

| | Open | Rust | Browser | Exact B-rep | Joints | STEP | Fit for us |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SolidCraft (Truck) | Yes | Yes | Yes | Yes | Yes | Yes | **Best** |
| SMLib / smlibwasm | Yes | No (C++) | Yes | Yes, exact mass | No | Yes | Precision fallback |
| ClassCAD | No | No | Yes, 55 MB | Yes | Yes | Yes | Partner only |
| OCCT / opencascade.js | LGPL | No | Yes, 2.4-9 MB | Yes | No | Yes | No |
| Manifold | Yes | No (C++) | Yes | No, meshes | No | No | Meshes only |
| Fornjot, CADmium | n/a | Yes | n/a | n/a | n/a | n/a | Archived |

## What to learn and adapt

In full in [adaptation.md](adaptation.md). In short [I]:

1. **Part vs Solid:** history for things people and agents edit by name;
   direct code for generators; one block type joins them, and it recomputes.
2. **`part.v1` and `assembly.v1`** in our Lab pattern: typed JSON, versions
   with summary, author, digest and measured mass properties, one checked
   edit path. Feature ids are SolidCraft command ids.
3. **CAD Lab:** `cad_*` chat tools; every edit must rebuild, be a valid
   solid, pass its numbers, have no interference, read back as STEP, and
   render the same fixed views for Compare.
4. **Kernel:** SolidCraft's crates on Truck, pinned and wrapped; SMLib for
   exact mass cross-checks and as the fallback; no ClassCAD licence; no OCCT.
5. **Rendering:** tessellate with face ids into `verse-pbr` on web and
   native; add a glTF writer.
6. **Exports:** STEP, 3MF, glTF, URDF, `.solidcraft`.
7. **Robot Lab and Lagrange 2:** URDF from assemblies into
   `robot-description`; real mass, full inertia and latch ports into the
   spacecraft document.

## Build order (first three; full list in adaptation.md)

1. **Spike (S):** `crates/cad` with SolidCraft's crates pinned; native and
   wasm32 builds; a filleted plate with a hole pattern whose volume matches
   the analytic value; STEP and 3MF out, STEP read back.
2. **Documents (M):** `part.v1` and `assembly.v1`, private versioned store,
   checks, `openagents cad` CLI.
3. **The rover, #11627 (L):** built by an agent through `cad` commands only;
   shipped as history, `.solidcraft`, STEP, 3MF, URDF, renders and a build
   video, MIT OR Apache-2.0.

## Open questions for the owner

1. Partner with or license ClassCAD? We say not now.
2. Depend on SolidCraft, fork it, or contribute upstream? We say pinned
   dependency plus upstream fixes.
3. Scope: CAD for our own worlds first, or a user-facing CAD product?
4. Is tessellation-based mass good enough for games, with exact SMLib mass
   only for Lagrange 2?
5. Rover size for the first cut.
