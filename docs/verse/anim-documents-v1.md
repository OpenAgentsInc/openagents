# Animation documents v1

Animation Lab uses six JSON document types. The readers in
[`aethyr/anim/`](../../aethyr/anim/) admit documents into bounded, owned Coax
records. Admission returns a named error and leaves the output unchanged
on failure. A document never supplies an executable path, shader or script.

These formats describe animation data. They do not grant permission to use
an asset. Follow [the private asset policy](private-assets.md) separately.

## Common rules

Every document has a `schema` and a `name`. Names contain 1–64 ASCII letters,
digits, underscores, hyphens or periods. `.` and `..` are invalid. Names are
case-sensitive. Lists of named items have no duplicates. Unknown keys and
duplicate decoded JSON keys are errors.

A document is at most 262,144 bytes and 32,768 JSON nodes. Decoded strings
are valid UTF-8, contain no control characters and fit in 4,096 bytes;
individual fields can have smaller limits. JSON numbers have at most 40
source bytes and a decimal exponent between −38 and 38. NaN and infinity
are invalid, including in binary float tracks. Times are seconds, distances
are metres, angles are radians, and rotations are `[x,y,z,w]` quaternions.
The compiled FRG1 block is an adapter exception: its evaluator retains the
source rig's centimetres and Euler degrees internally.
Quaternion squared length must be strictly between 0.99 and 1.01. Readers
preserve the supplied floats; they do not normalize them during admission.

All fields below are required unless marked optional. Empty lists are
allowed where a minimum is not stated. Documents reference other admitted
documents by name. The caller supplies those records as the catalogue.

### Binary parts

Large arrays stay in binary packs. A part has this shape:

```json
{"pack":"<64 lowercase SHA-256 digits>","offset":0,"count":2,"stride":28,"width":28}
```

`offset`, `count`, `stride` and `width` are plain JSON integers. `count` and
`width` are positive; `stride` is at least `width`. Every record from
`offset` through `offset + (count - 1) * stride + width` must fit in the
identified pack. All words are little-endian. Floats are IEEE 754 binary32.
Parts can reference different packs in the same document.

Before admission, the caller verifies each pack with `anim_pack_valid`.
Packs contain 1–1,073,741,824 bytes. The caller owns their storage and keeps
it immutable while an admitted document or player uses it. Readers retain
part descriptors, not pointers into temporary JSON or caller-local copies.
The SHA-256 establishes content identity, not provenance or licence rights.
The admitted snapshot also retains the catalogue order and identities used
for pack, rig, clip, performance and scene indexes. Replace a snapshot as a
whole when importing or editing; do not reorder its catalogues during play.

### Errors

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | `ok` | Admission succeeded |
| 1 | `json` | Malformed JSON, duplicate keys or invalid string encoding |
| 2 | `schema` | Wrong schema, unknown key, or incompatible field combination |
| 3 | `limit` | A count or document size exceeds the format's bound |
| 4 | `name` | Invalid or repeated name, or an unknown enumerated name |
| 5 | `number` | Invalid number, time, quaternion or nonfinite sample |
| 6 | `pack-range` | Missing pack, invalid part, or inconsistent track layout |
| 7 | `skeleton` | Invalid parent or a cycle |
| 8 | `reference` | A named rig, clip, shape, joint or camera is unavailable |
| 9 | `capacity` | The caller supplied no output slot |
| 10 | `format` | An unsupported binary or legacy composition format |

## Rig: `anim.rig.v1`

Reader: `anim_rig_load` in `rig-document.coax`.

| Field | Shape and limits |
| --- | --- |
| `joints` | 1–512 objects with `name`, `parent`, `mannequin`, `offset` |
| `rest` | Binary part: one 28-byte local pose per joint, translation then quaternion |
| `bind` | Binary part: one 48-byte, row-major inverse bind matrix per joint |
| `sections` | 1–64 objects with `format`, `vertices`, `indices`, `material` |
| `materials` | 1–64 slot names |
| `shapes` | Up to 256 blend-shape names |
| `controls` | Up to 1,024 DNA control names |
| `face_bones` | Up to 64 unique skeleton joint names |
| `face_map` | Up to 52 ARKit input mappings |
| `dna` | Optional binary part containing one compiled FRG1 face-rig block |
| `morphs` | Optional list of up to 256 sparse shape bindings |

A parent is −1 for a root or an earlier joint's index. This ordering rejects
cycles before playback. Multiple roots are allowed, including groom chains.
`mannequin` is one of the OpenAgents Metaverse Engine's 65 mannequin names, or `null` for an unmapped
joint. `offset` is a quaternion applied by retargeting.

A section's `format` is `skin8.v1`. Each vertex record is 96 bytes:

| Offset | Data |
| --- | --- |
| 0 | Position, three floats |
| 12 | Normal, three floats |
| 24 | UV, two floats |
| 32 | Material word |
| 36 | Eight unsigned 16-bit joint indices |
| 52 | Eight unsigned byte weights, positive sum |
| 60 | Tangent, four floats |
| 76 | Five retained source/GPU words |

