# Runbook: study a kit, build our own models in Blender

This runbook is for an agent who turns an asset kit, such as Quaternius's
Medieval Village MegaKit, into models of our own: variations that look like
the kit but aren't copies of its catalog, and reference models built from
scratch in its style. It records the process Everglade's rounds used on
October 5, 2026: 9 village buildings, 14 more buildings, 30-odd props, the
landmarks, animals, levels of detail, and their admission into the pack.
Repeat it for any kit and any zone.

Read [Generated models with Blender](blender-pipeline.md) for the pipeline's
design. This page is the procedure.

## Decide the mode first

Every model you make takes one of three modes. Write the mode down in the
model's provenance before you start; it decides what you may commit.

| Mode | What it is | Example | What ships |
| --- | --- | --- | --- |
| **Keep** | The kit's model, admitted as it is | A Quaternius tree in `assets/verse/everglade/nature/` | The kit file, pinned in a source manifest |
| **Compose** | A new whole model assembled from the kit's own pieces and textures | `townhouse_jettied`: kit walls, windows, roofs, and brackets, stacked three stories with a jetty | Our model, made of the kit's pieces; the kit's license applies |
| **Reference** | A new model built from primitives, in the kit's style, using at most the kit's textures | The observatory, the fountain, the bear | Our model; textures, if any, from the kit |

Rules that apply to all three:

- **License first.** Only use kits whose license allows redistribution and
  derivative works. Quaternius's kits are CC0 1.0: read the kit's
  `License_Standard.txt` and record its SHA-256. If a license is unclear,
  stop and ask the owner. Never use art from commercial games, such as the
  removed WoW tooling's sources: study techniques only (see
  [particles](particles.md)).
- **Kits stay out of git.** The owner downloads kits to `~/Downloads/<Kit
  name>[Standard]/`. Commit only what ships: admitted kit files (Keep), and
  the models we make (Compose, Reference) with their scripts.
