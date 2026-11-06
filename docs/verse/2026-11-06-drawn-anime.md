# Drawn Anime: from toon-shaded 3D to a hand-drawn production

Written 2026-11-06. Status: plan. Nothing here is implemented yet.

Owner, 2026-11-06:

> Our anime cel drawing attempt here in this repo is disgustingly ugly. It's
> just horrible pastel shaders on a 3D scene. Help me plan a refactor to more
> of a Hayao Miyazaki inspired real anime creation thing, extending our
> animation lab.

This plan replaces the anime look of Anime Lab
([anime-lab.md](anime-lab.md)) and the cel half of Animation Lab's A6
([2026-11-06-animation-lab.md](2026-11-06-animation-lab.md), #12971) with a
real 2D animation pipeline: layouts, key drawings, in-betweens on an
exposure sheet, ink and paint, painted backgrounds and compositing. Every
frame is still drawn by Aethyr, from typed documents, deterministically.
Animation Lab's 3D work (rigs, clips, retargeting, layers, scenes, cuts)
stays and becomes the layout and reference stage, not the final pixels.

"Miyazaki-inspired" here names a set of craft principles we can state in
words (below). We do not copy any studio's characters, designs, frames,
backgrounds, music or films, and we do not train on them. Our cast, our
world (Skyreach Academy, Everglade) and our stories.

## Why the current look fails

What exists (`verse/style.coax` `verse_style_anime_*`, the presets in
`verse/style-preset.coax`, `apps/openagents-web/animelab*.coax`,
`apps/openagents-web/gpu/animelab.coax`):

- **The characters are 3D primitives.** `animelab-cast.coax` builds bodies
  from capsules, cones and boxes with face marks on a sphere. A two-tone
  shader over a primitive is still a primitive. No line in it was drawn, so
  nothing in it has a draughtsman's intent: no silhouette design, no
  simplification, no "good drawing".
- **The lines are measured, not drawn.** Ink comes from depth and normal
  differences in screen space (`verse_style_anime_ink_edge`). It outlines
  everything with the same mechanical width, finds creases a person would
  never ink, misses the ones they would, and swims as the camera moves.
- **Shading is computed light.** Half Lambert cut into two bands means the
  shadow shape is whatever the geometry produces. In drawn anime the shadow
  is a *designed shape* painted on the cel (the kage), chosen for the
  drawing, often not physically right.
- **The palette is a saturation dial.** Saturation 1.15 to 1.25, bloom on
  everything emissive, blue-violet shadow tint, vivid sky gradients and
  sparkles. That is the generic "anime filter" look. Miyazaki's films are
  the opposite: earthy, muted, naturalistic colour with a few saturated
  accents, and values that read in greyscale.
- **The world is a lit 3D set, not a painting.** Backgrounds in drawn
  anime are painted plates (gouache/poster colour, watercolour), designed
  per shot from a layout, with brush texture, edge variation and a focal
  composition. A procedural sky gradient over shaded boxes has none of
  that.
- **It moves like CG.** Every joint is interpolated every frame, the camera
  orbits freely in 3D, and nothing holds. Drawn animation is timed on an
  exposure sheet: on twos (12 drawings a second) by default, ones for fast
  action, holds, anticipation, overshoot, smears, and a camera that mostly
  pans and trucks across flat levels.

None of this is fixed by tuning numbers in the presets. The output has to
come from drawings.

## The principles we are aiming at

Stated as checkable craft, not as a studio to imitate:

1. **Drawings, not renders.** Every character frame is a vector drawing:
   pressure-varying strokes that taper, close into paintable regions, and
   are designed for the pose. 3D may guide a drawing; it is never the
   drawing.
2. **Designed shadows.** One shadow shape (and at most a highlight shape)
   per region, painted, from the colour model. No light-computed bands on
   characters.
3. **Painted worlds.** Backgrounds are painted plates with visible brush
   texture, soft-and-hard edge variety, atmospheric value steps and a
   composition made for the shot's layout. Detail where the eye goes,
   suggestion elsewhere.
4. **Naturalistic palette.** Earth greens, warm stone, weathered wood,
   clear but not electric skies; saturation well under the current
   presets; values that hold up in greyscale. Saturated colour is an accent
   (a red scarf, a fire), not the base.
5. **Weight and air.** Things have mass: anticipation before effort,
   follow-through after it, cloth and hair that lag and settle. Wind moves
   grass, hair, clothes and clouds on drawn loops.
6. **Timing on the sheet.** Twos by default, ones for speed, holds that
   breathe. Quiet moments are allowed to be quiet (a character just looks;
   the wind moves; nothing else happens). Fast action is drawn fast, with
   smears and impact frames, then lands.
7. **A flat camera.** Shots are a field on flat levels: pans across long
   backgrounds, trucks in and out, multiplane parallax, the occasional
   camera shake. Free 3D orbits are the exception and are drawn (or laid
   out) for that shot.
8. **Clean compositing.** Light, diffusion and shadow cels added in a
   compositing pass the way photography (satsuei) does: a soft glow on
   lit areas, a few drawn light rays, a darkening gradient. No bloom on
   everything, no lens sparkles by default.

These turn into checks (see "Checks"): line width variation, region
closure, palette saturation and value ceilings, timing on the sheet, and
owner review of a fixed reference board.

## The pipeline

The stages of a 2D production, each a typed document with `#authority`
admission, one checked writer, versions you can diff, the same frame hashes
from the same inputs (Animation Lab's principles 2 and 3):

```
script / director words
   │
   ▼
storyboard (anim.board.v1) ── shots, timing, framing, dialogue
   │
   ▼
layout (anim.layout.v1) ─────── field, levels, camera moves, BG drawing
   │            ▲
   │            └── 3D previz: Animation Lab scene + cut, rendered as guides
   ▼
key drawings (anim.drawing.v1)  ── the poses that tell the story
   │
   ▼
timing + in-betweens (anim.xsheet.v1, anim.drawing.v1)
   │
   ▼
ink & paint (anim.colormodel.v1) ── regions filled from the colour model
   │
   ▼
backgrounds (anim.background.v1) ── painted plates, per layout
   │
   ▼
compositing (anim.composite.v1) ── levels, camera, light, diffusion
   │
   ▼
cut (anim.cut.v1, existing) ── the edit, sound, render
```

### Documents

New document types beside Animation Lab's six (`anim.rig.v1` to
`anim.pose.v1`), under `aethyr/anim/` for readers and admission, with a
format page `docs/verse/anim-drawn-v1.md` written first, in the style of
[anim-documents-v1.md](anim-documents-v1.md). They are new versions, not
changes to the published ones.

| Document | What it holds |
| --- | --- |
| `anim.board.v1` | Storyboard: shots in order, each with panels (a rough drawing or a reference frame), duration, framing, action notes, dialogue and sound cues |
| `anim.layout.v1` | One shot's field (aspect, field size), its levels (BG, BOOK overlays in front of the cels, cel levels A, B, C...), the camera over the field by frame (pan, truck, shake, multiplane depth per level), and the layout drawing the background is painted from |
| `anim.drawing.v1` | One drawing on one level: strokes (centreline points with pressure, a pen, a colour role), the closed regions the paint fills (by stroke references, with gap closing up to a stated tolerance), and region labels (skin, hair, coat, coat-shadow, hair-highlight...). Roughs, keys, breakdowns and in-betweens are all this type, with a `role` field |
| `anim.xsheet.v1` | The exposure sheet: for each level, which drawing shows on each frame (on ones, twos, threes, holds), plus the in-between charts (spacing between two keys: even, slow-in, slow-out, favouring) that generated in-betweens follow |
| `anim.colormodel.v1` | A character's or prop's colour model: each region label's normal, shadow and highlight colours, per lighting condition (day, dusk, night, firelight, rain) |
| `anim.background.v1` | A painted plate: the size in field units, a stack of paint layers (each a set of brush strokes, washes and masks), the paper, the palette, and its parallax split into planes for a multiplane camera |
| `anim.composite.v1` | One shot's photography: level order, per-level light (a drawn highlight cel, a shadow cel), diffusion (glow) amount and threshold, light rays, gradients, film grain, transitions |

The existing `anim.cut.v1` edits composited shots as it edits 3D shots now.
`anim.scene.v1`'s `style` field gains `drawn`: a scene with a drawn style
renders through a layout and an exposure sheet instead of through the 3D
renderer.

### 1. Storyboard and layout from the 3D lab

Animation Lab already stages a scene in 3D with authored cameras and a cut
(A5). That becomes **previz**: blocking, timing and camera, not the look.

- `labs anim layout SCENE --shot S3` renders the shot's 3D previz as
  **guide layers**: a silhouette and feature-line render of each posed
  character (outer contour, the face's centre lines, hands), the floor plane
  and horizon, and the camera's field. Guides are drawn under the drawing
  level, the way a layout artist works over a 3D blocking pass.
- The layout document records the field and the camera as a 2D move
  (pan, truck) derived from the 3D camera where the shot allows it. Shots
  that truly need a 3D camera keep it and are marked so.
- The board can be made by hand or by the director (below) from words.

### 2. Key drawings

The heart of the look, and the hardest part. There are three sources, in
the order we'll build them:

1. **A person draws them.** A drawing station in the realm: a pen (mouse,
   trackpad, Apple Pencil on the iOS app) on a level, with the guide layers,
   onion skin, a light table, a stroke smoother and pressure-to-width. This
   is the ground truth for quality and the data the later sources learn
   from, with consent.
2. **The engine traces a key from the guide, then cleans it.** A
   procedural clean-up in Coax: take the guide's contour and feature curves,
   simplify them (fewer, longer curves), vary width by curvature and
   direction (thick on the shadow side and at overlaps, thin at the lit
   side, tapered ends), break lines where a person lifts the pen, and add
   designed details from the character's model sheet (hair clumps, cloth
   folds keyed to the pose). This is better than screen-space edges but will
   not reach good drawing alone. It is the fallback and the rough.
3. **A model draws them.** A Psionic model that takes a guide (pose
   silhouette and model sheet) and returns `anim.drawing.v1` strokes, and a
   second that cleans a rough into a key. Trained only on drawings we own or
   have permission for (our own artists' keys from source 1), never on a
   studio's frames. This is Psionic work and gets its own target doc in
   `psionic/docs/targets/` before any training.

The **model sheet** for each character is a document set (turnarounds,
expressions, hands, mouth shapes, colour model): the reference every key is
checked against, and the input to sources 2 and 3.

### 3. In-betweens and timing

- **The sheet times everything.** Keys and breakdowns are placed on
  frames; the in-between chart says how many drawings go between and their
  spacing. Default twos.
- **Stroke-matched in-betweening in Coax.** Between two drawings whose
  strokes correspond (same stroke ids, or a correspondence the artist or a
  matcher sets), each stroke's points interpolate along the chart's
  spacing, on arcs rather than straight lines (an arc hint per stroke or per
  region, from the guide's joint paths). Regions follow their strokes, so
  paint stays closed.
- **Where matching fails** (a hand turns over, hair crosses the face) the
  sheet asks for a breakdown drawing instead of inventing one. A generated
  in-between that breaks region closure or crosses a stroke is refused, not
  shipped.
- **Smears, multiples and impact frames** are drawing roles, placed on the
  sheet like any drawing.
- **Secondary motion loops**: hair, cloth tails, grass and the like are
  short drawn cycles on their own levels, exposed on the sheet, with wind
  strength choosing between cycles.

### 4. Ink and paint

- Regions are filled from the colour model for the shot's lighting
  condition. The shadow and highlight are their own regions, drawn as shapes
  (a shadow line in a colour role that is not inked), not computed from
  light.
- Line colour per region (a darker shade of the fill for skin and hair,
  near-black for the outline), as cel colour trace lines.
- The renderer is 2D and runs on WebGPU and Metal like the rest of Aethyr:
  stroke tessellation into triangles with width per point and round caps,
  analytic coverage antialiasing, region fill by triangulating the closed
  outline (or a winding-number pass on the GPU), all in Coax and emitted as
  WGSL/Metal from `aethyr/render/` the way existing passes are. No raster
  drawing done in a host language.
- Line boil (the small frame-to-frame jitter of hand-drawn lines) is a
  seeded, opt-in parameter per level, off by default for holds.

### 5. Painted backgrounds

- **A brush engine in Coax** that paints a plate once, offline or on load,
  into a texture through `aethyr/render/host-image.coax`: flat gouache
  strokes with ragged edges, watercolour washes with wet edges and
  granulation on a paper texture, dry brush for foliage, and masks for hard
  architectural edges. Seeded and deterministic, so the same document paints
  the same pixels.
- **Painted from the layout**: the background document's layers are
  authored (by a person in a painting station, by the engine from the
  layout's 3D set as an underpainting, or later by a model) and the
  composition is the shot's.
- **Atmosphere by value**: far planes lighter, cooler and lower in contrast
  in deliberate steps, not a fog curve. Clouds are painted shapes with a lit
  top, a shadowed base and soft edges where they thin.
- **Multiplane**: a plate splits into planes (sky, far, mid, near, BOOK
  overlays) that the layout's camera moves at different rates.
- The 3D set of the courtyard stays useful: it renders the perspective
  underpainting and the guide lines, so a painted background agrees with
  the previz staging.

### 6. Compositing

- Levels stack in the layout's order over the background planes; the
  camera's field crops and moves them.
- Photography: a diffusion pass (a soft, thresholded glow on bright areas,
  controlled per shot), drawn light-ray and shadow cels, a gentle gradient,
  optional grain. Defaults are restrained; the current presets' saturation,
  bloom, sparkles and streaks are not carried over.
- Output is the same as now: frames hashed per frame, rendered headless to
  video through `labs anim play` and `coax/web/tool.sh clip`.

## What changes in the code

| Today | After |
| --- | --- |
| `verse/style.coax` anime functions, `verse/style-preset.coax` anime presets | Kept for realms that want a cel-shaded 3D look (Everglade zones may), renamed `toon` in [art-styles.md](art-styles.md). Anime Lab and Animation Lab stop using them. No more tuning of them for "anime" |
| `apps/openagents-web/animelab-cast.coax` primitive cast with face marks | Replaced by drawn characters: model sheets, colour models and a drawing library per archetype. The primitive cast stays only as a previz stand-in |
| `apps/openagents-web/animelab-world.coax` 3D courtyard | Becomes previz and the underpainting source for painted backgrounds |
| `apps/openagents-web/gpu/animelab.coax` cel shading, screen-space ink, bloom | Replaced by `aethyr/render/` 2D passes: strokes, fills, painted plates, multiplane compositing, diffusion |
| `verse/anime/stage.coax` poses blended per frame | Directions resolve to drawings on an exposure sheet: an `act` picks a drawn action from the library and times it; an `emote` swaps a head or mouth drawing; a `shot` is a layout with a camera move |
| `verse/anime/render-profile.coax` | Stays as the honest list of what the drawn library supports. It starts small again and grows with each drawn action |
| `verse/animlab/` scenes and cuts | Gain the drawn style: a scene can render as 3D previz or as drawn shots from its layouts and sheets |

New code, in Coax, with checks written first:

- `aethyr/anim/board-document.coax`, `layout-document.coax`,
  `drawing-document.coax`, `xsheet-document.coax`,
  `colormodel-document.coax`, `background-document.coax`,
  `composite-document.coax`: readers and `#authority` admission, `coax
  mutate` with zero survivors.
- `aethyr/anim/inbetween.coax`: stroke correspondence, arc interpolation,
  chart spacing, closure checks.
- `aethyr/anim/trace.coax`: guide contours and feature lines from a posed
  rig, and the clean-up into strokes.
- `aethyr/render/stroke.coax`, `fill.coax`, `brush.coax`, `multiplane.coax`,
  `diffusion.coax`: the 2D passes.
- `verse/animlab/` stations: `#board`, `#layout`, `#draw` (with a light
  table and onion skin), `#sheet`, `#paint`, `#bg`, `#composite`.
- `openagents labs anim` commands: `board`, `layout`, `draw` (import SVG or
  a stroke file), `sheet`, `inbetween`, `paint`, `bg`, `composite`.

## The director

Anime Lab's director (`verse/anime/director.coax`) keeps its catalogue and
its rule that words go through a decision model with typed answers, never
keyword matching. What changes is what a direction resolves to:

- `act`, `emote`, `cast`: a drawn action from the character's library
  (keys, breakdowns, charts) placed on the sheet, with anticipation and
  follow-through drawings the action carries.
- `shot`, `cut`: a layout and a camera move from a layout library
  (close-up, medium, wide, pan across, truck in) with a painted background
  matched to the place and time.
- `set`: the background's lighting condition and the colour model's
  condition together.
- `make`: draws in, as now, but the draw-in is a real one: the rough
  sketch, then the clean line, then flats, then the shadow shapes.

A7 (#12972) then directs drawn scenes as it was going to direct 3D ones:
"hold on her face longer", "she looks up at the clouds before she speaks",
"put the wind in her hair" are edits to the sheet and the layout.

## Checks

Each document gets admission checks, and the drawn output gets measured
checks shown in the stations, as Animation Lab's are:

| Check | Measures |
| --- | --- |
| Closure | Every paint region closed within the gap tolerance; no fill leaks |
| Line quality | Width varies along each stroke (min/max ratio over a floor), ends taper, no stroke is a constant-width polyline unless its pen says so |
| Palette | Per shot: mean saturation and the share of pixels over a saturation ceiling; greyscale value separation between character and background |
| Timing | Drawings per second per level (twos by default), holds counted, no level changing every frame unless marked as ones |
| In-betweens | Generated drawings keep closure, don't cross strokes, and stay within a distance of the arc; failures listed as breakdowns needed |
| Model | Head and body proportions of each key against the model sheet, within a stated tolerance |
| Determinism | Same frame hashes from two renders, as now |
| Review | A fixed reference board of original studies (below); the owner signs off each milestone's frames against it, recorded in `NEEDS_OWNER.md` |

The reference board is ours: a few original frames painted or drawn to the
target quality (a character in wind on a hill, the academy courtyard at
dusk, a quiet interior, a flight over the lake, a fire). It defines "good"
for this project in pictures, since the principles above are only words.

## Rights

- Inspiration is principles stated in words. No studio's frames, model
  sheets, backgrounds, character designs, music or titles are committed,
  traced, used as guides or used for training. No "in the style of" prompts
  naming a studio or a person to any model.
- Every drawing and painting document records its source: a person (with
  their consent to use and, separately, to train), the engine's trace, or a
  model with its weights' identity.
- Licensed motion (Valley, Slay) stays private as now. A traced key from a
  licensed clip's guide is a derivative and stays private too; published
  scenes use our own motion, as A8 already requires.

## Milestones

Each ends on a check run on our own machines and frames the owner reviews.

| | Milestone | Done when |
| --- | --- | --- |
| D0 | Formats and the reference board | `anim-drawn-v1.md` written; the seven documents admit with zero surviving mutants; the reference board exists and the owner approves it |
| D1 | Strokes and fills | One drawn character turnaround (from a person or an imported SVG) renders through the 2D passes on WebGPU and Metal: tapered strokes, closed fills, shadow shapes, line colour by region; the closure and line checks pass |
| D2 | The sheet and in-betweens | A walk on twos and a head turn: keys and breakdowns on an exposure sheet, in-betweens generated along charts and arcs, refusals listed; the timing and in-between checks pass |
| D3 | Painted backgrounds | The courtyard at dusk and a hillside in day as painted plates from the brush engine, split into planes; the palette check passes; owner review against the board |
| D4 | Layout and compositing | One shot end to end: layout with guides from the 3D previz, levels over a multiplane background, a pan, diffusion; `labs anim play` renders it, same hashes twice |
| D5 | Drawing station | `#draw` in the realm with pen pressure, light table, onion skin and guides; the iOS app's Pencil writes the same documents |
| D6 | Trace and clean-up | Keys traced from a 3D performance and cleaned procedurally; measured against hand keys for the same pose; used as roughs |
| D7 | The Guardian, drawn | The Guardian's six shots as drawn shots from the same scene and cut: previz and drawn side by side, shot boundaries and event times matching (this is A6, #12971, redefined) |
| D8 | Directed drawn scenes | Anime Lab's director resolves to drawn actions and layouts; the demo session plays drawn |
| D9 | Model-assisted keys and in-betweens | Psionic target doc, consented training data from D5, then models that propose keys and in-betweens as documents a person accepts or edits |

D1 to D4 are what make the look stop being ugly. They don't depend on any
model: they need drawings (people) and a renderer that respects them.

## Owner decisions needed

To be added to `NEEDS_OWNER.md`:

1. **Who draws the first keys and paints the reference board.** A person
   (the owner, a commissioned animator and background painter) or the
   engine's trace only. Quality depends on this more than on any code; the
   plan assumes at least one artist for the board, one character's model
   sheet and D1 to D3's drawings.
2. **Whether A6 (#12971) is redefined** as D7 (previz plus drawn) rather
   than "realistic plus cel shader". Recommended.
3. **Whether the existing anime presets are renamed `toon`** and kept for
   3D realms, or removed.
4. **Training consent terms** for D9, before any drawing from a person is
   kept for training.

## Issues to file

One issue per milestone (D0 to D9), each linking this doc, with the rules
of the Animation Lab issues (Coax only, checks first, private packs for
licensed sources, Verse renders every frame), plus edits to #12971 (A6),
#12972 (A7) and #12973 (A8) pointing at the drawn path.