The vertex and index parts have widths 96 and 4. A section has at most
1,000,000 vertices and 1,000,000 triangles. Indices are unsigned 32-bit
indices relative to that section's vertex part. All eight influence indices
must name existing joints, including influences with zero weight. Playback divides each weight by their
sum, preserving quantized source weights without changing their bytes. Its
`material` names an entry in `materials`.

Each face-map entry has `input` (an ARKit shape name) and 1–8 `outputs`.
An output has `kind` (`shape` or `control`), `name` and `weight` (0–1).
The target must exist in the matching table. An input cannot repeat, and
an input cannot name the same output twice. One input can drive shapes and
controls together.

A morph binding has `shape`, `section` and `data`. `shape` names an entry
in `shapes`; `section` indexes `sections`. Each shape/section pair appears
at most once. The data part uses 32-byte records: unsigned vertex index,
unsigned shape index, three position-delta floats and three normal-delta
floats. Vertex indexes fit the section's vertex part and every shape index
matches the binding. A shape with no deltas has no binding. This layout
retains the EFC1 sparse records without changing their bits.

Compiled DNA admission bounds every FRG1 section before reading it, checks
all control, joint and point references, and rejects nonfinite coefficients.
It allows up to 1,024 controls, 1,024 face joints, 1,000,000 points,
1,000,000 vertex references, 256 groups, 2,048 targets and 32 expressions.
The existing face-rig evaluator consumes the admitted block.

## Clip: `anim.clip.v1`

Reader: `anim_clip_load` in `clip-document.coax`.

| Field | Shape and limits |
| --- | --- |
| `rig` | Admitted rig name |
| `frames` | 2–2,000 |
| `rate` | 0.001–240 frames per second |
| `length` | 0.001–600; equals `(frames - 1) / rate` within 0.0001 seconds |
| `loop` | Boolean |
| `joints` | Unique rig joint names, in track order |
| `body` | Required when `joints` is nonempty; otherwise absent |
| `curves` | Unique rig shape names, in track order |
| `face` | Required when `curves` is nonempty; otherwise absent |
| `face_bones` | Unique members of the rig's `face_bones` |
| `face_pose` | Required when `face_bones` is nonempty; otherwise absent |
| `root` | Optional translation track |
| `events` | Up to 256 ordered events |
| `source` | `pack` and `receipt` SHA-256 strings, and a `role` name |

At least one body, curve or face-bone track is required. Body and face-bone
parts contain `frames * tracks` local poses, frame first and then track.
Each pose has three translation floats and a quaternion, width 28. Face
parts contain `frames * curves` floats, width 4. Root parts contain `frames`
translations, width 12. Source face values are retained without clamping;
the face-range diagnostic reports values outside 0–1 during playback.

Events contain `at`, `kind` (`foot-down`, `foot-up`, `contact`, `cast`) and
`joint`. Times are nondecreasing and within the clip; the joint must exist.
The source receipt can identify an external source that is not loaded for
playback. Sample parts must identify loaded, verified packs.

## Performance: `anim.performance.v1`

Reader: `anim_performance_load` in `performance-document.coax`.

`rig` names an admitted rig. `length` is 0.001–600 seconds. `layers` has
1–32 entries. Every layer contains:

| Field | Values |
| --- | --- |
| `kind` | `base`, `override`, `additive`, `face`, `fix-up` |
| `start`, `end` | Within the performance, with `end > start` |
| `speed` | 0.01–16 |
| `blend_in`, `blend_out` | Nonnegative; their sum fits the layer's duration |
| `weight` | 0–1 |
| `mask` | `all`, `face`, `head`, `upper`, `arm_l`, `arm_r`, `hands`, `legs` |

There is exactly one base layer. It comes first and uses `all`. Ordinary
layers name a `clip` admitted for the same rig. Fix-ups follow all ordinary
layers and contain no `clip`. Instead they name `solver` (`foot-lock`,
`look-at`, `hand-to-prop`, `reach`), a rig `joint`, and a three-float `target`
whose coordinates are within ±1,000,000 metres. Ordinary layers cannot
contain solver fields. This format records the target; A3 implements the
solvers and their measurements.

## Pose: `anim.pose.v1`

Reader: `anim_pose_load` in `pose-document.coax`.

`rig` names an admitted rig. `joints` contains up to 65 `{name, rotation}`
entries using the mannequin's joint names and unit quaternions. `face`
contains up to 256 `{name, value}` entries using this rig's shape names and
values from 0 to 1. Names in each list are unique, and the two lists cannot
both be empty. A pose is sparse: an absent joint or curve has no edit.

## Scene: `anim.scene.v1`

Reader: `anim_scene_load` in `scene-document.coax`.

`length` is 0.001–600 seconds. `style` is `realistic` or `cel`. Positions are
three floats within ±1,000,000 metres. Every key list is strictly ordered
by `at`, within the scene's duration. Ordering and positive intervals must
remain valid after conversion to the stored binary32 clock.

