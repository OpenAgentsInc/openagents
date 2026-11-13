# vork against Animation Lab, Everglade and Character Create

Research note, 2026-11-07. For each of our three surfaces: what it has
today, what vork has in the same area, and what is worth taking. vork
references are to `voyager-so/vork` at `44188ba`: its help pages
(`docs/help/*.md`), its field reference
(`crates/vork-scene/src/v2/reference.txt`) and its examples. The desktop
render path these items depend on is in the
[film renderer note](2026-11-07-desktop-film-renderer.md).

Priority means what each item is worth to us: **high** (do it with the
film profile), **medium** (worth a task), **low** (note only).

## vork's relevant features

| Area | vork 0.1.0 |
| --- | --- |
| Scene | JSON v2 nodes: mesh, light, camera, asset (glTF). `std/` geometry builders. Instances with TRANSLATION, ROTATION, SCALE, `time_offset`, `pass_index` and custom columns bound to material inputs |
| Animation | Ordered layers, each a glTF clip or tracks; `replace` or `add`; animatable `weight`; `offset` and `speed`; `loop`. A clip from another glTF file is retargeted by node name. One track path syntax for every numeric property: node transforms, morph weights, material and program inputs, camera optics, light fields, layer weights and render fields |
| Skinning and morphs | glTF skins (joints and inverse binds), named morph targets with POSITION and NORMAL, `node.morph_weights` |
| Geometry | Triangles, points (rendered as spheres), curves (hair), implicit surfaces (a field's zero level). Catmull-Clark or simple subdivision with OpenSubdiv creases at render time |
| Materials | `std/openpbr` by default. `Closure c = {};` gives OpenPBR's defaults. Thin-walled subsurface for leaves and cloth; transmission. `std/hair` is Blender's Principled Hair (Chiang) on curves |
| Programs | Slang stages a scene may supply: surface, displace, aov, lit, environment, composite, camera, sensor, display, compute, propagation |
| Simulation | `scene.simulation`: `t0`, a fixed `dt`, `seed`, state buffers, ordered init and step programs. Seeking replays from initialization or a checkpoint |
| Camera | `std/camera` follows Blender's physical camera: lens, sensor, shift, clipping, depth of field, `focus_distance` or `focus_object`, aperture |
| Quality | Profiles `std/previs`, `std/draft` (4 samples), `std/final` (8 samples), `std/path`. OIDN denoising. `--set samples=N`, `--set seed=` |
| Time | `render.motion_blur`: `shutter` in frames; `position` start, center or end |
| Output | PNG, frame directories, multilayer OpenEXR, H.264, ProRes 4444 with alpha. Channels: depth, view normal, object index, motion, shadow catcher, AOVs. `meta.json` and OpenCV `camera.json` sidecars |
| Compositor | An ordered image graph of Slang composite programs; Blender-matched `std/bloom` pyramid; glare, lens distortion, vignette and white balance libraries |
| Footage | Video and EXR sequences as textures; shadow catchers and holdouts for inserting CG into real plates (`examples/vfx-artifact`) |
| Agent interface | `vork help` (about 2,500 tokens, with a complete first scene), topic help (about 19,500 tokens), `vork help field PATH` for a single field |

What it lacks: fog and volumes, any real-time or interactive mode, a
browser target, non-NVIDIA and non-Apple GPUs, and gameplay. Hair
displacement and lit stages are ignored. Computed geometry with skinning
uses the authored snapshot.

---

## Animation Lab

### Today

The [Animation Lab plan](../../verse/2026-11-06-animation-lab.md) runs A0
to A8. A6 (finished styles) and A8 (the publishable scene) are pending.

- **Documents.** `anim.rig/clip/performance/pose/scene/cut.v1`, with packs
  named by hash (`docs/verse/anim-documents-v1.md`).
- **Rigs.** Up to 512 joints, 8 influences per vertex, 256 sparse morphs,
  the ARKit face map, and the FRG1 DNA face block (`aethyr/anim/facerig.coax`).
- **Layers.** base, override, additive, face and fix-up, with masks
  (`aethyr/anim/layers.coax`).
- **IK.** Foot-lock, look-at, hand-to-prop and reach
  (`aethyr/anim/fixup.coax`).
- **Retargeting.** Anchor-based (`canonical-anchor.v1`).
- **Imports.** glTF, BVH, ARKit CSV, FBX through Blender, Unreal through a
  commandlet.
- **Directing.** A typed single-operation directing API with receipts and
  undo (A7).
- **Films.** `labs anim play` renders 960×540 films through headless Chrome
  at 1.62 s per film second (A5).
- **Rendering** (`apps/openagents-web/gpu/animationlab.coax`, 28 lines) is
  inspection shading only. Lambert from one fixed direction plus constant
  ambient, keyed lights summed into one key, no shadows, textures, PBR,
  hair or post.
- **Stored but unused.** The scene camera's `focus` does nothing. Weather,
  time of day and focus "remain metadata". Effects other than the guardian
  bird draw as markers.

### What vork adds

Animation Lab's animation model is already richer than vork's. vork has no
IK, no masks, no face rig and a weaker retarget (by node name only). The
value is in how vork renders and delivers.

1. **A finished render profile for A6: high.** A6 says "the same scene file
   renders realistic and cel". That is vork's content-and-quality split,
   with the style as a third axis: the scene document, then a style
   (realistic or cel), then a quality profile (live or film). Build A6 on
   the film profile's F0 and F1 and reuse the existing renderers:
   - **Realistic:** Valley's character pass (`aethyr/render/character.coax`:
     dual-lobe skin, strand hair with dual scattering, depth of field in
     `vc_post_fs`).
   - **Cel:** Anime Lab's (`gpu/animelab.coax`: two-tone cel, rim, shadow
     map, ink from depth and normal edges, bloom).
   
   Don't grow the 28-line inspection shader into a third look.
