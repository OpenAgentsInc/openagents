# ClassCAD and its family, in depth

Researched 2026-10-14. Part of [CAD research](README.md).

Marks: **[V]** we read it on the linked page, repo or registry on
2026-10-14. **[I]** we inferred it from those sources. Quotes are short and
attributed; everything else is our paraphrase. We did not download or run
any of their code. Repo and npm facts come from the GitHub API and
`npm view` metadata only.

## The family at a glance

AWV Informatik AG, a small employee-owned company in St. Gallen,
Switzerland, makes seven things that its site's "Family" menu lists
together [V, [classcad.ch/docs](https://www.classcad.ch/docs/)]:

| Name | Their tagline | What it actually is | Open? |
| --- | --- | --- | --- |
| [ClassCAD](https://classcad.ch) | "Headless, programmable CAD." | The engine: a C++ runtime plus CAD logic written in its own language, on the SMLib kernel. Runs as WebAssembly in a Web Worker or as a native server. | Closed, keyed [V] |
| [buerli.io](https://buerli.io) | "CAD wherever JavaScript reaches." | The JavaScript SDK (`@buerli.io/classcad`, `@buerli.io/react`, `@buerli.io/headless`): connects to the engine and draws into three.js. | npm packages ISC; still needs a ClassCAD key [V] |
| [Buerligons](https://buerligons.io) | "Parametric CAD in a browser tab." | A history-based CAD app built on buerli. | App source MIT on GitHub; engine still keyed [V] |
| [ClassCAD PDM](https://pdm.classcad.ch) | "Versions and releases, on your servers." | Product data management: versions, branches, releases, BOM, generators from git. "In development, not released." | Not stated [V] |
| [classcad.ai](https://classcad.ai) | "Plain language in, verified geometry out." | Agent tooling: a skill (API docs and recipes), a script runner, a renderer for checks, an MCP server and an in-app chat assistant. | MIT packages; they drive the keyed engine [V] |
| [smlibpy](https://smlibpy.classcad.ch) | "SMLib's NURBS kernel, scripted in Python." | A build123d-style Python library straight on the kernel, plus `@classcad/smlibwasm` for the browser. | MIT, with SMLib under Apache-2.0 [V] |
| [AWV](https://awv-informatik.ch) | "Employee-owned, in St. Gallen." | The company. Says the engine has worked inside other companies' software for "twenty-five years". | n/a |

Elsewhere: [GitHub](https://github.com/awv-informatik),
[Discord](https://discord.gg/MEbR7xyPMS),
[LinkedIn](https://ch.linkedin.com/company/awv-informatik),
[X](https://x.com/buerli_io) [V].

## The engine: ClassCAD

### What it is

"A CAD engine without a user interface" [V,
[docs](https://www.classcad.ch/docs/)]. Programs create and change parts,
sketches, features, assemblies and drawings through one API. The engine
computes the geometry and returns it ready to render.

### Architecture: three layers

From [Architecture](https://www.classcad.ch/docs/architecture/) [V]:

1. **Runtime.** A C++ program that "compiles and runs ClassCAD's own
   language", keeps the model in memory and answers API calls one after
   another. It targets Windows, Linux and WebAssembly.
2. **Classes.** The CAD logic itself, written in the ClassCAD language, one
   class per file, compiled to bytecode for the runtime's virtual machine.
   The system classes (Common, BaseModeling, CAD, System) define the API.
3. **Plugins.** C++ libraries loaded at start: the geometry kernel (SMLib),
   2D and 3D constraint solvers, and file formats (STEP, DXF, glTF).

The public API, `api.v1.<domain>.<method>`, is generated from the system
classes for each release as typed packages for TypeScript, Python and .NET
[V].

The [ClassCAD and SMLib](https://www.classcad.ch/docs/architecture/classcad_and_smlib)
page draws the line we care most about [V]:

- **The kernel** does B-rep creation and editing, intersections, blends,
  offsets and tessellation.
- **The kernel lacks** feature history and parametric rebuild, stable
  references across topology changes, assemblies, constraints, drawings and
  API contracts. The page says building directly on a kernel means
  implementing most of the CAD platform yourself.
- **ClassCAD adds** exactly those, and links back to the kernel through
  service functions, kernel callbacks on copy, split and merge (which drive
  reference updates), attributes on kernel elements, and a tessellation
  that keeps topology identity so a picked triangle maps to a face.

That last list is a good checklist of what any CAD layer we build above a
kernel has to own [I].

### WebAssembly vs native

The same program runs on both; "only the line that connects changes" [V,
[docs](https://www.classcad.ch/docs/)].

| | WebAssembly | Native |
| --- | --- | --- |
| Where | A Web Worker in your page, one engine per page [V, [wasm](https://www.classcad.ch/docs/wasm)] | Your server; a Node 20+ "ClassCAD server" starts one engine process per session, port 9091 [V, [native](https://www.classcad.ch/docs/native)] |
| Platforms | Browsers | Windows x64, Linux x64 and arm64 [V] |
| Connect | `new WASMClient(id, { token: 'ccpk_…' })` | `new SocketIOClient('ws://…:9091', id)` [V] |
| Download | About 55 MB on first visit, then cached [V, [buerli.io](https://buerli.io)] | An archive from your account [V] |
| Key | A public token bound to registered domains; localhost always works [V] | A secret token, or an offline key file [V] |
| Clients | JavaScript | JavaScript, Python and .NET connectors [V] |

The 55 MB WASM engine is large next to its own kernel alone (SMLib as
`smlibwasm` is about 1.6 MB brotli, below). The difference is the runtime,
class bytecode, solvers and format plugins [I].

### API shape

[Guides](https://www.classcad.ch/docs/api-usage/) and the API reference [V]:

- **Domains:** part (76 methods), assembly (63), sketch (50), common (22),
  curve (21), solid (21), drawing2d (11): 264 in all [V, classcad-ai
  [README](https://github.com/awv-informatik/classcad-ai)].
- **One parameter object per call**, so new optional keys never break old
  calls. Some methods take an array of objects to batch. With
  `@buerli.io/classcad` calls return ids and throw on errors; the raw API
  returns `{ result, messages }` [V].
- **Selections by geometry:** `getGeometryIds` finds the B-rep element
  nearest given points; `getGeometryPositions` returns identifying points
  for elements [V, [part API](https://www.classcad.ch/docs/API/part)].
- **Expressions:** named, product-level expressions linked to sketch
  dimensions and feature parameters (`expression`, `linkWithExpression`,
  `updateExpression`) [V].
- **Mass properties:** `calculateMassProperties` returns volume and centre
  of gravity for a solid, part, subassembly or the whole model [V]. We did
  not see an inertia tensor in the ClassCAD API (smlibpy has one, below)
  [V for what we saw; the reference pages were long and partly
  truncated in our reads].
- **Import and export** ([common](https://www.classcad.ch/docs/API/common)) [V]:
  - Load: OFB (their native format), STEP, IWP; STL as a mesh "PolyBrep".
  - Save: OFB, SCG, STEP (AP203, AP214, AP242), IWP, STL, DXF (2D only).
  - glTF is listed among the plugins, and `createBufferGeometry` and
    `createScene` hand three.js geometry to the page [V]. IGES and SAT are
    "not yet supported" in Buerligons [V,
    [buerligons.io](https://buerligons.io)].
- **No interference or clash check and no BOM method** in the API tables
  we read [V for the tables].

### The two modelling APIs: Part (history) vs Solid (direct)

From [Part API vs Solid API](https://www.classcad.ch/docs/api-usage/classcadPartVsSolidApi) [V]:

- **Part API.** Each call (`box`, `extrusion`, `fillet`, `boolean`, …) adds
  a feature to the part's history. "The call does not return a solid you
  can change: it adds a feature to the history." The engine tracks which
  feature made which solid and keeps references to faces and edges valid
  across rebuilds. Every feature has an `update*` twin, so a parameter
  change replays the history. A rollback bar (`openFeature`,
  `operationMoveBefore`) moves through it.
- **Solid API.** Operations (`box`, `union`, `subtraction`, `translation`,
  `extrusion`, …) return solids directly. A solid variable lives until an
  operation consumes it; `keepTools: true` keeps the tools. Face and edge
  references are not tracked. It is faster, and suits code that recomputes
  the whole model whenever its parameters change.
- **How they meet: `EntityInjection`.** Solid API code runs inside a part
  as one feature (`api.v1.part.entityInjection`). Today that feature does
  not recompute when earlier features or expressions change; "their
  geometry behaves like an imported STEP model", and recalculation is
  planned. `useSolid` copies a Part API solid into the injection so the
  original stays intact for rollback. Their example: a perforated sheet with
  a Part API base and Solid API holes along a sine curve.
- **Consumed solids.** Features consume solids. Touching a consumed solid
  throws. In code you never reorder history; you write calls in order and
  pass solids forward through `targets` and `tools`.
- **Their advice:** use the Part API for structured, fully parametric parts
  (they say these are comparable to high-end CAD systems with fewer
  features, and move between CAD systems); use the Solid API for parts
  generated from code.

### Assemblies

From [Building assemblies](https://www.classcad.ch/docs/api-usage/assembly_building)
and the [assembly API](https://www.classcad.ch/docs/API/assembly) [V]:

- **Templates and instances.** Parts and subassemblies are templates; an
  assembly holds instances of them, nested to any depth.
- **Mates** are coordinate systems on faces, edges or points.
  **Constraints** join two mates: `fastened`, `fastenedOrigin`, `revolute`
  (with rotation limits), `slider` (with offset limits), `cylindrical`,
  `planar`, `parallel`, `spherical`, plus `gear` relations between two
  constraints and linear and circular patterns.
- **Position layers:** origin by default, then a user transform, then the
  constraint solver's result, each overriding the last.
- **Mate paths** reach a mate inside a subassembly (Assembly → Sub → Part).
- **Motion:** `moveUnderConstraints` drags a mechanism within its limits.
- **From data:** `assembly.from` builds a whole assembly from JSON (or
  ECXML): templates pointing at `.ofb` files, instances, and a constraints
  array.

A constraint set with limits (revolute, slider, spherical) is very close to
a URDF or MJCF joint list [I]. That is the bridge to Robot Lab.

### Polylines, DXF and the Solid API

[The guide](https://www.classcad.ch/docs/api-usage/PolylinesDxfSolidApi) [V]:
aluminium window and facade profiles drawn as 2D polylines in DXF. An
AutoLISP exporter writes only the intended contours to JSON (points, bulge
values for exact arcs, explicit closure and orientation), which
`api.curve.polyline2d` reads and `api.solid.extrusion` extrudes. They call
the JSON the "neutral single source of truth": DXF stays the authoring
format; a small, reviewable, versioned JSON becomes the exchange and runtime
format. That is the same move as our Lab documents [I].

## buerli.io, the SDK

[buerli.io](https://buerli.io) [V]:

- `@buerli.io/classcad` (engine client: `init`, `WASMClient`,
  `BuerliCadFacade`) and `@buerli.io/react` (`useBuerliCadFacade`, which
  returns `{ api, Geometry }`; `Geometry` is a react-three-fiber component
  that suspends until the model is ready).
- A model is "a plain async function" calling `api.v1`; changing a prop
  rebuilds it.
- Customers named: wemonte (test-adapter configurator, "weeks to about
  one"), FMS (configurator to STEP and drawings with no engineer), and
  glowbuzzer (loads STEP, picks points and edges to plan robot moves).
- npm: `@buerli.io/classcad` 1.2.0, ISC, 2026-10-05 [V, npm registry].

## Buerligons, the app

[buerligons.io](https://buerligons.io) and
[GitHub](https://github.com/awv-informatik/buerligons) (MIT, 181 stars) [V]:
sketches with constraints and named dimensions, extrude, revolve, fillet,
chamfer, patterns, booleans, expressions, assemblies with draggable joint
limits. Opens OFB, STEP and IWP; saves OFB, STEP and STL. No 2D drawings,
sweeps, SAT or IGES yet; phones get a viewer only. The app is open source,
but it needs a ClassCAD key to run the engine [V, README].

## ClassCAD PDM

[pdm.classcad.ch](https://pdm.classcad.ch) [V]: "in development, not
released". Parts and assemblies are database records. Saving overwrites a
draft; a version is a numbered frozen snapshot; branches and restore;
releases get revision labels. Assemblies pin each referenced part to a
version and flag when a newer one exists. Generators are code in a git repo,
run on workers, each result saved as a document, frozen to its commit when
released. Stack: NestJS on Node 20, Postgres 16, a SHA-256
content-addressed file store, a scheduler pool of ClassCAD 21.2 engines, an
OpenAPI REST API with ETag protection and an audit log.

This is almost exactly the shape of our Lab version store (immutable
versions, parent pointers, digests, "edit must name the latest version") [I].

## classcad.ai, the agents

[classcad.ai](https://classcad.ai) and
[classcad-ai](https://github.com/awv-informatik/classcad-ai) (145 stars; npm
`@classcad/mcp` 0.4.1, MIT, published 2026-10-14) [V]:

- **The skill** (`@classcad/skill`): all 264 methods documented "and
  verified against a live engine", six recipes, and a BM25 search that knows
  CAD synonyms ("a hole is a bore, a round a fillet").
- **The script** (`@classcad/script`): the agent writes one JS/TS program
  and runs it with `run_script`, ideally in one turn.
- **The renderer** (`@classcad/renderer`): deterministic snapshots from
  several angles, sections, markers, highlights, drawings in first- or
  third-angle projection, and pixel diffs.
- **The envoy:** an MCP server (one daemon per machine, for Claude Code,
  Codex, Cursor, VS Code, OpenCode, Claude Desktop) and `@buerli.io/ai`, an
  in-app chat panel for any tool-calling LLM.
- **Sharing:** a relay link lets others join a live session.

**What "verified geometry" means here.** The agent looks (deterministic
renders and pixel diffs) and measures (`calculateMassProperties`: volume,
centre of gravity, extents) and compares against an expectation, such as a
hand-computed volume, then iterates [V]. We found no formal validity
pipeline, CI benchmark or published accuracy numbers [V for absence on the
pages we read]. "Verified" is closer to "the agent checked its work with
numbers and pictures" than to a proof [I].

Pricing per classcad.ai: the tools are free; you pay for the engine. Free
covers private use and evaluation; Solo, Pro and Business add commercial
use, STEP and other exports, sharing, own domains and self-hosting. AI usage
is billed by your provider. No royalty on products designed with it [V].
DEVELOP3D covered it on 2026-10-02
([article](https://develop3d.com/ai/classcad-ai-cad-mcp/)) [V].

## smlibpy and smlibwasm, the kernel in Python and JavaScript

This is the most important finding for us.

**SMLib is now open source.** NVIDIA, which bought Solid Modeling Solutions
(the makers of SMLib, NLib and GSNLib) some years ago [I from a
[forum post](https://forums.developer.nvidia.com/t/where-can-i-find-software-that-nvidia-bought-smlib-nlib-etc/345990)
and [SIGGRAPH history](https://history.siggraph.org/?p=183509)], published it
as **USD BRep** on 2026-10-08 under **Apache-2.0** [V,
[NVIDIA-Omniverse/usd-brep](https://github.com/NVIDIA-Omniverse/usd-brep),
CHANGELOG "1.0.0 - 2026-10-08 Initial public release"]. It is about 45 MB of
C++ with Python bindings and OpenUSD import and export, and it already had
about 1,400 stars six days after release [V]. Its README says it is "not
accepting contributions"; issues are welcome [V].

Its own [KNOWN_ISSUES.md](https://github.com/NVIDIA-Omniverse/usd-brep/blob/main/KNOWN_ISSUES.md)
is candid [V]:

- `remove_fillet` reports success and removes nothing.
- `heal_brep` can report success when a repair failed; healing of closed
  surfaces (tori, spheres, cylinders) imported from other CAD systems has a
  list of failure cases.
- Tessellation can wind triangles inside out next to fillets.
- Area, volume and mass properties can be off by up to about 1.4% at the
  default accuracy of 1e-3 for faces trimmed by rational circles; tightening
  the accuracy fixes it.

The last one matters for us: mass properties must be computed at a tight
accuracy, and checked against an analytic case, before a part's inertia
feeds the simulator [I].

**smlibpy** ([repo](https://github.com/awv-informatik/smlibpy), MIT, created
2026-10-12, 122 stars; [site](https://smlibpy.classcad.ch)) [V]:

- "It is new, version 0.1." Python 3.10 to 3.14, wheels for Linux, macOS and
  Windows; `pip install smlibpy`. Free, no account.
- API "follows the style of build123d's algebra mode": shapes are values,
  every operation returns a new one, `+ - &` for booleans, selections by
  geometry (`part.faces().top()`, `edges().parallel_to(Z).concave()`).
- Primitives, holes, patterns, extrude, revolve, loft, sweep (also helical),
  shell, offset, thicken, draft, mirror, split, section, G2/G3 fillets,
  chamfers, sheet metal with fold, flange, unfold and flat pattern.
- Reads and writes STEP, USD and DXF; writes STL and glTF.
- Mass properties integrated over the exact surfaces, including an inertia
  matrix, principal moments and axes.
- If a fillet is too large, it reports the largest radius that fits.
- No constrained assemblies, 2D drawings or text yet.
- Comes with an MCP server and a Claude Code plugin.
- It vendors usd-brep as a git submodule, with a few AWV defect fixes kept
  as patches with notices [V, NOTICE].

**smlibwasm** (`@classcad/smlibwasm` 0.3.0, licence
`MIT AND Apache-2.0 AND OFL-1.1`) [V, npm;
[docs](https://smlibpy.classcad.ch/docs/javascript)]:

- The same API compiled to WebAssembly for browser and Node; "needs no
  server and has no dependencies".
- Kernel `.wasm` about 1.6 MB brotli (6.5 MB uncompressed); about 1.9 MB
  brotli all in.
- Synchronous after the first load; `part.mesh(tolerance)` for three.js;
  `toStep`, `toGlb`, `fromStep` in memory; `part.massProperties(density)`
  returns mass, volume, area, centre and a 3x3 inertia.
- Kernel memory sits outside the JS garbage collector; call `dispose()`.

## Licence and price

- **Engine licence** ([awv-informatik.ch/license](https://awv-informatik.ch/license/)) [V]:
  non-commercial use is free with a personal key; internal business use
  needs a commercial subscription; one key covers native and WASM; WASM keys
  must be bound to origins. **Anyone who gives third parties access** to the
  engine's functionality, including a hosted web app or a WASM deployment
  reachable from outside the organization, is a "Distributor" and needs a
  distribution licence. That is us if we ship it in openagents.com.
- **Price.** AWV's home page and older
  [pricing post](https://awv-informatik.ch/blog/pricing/) say: free for
  non-commercial, CHF 100 a year for commercial use, CHF 5,000 a year to ship
  it inside your own product [V for what the pages say]. The newer
  [plans page](https://www.classcad.ch/plans) names Free, Solo, Pro and
  Business but renders prices client-side; we could not read the amounts
  [V]. From the docs: Pro allows one WASM domain per seat; Business allows
  any number, the native engine and self-hosting the WASM files [V,
  [wasm](https://www.classcad.ch/docs/wasm),
  [native](https://www.classcad.ch/docs/native)]. Treat CHF 5,000 a year as
  the order of magnitude for distribution and confirm with AWV [I].
- **Ownership:** TDM Systems holds 26% of AWV and is its largest customer
  [V, [awv-informatik.ch](https://awv-informatik.ch)].

### Open versus closed

| Piece | Status |
| --- | --- |
| SMLib kernel (usd-brep) | Open, Apache-2.0, NVIDIA [V] |
| smlibpy, smlibwasm | Open, MIT on top [V] |
| ClassCAD runtime, classes, constraint solvers, history, references, assemblies, drawings | Closed, keyed subscription [V] |
| buerli SDK packages | ISC on npm, but useless without the keyed engine [V, I] |
| Buerligons app | MIT source, keyed engine [V] |
| classcad.ai tools | MIT, keyed engine [V] |
| ClassCAD PDM | Unreleased, licence not stated [V] |

The valuable, hard-won part (history, stable references, solvers,
assemblies) is the closed part. The kernel underneath is now free.