| Field | Shape and limits |
| --- | --- |
| `stage` | `realm`: `animation-lab`, `anime-lab`, `valley-lab`; `place`: `studio`, `courtyard`, `canyon`; `time`: 0–1; `weather`: `clear`, `wind`, `rain`, `snow` |
| `cast` | Up to 16 `{name, rig, look, performance, path}` entries |
| `props` | Up to 64 `{name, rig, position, rotation, scale}` entries |
| `effects` | Up to 64 `{name, kind, start, end, position, size, seed}` entries |
| `lights` | Up to 16 `{name, keys}` entries |
| `cameras` | 1–16 `{name, fov, focus, keys}` entries |
| `sounds` | Up to 32 sound cues |

A cast entry's rig and performance must exist and agree. The performance
covers the scene's duration. `look` is `default`, `heroine`, `hooded` or
`guardian`. A path has 1–32 `{at, position, yaw}` keys; yaw is within
±1,000 radians. A prop uses a unit `rotation` quaternion and a `scale` from
0.001 to 1,000.

Effect kinds are `guardian`, `flare`, `wind`, `smoke`, `light` and `embers`.
Their time range is positive and inside the scene; `size` is 0.001–1,000,
and `seed` is an integer from 0 to 2,147,483,647.

A light has 1–16 `{at, position, colour, intensity}` keys. Colour channels
are 0–64 and intensity is 0–10,000. A camera has 2–16
`{at, eye, look, cut, ease}` keys. `eye` and `look` must be at least 0.0001
metres apart. `cut` is a
boolean; `ease` is the Metaverse Engine's easing index 0–3. `fov` is 0.1–3 radians and
`focus` is 0.01–1,000,000 metres.

A sound cue has `name`, `kind` (`voice`, `music`, `effect`), `start`, `end`
and `gain` (0–4). A voice cue also has a `cast` name, `voice` name and
1–256 UTF-8 bytes of `words`; it cannot have a `source`. Other cues have a
`source` binary part with `count:1` and no voice fields. Audio decoding is
a separate playback step.

## Cut: `anim.cut.v1`

Reader: `anim_cut_load` in `cut-document.coax`.

`scene` names an admitted scene. `fps` is an integer from 1 to 120. `shots`
has 1–128 entries, each with `name`, `camera`, `from`, `to`, `transition`
and `blend`. Shot names are unique. `camera` names a scene camera; times
select a positive interval inside the scene. The cut is at most 600 seconds.

`transition` is `cut`, `fade` or `crossfade`. A cut uses zero blend; the
others use a positive blend shorter than the shot and its predecessor.
The previous source interval also has room for the next shot's blend.

The adapters `anim_cut_lines` and `anim_cut_json` read the existing
[`.cut` lines and `cut.v1` JSON](../cli/cut-format.md). A scene camera is
written as `src=camera:NAME`. The admitted record keeps the complete media
composition, including size, audio, fades and titles. Unsupported legacy
fields produce a named refusal rather than disappearing. The general
`labs cut` media-file workflow continues to use the same shared parser.

## Echo and Slay conversion

The existing ECH1 and EFC1 readers remain source adapters. Valley Lab uses
the shared ECH1 reader in `aethyr/anim/echo-pack.coax`; A3 moves its layering
onto performances. The source formats and their pose arithmetic are unchanged.

Convert the existing private exports with:

```sh
coax/web/tool.sh aethyr/tools/anim-echo-documents.coax ECHO_DIR FACE_DIR OUTPUT_DIR
```

The output contains an admitted rig, 24 body clips, 20 face clips, an export
receipt and a binary file named by its SHA-256. The binary retains the
original rig, face, vertex and index bytes. The converter checks every
admitted rest/bind float, body and face sample, and sparse morph record
against those original bytes. It does not resample or clamp motion.

Keep the output in the private Animation Lab cache. The receipt records
source-part hashes and offsets; the rig and clips reference the resulting
binary by hash and range. Texture and groom rendering remain in the source
adapter until the corresponding lab stations adopt them.
The receipt also records the converter version, source sampling and chosen
loop defaults; ECH1 does not encode a loop flag. Those defaults can be
overridden during playback without changing the source samples.

Compile the admitted documents for Valley's existing GPU consumer with:

```sh
coax/web/tool.sh aethyr/tools/anim-echo-documents.coax --compile DOCUMENT_DIR TEMPLATE_VPK OUTPUT_VPK
```

This reads and admits the rig and all 44 clips again, then emits ECH1 and
EFC1 from their typed records and referenced samples. It rebuilds rest poses,
bind matrices, hierarchy, body samples, face names, morph records, curves and
facial bone samples. The template supplies scenery, textures, groom, GPU mesh
preparation, source-shot cameras and locomotion reference speeds. It does
not supply the rebuilt animation samples. A digest covers the receipt, rig
and clip documents in that order.

The compatibility backend keeps Valley's 24 body roles, 30 Hz body clock,
track order, loop defaults and catalogue sizes. It rejects events, independent
root tracks, face loops and incompatible track tables. These are backend
limits, not document admission limits. New lab playback can use the complete
document model without this legacy backend. The unchanged converted source
produces the exact original VPK bytes; Valley can load that emitted private
package directly.