2. **Physical camera fields in `anim.scene.v1`: high.** Add vork's
   Blender-compatible set: `lens`, `sensor_width/height`, `shift`,
   `clip_start/end`, `use_dof`, `focus_distance`, `focus_object` and
   `aperture_fstop`. The A4 Blender and Unreal imports can then carry
   cameras across unchanged, and `focus` stops being decoration. vork's
   axis and unit conversion notes (`docs/help/nodes.md`, "FROM BLENDER")
   are a useful checklist for our importer: Y-up, quaternions xyzw, frames
   0-based, and motion vectors current minus previous.
3. **Every numeric property as a track: medium.** vork animates material
   inputs, camera optics, light fields, layer weights and render settings
   with one path syntax (`["materials", m, "inputs", i]`, `["animation",
   layer, "weight"]`, `["render", "motion_blur", "shutter"]`). A5 refuses
   weather and time keys and keeps focus as metadata. One typed track
   address over the scene's numeric fields, admitted by the same document
   reader, would let a scene key the time of day, a lamp, an f-stop or a
   fade. A7's directing catalogue could then add operations such as "rack
   focus to her at 3 s" without a new document kind.
4. **Shutter motion blur: high, for film.** `render.motion_blur.shutter`
   in frames and `position`. Animation Lab clips go up to 240 fps and the
   clock is fixed, so sub-frame samples of the evaluated pose are exact.
   Fast moves (casts, swings, the guardian's wings) will look much better
   with it.
5. **Outputs a cut can use: high.** Replace the Chrome-captured 8-bit PNGs
   with EXR beauty plus depth, normal, object id and motion channels, and
   `camera.json`. Then `labs cut`'s grading, overlays and crossfades (which
   A5 currently refuses) work on real layers, and a director can fix a
   grade without re-rendering. Keep the per-frame hash receipt.
6. **Small preview stills in the directing loop: medium.** vork's loop is
   `render --at T --set resolution=320x180`, then look, then revise. Add
   `openagents labs anim still FILE --at 3.0 --size 320x180 OUT.png`. After
   an A7 edit, the model or the owner sees the changed moment in one image,
   which is cheaper than a film and more useful than a JSON check.
