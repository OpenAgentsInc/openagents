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

Built locally by `scripts/blender/draconia.py` (not committed: 1.5-2.3 M
triangles, 4K maps); rebuild with the commands under
[Build and render](#build-and-render). Every build is skinned on the
Universal rig (her own proportions, the Universal joint orientations), so
every Universal clip plays on her, and it carries extra joints: `eye_l`,
`eye_r` (gaze) and hair chains `hair_<chain>_<link>`.

| Asset (round 2, 2026-10-13) | Path | What |
| --- | --- | --- |
| Long, loose, waist-length hair, glare, gold hoops (peacock look) | `~/work/film-out/draconia/build-long/draconia-long.glare.glb` | `--hair long --earrings --expr glare`; 18 hanging chains plus fringe and temple locks on the head |
| Long hair, battle decals, lips parted (D1 shot 1) | `~/work/film-out/draconia/build-battle/draconia-long.parted.glb` | `--hair long --earrings --battle --expr parted` |
| The parity updo (the classroom reference) | `~/work/film-out/draconia/build-updo/draconia.neutral.glb` | Updo; temple locks on chains |
| Draconia's rigged MetaHuman, neutral / smile / speaking, with MetaHuman's own skin, eyes and teeth, the uniform on her body (round 6; private MetaHuman asset, never for AI training) | `~/work/film-out/draconia/metahuman/build-rig-{neutral,smile,speak}/mh.<expr>.glb`, set `~/work/film-out/draconia/build/draconia_set_parity.glb` | Static (not on the Universal rig yet): `draconia_portrait --static`; close-ups and the medium shots (`--shot medium24`, `medium29`) |

**Through the zone renderer (`Beast`, the peacock scene):** see
`crates/verse/examples/draconia_form.rs`, which does exactly this:

```rust
let mut draconia = characters::film_character(&mut engine, &dir, &glb,
    &root.join("assets/verse/characters/quaternius"),
    characters::ImportOptions { max_texture_edge: 1024 })?;
let spheres = hair_chains::humanoid_spheres(&draconia);
let rise = hair_chains::bake(&draconia, 0, 901, 2.0, 30.0, 2.0,
    Spring { stiffness: 25.0, ..Default::default() }, &spheres, rising_gust(0.2))?;
draconia.clips.push(rise);
let form = everglade_pack::film::add_character(&mut zone, &engine, &dir, &draconia,
    "film/draconia", &clips /* PLAYER_CLIPS + ("rise", 901) */, 1024)?;
let beast = Beast::new(&zone, &form)?; // then beast.pose_clip(.., "rise", t, ..)
```

`rising_gust` (in that example) is an upward, outward and backward blast
that peaks 0.25 s after it starts and fades over 1.15 s. The zone renderer
draws base colour only.

**Through the imported renderer (film close-ups):** see
`crates/verse/examples/draconia_portrait.rs`: `characters::film_character`
with 4096-pixel maps, `Prepared::load` with a raised `Budget`, then
`Renderer::from_prepared`; `hair_chains::bake` for the wind and
`hair_chains::aim` to lock the eyes on a lens.

## Current state vs reference

| Area | State | Gap |
| --- | --- | --- |
| Face shape | Round 18: MetaHuman Creator's rig of a preset blend (round 7) under a structural sculpt layer (`face_v`): shorter tapered lower face, slimmer jaw angles and neck, cheekbones with hollows, narrower sides, slimmer nose; eyes 14% larger | Chin less pointed and cheek hollows shallower than the reference's; 5% narrow in the neutral and 15% narrow at the jaw in the smile |
| Skin | Round 18: MetaHuman's albedo graded paler, 30% evened toward its blur; its normal eased 45% toward flat with our micro normal over | Creator's palest tone is still warm; wrinkle maps not applied per expression yet |
| Freckles | Round 7: MetaHuman's freckles at density and strength 0.25, small dark spots lifted 85% | Fine and sparse; close to the reference |
| Eyes | Round 18: 14% larger across the face; refracting light grey-green iris (larger in the eye), cornea shell, limbal ring; lids fully open at rest | 12% more open than the reference's; under-eye shadow ring; lower lashes spiky up close |
| Brows | Round 10: MetaHuman `Eyebrows_M_Full` cards over a feathered painted brow and 700 strand brows | Close in shape; slightly darker and more even than the reference |
| Lips | Round 18: rose vermilion drawn on MetaHuman's lip outline in UV over its mauve-grey; fuller, less projecting; no roll-in at rest | Lower lip +9% when speaking; the bow less crisp than the reference's |
| Hair (updo, strands) | Round 16: MetaHuman `Hair_S_UpdoBuns` cards under our grown strands: a centre part with curtains over the temples, clumped locks, heavier face-framing locks, flyaways; platinum | A fine haze up close, not glossy separated locks; too little root-to-length contrast; the cards' warmer blonde shows through |
| Uniform | Round 15: cut from MetaHuman's body and neck (arms lowered): a collar band that is a shell of the neck, a fall with points laid on the chest, a seated knot, striped tie, sweater V with trim, open robe with green facing; fabric weave normals | Points wider and flatter than the reference's; tie stripes too coarse; robe reads flat black, ragged over the shoulders, no hood lining |
| Lighting response | Measured against 0:54: warm key screen left, warm fill, short teal rim, near-black background, soft shadows from moving area lights; f/2 bokeh of glowing jars | No Unreal-grade skin shadowing (screen-space scattering) |
| Expressions | Round 6: her full rig through OpenRigLogic (joints and blend shapes; smile, speaking, neutral) | Built per expression, not live in Verse; wrinkle and blood-flow maps not yet driven |

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
python3 scripts/verse/draconia_parity.py columns GRID.png "reference,Unreal 5.8,Verse" REF UE OURS ...
```

The MetaHuman path (round 4 on; private, never committed, never used to
train AI models):

```sh
sh scripts/unreal/build_dna_dump.sh       # OpenRigLogic (MIT) and dna_dump
DNA="/Users/Shared/Epic Games/UE_5.8/Engine/Plugins/MetaHuman/MetaHumanCoreTechLib/Content/ArchetypeDNA/SKM_Face.dna"
dna_dump "$DNA" MH/archetype                                  # rest
dna_dump "$DNA" MH/archetype-smile --controls scripts/unreal/expressions/smile.txt
MH_NEUTRAL_DIR=MH/archetype MH_BODY_DIR=MH/archetype-body \
  $B -b --factory-startup --python scripts/blender/metahuman_import.py -- \
  MH/archetype-smile MH/textures MH/build-smile/mh.smile.glb
draconia_portrait OUT.png --model MH/build-smile/mh.smile.glb --static --shot fullface --light ref
DRACONIA_UE_KEEP=1 scripts/unreal/run_ue.sh scripts/unreal/render_reference.py   # Unreal's column
# Round 5: Draconia's shape, fitted by MetaHuman Creator, on every expression
MH_SHAPE_OUT=MH/draconia_shape_head.txt MH_NEUTRAL_DIR=MH/archetype ... metahuman_import.py ...  # writes the target
scripts/unreal/run_ue.sh scripts/unreal/fit_face.py           # MH/export/fit/Draconia_Head.fbx
MH_SHAPE=fit MH_FIT_FBX=MH/export/fit/Draconia_Head.fbx MH_NEUTRAL_DIR=MH/archetype ... metahuman_import.py ...
$B -b --factory-startup --python scripts/blender/draconia_set.py -- ~/work/film-out/draconia/build/draconia_set_parity.glb --soft-jars
```

`MH` is `~/work/film-out/draconia/metahuman`; the textures come from
`scripts/unreal/export_textures.py`, the body meshes from `dna_dump` on
`SKM_Body.dna`.

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

### Round 2, 2026-10-13: a rig, hair that moves, and the peacock-open close-up

- **Gap.** Round 1's Draconia was a static mesh: no clips, no hair motion,
  no gaze; the peacock scene couldn't use her. Then (the owner's priority)
  her face in the peacock opening's eye close-up.
- **What changed.**
  - **Rig** (`scripts/blender/draconia_rig.py`): her limbs turn to the
    Universal rest directions; the CC0 skeleton's 163 bones map onto the
    Universal 65; the glb's skin keeps the Universal joints' orientations
    at her own joint positions, plus `eye_l`/`eye_r` and hair chains.
    Clothes keep the body's own weights vertex for vertex.
  - **Hair chains** (`crates/verse-engine/src/hair_chains.rs`): Verlet
    spring chains with gravity, wind (gusts, turbulence, an outward blast),
    link lengths and body spheres, baked into clips; `aim` locks eye
    joints on a target. Tested.
  - **Zone-pack forms** (`everglade_pack::film::add_character`): a film
    character added to a loaded zone pack in memory for `Beast`.
  - **Long hair** (`--hair long`): waist-length on 18 chains, a
    side-swept fringe and temple locks; earrings (`--earrings`); a glare
    and a parted expression; soot and blood (`--battle`); pale brows
    (`--brows pale`).
  - **Eyes**: eye joints and gaze; socket occlusion in the eye's map
    (lid shadow, recessed corners) applied to its direct light; a tear
    line and caruncles; heavier lashes with outer wings; a finite-size
    catchlight on the cornea.
  - **Brows**: hair strands over paint; the paint and strands are made on
    the neutral face, so expressions carry them.
  - **Face**: a narrower face and nose, heavier lids, a shorter forehead.
  - **Portrait renderer**: head turn, gaze, push-in video, peacock and
    low-key lighting, the `peacock_open` shot, focus on the corneas.
- **Artifacts.** `~/work/film-out/draconia/parity/round-02/sbs-*.png`;
  peacock-open v1-v3 in `~/work/film-out/draconia/parity/peacock-open/`
  and `~/Downloads/draconia-face-peacock-open-v{1,2,3}.{png,mp4}`.
- **Honest state.** The owner judged v2's heavy glare "horrible" and v1
  "good enough"; v3 returns to v1's expression. Hair still reads
  translucent and thin at the temples; skin still reads smooth and CG.
- **Next.** A full-face portrait every round (neutral, slight smile,
  speaking) for the mouth; opaque, shadowing hair; ash-blond brows; gold
  that reads as gold.

### Round 3, 2026-10-13: the full face against the photoreal reference

- **Direction.** The owner: compare only against the photoreal video, and
  show the full face (mouth, smile, speaking) every round; the primary
  target is the video at 0:54 (a gentle closed-mouth smile).
- **What changed.**
  - A `fullface` shot preset matching 0:54; the parity lighting's key moved
    to screen left (it had been on the wrong side), teal rim screen right,
    dimmer jars.
  - Mouth: the lip colour follows the mesh's own lips (a `lipmask`
    attribute from the CC0 lip-volume targets, baked to the texture);
    rose-brown lips with a darker meeting line, moister and glossier with
    fine lip lines; soft nasolabial folds; less pouty lips, a wider mouth;
    a gentle closed-mouth smile with dimples.
  - Skin: baked ambient occlusion in the skin's ORM, applied to direct
    light too; paler, cooler, blotchier tone; less subsurface red; a
    matter specular (0.3).
  - Brows lighter ash brown; eyes a little more open; puffier updo with
    more, wavier face-framing strands.
- **Artifacts.** `~/Downloads/draconia-fullface-v3.png` (0:54 smile, 0:24
  neutral, 0:29 speaking, 1:22 eyes, each against ours),
  `~/Downloads/draconia-parity-round-03.png`, the progress sheet;
  `~/work/film-out/draconia/parity/round-03/`.
- **Honest state.** Still a CG mannequin next to the video. The speaking
  mouth shows the CC0 teeth as a pale block; the smile reads as a smirk;
  skin is smooth and orange-tinted; the updo's top is a fuzzy cloud.
- **Next.** A mouth interior (cavity, teeth, gums), thinner lips and a
  symmetric smile; skin texture and sheen; a softer, messier updo; a
  darker, softer background.

### Round 4, 2026-10-13: the MetaHuman archetype in Verse, posed by RigLogic

- **Direction.** The owner: adapt MetaHuman's stack specifically; use a
  real MetaHuman through the private asset pipeline; show the photoreal
  video, Unreal and Verse side by side. The coordinator's review of MH v1
  set the order: sclera, skin albedo, light and grade, hair and clothes,
  brows.
- **What changed.**
  - Expressions from MetaHuman's own rig: `dna_dump` now poses the
    archetype face with OpenRigLogic. Its joint outputs are deltas from the
    neutral joint attributes (the bug that zeroed every posed vertex was
    reading them as absolute: scale 0). Smile, speaking and neutral are
    control files in `scripts/unreal/expressions/`; a posed head is painted
    from the rest head so the lips' colour stays on the lips.
  - Light, after measuring the 0:54 frame: the key really is on screen left
    now. Round 3's note was wrong: the camera looks along +Z, so screen
    left is world +X, and the key, candle and rims were mirrored. The fill
    is warm-neutral and brighter (the reference's shadow cheek reads
    97,75,66 to the lit cheek's 124,112,107); the teal rim is short-ranged
    so it edges her without lighting the shelves; the jars' glow is cut to
    0.2 for the full face (`--set-glow`); the background now reads near
    black (1,8,6 against the reference's 6,9,7).
  - Soft area lights: a soft source is a few points that move over its
    disc from subframe to subframe, so the strongest get the four shadow
    maps and the accumulated frame has soft shadows.
  - Renderer (general): a shadow's normal offset now scales with the
    receiver's distance from the light (two texels of a 512 cube face,
    capped at the old 2.5 cm), so a light near a face casts the nose's and
    cheek's shadows. That removed a teal streak down her nose from an
    unshadowed rim.
  - Hair: platinum strands (ash roots), ten thick face-framing locks a
    side; the uniform's collar and robe placed on the MetaHuman body's
    real neck height (its mesh stops at the shoulders, 1.29-1.31 m).
  - Brows softer (less painted fill), a lighter grey-green iris, teeth
    less yellow.
  - Unreal: UE 5.8 is in use; `render_reference.py` renders the archetype
    in Unreal for the grid's middle column; API changes are in
    `metahuman-study.md`.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v2.png` (rows: 0:54
  smile, 0:24 neutral, 0:29 speaking, 1:22 eyes; columns: reference,
  Unreal 5.8, Verse), `~/Downloads/draconia-parity-round-04.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-04/`.
- **Honest state.** The face is still MetaHuman's generic archetype, not
  Draconia: wider jaw, older and heavier than the reference's young face,
  with our procedural skin paint. Unreal's column is the same archetype
  in its untextured identity material, because the face rig and the
  textures wait on the owner signing in to Epic. The brows read as a
  drawn band at eye-ECU range; the hair's top is a thin fuzz.
- **Next.** An original Draconia face as a real MetaHuman: our sculpted
  head fitted into MetaHuman's identity space (`fit_state_to_target_vertices`,
  local in 5.8), then rigged and textured once the owner signs in; skin
  from MetaHuman's texture sources; strand brows.

