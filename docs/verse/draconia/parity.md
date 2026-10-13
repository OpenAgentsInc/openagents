# Draconia: character parity with a photoreal reference

Status: in progress, 2026-10-13. One entry per round below; newest last.

The goal is Verse rendering Draconia, our own original face of the same
type, at the realism of a photoreal reference clip: a young woman with a
messy platinum updo, freckles and grey-green eyes in a dim dungeon
classroom. Her face is the target look and quality, not a copy. The
reference clip and stills from it stay out of git (`~/Desktop/hICimQqsWFwwDPcE.mp4`,
stills in `~/work/film-out/draconia/parity/ref/`). Review artifacts live in
`~/work/film-out/draconia/parity/round-NN/` and `~/Downloads/draconia-parity-*`.

## Current asset for scenes

- **Round 1 (2026-10-13): `~/work/film-out/draconia/build/draconia.neutral.glb`**
  (built locally, not committed: 1.6 M triangles, 4K maps; rebuild with the
  commands under [Build and render](#build-and-render)). Load it as
  `crates/verse/examples/draconia_portrait.rs` does:
  `characters::import_with(&mut pack, dir, path, ImportOptions { max_texture_edge: 4096 })`,
  then `verse_engine::loading::Prepared::load` with a raised `Budget` (the
  manifest is ~400 MB of JSON) and `Renderer::from_prepared`; place it with
  `chamber::basis()`. `draconia.landmarks.json` beside it gives her eye,
  mouth and chest points for aiming cameras. Her materials carry the
  character shading models, so any scene on the imported renderer gets her
  skin, eyes, hair and cloth shading.
- **Limits for scene work today:** she is a static posed mesh (arms down),
  not yet skinned to the Universal rig, and her hair has no joint chains
  yet, so it can't blow in wind. Joint-chain hair (spring chains driven by
  a wind field) is the next round's first item.

## Current state vs reference

| Area | State | Gap |
| --- | --- | --- |
| Face shape | Realistic proportions (CC0 base, our face weights) | Reads young and soft, a long lower face; the reference's face is fuller in the cheeks with a stronger brow and heavier lids |
| Skin | Wrapped red-shifted diffuse, two lobes, pores and fine lines in a 4K normal map | Still smooth and waxy at close range; no cavity or micro-normal detail, too even in tone |
| Freckles | A light dusting over nose and cheeks | Present; reads as dots, needs softer, varied, clustered freckles |
| Eyes | Separate eyeballs, a refracting iris under an additive cornea, limbal ring | The lids sit too open and the sclera shows a hard white wedge at the corners; no caruncle, tear line or wet rim |
| Brows | Painted strands | Too thin and drawn-on; the reference's brows are fuller, natural |
| Lips | Analytic vermilion, rose, glossier | Shape fine; colour too orange-pink and edge too crisp |
| Hair (updo, strands) | 10,700 strand ribbons over a cap, a knot, locks at the temples, flyaways | Reads as a tight helmet with a visible cap; no volume, no messy loops at the sides, no root-to-tip shading |
| Uniform | Shirt collar, striped tie, V-neck sweater with trim, open robe with green facing | Blocky shells with hard cuts; no folds, no wool or knit texture, no hood drape |
| Lighting response | Teal rims and jar glow, a soft key, true depth of field | Background bokeh is blocky; jars are plain glowing cylinders; the face lacks the reference's soft wrap and deep shadow side |
| Expressions | Five FACS-unit presets (neutral, speak, surprise, closed, smile) at build time | Built per expression; no runtime blend shapes or rig yet |

## How it's built

- **Rights.** The face is original (MakeHuman's CC0 base mesh and targets
  with our own face weights), never a copy of the reference actress or
  anyone else; the professor is not modelled; the uniform's colours are
  generic green and silver with no crest of any real school.
- **Assets** (`scripts/blender/`): `draconia.py` builds her (head and bust,
  eyes, lashes, teeth), `draconia_paint.py` paints skin and eye maps,
  `draconia_hair.py` grows the updo, `draconia_cloth.py` cuts the uniform,
  `draconia_set.py` builds the dungeon backdrop, `draconia_mh.py` reads the
  CC0 MakeHuman source (fetched outside the repo, see below).
- **Renderer** (`crates/verse-pbr/src/imported/scene.wgsl`): character
  shading models any imported glTF can use through a material extra
  (`"verse_shading": "skin" | "eye" | "cornea" | "hair" | "cloth"`), see
  [Renderer features](#renderer-features).
- **Comparison renders** (`crates/verse/examples/draconia_portrait.rs`).

## Renderer features

All are general: any glTF imported through `verse_content::compiler::characters`
gets them by naming the model in its material extras, and
`verse_engine::material::Shading` documents the parameters.

| Model | Extras | What it does |
| --- | --- | --- |
| `skin` | `"verse_shading": "skin"`, optional `"verse_shading_params": [sss, lobe_mix, spec_weight, 0]` | Per-channel wrapped diffuse (red wraps furthest), a blurrier normal for red, a red-tinted shadow penumbra, two GGX lobes at 0.75x and 1.3x roughness |
| `eye` | `"verse_shading": "eye"`, params `[iris_depth_m, iris_uv_radius, 0, 0]` | Offsets the iris texture by the refracted view ray's travel to the iris plane (parallax under the cornea) |
| `cornea` | `"verse_shading": "cornea"` | Specular only, drawn additively over the eye (the importer sets blend 3) |
| `hair` | `"verse_shading": "hair"`, params `[shift_r, shift_trt, transmission, 0]` | Strand lighting along the surface's +v (root to tip): Kajiya-Kay diffuse, two shifted lobes (R white, TRT tinted), backlit transmission; cutout coverage is stochastic per sub-frame, so averaged frames give soft strands |
| `cloth` | `"verse_shading": "cloth"`, params `[sheen, sheen_roughness, 0, 0]` | Charlie sheen over a softly wrapped diffuse |

Other shared pieces:
- `characters::ImportOptions { max_texture_edge }` keeps textures up to
  4096 (game characters stay at 1024).
- The material uniform grew from four to six vec4s (the shading model and
  its parameters); standard materials serialize exactly as before, so
  existing packs and their digests don't change.
- Depth of field and anti-aliasing in `draconia_portrait` are the film
  path's: the mean of sub-frames over the lens
  (`camera_rig::lens_view_projection`) with a sub-pixel jitter. Bloom is
  the imported renderer's own.

## Build and render

```sh
scripts/blender/draconia_fetch.sh          # CC0 MakeHuman data, outside the repo
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/draconia.py -- [--expr neutral,speak,surprise,closed,smile]
$B -b --factory-startup --python scripts/blender/draconia_set.py
cargo build --release -p verse --example draconia_portrait
draconia_portrait OUT.png --shot closeup|eyes|medium|three|side|back [--subframes 48]
python3 scripts/verse/draconia_parity.py sbs REF.jpg OUT.png "Verse round N" SBS.png
```

## Rounds

### Round 1, 2026-10-13: from nothing to a photoreal pipeline

- **Gap.** There was no Draconia: Alice's stylized, base-colour-baked head
  can't reach photoreal, and the imported renderer had one shading model,
  1024-pixel character textures and no film portrait path.
- **What changed.**
  - A new head and bust on the CC0 MakeHuman base mesh (19k vertices,
    realistic topology) with our own face weights, posed arms-down with the
    CC0 skeleton's weights, subdivided twice; 34 CC0 expression units as
    FACS-like controls with five presets.
  - Separate 11.9 mm eyeballs seated behind the lid margins, cornea domes,
    eyelash ribbons from the lid margins, CC0 teeth and tongue.
  - Skin painted from baked position maps: mottling, redness zones,
    freckles, lips, painted brow strands; a 4K normal map of pores, fine
    lines and lip lines; a roughness map with an oilier T-zone.
  - A grey-green iris with fibres, crypts, collarette and limbal ring.
  - A messy updo as ~10,700 strand ribbons with painted strand strips,
    a knot of loose loops, temple locks and flyaways.
  - A uniform of offset shells with plane-cut openings: collar, striped
    tie, V-neck sweater with trim, open robe with green facing.
  - The dungeon set: stone blocks, shelves of glowing jars, a cabinet, a
    candle on a table.
  - Renderer: the five character shading models above, 4K character
    textures, and `draconia_portrait` with teal-and-candle lighting and
    lens-sampled depth of field.
- **Files.** `scripts/blender/draconia*.py`, `scripts/blender/draconia_fetch.sh`,
  `scripts/verse/draconia_parity.py`, `crates/verse-engine/src/material.rs`,
  `crates/verse-content/src/compiler/characters.rs`,
  `crates/verse-pbr/src/imported/{scene.wgsl,material_gpu.rs}`,
  `crates/verse/examples/draconia_portrait.rs`.
- **Artifacts.** `~/work/film-out/draconia/parity/round-01/sbs-{closeup,eyes,medium}.png`,
  `~/Downloads/draconia-parity-round-01.png`, `~/Downloads/draconia-parity-progress.png`.
- **Honest state.** Far from the reference: she reads as a CG mannequin.
  Skin is waxy, the hair is a helmet, the eyes stare, the uniform is
  blocky, the background bokeh is blocky.
- **Next gap.** Hair: joint chains for scenes (the peacock scene needs
  them), then real volume and a messy updo silhouette; then the eyes
  (lids, caruncle, tear line, sclera shading).

## Peacock scene

The peacock guardian (reference 113.0–118.0 s, [shots.md](shots.md)),
rendered by `crates/verse/examples/film_shot.rs` scene kind `peacock` from
`docs/verse/film/verse-shots/draconia-peacock-feathers.json` (P1+P2) and
`draconia-peacock-tail.json` (P3). Internal study only. Review files:
`~/Downloads/draconia-peacock-vN.mp4`, `-vN-side-by-side.mp4` (reference on
top), and `~/Downloads/draconia-peacock-progress.png` (one row per version).

### v1, 2026-10-13

- **Built:** the `feather` sprite sheet (a feather of light and a soft dot,
  `scripts/blender/fx/feather.py`), `verse::fx::feathers` (one feather, a dot,
  and a fanned tail), the effects `guardian_flash`, `ground_mist`,
  `night_embers` and `ruin_fire`, and the `peacock` scene: a starry void, a
  neon stage with cobbles, ruined walls, fires, and Alice recolored as
  Draconia (platinum hair band, the green coat to black).
- **Matches:** the cut timing (P1 1.3 s, the flash, P3 from 1.5 s), 21:9 at
  1920×822, the push in to one eyespot and the white-out, the wide 28 mm hold
  with a slight ease back, the fan's half-ellipse filling the frame, her
  centred backlit figure, fires and embers at the edges.
- **Gaps:** feathers are straight sprites, so P1 lacks the reference's curved
  shafts, fine dense barbs and rainbow fringe; P3's fan washes toward one
  white mass instead of crisp rows of eyespots on dark gaps; she is Alice's
  explorer outfit recolored (a staff on her back, no skirt or tights), standing
  from the start rather than rising from a crouch, with no hair lift; the face
  close-up at 1.5 s is our overexposed Alice, not the reference's eyes.

### v2, 2026-10-13

- **Gap fixed:** the tail washed into one white sheet. `feathers::Fan` now
  caps outer feathers at half the fan's radius, so eyespots stay apart along
  each arc, and takes a `vane` share: vanes and shafts burn at 0.42 of the
  eyespots' light, with a bright point on every eyespot. The fan has 175
  feathers in 9 rows over a faint haze.
- **Now matches:** crisp rows of white eyespots on darker gaps, with shafts
  converging on a white-hot core behind her silhouette, as in the reference.
- **Still open:** curved P1 feathers and the rainbow fringe; the haze between
  the reference's eyespots is brighter and bluer; her outfit, the rise from a
  crouch, and the lifting hair.

### v3, 2026-10-13

- **Gaps fixed:**
  - P1's feathers were straight sticks. `feathers::curved` now draws the head
    as a sprite, with the shaft and lower vane as ribbons along a curve, so
    the feathers sweep in arcs. `feathers::rainbow` adds the soft rainbow
    fringe low across the frame.
  - P3: she was front-lit. The tail now backlights her into a near-black
    silhouette: no key light, no lamps near her, the cobbles lit by low lamps
    at the fan's sides, and a pool of light under the core.
  - Alice's staff no longer shows (its vertices collapse every frame).
  - The P3 framing is closer: she's larger, and the fan (radius 7.6 m) reaches
    near the top of the frame.
- **Still open:**
  - the P3 open: the reference rises from a crouch with her hair flying up;
  - her outline is Alice's coat, not the skirt, tights and boots (waiting for
    the face agent's Draconia asset);
  - P1's barbs are sparser and less fine than the reference's;
  - the reference's walls and fires read more clearly at the frame edges.

### v4, 2026-10-13

- **Gap fixed:** the P3 open. She now lands in a crouch (Alice's swing
  stance, sunk 10 cm) and rises to her idle between 0.15 and 0.4 s, while the
  camera pulls back from her lowered face to the wide. The pack has no
  crouch-to-stand clip, so two rigs hold the two poses and their vertices
  blend.
- **Still open:**
  - her hair doesn't fly up; the pack's skinned hair has no joints to lift;
  - her outline is still Alice's coat;
  - P1's barbs are sparser than the reference's;
  - the walls and fires at the frame edges read weaker than the reference's.

### v5, 2026-10-13

- **Gaps fixed:**
  - v3's hazy tail eyespots (a regression). `feathers::Fan` gains `slim`:
    narrower feathers with egg-shaped eyespots and darker gaps (0.7 here).
    Vanes burn at 0.3 of the light, and each eyespot gets a larger white core,
    so the rows read as crisp white eyes again, with v3's silhouette and
    framing.
  - P1 was blobby and overexposed. The feather sheet has 520 finer barbs a
    side, feathers burn at about two-thirds the light, and the lower vane's
    ribbon is dimmer, so fine barbs show with dark between.
  - The edges: the walls stand back at the fan's sides and rise into the
    top corners against a teal night, occluding the fan's outer feathers as
    the reference's ruins do.
- **Still open:**
  - her hair doesn't fly (the face agent is adding hair joint chains);
  - her outline is still Alice's coat;
  - the fires at the edges are soft orange blobs, not the reference's crisp
    flames;
  - the P1 sheet is 512 px, so close-ups soften.

### v6, 2026-10-13

- **Gaps fixed:**
  - P1 now has five huge feathers, not thirteen. The sheet's eyespot is an
    egg: longer along the shaft, wider toward the tip, with a thick bright rim
    round a softer, slightly darker fill. The rainbow is fainter.
  - The guardian has a body: a glowing breast, a neck and a head with a
    three-point crest rise from the tail's center above her head.
    `feathers::glow_line` draws the strokes and is reusable for wand beams.
  - The fan is a sparser six rings of 130 slim feathers with dim vanes, so it
    reads as thin radial shafts with eyes at their tips.
  - She stands with her feet apart: below the hips her idle leans halfway to
    the crouch's wide stance.
  - The fires are licking tongues (velocity-stretched flames), not orange
    puffs.
- **Still open:**
  - her hair and outfit wait on the face agent's skinned Draconia and its rise
    clip with an upward gust;
  - the reference tail is brighter and denser near the core;
  - the peacock's body reads small next to the reference's.

### v7, 2026-10-13

- **Gaps fixed:**
  - P1 was soft and smeared. `feathers::fine` draws a feather as strokes of
    light: an egg rim loop, a curved shaft, about 230 single-stroke barbs,
    and a crown round the tip, over a flat disc fill. The close-up stays
    crisp however near the push comes, at any resolution. The five feathers
    lie flat to the camera, so their eggs draw true.
  - The tail's eyespots were teardrops. The sheet is now four 512×128 cells
    (feather, dot, a round eyespot with a dark pupil, a flat disc).
    `feathers::eyespot` puts a round eye with a darker center on every
    feather tip.
  - P3 was grey and dim. The tail burns brighter (eyespots 3.0–4.1 cd/m²),
    with bloom at 0.3, a brighter core, a blue haze filling the fan, and a
    mid-blue night sky.
  - The guardian's body and neck are larger. The fires' flames are shorter
    tongues, no longer blocky.
- **Still open:**
  - her hair and outfit (the face agent's Draconia);
  - P1's rim and fill leave a thin dark band between them;
  - the reference haze between feathers is denser.

### v8, 2026-10-13

- **Gaps fixed (v7 overshot toward flat vector graphics):**
  - P1's eyes are elongated ovals tilted along the shaft. A bright rim has a
    wider faint rim blooming round it, the fill is soft overlapping discs
    with no dark outline, and the barbs are softer, glowier strokes. The
    rainbow is a faint fringe, and a blue nebula of soft clouds sits behind
    the stars.
  - P3's eyes are smaller ovals pointing out along each shaft, from the
    feather sheet's own egg, with a small glow cap instead of a coin. The
    vanes show (0.4 of the light), and the fan has a little irregularity in
    angle and radius. Cloudy light (smoke-sheet sprites, additive) fills the
    gaps. Five rings of 100 feathers droop to the ground at the sides.
  - The guardian's neck is a thick S-curved stroke of light rising from a
    breast just above her head, with a head, beak and five-point crest. (A
    `glow_line`'s bright core is about a fifth of its half width.)
  - Bloom is wider: 0.3 on P1, 0.42 on P3.
- **Still open, from the v8 side-by-side at 0.6 s:**
  - the reference's P1 barbs are long curved strokes flowing along the
    feather toward the tip, not straight ones crossing it;
  - its shaft and rim are much thicker and blown out;
  - its sky is a lighter blue nebula; ours is still dark.

### v9, 2026-10-13

- **Gaps fixed:**
  - P1's barbs are long strokes that leave the shaft at an angle and bend to
    flow toward the tip, 140 a side. The shaft and the egg's rim are thick
    and blown out, and a mid-blue nebula of soft clouds fills the sky.
  - The engine change behind it: `verse::fx::set_budget` lets an offline
    render raise the per-frame sprite and ribbon budget (High is 2048).
    `film_shot` raises it to 16,384, since a film frame's cost doesn't matter
    and a stroked close-up draws thousands of ribbon segments.
  - P3:
    - the white-hot core is a big wash from her hip to her shoulders;
    - a bright pool of light lies on the cobbles under her feet, and the
      ground mist burns brighter;
    - the fires are two small, ragged clusters with smoke at the far left.
- **Still open:**
  - the guardian's neck still blends into the core rather than reading as a
    white peacock;
  - the fan's eyes are greyer and less contrasty than the reference's white
    rings;
  - the core light shows the cobbles as a hard checker pattern.

### v10, 2026-10-13

- **Gaps fixed:**
  - The guardian's neck now reads as a shape: a soft fill between two bright
    edge strokes, curving up above her head to the head and crest. The core
    is a horizontal band of light from her hips to her shoulders, so the neck
    stands against the fan, not the glare.
  - The fan's eyes are brighter (4.4–5.8 cd/m²), with vanes at 0.32 of that
    and grade contrast at 1.35.
  - The cobbles' checker is gone: the pool of light lies just above the
    stones, not through them.
  - The fire: one ragged cluster at the far left, as in the reference, and
    fewer, dimmer embers. The ground mist is thinner, so it no longer fogs
    the whole frame by the last second.
- **Still open:**
  - the reference's eyes are still crisper white rings;
  - the core's band reads as four lobes;
  - her hair and outfit.

### v11, 2026-10-13

- **Gaps fixed:**
  - The walls were Lego stairs. They're now ragged near-black silhouettes:
    vertical strips with irregular broken tops, deep notches and pointed
    Gothic window holes, plus three broken pillars with stepped, slanted
    breaks, kept to the frame's edges.
  - The core: a blinding, clipped-white center behind her torso and hips
    over the hip-to-shoulder band, so her silhouette pops.
  - Her scale and position: the camera is closer (5.5 m) and aimed lower, so
    she stands higher and larger in frame.
  - The neck is brighter (3.5× the eyespots' light) and stands in front of
    the fan.
  - The sheet's eyespots have a darker fill, so the white rims read as rings.
- **Still open:**
  - the neck still reads faintly against the bright fan;
  - the fan's eyes are paler than the reference's white;
  - her hair and outfit (waiting on the face agent's skinned Draconia).

### v12, 2026-10-13

- **Owner flag:** the P3 opening showed a dark, smeared, ghosted face (two
  rigs blended under a fast pull-back, buried in backlight and fog). It's
  cut. P3 now opens on a medium-wide backlit silhouette out of a white flash
  held 0.1 s longer, then pulls back to the wide. No face close-up shows
  until a proper one exists.
- **Next:** the face agent is rendering a front-lit extreme close-up of her
  eyes for this moment
  (`~/work/film-out/draconia/parity/peacock-open/peacock-open-vN.mp4`). When
  it lands, it gets cut in at the P3 opening at the reference's timing
  (114.5–114.7 s), before the pull-back.

## Scene: D1 (shots 1–6, reference 0–9.7 s)

Rendered by `crates/verse/examples/film_shot.rs` scene kind `courtyard`
(`crates/verse/examples/film/courtyard.rs`) from
`docs/verse/film/verse-shots/draconia-d1-{face,girl,boy,cloak,duel}.json`,
cut by `~/work/film-out/draconia/d1-cut.sh` into
`~/Downloads/draconia-d1-vN.mp4`, `-vN-side-by-side.mp4` (reference on top)
and `~/Downloads/draconia-d1-progress.png`. Internal study only.

**Rule:** no stand-in face appears in a close-up or medium shot. Shot 1
carries a slate ("face: waiting on Draconia") over a fire-and-embers plate
until the face work renders it. In the mediums, the stand-ins turn away or
stand in silhouette.

### v1, 2026-10-13

- **Built:**
  - The `courtyard` scene: cobbles and brick walls in staggered courses (one
    with a ragged broken top), plus a cast of pack rigs joined into one
    figure (Alice, and the male ranger for the boys).
  - The wall top as loose blocks that fly when the bolt hits.
  - `feathers::beam`, a spell bolt as a hot white core in a green glow, and
    the `wall_blast` effect (a golden flash, rolling warm dust, thrown grit,
    and haze).
  - The shots, at the reference's cut times:
    1. a fire plate (slated);
    2. the girl against the wall as the bolt flashes past and dust falls;
    3–4. the boy silhouetted against the wall as the bolt passes over it,
       then the top blows out in golden dust and he ducks and runs;
    5. a dark cloak crossing the lens;
    6. a high wide shot of five duelers with a bolt and a shield shimmer.
- **Gaps:**
  - Shot 1 needs the face work's Draconia close-up.
  - The stand-ins aren't the reference's students: no uniforms, the girl
    faces the wall instead of the camera, and the boy is a silhouette.
  - The walls are clean brick boxes, not weathered stone.
  - The blast dust is grey-gold, not the reference's warm amber, and the wall
    top's blocks don't read as a burst.
  - The duel lacks the reference's crowding and motion.
