# Animation Lab A2 verification

Issue [#12967](https://github.com/OpenAgentsInc/openagents/issues/12967).
Shared retargeting is in `aethyr/anim/retarget*.coax`. The unlisted realm's
`#retarget` station draws source and target on one source clock. Green bones
are paired anchors; purple bones are unpaired. Source and target can be
reversed. Both meshes use the same eight-influence and sparse-morph upload.

## Mapping and accuracy

`canonical-anchor.v1` selects one rotational anchor per mannequin slot.
Canonical names take precedence over folded helpers. Ambiguous aliases,
unknown slots and cyclic or forward parents are refused before publication.
Rotations carry world-space deltas through the source and target rest axes.
Unpaired target joints keep local rest. Root displacement scales by the
mean of the two thigh–calf–foot chains, or reports a scale-one fallback if
those chains are absent. Non-root translations remain at local rest.

All **24 Echo body clips**, **2,696 frames**, were measured after retargeting
to the mannequin and back. Each private report row includes every source
joint's declared slot, paired-anchor status and maximum world rotation error.
Of 334 source joints, **53 are paired rotational anchors**. Their maximum
error is **0.000036860038°**, below the stated **2°** limit.

The 65-slot mannequin cannot encode all independent helper and facial joint
motion of the 334-joint rig. Those unpaired joints are reported and are not
covered by the 2° gate: their maximum residual is **89.192986°**. This is an
explicit limit on the issue's mapped-joint criterion, not a claim that all
joints with a fold alias round-trip. No joint errors were removed from the
report to pass the gate. The mapping decision and reason are in the spec.

The full private report SHA-256 is
`5b536badb8bf5d07e1cf68f3f713e7df55cd06342430701ef51cd673a5e9bccb`.
It remains under `~/.openagents/aethyrforge/animation-lab/a2-retarget/`.
No source payloads, joint-name reports or captured images are committed.

## Contact heights

Echo's converted clips have no authored contact events. Before looking at
target error, the check infers a source contact when the foot joint is within
2 cm of its clip minimum and has vertical speed at most 0.15 m/s. The error
is target foot-joint Y minus the sum of target rest-ground height and the
leg-scaled source height above its own rest ground. These are joint anchors, not soles.

| Clip index | Left/right contact frames | Maximum left/right error, cm |
| --- | --- | --- |
| Walk, 1 | 39 / 38 | 7.449475 / 7.7521815 |
| Jog, 2 | 1 / 1 | 8.683677 / 11.331922 |

These residuals are reported without a low-error gate. A3 adds contact fixes.
The jog's two qualifying frames are a narrow inferred sample, not an authored
stance annotation or a general foot-clearance guarantee.

## Native export

The actual native `openagents labs anim retarget CLIP --to mannequin --out
PRIVATE_DIR --json` command emitted **42 frames**, **65 joints**, **53 paired
anchors** and a **140,660-byte** compact binary. All generated JSON is
re-admitted before writing. Rig, clip, mapping report and hash-linked receipt
stay in a new directory below the canonical `~/.openagents/` root. Existing
outputs and unknown CLI flags are refused. A process lock serializes writers
in one output parent. Failed admission cannot publish a new clip.

| Output | SHA-256 |
| --- | --- |
| Binary | `b233d9933370754067b7d44cbe675384142f4a338d8729494b32a6031b5672ad` |
| Clip JSON | `e47218324fd69840e4983dc59c0c3c744be80512d9116b1f2d5c9b0819a843ae` |
| Receipt | `c66acda474707c4e9da0993e64e6589847197c0ed0080e238f7bc550399a1be6` |
| Re-admitted ALB1 | `9c029e0ea8603056c79246c46c0541417c15f2ae19c2c2f357b955ba2c0a96f3` |

The ALB1 is **148,701 bytes**, one rig and one clip. Native input is bounded
to one binary of at most 64 MiB per rig. Output is also at most 64 MiB. Three bounded 64 MiB byte banks keep the
native exporter within the CLI host stack; the initial 128 MiB banks exceeded
that stack after adding export locking and were reduced before release.
Face curves transfer by exact name; unknown curves and unpaired events are
counted and omitted. Root motion is baked even if a separately labelled root
track cannot represent the selected root. The report records that omission.

The native CLI build required removing an invalid `#effects(all)` annotation
from an existing constant and renaming Ice Cream's internal `Shot` type to
`IceShot` to avoid its collision with the engine camera type. No data layout,
game arithmetic, effect declaration on a procedure or media format changed.
Ice Cream's seven rules checks pass after the type-only rename.

## Checks

- Station/shared playback: **252 distinct checks pass**. The full 251-check
  suite passed before the new nested-root regression; that regression and
  the affected retarget station check also pass on the final source.
- Retarget bake/writers/shared admission: **202/202** checks on final source.
- Headless harness and frame-report helpers: **136/136** checks.
- New authority gates: mapping **81**, private path **32**, CLI grammar **115**
  mutants killed, **228 total, zero survivors**. Gates use root-only sites
  with all dependency checks retained.
- Shader drift, no-DOM-UI check and `tools-check` pass.
- No whole-repository gate was run. A0/A1's unchanged authority gates retain
  their earlier passing records.

## Fixed frames and budget

Muted headless Chrome with WebGPU on Apple Metal renders Echo→mannequin at
frames 4 and 18, and mannequin→Echo at frame 18. Both viewports produce the
same three complete GPU hashes in two runs. Each view has 53 paired anchors
and 24 mesh section draws. There are zero browser errors.

| View | Desktop 1280 × 800 | Phone viewport 390 × 844 |
| --- | --- | --- |
| Echo→mannequin, 4 | `2d0a6cb7460d0c758639e2aa6a72fe7a7b8a50a1348229b5095d3b818b21f4b1` | `00e459c89a80c06fc6a93f9723305c53c09c46d3f3d8e10cd7f3a923846df5f7` |
| Echo→mannequin, 18 | `2dba6d7bdfddc02b5c26fce416850cfa1713cd6283bc0a61b22860f5234e04e1` | `ea32608ca3272aa330e449508cafcae2ab11d90799c15e3cd0fe10d53a5ef65f` |
| Mannequin→Echo, 18 | `38d3698e6bfa83466c87b77032fccb2d6ffb098ebe5dcd87a05bf8ea65e671a7` | `462b923c6c0889ce0196faf1f069a893afaf146adedfef9722730f05dd341ef8` |

| Run | Median ms | p95 ms |
| --- | ---: | ---: |
| Desktop first / second | 2.075 / 1.900 | 4.735 / 5.215 |
| Phone viewport first / second | 2.475 / 2.070 | 4.945 / 4.190 |

Each measures 60 complete held-source frames including GPU completion. The
budget is 33.3 ms. The release Wasm is **241,835 bytes**, SHA-256
`fb65841d62a7a6abaa483cd6cc4e480d45bed7cd45e141d79a58a0549b85f6b6`;
the generated shader is
`958f6ca21885a9b8577a25c9e0794916819c28dbd41cde24f481d3409660367f`.
The inspector retains its 256 MiB memory and combined 64 MiB geometry limits.

A physical iPhone run remains an owner action. The exact RETARGET taps are
pushed to [workspace NEEDS_OWNER.md](https://github.com/AtlantisPleb/workspace/blob/main/NEEDS_OWNER.md)
in commit `344e6b8c55`. Headless phone results use the Mac GPU. No production
deployment was performed. #12962 remains open and Valley behavior is unchanged.