### Round 5, 2026-10-13: Draconia's own face as a MetaHuman, a soft updo, soft bokeh

- **Direction.** The coordinator's read of round 4 ("the first version that
  reads as a real person"): fix the bare, static-shock crown first; then
  the face fit (younger, narrower jaw, fuller lips, higher cheekbones);
  a lighter grey-green iris and a pinkish sclera; soft bokeh instead of
  flat squares.
- **What changed.**
  - Face: our own edits to the archetype's rest head (`draconia_shape` in
    `metahuman_import.py`: lower face up to 11% narrower at the jaw line,
    chin 5 mm higher, lips 2.2 mm fuller forward, cheekbones 3 mm higher
    and 2.5 mm forward, all fading out before the neck seam, the ears and
    the skull), then **fitted into MetaHuman's identity space** by
    MetaHuman Creator in UE 5.8 (`scripts/unreal/fit_face.py`:
    `fit_state_to_target_vertices` with rigid alignment and scale and no
    high-frequency residual; local, no Epic sign-in). The fit's head comes
    out as geometry (`export_geometry`, then FBX); `metahuman_import.py`
    (`MH_SHAPE=fit`) matches it to the DNA's vertices by texture
    coordinate, aligns it by a similarity transform (median 1.4 mm off our
    edit), and adds the offsets to every expression the rig poses. The
    painted nasolabial folds are off for MetaHuman heads (the mesh has its
    own; painted on top they aged her).
  - Two traps found on the way, now in the scripts' comments: the target
    head must be given in Unreal's own DNA axes (x, z, y), or the fit
    pitches the face back over the neck; and the skeletal mesh holds every
    face part with overlapping UVs, so only the head section is matched.
  - Hair: a soft updo instead of a frizzed helmet. The strands lie close
    (lift 7 mm, not 17), wander less, and gather into the bun; the
    hairline comes 0.06 rad lower at the front and 0.26 at the temples,
    with 5,000 extra fine roots along it; the cap under the strands reaches
    the hairline in the hair's own shade, its edge cut on a subdivided
    scalp so it no longer steps; 260 flyaways that fall instead of 900
    that stand up. All are `draconia_hair` settings, the CC0 builds'
    defaults unchanged.
  - Eyes: the iris's fibre pattern pushed up and mostly grey with a green
    cast; the sclera faintly pink; softer brows (400 strands, less painted
    fill).
  - Background: jars glow along their height with a meniscus line and
    cloudy variation (`draconia_set.py --soft-jars`, written to
    `build/draconia_set_parity.glb`, the shared set untouched); the
    full-face shot opens to f/2 with 40 sub-frames and the jars at 0.35,
    so they melt into soft round shapes.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v3.png` (reference |
  Unreal 5.8 | Verse; rows 0:54 smile, 0:24 neutral, 0:29 speaking, 1:22
  eyes), `~/Downloads/draconia-parity-round-05.png`, the progress sheet,
  `~/Downloads/draconia-parity-round-05.mp4` (the rounds so far);
  `~/work/film-out/draconia/parity/round-05/`.
- **Honest state.** Better, still not there. The fitted face is younger
  and narrower but still reads older and harder than the reference's
  (under-eye hollows, a long nose); skin is our procedural paint, not
  MetaHuman's texture sources; the updo's strands read grainy at the
  crown; the shoulders show no robe in this frame. Unreal's column shows
  the fitted head in the plain identity material: no rig, no textures
  until the Epic sign-in.
- **Next.** MetaHuman's own skin (texture sources once signed in, else a
  synthesized albedo from the plugin's skin-tone data); smoother strand
  shading at the crown; under-eye and nose edits through the fit; the
  robe and sweater in frame.

### Round 6, 2026-10-14: measured against the reference, and her real rig

- **Direction.** The coordinator's round 5 review, in order: a shorter,
  younger face fitted to the 0:54 frame's proportions with the error per
  feature reported; a loose updo instead of two lank curtains; warmer,
  real skin and the reference's light; the uniform and each row's
  framing; the Unreal column dropped until it shows her properly. Then the
  owner signed in to Epic, and the coordinator asked for her real rig and
  MetaHuman's own skin.
- **Measuring.** MetaHuman's own face tracker runs on the reference
  stills and on our renders (`scripts/unreal/landmarks.py`, local, the
  tracker MetaHuman Creator conforms portraits with);
  `scripts/verse/draconia_landmarks.py` turns its lips, eyelid and
  nasolabial curves into proportions over the eyes' spacing. Face width
  and the chin, which the tracker doesn't give, are hand-marked. Same
  tracker on both sides; each reference row against the render of the
  same shot.
- **What changed.**
  - Face: the shape edits re-tuned on those numbers (a shorter upper lip
    and chin, rounder, fuller cheeks, the under-eye hollows filled, the
    mouth's corners drawn in, lids opened by the rig's controls). The
    face's width to height went from 22% narrow (round 5) to 3% at the
    0:54 frame; nose base to lip from +27% to +3% (on our edits alone).
  - Her real MetaHuman: a headless run on a copy of the project
    (`scripts/unreal/draconia_rebuild.py`; the owner's open editor
    untouched) removes the rig, fits these edits into MetaHuman's identity
    space, sets her look in Creator's controls (the palest tone, freckles,
    grey-green eyes, the `Hair_S_Updo` groom, natural brows), creates the
    full rig (joints and 782 blend shapes) and downloads the texture
    sources, using the owner's stored sign-in; no person needed. Verse
    poses that rig through OpenRigLogic and takes MetaHuman's albedo and
    normal. Creator's palest tone is still a saturated peach (measured:
    `skin_tone_probe.py`), so the albedo is graded paler and less
    saturated toward the reference (`MH_ALBEDO_GRADE`, lips kept), and its
    freckles softened (`MH_FRECKLE_SOFTEN`). The DCC export's roughness
    map came out empty, so the roughness stays ours.
  - Hair: pale golden blonde with darker honey roots; 14 thinner locks a
    side that fall close by the cheeks and curl toward their ends; more
    volume at the crown with less wander (no frizz), wider ribbons so the
    crown isn't grainy.
  - Light: no flat frontal fill: the shadow side gets only a soft, faintly
    teal bounce from the side; a softer, warmer key from camera left; a
    stronger teal rim.
  - Wardrobe and framing: MetaHuman's body posed with its arms down
    (`dna_dump --joints`), the uniform cut from the body and the head
    mesh's neck together so it reaches the collar; new `medium24` and
    `medium29` shots framed as the 0:24 and 0:29 rows, which now show the
    collar, the striped tie and the robe's green facing.
  - Renderer (general): a soft light's shadow filter no longer samples
    past its cube face's edge (that drew a seam where two faces of a near
    light met).
- **Measured now** (MetaHuman's tracker; ours minus the reference, as a
  share of the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -6.9% | -7.3% | -7.7% |
  | Eye opening | +1.4% | +3.0% | +6.7% |
  | Eye line to nose base (midface) | +1.4% | -1.7% | -2.6% |
  | Nose base to upper lip | +10.4% | +8.1% | +6.3% |
  | Upper lip height | -22.4% | -11.5% | -5.1% |
  | Lower lip height | -9.2% | -8.8% | +3.4% |
  | Mouth width | +3.1% | -1.3% | -9.2% |

  Hand-marked at 0:54 on our edits before the rig: face width to
  eye-to-chin height -3.0% (round 5: -22.2%); lip to chin +18% (long).
  The rigged head (Creator's fit of the edits) gave back some of the
  upper-lip shortening: its fit regularizes toward MetaHuman's faces.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v4.png` (reference |
  Verse; rows 0:54 smile, 0:24 neutral, 0:29 speaking, 1:22 eyes),
  `~/Downloads/draconia-parity-round-06.png`, the progress sheet;
  `~/work/film-out/draconia/parity/round-06/`, the landmark files in
  `~/work/film-out/draconia/parity/landmarks/`.
