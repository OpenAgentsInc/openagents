# Animation Lab A4 verification

The import command uses shared Coax readers and publishes admitted rig/clip
JSON, a SHA-addressed binary, ALB1 and a receipt. glTF, BVH and CSV inputs
are made directly in Coax; the FBX and Unreal fixtures are our original
animated triangle and single bone. No licensed source or captured image is
committed. SDK exports and browser captures stay below
`~/.openagents/aethyrforge/animation-lab/a4-imports/`.

## Native imports

The production path is exercised by
`aethyr/tools/anim-import-native-check.coax`. Each valid input is admitted,
then an attempt to overwrite its output must fail. Each malformed source
must return a named reason and leave its destination absent.

| Original source | Clips | Joints | Frames | Binary bytes | Binary SHA-256 |
| --- | ---: | ---: | ---: | ---: | --- |
| glTF | 1 | 2 | 31 | 2,344 | `2d7aed266ca68cc36536313bd01373536b54fbf4518c070e2e50f61f13a25e79` |
| BVH | 1 | 2 | 2 | 2,088 | `b3ec19460b24ece0bb1568a386c70dbe133b726c3248661f2dd3a996ff3a616e` |
| ARKit CSV | 1 | 1 | 3 | 3,360 | `957f36af0bd8db4adbe67168660d2bf5aa637ecd667bdfa3be348f172526b124` |
| FBX through Blender | 1 | 3 | 32 | 3,216 | `2997f1d535ff2c1f75d544ac0efb0f0948f59e274983f9af53f75feaeea220b5` |
| Unreal body sequence | 1 | 3 | 31 | 3,300 | `97f9eac4d2e43fe115b32dcb285f174fe455f87242209bcd073cda6077771d7f` |

FBX retains Blender's exported range, including its lead frame. Unreal's
original fixture is imported into a private data project with a 0.01 unit
scale, then read by the generalized SDK. Its receipt retains source asset
paths, exported glTF/binary hashes and settings. The trusted SDK plugin
build passes for Mac arm64 and x86_64. The qualified native library SHA is
`aa592b879d8867879908a76fa2c5c08dbc34e324628b35951a8a158911c3a5ef`.
The SDK is Unreal 5.8.3; Blender is 5.2.2 LTS. Neither renders a lab frame.

The independent packed-geometry byte oracle has SHA
`bf4ce9c1d3deecbafd48fc8c41f545b65fb3cbdaf0a1e33503d1440f36c957c4`.
It covers every byte of the original glTF geometry, including padding,
eight influences, inverse binds, indices and sparse morph records.

## Browser playback

`coax/web/anim-import-frame-check.coax` launches muted headless Chrome,
loads an owner-local private bundle, opens the actual `#clip` station,
seeks source frames, captures GPU pixels, restores frame zero and measures
30 complete held steps. No visible browser is used. The original glTF and
CSV captures were visually inspected. Licensed Echo captures were qualified
by probes and hashes, without exposing their pixels to an AI tool.

All four 1280 × 800 runs report no browser errors. Each first-frame hash is
reproduced exactly after seeking through the other checkpoints.

| Source | Checkpoint frames | p95 step, ms | Frame-zero GPU SHA-256 |
| --- | --- | ---: | --- |
| glTF | 0, 15, 30, 0 | 3.16 | `656b96d1efd2ca4de8ddde69c43ce64c4171e876c4a4f24ea16e626c4078780a` |
| BVH | 0, 1, 1, 0 | 1.11 | `9eea39dc428f0dde9acbf31fafe230818aed77fd1fd9d567a29e1ac4eb401074` |
| CSV | 0, 1, 2, 0 | 1.00 | `bdd2dd1f8dec852fc6550c0ee0364acf8dd0d8fa2dd6daf5deb6208205b56c6e` |
| BVH on Echo through A2 | 0, 1, 1, 0 | 3.44 | `6b1085eec784fb32cbcbc5a78e0c89b267dd3cd6b4e8c17f7115e3cdcf641e78` |