7. **Curves for hair at film quality: low, later.** vork renders hair as
   curves with Blender's Chiang hair BSDF. Valley's groom is 118,339 strands
   drawn as ribbons. A traced film profile (F3) could intersect real curves.
   That only matters once A8's characters exist.
8. **Subdivision at render time: low.** Catmull-Clark with creases is
   useful for close-ups of low-poly stand-ins, but it changes silhouettes.
   Make it an explicit per-asset choice, never a default.

Not worth taking: name-based retargeting (ours is better), vork's layer
model (ours is a superset), and glTF-clip-only animation.

---

## Everglade

### Today

From `docs/verse/everglade.md`, `apps/openagents-web/gpu/everglade-v1/gpu.coax` and
`gpu-fx.coax`, and the art audit in `docs/verse/art-styles.md`.

- **Look.** A painterly style: three toon bands, a cool shadow band,
  hemisphere ambient, a warm rim and a baked sky-visibility grid.
- **Shadows.** One 2048² sun map following the eye, 60 m reach, 9 PCF taps.
  Leaf cards cut the map, giving dappled shade.
- **Sky and weather.** A painted sky with three ridge rings, a 48-minute
  day, a seeded weather schedule (rain, storm, fog), wetness and puddles.
- **Water.** Gerstner waves, Fresnel, Snell's window, 20-step screen-space
  reflections, caustics and underwater shafts.
- **Vegetation.** About 1,000 trees, with kit meshes within 55 m and cones
  beyond. Instanced grass out to 32 m.
- **Effects.** Two passes (premultiplied smoke, then additive fire),
  written by `everglade-spells.coax` rather than `aethyr/fx`.
- **Post.** rgba16f, bloom at ¼ and ⅛ resolution, light shafts, then the
  tone curve.
- **Quality ladder.** Seven rungs in `aethyr/core/quality.coax` and
  `everglade-quality.coax`. The 60 FPS rule is in `aethyr/core/frame-report.coax`.
- **Transparency.** The scene pass has no blending, so the Water Orb and
  similar effects use screen-door dithering.
- **Cast.** At most 10 NPCs drawn (6 on a phone). 63 raid bodies on the
  people rig. A 7-bird flock companion (`aethyr/anim/flock.coax`).
- **Cinematics.** `?raid=preview&cine=1` is a keyed flight "for a film".
  Movies are clip-recorder captures.

### What vork adds

Everglade is a real-time stylized world, and vork is an offline physically
based renderer. The overlap is the film path and GPU simulation, not the
shading.

1. **A film profile that lifts every real-time limit: high.** This is the
   [film renderer](2026-11-07-desktop-film-renderer.md)'s main case. Under
   film settings, the same Everglade passes get:
   - kit trees out to the ridge, full grass and no LOD;
   - shadow cascades instead of one 60 m map;
   - real sorted or weighted transparency for orbs, smoke and glass;
   - supersampling, motion blur and depth of field;
   - full puff and debris budgets. The adaptive-quality doc measured
     570–730 puffs during a destroy and capped phones far below that.
   
   It stays painterly, because `eg_paint` is unchanged and only its
   visibility inputs improve.
2. **Ray-traced visibility, kept painterly: medium (F3).** The terms
   Everglade approximates are exactly the ones hardware ray queries make
   exact:
   - the sun shadow beyond 60 m and through canopy;
   - the sky-visibility/AO grid (2 m points, so no contact detail);
   - water reflection (20-step screen-space reflections, which miss
     anything off-screen);
   - window and lamp light in interiors (the 44 district halls are dark).
   
   Feed ray-cast visibility into the existing bands and the look holds.