- **Honest state.** Her real rig and MetaHuman's skin are in, but she
  doesn't pass as the reference yet: the face still reads older and
  broader, the upper lip thin and long, the eyes narrow for their spacing;
  MetaHuman's albedo needed grading to come near the reference's pale
  skin. Unreal's own render of her is bald (the groom isn't in the DCC
  bundle) and in default lighting, so its column is out of the grid until
  it shows her with hair and clothes.
- **Next.** Lips and philtrum through Creator's own sculpting on the rigged
  head (the rig must be removed and remade); the eyes wider; wrinkle and
  blood-flow maps driven per expression; MetaHuman's groom as strands in
  Verse; her hair and the uniform in Unreal's column.

### Round 7, 2026-10-14: from MetaHuman's presets, not the archetype

- **Direction.** The coordinator's read of round 6: still not her (a broad
  round jaw, a heavy nose, small deep-set eyes, older and blotchy), and
  the archetype is the problem. Start from the closest Creator presets,
  scored against the reference; blend them; small edits on top. Then
  finer freckles, platinum ash hair with a looser crown, folds in the robe.
  Mid-round: her eyes stay open and large while she smiles.
- **Presets, scored.** `scripts/unreal/presets_export.py` exports all 29
  presets in UE 5.8's Optional/Presets as geometry, headless;
  `scripts/verse/draconia_presets.sh` builds each as a fast, face-only Verse
  model smiling (the archetype rig's smile on the preset's shape), renders
  it at 0:54's framing and light, and `draconia_preset_score.py` ranks them
  by MetaHuman's tracker against 0:54 plus the face's 3D breadth against
  the near-frontal 0:24 still (hand-marked there). Best: Vivian 11.7%,
  Sunita 12.0%, Celeste 12.6%, Ada 12.7%, Etta 13.1%, Aera 13.8%; the
  archetype-derived round 6 head was older and broader than any of the top
  six by eye. Contact sheet: `~/Downloads/draconia-presets-r7.png`
  (`draconia_preset_sheet.py`). Python has no preset-blend call, so the
  blend is vertex-wise over the matched heads (`draconia_preset_blend.py`):
  Vivian 0.45, Celeste 0.30, Sunita 0.25, the same thing Creator's preset
  blend does in its identity space, and Creator's fit takes it from there.
