# Generated models with Blender

Status: proven, October 5, 2026; the plan below is proposed.

Verse's zones are built from CC0 kits, and the kits run out. The city map needs
landmarks no kit has, such as a fountain and an observatory. Wild Shape needs
animals. The demolition yard had to make its sledgehammer out of boxes in Rust.
Blender, driven by scripts, can fill that gap: agents write a Python script
that builds or converts a model, Blender runs it headless, and the glTF it
writes enters a zone pack like any kit model.

## What was proven

On October 5, with Blender 5.2.2 LTS on this Mac
(`/Applications/Blender.app/Contents/MacOS/Blender`):

- `scripts/blender/sledgehammer.py` builds a beveled iron-and-wood
  sledgehammer from code and writes binary glTF (18 KB).
- `scripts/blender/convert_fbx.py` converts the Spider from Quaternius's Easy
  Animated Enemy Pack (CC0, FBX only) to glTF with its armature and all five
  actions: Attack, Death, Idle, Jump, and Walk. The pack's materials import
  with alpha 0 and hashed blending, so they render invisible; the converter
  makes every material opaque.
- `scripts/blender/preview.py` renders a framed PNG of any glTF, so an agent
  can look at a model before admitting it.

Each runs headless:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/convert_fbx.py -- Spider.fbx spider.glb
$B -b --factory-startup --python scripts/blender/preview.py -- spider.glb spider.png
```

The scripts are asset-build tooling, an infrastructure exception like the
existing bake scripts. Product behavior stays in Rust.

## Scripts, not the MCP, for assets

[Blender MCP](https://pypi.org/project/blender-mcp) is a Blender add-on that
listens on `localhost:9876` and runs Python an MCP client sends it, with
Blender's window open. It's good for interactive work: the owner watches a
model take shape while an agent sends code.

A pack needs something else. Every committed model must be rebuilt from its
sources by anyone. So the rule is:

- **Committed assets come from scripts.** Each generated or converted model has
  a script under `scripts/blender/` that, given its pinned inputs, writes the
  glTF. The script is the model's source, reviewed like code.
- **The MCP is for sketching.** An agent may explore a shape interactively,
  then writes the script that reproduces it. Nothing reaches a pack from an
  interactive session.

## Where outputs go

- **Generated models** are written under
  `assets/verse/<zone>/generated/<name>.glb`. A `PROVENANCE` record names the
  script and its Blender version.
- **Converted models** keep their source license. The spider goes to
  `assets/verse/<zone>/beasts/` with the pack's CC0 notice and the source
  archive's name and digest.
- **Admission** is the existing path: the pack compiler reads admitted glTF,
  checks budgets, and the pack is repinned with the old digest retained.
- **Determinism:** Blender's glTF export isn't promised byte-identical across
  versions. Commit the generated `.glb` and treat the script as its recipe.
  CI-free rebuilds compare previews, not bytes. Rebuilding the pack itself
  remains #10622's concern.

## First models

| Model | How | For |
| --- | --- | --- |
| Giant Spider | Convert the CC0 FBX; scale up; rename actions to Verse's clip names | Wild Shape (#10610), Grove |
| Sledgehammer | `sledgehammer.py` | The demolition yard, replacing the Rust-built boxes |
| Fountain | Generated: stepped basin, column, bowls | Fountain Plaza on the city map |
| Observatory | Generated: drum tower, dome with slit, stair | Observatory Hill |
| Bandshell, boathouse, market stalls with awnings | Generated from simple solids with the kits' textures | The Commons, Lantern Quarter |
| Training dummy variants (armored, warded) | Kit dummy plus generated plates and runes | The Grove's dummy types |
| Stylized bear, wolf, eagle | Generated low-poly bodies on a simple rig, or a CC0 pack converted once found | Wild Shape, until real packs exist |
| Rat, frog, snake, wasp | Convert from the same CC0 pack | Ambient wildlife in Everglade's woods |

Generated buildings reuse the village kit's textures, so they sit with the
kit's pieces. Low-poly animals with a simple rig and a few keyed actions
(idle, walk, attack) are within reach of scripts. Detailed organic models are
not; those come from CC0 packs.

## How an agent makes a model

1. Write `scripts/blender/<name>.py` that builds the model from primitives,
   modifiers, and the kit's textures, at Verse's scale (1 unit = 1 m, +Y up
   after export).
2. Run it headless and render a preview with `preview.py`; look at the
   preview and iterate.
3. Admit the glTF into the zone's sources, run the pack tests, and capture the
   model in the zone with that zone's capture example.
4. Commit the script, the glTF, the provenance, and the repinned pack.

## Delivery

1. Convert the spider and admit it for Wild Shape (#10610).
2. Replace the demolition yard's sledgehammer with the generated one.
3. Generate the fountain and the observatory for the city map.
4. Generate stylized bear, wolf, and eagle for Wild Shape.
5. Optionally install Blender MCP for interactive sketching sessions with the
   owner.

## Open questions

- Whether Blender's path should be configurable (`BLENDER`) for other
  machines, including the Linux hosts.
- How many generated models a pack may carry before it needs its own budget.