3. **GPU crowds and flocks that can seek: medium.** vork's murmuration
   shows the pattern. A deterministic grid (32³, four representatives a
   cell), seven neighbours chosen from at most 108, then separate clear,
   grid, advance and place dispatches. It reads the old generation and
   writes scratch, steps at a fixed 60 Hz independent of render fps, seeks
   by replay, and uses mesh birds near the camera and silhouettes far
   away. Our pieces already fit:
   - `aethyr/anim/flock.coax` steps at a fixed 120 Hz;
   - `aethyr/core/rand.coax` streams are counter-based;
   - `coax cuda` and `coax spirv` kernels can do the compute;
   - chapters 23 (particles and neighbours) and 31 (many worlds in one
     step) plan the batch dimension.
   
   A starling murmuration over the Everglade river at dusk, or a
   thousand-strong cultist host for a raid trailer, is a film shot we could
   produce. vork's instance `time_offset` column, which desynchronizes one
   clip across thousands of instances, is a cheap trick worth copying for
   raid crowds now. Each raid body already has its own gait phase.
4. **Effects as a stateless function of time: already ours, low.** Our
   puffs are a pure function of seed, index and age (`aethyr/fx/puff.coax`),
   which is exactly what motion-blurred film effects need: sub-frame
   samples are free. Moving Everglade's own `sp_fx` onto `aethyr/fx`
   (chapter 72's remaining work) is a prerequisite for film effects, not
   something vork gives us.
5. **A bloom and glare pyramid with a stated reference: low.** vork's
   `std/bloom` follows Blender's arithmetic through an authored image
   pyramid (prepare, downsample with Karis weighting, upsample, combine).
   Everglade's two-level bloom is fine at 60 FPS. For film, a deeper
   pyramid with an explicit reference is a small, checkable upgrade in
   `aethyr/render/post.coax`.
6. **Compositing over real footage: low.** vork's VFX examples insert CG
   into real plates with shadow catchers, holdouts and camera solves. Not
   an Everglade need, though a shadow-catcher channel would let promo
   titles and UI sit over film frames with correct shadows.

Not worth taking: OpenPBR for Everglade's world (it would end the painterly
look), and path transport. Fog and volumes, light shafts and underwater
shafts are areas where Everglade is ahead of vork.

---

## Character Create

### Today

From `docs/verse/character-creator.md`,
`apps/openagents-web/gpu/charactercreator.coax`, `gpu/character-glade*.coax`
and `aethyr/character/character-appearance.coax`.

- **Controls.** Discrete cyclers: body, clothes preset, hood, colourway,
  dye, skin, hair style and colour, beard, eye colour, animation, and
  per-garment wear. The wardrobe is 76 Fantasy Source meshes in 40 slots.
- **Saves.** A 56-byte `CC2` record. Everglade draws the saved player.
- **Skeleton.** The 65-joint Quaternius/UE mannequin, with baked 30 fps
  clips. No morphs or face rig.
- **Shading** (`cc_player_fs`): one model for skin, hair and cloth. Lambert
  from a fixed sun, hemisphere fill, a camera-side bounce and one Blinn
  lobe (power 24, ×0.12). Normal and ORM textures exist but are never
  sampled. No SSS, hair or eye model.
- **Scene.** The glade: a shadow map with 12-tap Poisson PCF, 4-sample
  height fog, contact AO, foliage reconstruction, 2-level bloom, FXAA and a
  peak-channel shoulder tonemap. The sky is an ImageGen plate.
- **No portrait or turntable renderer.** The page's og:image,
  `/og/realms/character-creator.jpg`, does not exist.
- **WebGPU only**, and a legacy consumer in `aethyr/render/adoption.coax`.
- **The hero character pass** for Echo (`aethyr/render/character.coax`) is
  a separate UE-derived PBR path: skin, strand hair and eyes.

### What vork adds

1. **Portraits, turntables and cards from the film profile: high.** vork's
   `--at T` stills, transparent backgrounds (`render.background:
   "transparent"`, PNG alpha, ProRes 4444), and a camera orbit as an
   ordinary track give us:
   - the missing og image;
   - a portrait for every saved `CC2` (nameplates, party frames, the
     Everglade join screen, profile pages);
   - turntable clips for sharing a character.
   
   Determinism makes a portrait reproducible from the 56-byte record:
   cache by the record's hash and re-render only when the record or the
   renderer changes. That is a small Coax tool on F0: load the record, pose
   with the idle clip at time t, frame with a portrait camera, render at
   film quality, write PNG with alpha.
