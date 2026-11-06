# Animation imports, version 1

`openagents labs anim import SOURCE --rig NAME` writes admitted rig and clip
JSON, a SHA-256-addressed binary, `library.bundle` and `receipt.json`.
The default output is a new directory under
`~/.openagents/aethyrforge/animation-lab/imports/`. Use `--out PRIVATE_DIR`
to choose another new directory below `~/.openagents/`. Existing directories
are refused. `--rate 30|60` selects the output clock for glTF and the timecode
clock for CSV. BVH declares its own rate and refuses `--rate`.

Parsing, pose conversion, interpolation, skin quantization, admission and
receipts are Coax. Blender and Unreal only read and export source data. They
run headless, without audio. Verse renders all inspection frames.

## Inputs and limits

| Input | Supported profile | Named refusals include |
| --- | --- | --- |
| `.gltf`, `.glb` | glTF 2.0; one buffer and one skin; indexed triangles; up to eight influences; dense, interleaved and sparse accessors; position/normal morphs; LINEAR, STEP and CUBICSPLINE animation | Required extensions, node matrices, nonunit scale, extra influence sets, tangent morphs, invalid indices, malformed accessors and duplicate animation channels |
| `.fbx` | Blender imports FBX and exports separate glTF, all actions, forced sampling, skins and morphs; the same glTF reader then admits it | Missing SDK, SDK failure, or an unsupported exported glTF feature |
| `.bvh` | Hierarchy, offsets, declared channels and motion; source centimetres and Y up; source rotation order; translations before rotations | Truncation, repeated channels, invalid hierarchy, unsupported channel order, nonfinite values and inconsistent frame counts |
| `.csv` | Wide Live Link Face CSV: `Timecode`, `BlendShapeCount`, all 52 ARKit names, `HeadYaw`, `HeadPitch`, `HeadRoll`; optional six eye-angle columns | Missing or duplicate columns, unknown names, incorrect counts, nonincreasing clocks and out-of-range values |
| `.unreal.json` | `anim.unreal.source.v1`, a source project, one skeletal mesh and named body sequences; generalized read-only SDK export, followed by the glTF reader | Unavailable SDK/project, unsupported profile, missing assets, incompatible sequence joint names and the glTF profile limits |

Source files and binaries are bounded at 64 MiB. There are at most 512 rig
nodes, 64 clips, 64 sections/materials, 256 shapes/sparse descriptors, and
2,000 frames per clip. glTF accessors have at most 1,000,000 elements.
BVH steps range from 1/240 to 1 second and clips span at most 600 seconds.
The final binary and bundle each fit 64 MiB. Oversize inputs are refused;
partial data is never published as a library.

The inspector keeps neutral material slots rather than importing textures.
BVH and CSV receive original diagnostic geometry so they can play without
another character pack. BVH aliases include the common Hips/Spine/LeftArm/
LeftUpLeg naming, plus the mannequin names. Namespaces are not inferred.
Use A2 retargeting to put a BVH clip on an admitted character.

CSV curve values are in [0, 1]. Head and optional eye angles are radians in
[-pi, pi]. Head rotation composes yaw about Y, pitch about X and roll about
Z. Optional eye columns are validated and counted; eye motion uses the
52 named curves. The timecode rate defaults to 60 and can explicitly be 30;
it is never guessed from timestamps. Missing source rows interpolate onto
that grid. CSV does not replace a real character's shape or DNA mapping.

Animated glTF scale must stay within 2^-16 of one, with cubic scale tangents
within 2^-16 of zero. This accommodates SDK float noise without adding scale
to the rigid pose model. Cubic quaternion interpolation refuses a squared
norm below 2^-16 before normalization. Source endpoints are retained; output
rate can differ slightly from the requested rate to land on the endpoint.
A 10-microsecond grid tolerance avoids an extra frame from SDK float noise.
Blender's exported source range is retained, including any SDK-added lead
frame; the importer does not silently trim it.

## Unreal SDK

Build the trusted plugin in `aethyr/tools/unreal-echo-sdk/` with the installed
Unreal SDK. Point `OPENAGENTS_UNREAL_SDK` at the compiled plugin directory.
`OPENAGENTS_UNREAL_EDITOR` and `OPENAGENTS_BLENDER` override SDK executable
paths. They are explicit local overrides, not fields supplied by a source.

A minimal source plan is:

```json
{
  "schema": "anim.unreal.source.v1",
  "project": "/absolute/private/project/Source.uproject",
  "profile": "generic",
  "meshes": [{"asset": "/Game/Characters/Body", "name": "body"}],
  "animations": [
    {"asset": "/Game/Animations/Idle", "name": "idle", "face_only": false}
  ]
}
```

The adapter creates its own data-only project. It links the source Content
and the trusted plugin, without loading source project modules or saving
source assets. SDK logs and temporary exports stay private. It waits for its
owned editor PID, not pipe EOF from Unreal's background helpers.

The lower-level `aethyr/tools/anim-unreal-export.coax` takes this plan, a
compiled plugin path and a new private directory. It can export up to eight
meshes and 64 body or facial sequences by asset path, including raw sampled
curves and bones. The ordinary import profile currently accepts one mesh
and body clips. Facial SDK samples and multi-mesh compatibility conversion
remain separate from the ordinary glTF import path. Their refusal is
explicit, before publishing a pack.

### Echo and Slay compatibility limit

A4's exact A0 re-import qualification is **not met**. The generalized SDK
reproduces all 7,602 Echo morph accessor byte spans and the consumed mesh
attributes, but the regenerated Idle animation differs in 132 of 134
translation channels and all 134 quaternion channel byte arrays. Scale and
time channels match. Running the SDK through editor Python instead of a
commandlet does not remove the difference. Raw and compressed-ready sampling
were both checked; neither reproduces the frozen A0 body bytes.

The existing A0 conversions remain the qualified Echo/Slay source. A4 does
not replace them, alter Valley playback, or claim equality from a tolerance
comparison. The `echo-a0` export plan profile is diagnostic; the ordinary
import command refuses it with
`unreal-profile-requires-compatibility-export`. Full Echo/Slay re-import
requires resolving the SDK sampling difference and a streaming/multi-mesh
compatibility path, tracked in [#12983](https://github.com/OpenAgentsInc/openagents/issues/12983). The original Echo binary is about 457 MB, beyond the
ordinary reader's 64 MiB source limit.

## Publication and receipts

Native imports stage generated documents and the pack in an owned private
directory. They re-admit every rig, clip and ALB1 bundle before atomically
renaming the result into the requested output. Canonical parent resolution
and the shared output policy confine publication below `~/.openagents/`.
Bad input, an existing destination or failed admission publishes nothing.
SDK staging may be created during conversion and is removed on failure.

Receipts record source and converted hashes, output hash, frames, joints,
vertices, triangles, shapes, clip counts, rate, units and adapter settings.
Unreal body imports additionally record each source asset and the hashes of
its exported glTF and binary. Keep receipts with their packs. Licensed
payloads and captures must remain private; public verification uses counts,
aggregate errors and hashes.

## Inspect an import

Expose its `library.bundle` as the owner-local web alias
`~/.openagents/verse/private-build/web/animation-lab.bundle`. In the local
Animation Lab realm, tap **PRIVATE**, then **CLIP**, or open
`#clip?rig=NAME`. PRIVATE selects the actual loaded rig; it is not hardcoded
to Echo. The camera fits both source joint pivots and bind-space vertices.
Use PLAY, PAUSE, step arrows and the timeline to inspect motion.
