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
    pinion). Gears may drive gears; a loop is refused. A joint's `origin`
    and `axis` may be a **mate** instead of numbers: `{"face":
    "base:+z"}` (a flat face's centre and normal; a round face's axis and
    the point on it nearest the face's centre) or `{"edge": ["base:+z",
    "base:+x"]}` (a circle's centre and normal, a straight edge's middle and
    direction), on the parent's part, `"flip": true` to turn the direction
    round. They are worked out from the part's named faces on every build
    (the frames are kept with the built part), so a mate follows its face
    through edits.
  - **Mechanisms (loops):** a joint with `"closes": true` places nothing;
    it holds two instances the tree already places together, closing a
    loop (a four-bar's last pin, a rocker-bogie pinned at both ends):
    `origin`/`axis`/`x_dir` in the parent's frame (no `parent`: fastened to
    the assembly's origin), `child_origin`/`child_axis`/`child_x_dir` in the
    child's (mates allowed). Its type says what it holds: `fastened`,
    `revolute`, `slider`, `cylindrical`, `planar`, `spherical`, or
    `parallel` (axes only); `{"type": "gear", "closes": true, "driver":
    "a", "driven": "b", "ratio": 2}` couples two joints. The solver
    (`crates/cad/src/solve.rs`, #11646) takes the values of the joints
    around each loop as unknowns and runs damped Newton steps
    (Levenberg–Marquardt) from the values written, inside each joint's
    limits, so a mechanism keeps the branch it was drawn in; a joint with
    `"drive": true` keeps its value and the rest follow. The report's
    `solver` gives each loop's joints, its residual (under 10⁻⁹ in the
    four-bar test), `mobility` (a four-bar's is 1), `dof` (left once the
    driven joints are held), and `redundant` equations (a planar linkage
    written in 3D repeats three). A loop that can't close (a link too
    short, a joint at its limit, more joints driven than it can take) is
    refused with the reason, never forced. `cad_assemble drive` sets one
    joint and saves where the others went; `cad_motion` moves a joint
    through a range and says where a limit or a dead point stops it
    (ClassCAD's `moveUnderConstraints`).
  - **Subassemblies:** an instance with `assembly: "bogie"` instead of a
    `part` places a copy of that template. It becomes a group (a frame with
    no solid) and the template's instances, joints and contacts are copied
    under it, named by path (`bogie_l.wheel`, `bogie_l.spin`), to any depth.
    Outer joints and contacts reach inner instances by path; a contact or
    joint naming a group covers everything inside it.
  - **Instance patterns:** an instance's `pattern` places copies of it
    (and of a subassembly's whole tree): a row or grid (`{"count": 3,
    "step": [60, 0, 0], "count2": 2, "step2": [0, 80, 0]}`) or a ring
    about an axis (`{"count": 6, "axis": [0, 0, 1], "origin": [0, 0, 0]}`,
    `angle` between copies optional). The first copy keeps the id; the
    rest are `id-2`, `id-3`, ...
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
   (a missed edge pick, a profile that doesn't close, a boolean that fails),
   and every loop of joints closes (see Mechanisms below).
3. **Closed solids:** every solid's shells close and it has volume.
4. **No interference:** no two instances overlap by more than 1 mm³ unless
   they are declared contacts or a joint joins them. The overlap volume is
   exact on the solids' triangles (`crates/cad/src/clash.rs`, #11648): the
   volume inside both is a third of the flux of `x − x0` through the part
   of each surface inside the other, so each triangle adds its height
   times its area inside the other solid; triangles nobody crosses take one
   point-in-solid test, crossed ones the area Green's theorem gives from
   the cut segments and their own edges' inside pieces, and both meshes'
   triangles sit in bounding-volume hierarchies. A 0.2 mm slab the old
   28³ sampler stepped over is caught to the cubic millimetre. Faces
   resting on each other are **touching**, not interference, and the
   report lists them (`checks.overlap_check.touching`). Pairs of
   flat-faced solids that do overlap are measured again by the kernel's
   B-rep intersection (exact, and quick without curved faces, where it is
   slow). Each pair's result is remembered by its two meshes' fingerprints
   while the Lab runs, so an edit measures only the pairs it moved or
   changed: on the rover (35 instances, 53 pairs whose boxes meet) the
   whole check takes 10 ms in parallel (the old sampler took 165 ms on 40
   pairs), the same check again 1 ms, and moving the sample cassette 1 mm
   re-measures its 13 pairs in 7 ms (`interference_time`, release build).
   Curved faces are measured on their triangles, which stand off the true
   surface by at most the display tolerance.
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
| `cad_assemble(doc, instances?, joints?, contacts?, templates?, remove?, drive?)` | Place, move or remove instances of parts or subassemblies; add or move joints (loops and mates too); define subassembly templates; declare contacts; drive one joint of a mechanism (`{"joint": "crank", "value": 45}`) with the others following |
| `cad_motion(doc, joint, from?, to?, steps?)` | Move a joint through a range with every loop closed at each step; where each joint goes and what stops it; nothing saved |
| `cad_from_json(doc, document)` | A whole document at once (assembly from JSON), templates copied from other documents' versions |
| `cad_check(doc, version?)` | Rebuild and check a version again |
| `cad_versions(doc)` | Each version's summary, author, mass and parts |
| `cad_compare(doc, a?, b?)` | What changed: parts, features, instances, joints, mass and centre of mass |
| `cad_export(doc, version?, formats?)` | glTF binary (`.glb`, for the web and Verse), 3MF, STEP (exact solids), STL, `urdf` (a robot for Robot Lab) and `feed` (mass properties for Lagrange 2); STEP (AP242) and 3MF keep the assembly tree: each part once in its own frame, placed by its instances, subassemblies as products or objects of their own |
| `cad_import_step(doc, part, feature, path? / step?)` | A STEP model (AP203, AP214 or AP242) as a feature of a part (`solid.import_step`, the file kept in the Lab's store by its SHA-256) |
| `cad_solid(ops, sketches?, material?, formats?)` | Direct operations with no document: mass, size and checks, and files if asked; nothing saved |
| `cad_import_dxf(doc, part, feature, dxf, plane?, offset?, scale?, at?)` | A DXF drawing as a new sketch: touching lines and arcs become polylines (JSON the part keeps), with circles, splines and text |
| `cad_export_dxf(doc, version?, part?, sketch?, plane?, at?)` | A sketch as DXF, or the whole model cut by a plane |
| `cad_faces(doc, part, after?)` | A part's faces and edges by the names features pick them with |
| `cad_place(doc, version?, zone?, at?, heading?, scale?, remove?)` | Put a version in a Verse zone (Everglade, Meteor Showcase, Meteor Stress Test); a newer version of the same document replaces it |
| `cad_drawing(doc, version?, views?, sections?, dimensions?, sheet?, angle?, title?, hidden?, formats?)` | A 2D drawing: projected views with hidden lines, sections, dimensions and a title block, as SVG, PDF and DXF (see Drawings) |
| `cad_render(doc, version?, view, focus?, yaw?, pitch?)` | A picture: hero, rear, plan, front, side, iso, under or detail, framing some instances; yaw and pitch turn and raise the camera to match a photograph |

## Drawings

`cad_drawing` (and `openagents cad drawing DOC`) draws a version on a
sheet, projected from its solids (`crates/cad/src/drawing.rs`):

- **Views:** `front` looks at the model's front (+X) with Z up; `top`
  looks down with the front at the foot of the view; `right` looks from
  the right of the front view (+Y); `left`, `bottom` and `back` are their
  opposites; `iso` looks from (1, 1, 1). Third-angle projection (the top
  view above the front, the right view to its right) unless `angle` is
  `first`. One standard scale fits every view on an A4, A3 or A2 sheet.
- **Lines:** every B-rep edge except seams and smooth (tangent) joins, and
  the silhouettes of curved faces, found on the circular arc each mesh
  edge stands for, so a cylinder's silhouette is on the cylinder. A line
  is split where it passes behind another and each piece tested against
  the triangles in front of it: hidden lines are dashed. Where lines fall
  on each other a section's outline wins over a visible line and a visible
  line over a hidden one. The iso view and sections show no hidden lines.
- **Sections:** `sections: [{"view": "front", "at": 0}]` cuts the model
  with a plane square to that view (through the middle when `at` is left
  out), takes away what is in front, draws the cut's outline and hatches
  it at 45°, and marks the cut with a chain line on the view it crosses.
- **Dimensions:** the front view's width and height and the top (or
  right) view's depth; on a part's drawing, each hole size once with how
  many there are (`4× Ø3.4`); and distances between named parallel flat
  faces (`dimensions: [{"part": "plate", "faces": ["body:start",
  "body:top"], "view": "front"}]`).
- **Files:** SVG and PDF (vector, Helvetica) of the sheet, and DXF
  (AutoCAD R12, sheet millimetres) with a layer per kind of line:
  VISIBLE, HIDDEN (DASHED), SECTION, HATCH, CUTTING (CENTER), DIMENSION,
  FRAME and TEXT.

A drilled block's views have exactly the lines they should (four visible
and two hidden in the front view, the hidden ones at ±5 mm), its section
is two closed rectangles of cut material, and the drawing writes as all
three files (`crates/cad/src/drawing.rs` tests). The rover (38 parts)
draws its four views and a section in about 10 s.

## Seeing it: the card

Every CAD Lab answer has a card under it: the version turning in 3D (WebGL2
from the version's `view.json`, or an older version's glTF; drag to turn,
right-drag to move, scroll or pinch to zoom, tap a face or edge to see its
name, Measure, Section, Explode and Parts; its picture when WebGL isn't
there), its mass and centre of mass, its size and counts, the
render or the exports the turn made, and **Compare with** the version
before.

On a saved design's page, pick an extrusion's end, a box face, or a fillet's
rounded face, then drag **Pull** along its normal. The browser rebuilds and
checks the preview with the Rust CAD engine. Releasing saves a checked version;
**Cancel** or Escape restores the starting shape. You can also enter a movement
in millimeters, select **Preview**, and select **Save change**. Box pulls keep
the opposite face fixed. Cuts, formulas, and unsupported faces stay editable
through chat. Viewers cannot pull faces, and a page based on an older version
must be reloaded before saving ([#11661](https://github.com/OpenAgentsInc/openagents/issues/11661)).

**Share with sliders** opens the owner's configurator settings. Choose up to
eight numeric feature fields or literal part parameters, name their sliders,
and set limits and steps. The link shares that version's complete design;
visitors can rebuild it while signed out and download STEP, STL, PDF, or DXF
for their values. Their edits stay in their browser and don't create versions
in your account. **Stop sharing** removes a link; deleting the design removes
its links too. A previously downloaded design stays with its recipient.
Each design allows 20 links, with an 8 MiB snapshot limit
([#11662](https://github.com/OpenAgentsInc/openagents/issues/11662)).

## In the browser

`crates/cad-wasm` is the same engine built for `wasm32-unknown-unknown`
(SolidCraft's kernel builds for it; our crate swaps threads for a loop,
the disk for the kernel's in-memory file store, and the clock for none,
in `crates/cad/src/sys.rs`). A page holds its documents and calls
`run(tool, args)` for `cad_check`, `cad_solid`, `cad_faces`,
`cad_import_dxf`, `cad_export_dxf` and `cad_drawing` (its report and
SVG; `drawing_file(args, format)` for the PDF or DXF), with the Lab's arguments and
answers, and `export_file(document, format)` for glb, STEP, 3MF or STL
bytes. Versions, renders and the file store stay with the Lab.

`scripts/cad-wasm-test.sh` builds it, runs `wasm-bindgen`, and drives it
under Node: a filleted plate with a hole rebuilds and checks in about
180 ms to within 0.01% of its analytic volume, and exports all four formats.
The `.wasm` is 26 MB before `wasm-opt` and compression.

**On openagents.com** (`/cad`, #11649): a sample plate with four holes
and five sliders (length, width, thickness, corner radius, hole size).
Each move rebuilds, checks and draws it in the browser, with no request
to us; its mass, volume and size, and STEP, STL and drawing (PDF, DXF)
files, come from the same build. The plate's numbers stay in the browser's
storage. `scripts/build-cad-web.sh` builds what the site serves (the
`cad-web` profile: size-optimized, link-time optimized, then `wasm-opt
-Oz`, then brotli and gzip copies):

| | Bytes |
| --- | --- |
| Before (release, as above) | 26 MB |
| Built (`cad-web` profile) | 15.2 MB |
| After `wasm-opt -Oz` | 12.5 MB |
| Shipped, brotli | 3.3 MB (gzip 4.8 MB) |

Only `/cad` loads it (`crates/openagents-web/src/pages/cad.rs`), in a
worker (`crates/cad-wasm/web/`), so the page keeps answering while a part
builds; the site sends the brotli copy when the browser takes it
(`serve_build`). Measured on a Mac (M-series): the engine loads and the
first plate is drawn 0.9 s after the page opens (served locally); a slider
move rebuilds, checks and draws the plate in 0.3 to 0.4 s, or about 25 ms
when it's a shape already built. The rover's wheel (18 treads patterned
round a drum, recessed both sides) rebuilds from nothing in 28 s in the
browser build (9 s natively, where parts build side by side) and in 32 ms
once built; its drawing takes 75 ms.

## Into Verse (#11642)

`cad_place` (or `openagents cad place rover --zone everglade --at 3,-4
--heading 150`) writes the version's glTF, zone, place and heading into
`ROOT/.verse/placements.json` (`verse-cad-placements.v1`;
`$VERSE_CAD_PLACEMENTS` moves it). The zone loads that file
(`crates/verse/src/zones/cad_place.rs`): every placement named for it
stands on the ground, its lowest point resting there, and a change to the
file shows within a second. One placement per document and zone, keeping
its version, so placing v39 replaces v38. With no `at`, the model stands
in front of where the player arrives. Native Verse only; the browser build
reads no local files. `cad_place_capture OUT.png` (a `verse` example)
photographs it.

## Robot Lab and Lagrange 2 (#11643)

- **URDF** (`cad_export` `urdf`, `crates/cad/src/urdf.rs`): a link per
  instance under a massless `base_link` at the ground, each with the
  report's mass, centre of mass and inertia carried into its link frame
  (metres, kg·m²), and the joints: revolute → `revolute` (radians;
  unlimited or past a turn → `continuous`), slider → `prismatic` (metres),
  cylindrical → `prismatic` to a `<child>_slide` link plus `revolute`,
  fastened or none → `fixed`. A moving instance's link frame sits at its
  joint's origin, so URDF turns it about the right point. Each instance's
  solids are STL visuals by colour. The URDF's zero pose is every joint at
  zero. It is written as a pinned folder, `vN/out/urdf/` with
  `source.json` (revision: the SHA-256 of the version's document), which
  Robot Lab's `robot_import` reads through `robot-description` as source
  `cad:<doc>/vN` and stands on a floor (inspect and replay; the URDF has
  no contact shapes for simulation yet). A round-trip test reads it back
  with `robot-description` and gets every link's mass, centre of mass
  (0.06 mm) and inertia (10⁻⁶ relative) and every joint's kind and limits.
- **Mass feed** (`cad_export` `feed`, `cad::feed::MassFeed`,
  `cad-mass-feed.v1`): the typed record Lagrange 2's spacecraft document
  reads: per instance its part, material, mass, centre of mass and inertia
  (SI, assembly axes) and a port per joint on both sides (origin, axis,
  kind, values, limits), with the parallel-axis total.

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
- **On openagents.com** (#11653, `crates/openagents-web/src/cad_web.rs`):
  every signed-in web chat gets the same tools (except `cad_place`, a STEP
  file by path, and URDF), with no Lab on a Mac. The turn runs on the site,
  so each tool call runs `cad-lab tool NAME -` (shipped in the web image)
  as a child process over a working copy of the person's documents: one at
  a time, within 75 s and 640 MB, at most 150 calls a person an hour,
  arguments up to 256 KB and a document up to 512 KB. Documents and
  versions live in the private chat store under the account, or under the
  project's home account when the chat is in a project; viewers read a
  project's models and editors change them. Each version keeps
  `view.json` (`crates/cad/src/view.rs`): the triangles with each face's
  stable name, so the card's viewer (`static/cad-view.js`) shows
  `body:top` or `{between: [..]}` when a face or edge is picked and puts
  it in the composer. The viewer also measures, cuts with a section
  plane, explodes an assembly and hides parts. `/cad/{doc}` (with
  `?project=` for a project's) lists the versions and downloads GLB, and
  STEP, STL, 3MF and PDF/DXF drawings made in the browser by
  `crates/cad-wasm`; a project's page lists its designs.

See [mvp.md](mvp.md) for the rover built with it and what is still missing,
and [parity.md](parity.md) for how the engine covers ClassCAD's documented
functionality (#11628).