2. **A material record with OpenPBR-style fields, using the textures
   already shipped: high.** vork's lesson is that one material record with
   known defaults (`Closure c = {};` is OpenPBR's defaults) lets every
   asset carry the same fields:
   - base colour, metalness, roughness and normal;
   - thin-walled subsurface for cloth, leaves and ears;
   - a hair model.
   
   Each renderer then approximates what it can afford. Character Create
   already ships normal and ORM maps that the shader ignores. Chapter 12's
   next step ("a material table in each realm ... a material ID per
   instance") is the same idea. Do it for the character first:
   - the live shader samples normal and roughness;
   - the film profile adds real specular, sheen for cloth and wrap or
     subsurface for skin, borrowing the forms already written in
     `aethyr/render/character.coax`;
   - dyes stay a parameter of the record.
   
   This lifts Create, Everglade's player and NPCs (the same Quaternius
   parts) and the Fantasy wardrobe at once. Everglade's art audit already
   notes the Ranger "reading flatter than under the old engine's PBR".
3. **Colour discipline: medium.** vork is strict: authored colour is linear
   Rec.709, sRGB picker values are converted before storage, and data
   textures are `Non-Color`. Create applies dye masks in display space and
   then decodes. That may be intentional for the look, but it should be a
   stated choice, written down next to `CA_DYES`, with a check, so the film
   renderer does the same thing.
4. **Hair: low now, medium for film.** Create's hair is mesh cards with the
   shared Lambert. A film portrait needs at least an anisotropic highlight
   on cards (Kajiya-Kay or Marschner-style lobes as Coax functions). vork's
   Chiang hair on curves is the high end. The Echo pass's dual-scattering
   LUT is the in-house middle ground.
5. **Subdivision for close portraits: low.** Optional Catmull-Clark on head
   and hands only, at film quality, to remove facets on 31k-triangle
   Quaternius bodies.

Not worth taking: implicit-surface geometry, and vork's footage tools.

---

## Across all three: the agent interface

vork's help system is designed for agents, and our three surfaces each
invented their own way in:

- Animation Lab has the `labs anim` verbs and A7's typed catalogue.
- Everglade has a 34-item edit catalogue behind a JSON schema.
- Character Create has `CC2` and its cyclers.

vork's pattern is:

- a first page of about 2,500 tokens with a complete working example;
- topic pages with `--all` for detail;
- `help field PATH` for one field;
- errors that name the node, field and time.

The help is generated from the same sources the reader admits. For us, an
`openagents help anim`, `help realm-edit` or `help appearance` generated
from the Coax catalogues (`chapters.json`, the edit catalogue,
`charactercreator-parts.coax`) would give an agent one short entry point
per surface. It would also be checked against the code, which hand-written
prompts are not. Medium priority; it pairs naturally with chapter 79's
single catalogue.

## Where vork's claims need care

Before comparing anything against vork:

- **The tweet says "up to 24x faster rendering times".** In the README,
  numbers that large appear only in two places. One is the preview range
  from an earlier build (EEVEE/vork 14.63–36.40×, which the README says
  doesn't describe final rendering). The other is the bird projections.
  The default final renders measure 1.14–2.32× for movies and 1.38–1.66×
  for stills against EEVEE, at different quality settings. The 31-minute
  EEVEE bird figure is a projection at 16 samples against vork's 4, and
  excludes simulation and encoding. The only matched four-sample
  projection is about 8–11×.
- **Setup and first compile are excluded from headline times.** Setup took
  about 160–250 s, and a scene's first final still can take 82 s against a
  1.8 s warm median.
- **vork's path tracer is slower than Cycles** in both published cases.
- **The Blender comparison helper doesn't run on Blender 5.2.1**, so the
  comparisons can't currently be independently reproduced.

vork says all of this itself, which is to its credit. Chapter 37's rule
(compile and startup count, same scene and same hardware or no comparison)
is the standard any Metaverse Engine film benchmark should meet.