- **What changed.**
  - Face: that blend plus `draconia_refine` (a V jaw tapering to the chin,
    the nose's base 10% slimmer, the mouth 1 mm higher for a shorter upper
    lip, each eye's outer corner 1.8 mm out for longer, almond eyes), fitted
    and rigged by Creator headless, with texture sources. A first pass
    lifted the upper lip alone and its vermilion grew 17-22% too tall; the
    second pass lifts the whole mouth.
  - Expressions: the smile no longer closes her eyes (the cheeks barely
    raise the lower lids, no inner squint); neutral needs no lid lift on
    this head.
  - Skin: Creator's freckles at density and strength 0.25, and the albedo's
    small dark spots lifted 85% toward their surroundings (they read as
    blotches); a glossier, dewy forehead and nose (T-zone roughness down
    0.22).
  - Hair: platinum ash with darker roots, more lift and puff at the crown,
    a messy centre part (strands near the middle choose their side with
    some randomness), 12 wispy locks a side falling past the jaw.
  - Robe: soft vertical folds below the shoulders (`draconia_cloth.FOLDS`).
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -3.2% | -3.2% | -3.9% |
  | Eye opening | +17.6% | +11.1% | +14.9% |
  | Eye line to nose base (midface) | +3.4% | -1.4% | -0.1% |
  | Nose base to upper lip | +2.7% | +6.6% | +0.9% |
  | Upper lip height | +6.8% | +15.5% | +19.5% |
  | Lower lip height | -3.0% | -2.8% | +9.6% |
  | Mouth width | +5.2% | +6.2% | -4.3% |

  Eyes now measure long enough and a little too open; the upper lip is
  fuller than the reference's (as asked) and somewhat too tall.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v5.png`,
  `~/Downloads/draconia-presets-r7.png`,
  `~/Downloads/draconia-parity-round-07.png` and `.mp4` (the rounds so
  far), the progress sheet; `~/work/film-out/draconia/parity/round-07/`.
- **Honest state.** The first version that reads young and slim; closer to
  the reference's type than any round before. Still off: the jaw and
  cheeks are fuller than hers, the hair's crown reads as a white cap, the
  lids carry a dark ring, the robe is still a hard-edged shell.
- **Next.** The crown and hairline (MetaHuman's updo groom as strands in
  Verse); the lid ring; the jaw through Creator's own sculpting; fabric
  normals and the collar's points.

### Round 8, 2026-10-14: MetaHuman's own hair cards, moodier light

- **Direction.** The coordinator's read of round 7 (the best yet; she reads
  young): the hair a white wig cap with crimped ribbons; brows nearly
  invisible; a dark ring round the eyes; flat, even light making her look
  fuller; the robe and collar hard flat cards. MetaHuman's grooms
  suggested if our strand shading is the limit.
- **What changed.**
  - Hair from MetaHuman's grooms: `scripts/unreal/groom_export.py` exports
    a groom's cards meshes (LOD0, every group) and its cards atlas,
    headless. The cards' FBX lands in the archetype DNA's own frame (checked
    against the brow cards, 1.5 cm over the archetype's eyes; a first try
    through the face skeletal mesh's frame put the hair 13 cm low), so
    `metahuman_import.py` (`MH_GROOMS`) takes them onto her rigged head by
    a similarity transform plus each card vertex's nearest-skin residual,
    lifts any card left under her skin to just above it, and shades them as
    Verse hair: the atlas's R channel is the coverage (alpha), G the depth
    (darker in the clumps), B the position along the strand (root to tip
    colour). Warm pale blonde with darker roots. `Hair_S_UpdoBuns` (pulled
    back into buns) fits the reference best of those tried (`Hair_S_Updo`
    has a fringe; the `S_` grooms such as `Messy` and `SweptUp` are short
    cuts that lie flat; `L_MessyClumps` hangs loose); our own grown locks
    stay only as five fine, nearly straight wisps a side past the jaw.
  - Brows: MetaHuman's `Eyebrows_M_Full` cards, dark ash-brown, replacing
    our strand brows.
  - Lids: our baked occlusion at half strength (it dug a dark ring into
    the lids and sockets).
  - Light: the key further round to the side (about 60 degrees off the
    lens) and the bounce four stops under it, so the far cheek falls into
    soft shadow and the cheekbones model.
  - Wardrobe: the shirt collar's points longer and spread over the sweater
    (`draconia_cloth.COLLAR_POINTS`).
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -1.6% | -2.3% | -2.4% |
  | Eye opening | +21.5% | +12.2% | +20.2% |
  | Eye line to nose base (midface) | +3.4% | +0.4% | +1.3% |
  | Nose base to upper lip | +3.2% | +6.4% | +0.8% |
  | Upper lip height | +7.7% | +13.0% | +20.3% |
  | Lower lip height | -3.2% | -1.5% | +8.2% |
  | Mouth width | +2.5% | +6.1% | -3.9% |

- **Artifacts.** `~/Downloads/draconia-fullface-mh-v6.png`,
  `~/Downloads/draconia-parity-round-08.png`, the progress sheet;
  `~/work/film-out/draconia/parity/round-08/` (with `try-*.png`, the grooms
  tried).
- **Honest state.** The hair now has real strand texture and colour, but
  the buns groom pulls it tight and flat over the crown where the
  reference's is loose and full; the brows are there but still faint; the
  eyes measure too open (+12-22%); the new collar points read as a stiff
  white bib in the medium shots; the robe is still a shell.
- **Next.** A fuller, looser crown (MetaHuman's long loose groom blended
  with the buns, or our own body layer under the cards); stronger brows
  (denser cards, a painted brow under them); the lids lowered a touch;
  real cloth for the collar and robe.

### Round 9, 2026-10-14: brows she can be known by, a fuller crown

- **Direction.** The coordinator: round 8 regressed against round 7 (the
  buns groom scraped flat back: nearly bald, a huge forehead and temples;
  brows still invisible: the face looked alien). Brows first; a hair
  silhouette that frames the face; the lids down; a real shirt collar;
  rounds 7 and 9 side by side.
- **What changed.**
  - Brows: our painted brow (strands thick at the inner head, a defined
    arch, a fuller soft fill) laid dark ash-brown over MetaHuman's albedo
    at 72% (`MH_PAINTED_BROW`; `draconia_paint.LAST_BROW` carries the
    brow's coverage out of the paint), under MetaHuman's `Eyebrows_M_Full`
    cards. Close-up: `~/Downloads/draconia-brows-r9.png`.
  - Hair: our combed body (with its bun and volume at the crown) grown
    again, a shade darker than the cards, under MetaHuman's
    `Hair_S_UpdoBuns` cards, so the strand texture stays and the crown is
    full; eight locks a side, some from the front of the hairline
    (`draconia_hair.LOCK_AZ`), not only behind the ears.
  - Lids: the rig's eye-relax control at 0.15-0.2 in every expression
    (round 8 measured them 12-22% too open).
  - Collar: the points concentrated at the very front (two flaps either
    side of the knot), not spread into a bib.
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -4.8% | -4.2% | -4.0% |
  | Eye opening | +4.0% | -4.0% | +1.8% |
  | Eye line to nose base (midface) | +2.2% | -1.8% | -0.1% |
  | Nose base to upper lip | +2.0% | +5.3% | -0.5% |
  | Upper lip height | +6.9% | +11.5% | +19.8% |
  | Lower lip height | -3.7% | -3.0% | +7.6% |
  | Mouth width | +2.4% | +4.4% | -4.6% |

  The eyes now open as the reference's (round 8: +12-22%); every feature
  but the upper lip's height is within about 5%.
- **Honest state.** Better than round 7 side by side: visible, dark brows;
  calmer eyes; real strand texture with volume at the crown; the moodier
  light. Still off: the front hairline sits high and straight, with no
  hair over the temples; the brows are a little heavy and blocky at the
  close-up; the upper lip is tall when she speaks; a dark band under the
  lower lids; the collar and robe still read as shells.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v7.png` (reference |
  round 7 | round 9), `~/Downloads/draconia-brows-r9.png`,
  `~/Downloads/draconia-parity-round-09.png`, the progress sheet (round 8
  kept in it, so its regression stays visible);
  `~/work/film-out/draconia/parity/round-09/`.

### Round 10, 2026-10-14: hair-like brows, a soft shadow side

- **Direction.** The coordinator's read of round 9 (better than round 7):
  the brows read painted-on (hard edges, a uniform fill, a square tail); a
  hard diagonal shadow across the far cheek and nose; the crown a smooth
  helmet with a hard hairline; the dark band under the lower lids; the
  upper lip tall when speaking.
- **What changed.**
  - Brows: our painted base at 50% (from 72), feathered (the mask blurred,
    so no edge or square tail), a lighter ash-brown; our strand brows back
    on top (700 strands, tapering, thicker at the inner head) over
    MetaHuman's brow cards.
  - Shadow side: the key light 0.9 m across (from 0.45), so the nose's and
    the hair's shadows on the far cheek fall off softly.
  - Hair: flyaways at the crown and bun again; 900 baby hairs and short
    wisps along the hairline; the hairline lower over the temples; the
    face-framing locks rooted in front of the ears.
  - Lower lids: the under-eye region of MetaHuman's albedo lifted 18% toward
    the cheek's tone (`draconia_paint.LAST_UNDER`), and our baked occlusion
    at a quarter strength.
  - Speaking: the upper-lip raise eased (0.12 to 0.03).
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -3.5% | -3.7% | -3.4% |
  | Eye opening | +3.8% | -4.0% | +0.1% |
  | Eye line to nose base (midface) | +1.5% | -3.0% | -1.0% |
  | Nose base to upper lip | +2.3% | +5.4% | +3.0% |
  | Upper lip height | +5.9% | +13.4% | +19.9% |
  | Lower lip height | -5.0% | -4.3% | +8.5% |
  | Mouth width | +3.7% | +3.9% | -4.9% |

  The upper lip's height when speaking didn't move with the control: it
  is the blended face's own full upper lip, so it needs a shape change
  (Creator), not an expression.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v8.png` (reference |
  round 9 | round 10), `~/Downloads/draconia-parity-round-10.png` and
  `.mp4`, the progress sheet; `~/work/film-out/draconia/parity/round-10/`.
- **Honest state.** Softer, more natural brows and a gentler shadow side
  than round 9. Still off: the crown still reads smooth and the front
  hairline high; the upper lip is full and tall; the lower lids keep a
  faint band; the collar and robe are shells.

### Round 11, 2026-10-14: wet eyes, a clumped crown, a shorter upper lip, fabric

- **Direction.** The coordinator's read of round 10 (a clear step up; the
  brows and the shadow good): the eyes matte (no catchlight, a grey-dark
  iris, pinkish lid edges); the crown a smooth cap; the upper lip's shape
  change in Creator; the collar, tie and robe the most "CG" part left.
- **What changed.**
  - Eyes: a cornea shell over each eyeball (the eyeball scaled out 0.4 mm,
    specular only, roughness 0.03), so the key light leaves a sharp
    catchlight; the iris a clear sea green with a darker limbal ring; the
    sclera near white. The wet strips at the lid margins and inside the
    lips stay.
  - Crown: fewer, wider, messier strands in our body layer (5,000 at 2.4x
    width) over MetaHuman's buns cards, so it breaks into clumped sections
    with gaps and highlights instead of one sheen.
  - Upper lip: `draconia_refine` lowers the upper lip's top edge 1 mm; the
    blend refitted by Creator, rigged and textured again headless.
  - Uniform: fabric normal maps (`draconia_cloth.FABRIC`: twill on the
    robe and its facing, a knit on the sweater, a fine weave on the shirt
    and collar), a 7 mm robe (its opening edge reads as wool), a 2.5 mm
    collar, deeper folds (9 mm). The shirt's and collar's plain weave is
    left off: through the cloth shading it turned the white parts grey.
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -3.3% | -3.1% | -2.3% |
  | Eye opening | +11.3% | +2.2% | +8.4% |
  | Eye line to nose base (midface) | +1.9% | -1.6% | -0.9% |
  | Nose base to upper lip | +2.2% | +5.1% | +2.8% |
  | Upper lip height | +5.8% | +13.2% | +18.5% |
  | Lower lip height | -5.8% | -4.5% | +6.4% |
  | Mouth width | +3.0% | +4.0% | -5.6% |

  The 1 mm upper-lip edit barely survived Creator's fit (its identity
  space regularizes small local edits): the lip is still tall. The next
  try must be larger or go through Creator's own sculpting landmarks
  (`translate_face_landmarks`).
