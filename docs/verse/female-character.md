# Female character: an original Verse explorer

This page specifies an original female player character for Verse, built in
Blender by script and played on the Universal rig. It covers the design
brief, proportions, budgets, skeleton, skinning, face, hair, gear, materials,
animation, export, selection, tests, and a phased plan.

Status: proposed, October 5, 2026. Nothing is built yet.

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

- **Shape.** Chin-length, layered, with a side part, in a dark color; a
  hooded variant hides most of it. No buns, braids, or ponytail.
- **Construction.** A sculpted shell: one closed low-polygon mass with
  strand ridges carved in, plus four to six alpha-tested cards at the fringe
  and nape. Use alpha test, never blending, so it draws on WebGL2 without
  sorting.
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