The BVH-to-Echo CLI export has 334 joints, two paired anchors, no dropped
curves/events and a 54,085,872-byte binary. Its admitted ALB1 is 54,176,102
bytes, SHA
`2592171c0095c714851f70a1585dd1a4f701120ac76155f6c22096a74c464684`.
This proves the imported BVH reaches the existing A2 path and A1 renderer.

The viewer now selects the loaded private rig by its actual name. It fits
bind-space geometry as well as joint pivots, so geometry away from a
single-bone origin remains visible. Narrow header labels no longer run
together. Its 256 MiB memory and 4 MiB stack declarations are unchanged.
The captured Wasm is 270,982 bytes, SHA
`8532b7eb79709731d53bc0593211ecf2782b47d66cca35c68f60ad8c5df6a40b`.

## Admission checks

The source tests cover malformed/truncated input, inclusive limits,
nonfinite numbers, duplicate fields/channels, hierarchy cycles, each source
clock, sparse and interleaved accessors, all supported integer widths,
normalization, cubic tangents, quaternion indexing, independent geometry
fields, output windows and exact packed-byte oracles.

Qualified root mutation gates, with no equivalent waivers:

| Unit | Killed | Survived |
| --- | ---: | ---: |
| BVH | 291 | 0 |
| Common source names | 25 | 0 |
| CSV | 492 | 0 |
| glTF accessor | 279 | 0 |
| glTF scene | 707 | 0 |
| glTF animation | 467 | 0 |
| Container/URI policy | 88 | 0 |
| Unreal source plan | 169 | 0 |
| CLI grammar | 139 | 0 |

Unchanged gate results are retained from their passing A4 revisions; they
are not rerun simply to confirm. The scene and animation shard records merge to 707/707 and 467/467 killed.
Across the nine units, 2,657 mutants are killed with zero survivors. The
station suite passes all 282 checks. `tools-check` reports 1,843 tracked
tool files and zero new files without a firm reason. The native CLI and
SDK both build; the built CLI imports glTF, FBX and the original Unreal
sequence successfully. CLI SHA-256:
`e041598d95b7eaebc1c9469475da019190e255e2fd82ec66bf67af8ef3d40c5a`.

The Valley producer has 64 source dependencies; none is changed by A4. Its
A3 exact-frame result is retained rather than rerunning unchanged code.

## Unmet exact Unreal re-import qualification

The generalized SDK reads a mesh and animation list by asset path and emits
both glTF and raw curve/bone samples. Its original fixture imports end to
end. **The Echo/Slay equal-to-A0 Done when item is not met.**

Echo's 21 primitive attribute/index spans, inverse binds and all 7,602 morph
accessor spans match the frozen source bytes. Regenerated Idle clocks and
scale channels also match, but 132 of 134 translation channels and all 134
quaternion channel byte arrays differ. Raw-data-model sampling has a
measured maximum orientation difference of 0.0887658507 degrees from the
frozen export; quaternion signs also differ. Compressed-ready sampling and
an editor-Python call through the same native exporter do not restore exact
bytes. No tolerance comparison is reported as exact equality.

Full Echo source geometry is about 457 MB and uses separate body/groom
meshes. The ordinary v1 reader is one skin with a 64 MiB source binary.
It refuses multi-mesh `echo-a0` compatibility plans and SDK facial sample
conversion explicitly. These limits and the failed equality check are
recorded in [the import profile](../../verse/anim-imports-v1.md) and #12969.
The outstanding qualification is tracked by [#12983](https://github.com/OpenAgentsInc/openagents/issues/12983).
The lower-level generalized SDK still exports body and facial sources.
The qualified A0 Echo/Slay packs are retained; A4 does not replace them or
change Valley behavior. #12962 stays open.

## Owner and release scope

No production deploy was performed. Physical iPhone playback remains an
owner check in AtlantisPleb/workspace `NEEDS_OWNER.md`, commit
`e53c9ab27b`. It names Animation Lab, PRIVATE, CLIP, PLAY/PAUSE, step arrows,
timeline and the BVH-on-Echo follow-up. Desktop Metal captures do not
qualify an iPhone GPU.