- **Honest state.** The eyes are wetter and greener with a catchlight
  (perhaps now a touch too saturated); the crown breaks up more; the robe
  has twill and a thicker edge. Still off: the upper lip; the eyes a
  little open in the smile; the hairline high; the collar a stiff band.
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v9.png` (reference |
  round 10 | round 11), `~/Downloads/draconia-parity-round-11.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-11/`.

### Round 12, 2026-10-14: a muted iris, white cloth that stays white

- **Direction.** The coordinator's read of round 11: the catchlight works
  but the iris glowed cartoon teal; the upper lip; the eyes in the smile
  and speaking; the collar; the front hairline.
- **What changed.**
  - Iris: sampled from the reference at 1:22 (its iris reads about 32,41,29
    sRGB, green over red 1.28, blue over red 0.91): a muted olive
    grey-green at about half round 11's saturation and darker, lighter
    flecks round the pupil, a softer limbal ring; the sclera slightly
    off-white. The cornea catchlight stays. Ours now reads about
    24,40,29 (green over red 1.7): closer, still a little green.
  - Upper lip: a sculpt layer over the rigged head (`MH_SHAPE=post`,
    `lip_post`: the line where the lips meet raised 1.6 mm), since
    Creator's fit smooths small local edits away. It moved the smile to
    +3.9% and the neutral to +11% (from +13%); speaking still +17.6%.
  - Eyes: the eye-relax control at 0.35 smiling and 0.3 speaking; the
    opening is now within 1% in all three rows.
  - Cloth: the bug behind round 11's grey shirt and collar was ours, in
    `draconia_cloth._weave`: Blender drops an image's pixels when its
    colour space changes after they are written, so the packed weave
    normal map came out black (every normal tilted). Set Non-Color first;
    the plain weave is back on the white shirt and collar, and they stay
    white. The collar's points reach further over the sweater.
  - Hairline: the grown layer's front hairline lower (0.16 rad).
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -4.7% | -1.8% | -2.5% |
  | Eye opening | -0.8% | +1.0% | 0.0% |
  | Eye line to nose base (midface) | +0.9% | -3.1% | -1.6% |
  | Nose base to upper lip | +1.9% | +5.5% | +3.1% |
  | Upper lip height | +3.9% | +11.0% | +17.6% |
  | Lower lip height | -4.6% | -3.9% | +9.3% |
  | Mouth width | +3.9% | +3.9% | -5.3% |

- **Artifacts.** `~/Downloads/draconia-fullface-mh-v10.png` (reference |
  round 11 | round 12), `~/Downloads/draconia-parity-round-12.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-12/`.
- **Honest state.** The iris no longer glows; the eyes open as hers in
  every pose; white cloth is white. Still off: the upper lip at rest and
  speaking; the hairline; the robe and collar's shapes.

### Round 13, 2026-10-14: a light green iris, the upper lip within 5%, an open robe

- **Direction.** The coordinator's read of round 12: the iris overshot to
  near black at medium framing (the 1:22 sample was in shadow); the collar
  and tie looked unchanged; the speaking upper lip +18%; the crown a cap.
- **What changed.**
  - Iris: a light sage grey-green toward the lit outer iris at 0:54 and
    in the close-up (about 95,120,105 sRGB), with darker flecks; ours now
    reads about 93,108,92 at its lit 90th percentile. In the close-up it
    reads light, perhaps a little pale round the pupil.
  - Upper lip: the rig's upper-lip roll-in (0.15 at rest, 0.3 speaking)
    on top of round 12's sculpt layer: +3.6% / +4.5% / +3.9%, within 5%
    in all three rows for the first time.
  - Collar and tie: two bugs found by rendering the uniform alone. The
    collar band's ring was sorted by plain angle, so its strip bridged the
    gap left for the knot and closed into a white band (now sorted from
    the front, the gap between the list's ends); and the robe's front
    opening stopped 2 cm under the neck, leaving a slab of robe and facing
    across the chest (`draconia_cloth.ROBE_OPEN_TOP`, now open to the top).
    The collar is a short band open at the front with two separate points
    either side of a fuller knot (`COLLAR_STYLE = "points"`); the robe's
    top edge is cut straight.
  - Crown: more lift, longer and springier flyaways (loops off the crown).
- **Measured** (MetaHuman's tracker; ours against the reference):

  | Feature (over eye spacing) | 0:54 smile | 0:24 neutral | 0:29 speaking |
  | --- | --- | --- | --- |
  | Eye width | -2.7% | -1.7% | -2.1% |
  | Eye opening | +4.4% | +5.2% | +4.2% |
  | Eye line to nose base (midface) | +1.1% | -1.9% | -1.4% |
  | Nose base to upper lip | +2.1% | +6.9% | +6.7% |
  | Upper lip height | +3.6% | +4.5% | +3.9% |
  | Lower lip height | -4.8% | -3.7% | +8.9% |
  | Mouth width | +3.9% | +3.7% | -5.7% |

- **Artifacts.** `~/Downloads/draconia-fullface-mh-v11.png` (reference |
  round 12 | round 13), `~/Downloads/draconia-parity-round-13.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-13/`.
- **Honest state.** Light green eyes that read at medium framing; the lips
  within 5%. The collar is no longer a bib, but its points read as two
  long white slivers and the neckline around them is still awkward; the
  crown still reads smooth from the front; the iris's lighter ring round
  the pupil is too bright in the close-up.

### Round 14, 2026-10-14: a greener, darker inner iris; a modelled collar and knot (still wrong)

- **Direction.** The coordinator's read of round 13: the iris reads light
  green at medium framing (keep), but in the 1:22 close-up the white
  starburst round the pupil made it silver-blue; the collar and tie read
  as two white slivers either side of a black cross; stop building them
  from primitives and use a real modelled asset; the crown still smooth.
- **Asset search.** MetaHuman's `Optional/Clothing` holds only
  `DefaultGarment` (a crewneck), and the local Fab library has no school
  uniform, so there is no shirt-and-tie asset to carry over. A Fab
  uniform would need an owner purchase (not done). The collar is modelled
  instead.
- **What changed.**
  - Iris (`metahuman_import.py`): contrast 0.25 (was 0.7), a soft
    shoulder on the green channel so the bright fibres no longer clip
    white, flecks at 0.05, the inner iris darkened 18%; tint
    (0.46, 0.64, 0.50) at gain 4.4. In the close-up the iris now reads
    green with a darker centre; lit 90th percentile about 74,85,67 sRGB.
  - Collar (`draconia_cloth._collar_points`): each point a curved flap
    (a bilinear patch from the band's top edge to a tip on the chest,
    pressed onto the sweater), thickened and subdivided once.
  - Knot (`draconia_cloth._tie_knot`): a trapezoid (3 cm over 1.7 cm,
    2.4 cm tall), bowed and creased, in the tie's diagonal stripes.
  - `SWEATER_TOP`: how far under the neck the sweater stops (MetaHuman
    builds 3 cm), which removes the dark choker band; the shirt runs all
    round above it.
- **Measured** (ours against the reference): face unchanged from round 13
  (all features within 7%; upper lip +3.7% / +4.5% / +4.1%).
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v12.png` (reference |
  round 13 | round 14), `~/Downloads/draconia-parity-round-14.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-14/`.
