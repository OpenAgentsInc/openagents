# CAD Lab MVP (#11627)

What the first slice ships, how to run it, and what is still missing. The
design is in [README.md](README.md).

## Run it

```
scripts/dev/full-local.sh start --cad      # or --lab: Video, Game and CAD Lab
open http://127.0.0.1:4301                 # a new chat: "Start a rover ..."
openagents cad list
openagents cad render rover --view rear
openagents cad export rover --formats step,3mf,glb
```

## The rover (2026-10-14)

Built in one local web chat (`/chat/5ddd9179-cd96-4260-a245-a91c44f8ac69`
on http://127.0.0.1:4301) with twelve plain messages; Gemini 4 Argon chose
every `cad_*` call. 38 versions, each rebuilt and checked:

- a hollow aluminium chassis (760 × 440 × 170 mm, 40 mm rounded corners,
  5 mm walls) with a raised OA emblem on the rear;
- rocker-bogie suspension: two titanium rockers on revolute pivots (±25°)
  with steel shafts, two bogies on the rockers (±30°), six treaded wheels
  with lightening holes on spin joints;
- a mast with a stereo camera head on a pan joint;
- an avionics bay with a flight computer board and two battery packs under
  a hinged lid (v37 opens it 105°, v38 closes it);
- a sample cassette with six capped sample tubes;
- a high-gain dish, two solar wings, deck rails and two hazard cameras.

When the open lid struck the antenna, the overlap check refused it and the
chat moved the antenna (v36-v37); `cad_set_part from_version` brought the
antenna back after earlier attempts.

v38: 18 parts, 35 placed, 51 solids, 79 features, 12 joints,
909 × 980 × 1122 mm, **87.94 kg**, centre of mass **(16.0, −9.7, 310.2) mm**
(X forward, Y left, Z up from the ground), inertia about it
Ixx 15.52, Iyy 11.85, Izz 14.28 kg·m². An edit rebuilds and checks the whole
rover in about 1.5 s once its parts are built.

Outputs (owner's machine): `~/Downloads/cad-lab-rover-build.mp4` (1080p),
`cad-lab-rover-{hero,rear,plan,open-avionics,suspension,camera-head,sample-cassette}.png`,
`cad-lab-rover.{step,3mf,glb}`. The film is drawn with
`cad-lab frames rover OUT --from N --to N --fit 38`, one camera for every
version.

## Shipped

- `crates/cad`: `part.v1` and `assembly.v1` with plain-words validation and
  JSON Schemas; the build on SolidCraft's Truck-based kernel (pinned),
  parts built side by side and cached by digest; placements with parent
  frames and revolute, slider, cylindrical and fastened joints with limits;
  closed-solid, interference and mass-property checks; STEP, 3MF, STL and
  glTF exports; CPU renders from named views.
- `crates/cad-lab`: versions with summary, author, digest and report; the
  twelve `cad_*` tools; cards; files.
- The web chat: the tools in the Labs' one Gemini turn, the card with the
  WebGL view, Compare.
- `openagents cad`, and `full-local.sh --cad` (in `--lab`).

## Gaps, largest first

1. **No joint solver:** every joint kind moves (fastened, revolute, slider,
   cylindrical, planar, spherical, gear; #11628), but instances are placed
   by their parents' frames, so closed loops and driving by constraint are
   not solved (#11646).
2. **Stable references:** done (#11628): features name faces and edges
   (`body:top`, `{"between": [...]}`), resolved through the kernel's
   persistent naming on every build; points still work.
3. **Assembly structure in exports:** done for STEP (AP242) and 3MF
   (#11628): parts once, placed by instances, subassemblies nested. URDF
   from `assembly.v1` with the measured mass and inertia and our joints
   is shipped (#11643; Robot Lab imports it as `cad:<doc>/vN`, without
   collision shapes, so inspect and replay only), as is the typed mass
   feed for Lagrange 2.
4. **Interference by sampling:** overlaps are measured on triangles over a
   28³ grid; slivers thinner than a grid cell can be missed. Exact checks
   wait on a faster kernel intersection for touching curved faces (#11648).
5. **Mass from triangles:** exact for the triangles; curved parts differ from
   the exact solid by the display tolerance (well under 1% on the rover's
   parts). Parts flagged `exact_mass` are cross-checked against SMLib's
   exact integrals on every edit when `smlibpy` is installed (#11628).
6. **Verse:** `cad_place` puts a version in Everglade or a Meteor stage
   (#11642); the CAD card has no "Put it in Verse" button yet (the chat
   calls the tool), and the browser Verse build doesn't load placements.
7. **Browser builds:** `crates/cad-wasm` runs the engine as WebAssembly
   (check, Solid API, faces, DXF, exports; `scripts/cad-wasm-test.sh`), but
   no web page uses it yet: the web chat still calls the local Lab and
   views the glTF.
8. **DXF and STEP import:** done (#11628). 2D drawings with projected and
   hidden lines, and sheet metal, are not exposed (see
   [parity.md](parity.md)).
