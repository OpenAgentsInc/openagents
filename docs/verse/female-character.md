# Female character: an original Verse explorer

This page specifies an original female player character for Verse, built in
Blender by script and played on the Universal rig. It covers the design
brief, proportions, budgets, skeleton, skinning, face, hair, gear, materials,
animation, export, selection, tests, and a phased plan.

Status: the first build, Alice, exists as a placed character (an NPC), October
5, 2026. The rest of this page is the original specification; where it says
"player", read it as the later player character described below.

## Alice, the first build

The owner named her Alice and decided that she is **an NPC, not a playable
character**: players can't select her, and the owner will specify her role
later. A female player character is a later, separate build that reuses
Alice's base body and head with an outfit of its own.

Defaults taken for the owner's open questions, which the owner can change:

| Question | Default |
| --- | --- |
| Role | Explorer-druid (option C) |
| Palette | Forest green, warm brown leather, and cream linen, with copper accents |
| Hair | Auburn, long and softly wavy past the shoulder blades, with a center part and locks that frame the face (the owner's choice, October 6, 2026) |
| Clips | The Everglade pack's eight (idle, walk, run, jump, backpedal, two strafes, and the swing); raising `MAX_CLIPS` is a later phase |

What was built:

- **Script.** `scripts/blender/alice.py` builds her in three reusable parts:
  `build_body` (the complete base body in skin: torso, arms, hands, and
  legs, lofted on the Universal rig with scripted weights), the head
  (`ubc_head` and `build_head`: the reshaped head, its painted face and
  eyes, and the hair), and `build_outfit` (one of the outfits, as layers
  over the body). `scripts/blender/alice_paint.py` paints her face, eyes,
  and hair strips. It writes `alice.<variant>.glb` (the coat) and, for
  `lod1`, `alice-light.lod1.glb` and `alice-summer.lod1.glb` to
  `assets/verse/characters/original/alice/build/`.
- **Base body.** A complete body that reads well with little clothing: a
  waist that narrows to about 60 cm around, hips and glutes about 93 cm
  around with a real curve in profile, a round bust with a crease under
  it, collarbones, shoulder blades, the small of the back, thighs that
  taper to slim knees, calves with a swell behind, and slim ankles and
  wrists; hands with fingers and painted nails. Its curves come from the
  body study ([Lessons from a body reference](#lessons-from-a-body-reference)).
  `--outfit base` builds it in a plain bandeau and briefs, for review only.
- **Outfits.** Each garment is a layer over the body (or over the garment
  under it): a surface offset from the one beneath with a tension envelope,
  so cloth bridges the hollow under the bust and between the breasts where
  real cloth would, and follows the waist where it's tailored. An outfit
  hides the skin under its opaque garments, which costs no triangles. Three
  outfits:
  - `coat` (the default): a cream linen shirt and close-fitting trousers
    under a tailored forest-green coat that follows the bust, waist, and
    hips and flares from the hips to the knee, with the sash cinching it,
    the satchel, its strap, bracers, and knee boots;
  - `light`: the coat off, a V-necked shirt with its sleeves rolled to the
    elbow, tucked into the trousers, and the sash cinching the waist;
  - `summer`: a fitted sage-linen dress with a straight neckline, narrow
    straps, and a skirt that flares to the knee, the sash, and ankle
    boots.

  `lod1` builds all three on one atlas (the wardrobe), so the pack's
  `npc/alice-light` and `npc/alice-summer` forms add geometry but no
  texture. The bake shades each outfit on its own, so one outfit's
  garments don't darken another's skin. The owner picks one with `verse
  --alice-outfit coat|light|summer` or `VERSE_ALICE_OUTFIT`.
- **Head.** The CC0 Universal Base Characters female head (Compose mode),
  cut from `Superhero_Female_FullBody.gltf` and reshaped by `reshape`: a
  slightly larger head for our stylized proportions, a softer and narrower
  jaw, a rounder chin, a shorter and slightly upturned nose, fuller lips and
  cheeks, a softer brow ridge, eyes 2.2 mm closer to the nose and 4 percent
  larger than the base's, and ears removed under the hair. It keeps the
  base's real structure: edge loops around the eyes and mouth, separate
  eyeballs, eyelids, a nose, and upper and lower lips. The inside of the
  mouth, which the closed lips hide from every side, is removed.
- **Painted face.** `alice_paint.face` paints her face at 2048 pixels in a
  front projection over 21 cm, measured from her own head: the eye openings
  (where, seen from in front, an eyeball is nearer than the skin), the
  mouth's line, and the nose's tip. It paints skin that varies in warmth;
  warm cheeks, nose, and chin; shadowed sides of the nose with a light
  bridge, soft nostrils, and creases around its wings; a warm crease in each
  eye socket; an upper lash line that thickens toward the outer corner and
  flicks up and out; a fainter lower lash line; brows of about 340 painted
  strands each, rising at the head and lying along the arch; and lips with a
  Cupid's bow, a darker upper lip, a fuller lighter lower lip with a
  highlight, and a dark line between them. `alice_paint.eyes` paints the
  eyes in the same projection: a sclera no brighter than her skin, pinker
  toward the corners and shaded under the upper lid; a hazel-green iris
  (0.205 of the opening's width) with radial fibers, a dark limbal ring, and
  a pupil; and a catchlight up and to her left in both eyes. The face takes
  6.5 times the atlas's texel density, so `lod1`'s face is about 300 pixels
  across.
- **Hair.** Long, softly wavy auburn hair with a center part, after the
  owner's long-hair reference (see [Lessons from a long-hair
  reference](#lessons-from-a-long-hair-reference)), built as original
  geometry: an opaque scalp cap and back sheet, dark at the roots, close to
  the skin at the hairline, and over it alpha-tested hair cards (`lod0`
  has 90, `lod1` 72). Each card leaves the center part (or, at the back,
  the crown), lifts off the crown for volume, settles onto the head at the
  temples, and falls in S-waves whose period grows from 7.5 to 12.5 cm and
  whose amplitude grows from 3 to 16 mm toward the tips. The cards nearest
  the face become locks that frame it and fall in front of the shoulders to
  below the bust; the rest fall behind the shoulders to the shoulder blades,
  longest at the middle of the back. An under layer of wider, darker cards
  fills the volume, and a few thin wisps break the outline; the hair falls
  behind the shoulders onto the back, in a line that leans out to meet the
  shoulder blades rather than kinking over them. Every card samples one of
  eight painted strips (`alice_paint.hair_strips`) in the atlas's bottom
  band, so the cards share their texels: soft strands, a gentle shadow
  toward each clump's edges, dark roots, a glossy band near the crown and
  a fainter one lower, lighter ends, and tips cut into strands of
  different lengths. The strips differ in shade, so neighboring locks read
  apart, and the color is a natural auburn: brown in the shade, copper in
  the light.
- **Clothing detail.** The bake paints what a base-color-only renderer can
  show: a crossed weave in the linen, fuzz and long folds in the wool coat
  with princess seams and a row of stitches beside each, grain, creases,
  and worn, lighter edges on the leather, fold streaks in the sash,
  polished edges on the copper, and warmth at the knuckles and fingertips.
  Cycles' pointiness finds the worn edges.
- **Weights.** Generated parts take their chain's bones with smoothstep
  blends around each joint; the head keeps the base's weights. At most four
  influences a vertex, normalized, rounded to 255ths at admission.
- **Atlas.** One baked base-color atlas per variant: diffuse color times
  ambient occlusion and a soft top light, at seven bits a channel. Its
  alpha is opaque except in the hair strips' band (the bottom 16 percent).
- **Admission.** `scripts/blender/character_admit.py` writes each variant's
  `.gltf`, `.bin`, and `.png` with the base rig's own joint nodes and inverse
  binds, checks the budgets and weights, and writes `manifest.json`
  (`openagents.verse.character-sources.v1`). Each variant has two
  materials on the one atlas: `alice`, opaque, and `alice_hair`,
  alpha-masked at 0.5 and double-sided, for the cards. `crates/verse-content`'s
  `characters::alice` imports a variant and gives her every Universal clip.
- **Placement.** The Everglade pack carries `lod1` as the form `npc/alice`
  (the format needs no new section). She is the workshop agent's body
  ([Workshop agent](workshop-agent.md)): the studio draws the seat whose
  look is `alice` as the form of the outfit she wears (`everglade::npcs::form_of`), so she stands
  at her workstation, a standing desk, in the owner's house (she never
  sits), walks to its console and its lectern with the player's retargeted
  walk, and holds the studio's postures, which the zone authors from her
  idle. She no longer stands by the approach.
- **High definition.** The near levels are built coarse and subdivided
  (`lod0`'s head too), then shaded smooth with weighted normals, keeping
  creases sharper than 60 degrees. The cards are never subdivided. Her
  nostrils close to a soft underside and the face bakes with almost no
  occlusion, so no dark wedges show in the eyes, nose, or mouth.
- **Stance and motion.** Her idle rests her weight on her left leg: the
  hips tilt 0.07 rad and shift over that foot, the spine and shoulders
  tilt back the other way, and the right knee relaxes, an S-curve rather
  than a stiff stance (`characters::contrapposto`; a test checks it).
  The Universal clips hang a slimmer figure's arms, so `characters::alice`
  turns her upper arms out by `ALICE_ARMS_OUT` (0.17 rad) in every clip,
  and her hands hang clear of the coat standing and walking; a test checks
  it. The skirt's front and back follow the thighs, so a stride doesn't
  push a knee through it; the deformation poses in `alice_views.py` (walk,
  jog, sprint, crouch, cast) show no collapse at the hips or the bust.
- **Boots.** Shaped to the leg: a fitted shaft, a folded cuff whose edge
  turns back in to the leg, a narrow ankle, a low heel, and a toe that
  rises; knee boots with the coat and the light outfit, ankle boots with
  the dress.

| Variant | Triangles | Budget | Atlas |
| --- | ---: | ---: | --- |
| `lod0` (chamber, close views; the coat) | 80,646 | 100,000 | 2048 |
| `lod1` (Everglade: coat, light, summer) | 37,712, 31,772, 29,304 | 46,000 | 1024, shared |
| `lod2` (phones, once packs split by tier) | 6,870 | 10,000 | 256 |
| `lod3` (distant, once skinned levels of detail exist) | 2,900 | 3,000 | 256 |

`Limits::EVERGLADE` allows a 48,000-triangle character for her near level
and 590,000 triangles in the pack. The glb files `alice.py` writes go to
`build/`, which isn't committed; the admitted glTF files are. To rebuild and
review her:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
for v in lod0 lod1 lod2 lod3; do $B -b --factory-startup --python scripts/blender/alice.py -- $v; done
python3 scripts/blender/character_admit.py
$B -b --factory-startup --python scripts/blender/alice_views.py -- assets/verse/characters/original/alice/build/alice.lod0.glb OUT_DIR
```

`alice_views.py` renders the turnaround, a turntable, face close-ups,
silhouettes beside the Ranger, and the Universal clips that stress each
joint.

### Lessons from Echo's face

The first face, a lofted ellipsoid with painted decals, read as a flat
mask. Echo's asset names (Valley of the Ancient, studied in Reference mode
only) show how a game face is assembled, and Alice now follows the same
construction with none of Echo's likeness:

- separate eyeballs (`M_EyeL`, `M_EyeR`, `FACIAL_L_Eye`) set into sockets
  under a brow ridge, with lids (`FACIAL_L_EyelidUpper`, `_Lower`) rather
  than eyes painted on skin;
- the eye's layers: an occlusion shell (`M_EyeBlend`), a wet tear line
  (`M_EyeWet`, `M_Lacrimal`), and lashes (`M_EyeLashes`), which at our scale
  become the baked shadow in the sockets and the lids' dark lash line;
- a jaw and teeth as their own parts (`FACIAL_C_Jaw`, `M_Teeth`), with lips
  that close over them in an upper and a lower volume;
- brows as their own layer (`MI_EchoGroom_eyebrow`);
- proportions: the eye line at half the head's height, eyes about one eye
  width apart, a defined nose bridge and tip, a philtrum, and a chin and jaw
  that are planes, not a cone.

A second pass measured Echo's LOD0 head itself. The installed engine
exported her geometry headless (Geometry Script, to OBJ) and her textures,
to a local study folder that is never committed; Blender rendered study
views of the bare head. Nothing from it is copied into Alice: no geometry,
UVs, texture, or likeness. What it teaches:

| Measure | Echo's head |
| --- | --- |
| Head, including the neck's top | About 23,800 triangles; each eyeball 1,536; teeth 4,348 |
| Eye line | 0.112 m below the crown, at half the crown-to-chin height |
| Eye centers | 0.065 m apart, with the gap between the eyeballs about 0.75 of an eye's width |
| Nose tip | 0.036 m below the eye line, 0.029 m in front of the corneas |
| Mouth | 0.063 m below the eye line, close under the nose |
| Eye assembly | Six shells around each eyeball (2,300 triangles together): lashes, an occlusion blend, a wet tear line, and an overlay |
| Brows | Strand grooms with a brow mask in the head texture, not mesh |

The construction lessons Alice takes:

- **Eyes read through their frame, not their size.** The lash shells give a
  dark upper-lid band and the occlusion shell darkens the socket's rim, so
  the white and the iris stand out. At our scale, Alice's lash line is baked
  into the head texture as a dark band with a flick at the outer corner,
  and her eye texture gets more contrast and whiter whites.
- **Large, open, almond eyes about one eye width apart**, with the outer
  corners a little higher than the inner, set under a soft brow ridge. Alice's
  eye openings are 16 percent larger than the base head's, and her eyes turn
  up a little so her gaze meets the viewer's.
- **Small, soft features low on the face.** A narrow nose bridge, a rounded
  tip with soft nostril wings, and a mouth close under it, with a full upper
  and lower lip and a defined philtrum. Alice's nose sits back in profile
  (less beak), and her mouth's corners turn up.
- **An oval face from full cheeks and a soft jaw**, a small rounded chin,
  and a slim neck. Alice's cheeks are fuller and higher, her jaw narrows
  less than before, and her neck is slimmer.
- **Painted warmth carries the face at a distance**: warm cheeks and nose,
  rose lips, and darker brow strokes. Alice's bake adds warmth on her cheeks,
  nose, and lips, and her brows are fuller and darker.

### Lessons from a painted reference

On October 6, 2026, the owner gave a higher-fidelity reference: a rigged,
animated fantasy woman from Fab (AI-generated, standard license), one fused
94,000-triangle mesh with one 4096-pixel base-color texture on generated
UVs and a 65-bone Mixamo rig. It was studied only: rendered and measured in
Blender headless, with its renders kept outside the repository. Nothing
from it is in Alice or in the repository: no geometry, UVs, texture pixels,
or likeness. Its license forbids redistribution, and this repository is
open source. What it teaches, as shares of its height (H) and, in
parentheses, at Alice's 1.767 m:

| Measure | The reference |
| --- | --- |
| Head, crown to chin | 0.136 H (24 cm), about 7.4 heads |
| Shoulder joints | 0.198 H apart (35 cm) |
| Waist and hips | 0.146 H (26 cm) and 0.213 H (38 cm) wide; waist to hip 0.69 |
| Bust | Its profile 0.166 H deep against 0.131 H at the waist, so it stands 0.035 H (6 cm) past the underbust, with its fullest point at 0.72 H |
| Legs and hands | Crotch at about 0.47 H, knees at 0.30 H; a hand 0.108 H long |
| Eyes | Centers 0.038 H (6.7 cm) apart, 0.52 of the face's width at the cheekbones (12.8 cm); each opening 3.0 cm wide and about 0.4 as tall as wide, almond-shaped; 1.3 eye widths between them |
| Lower face | Eye line to the nose's base 4.5 cm; the nose's base to the line of the lips 2.5 cm; that line to the chin 4.0 cm; the lips 5.6 cm wide, 0.44 of the face |
| Brows | About 1.6 cm above the eye's center, thin, arched, darker at the head |

Its base-color texture carries the shading a base-color-only renderer
needs. In its flat color (value and saturation in HSV):

- Skin isn't one tone: the forehead is at value 0.76 and saturation 0.15,
  and the cheeks at 0.24 saturation and a redder hue (16 degrees against
  21).
- The eye socket is painted dark and warm: the upper lid's crease is at
  value 0.43 and saturation 0.49, 45 percent darker than the forehead, and
  under the eye is at 0.67. The lash line is near black (0.13).
- The sides of the nose are 28 percent darker than its tip, which is what
  shapes it from the front.
- The lips are as bright as the skin (0.68 to 0.71) but far more saturated
  (0.43 to 0.50, a rose hue), with a dark line between them (0.37).
- The sclera is no brighter than the skin (0.81), so the eyes read as part
  of the face rather than as two lit discs.
- The hair's surface is at median value 0.6; 14 to 21 percent of it is dark
  gaps between strands, about half the median, every 2 to 6 mm, with the
  part darkest.
- The clothing's detail is in the color, with no normal map: leather at
  median luminance 0.20 with soft long crease streaks; seams as thin light
  lines of piping that lift the 95th percentile to 0.41 on a 0.16 median;
  boots and hardware with painted highlights on their edges.

What Alice took, as her own scripted build:

- Her eyes were 3.8 by 1.8 cm, round, 8.2 cm apart, with a sclera far
  brighter than her skin and a glint disc on each. Now each eye moves
  2.2 mm toward the nose, the openings are 4 percent larger than the
  base's instead of 16, the sclera is painted no brighter than her skin and
  shaded under the upper lid, and the catchlight is painted.
- Her brow meshes were blocks; her face's warmth was three vertex-color
  blobs. Now `alice_paint.face` paints the socket's crease, the lash lines,
  the nose's planes, the lips, and the brows as strands, at 2048 pixels.
- Her jaw narrows 6.5 percent toward the chin (from 4), and her bust is
  rounder and stands a little farther forward (5.6 cm at its fullest).
- Her clothing's procedural shading gained seams, stitching, weave, folds,
  creases, and worn edges.

### Lessons from a body reference

For her body, the owner gave a third Fab model, a stylized young woman in a
short dress (AI-generated, standard license; one fused mesh, no rig),
under the same rule: studied only, measured from orthographic silhouettes
in Blender headless, with nothing of it in the repository. Her arms hang
in an A pose, so widths are read where they stand clear of her sides. As
shares of her height (H) and in centimeters at Alice's 1.767 m, beside
Alice before (her old tunic and coat had no body under them) and after
(the base body):

| Measure | The reference | Alice before | Alice after |
| --- | --- | --- | --- |
| Height in heads | 6.9 | 7.2 (crown to chin 24.5 cm) | 7.2 |
| Shoulders, deltoid to deltoid | 0.21 H (37 cm) | hidden by the coat | 0.21 H (37 cm) |
| Bust | 0.125 H deep (22 cm), standing 3.5 cm past the underbust | 30 cm deep in the coat | 24 cm deep, standing about 5 cm past the underbust in skin |
| Waist | 20 cm wide, 17.5 cm deep, about 59 cm around | 27.6 cm wide in the tunic | 22 cm wide, 16 cm deep, about 60 cm around |
| Hips and glutes | about 34.5 cm wide, glutes 24 cm deep and 5 cm past the small of the back, about 92 cm around | the coat's 42.6 cm flare | 35.4 cm wide, 22.9 cm deep, about 93 cm around |
| Waist to hips, around | 0.64 | 0.69 (clothing) | 0.65 |
| Bust to waist, around | about 1.25 | 1.28 (clothing) | 1.34 |
| One leg, top of the thigh, above the knee, knee, calf, ankle | 14.2, 10.8, 9.6, 9.6, 6.1 cm | 20 to 22 cm through the trousers | 13.7, 10.3, 9.4, 9.4, 5.5 cm |
| Crotch height | about 0.45 H | about 0.47 H | about 0.47 H |

What Alice took:

- **A body, not a costume.** Her old torso was the tunic itself, so a
  lighter outfit had nothing under it. She now has a complete skin body,
  and every outfit is a layer over it.
- **Curves that read in silhouette:** the waist narrows to about 0.65 of
  the hips around, and the hips flow into the thighs without a step (the
  thighs' tops sit inside the hips). In profile the bust and the glutes
  both stand clear of the waist, and the small of the back curves in.
- **Tailoring follows the body.** Her coat is cut from the body's own
  sections, tight through the waist and flaring from the hips; the shirt
  and the dress bridge only the hollows real cloth bridges.
- **Weight on one leg.** The reference stands at ease; Alice's idle now
  does too, with her hips tilted over her standing leg.

### Lessons from a long-hair reference

The owner then chose a hairstyle from a second Fab model (a stylized
long-haired woman, AI-generated, standard license; one fused
225,000-triangle mesh with no separate hair and no rig), with the same
rule: studied only, and Alice's hair is original scripted geometry. As
shares of its head's height (crown to chin):

| Measure | The reference |
| --- | --- |
| Length | At the back, 2.15 head heights below the crown (the shoulder blades); the front locks 2.4 (below the bust) |
| Width from the front | 0.66 at the crown, 0.81 at the temples, 1.0 to 1.1 at the jaw, 1.5 to 1.6 where it spreads over the shoulders |
| Depth from the side | 0.91 at the crown, with volume behind it |
| Waves | Soft S-waves with a period of about 0.29 head heights by the face, loosening to about 0.45 at the ends; amplitude about 0.03, growing to 0.06 at the ends |
| Clumps | About 12 major clumps across the back at 40 percent of the length |
| Part and hairline | A center part from the hairline to the crown; a rounded hairline |
| Framing | Locks leave the part, sweep over the temples, frame the face from the cheekbones down, and fall in front of the shoulders onto the chest |
| Color | Dark brown at median luminance 0.14 to 0.16 from root to tip, varying about 12 percent between strands; the part's roots darkest, at about half the mid tone |

Alice's hair follows that shape in her own auburn: a center part, volume at
the crown, face-framing locks over the chest, and loosening S-waves to the
shoulder blades, as cards with tapered, strand-cut tips over a dark cap.

## Mode and the Echo rule

The character is a **Reference**-mode model under the
[asset runbook](asset-runbook.md): original geometry and original textures,
made by a script under `scripts/blender/`.

Epic's Echo, from [Valley of the Ancient](valley-of-the-ancient-index.md#echos-folder),
informs only general qualities:

- an athletic explorer's proportions;
- a silhouette that reads at a distance;
- practical, layered field gear;
- a locomotion set that feels expressive because of starts, stops, and
  ground contact, not because of its clip count.

Nothing else carries over. Don't use her face, hair, costume design,
colors, likeness, or any of Epic's files, and don't open Valley content in
Blender, trace it, or feed it to a model ([licensing](ue5-ruins-study.md#licensing)).
The character must read as our own, not as a lookalike. These are Echo's
signature features, known from her asset names; the design avoids every one:

| Echo has | Ours doesn't |
| --- | --- |
| Blonde hair in an updo with buns, braids, and a ponytail | Any of those hairstyles, or blonde as the main hair color |
| A long scarf | A scarf |
| A skirt over trousers | A skirt layer over leggings |
| A single shoulder pad | A lone shoulder pad |
| A canteen and gold buckles | A canteen, or gold hardware as an accent |
| A realistic face with 181 facial shapes | A realistic face; ours is stylized |

The Quaternius female peasant also wears buns (`Hair_Buns`), so avoiding
them keeps her distinct from our own existing characters too.

## Design brief

**Who she is.** A wayfarer of Verse: someone who walks between the zones,
reads ruins, and works with the living world. She suits Everglade's town and
forests, the Grove, and the planned ruins passage. The working ID is
`wayfarer`; the owner names her.

**Style.** She sits in the Quaternius-adjacent stylized look of our world:
smooth low-polygon forms, flat colors with soft baked shading, slightly large
hands, feet, and head for readability, and no realistic skin detail. She
should stand beside the Universal Ranger without looking like another
game's character.

Three role options for the owner:

| Option | Identity | Silhouette anchors | Default gear |
| --- | --- | --- | --- |
| A. Explorer | A ruin scout and mapmaker | Short hooded cowl, cross-body map case, wrapped forearms, tall boots | Walking staff or short bow |
| B. Druid | A Grove keeper | Long open coat with a leaf-cut hem, sash, a circlet of branches | Gnarled staff |
| C. Explorer-druid (recommended) | A field druid who travels | Hooded cowl, knee-length open coat over a fitted tunic and leggings, sash, satchel | Staff that doubles as a walking stick |

Option C fits both spellcasting (cast clips, levitation) and traversal, and
its coat gives motion without cloth physics.

**Palette.** Moss green, bark brown, and undyed linen, with one accent in a
player color through a tint slot (as the studio seats do). Hair is a dark
color (chestnut, black, or deep auburn). No gold.

## Proportions and silhouette

Her joint positions come from the Universal female rig (see
[Skeleton](#skeleton)), so her limb lengths and height are fixed by it:
about 1.75 m to the crown. Within that frame:

| Measure | Target |
| --- | --- |
| Height to crown, without hood | 1.74 to 1.77 m (the Universal female body reaches 1.767 m) |
| Head height | About 0.25 m: seven heads tall, a little larger than realistic |
| Shoulder width | 0.40 m |
| Hip width | 0.35 m |
| Hands and feet | 10 to 15 percent larger than realistic |
| Stance | Athletic: straight back, weight forward, no exaggerated curves |

Silhouette rules, checked at 64 px and 24 px tall:

- **One strong head shape.** The hood up, or chin-length hair with a
  headband when the hood is down.
- **One asymmetric element.** A cross-body satchel strap and a satchel on
  one hip, so her facing reads from behind.
- **Three value bands.** Dark boots and bracers, mid coat, light tunic and
  sash, so the figure doesn't merge into foliage.
- **Motion carriers.** The coat's tails and the sash ends, skinned to the
  thighs and pelvis, swing with her gait without simulation.

## Topology and triangle budget

The engine has no skinned levels of detail today. Each tier gets its own
variant, chosen when the pack or scene is built:

| Variant | Where | Triangles | Notes |
| --- | --- | --- | --- |
| `lod0` | Chamber, desktop (High) | 24,000 or fewer | The Ranger composition is about 27,000 plus a head |
| `lod1` | Everglade pack: desktop, web, and phone share it, and it's skinned on the CPU | 16,000 or fewer | `Limits::EVERGLADE.character_triangles` is 40,000; leave room for a second player model |
| `lod2` | Phone (Medium) once packs split by tier | 10,000 or fewer | |
| `lod3` | Distant remote players, once skinned levels of detail exist | 3,000 or fewer | Planned; needs engine work |

Where `lod0`'s triangles go:

| Part | Triangles |
| --- | --- |
| Head and face | 3,000 |
| Hair or hood | 2,500 |
| Visible body (neck, forearms, hands) | 3,500 |
| Hands (two, with three-joint fingers) | 2,400 |
| Legs and boots | 3,000 |
| Clothing layers | 6,000 |
| Gear (satchel, straps, staff) | 3,000 |
| **Total** | **23,400** |

Topology rules:

- Quads in the source, triangulated at export; edge loops at every joint
  that bends: three loops at elbows and knees, two at wrists and ankles,
  a fan at the shoulders and hips.
- Remove body faces that clothing covers, per outfit, as the Quaternius
  outfit pack does.
- Each mesh primitive holds at most 65,536 vertices, and a model at most
  16 primitives (`MAX_PRIMITIVES` in the Everglade pack format).
- The lower variants come from the same script: fewer ring segments, fewer
  loops, merged gear, and no finger separation in `lod2` and `lod3`.

## Skeleton

She uses the Universal rig's 65 joints with the same names, parents, and
rest transforms, so the Universal Animation Library and our gaits play on
her unchanged. The joint list lives in `skins[0].joints` of
`assets/verse/characters/quaternius/base/Superhero_Female_FullBody.gltf`
(CC0):

- `root`, `pelvis`, `spine_01` to `spine_03`, `neck_01`, `Head`;
- per side: `clavicle`, `upperarm`, `lowerarm`, `hand`, five fingers with
  `_01` to `_03` and an `_04_leaf`, `thigh`, `calf`, `foot`, `ball`,
  `ball_leaf`.

Why the rest transforms must match: `retarget_clip` in
`crates/verse-content/src/compiler/characters.rs` matches channels by joint
name and corrects each rotation as `target_rest * source_rest⁻¹ * key`, and
each translation and scale by its rest delta. That's exact only when both
rigs share joint orientations. The script therefore reads the joints from the
CC0 base file and never authors its own.

Optional extra joints, all leaves the Universal clips don't animate:

| Joint | Parent | Purpose |
| --- | --- | --- |
| `jaw` | `Head` | An open mouth for a shout or a cast, when the engine can drive it |
| `eyelid_l`, `eyelid_r` | `Head` | Blinks, when a procedural layer exists |
| `prop_r`, `prop_l` | `hand_r`, `hand_l` | Sockets for a staff or bow |
| `satchel` | `pelvis` | Rigid gear |

That brings her to 71 joints, well under the pose limit
(`MAX_POSE_BONES`, 256, lower where a device's uniform space is small; see
`crates/verse-pbr/src/imported/admission.rs`) and the pack's `MAX_JOINTS`
of 256. A clip with no channel for a joint leaves it at
rest. Don't add cloth or hair dynamics joints.

## Skinning by script

`scripts/blender/kit.py` binds rigidly today (`bind` puts every vertex of a
part on one bone), and `dragon.py` blends one spine by hand. A humanoid needs
smooth weights, written by the script so they're deterministic:

1. **Rig.** Import the CC0 base glTF, keep its armature, and delete its
   meshes. After the first export, compare each joint's rest local transform
   with the source's; if Blender's bone conversion changed any beyond
   0.0001, the admission script copies the source joints' transforms into
   the exported glTF and recomputes the inverse bind matrices from them.
2. **Body.** Loft the body along the bones: a table gives, for each bone,
   rings of elliptical cross sections at fixed stations, which the script
   bridges and caps. Hands and feet are separate lofts joined at the wrist
   and ankle loops. Each vertex records which bone segment it came from and
   its position along that segment.
3. **Weights.** Each vertex takes its segment's bone at full weight,
   blending to the parent or child with a smoothstep over 20 to 30 percent of
   the segment length around each joint. Special zones: the shoulder blends
   `clavicle`, `upperarm`, and `spine_03`; the hip blends `pelvis` and
   `thigh`; the neck blends `spine_03`, `neck_01`, and `Head`.
4. **Clothing.** Transfer weights from the body to each clothing layer with
   a Data Transfer modifier (nearest face, interpolated), then apply it.
   Coat tails take `thigh` weights blended toward `pelvis`, so they swing
   but don't stretch. Rigid gear (buckles, the satchel) binds to one joint
   with `kit.bind`.
5. **Limits.** Keep at most four influences per vertex
   (`vertex_group_limit_total` with a limit of 4), normalize, and round to
   1/255, which is how the pack stores weights.
6. **Mirror.** Weight the left side and mirror to the right by name, so the
   two sides match exactly.

Check deformation before texturing: pose the rig in arms-up, a deep squat,
a sprint stride, and a cast, render each with `scripts/blender/preview.py`,
and add loops wherever an elbow, knee, or shoulder loses volume.

## Face

The engine has no morph targets: `retarget_clip` rejects morph channels,
and the skinned figure path has none. The face is therefore simple:

- **Version 1.** A stylized head with modeled brows and a nose, and eyes
  painted in the texture (dark irises, a highlight), as the Universal bodies
  do. No facial animation.
- **Version 2.** Separate low-polygon eyeballs, eyelids skinned to
  `eyelid_l` and `eyelid_r`, and `jaw` for an open mouth, driven by a small
  procedural layer in the animation system (a blink every 3 to 6 seconds, a
  jaw open on a cast or shout). This is engine work of about 2 hours.
- Expressions beyond that, such as smiles, wait for morph target support.

## Hair

- **Shape.** Alice's is long and softly wavy with a center part (see
  [Alice, the first build](#alice-the-first-build)); a hooded variant hides
  most of it. No buns, braids, or ponytail.
- **Construction.** An opaque cap and back sheet under alpha-tested cards
  that carry the strands, the waves, and the tips. Use alpha test, never
  blending, so it draws on WebGL2 without sorting.
- **Skinning.** Rigid to `Head`.
- **Physics.** None. The coat and sash carry the motion.

## Clothing and gear

Each layer is a separate mesh object named `part_<slot>_<variant>`, so an
outfit is a list of parts. The script writes one glTF per outfit with the
body faces under its parts removed (a mask vertex group per slot decides
which body faces go).

| Slot | Variants |
| --- | --- |
| `base` | The body: head, neck, hands, and whatever the outfit leaves visible |
| `hair` | `cropped`, `cropped_band` (with a headband) |
| `head` | `none`, `hood_up`, `circlet` |
| `torso` | `tunic` (fitted, hip length), `jerkin` (sleeveless) |
| `outer` | `none`, `coat` (knee-length, open, leaf-cut hem), `cape_short` |
| `legs` | `leggings`, `trousers_wrapped` |
| `feet` | `boots_tall`, `boots_ankle` |
| `arms` | `bracers_wrapped`, `gloves_fingerless` |
| `belt` | `sash`, `belt_pouches` |
| `gear` | `satchel` (cross-body), `map_case`, `bedroll` |
| `held` | `staff`, `bow_short` (on `prop_r` and `prop_l`) |

Three outfits to start, one per role option: `wayfarer` (C: hood, coat,
tunic, leggings, tall boots, bracers, sash, satchel, staff), `explorer` (A:
hood, jerkin, wrapped trousers, ankle boots, gloves, belt pouches, map case),
and `grove` (B: circlet, coat, tunic, leggings, tall boots, sash, staff).

Outfits fit the existing outfit catalog: `Outfit { id, name, model }` in
`crates/verse-world/src/service/outfits.rs`, where `model` is a string such
as `original-wayfarer-explorer`.

## Materials and textures

Everglade draws characters with base color only, so the look is baked into
one image:

- **Palette atlas.** One base-color atlas of flat swatches with soft
  gradients, in the Quaternius manner. Every part's UVs land on its swatch.
- **Baked shading.** A deterministic Cycles bake (fixed samples and seed)
  of ambient occlusion and a soft top light, multiplied into the atlas, so
  folds and seams read without a normal map.
- **Tint slot.** The accent (sash, hood lining) samples a neutral swatch
  with a material base-color factor, so a player color recolors it without
  a new image.
- **No source textures from any kit** are needed. If a material later
  wants a fabric or leather pattern, take it from a CC0 kit already admitted
  and record it in the provenance.

| Variant | Base color size |
| --- | --- |
| `lod0` (chamber) | 1024 x 1024 (the importer's cap) |
| `lod1` (Everglade) | 512 x 512 (`PLAYER_TEXTURE_EDGE`) |
| `lod2`, `lod3` | 256 x 256 |

When the textured path reads normal maps ([plan item 2](ue5-ruins-study.md#the-plan)),
add a baked tangent-space normal map at the same size for `lod0` only.

## Animation set

The Universal Animation Library is unisex on one rig, so every clip below
plays on her as it does on the Ranger. `gaits.glb` is the original library
(no root motion) and `animations.glb` is library 2; both are retained under
`assets/verse/characters/quaternius/`. Clip IDs and their states are bound
in `bind_states` (`crates/verse-content/src/compiler/original.rs`), and
Everglade's player clips in `PLAYER_CLIPS`
(`crates/verse-zone-everglade/src/zones/everglade_pack/compile.rs`).

| State | Source today | Status |
| --- | --- | --- |
| Idle | Clip 0, authored by `humanoid_motion`; `Idle_Loop` available | Plays |
| Walk | Clip 4, `Walk_Loop` (foot sync over 1.3 m per loop) | Plays |
| Jog and run | Clip 5, `Jog_Fwd_Loop` (5.0 m per loop), from 5 m/s | Plays |
| Sprint | Plays the run clip at `SPRINT_MULT` 1.6; `Sprint_Loop` is in `gaits.glb` | Gap: bind `Sprint_Loop` |
| Strafe left and right | Clips 14 and 15, authored | Plays |
| Backpedal | Clip 13, authored | Plays |
| Jump | Clip 37, `NinjaJump_Idle_Loop` (one airborne loop) | Partial: `Jump_Start` and `NinjaJump_Start` unused |
| Fall | The same airborne loop | Gap: bind `Jump_Loop` |
| Land | None | Gap: bind `Jump_Land` or `NinjaJump_Land` |
| Levitate | None | Gap: author a hover from `Spell_Simple_Idle_Loop`'s upper body with tucked legs and a slow bob |
| Cast | Clips 52 (cast) and 53 (release), authored; 25 and 51 ready poses | Plays |
| Cast variants | `Spell_Simple_Enter`, `_Idle_Loop`, `_Shoot`, `_Exit`, `OverhandThrow` | Gap: bind |
| Bow | Clips 109 and 46, authored (`archery`) | Plays |
| Hit react | None | Gap: bind `Hit_Chest`, `Hit_Head`, `Hit_Knockback` |
| Death | Clip 1, authored; `Death01` available | Plays; `Death01` optional |
| Swim (later) | `Swim_Fwd_Loop`, `Swim_Idle_Loop` | Gap: needs water states |
| Climb (later) | `ClimbUp_1m` only | Gap: no wall-climb loop exists in either library |
| Vault | None | Gap: compose from `ClimbUp_1m` and `Roll`, or author |
| Slide, roll | `Slide_Start`, `_Loop`, `_Exit`; `Roll` | Gap: bind when the controller has them |
| Crouch, sit, interact | `Crouch_*`, `Sitting_*`, `Interact`, `PickUp_Table` | Gap: bind as needed |
| Starts, stops, turn in place | None | Gap: procedural (a short lean and foot plant over the 0.22 s transition) |

Two engine limits shape this:

- **Eight clips per character in the Everglade pack.** `MAX_CLIPS` is 8,
  and `PLAYER_CLIPS` already uses all eight (idle, walk, run, jump,
  backpedal, two strafes, swing). Adding sprint, fall, land, hit, and
  levitate means raising `MAX_CLIPS` and the pack format's version. This
  helps every character, not only her.
- **Linear keys only.** `retarget_clip` accepts linear interpolation, so
  authored clips must be baked to linear keys.

The clips most worth adding first, because Echo's study shows they carry the
feel: land, sprint, hit react, and a start and stop lean.

## Export into the packs

1. `scripts/blender/wayfarer.py` builds the rig, body, parts, and outfits,
   bakes the atlas, and writes each outfit and variant to
   `assets/verse/characters/original/wayfarer/<outfit>.<variant>.glb` with
   `kit.export` (GLB, applied modifiers, Y up, no animations). It prints one
   `MODEL name triangles` line per file.
2. A new admission script, `scripts/blender/character_admit.py`, modeled on
   `beasts_admit.py`, splits each GLB into `.gltf`, `.bin`, and PNG, checks
   the rest transforms against the Universal base, and writes a manifest in
   the `openagents.verse.character-sources.v1` shape with SHA-256 digests.
3. `PROVENANCE.md` in that folder records the mode (Reference), the script
   and command, the Blender version, that Valley of the Ancient was studied
   in Reference-only mode, and that no Valley or Echo content was used.
4. In `crates/verse-content/src/compiler/characters.rs`, add the outfits to
   `APPEARANCES`, and give `appearance` an arm that imports an original
   whole model directly instead of composing a Quaternius outfit with a base
   head. Then call `animations` as the other arms do.
5. Budgets: the character admission enforces
   `Limits::EVERGLADE.character_triangles` (40,000), `MAX_JOINTS` (256),
   `MAX_CLIPS` (8), four joints per vertex, and the pack's 12 MiB and 48 MiB
   decoded-texture limits.

## Selecting her

Alice herself is not selectable (see [Alice, the first build](#alice-the-first-build)).
This section applies to the later female player character built from her
parts.

Today the player model is fixed: the chamber takes `--appearance` (default
`male-ranger`), and Everglade packs `PLAYER_APPEARANCE`, a constant set to
`male-ranger`. To make her a choice:

1. **Chamber.** `verse_play --appearance wayfarer` works once step 4 of the
   export lands.
2. **Everglade.** The pack carries one player model (`player/male-ranger`).
   Carry each selectable model under its own name (`player/wayfarer`), or
   build one pack per choice. Read the character section of
   `crates/verse-zone-everglade/src/zones/everglade_pack/format.rs` before
   choosing, and check the 12 MiB pack limit.
3. **Save.** Store the chosen appearance in the player's save; the host
   already notes that additional appearances belong to the save
   (`crates/verse-world/src/service/host.rs`).
4. **Picker.** Add a body choice beside the outfit list in
   `crates/verse-imported/src/imported/character_panel.rs`, and offer it on
   first run.
5. **Multiplayer.** Other players must draw the right model, so the
   appearance ID travels with the player's presence. Check what NIP-MV
   entity state carries (`nips/openagents/NIP-MV.md`) before adding a field.

## Tests and captures

In the Blender script, assert for each variant:

- triangles within the variant's budget;
- every vertex has one to four influences that sum to 1 after rounding;
- no vertex is unweighted, and left and right weights mirror;
- the joint names are the Universal 65 plus the listed extras, with rest
  transforms within 0.0001 of the base file.

In Rust:

- a `characters` test that imports each `wayfarer` appearance, retargets
  every clip the state table binds, and stays within the triangle budget;
- an Everglade pack test that the pack compiles within `Limits::EVERGLADE`
  with her included.

Captures, looked at before every commit:

- a turnaround (front, three-quarter, side, back) with `preview.py` and the
  gallery (`build-models.sh`);
- silhouettes at 64 px and 24 px tall, in black, beside the male Ranger at
  the same scale;
- the chamber: `verse_play --appearance wayfarer`, one frame mid-cycle per
  clip, and the deformation poses;
- Everglade: the spawn view and a street view with the zone's capture
  example;
- the web build compiles: `cargo check -p everglade-web --target
  wasm32-unknown-unknown`.

The owner reviews the turnaround against the [Echo rule](#mode-and-the-echo-rule)
before she ships.

## Plan

Estimates are agent-hours at the pace in
[the smart terminal's estimate basis](../terminal/smart-terminal.md).

| Phase | Work | Hours |
| --- | --- | --- |
| 0 | Owner picks the role, name, hair, and palette ([open questions](#open-questions)) | 0 |
| 1 | Rig from the CC0 base, lofted body, scripted weights, rest-transform check, deformation poses; plays `Walk_Loop` and `Jog_Fwd_Loop` in the chamber | 4 to 6 |
| 2 | Head, version 1 face, hair shell and cards | 3 to 4 |
| 3 | Clothing and gear parts, three outfits, body masking | 4 to 6 |
| 4 | Palette atlas, baked shading, tint slot | 2 to 3 |
| 5 | `lod1` to `lod3` variants from the same script | 2 to 3 |
| 6 | Admission script, manifest, provenance, `APPEARANCES` and `appearance`, chamber capture | 3 to 4 |
| 7 | Selection: Everglade pack, save, picker, multiplayer appearance | 4 to 6 |
| 8 | Animation gaps: raise `MAX_CLIPS`, bind sprint, fall, land, hit, and cast variants, author levitate, start and stop leans | 6 to 10 |
| 9 | Version 2 face: eyelids, jaw, procedural blink | 2 to 3 |
| | **Total** | **30 to 45** |

Phases 1 to 6 give a playable character in the chamber in 18 to 26 hours.
Phase 8 is engine work that improves every character and can run in
parallel with phases 2 to 5.

## Open questions

Alice answered the name (Alice), and took defaults for the role, hair,
palette, and clips; she is an NPC, so question 4 applies to the later player
character. Her role in the world is the owner's to specify.

1. Which role: explorer, druid, or the recommended explorer-druid?
2. What's her name? The working ID is `wayfarer`.
3. Which hair color and accent palette?
4. Should she become Everglade's default player, or a choice beside the
   male Ranger? Do the Quaternius female Ranger and Peasant stay
   selectable?
5. Can the Everglade pack format raise `MAX_CLIPS` from 8 now, or should
   she ship with the current eight clips first?
6. Is the version 2 face (blinks and an open jaw) worth 2 to 3 hours before
   other work?
7. Is 16,000 triangles right for the Everglade variant, or should it match
   the Ranger's budget?