- **Scripts are the source.** Every Compose and Reference model is made by a
  Python script under `scripts/blender/` that Blender runs headless. Nothing
  reaches a pack from an interactive session; the
  [Blender MCP](blender-pipeline.md#scripts-not-the-mcp-for-assets) is for
  sketching only.

## Before you begin

- Blender 5.2 or later at `/Applications/Blender.app/Contents/MacOS/Blender`
  (override with `BLENDER=`). Run it headless:
  `Blender -b --factory-startup --python SCRIPT -- ARGS`.
- Python 3 with NumPy and Pillow, for the admission script.
- A build directory of your own (`CARGO_TARGET_DIR=~/work/openagents-target-agentN`),
  and at least 15 GB free disk (`df -h ~`). Stop and report if it's lower.
- Claim the work: if it's an issue, comment `Claimed by an agent: starting
  work now.` before you begin, as [AGENTS.md](../../AGENTS.md) says.

## Step 1: Study the kit

Before you build anything, learn the kit's style, so your models sit beside
its pieces without looking foreign.

1. List the kit: `ls "~/Downloads/<Kit>[Standard]/glTF"`. Note the formats
   offered (glTF, FBX, OBJ, Blender). Prefer glTF; convert FBX with
   `scripts/blender/convert_fbx.py` (it also fixes the alpha-0 materials some
   FBX exports carry).
2. Render the pieces you'll use. `scripts/blender/preview.py -- IN.glb OUT.png`
   renders a framed preview of one model; `scripts/blender/building_views.py`
   renders a building from three-quarter, street-level, rear, and front
   views. Look at every render.
3. Write down the style rules you see, in the script's header comment. For
   the Medieval Village kit they were:
   - a 2 m grid for walls, with floors about 3 m apart;
   - timber framing in a regular rhythm, darker timber on darker houses;
   - stone plinths under plaster walls;
   - steep tiled roofs with overhangs and gables, roof moss in the tile
     texture;
   - upper floors that jut out over the street on beam-end brackets;
   - arched and rectangular windows, round-topped doors.
4. Measure the pieces you'll reuse: their bounds, their origin, and which
   way they face. Kit pieces often face a different axis than glTF's front.
   Record the frame in the script: in Blender our fronts face -Y, which the
   glTF export turns into +Z.

## Step 2: Plan the models

Before writing code, list what to make and why:

- **Fill gaps the map names.** Read the zone's map
  ([Everglade's](everglade-map.svg)) and its layout doc
  ([Everglade](everglade.md#layout)). Prefer building types the map wants
  but the town lacks or repeats too often.
- **Vary, don't duplicate.** A variation should differ in shape, not just
  color: a second story, a jetty, a dormer, a turret, a porch, an L-shaped
  wing, a different roof form (hip, gambrel, cone), a different footprint.
  Color variation comes free later (step 4).
- **Budget each model** before you build it:

  | Kind | Triangles |
  | --- | --- |
  | Small prop (bench, sign, lamp, planter) | under 1,000 |
  | Animal or creature | under 3,000 (ambient wildlife under 700) |
  | House or shop | under 10,000 |
  | Landmark or large hall | under 20,000 (the pack's per-model limit) |

  Check the pack's current budgets in
  [`everglade_pack.rs`](../../crates/verse-zone-everglade/src/zones/everglade_pack.rs) and
  [Everglade](everglade.md#admission-and-the-zone-pack): pack size,
  committed size, pack triangles, placed and drawn triangles. If a round
  would exceed one, make room first (round 3 shrank the pack from 28 MB to
  8.7 MB) or ask the owner.

## Step 3: Write the generator script

Put the script under `scripts/blender/`, one per family of models
(`buildings.py`, `town_props.py`, `wildlife.py`). Follow the existing
scripts' shape:

```python
"""<What it builds>.

Run headless:
    Blender -b --factory-startup --python scripts/blender/<name>.py -- \
        OUT_DIR [NAME ...] [--kit KIT_DIR]

<Style rules from step 1. The frame: 1 unit = 1 m; fronts face -Y in
Blender, +Z after export; origin on the ground at the front wall's center
(a landmark's at its base's center).>
"""
```

- **One function per model**, registered in a table, so `NAME ...` builds a
  subset. Each starts from an empty scene
  (`bpy.ops.wm.read_factory_settings(use_empty=True)`).
- **Compose models** import kit pieces with `bpy.ops.import_scene.gltf`,
  place them on the kit's grid, and join them. `buildings.py` has helpers
  for walls, stories, jetties, roofs, gables, dormers, chimneys, and steps;
  reuse them rather than writing new ones.
- **Reference models** build from primitives (`primitive_cube_add`,
  `primitive_cylinder_add`, `bmesh`), with bevels for softness and low
  segment counts. Give every material a clear name.
- **Textures.** Reuse the kit's textures by name so admission can point at
  the already admitted image (step 6); don't paint new ones unless the model
  needs them. Recolor plaster and roof tiles through material factors rather
  than new images.
- **Animated models.** Build a simple armature, parent parts to bones, and
  key a few actions (`idle`, `walk`, and the model's own, such as `flap` or
  `attack`). Name actions in Verse's clip vocabulary. Converted creatures
  keep their source actions, renamed.
- **Collision data.** Write `<name>.footprint.json` beside each building:
  its frame, triangle count, and named collision boxes (walls, wings,
  plinths). Follow the real outline: a curved wall is a chain of small
  boxes, and a stage, steps, deck, or porch is a walkable surface, never one
  rectangle around the whole model.
- **Export** with `bpy.ops.export_scene.gltf(export_format="GLB",
  export_apply=True)` to `assets/verse/generated/<family>/<name>.glb`, and
  print one `MODEL name triangles` line so `build-models.sh` can report it.
- **Determinism.** Seed any randomness from the model's name. Rerunning the
  script must give the same model.

## Step 4: Review the models

Look at every model before you admit it. Rejecting a model here is cheap;
fixing it in the zone isn't.

1. Render previews: `scripts/blender/preview.py`, and for buildings
   `scripts/blender/building_views.py` (four views, close-ups, and a
   footprint overlay that draws the collision boxes in red over the model).
2. Render the gallery: `scripts/blender/build-models.sh GALLERY_DIR` rebuilds
   every model and renders one labeled overview plus a preview each.
   `GALLERY_ONLY=1` renders from the committed models without rebuilding.
3. Check each model for:
   - recognizable at a glance, and in the kit's style;
   - no gaps between pieces, no z-fighting, no floating parts, no inverted
     normals;
   - correct scale against a 1.8 m person, and the origin and front facing
     right;
   - textures aligned, not stretched across a joint;
   - collision boxes that hug the model (the red overlay);
   - within its triangle budget.
4. Fix and rerender until every model passes. Copy the gallery to
   `/private/tmp/claude-501/` (or your scratchpad) and include it in your
   report so the owner can see all models at once.

## Step 5: Record provenance

Every output folder has a `PROVENANCE.md`. For each model, record:

- the mode (Keep, Compose, or Reference);
- the script and the exact command that built it, and the Blender version;
- the source kit's name, where the owner downloaded it, its license, and
  the license file's SHA-256;
- for Compose models, every kit piece and texture used;
- for converted models, the source archive's name and SHA-256.

The credit line for Quaternius kits is "Credit: Quaternius". Follow
[`assets/verse/generated/buildings/PROVENANCE.md`](../../assets/verse/generated/buildings/PROVENANCE.md).

## Step 6: Admit the models into a zone pack

The pack compiler reads glTF with separate `.bin` buffers and PNG images,
and only images it has admitted. A binary `.glb` with embedded JPEGs isn't
admissible as it is.

1. Run the admission script: `python3 scripts/blender/everglade_admit.py`
   (add your models to its `MODELS` table). It converts each `.glb` to
   `assets/verse/everglade/generated/<name>.gltf` and `.bin` without
   touching the geometry, points every textured material at the kit image
   already admitted (deriving neutral images for recolored plaster and roof
   tiles), and rewrites the source manifests with digests.
2. Animated models go in the pack's optional animated section;
   `scripts/blender/beasts_admit.py` admits the Wild Shape forms and is the
   pattern.
3. For heavy models, add far levels of detail:
   `Blender -b --factory-startup --python scripts/blender/everglade_lod.py -- NAME`
   writes a simplified version into the pack's `lod` set.
4. Compile and repin the pack:

   ```sh
   cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
   ```

   It writes `<sha256>.vtp`, removes the earlier pack, and prints the
   `PACK_SHA256` and `PACK_BYTES` to pin in
   [`everglade_pack.rs`](../../crates/verse-zone-everglade/src/zones/everglade_pack.rs).
   Move the old digest into `EVERGLADE_PACK_HISTORY`; never delete history.
   Repin once per round, at the end, after rebasing on other agents' pack
   changes. The compile isn't yet bit-identical across machines (#10622), so
   another machine's digest may need adding to the history too.
5. Check it: `cargo run --release -p verse --example everglade_pack -- assets/verse/everglade --check`.

## Step 7: Place the models

1. Place them in the zone's layout code; for Everglade, the modules under
   [`crates/verse-zone-everglade/src/zones/everglade/layout/`](../../crates/verse-zone-everglade/src/zones/everglade/layout/)
   (`city.rs` for buildings, `greens.rs` and `parks.rs` for parks and
   props, `streets.rs` for street detail). Generated buildings are placed
   through
   [`layout/generated.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/generated.rs),
   which turns each footprint into blockers, its roofs into landing
   surfaces, and its door into a reachable step.
