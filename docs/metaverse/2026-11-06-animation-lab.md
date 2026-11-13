# Animation Lab

Written 2026-11-06. Status: A0 documents, A1 rig/clip stations, A2 retargeting and A3 layers/fix-ups are implemented.
A4 imports are implemented within the [v1 profile](anim-imports-v1.md); the exact
Echo/Slay re-import qualification remains open in [#12983](https://github.com/OpenAgentsInc/openagents/issues/12983). A5 scenes/cuts and the private Guardian study are implemented within the
[scene profile](anim-scenes-v1.md). A7 is implemented in the [directing profile](anim-directing-v1.md); A6 and A8 are pending.

Animation Lab is an unlisted realm and a set of `openagents labs` commands
where you take any character's animation apart, change it and put it back
together, then build scenes and cuts from it. Everything renders in the OpenAgents Metaverse.

It generalizes three things we already do:

- **Valley Lab's Echo work:** read a character and its clips from a source
  project, keep the data in a private pack, play it on the OpenAgents Metaverse Engine and compare it
  with the source. Today that's written for one character (Echo) and one
  source (Valley of the Ancient, then Slay's face clips).
- **Anime Lab's director:** words become typed directions, checked against a
  catalogue, laid out on a timeline and played.
- **The pre-Nuke Video Lab:** a film is typed documents (cut, shot, scene),
  every edit is a version, and one checked writer changes them.

The example production is a short original Draconia scene (see "Example:
Draconia, The Guardian").

## What exists

| Piece | Where | What it does today |
| --- | --- | --- |
| Animation documents (A0) | [Formats](anim-documents-v1.md), `metaverse-engine/anim/*-document.coax` | Six bounded document readers, compiled-DNA admission, lossless private Echo/Slay conversion and shared source adapters; [verification](../verification/animation-lab-a0-2026-11-06/README.md) |
| Rig and clip stations (A1) | `metaverse/animlab/`, `/worlds/animation-lab` | Neutral inspection, weights, shapes, scrub/play/step, skeleton ghosts and cached plots; [verification](../verification/animation-lab-a1-2026-11-06/README.md) |
| Retargeting (A2) | `metaverse-engine/anim/retarget*.coax`, `#retarget`, `labs anim retarget` | Rest-corrected anchor rotations, leg-scaled root motion, private exports and paired views; [verification](../verification/animation-lab-a2-2026-11-06/README.md) |
| Layers and fix-ups (A3) | `metaverse-engine/anim/layers.coax`, `fixup.coax`, `#layers` | Base, masked override, additive and facial layers; planted feet, head/eye aims and hand targets; [verification](../verification/animation-lab-a3-2026-11-06/README.md) |
| Imports (A4) | `metaverse-engine/anim/import-*.coax`, `gltf-*.coax`, `labs anim import` | glTF/FBX, BVH, ARKit CSV and bounded Unreal body imports; [limits](anim-imports-v1.md) and [verification](../verification/animation-lab-a4-2026-11-06/README.md) |
| Scenes and cuts (A5) | `metaverse/animlab/scene.coax`, `cut.coax`, `#scene`, `#cut`, `labs anim play/check` | Staged projects, authored cameras, light/effect keys, speech-driven stand-in faces, headless films and JSON checks; [profile](anim-scenes-v1.md) and [verification](../verification/animation-lab-a5-2026-11-06/README.md) |
| Pose math | `metaverse-engine/anim/pose.coax` | Quaternions, local poses, blending, the joint chain and the skinning palette |
| Blend shapes | `metaverse-engine/anim/morph.coax`, `morph-gpu.coax` | Sampled curves; sparse morph deltas applied on the GPU before skinning |
| Face playback | `metaverse-engine/anim/face.coax`, `face-pack.coax` | Plays face clips as morph weights plus face-bone overlays on a body pose; the EFC1 face pack format and its checks |
| Face rig | `metaverse-engine/anim/facerig.coax` | A Coax port of OpenRigLogic: controls, correctives, joint groups and blend shapes from a MetaHuman DNA file |
| Clip playback | `metaverse-engine/anim/skin.coax` | Skinned clips chosen by speed (stand, walk, run, jump) |
| Retargeting by name | `metaverse-engine/anim/mannequin.coax` | The 65 UE-mannequin joint names; maps other skeletons onto them by name |
| Cameras | `metaverse-engine/cam/cinematic.coax`, `metaverse-engine/cam/shot.coax` | Authored camera flights with keys, easing and cuts; orbit and dive shots |
| Determinism | `metaverse-engine/core/replay.coax` | Record and replay of inputs, so the same inputs give the same frames |
| Speaking | `metaverse-engine/audio/speak.coax`, `coax/web/tool.sh speak` | Text to speech, word timing, visemes and moods on a MetaHuman face, rendered to video |
| Echo port | `metaverse/valley/echo*.coax`, `metaverse/tools/valley-echo-*`, `metaverse-engine/tools/unreal-echo-sdk/` | Echo's mesh, 334 joints, 24 clips and source walking profile, in a private pack |
| Slay face | `metaverse/valley/slay-shot.coax`, `slay-shot-play.coax` (#12962) | Slay's facial clips on Echo, with a fixed source-shot view |
| Anime director | `metaverse-engine/anime/director.coax`, `take.coax`, `stage.coax`, `coax/web/api/anime-director-policy.coax` | Words to typed directions with confidence; `.take` files (`anime.v1`); timeline grammar |
| Labs CLI | `apps/openagents-cli/labs.coax`, `docs/cli/2026-11-01-openagents-labs.md` | `labs shot`, `clip`, `frames`, `cut`, `compare`, `anime` |
| Private packs | `docs/metaverse/private-assets.md`, `docs/metaverse/metahuman-support.md` | Licensed data stays out of git, addressed by SHA-256, resolved owner-locally |

Before the Nuke (`git show 0b401561a0^:<path>`):

- `docs/metaverse/draconia/`: the Draconia plan, its character build (Blender on
  the 65-joint rig; hair chains, blend shapes, cloth as bone chains), the
  night-courtyard world, the MetaHuman pipeline (UE Python, DNA with 782
  blend-shape channels and 870 joints) and 20 parity rounds against a
  photoreal reference.
- `docs/video-lab/`: typed `cut.v1`, `shot.v1` and `scene.v1` documents and
  the `lab.*` chat tools.

The documents and admission are shared. The remaining milestones build scene tools and the director
on them.

## Principles

1. **One character model for every source.** A Slay clip, a Valley clip, a
   Mixamo-style FBX, a mocap BVH, a phone face capture and a hand-keyed pose
   all become the same clip type on the same rig model.
2. **Documents, not a timeline UI.** A scene is a typed file. The realm, the
   CLI, voice and agents all change it through one checked writer, and every
   change is a version you can diff and undo.
3. **Same inputs, same frames.** Rendering a scene twice gives the same
   frame hashes. That's what makes compare, review and agent iteration work.
4. **Sources stay private.** Licensed rigs, clips and faces live in private
   packs addressed by hash. Documents name packs by hash and role, never by
   their licensed content. Nothing licensed is committed or used for training.
5. **The Metaverse renders.** Unreal, Blender and other tools only read and export
   source data. Every frame we show is drawn by the Metaverse Engine.
6. **Checks you can see.** Foot sliding, joint limits, intersections and cut
   continuity are measured and shown on screen, with their limits stated.
7. **Directions are typed.** Spoken or written directions go through a
   decision model with a response schema, as Anime Lab's director does. No
   keyword matching.

## The model

Six document types. Each is JSON with a schema name, bounds, and a Coax
`#authority` admission procedure, the way `metaverse-engine/anime/take.coax` admits
`anime.v1` takes.

### Rig (`anim.rig.v1`)

What a character is made of:

- **Skeleton:** joint names, parents, rest pose, bind matrices.
- **Mesh:** skinned sections, up to 8 influences per vertex, material slots.
- **Face:** blend shapes by name, face bones, and optionally a DNA face rig
  (`metaverse-engine/anim/facerig.coax`) with its controls.
- **Retarget map:** each joint's match on the 65-joint mannequin
  (`mannequin_fold`), plus per-joint rest-pose offsets.
- **Face map:** how ARKit's 52 shape names map onto this face: direct shapes,
  DNA controls, or a weighted mix. This is what lets one face capture drive
  Echo's shapes and a MetaHuman's controls.
- **Pack:** the private pack's SHA-256 and the role of each part.

### Clip (`anim.clip.v1`)

One piece of motion:

- Joint tracks (local rotation and translation per frame), root motion, face
  curves by name, face-bone tracks, and events (foot down, contact, cast).
- Rate, length, loop flag.
- **Source:** where it came from (pack hash, source asset role, export
  receipt), so a clip's origin is always known.

### Performance (`anim.performance.v1`)

What one character does over time: a stack of layers, each with a clip, a
start time, a speed, a blend in and out, and a mask.

- **Base:** locomotion or a full-body clip.
- **Override:** replaces masked joints (upper body, one arm, the face).
- **Additive:** adds on top (breathing, a flinch, a head turn).
- **Face:** face curves and face bones only, as Slay's face clips are.
- **Fix-ups:** solved after the layers: foot lock, look-at, hand to prop,
  reach to a point. Each fix-up names its target and its weight over time.

Masks are named joint sets on the mannequin (`face`, `head`, `upper`, `arm_l`,
`hands`, `legs`), so they work on any rig with a retarget map.

### Scene (`anim.scene.v1`)

A stage and what happens on it:

- Stage: a realm and a place in it (Valley Lab's canyon, Anime Lab's
  courtyard), with time of day and weather.
- Cast: characters (rig plus look), each with a performance and a path.
- Props and effects with their own timed events.
- Lights with keys.
- Cameras: named flights (`metaverse-engine/cam/cinematic.coax` keys), lens and focus.
- Sound cues: voice lines (through `metaverse-engine/audio/speak.coax`), music and effects.
- Style: the render profile (realistic, anime cel), so one scene renders in
  either look.

### Cut (`anim.cut.v1`)

The edit: a list of shots, each a camera and a time range in the scene, with
transitions. Reuses the `.cut` line format from `docs/cli/cut-format.md` and
reads `cut.v1`.

### Pose (`anim.pose.v1`)

A single named pose (for a pose library and keyframes): joint rotations on
the mannequin, plus face curves.

## What you can do

### Bring in

| Source | How |
| --- | --- |
| Unreal projects (Valley, Slay, Epic samples) | The editor-only SDK plugin and Python exporters, generalized from `metaverse-engine/tools/unreal-echo-sdk/` and `valley-echo-*` |
| glTF and FBX | glTF reads directly; FBX uses headless Blender, then the same reader |
| BVH mocap | Read directly; retarget by name |
| Phone face capture | ARKit curves (Live Link Face CSV) straight into face curves |
| MetaHuman | The existing pipeline (`docs/metaverse/metahuman-support.md`), DNA through `metaverse-engine/anim/facerig.coax` |
| Hand keys | Poses from the pose library, set at times |

Every import writes a receipt (source, export settings, counts, hashes) and
a private pack.

### Change

- **Retarget:** play any clip on any rig through the mannequin map, with
  rest-pose correction. Shown side by side: source rig and target rig.
- **Trim, loop, speed, mirror** a clip.
- **Layer and mask:** Slay's face over Valley's walk is one example; an
  upper-body wand cast over a run is another.
- **Key:** set a joint or face curve at a time; the lab blends to and from
  it.
- **Fix up:** lock feet that slide, aim the head or eyes, put a hand on a
  prop.
- **Transitions:** blend lengths and curves between clips, chosen per
  transition.
- **Face:** edit curves, add blinks, and drive visemes and moods from a
  voice line (`metaverse-engine/audio/speak.coax`).

### Check

| Check | Measures | Shown as |
| --- | --- | --- |
| Foot sliding | Distance a planted foot moves while its contact event is on | Centimetres per contact, red over the limit |
| Joint limits | Rotation outside a per-joint range | The joint and frame |
| Intersections | Capsule overlap between body parts, and with props | The pair and frame |
| Continuity | Jumps in pose or camera at a cut | Per cut |
| Face range | Curves outside 0–1, or mixes that exceed the face's limit | Curve and frame |
| Determinism | Frame hashes from two renders | Same or different |
| Parity | Pixel and pose difference against source frames | Per frame, for imported shots |

### See

Stations in the realm, each with a deep link, as Valley Lab does:

- `#rig`: skeleton, weights and face shapes; pick a joint or shape and see
  what it moves.
- `#clip`: scrub a clip, show ghost frames, plot joint and face curves.
- `#retarget`: source and target side by side on the same clip.
- `#layers`: one performance with each layer toggled.
- `#face`: a close-up view for face clips (Slay's source-shot view,
  generalized).
- `#scene`: the stage playing the scene, with the timeline under it.
- `#cut`: the cut playing through its cameras.
- `#compare`: our frames against a source's frames, side by side.

### Direct

Anime Lab's director, extended to these documents: spoken or written
directions become typed edits through the Decisions API with a response
schema, each with confidence.

Examples of what a direction can become:

- "She walks slower toward the gate": the base layer's speed and path.
- "Look at the figure on the wall when he speaks": a look-at fix-up timed to
  his line.
- "Hold the wand raise longer": a clip's speed or a key.
- "Cut to her face when the light flares": a new shot in the cut, aligned to
  the effect event.

A direction that maps to nothing in stock says so, as Anime Lab does now.

### Command line

New `openagents labs anim` commands, on the existing labs CLI:

```sh
openagents labs anim import SOURCE --rig NAME          # writes a rig, clips and a private pack
openagents labs anim rig NAME                          # counts, joints, shapes, maps
openagents labs anim retarget CLIP --to RIG            # a new clip on another rig
openagents labs anim edit FILE "direction or op"       # one checked change, a new version
openagents labs anim undo FILE                         # publish the previous version
openagents labs anim diff FILE                         # full changed-document JSON
openagents labs anim check FILE                        # all checks, JSON
openagents labs anim play FILE out.mp4                 # render a performance, scene or cut
openagents labs anim compare FILE --source FRAMES      # parity against source frames
```

## Example: Draconia, The Guardian

A 30-second original scene, rendered twice from one scene file: once
realistic and once in Anime Lab's cel style.

**Rights.** The pre-Nuke Draconia work began as an internal study of a fan
anime. That study stays internal. This scene is original: our character, our
academy (Skyreach Academy, Anime Lab's courtyard), our story beats. Nothing
in it copies the fan film's shots.

**Cast.**

- **Draconia:** in the realistic render, her private MetaHuman (face through
  the DNA rig); in the cel render, Anime Lab's heroine look. One rig per look,
  both mapped to the mannequin, so the same performance drives either.
- **Two hooded figures:** mannequin-rigged extras.
- **The guardian:** a summoned spirit animal made of light. It's an effect
  with its own rig, not a character with a face.

**Sources.**

| Need | Source |
| --- | --- |
| Walk, stop, turn | Valley Echo's locomotion clips, retargeted, for study renders only |
| Wand raise and cast | Hand-keyed from the pose library, then a recorded mocap take when one exists |
| Face | The owner's phone face capture of the lines (ARKit curves through the face map); Slay's face clips only for internal parity tests |
| Voice | `metaverse-engine/audio/speak.coax` with a chosen voice |
| Guardian | An effect rig keyed by hand |

Valley's and Slay's clips are Epic sample content. They're fine for the lab's
internal tests and comparisons, but a published render uses our own captured
or keyed motion and faces.

**Shots** (scene time):

| Shot | Time | Camera | What happens | Layers in use |
| --- | --- | --- | --- | --- |
| S1 | 0–5 s | Wide crane down into the night courtyard | Wind; Draconia walks in from the gate | Base walk; additive breathing; foot lock |
| S2 | 5–9 s | Medium, slow push in | She stops and turns toward the hooded figures on the wall | Base stop and turn; look-at the nearer figure |
| S3 | 9–12 s | Close-up | "Leave. Now." | Face from the capture; visemes from the voice line; head additive |
| S4 | 12–18 s | Low angle, rack focus to the wand | She raises the wand; light gathers | Override upper body with the wand raise; hand-to-wand fix-up |
| S5 | 18–25 s | Wide, orbit | The guardian forms and spreads its wings; the figures recoil | Guardian effect events; extras' flinch additives |
| S6 | 25–30 s | Close-up, then cut to black | She lowers the wand; a small breath | Face breath; upper-body override out over 0.4 s |

**What it shows.** Every layer type, retargeting across rigs, a face from a
capture, a voice line, effect events driving cuts, and the same scene in two
render styles. The checks run on it in CI: foot sliding in S1 and S2,
continuity at each cut, determinism of the full cut.

## Milestones

Each ends on a check run on our own machines.

| | Milestone | Done when |
| --- | --- | --- |
| A0 | Documents and admission | `anim.rig.v1`, `clip`, `performance`, `scene`, `cut` and `pose` have schemas and `#authority` admission with 0 surviving mutants; Echo's rig and clips convert into them losslessly (same frames as today's port) |
| A1 | Rig and clip stations | `#rig` and `#clip` show Echo and a mannequin character; curves plot; ghost frames show |
| A2 | Retargeting | An Echo clip plays on a mannequin rig and back; joint error after the round trip is reported and under a stated limit |
| A3 | Layers and fix-ups | Slay's face layered over Valley's walk through a performance file, with foot lock and look-at; the foot-sliding check reports before and after |
| A4 | Imports | glTF, BVH and ARKit face CSV imports, each with a receipt; one of each plays in `#clip` |
| A5 | Scenes and cuts | The Draconia scene file plays S1–S6 in the realm and renders to video with `labs anim play`; two renders give the same frame hashes |
| A6 | Two styles | The same scene file renders realistic and cel; the cut and timing match |
| A7 | Directing (implemented; #12972) | Ten typed directions (speed, look-at, hold, cut-on-event and others) change the Draconia scene through the director, each as one version with confidence |
| A8 | Published version | The scene uses only our own motion, faces and voice, and is ready to show |

## Issues

| Milestone | Issue | Relation to work in flight |
| --- | --- | --- |
| A0 | #12965 | Converts the Echo port (599558ee03) and the Slay face data (e7e0b0e71c) into the documents |
| A1 | #12966 | New realm; Valley Lab's J and F views stay as they are |
| A2 | #12967 | Extends `metaverse-engine/anim/mannequin.coax` |
| A3 | #12968 | Moves #12962's face layering and ambient segments onto shared layers, same frames |
| A4 | #12969 | Generalizes the Echo and Slay exporters and the `EchoSourceSDK` plugin |
| A5 | #12970 | Uses `metaverse-engine/cam/cinematic.coax` and the speaking pipeline (#12798) |
| A6 | #12971 | Uses Anime Lab's render profile and cast |
| A7 | #12972 | Extends Anime Lab's director |
| A8 | #12973 | Replaces Epic sample content with our own capture |

#12962 (Slay on Valley Lab's Echo) stays open for its exact-render work: orb deformation, flare, lighting, skin, hair. It isn't blocked by this lab.

## Decisions

Recorded 2026-11-06 while implementing the milestones:

1. **Key the original motion first.** The publishable Guardian uses our own
   keyed body and face animation. A phone or suit capture can replace it
   later, with an authoring receipt. This makes publication independent of
   hardware, a paid capture service or an owner capture session.
2. **Keep a rig per look.** The realistic and cel characters can have
   different proportions. Both map to the 65-joint mannequin and use the
   same performance, scene time and cut. A render style alone cannot change
   proportions while preserving contact targets.
3. **Use authored targets in v1.** Fix-ups play the targets recorded in the
   performance. They do not read a live clock, network state or a live
   entity. Fixed targets cover the Guardian's planted feet, figures on the
   wall and wand hold, and make captures reproducible. Following a moving
   target needs an explicit, versioned binding in a later format.
4. **Start original assets in private packs too.** Animation Lab uses
   `~/.openagents/aethyrforge/animation-lab/`. Original extras and props move
   into public packs only with a recorded source and rights review. A
   content hash alone does not establish permission to publish.
5. **Compile documents for Valley's existing consumer.** A0 lowers admitted
   rig and clip records into ECH1/EFC1, retaining the template's GPU assets
   and source-shot cameras. This lets Valley render the converted documents
   while preserving its exact frames. The backend rejects edits outside
   its fixed legacy profile; the document formats retain their wider limits.

The exact formats and admission limits are in
[Animation documents v1](anim-documents-v1.md).

A1 uses a neutral inspection material and skeleton ghosts. Its source bundle
is limited to one rig, up to 64 clips and one immutable binary of at most
64 MiB. A GPU mesh upload is at most 64 MiB. The browser and linker share
an explicit 256 MiB memory declaration for this inspector; other pages keep
64 MiB. These bounds retain the full Echo rig, all body and Slay face clips,
and all eight influences without adding the source groom or texture cost.
The frame budget is 33.3 ms. Headless phone viewport results use the Mac GPU;
physical iPhone qualification is an owner action, recorded in the workspace.

A2 uses one rotational anchor per mannequin slot. A canonical joint name,
matched without case, takes precedence over folded helper aliases. A single
custom alias is accepted; multiple aliases without a canonical anchor and
duplicate canonical anchors are refused. The mannequin has 65 slots and
cannot encode independent rotations for every joint of a larger source rig.
Unpaired target joints retain local rest and are listed in the report. The
2° round-trip limit applies to paired anchors; the report includes actual
world-rotation error for every source joint, including folded helpers.
Echo has 53 paired anchors. Its other joints do not satisfy that guarantee.

Retargeting carries world-space rotation deltas through the declared source
and target rest offsets, then solves target local rotations in parent order.
Only root translation transfers, scaled by the mean of both thigh–calf–foot
chain lengths. If either chain is missing, scale is one and the report says
leg scaling was unavailable. Other translations stay at target local rest.
Exact-name face curves transfer; unknown curves and unpaired events are
counted and omitted. An independent root track overrides body and face-bone
translation on the first root joint.

Echo has no authored contact events. A2 infers contact frames from the
source foot joint before measuring the target: height within 2 cm of that
foot's clip minimum and vertical speed at most 0.15 m/s. Height error uses
foot joint anchors relative to each rig's rest-ground height and the leg
scale. It does not measure sole clearance. The walk/jog residuals are
reported without a small-error gate; A3 adds contact fix-ups.

The native exporter reads one immutable binary per rig, up to 64 MiB each,
and emits a compact binary plus admitted rig/clip JSON, a mapping report and
a hash-linked receipt. It creates a new directory under `~/.openagents/`
and refuses overwriting an existing output or resolving its parent outside
that root. The viewer keeps A1's two-library and 64 MiB GPU limits. Two
large meshes that exceed the combined budget are visibly refused.

A3 uses smoothstep blend windows. Base and override layers replace selected
local poses; additive layers apply deltas from the rig's local rest pose.
Face layers use only facial tracks and curves. Masks inherit the nearest
mapped ancestor for helper joints; facial membership is the rig's explicit
face-bone list. Combined face weights clamp to 0–1. Valley's admitted EFC1
adapter shares the pose, fade and schedule primitives and retains its source
sampling and unclamped source weights, so its existing J/F/ambient views keep
identical frames.

Fix-ups run after layering: feet, then head/eye aims, then hand/reach targets.
Two-bone IK preserves each frame's bone lengths and reports unsupported or
unreachable targets. A shared pelvis can lower by at most 15 cm to keep a
plant reachable; the swinging foot keeps its incoming world position.
A foot-up event starts an authored release blend. Look-at rows name the head
or eye joints explicitly, use the rig's rest axes with character-forward +Z,
and limit head-anchor corrections to 45° and other aim joints to 20°.
Authors supply eye-joint rows. Targets are in character space.

The private study recipe infers contacts before choosing targets. It uses
A2's height/speed thresholds, excludes the base layer's 0.15-second intro
and outro blends, and merges gaps shorter than the 0.15-second release.
This prevents a new plant from overlapping the previous release. All eight
resulting contacts are measured at intervals of at most 1/120 second, including
cumulative travel and a sample just before foot-up. Foot lock must
stay under 1 cm per contact. Blend checks compare poses 0.1 ms either side
of every layer/fade boundary: limits are 0.25° and 0.25 cm. These finite-step
limits include finite motion around near-straight IK knees.
The measurements and exact source identities are in the A3 record.

The layers station has individual selection/toggles and five kind-group
shortcuts. It loads a performance file plus one annotated clip beside
its private bundle. The bundle keeps its public source-clip count and uses
one spare slot for the derived clip, so this station accepts at most 63
source clips. Rig/clip inspection retains the 64-clip bound. Pending JSON
views refer to fixed host-owned buffers; admitted names and values are owned.
Each JSON admission uses one bounded scratch arena and a separate ABI entry.
Rig views avoid copying whole skeletons into the frame stack. These choices
kept A3 inside the existing 256 MiB memory and 4 MiB stack declarations.
A5 raises the stack to 8 MiB for staged project admission.

## A4 import decisions

The first importer uses bounded, admitted source profiles rather than guessing
missing units, unsupported transforms or timecode rates. CSV defaults to an
explicit 60 fps timecode clock; BVH uses centimetres and Y up. Unknown fields
that change the supported geometry or animation representation are refused.
The profile, SDK behavior and diagnostic geometry are specified in
[Animation imports v1](anim-imports-v1.md).

The generalized Unreal exporter reproduces Echo mesh data, but regenerated
body samples fail exact A0 equality. Keep the frozen A0 conversions as the
qualified source. Do not substitute regenerated clips in Valley Lab. This
unmet A4 qualification is recorded in the verification record and #12969;
#12962 remains open for its separate exact-render work.

## A5 scene and cut decisions

A5 uses retained ALP1 projects: whole admitted libraries, performance
records, a scene, a cut and optional SAT1 speech/audio entries. Staging
occurs in the inactive source bank; publication follows complete admission.
The realm and CLI sample the same scene clock. No solved frame feeds the
next one. Held-frame messages use authored frame numbers rather than wall
time, so frame hashes can be compared across complete renders.

Keep the initial renderer bounded: studio/courtyard, two libraries, 16
performances, 32 voice cues and 64 draw commands. Canyon and image crossfades
are refused. Hard cuts and incoming fades are implemented. The headless
profile captures 960 × 540 at the edit rate, up to 60 seconds per invocation;
this keeps capture and PCM memory bounded. Native audio is mixed from edit
clocks, then muxed after the Metaverse frames have been captured. Live seeks are
silent rather than restarting a cue at an incorrect offset.

The Guardian is an internal blocking study. It uses the qualified Valley
walk on the original mannequin, original arm/recoil keys, a hand-bound wand,
original props and bird geometry, and an offline stock voice with
speech-aligned jaw motion. Stage weather/time and camera focus remain
metadata until the finished A6 rendering work. The study is not the A8
publishable scene. The [scene profile](anim-scenes-v1.md) records these
limits and the [recipe](../../metaverse/animlab/guardian/README.md) reproduces it.

JSON diagnostics name their models: canonical elbow/knee angle screening
and upright character capsules. Rig v1 does not declare per-joint limits,
self-collision or prop collision shapes. Reports include these boundaries,
raw face ranges, every cut's pose/camera jumps, planted-foot travel and
CPU determinism. GPU determinism is recorded by complete film captures.

Staged JSON and library readers need an 8 MiB Wasm stack. Retained project
banks and GPU palettes remain inside the existing 256 MiB memory ceiling;
source and geometry windows stay at 64 MiB. A5 changes no Valley Lab source
or behavior, and leaves #12962 and the A4 qualification issue #12983 open.