- **Honest state.** The iris is the best it has been in the close-up. The
  collar is still wrong: the points stand up off the neck like two white
  fins, the knot floats in front of a bare neck, and a white yoke fills
  the chest where the sweater's V should be. It needs rebuilding around
  the neck itself (round 15). The crown is unchanged.

### Round 15, 2026-10-14: a shirt collar built from her neck

- **Direction.** The coordinator: build the collar from the rigged body's
  own neck: the band an offset shell of the neck that touches it, the
  points cut from it and folded down onto the sweater (a short cloth
  simulation pinned at the band was suggested), the knot seated in the
  band's gap, the sweater's V visible with the shirt only inside it.
  Free Fab assets were allowed; nothing to be bought.
- **Fab.** Fab's listing pages refuse automated reads (HTTP 403), and
  adding even a free asset needs the owner's Fab account in a browser, so
  nothing was fetched. Two listings turned up by search are noted for the
  owner (prices not verified): "Japanese Schoolgirl Uniform – Modular
  MetaHuman Outfit Pack" (shirt, tie, cardigan; MetaHuman-fitted) and
  "Japanese Sailor School Shirt – 2 Styles" (shirt only, UE 5.7+). Both
  are a different style from the reference's British uniform.
- **What changed** (`draconia_cloth.py`, `COLLAR_STYLE = "shell"`).
  - The uniform is now cut from a neck that exists: the head's bib now
    runs 5 cm up the neck. The body faces it covers are dropped, and the
    seam where the two meet is welded (`metahuman_import.drop_covered`,
    `weld_seam`). Before, the two overlapping skins gave every garment
    two layers, with a crack of skin across the chest.
  - Band: a 3 mm shell of the neck, 5.5 cm up it at the back and 1.8 cm
    lower at the front, cut along clean planes.
  - Fall and points: a grid from the band's top edge (found by casting
    from the neck's measured centre) round to the back. It is 3.2 cm deep
    at the sides and drawn out at the front into points whose edges run
    from beside the knot down and out to tips 7.5 cm below. Every vertex
    is laid on the outermost layer under it (band, shirt or sweater),
    measured along the body's normal, so the points lie on the chest
    instead of standing off it.
  - The cloth simulation was tried (`COLLAR_SIM`, pinned at the fold).
    At this scale it crumpled the points and the tie, so it is off; the
    projection is deterministic.
  - Knot: 15% larger, seated against the band 7 mm under the fold. Tie
    blade: laid on the shirt's most forward point across its width and
    smoothed (along the centre line alone it dipped behind a ridge).
  - Sweater V and shirt: "front" for the V and the shirt panel now
    reaches back over the upper chest (`V_FRONT`; the MetaHuman `neck`
    mark sits at the neck's front, so the V had been cut only at the
    very front). The sweater runs up to the collar.
  - Robe: open wide at the collarbones (`ROBE_FLARE`, its cut planes
    matching the opening), with a 2 cm green facing.
  - `MH_CLOTH_BLEND` saves the body the uniform is cut from, so the
    uniform can be iterated alone in seconds.