2. Follow the map's districts. Mix generated buildings with kit-built ones
   so streets vary; avoid repeating one model in a row.
3. Keep the zone's invariants: in Everglade, the workshop, Agent Studio's
   stations, the yard, and the spawn view keep working;
   every door is reachable from spawn; no blocker covers a road.
4. Small props must cull with distance; check the drawn-triangle budget.

## Step 8: Verify in the zone

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-agentN
cargo test -p verse --lib -- everglade everglade_pack demolition fx
cargo fmt
cargo check -p everglade-web --target wasm32-unknown-unknown
cargo check -p coder-mobile
```

Then capture the models in the world with the zone's capture example. For
Everglade, `cargo run --release -p verse --example everglade_capture`
accepts named views, a free camera (`at:X,Z,YAW,TILT`), and an aerial one
(`air:EX,EY,EZ,TX,TZ`). Capture each placed model at street level and the
district from above, and look at every capture: floating or sinking models,
clipping, wrong facing or scale, colliders that don't match (walk into the
model in a capture view), and empty or repetitive stretches. Report the
triangle numbers from the test `a_frame_draws_a_fraction_of_the_city`
before and after, and the pack size.

## Step 9: Commit and report

- Commit the scripts, the generated `.glb` and footprint files, the
  provenance, the admitted sources and manifests, the repinned pack, the
  layout changes, and a short update to the zone's doc, in one or a few
  commits. Commit messages follow the
  [Google developer style](../../.agents/skills/google-developer-style/SKILL.md)
  and end with the session's co-author trailer.
- Rebase on `origin/main`, rebuild and rerun the checks after the rebase,
  then push. Check that your commits touch only your files.
- Report: the models made (name, mode, triangles), where each was placed,
  the pack size and triangle numbers before and after, the gallery and
  capture paths, and anything you skipped and why.

## Keep or replace the originals

When a round makes a model that does the same job as a kit model, decide per
model and record the decision in the provenance:

- **Keep the original** when it's already right and other placements use
  it. Add your variation beside it for variety.
- **Replace the original** when ours is better or the kit's doesn't fit.
  Change the placements to ours, then remove the kit model from the admitted
  set and its manifest, so the pack stops carrying it. Removing unused
  sources is how the pack stays small.
- **Reference only** when the kit was only inspiration: nothing from it
  ships, and the provenance says which kit was studied.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| Blender exits with code 137 at launch | macOS quarantine on a fresh install | Open Blender once from Finder and approve it |
| A converted model renders invisible | FBX materials with alpha 0 and hashed blending | `convert_fbx.py` forces them opaque |
| A converted creature shrinks to a few centimeters | Its clips animate the rig object's own scale | The converter drops object-level curves |
| Preview is empty | The glTF importer's helper icosphere at the origin threw off framing | `preview.py` frames from evaluated mesh vertices and hides helpers |
| Pack test fails "rebuild and repin" after another agent's push | Two agents repinned, or another machine compiled a different digest | Rebase, recompile, repin once, keep every reviewed digest in history |
| Builds fail with no space | The disk filled | Remove merged worktrees and idle `~/work/openagents-target-*` directories; `kache gc` |
