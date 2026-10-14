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

1. **Joints that don't move yet:** planar, spherical and gear joints are in
   the schema and refused plainly; no joint solver (closed loops, drive by
   constraint). Instances are placed by their parents' frames.
2. **Stable references:** fillets, chamfers and shells pick edges and faces
   by a point in the part's frame; a change that moves the edge can make a
   pick miss. The kernel has persistent face and edge history; the document
   doesn't use it yet.
3. **Assembly structure in exports:** STEP and 3MF carry every placed solid
   flat, not an assembly tree with instances; URDF (the kernel writes it) is
   not wired to our joints yet. Robot Lab and Lagrange 2 read mass from the
   report today.
4. **Interference by sampling:** overlaps are measured on triangles over a
   28³ grid; slivers thinner than a grid cell can be missed. Exact checks
   wait on a faster kernel intersection for touching curved faces.
5. **Mass from triangles:** exact for the triangles; curved parts differ from
   the exact solid by the display tolerance (well under 1% on the rover's
   parts). SMLib's exact integrals are the planned cross-check.
6. **Verse:** the glTF is ready for it; placing a version in a Verse zone is
   a follow-up.
7. **Browser builds:** the engine compiles for wasm32; the Lab runs it
   natively and the browser only views the glTF.
8. **DXF, drawings, STEP import as a feature, sheet metal**: the kernel has
   them; the documents don't expose them yet (#11628).
