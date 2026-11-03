# MetaHuman support in Aethyr

Status: design report, 2026-11-02. The first pieces are built: MetaHuman
Lab and `aethyr/mannequin.coax` (see [MetaHuman Lab](#metahuman-lab)). The
rest is not built yet. It covers how Aethyr, the Coax engine that every Verse realm runs on, loads,
animates and draws Epic MetaHuman characters. It also covers a first plan for
5E Lab (`openagents.com/worlds/5e-lab`) to draw MetaHumans in place of its
analytic capsule figures.

The owner's direction: Unreal lets projects like ours use MetaHumans, so
Aethyr gets full MetaHuman support. Verse renders the characters itself.
Unreal is at most an offline export tool.

## Summary

- Before the Nuke (`0b401561a0`, 2026-10-27), MetaHumans worked in the Rust
  engine as a private, offline pipeline for one character, Draconia.
  Unreal Engine 5.7/5.8 built the character headlessly. A C++ tool on Epic's
  MIT-licensed OpenRigLogic read the DNA files and posed the face. A Blender
  importer mapped the meshes to Verse shading models. A Rust module rigged
  the body on MetaHuman's own skeleton. The face was posed per expression
  offline, never live, and the work never reached the web.
- None of that code is in the tree now. The rules, the private bucket and
  the broker survive: the protocol spec keeps `PrivateAsset`, the bucket
  `openagentsgemini-verse-private-assets` and the Cloud Run service
  `verse-assets` are still live, and the exports are still on the owner's
  Mac under `~/work/film-out/draconia/metahuman/`.
- Aethyr already has most of the body runtime. `aethyr/skin.coax` plays
  skinned clips with cross-fades. `verse/tools/people.py` rebinds parts onto
  a shared skeleton. Everglade's shader (`verse/everglade/gpu.coax`) skins
  vertices pulled from a storage buffer with four joints each. The
  Quaternius skeleton it uses has the same Unreal mannequin joint names as
  the MetaHuman body (`pelvis`, `thigh_l`, `calf_l` and so on).
- Recommendation: build every runtime piece so it does not depend on any
  one asset source, and test it in public on the CC0 Quaternius people. A
  MetaHuman then becomes one private pack format that the same runtime
  loads. The face rig is a port of the OpenRigLogic evaluator to Coax, with
  the MIT notice.
- The main risk is the licence. Epic's own licence page needs a login, so
  every claim here comes from the pre-Nuke study and from press reports. The
  owner has to read the terms while logged in and confirm them before a
  MetaHuman is sent to any browser other than the owner's.

## Binding rules

These rules come from the pre-Nuke `docs/verse/private-assets.md` and
`docs/verse/draconia/metahuman-study.md` (both at `853417f290`, the Nuke's
parent), from the current protocol spec
([§10.17](openagents-verse-protocol.md)), and from the owner. They apply to
every phase below.

1. **MetaHuman assets never enter git.** This covers DNA files, meshes,
   textures, grooms, compiled packs and anything derived from them. Local
   builds go under `~/.openagents/verse/private-build/` or
   `~/work/film-out/`. The repository holds code, schemas and manifests
   only. A manifest in the repository names no private asset, digest,
   length or placement. Placements stay in owner-local configuration
   (`~/.openagents/verse/private-assets.json`).
2. **They are never used to train AI models.** Under the Unreal Engine EULA
   (MetaHuman terms since UE 5.6), MetaHumans may be used in AI-assisted
   workflows but may not be used to train or improve AI models. Every export
   folder carries a `NO-TRAINING.txt` marker (it is present in
   `~/work/film-out/draconia/metahuman/` today). The rule also covers
   renders. A golden, screenshot, film frame or trace image that shows a
   MetaHuman must never go into a dataset, gym, eval set, public trace
   (`/trace/{uuid}`) or Labs output. Pack and render provenance sets
   `Provenance.no_training = true` (protocol §10.17).
3. **They arrive only through the private pipeline.** Packs are stored by
   content digest under `packs/` in the private bucket. The `verse-assets`
   broker hands out a signed URL that lasts 300 seconds, and only for a
   reader key that the asset's manifest lists (a NIP-98 request). Every
   other client draws the public fallback.
4. **Verse renders them, not Unreal.** Unreal (5.7 or later, headless) is
   used only offline, to create, rig and export characters. No Unreal code
   ships, and the runtime never calls Unreal.
5. **No Epic trademarks in our names.** OpenRigLogic's `TRADEMARKS.md`
   forbids "MetaHuman", "RigLogic" and "OpenRigLogic" in product or crate
   names. The text "based on OpenRigLogic" is allowed as attribution. Coax
   modules get generic names (`aethyr/face-rig.coax`, not
   `metahuman.coax`). The pre-Nuke `metahuman_rig.rs` broke this rule.
6. **Private-asset renders stay out of user tools.** The Labs spec puts them
   out of scope ([openagents labs](../cli/2026-11-01-openagents-labs.md)).

## Licence: what we know and what is unconfirmed

| Item | Terms as recorded (pre-Nuke study, 2026-10-13) | Confidence |
| --- | --- | --- |
| MetaHuman characters: meshes, textures, grooms, animation | Under the standard UE EULA since UE 5.6 (June 2025). They are "Non-Engine Products": usable in other engines and DCC tools with no royalty. You may not redistribute them as source-format assets. A UE seat licence is needed above $1M a year in gross revenue if you render MetaHumans outside Unreal. You may not use them to train AI. | Secondary. The press ([CG Channel](https://www.cgchannel.com/2025/06/you-can-now-sell-metahumans-or-use-them-in-unity-or-godot/), [Digital Production](https://digitalproduction.com/2025/06/05/metahumans-graduate-ready-for-unity-godot-and-the-fab-cash-register/)) names Unity and Godot. `metahuman.com/license` needs a login, and the EULA change log returned 403 when fetched on 2026-11-02. |
| OpenRigLogic (DNA reader and writer, RigLogic evaluator), `EpicGames/openriglogic` | MIT, created 2026-06-05. We may port it to Coax if we keep the notice. | Recorded from its LICENSE in the pre-Nuke study. The pre-Nuke `dna_dump` linked against it. Recheck the notice before porting. |
| MetaHuman DNA Calibration | A custom licence: "You may not distribute the Software." | Recorded from its LICENSE in the pre-Nuke study. Do not use it. |
| Unreal Engine as an export tool | Running UE headless to author and export is ordinary EULA use. | Inferred |

Open questions the owner must settle while logged in to Epic:

1. **A custom engine.** The press describes use "in any game engine or DCC
   application". Confirm that a custom engine (Aethyr, in Coax, on WebGPU)
   counts, not just named engines.
2. **Sending packs to browsers.** A web page that downloads a compiled pack
   ships asset bytes that anyone can pull out with developer tools. Confirm
   that a compiled, non-source format inside a product is "distribution in a
   Product" and not redistribution of source assets. Before the Nuke, Fab
   kits got the same yes (the licensed-kit class in `private-assets.md`).
   The private character path was never put on the web.
3. **Revenue threshold.** Confirm the seat-licence rule and price ($1,850
   per seat a year in 2025, from a secondary source). Record which OpenAgents
   entity holds the licence.
4. **Training.** Confirm how the AI clause reaches renders of MetaHumans,
   because our release walks keep goldens that agents look at. This document
   assumes the strict reading: no MetaHuman render goes into any training
   data.

Until these are settled, MetaHuman packs load only for reader keys the
owner grants, starting with the owner's own. Every public visitor sees the
fallback figures.

## What existed before the Nuke

All paths below are at `853417f290`, the Nuke's parent. Restore them with
`git show 853417f290:PATH`.

### Commits

| Commit | Date | What it added |
| --- | --- | --- |
| `0cfdcec775` | 2026-10-13 | The pipeline. `scripts/unreal/draconia_metahuman.py` authors the character headless. `export_textures.py` exports its textures. `dna_dump.cpp` and `build_dna_dump.sh` read DNA through OpenRigLogic. `scripts/blender/metahuman_import.py` builds the glb. It also added `docs/verse/private-assets.md` (the MetaHuman section) and `docs/verse/draconia/metahuman-study.md`. |
| `8ce0e6d4ec` | 2026-10-13 | RigLogic joint outputs read as deltas, which fixed the zeroed pose. Expression control files (`scripts/unreal/expressions/{neutral,smile,speak}.txt`). `render_reference.py`, a UE 5.8 scene capture used as the quality reference. |
| `12e096eb26` | 2026-10-13 | `fit_face.py`: Creator's `fit_state_to_target_vertices` fits our own head into MetaHuman's identity space (1,397 coefficients). The fit is matched by UV and aligned to a median error of 1.4 mm. |
| `ad3ff69d18` | 2026-10-14 | `draconia_rebuild.py`, run headless on a copy of the project: fit, skin, eyes, `request_auto_rigging(JOINTS_AND_BLEND_SHAPES)` (782 blend shapes, 870 joints), texture download, then DNA, DCC, geometry and material exports. Also `landmarks.py` (MetaHuman's face tracker as a metric), `skin_tone_probe.py` and `dna_dump --joints`. |
| `3733ca7592` | 2026-10-14 | `presets_export.py` exports all 29 Creator presets headless. A scorer ranks them and a vertex blender mixes them. |
| `9390a96864` | 2026-10-14 | `groom_export.py`: MetaHuman hair and brow cards with their atlases, carried onto the rigged head and shaded as Verse hair cards. |
| `9641a95e3a` | 2026-10-14 | Look refinements: painted brows under the cards, and an eye-relax control. |
| `2afc6f87ac` | 2026-10-15 | `crates/verse-imported/src/imported/metahuman_rig.rs` (988 lines): body skinning on MetaHuman's own skeleton. Also `groom.rs` (strand LODs), `pack_cache.rs`, and the `metahuman_meteor_film` example. |

### The pipeline, step by step

1. **Author (Unreal, headless).** `scripts/unreal/run_ue.sh` picks the
   newest installed engine and runs a Python script against a copy of the
   project, never the one open in the editor. Creator's Python API fits
   the face, sets skin, eyes and grooms, and asks Epic's cloud service to
   auto-rig the character and supply textures. That service needs the
   owner's Epic sign-in once. The stored refresh token was recorded as
   lasting until 2027-04.
2. **Export.** `MetaHumanCharacterExportBlueprintLibrary` (UE 5.8) writes a
   head DNA and a body DNA, a DCC bundle and geometry. The DCC bundle holds
   the head base colour, normal, three animated colour maps, three wrinkle
   normal maps and their masks, and the eye, teeth, lash and body maps. In
   UE 5.8.3 `Head_SRMF.png` came out all zero. Grooms are not in the
   bundle, so `groom_export.py` exports their cards separately.
3. **Read the DNA.** `dna_dump HEAD.dna OUT [--lod N] [--controls F]
   [--joints F] [--skin]` writes each mesh as OBJ (in centimetres),
   `joints.txt` (each joint's parent and neutral transform) and, with
   `--skin`, each vertex's weights. With `--controls`, RigLogic evaluates
   raw controls such as `CTRL_expressions_mouthCornerPullL`: blend shapes
   first, then joint deltas by linear blend skinning. Joint outputs are
   deltas from the neutral pose (translation in cm, Euler degrees, scale 0
   meaning unchanged).
4. **Import (Blender, headless).** `metahuman_import.py` gives each mesh a
   Verse shading model through glTF extras: head `skin`, teeth `standard`,
   eyes `eye`, saliva and eyeEdge `cornea` (specular only), eyelashes
   `hair`. It drops eyeshell (its occlusion is baked into the eye) and
   cartilage.
5. **Rig the body (Rust).** `metahuman_rig::Rig` keeps the 63-joint main
   skeleton plus two eye and two upper-lid joints that fit a 256-entry
   palette. Every twist, corrective and helper joint gives its weight to
   its nearest kept ancestor. Clothes take weights from the nearest body
   vertices, and follow an arm only within a sleeve's reach. The head goes
   to the `head` joint above the neck line. A frame check refuses a body
   that is not on the DNA body. Poses come from `rotate`, two-bone `reach`,
   `aim`, finger `curl`, `turn_palm` and eye `look`.
6. **Load privately.** The `verse-private` crate held the manifest
   `openagents.verse.private-asset.v1`, the broker, signed URLs, placements
   and phone grants.

### What worked

- A rigged MetaHuman posed through the MIT RigLogic: neutral, smile and
  speak.
- MetaHuman skin, eye and teeth maps under Verse's own shading models, with
  a curvature-based skin LUT, dual specular, a refracting iris with a
  limbal ring, a cornea shell and a tear strip.
- MetaHuman hair and brow cards, carried onto a fitted head.
- Body skinning and procedural posing, enough for one film shot (a caster
  throwing Meteor Swarm).
- Private delivery to the owner's desktop and to paired phones.

### What was unfinished

- **No live face rig in Verse.** Each expression was baked offline into its
  own glb ("built per expression, not live in Verse"). Wrinkle and
  blood-flow maps were never driven.
- **No clips on the MetaHuman.** It was posed, not animated. It was not on
  the Universal rig ("static, not on the Universal rig yet").
- **No LODs for the body or face.** Only the groom had levels of detail.
- **Garments without sleeves.** The parity robe was a shell cut around the
  body with the arms down. An arm raised above the shoulder pulled the
  robe into a sheet. Casting animations need real sleeves.
- **No web path.** `private-assets.md` said: "The web build has no
  private-character path."
- **One character only.** Draconia was a film and portrait subject, and
  there was never a crowd.

## What exists now

| Piece | Where | Reuse |
| --- | --- | --- |
| Skinned clip playback | `aethyr/skin.coax`: 96 joints, rest pose and inverse binds, translation lerp and rotation slerp, a 0.22 s smoothstep cross-fade, gaits that advance with distance, and palettes written as three rows of four | The body animation core. Its joint limit has to rise, and it needs layers. |
| Part rebinding and clips | `verse/tools/people.py`, `verse/everglade/people-rig.coax`: Quaternius CC0 parts rebound onto 65-joint male and female skeletons, with 12 clips (idle, walk, talk, nod, fold, lantern, spell, slash, run, die, knock, cower) | This is the retarget method. The joint names match the MetaHuman body's main skeleton, so the clips retarget by name. |
| GPU skinning | `verse/everglade/gpu.coax`: a 24-byte vertex pulled from a storage buffer, four joints and weights, per-person pose blocks with tint rows after the joints | The vertex path for crowds |
| Pack format | `verse/tools/pack_realm.py` (VPK1): `C` lines for skinned characters, licence per entry, budget, style checks | Extend it for a human pack, or write a sibling compiler |
| Asset identity | `aethyr/assets.coax`: content-addressed packs admitted whole or refused | A private pack is admitted by digest |
| Frame budgets | `aethyr/budget.coax` | Every LOD and crowd decision is checked against it |
| Figures | `aethyr/figure.coax`: rounded-bone figures and gaits for soft-shape realms | The fallback, and the shape of the skeleton plan |
| Rigid bodies | `aethyr/rigid.coax` | Skeletons collapsing on death |
| Painterly style | `verse/style.coax`, [art-styles.md](art-styles.md) §10, §11, §14 | The character shading rules for 5E Lab |
| Asset protocol | `openagents.verse.asset.v1` `PrivateAsset`, `PrivatePack` (with `rig`, `clips` and `forms`), `PrivatePlacements` and `PrivateUrlRequest` ([§10.17](openagents-verse-protocol.md)) | The schema is already specified |
| Live infrastructure | Bucket `gs://openagentsgemini-verse-private-assets` and Cloud Run `verse-assets` (both checked as present on 2026-11-02) | Reuse them, with a Coax client in place of the Rust one |
| Private exports | `~/work/film-out/draconia/metahuman/`: 29 preset heads, 9 grooms (such as `Hair_S_UpdoBuns`, `Eyebrows_M_Full`), DNA, DCC bundle, `NO-TRAINING.txt` | Off-repo test data for the owner's machine |

Nothing in the tree loads glTF at runtime. Offline tools turn glTF into
packs, and the runtime reads only packs.

## Architecture

The engine is asset-agnostic. A "human" is a pack that holds a body
skeleton, skinned LOD meshes, an optional face rig, materials and hair
cards. The CC0 Quaternius people and MetaHumans both compile to that pack.
Only the MetaHuman pack is private.

```text
offline, owner's Mac only                      repo (code)                 runtime (browser / native)
---------------------------------------------  --------------------------  -------------------------------
UE 5.8 headless: create, rig, export   ──►     verse/tools/humans/*        aethyr/assets.coax admits pack
  head.dna, body.dna, DCC maps, cards           (export scripts,            aethyr/skin.coax + anim-graph
dna tool (OpenRigLogic, MIT)           ──►      dna reader, pack compiler)  aethyr/face-rig.coax (near only)
pack compiler: LODs, rig, paint pass   ──►     manifests schema only       verse/gpu/human.coax (WGSL)
upload pack to private bucket          ──►     (no asset names)            verse-assets broker ⇢ signed URL
```

### 1. Import: mesh and skin

- **Source.** Read the DNA itself, not FBX. The DNA's Geometry layer holds
  every mesh at every LOD, with UVs, normals, skin weights and blend-shape
  deltas. Its Definition layer holds the joint hierarchy and bind pose.
  This is one format with an MIT reader, and it needs no Blender step for
  geometry.
- **Tool.** Write `verse/tools/humans/dna.coax`, a DNA binary reader
  ported from OpenRigLogic's `dna` library, with the MIT notice. Until then,
  restore `dna_dump.cpp` as a stopgap that the pack compiler shells out to.
- **Pack compiler.** Write `verse/tools/humans/pack_human.coax`. It does
  the following:
  - keeps the main body skeleton and folds helper joints into their kept
    ancestors, as `metahuman_rig.rs` did;
  - limits each vertex to four influences and renormalizes the weights;
  - quantizes positions to 16 bits over the part's bounds and packs normals
    to 8-bit snorm, the 24-byte vertex `people.py` already uses;
  - transfers costume weights from the body (port `Rig::skin`, including
    the sleeve rule);
  - writes LOD slices, the face-rig tables, painted texture layers and hair
    cards, and records the provenance with `no_training`.
- **Output.** A VPK1-family pack with `style` set and the licence ID set to
  the MetaHuman licence. It is written outside the repository and uploaded
  to `packs/<sha256>` by the existing `verse-private add` flow, ported to a
  Coax CLI.

### 2. Face rig: DNA and RigLogic evaluation

- **Module.** `aethyr/face-rig.coax`, a port of the RigLogic evaluation
  steps with the MIT notice. Its stages:
  1. GUI controls to raw controls: `clamp(slope * gui + cut, from, to)`.
  2. PSD correctives: products of raw controls, clamped to [0, 1].
  3. Joint groups: dense `K * x` blocks per face region give translation,
     rotation and scale deltas from the neutral pose.
  4. Blend-shape weights: a remap of the inputs.
  5. Animated maps: piecewise tables that give wrinkle and blood-flow mask
     weights.
- **LOD is a prefix slice.** The DNA orders joints, blend shapes and maps
  by LOD. A client loads only up to its maximum LOD, so unused data is
  never downloaded.
- **Compile, don't parse.** The pack compiler turns the Behavior layer into
  flat Coax tables (the "DNA-lite" the pre-Nuke study designed). The
  runtime never parses DNA.
- **Cost.** The full rig has about 250 GUI controls, about 800 raw and
  corrective expressions, 870 joints and 782 blend shapes. On the web,
  evaluate it only for figures that are close to the camera, and use the
  joints-only rig type (`MetaHumanRigType` offers joints only, and joints
  plus blend shapes). The face's joints form a second palette per near
  figure, so the body palette stays small. Blend shapes and wrinkle maps
  belong to the photoreal profile only.
- **Inputs.** A typed expression set (ARKit-style names where they fit,
  such as `jawOpen` and `eyeBlinkLeft`), mapped in the pack to the DNA's
  raw control names. Game code asks for "pain 0.7" and never sees
  `CTRL_expressions_*`.
- **Check.** A `#test` evaluates fixed controls and compares the joint
  deltas with values recorded from OpenRigLogic on the owner's machine. The
  recorded numbers are derived data, so they stay out of git. CI runs the
  same test on a synthetic DNA-lite rig that we write ourselves.

### The face rig as built (MetaHuman Lab)

The lab runs the face rig live. The pieces:

- `verse/tools/humans/dna_behavior.cpp` (the owner's machine, against
  OpenRigLogic) dumps the head DNA's Behavior layer as text and, with a
  third argument, its LOD0 blend-shape targets as binary. Both stay in
  `~/.openagents/verse/private-build/metahuman-dump/head/`.
- `verse/tools/humans/face_rig.py` compiles them into a flat block: the
  raw controls' correctives (PSDs), the LOD0 joint groups, the face joints'
  neutral pose, each face point's rest position, normal and six joint
  weights, the blend-shape deltas (0.01 cm and up, in 0.001 cm steps) and
  a few named expressions (neutral, smile, frown, surprise, talk, blink,
  jaw). It also evaluates them in Python as a reference, and the pack
  build fails if a blink or the jaw moves the head by less than 0.5 cm.
- `pack_human.py` writes the face pieces (head, teeth, eyes, lashes) as one
  vertex range and adds the line `F FIRST COUNT BYTES` to the catalog. The
  block follows the catalog's text after a NUL.
- `aethyr/facerig.coax` evaluates the block, ported from OpenRigLogic's
  evaluator with its MIT notice. Expressions set raw controls, PSDs
  multiply them, joint groups give each face joint's translation, rotation
  (degrees, XYZ order) and scale deltas, and the points take their
  blend-shape moves and then linear skinning against the neutral pose.
  The result is a move and a new normal for each face vertex.
- `apps/openagents-web/metahumanlab.coax`'s `lab_face` runs it each frame
  when the controls change: X cycles the expressions, they ease in over
  0.25 s, blinks come every 2.8 to 5.2 seconds, and talk opens and closes
  the jaw. The C face puts the rows after the body palette in the pose
  store, and the vertex shader adds the move before the body's skinning.
- The checks: `metahumanlab.coax` runs the rig on a tiny block written in
  the test, and `pack_checks.py` reads the material probe's blue channel
  (the rig runs) and how many face pixels an expression moves.

Not done yet: GUI controls (the block starts at raw controls), animated
maps (wrinkles and blood flow), and the joints-only LOD for figures far
from the camera.

### 3. Body skeleton and animation retargeting

- **Skeleton.** The MetaHuman body uses the Unreal mannequin hierarchy
  (`pelvis`, `spine_01` to `spine_05`, `neck_01`, `neck_02`, `head`,
  `clavicle_l`, `upperarm_l` and so on). The Quaternius skeleton in
  `people-rig.coax` uses the same names. Retargeting is a rest-pose rebind
  by name, which is the method `people.py` already applies: keep each
  joint's rotation keys, keep only the pelvis translation as its change
  from rest, and add that to the target's rest pose.
- **Clip sources, in order:**
  1. the CC0 Universal Animation Library clips we already pack;
  2. Fab animation packs, under the licensed-kit rules;
  3. our own capture.
  MetaHuman Animator output is a MetaHuman asset and follows the private
  rules.
- **Runtime.** Extend `aethyr/skin.coax`:
  - raise `SKIN_JOINTS_MAX` from 96, or keep it and use separate face
    palettes;
  - add layers: a base locomotion clip, an upper-body action clip masked
    from `spine_03` up, additive hit reactions, and procedural `aim` and
    `look` (ports of `Pose::aim` and `Pose::look`);
  - add two-bone IK for feet on uneven ground and for hand targets.
- **State machine.** `aethyr/anim-graph.coax`: states, transitions and
  timed events (a cast's release frame) as data. A pose is a pure function
  of (events, clock), so `aethyr/replay.coax` replays look the same as live
  play.

### 4. LODs

| LOD | Body source | Face | Hair | Use |
| --- | --- | --- | --- | --- |
| H0 | DNA LOD1 (LOD0 is film quality) | Joints-only face rig, eyes and lids | Cards | Close-ups, the acting figure |
| H1 | DNA LOD3 | Eyes, lids and jaw only | Cards, fewer | Mid-distance |
| H2 | DNA LOD5 to LOD7 | None (baked neutral) | One card shell | Far |
| H3 | Impostor | None | Baked in | Crowds, phones far away |

- Measure triangle counts on export. The compiler records them in
  `PrivatePack.triangles`, and `budget.coax` refuses a pack that cannot
  meet the realm's frame budget.
- Seam vertices between head and body share neck joints and identical
  weights at every LOD, as MetaHuman does, so no crack opens.
- Selection uses projected screen height with hysteresis. Levels
  cross-dissolve with a dither over 0.3 s, as `groom.rs` did for strands,
  so nothing pops.
- **Built in MetaHuman Lab.** `pack_human.reduce` writes two lower levels of
  every piece (except the eyes' blended layers) by vertex clustering on a
  0.8 cm and a 2.5 cm grid. All the vertices in a cell move to one place
  and take its skin weights, so the surface stays closed in every pose.
  Vertices facing another way or in another eighth of uv keep their own uv
  and normal there. They are the catalog's `D LOD NAME FIRST COUNT
  MATERIAL` lines. On the private pack that is 548k triangles at LOD0, 200k
  at LOD1 and 39k at LOD2. The lab draws LOD0 (with the face rig) while the
  figure stands taller than 340 px on the screen, LOD1 down to 130 px, then
  LOD2, with 12% hysteresis. The unlisted L key picks a level, and
  `pack_checks.py` compares each level's figure pixels with LOD0's (LOD1
  misses 0.4% and LOD2 1.3%).
- **Impostors.** Bake each class look into an octahedral atlas of N views
  times a few key poses (idle, walk phase, down) at pack time. Pose
  impostors cannot animate freely, so use them only past the distance
  where limbs read as a silhouette. An impostor is MetaHuman-derived and
  follows every private rule.

### 5. Hair and grooms

- **The web uses cards only.** Export Creator's card LODs and atlases
  (`groom_export.py`). Strands (Marschner shading, dual scattering) stay
  out of WebGPU v1.
- **Painterly profile.** Alpha-tested cards with the coverage atlas, the
  two toon bands, a root-to-tip tint from the U coordinate, and no
  specular lobe, or one soft band for the "sheen". Sort the card pre-pass
  front to back, and use alpha-to-coverage when MSAA is on.
- **Photoreal profile (later).** Kajiya-Kay with two shifted lobes, as the
  pre-Nuke study adopted.

### 6. Skin, eyes and teeth in our styles

5E Lab's glade is painterly. Art-styles §10 and §11 are binding for it:
character layers at most 512 texels (256 by default), no normal, roughness
or detail maps, fine-detail energy at most 0.035, two light bands, the
character rim, a tinted shadow, no specular, and textures sampled two mip
levels coarser.

| Surface | Painterly (5E Lab, Everglade) | Photoreal (film, Draconia) |
| --- | --- | --- |
| Skin | DCC base colour downsampled to 256 or 512, run through the pack-time paint pass (Kuwahara, 24 colours, saturation pulled into range), with a painted tone over it. Two bands, a warm rim and a cool shadow. No normal map. | Pre-integrated curvature LUT, dual GGX, cavity map, micro-normal, and wrinkle maps driven by animated maps |
| Eyes | A painted sclera and iris in one layer, a fixed catch-light, and a lid shadow baked into the albedo. Eye joints drive gaze, and lid joints drive blinks. | Refracting iris with parallax, limbal ring, cornea shell, occlusion shell and tear strip |
| Teeth and mouth | A flat light tone and a darkened mouth interior, drawn only at H0 | Standard PBR plus a saliva specular film |
| Brows and lashes | Painted into the skin at H1 and below, cards at H0 | Cards |

- **Built in MetaHuman Lab (photoreal).** The eye shell (`eyeshell`) is
  the occlusion and the eye edge (`eyeEdge`) is the tear strip. Neither
  has a texture. `pack_human.lid_coords` rewrites their uv as lid
  coordinates: u runs from the lid margin (0) inward (1), and v runs up the
  opening. They are materials 25 and 7, the pack's last pieces, and the
  face rig moves them with the lids. The lab draws them after the scene the
  way the host draws water: `water_transmit_fs` multiplies the light behind
  by the occlusion (darkest at the margin, the upper lid more), and
  `water_emit_fs` adds the tear film's glints and sky reflection. The CC0
  stand-in has no such meshes, so `eye_layers` gives it a procedural cap
  and ring on each eyeball to check the passes by eye. Saliva and the tear
  ducts (`cartilage`) are not drawn yet.
- **Anti-aliasing (MetaHuman Lab).** The lab draws through the host's post
  pass: bg_fs and scene_fs write linear light, and post_fs tones it, runs
  FXAA (Lottes' FXAA 2 filter on the toned image) and draws the HUD's
  strips. Probe views pass through unchanged. TAA would also settle the
  hair's dither, but it needs a history buffer and motion vectors that the
  host doesn't have yet.
- **Style conflict.** Art-styles §11 wants heads at 1/6 to 1/7 of height,
  with hands and feet slightly large. MetaHuman proportions are realistic,
  with the head at about 1/7.5. Creator's body and head controls can push
  toward the style. The owner decides how far, because the style check
  fails grey placeholders, not real proportions.
- The anime style uses the same painted layers with a cel band and an
  inverted-hull outline.

### 7. GPU budget in WebGPU

- **Vertex pulling.** Vertex data comes from one storage buffer, as in
  Everglade. Poses go to a storage buffer of three `vec4` rows per joint
  per figure. 44 figures times 65 joints times 48 bytes is about 137 KB a
  frame, which costs nothing to upload.
- **Instancing.** Draw each LOD mesh as one instanced call per class look,
  with a per-instance pose-block offset and tint rows, as Everglade's
  people do.
- **Skinning.** Do it in the vertex shader. A compute pre-skin pass (skin
  once, then reuse for the shadow pass) is an option if measurements show
  the shadow pass doubling the vertex cost.
- **Face rig.** Evaluate it on the CPU in Wasm for at most four H0 figures
  at 30 Hz, interpolated in between. That covers 60 to 120 raw controls
  per face after the LOD slice.
- **Measured (MetaHuman Lab, one figure, this Mac's Apple GPU, headless,
  1600x1000).** The GPU's span per frame is about 1.3 ms at LOD0 and
  0.9 ms at LOD2 (`frametime.ts`, p50), well inside 16.7 ms. The face rig
  costs 1.5 to 2 ms of CPU natively per frame in which its controls change
  (expressions easing, blinks, talk) and nothing on other frames. Two
  changes got it there from 5 to 8.5 ms. A joint group gathers its live
  inputs once, not per row. The points are unpacked into floats when the
  block opens, not read from bytes every frame.
- **Budgets.** Every realm declares a budget in `budget.coax`. Each LOD
  table entry has a triangle and byte cost, and the crowd planner picks LODs
  so that the frame fits, demoting the farthest figures first. Frame time
  is measured with `frametime.ts` at desktop size and at 390×844 css px, as
  art-styles §14 records for Everglade.

## Where each piece lives

| Piece | Path (new unless noted) | Public or private |
| --- | --- | --- |
| Unreal export scripts (author, rebuild, presets, grooms, textures) | `verse/tools/humans/unreal/`, restored from `853417f290:scripts/unreal/` | Code is public. It names no asset and takes folders as arguments. |
| DNA reader | `verse/tools/humans/dna.coax` (MIT notice) | Public |
| Pack compiler | `verse/tools/humans/pack_human.coax` | Public. Its output is private. |
| Private CLI (add, place, grant, revoke) | `verse/tools/humans/private.coax`, a port of `crates/verse-private` | Public |
| Retarget | `aethyr/retarget.coax`, the method of `people.py` | Public |
| Clip playback and layers | `aethyr/skin.coax` (extended) | Public |
| Animation state machine | `aethyr/anim-graph.coax` | Public |
| Face rig | `aethyr/face-rig.coax` (MIT notice) | Public |
| LOD and crowd planner | `aethyr/crowd.coax`, using `budget.coax` | Public |
| Shaders | `verse/gpu/human.coax` (skinned vertex, painterly and photoreal profiles, hair cards, impostors) | Public |
| Browser loader | `verse/private-load.coax`: a NIP-98 request to `verse-assets`, a digest check, then `assets.coax` admission | Public |
| Packs, DNA, maps, grooms, impostor atlases | `~/.openagents/verse/private-build/`, `gs://openagentsgemini-verse-private-assets/packs/` | **Private** |
| Placements and reader grants | `~/.openagents/verse/private-assets.json`, the manifest's `readers` | **Private** |
| Rules | Restore `docs/verse/private-assets.md` from `853417f290` with its MetaHuman section | Public |

## Phased plan

Each phase ends with Coax `#test`s, a budget check, and a frame-time number.
No phase commits an asset.

0. **Policy and licence.** The owner reads the Epic licence page while
   logged in and answers the four open questions above. Restore
   `docs/verse/private-assets.md` (it was deleted in the Nuke) and update
   it for the Coax runtime. Name the `no_training` handling for goldens and
   traces. **Gate:** the owner has given written answers to the four
   questions.
1. **The runtime on public characters.** Build the retarget, layers, state
   machine, LOD planner and `verse/gpu/human.coax` against the CC0
   Quaternius people. 5E Lab gets skinned people in public, with no
   licence question. **Gate:** 44 animated figures fit the budget on the
   phone tier.
2. **Export restored.** Bring back the Unreal scripts and the DNA tool.
   Export two or three Creator presets and their grooms on the owner's Mac.
   **Gate:** the DNA reader's numbers match OpenRigLogic's on the owner's
   machine.
3. **The first private MetaHuman in Verse.** The pack compiler, upload to
   the bucket, the browser loader, and an owner-only reader key. One figure
   in 5E Lab, painterly, with body clips. Everyone else sees phase 1's
   figures.
4. **Crowd.** MetaHuman LODs H0 to H2, impostors H3, and all class looks.
   Frame-time numbers at both sizes.
5. **Face.** A joints-only face rig for near figures: blinks, gaze,
   expression per state. The photoreal profile adds blend shapes and
   wrinkle maps for film.
6. **Costumes.** Garments with real sleeves, weight-transferred onto the
   body. Hair card sets per class.
7. **Wider delivery.** Only if phase 0 confirmed it: grant more reader keys,
   or move to the licensed-kit route (a pack served to every client and
   copied from the bucket at build time). Otherwise MetaHumans stay
   owner-only, and the public sees phase 1.

## MetaHuman Lab

`openagents.com/worlds/metahuman-lab` (unlisted) is the bench for phase 1.
It holds no MetaHuman asset and draws with none.

### Built

- **The figure.** One human figure on a studio floor. Its skeleton and clips
  are the CC0 people rig (`verse/everglade/people-rig.coax`: the Quaternius
  male and female skeletons, 65 joints, and the Universal Animation
  Library's 12 clips), played by `aethyr/skin.coax`. Its body is a building
  block written in `apps/openagents-web/metahumanlab.coax`: 25 capsules, one
  per bone, posed from the same joints, traced per pixel by
  `gpu/metahumanlab.coax`. N and P cycle the clips; walk and run carry the
  figure round a circle so the gait keeps pace with the floor. G switches
  bodies, and K draws the skeleton over a glass body with the mannequin's
  joint names.
- **The camera.** Aethyr's orbit (`aethyr/shot.coax`) as 5E Lab uses it,
  with its shoulder offset taken back out so the view centres on the
  pivot: drag orbits, the wheel or a pinch zooms, Shift-drag or two fingers
  pan, and held W, A, S, D, Q and E fly the pivot with eased speed.
- **Joint names.** `aethyr/mannequin.coax` names the 65 joints in the rig's
  order and folds another body's joint names onto them:
  `spine_04` and `spine_05` into `spine_03`, `neck_02` into `neck_01`,
  twist and corrective helpers into their bone on their side, metacarpals
  into the hand, and -1 for anything else (the compiler's nearest kept
  ancestor rule).

### Skinned meshes and private packs

- **The raster pass.** A human pack's mesh draws in the scene pass
  (`gpu/metahumanlab.coax` `scene_vs`, `scene_fs`, `shadow_vs`): each vertex
  is skinned on the GPU by four joints of the pose's palette (store 0,
  `lab_palette`), drawn into the depth buffer `bg_fs` writes the traced floor
  into, and lit by a three-light portrait rig: a warm key with a 2048 shadow
  map (16-texel PCF), a cool fill and a rim, plus a sky and floor ambient.
  Skin wraps and reddens past the terminator (a pre-integrated scatter) and
  takes two GGX lobes; base, normal and roughness maps come from the pack;
  output is ACES filmic and the sRGB curve. The capsules stay: they cast the
  figure's shadow on the floor and draw the figure when no mesh is in.
- **Bone lengths.** `lab_palette` turns each joint by the clip's change from
  the rig's rest and places it by its parent's turn of its own bind-pose bone,
  so a pack keeps its proportions. Bound at the rig's rest it equals
  `skin_palette` (a check runs every clip).
- **The pack.** `verse/tools/humans/pack_human.py` writes one VPK1 file:
  the catalog (body, 65 bind positions, materials, pieces), 32-byte vertices
  (position, octahedral normal and tangent, uv, four joints and weights, the
  layers its material reads), u32 indices and the texture layers. Weights
  must sum to 255 and joints stay under 65, or it refuses.
  `pack_human.py standin` builds a CC0 stand-in (the Quaternius superhero
  male with eyes, brows and hair, on the rig already) for checking the look.
  `pack_human.py metahuman EXPORT_DIR` is the MetaHuman source. When the
  export's portrait (`latest/mh.neutral.glb`) is there, its grooms (hair cards
  and brows, rigid to the head) and clothes join the pack, with the film's
  lower garments beside it (`world/*_lower.glb`: tights, skirt, the robe's
  lower half, shoes); each cloth colour becomes one of 17 cloth materials
  (8 to 24).
- **The bind pose is the clothes'.** The clothes were fitted to the body with
  its arms lowered (the export's `rig-body-down`), so that pose is the pack's
  bind: the body as `dna_dump` posed it, the clothes as authored, the head
  unchanged. Each joint's motion from the DNA's bind to it is fitted to the
  body vertices it nearly owns (Horn's method), and each `B` line carries the
  joint's turn from the bind to the rig's rest (`lab_palette`'s `bturn`), so
  every vertex is skinned once, from where it was authored. Pre-posing into
  the T pose and skinning again from the clip tore the cloth where weights
  mixed joints that turn apart.
- **Cloth weights.** Each clothing vertex blends the weights of the body
  vertices near it (a falloff with distance, less from a vertex the cloth is
  inside of), smoothed over the cloth's own neighbours. A vertex takes arm
  weight only on a sleeve: the nearest body point it lies outside of is an
  arm's (a robe's front over clasped hands lies outside the thigh), its foot
  falls on the upper arm or forearm (not past the cuff), and it sits on a
  large connected run of such points that wraps the upper arm (fills at least
  10 of 12 sectors around the bone; a robe's front by a hand or tights where a
  wrist rests on the hip don't). A sleeve never follows the hips or legs
  (a cuff by clasped hands lies over the thigh). Off a sleeve it takes weights from the
  body points off the arms only; hands give theirs to the forearm. Loose cloth
  (most of it more than 1 cm off the body: the robe, the skirts) hangs from
  the hips all the way down, half or more from the pelvis (up to 85% at the
  ankles, where a step parts the thighs by tens of centimetres), and blends its weights over a few centimetres of
  space, so where it parts from an arm or between the legs it stretches over
  that span. A layer over clothes (a sweater over a shirt, half a centimetre
  off) blends over a little space too, where its sleeve was fitted against its
  side. Closer cloth more than 3 cm off the body hangs from the hip. A
  piece under 40 cm (a tie, a collar's fall, a trim) moves as one. Body
  triangles under tight cloth (a garment within 2 cm outward that moves
  with them) are left out; the compiler refuses a pack whose hands are hidden.
- **Skin maps.** The head takes the portrait's maps (the export's
  Head_Basecolor toned by a per-channel line, its normal map, and an
  occlusion-roughness map); the body's base gets the same line so neck and face
  meet, and the head's mean roughness (the export's SRMF maps are empty, and an
  empty map is no layer). Skin occlusion rides the base's alpha.
- **Checks the compiler runs.** The garment list against the sources' (every
  garment packed), and each garment posed by the lab's clips with the
  runtime's own palette (idle and walk at several moments; talk, spell,
  slash, run and fold once): an idle or walk pose with more than 0.5% of its
  edges grown by half again and by over 1 cm, or one grown by over 4 cm (a
  spike), refuses the pack. The share stretched past 1.5x is printed too; on
  a fine mesh it counts millimetre edges that no one sees.
- **The armpit band.** Where a garment's weights mix the arm with the trunk
  (a sleeve meeting the side), and two rings round that, the weights are
  smoothed over the garment's neighbours 30 more times
  (`metahuman.armpit`). A raised arm then stretches the cloth across the
  band instead of along the seam. Only close garments (the sweater) get the
  band: in the big clips it cut the sweater's worst stretched share from
  7.8% to 5.4% (slash) and 4.6% to 3.2% (spell). A loose robe is blended
  over 6 cm already, and the band spiked its edges in a walk past the 4 cm
  limit, so the robe still stretches in slash and spell.
- **Probe views for scripts.** An unlisted V key cycles views a script reads
  without anyone looking at the frame: the albedo, uv and base layer, capsule
  pixels in magenta, each pixel's material, and its material with its base
  layer in one frame. Scripts compare the albedo
  with the source texture at each pixel's uv (mean difference and hue and
  saturation per region), count capsule pixels with a pack in (none), and
  check the skin's pixels take the scatter path.
- **Hair and eyes.** Hair cards are lit along their strands (two shifted
  Kajiya-Kay highlights, after Marschner's R and TRT lobes), not by GGX, with
  the normal bent toward the way out from the head; their faintest texels go,
  their solid ones stay, and a stable 4x4 dither fades the rest. Eyes add a
  Fresnel sky reflection on the cornea and a sharp key glint.
- **Floor shadow.** With a pack in, the floor takes the figure's shadow from
  the mesh's own shadow map; the capsules no longer draw or cast.
- **Private loading.** The world's assets are `private:human`. `host.js` asks
  for `/private/NAME.pack` only on a local address whose URL has `?pack`
  (`?pack=standin` for the stand-in), never in a golden; `serve.c` serves
  `/private/` from `~/.openagents/verse/private-build/web/` and only while
  watching. Anywhere else, or with no file, the four parts fail quietly and
  the capsule body draws. The deploy's asset rewrite never matches
  `private:human`, so no release carries a pack.
- Renders of a private pack are never committed, used as goldens or
  training data, or read by agents. The world's golden shows the capsule
  body only.

## 5E Lab

### Today

On `main` at `a35df9a4c9`, 5E Lab draws 24 figures, twelve a side
(`RS_MAX :: 24`, `FIVELAB_TOKENS :: 24`):

- the Circle of Moss: five Druids and seven Berserker Barbarians;
- the Ember Pact: five Fiend Warlocks and seven Open Hand Monks.

`gpu/fivelab.coax`'s `fivelab_body` builds each figure from a capsule body,
a sphere head and a staff, and lays it flat when it is down.
`fivelab_trace` ray-traces those shapes, the trees and the ground
analytically in the fragment shader. Token state arrives in a uniform
block: two `vec4`s per token (position, class, hit points, state,
condition bits, turn). The owner's plan adds Skeletons and up to 44
figures. That needs `RS_MAX`, the uniform layout and the token list to
grow, or the token list to move to a storage buffer.

### The rendering change

- The figures move from the trace to a raster pass. The ground, trees, sky
  and spell areas stay traced. The skinned figures are drawn first with
  depth, then the trace runs only where depth is empty, or the trace
  writes `frag_depth` and the two compose.
- Shadows and picking keep using capsules. Each bone gets a capsule posed
  from the same palette, much as `verse/cast.coax` does for the Grid's
  robots. The traced sun shadow and clicks stay analytic and cheap, and
  they match what is drawn.
- The flat view does not change.

### Class looks

Each class look is a Creator preset (or a blend of presets) plus a costume
and a hair card set. Team colour comes through tint rows, as Everglade's
outfit colourways do.

| Class | Body and face | Costume | Props | Silhouette at 20 m |
| --- | --- | --- | --- | --- |
| Druid | Lean, weathered, varied ages | Hooded robe with real sleeves, leaf and bark trim | Staff (rigid on `hand_r`) | Hood and staff |
| Fiend Warlock | Pale, sharp features | High-collared coat, Armor of Shadows as a dark emissive edge | Pact focus or tome | Collar and coat tails |
| Berserker Barbarian | Heavy build (Creator body controls) | Fur mantle, bare arms, leather | Greataxe | Width and the axe |
| Open Hand Monk | Athletic | Wrapped tunic, sash, bare feet | None (unarmed) | Narrow and upright |
| Skeleton | Not a MetaHuman (see below) | None, or scraps | Shortsword or bow | Thin limbs |

A few presets per class give variety: 5 to 7 figures of one class should
not look like twins. Variety comes from preset, skin tone, hair set and
tint. The Druids and Warlocks need sleeves that survive raised arms. That is
the pre-Nuke robe limit, and the costume phase fixes it.

### Animation states and the battle's events

The SRD engine already reports every change as a typed event
(`aethyr/rules/srd/engine.coax`). The animation graph reads those events.
It never reads text, and it never changes battle state.

| State or beat | Trigger | Clip or procedure |
| --- | --- | --- |
| Idle | Standing, no recent event | Idle loop, with a combat-ready variant once the battle starts |
| Walk | The token's drawn position slides to its square (`dx`, `dy`) | Walk gait, advanced by distance |
| Turn | `RS_EV_TURN` | Head and chest `aim` at the target |
| Cast | `RS_EV_CAST` (spell, caster, target) | Cast clip per spell shape (point, cone, area). The FX bolt leaves on the clip's release mark. |
| Melee | `RS_EV_ATTACK` or `RS_EV_MISS` with no spell | Barbarian two-hand swing, Monk punch or kick (Flurry of Blows chains two), Skeleton slash. A miss ends off target. |
| Hit | `RS_EV_DAMAGE` on the target | Additive flinch away from the attacker, sized by the share of hit points lost. Face: pain. |
| Save | `RS_EV_SAVE` | Brace, short |
| Pushed | `RS_EV_PUSH` | Stagger back along the push |
| Conditions | `RS_EV_CONDITION` and the token's condition bits | Restrained: struggle loop. Prone: lie. Paralyzed: clip time frozen. Poisoned: hunched upper-body layer. |
| Down | `RS_EV_DOWN` | Fall clip, ending lying |
| Death saves | Each roll (`RS_DS_OK`, `RS_DS_FAIL`), then `RS_EV_STABLE` | Lying loop with a breath. A failure jerks, a success eases. Stable: slow breathing. |
| Back up | `RS_EV_UP` (a 20, or a heal) | Get-up clip |
| Heal | `RS_EV_HEAL` | Straighten, glow on the FX layer |
| Dead | `RS_EV_DEATH` | Death clip, then a held pose. No breathing, eyes closed. |

In action-combat mode (`MODE_ACTION`, a turn every 1.5 s), clips have to
fit in 1.5 s. The graph shortens wind-ups and lets a new action interrupt a
recovery. The 6-second-round mode uses the full clips.

### Keeping 44 figures in budget

- **The acting figure and its target get H0.** They are the only faces
  rigged live, at most four at once.
- **The rest:** H1 within about 12 m of the camera, H2 beyond, and H3
  impostors on the phone tier past about 25 m, or when the planner has to
  demote.
- **Skeletons** cost almost nothing (see below).
- **Measure; don't assume.** The phase 1 gate is all figures animating at
  the phone tier's frame time. It is measured first on CC0 people, then on
  MetaHuman LODs.

### Skeletons

The SRD Skeleton is not human, and a MetaHuman is the wrong source for it.
Draw Skeletons on the same body skeleton and clips, with no MetaHuman
asset:

- **Analytic bones (recommended first).** Each bone is a capsule between
  two posed joints, and the skull is a small set of spheres, traced like
  today's figures but driven by the shared skeleton and clips. Skeletons
  then need no pack and no licence, and share every animation.
- **A mesh later, if wanted.** A CC0 or Fab skeleton mesh with rigid
  one-joint-per-bone skinning on the mannequin hierarchy.
- **Undead motion.** Skeletons get their own idle and walk (stiffer, with
  a jaw rattle), and use the shared cast, melee and hit clips.
- **Death.** Skeletons have no death saves. At 0 hit points they collapse:
  each bone becomes an `aethyr/rigid.coax` body with the bone's last
  velocity and settles into a pile. This is presentation only. The battle
  marks the Skeleton dead.

## Risks

| Risk | Effect | Mitigation |
| --- | --- | --- |
| The licence does not cover a custom web engine, or does not cover browser delivery | No public MetaHumans | Phase 0 gate. The public stays on CC0 people. |
| Seat licence above $1M a year | Recurring cost | Settle it in phase 0 |
| A MetaHuman render reaches training data | A licence breach | `no_training` provenance, `NO-TRAINING.txt`, no MetaHuman goldens in any shared set, private renders out of Labs and traces |
| Asset bytes extracted from a browser | Possible redistribution | Owner-only reader keys until phase 0 says otherwise. Ship compiled packs only, never DNA or source maps. |
| Epic cloud services (auto-rig, textures) or the stored sign-in change | The export breaks | Keep exports, re-run headless only on copies, and record the UE version per pack in provenance |
| The UE Python API changes between versions (it changed between 5.7 and 5.8) | Script breakage | Pin the UE version per pipeline run and test scripts after each upgrade |
| Painterly rules versus realistic MetaHuman faces | They look out of style | The paint pass, Creator proportion controls and the owner's review |
| Garment sleeves on raised arms | Robes tear into sheets | Garments with real sleeves, a phase 6 gate |