- **Measured.** Face unchanged (all features within 7%; the lower lip
  +9% speaking).
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v13.png` (reference |
  round 14 | round 15), `~/Downloads/draconia-parity-round-15.png`, the
  progress sheet; `~/work/film-out/draconia/parity/round-15/`.
- **Honest state.** For the first time the collar reads as a shirt
  collar: a band at the neck and two points on the chest, with the tie
  knot between them. Still off:
  - the points are wider and flatter than the reference's;
  - the tie's stripes are too coarse, and the knot shows none;
  - the robe's top edge over the shoulders is ragged and the robe reads
    flat black, where the reference's has a green-lined hood;
  - the crown is unchanged.

### Round 16, 2026-10-14: a centre part with curtains, clumped locks, finer tie stripes

- **Direction.** After the collar, the crown: from the front the hair was
  combed straight back from a high hairline (a smooth cap, the forehead
  bare), where the reference's falls from a centre part over the temples
  in loose curtains, with heavier locks in front of the ears.
- **What changed** (`draconia_hair.py`, set in `metahuman_import.py`).
  - Curtains (`CURTAIN`): strands rooted within a radian of the front
    first comb sideways and down from the part to a point above the ear,
    then turn back to the knot.
  - Clumps (`CLUMPS`, `CLUMP`): 160 guide strands are combed; every other
    strand follows its nearest guide, drawn in toward it along its length
    (95% by the tip), so the hair falls in locks with gaps between them
    rather than as an even haze; ribbons 3.5x wider.
  - Face-framing locks: 7 a side of 18-34 hairs each, 16-30 cm, so they
    read as locks, not single hairs.
  - Tie: stripes about 3 cm a repeat (`TIE_STRIPES`), on the knot too.
- **Measured.** Face unchanged (within 7.4%).
- **Artifacts.** `~/Downloads/draconia-fullface-mh-v14.png` (reference |
  round 15 | round 16), `~/Downloads/draconia-parity-round-16.png`, the
  progress sheet, and a clip of the rounds so far,
  `~/Downloads/draconia-parity-round-16.mp4`;
  `~/work/film-out/draconia/parity/round-16/`.
- **Honest state.** The hair's silhouette is closer: parted in the middle,
  over the temples, locks to the collar. Up close it is still a fine haze
  rather than the reference's glossy, separated locks, with less contrast
  between ash roots and bright lengths; MetaHuman's cards (warmer) show
  through the gaps. The robe is unchanged (round 17).

### Round 17, 2026-10-14: the robe's top follows the neckline, a hood behind the neck

- **What changed** (`draconia_cloth.py`, set in `metahuman_import.py`).
  - `ROBE_NECK_R`: the robe's top follows the neckline. It keeps every
    face further than 8.5 cm from the neck's axis instead of stopping at
    one height. On MetaHuman's body the old height lay below the
    shoulders' line and left a flat shelf.
  - `HOOD_DRAPE`: a soft roll lying on the robe round the back of the
    neck, thicker at the back. It is placed on the body's nearest point
    to a ring at the neck's base: rays cast down met the head or started
    inside the thick back of the neck. A thin green lining tube runs
    along its inner edge.
- **Artifacts.** `~/Downloads/draconia-parity-round-17.png` (reference |
  round 16 | round 17, the medium neutral shot only: the face and hair
  are unchanged, so the full-face grid was not re-rendered); the progress
  sheet; `~/work/film-out/draconia/parity/round-17/`.
- **Honest state.** A small change on screen: the robe now rises to the
  collar over the shoulders, its green facing with it, and the hood is
  mostly hidden behind her at this framing. The robe still reads near
  black where the reference's is a dark teal-green with a visible hood
  roll. Round 18 turns to the face (the owner: it does not read as
  Draconia).

### Round 18, 2026-10-14: a structural face rebuild (outline measures, a slimmer, younger face)

- **Direction.** The owner: "the face looks totally different than
  Draconia… eyes must be light green but the facial structure… is so
  completely not 'attractive girl'". The coordinator's read: hers is a
  slim heart/V face with high cheekbones and hollows, a defined jaw, a
  slender neck, large almond eyes, a slim nose and full but compact lips.
  Ours read broad, round-cheeked, soft-jawed and older. Every inner
  feature was within 7% on MetaHuman's tracker, so those numbers no
  longer guide the work alone.
- **A new measure** (`scripts/verse/draconia_outline.py`). MediaPipe's
  face landmarker (Apache-2.0, run from a local virtualenv, not a repo
  dependency) gives the face's outline, which MetaHuman's tracker does
  not. It reads the outline's width from the eye line to the chin, the
  chin's taper and the vertical segments, all over the eye spacing.
  - Against 0:24 it found the mouth-to-chin length **18.9% long**, the
    chin 5% wide, the nose 8% wide and the mouth 9% wide.
  - The tracker had missed all of these: a long, heavy lower face is
    what read as "older" and "broad".
- **What changed.**
  - `metahuman_import.face_v` is a sculpt layer over the rigged head
    (`MH_FACE_V`; added to every expression, like round 12's lip layer;
    Blender only, no Unreal run). It includes:
    - the lower face shortened 15%, with a V taper to the chin;
    - the jowls and the jaw's angles drawn in, and the neck slimmed 10%;
    - the area under the chin tucked;
    - cheekbones forward, with a deeper hollow under them;
    - the temples, cheekbones and cheeks 7% narrower;
    - the nose's wings 30% narrower, the bridge 15% slimmer and a
      smaller tip;
    - a softer brow ridge and slightly lower brows.
  - Eyes 14% larger (`eye_scale`), across the face only: the eyeball,
    shells, lids and lashes within 1.2 cm, fading by 3.6 cm.
    - Scaled in depth too, the lids came forward and the skin bunched in
      a ring under the eyes.
    - The iris is larger in the eye (UV radius 0.175) and lighter
      (tint 0.50,0.64,0.55 at gain 5).
  - The neutral's lids are fully open and its upper lip no longer rolled
    in (rounds 9 and 13 had tuned both toward the tracker's numbers).
  - Lips: MetaHuman's vermilion is a dull mauve-grey, so the lips read
    thin. It is recoloured rose inside the lip outline, drawn once on
    MetaHuman's shared head UV (`LIP_UV`), at 35%.
  - Skin: 30% of the albedo is replaced by its blur, for an even tone,
    and MetaHuman's normal is eased 45% toward flat (fewer creases), with
    the pores laid over after.
  - A face-only before/after (front and 3/4) went to the owner first:
    `~/Downloads/draconia-face-r18-before-after.png`.
- **Measured** (outline, ours against the reference, round 16 → round 18):

  | Measure | 0:24 neutral | 0:54 smile |
  | --- | --- | --- |
  | Mouth to chin | +18.9% → +1.9% | +16.3% → -0.9% |
  | Eye line to chin | +7.9% → +2.2% | +7.0% → +1.3% |
  | Width at the jaw (0.75) | +2.1% → -4.8% | -8.7% → -14.9% |
  | Width at the chin (0.90) | +5.1% → -4.6% | -7.5% → -14.8% |
  | Nose width | +8.3% → +2.4% | +1.0% → -5.3% |
  | Mouth width | +8.7% → +1.5% | +5.7% → -1.3% |
  | Eye opening | -0.7% → +12.1% | -2.8% → +11.9% |

- **Artifacts.** `~/Downloads/draconia-fullface-mh-v15.png` (reference |
  round 16 | round 18), `~/Downloads/draconia-parity-round-18.png`, the
  progress sheet, the face-only before/after above;
  `~/work/film-out/draconia/parity/round-18/`.
- **Honest state.** She now reads as a slim young woman of the
  reference's type: shorter, tapered lower face, slimmer nose, larger
  light eyes, defined rose lips. She is not yet the same person:
  - the reference's chin is more pointed and her cheek hollows deeper;
  - the reference's brows are finer and more arched;
  - the face is now 5% narrow in the neutral and 15% narrow at the jaw
    in the smile (the reference's smile widens her cheeks), so the width
    edits overshoot there;
  - the eyes are 12% more open than the reference's.

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

### v13, 2026-10-13

- **The eyes close-up is in.** The face work's
  `~/work/film-out/draconia/parity/peacock-open/peacock-open-v1.mp4` (the
  owner approved v1; v2's glare was overdone) is cut in at the reference's
  114.5–114.7 s, between the white flash and the pull-back. The cut takes
  0.2 s of it, from 0.5 s into the clip, at 30 fps. It replaces the first
  0.2 s of the tail shot, so the cut stays 5.0 s and lines up frame for
  frame with the reference. The cut script (`~/work/film-out/draconia/cut.sh`,
  `OPEN=…`) swaps in a later approved version the same way.

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

### v2, 2026-10-13

- **Gaps fixed:**
  - The walls are weathered stone:
    - dark mortar behind the stones;
    - stones of varied size, some chipped smaller, some proud or sunk, a few
      missing;
    - grime darkening toward the ground and in streaks;
    - tones from cool grey to warm.
  - The blast is warm amber, with brighter dust and haze. 34 wall-top chunks
    fly higher. Short-range lamps light the wall's face either side of the
    silhouetted boy, green as the bolt passes and amber after the blast.
  - The duel is busier: nine students, runners crossing, someone
    backpedaling, someone strafing, and two bolts (green and red), each with
    a shield shimmer and its own light.
- **Still open:**
  - shot 1's face (waiting on the face work);
  - uniformed students;
  - the blast frame goes dark on the right once the burst passes;
  - the cloak wipe is a recolored ranger, not a cloth cloak.

### v3, 2026-10-13

- **Gaps fixed:**
  - The blast frame stays lit after the burst: amber lamps keep the wall's
    face either side of the boy glowing.
  - The cloak wipe is cloth: a hooded drape of dark wool with deep,
    swinging folds and a flaring hem, drawn as lit geometry crossing the
    lens.
  - The students are in uniform. Green cloth recolors to black wool robes,
    and leathers to charcoal; skin, linen and red trim are kept.
- **Still open:**
  - shot 1's face;
  - real uniforms with ties and sweaters on a student base;
  - the boy's shot still reads as a silhouette.

## Scene: D2 (shots 7–15, reference 9.7–26.3 s)

The same `courtyard` scene kind, with the shots `profile`, `haul`, `runner`,
`shout`, `eye`, `alone`, `wave` and `wide`
(`docs/verse/film/verse-shots/draconia-d2-*.json`). They're cut by
`~/work/film-out/draconia/d2-cut.sh` into `~/Downloads/draconia-d2-vN.mp4`,
`-vN-side-by-side.mp4` and `~/Downloads/draconia-d2-progress.png`. Same rule
as D1: no stand-in face in a close-up or medium shot.

### v1, 2026-10-13

- **Built:**
  - Shot 7, the profile: a slated plate, with embers near the lens and the
    duel as a deep blur behind.
  - Shot 8, the haul: a boy hauling a girl away from us, seen from behind,
    as a green bolt cuts across, with fire in the foreground.
  - Shot 9, the runner: a boy runs at us past fires that backlight him, and
    pulls up short.
  - Shots 10 and 11–12, the shout and the eye: slated fire plates.
  - Shot 13: Draconia alone in the ruins, low and wide, with fires round
    her.
  - Shot 14: a dark wave rolling over the cobbles toward us. This is the new
    `shadow_wave` effect: dark smoke boiling forward, pale mist skimming
    under it, and black shreds thrown ahead.
  - Shot 15: the whole courtyard at night, with fires, the duel and its
    bolts, and a hooded cloak filling the right foreground.
- **Gaps:**
  - The close-ups (7, 10, 11) wait on faces.
  - Shot 13: she's small in frame and her hair doesn't stream.
  - The wave lacks the reference's tall rolling darkness.
  - Shot 15 lacks the reference's grand Gothic courtyard facades and wider
    scatter of groups.

### D2 v2 (and D1 v4), 2026-10-13

- **Gaps fixed:**
  - The courtyard has Gothic ranges:
    - one across the far side and a wing down each flank;
    - bays of tall pointed windows between buttresses with pinnacles;
    - crenellated parapets and a square tower.
    Shots 13, 14 and 15 sit inside them under a moonlit key.
  - Shot 13: Draconia stands nearer and larger, backlit by the moon, so her
    stand-in face doesn't read.
  - Shot 14's wave is taller and darker.
  - Shot 15: a hooded figure fills the right foreground. The cloak has a
    rounded hood and goes near-black (D1's cloak wipe gets the hood too, as
    D1 v4).
- **Still open:**
  - the close-ups' faces;
  - the reference's warm firelit facade faces and lit windows;
  - Draconia's streaming hair (waiting on the face work's joint chains).

### D2 v3, 2026-10-13

- **Gaps fixed:**
  - Fires along the far range's foot light its face warm.
  - Firelight burns in some of its pointed windows: tall, flickering warm
    glows.
- **Still open:** the close-ups' faces; Draconia's streaming hair.

## Scene: D3 (the night sky, reference 26–56 s)

The `night` scene kind (`crates/verse/examples/film/night.rs`, shots
`plate`, `vortex`, `sky`, `behind`, `boots` in
`docs/verse/film/verse-shots/draconia-d3-*.json`). It's cut by
`~/work/film-out/draconia/d3-cut.sh` into `~/Downloads/draconia-d3-vN.mp4`,
`-vN-side-by-side.mp4` and `~/Downloads/draconia-d3-progress.png`, 30 s at
the reference's times. Same rule: no stand-in face in a close-up or medium
shot.

### v1, 2026-10-13

- **Built:**
  - hooded wraiths: tattered cloaks with hoods and no faces, rags stirring;
  - a vortex of dark cloud and circling wraiths round the moon, seen
    looking up from the courtyard with Draconia tiny below in a ring of
    fires (31–34 s);
  - the black beat (34–35 s);
  - wraiths crossing the moonlit sky (35–37 s);
  - Draconia from behind, her platinum hair down her back, looking up at the
    sky full of them (43–47 s);
  - her boots walking toward us before a wall of fire, dueling silhouettes
    and red and green bolts (51–56 s);
  - slated plates for the close-ups and the students (26–31, 37–41 and
    47–51 s), and a slate for the spider web (41–43 s, not built).
- **Gaps:**
  - the faces;
  - the spiders and their web;
  - the reference's wraiths are torn and wispy, and darker against the
    moon;
  - the duelers behind her boots are wraith shapes, not students.

### D3 v2, 2026-10-13

- **Gaps fixed:**
  - The wraiths are torn and wispy. The cloak hangs in separate rags below
    the waist, with sky between, and the rags trail longer. Dark smoke wisps
    drift from every hem. The cloth is near-black.
  - The stand-off: in the 43–47 s shot, a field of forty wraiths hangs low
    over the far courtyard in ranks, facing her, fading into the dark, with
    more in the sky above.
- **Still open:**
  - the faces;
  - the spiders;
  - the reference's wraiths fly in looser, more dynamic poses;
  - the field could press closer as the shot goes on.

### D3 v3, 2026-10-13

- **Gaps fixed:**
  - The wraiths swoop and bank, each on its own rhythm: a forward lean that
    breathes and a rolling bank, so a crowd never hangs in one pose.
  - The stand-off field sinks and presses about 5 m closer to her over the
    shot.
  - The spiders (41–43 s), built where the slate was:
    - a great orb web across the lens, with radial threads and a capture
      spiral broken in places, silvered by the moon and trembling;
    - three giant spiders behind it, each with a dark abdomen and head and
      eight two-segment legs stepping in turn;
    - a green bolt, then a red one, striking among them and lighting them;
    - fires behind.
- **Still open:**
  - the faces;
  - the spiders are smooth and glossy, not hairy;
  - the garden's ruined arches behind them;
  - switching every Draconia shot to the face work's skinned build: under
    way, starting with the peacock tail shot.

### Peacock v14, 2026-10-13

- **The switch:** P3 now draws the face work's skinned Draconia
  (`~/work/film-out/draconia/build-long/draconia-long.glare.glb`). It's added
  to the pack in memory by `crates/verse/examples/film/draconia.rs` (as
  `draconia_form.rs` does). She plays her own baked `rise`: an upward gust
  0.05 s in throws her long hair up, and it settles over the shot. The shot
  file names the build in `scene.draconia`; without it the scene falls back
  to Alice recolored. The eyes close-up (peacock-open v1) stays cut in at
  114.5 s.
- **Gaps, for review against v13:**
  - Her silhouette reads boxy: the robe is a straight block, the legs look
    short, and her feet sit a little above the cobbles.
  - The gust piles her hair into a clump above her head rather than the
    reference's long streaming fan.
  - Both come from the asset and its baked clip (the face work's), not the
    scene.

### Peacock v15, 2026-10-13

- **Reverted** to Alice recolored, the same as v13 (PSNR about 52 dB
  against v13). In review, v14's skinned build read as a short, boxy blob
  with no legs or skirt line. The loader (`film/draconia.rs`, the shot
  file's `scene.draconia`) stays for later.
- **The rule from here:** the other Draconia shots stay on the stand-in. When
  this file lists the MetaHuman Draconia, it's tested in one shot first and
  compared with the stand-in, and only switched if clearly better.

## Scene: D4 (reference 56–92 s)

The `night` scene kind's `stride`, `star` and `attack` shots, plus
`draconia-d4-stand` (the stand-off held longer). They're cut by
`~/work/film-out/draconia/d4-cut.sh` together with clips from D1–D3, at the
reference's times, into `~/Downloads/draconia-d4-vN.mp4`,
`-vN-side-by-side.mp4` and `~/Downloads/draconia-d4-progress.png`. Same
rule: no stand-in face, hands or hair in close-up.

### v1, 2026-10-13

- **Built (35.9 s):**
  - 56–57 s: students fleeing past the fire.
  - 57–58 s: a blast.
  - 58–60 s: Draconia striding away from us through the burning courtyard as
    a green bolt cuts behind her.
  - 60–67 s: slated plates for the crowd's faces, the hand on the rubble,
    and the girl's close-up.
  - 67–70 s: a blue-white star flaring with long cross spikes in a cloudy
    sky, sparks drifting down.
  - 70–73 s: a wraith reaching for a student by the fires, others fallen.
  - 73–78 s: the stand-off from behind.
  - 78–80 s: her boots.
  - 80–92 s: slated plates for the hair, wand and face close-ups.
- **Gaps:**
  - About 19 s of the 36 are slated close-ups (faces, hands, hair, the wand)
    waiting on the cast.
  - The fallen students are dark heaps, not bodies.
  - The 56 s shot runs with students, not with Draconia.

## Scenes: D5 (reference 92–113 s) and D6 (118–150 s)

D5 leads into the peacock scene and D6 follows it. They're cut by
`~/work/film-out/draconia/d5-cut.sh` and `d6-cut.sh` (sharing
`seg.sh`) into `~/Downloads/draconia-d5-vN.mp4` and `draconia-d6-vN.mp4`,
each with a side-by-side and a progress sheet. They use new shots
(`draconia-d5-*`, `draconia-d6-*`), and the night scene gains `hood`,
`frost` and `recoil`. The peacock scene gains `wings` (the guardian alone,
wings of light spread and beating), and a tail shot without a rise skips the
opening flash. Same rule: no stand-in face, hands or hair in close-up. The
one face close-up used is the face work's approved peacock-open v1.

### v1, 2026-10-13

- **D5 (21.0 s):**
  - 92–93 s: a wraith's hood, close and dark.
  - 93–95 s: wide under the moon, wraiths filling the sky, frost spreading
    over the courtyard from Draconia, tiny.
  - 95–101 s and 102–104 s: slated plates (the boy, her face, the hand and
    wand).
  - 101–102 s: the stand-off from behind.
  - 104–105 s: her silhouette rising in the guardian's light.
  - 105–107 s: the approved eyes close-up.
  - 107–109 s: the white flash.
  - 109–113 s: feathers of light swirling.
- **D6 (32.0 s):**
  - 118–119 s: three hooded wraiths recoiling.
  - 119–122 s: the guardian alone with wings of light.
  - 122–124 s: feathers.
  - 124–126 s: low on her legs, the tail of light behind.
  - 126–130 s: slated plate (her profile).
  - 130–131 s: close on the tail's eyes.
  - 131–133 s: a wraith reaching.
  - 133–134 s: the frosted courtyard.
  - 134–142 s: slated plates (hair, the flashback girls).
  - 142–144 s: the Gothic courtyard.
  - 144–146 s: wraiths fleeing across the sky.
  - 146–148 s: a wraith.
  - 148–150 s: her silhouette in the white light.
- **Gaps:**
  - About a third of each is slated close-ups and the flashback cast.
  - The wraith close-ups are smooth hoods, not the reference's tattered,
    skeletal hands and fanged face.
  - The guardian's wings don't separate from the tail.
  - Her legs show Alice's bare skin rather than tights and boots.
  - The light wave over the courtyard (142 s) is a plain wide shot.

## Scene: D7 (reference 150–192 s) and the whole film

`~/work/film-out/draconia/d7-cut.sh` cuts D7 from earlier shots, slates,
a white-out and two cards of our own. `~/work/film-out/draconia/film-cut.sh`
assembles the latest cut of every scene (D1–D5, the peacock scene, D6, D7)
in reference order into `~/Downloads/draconia-film-vN.mp4` (191.9 s) and a
side-by-side over the full reference with its audio (192 s). Internal study
only.

### v1, 2026-10-13

- **D7 (42.0 s):**
  - her silhouette in the guardian's light;
  - the wings;
  - wraiths over the battlements;
  - feathers;
  - the white-out with her tiny figure (170–177 s);
  - slates for the flashback cast (the ball, the umbrella, the laughing pair,
    the sunlit room) and her face and hand;
  - a plain parchment card where the reference names a character;
  - our own end card ("Draconia, a Verse engine study, internal, not for
    release") in place of the reference's credits.
- **Gaps:**
  - The flashbacks are a sunlit world with their own cast (the boy, the
    friends, the ball). None of it is built; it waits on the cast.
  - The tiny figure in the white-out is not drawn (plain white).
