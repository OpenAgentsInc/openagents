# CAD Lab: design parts and assemblies by talking

Status: MVP, 2026-10-14 (#11627). Owner request: build an equivalent of the
[SolidCraft rover](https://github.com/0xlillith/solidcraft-rover/releases/tag/v1.0.0)
"in ours": a scientific rover built step by step by an agent through typed
CAD tools, shown in the chat, with real mass properties and STEP, 3MF and
glTF exports. The engine grows into our own CAD engine with ClassCAD-level
functionality (#11628); the research behind the kernel choice is
[docs/research/cad/](../research/cad/README.md).

CAD Lab is [Video Lab](../video-lab/README.md) and
[Game Lab](../game-lab/README.md) for CAD. It is not a new app: a set of chat
tools, a small local service, a card every OpenAgents chat can show, and a
CLI. You say "add a hinged lid to the avionics bay and open it 100 degrees";
the chat changes the document, the whole model is rebuilt and checked, and
the new version turns in 3D right in the conversation with its mass and
centre of mass.

## The pieces

| Piece | Where | What it does |
| --- | --- | --- |
| The engine | `crates/cad` | `part.v1` and `assembly.v1` documents, the build, the checks, mass properties, exports, renders |
| The kernel | SolidCraft's crates on Truck, pinned to one commit (`crates/cad/Cargo.toml`) | Exact B-rep solids: sketches, extrude, revolve, sweep, loft, booleans, fillets, chamfers, shells, patterns, mirror, STEP and 3MF |
| The Lab | `crates/cad-lab` (`cad-lab serve`, 127.0.0.1:8794) | Versions, the `cad_*` tools at `/api/tools`, cards, files |
| The chat | `crates/openagents-web/src/cad_lab.rs`, `labs.rs` | The tools in the same Gemini 4 Argon turn as Video Lab's and Game Lab's, and the card with the WebGL view |
| The terminal | `openagents cad` (`crates/openagents-cli/src/cad.rs`) | The same tools from a shell |
| The local stack | `scripts/dev/full-local.sh start --cad` (or `--lab`, which starts all three Labs) | The service and the chat wired together |

## The kernel, and why

The research recommended SolidCraft's Rust crates (MIT OR Apache-2.0) on
the Truck B-rep kernel (Apache-2.0), behind a boundary of our own, over
Manifold (meshes only), OCCT (LGPL and C++) and ClassCAD (closed and
licensed). We followed it:

- **It does what the rover needs, exactly.** Sketches with text, extrude,
  revolve, booleans, fillets and chamfers on real edges, shells, circular
  and rectangular patterns, and exact STEP out and back in. A filleted plate
  with four holes measures within 0.5% of its analytic volume, and its STEP
  file reads back to the same volume (`crates/cad` tests).
- **It is Rust and builds for wasm32** (`cargo check --target
  wasm32-unknown-unknown -p cad` passes), so the same engine can run in the
  browser later. The Lab runs it natively.
- **Only `crates/cad/src/build.rs`, `refs.rs`, `render.rs` and `export.rs` touch it.**
  Callers see our documents and JSON reports, so the kernel can be swapped
  (SMLib, Apache-2.0, is the precision fallback the research names).
- **Licences:** SolidCraft MIT OR Apache-2.0; Truck Apache-2.0, including
  the fixed copies of `truck-shapeops`, `truck-stepio`, `truck-meshalgo`
  and `matext4cgmath` that SolidCraft carries (patched in at the workspace
  root from the same pinned commit). Nothing of theirs is copied into our
  tree; it builds from the pinned git commit.

## The documents

Both are plain JSON with schemas in [schemas/](schemas/) (`cad-lab schema`
prints them). Lengths are millimetres, angles degrees; X is forward, Y left,
Z up.

- **`part.v1`** is a feature history: `features` in order, each
  `{id, op, params}`, replayed from the top on every build. A feature is a
  **sketch** (`op: "sketch"`, a plane and drawing steps such as
  `sketch.rectangle.center`, `sketch.slot.center_to_center`,
  `sketch.text`, or our own `sketch.polyline` (`points` as `[x, y]` or
  `[x, y, bulge]`, the bulge making that segment a circular arc as DXF's
  polylines do, and `closed`), then optional `constraints` such as
  `{"type": "tangent", "a": "l1", "b": "a1"}` and `dimensions` such as
  `{"entities": ["l1"], "value": "width"}`, which the kernel's solver holds
  the drawing to; each sketch's remaining degrees of freedom are in the
  report), a **modelling operation** (`solid.extrude`,
  `solid.revolve`, `solid.fillet`, `solid.shell`, `solid.pattern.circular`,
  booleans through each feature's `operation`: join, cut, intersect, new),
  or a **block** (`op: "block"`): direct operations on solids run as one
  feature of the history and recomputed with it whenever an earlier
  feature or a parameter changes. A block declares the `inputs` it reads
  (earlier features and parameters, checked before the build), and a step
  may `repeat` (`{"var": "i", "from": 0, "to": 7}`: `{i}` in its params is
  replaced by each value, so `"8 * sin({i} * 45)"` becomes an expression):
  code-like generators in plain JSON. The same steps run with no document
  at all through `cad_solid` (the Solid API). A part has a material
  (density), a colour and optionally a finish (`gloss`, `satin`, `matte`:
  the glTF's roughness and the card's highlight); a solid of its own colour
  is one `appearance.assign` feature. Copies a pattern or mirror makes keep
  the colour of the solid they copy, even when it is coloured later (the
  kernel names copies anew, so each is matched to its source by name, by
  the order the kernel replays the sources in, or by volume).
- **`assembly.v1`** holds `parts` (part templates, by id), `assemblies`
  (subassembly templates: instances, joints and contacts of their own),
  `instances` that place them (`at`, `rotate`, relative to a `parent`
  instance), `joints` with values and limits, and `contacts`: pairs of
  instances meant to touch.
  - **Joints:** `fastened`; `revolute` (degrees about `axis` through
    `origin`); `slider` (mm along `axis`); `cylindrical` (`[degrees, mm]`);
    `planar` (`[u mm, v mm, degrees]`: slides in the plane normal to `axis`,
    `u` along `x_dir`, and turns about `axis`); `spherical` (`[rx, ry, rz]`
    degrees about `origin`, X then Y then Z); `gear` (no value: turns about
    its own `axis` by `ratio` times its `driver` joint's angle, negative for
    meshing gears, or `ratio` degrees per mm of a slider driver, a rack and
    pinion). Gears may drive gears; a loop is refused.
  - **Subassemblies:** an instance with `assembly: "bogie"` instead of a
    `part` places a copy of that template. It becomes a group (a frame with
    no solid) and the template's instances, joints and contacts are copied
    under it, named by path (`bogie_l.wheel`, `bogie_l.spin`), to any depth.
    Outer joints and contacts reach inner instances by path; a contact or
    joint naming a group covers everything inside it.
  - **Templates from other documents:** `cad_from_json` and `cad_assemble`
    take `{"from": {"doc": "wheel", "version": 3, "part": "wheel"}}` for a
    part or subassembly template; it's copied in at that version and keeps a
    `source` saying where it came from.

Every change makes a new **version**: the document, a one-line summary, who
made it, a digest, and its check report. Versions live privately under the
Lab folder, never in git:

```
~/work/film-out/lab/cad/<doc>/vN/{doc.json,version.json,model.glb,hero.png}
~/work/film-out/lab/cad/<doc>/vN/out/        renders and exports
~/work/film-out/lab/cad/.parts/<digest>.step built parts, reused while unchanged
```

## Exact mass, cross-checked

Mass properties come from the solids' triangles, so a curved part is off
its exact value by the display tolerance (a 60 mm wheel: 0.14% in volume,
0.28% in inertia). A part with `"exact_mass": true` (or `{"tolerance":
0.002}`; `cad_set_part exact_mass`) also has SMLib, the open NURBS kernel
(Apache-2.0), integrate over the exact surfaces of the part's STEP model on
every edit; when the two disagree by more than the tolerance (1% unless
the part says) in volume, centre of mass (against the part's size) or
inertia (against its largest moment), the edit is refused. The report's
`exact_mass` gives SMLib's numbers and the differences
(`crates/cad/src/exact.rs`).

SMLib runs through its Python binding `smlibpy` (MIT) in a separate
process with a short script of ours; nothing of it is linked or copied.
Install it with `python3 -m pip install smlibpy` (Python 3.10 or newer) and
point `CAD_SMLIB_PYTHON` at that Python if `python3` isn't it. Without it a
flagged part's entry says the exact check didn't run, and edits aren't
refused for that. Results are kept beside the part's STEP file, so an
unchanged part isn't measured again.

## Naming faces and edges

A fillet, chamfer, shell, draft or face colour names what it works on by
how it was made, not by a point, so an edit upstream (a longer plate, a
moved hole) doesn't make the pick miss (`crates/cad/src/refs.rs`):

- A **face** is `feature:role`: `body:top` and `body:start` (an extrude's
  caps), `body:side:l3` (the side swept from sketch curve `l3`; a
  rectangle's sides are `l1` bottom, `l2` right, `l3` top, `l4` left, and
  `sketch.line` takes `ids` of its own), `boss:+z` (a box's faces by
  direction), `round:blend:...` (a fillet's face), `hollow:inner:body:top`
  (a shell's inside). A face split in pieces is `name#...`; the name
  without the suffix means every piece.
- An **edge** is `{"between": [face, face]}` (`"*"` for any face) or every
  edge of a face, `{"of": face}`.
- `cad_faces(doc, part, after?)` lists a part's faces and edges by these
  names, with their centres and normals.

The names come from the kernel's own persistent naming, its feature numbers
translated to our feature ids, and are worked out again on every build. A
point `[x, y, z]` on the face or edge still works.

## The checks on every edit

An edit that fails any check is refused with the reason in plain words, and
nothing is saved:

1. **The document:** every field, id, reference, joint value and limit, and
   material is valid (`cad::doc::check_assembly`).
2. **The rebuild:** every feature of every part replays without an error
   (a missed edge pick, a profile that doesn't close, a boolean that fails).
3. **Closed solids:** every solid's shells close and it has volume.
4. **No interference:** no two instances overlap by more than 1 mm³ unless
   they are declared contacts or a joint joins them. Overlap is measured on
   the solids' triangles by sampling the box they share (28³ points), in
   parallel; the kernel's exact intersection took minutes on touching
   curved faces, so it isn't used for this.
5. **Mass properties** for every instance and the whole assembly: volume,
   mass from the material's density (in kg and g, six significant digits,
   so a 30 g toy keeps its inertia), centre of mass, and the inertia tensor
   about the centre of mass, by the divergence theorem over each solid's
   triangles (exact for the triangles; curved faces follow the kernel's
   display tolerance).

Each part builds in a kernel session of its own, side by side, and is kept
as a STEP file under its digest (the part, the parameters, the kernel
build). The assembly reads unchanged parts back (13 ms for a wheel that
takes 9 s to build), so an edit costs its own part's rebuild plus about a
second of placement and checks.

## Talking to it: the tools

The chat changes models only through typed tools; the model chooses which
tool runs from the tools it is offered, never from words matched in the
message (workspace rule: no keyword routing).

| Tool | Does |
| --- | --- |
| `cad_list` | The documents |
| `cad_open(doc, version?)` | The whole document, its report and the card |
| `cad_new_part(doc, part, material, color, finish?, features, at?, rotate?, parent?, place?)` | A new part with its first features, placed once (makes the document if it's new) |
| `cad_add_feature(doc, part, features, after?)` | More of a part's history |
| `cad_edit_feature(doc, part, feature, params? / op? / suppressed? / remove?)` | Change one feature; later ones rebuild from it |
| `cad_set_part(doc, part, title? / material? / color? / finish? / remove?)` | A part's properties, or remove it and its instances |
| `cad_assemble(doc, instances?, joints?, contacts?, templates?, remove?)` | Place, move or remove instances of parts or subassemblies; add or move joints; define subassembly templates; declare contacts |
| `cad_from_json(doc, document)` | A whole document at once (assembly from JSON), templates copied from other documents' versions |
| `cad_check(doc, version?)` | Rebuild and check a version again |
| `cad_versions(doc)` | Each version's summary, author, mass and parts |
| `cad_compare(doc, a?, b?)` | What changed: parts, features, instances, joints, mass and centre of mass |
| `cad_export(doc, version?, formats?)` | glTF binary (`.glb`, for the web and Verse), 3MF, STEP (exact solids) and STL; STEP (AP242) and 3MF keep the assembly tree: each part once in its own frame, placed by its instances, subassemblies as products or objects of their own |
| `cad_import_step(doc, part, feature, path? / step?)` | A STEP model (AP203, AP214 or AP242) as a feature of a part (`solid.import_step`, the file kept in the Lab's store by its SHA-256) |
| `cad_solid(ops, sketches?, material?, formats?)` | Direct operations with no document: mass, size and checks, and files if asked; nothing saved |
| `cad_import_dxf(doc, part, feature, dxf, plane?, offset?, scale?, at?)` | A DXF drawing as a new sketch: touching lines and arcs become polylines (JSON the part keeps), with circles, splines and text |
| `cad_export_dxf(doc, version?, part?, sketch?, plane?, at?)` | A sketch as DXF, or the whole model cut by a plane |
| `cad_faces(doc, part, after?)` | A part's faces and edges by the names features pick them with |
| `cad_render(doc, version?, view, focus?, yaw?, pitch?)` | A picture: hero, rear, plan, front, side, iso, under or detail, framing some instances; yaw and pitch turn and raise the camera to match a photograph |

## Seeing it: the card

Every CAD Lab answer has a card under it: the version turning in 3D (WebGL2
from the version's glTF; drag to turn, scroll to zoom; its picture when
WebGL isn't there), its mass and centre of mass, its size and counts, the
render or the exports the turn made, and **Compare with** the version
before.

## Where it runs

- `cad-lab serve [--port 8794] [--dir DIR]` binds to loopback only. The web
  chat reaches it through `OPENAGENTS_CAD_LAB` (a loopback URL) and serves
  its files at `/chat/{id}/cad/file` and `/chat/{id}/cad/out` to whoever
  may read the chat.
- `cad-lab tool NAME JSON` runs one tool without the service (scripts,
  tests).
- `cad-lab mcp` serves every `cad_*` tool to MCP clients over stdio
  (`crates/cad-lab/src/mcp.rs`): the same tools, schemas and checks as the
  chat, a refused edit coming back as a tool error with the reason. Add it
  to Claude Code with
  `claude mcp add cad-lab -- cargo run -q -p cad-lab -- mcp` from the repo
  (or the built binary's path in place of `cargo run ...`).
- `openagents cad list|open|check|versions|compare|render|export|tool`
  starts the service when it isn't running.

See [mvp.md](mvp.md) for the rover built with it and what is still missing.
