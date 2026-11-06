# Animation Lab A3 verification

Issue [#12968](https://github.com/OpenAgentsInc/openagents/issues/12968).
The `#layers` station evaluates an admitted performance document: base,
masked override, additive, face, then feet, head/eye aims and hand targets.
Each layer has a numbered selector and toggle; five group shortcuts expose
each kind. All 13 layers are independently toggled in the headless check. The contact meter compares the
incoming layered foot position with its corrected position, in centimetres.

## Private study and contact measurements

The native Coax authoring/check tool emits a 13-layer performance, an
annotated source clip, a receipt and a measurement report. The study combines
Valley's walk, a slowed arm override, a small upper-body additive layer,
Slay facial animation, eight foot contacts and a head target. Both JSON
outputs pass the shared document readers before writing. The tool writes
only to a new directory below the canonical `~/.openagents/` root and locks
the output parent while publishing. A failed quality gate returns failure
and retains its private report for review.

Contacts are inferred before choosing their targets: foot-joint height
within 2 cm of the source clip minimum and vertical speed at most 0.15 m/s.
The base's 0.15-second blend intervals are excluded; gaps shorter than the
0.15-second release are merged before selecting a target. Measurements use
intervals at most 1/120 second, including a point just before each foot-up.
These are joint-anchor measurements on this study, not sole annotations or
a guarantee for every imported motion. The final short contact has three
samples and remains in the report.

| Contact | Samples | Max OFF, cm | Max ON, cm | ON cumulative travel, cm |
| --- | ---: | ---: | ---: | ---: |
| 0 | 30 | 36.979912 | 0.000021709 | 0.000377794 |
| 1 | 46 | 58.451431 | 0.000021073 | 0.000692867 |
| 2 | 65 | 52.493370 | 0.000021913 | 0.000788420 |
| 3 | 54 | 35.676773 | 0.000024979 | 0.000826333 |
| 4 | 38 | 46.663414 | 0.000022302 | 0.000476757 |
| 5 | 46 | 55.053947 | 0.000021709 | 0.000562639 |
| 6 | 33 | 40.701282 | 0.000019783 | 0.000336957 |
| 7 | 3 | 2.183240 | 0.000016610 | 0.000018834 |

All eight contacts pass the **1 cm** radius and cumulative-travel gates.
The maximum OFF radius is **58.451431 cm**. With fixes, the maximum radius
is **0.000024979 cm** and maximum cumulative travel is **0.000826334 cm**.
There are **zero unreachable samples**. Foot-up starts the authored release;
release residuals are reported separately from full-weight plant errors.

Every layer's start, end and blend boundaries is sampled 0.1 ms on either
side. Maximum differences are **0.199062°** and **0.151358 cm**, below the
stated **0.25° / 0.25 cm** finite-step limits. Initial runs exposed unreachable
plants and a swing-foot jump when lowering the pelvis. The final solver
lowers the shared pelvis by at most 15 cm and preserves the incoming swing
foot position. Those failing development reports remain private.

Targets are in character space. Two-bone IK preserves each sampled bone's
length and reports unsupported chains and unreachable targets. Look rows
explicitly name head or eye joints, carry the rig's rest axes, and limit
head-anchor corrections to 45° and other aim corrections to 20°. Feet run
before aims, then hands. Authoring joint masks inherit the nearest mapped
ancestor; facial membership uses the rig's explicit face-bone list.

| Private artifact | SHA-256 |
| --- | --- |
| Input ALB1 | `671ba58db50b48447423ccf56c5a9f90f3c20e920fe7f52c4abeb52243470a69` |
| Performance | `04773d43bb63a51592028aac711d6eddf2cc5376187374d598c83155d8c06ee8` |
| Annotated clip | `fc8cc0ed575516bb0e1e035e35a09271d363cc993be151dad7dffeac75edb6f6` |
| Receipt | `4271f0aa7828b826dbcd346098f733f87c6a900df89c4a1e3bd7404b12adf57a` |
| 120 Hz report | `62bbea2c3ffdbb545193a710980805ca60c9e274cc2ff1db4b1cbbc0ddc03bf6` |

Reports and captures stay under the owner's private Animation Lab folder.
No licensed payloads or captured images are committed.

## Checks and bounded memory

- Station/shared suite: **281 distinct checks** pass: the 280-check suite, then the new
  individual-selector keyboard/touch check. The authoring contact-gap
  regression and performance-writer round trip also pass.
- Authority gates: envelope/pose **30**, mask **82**, layers **88**, anchor
  selection **84**, bundle **47** mutants killed: **331 total, zero survivors**.
  Root-only sites retain all dependency checks; no equivalence waivers.
- Realm generator: **25 tests / 244 assertions**, canonical realm drift,
  shader drift, visual-UI policy and `tools-check` pass.
- Actual Wasm asset ingestion accepts the bundle and both metadata files.
  A rejected replacement preserves the active source bank. All 20 source
  face clips, pause/resume and device recovery pass the inspection harness;
  recovery takes **591 ms**, with identical held pixels and zero errors.
- No whole-repository gate was run. Unchanged A0–A2 gates retain their
  previously recorded results.

The browser retains **256 MiB** Wasm memory and a **4 MiB** stack. A single
bounded JSON arena is reused for bundle admission; metadata admissions use
separate ABI entries. Frame solvers pass rig views. This removes overlapping
multi-megabyte copies that the actual ABI checks exposed. Initialization
zeros the owned arena and applies scalar Coax defaults. No memory limit was
raised to accommodate the new evaluator.

The public library count stays at 44 source clips. Layers reserve one hidden
slot for the contact annotation, so this station accepts at most **63 source
clips**; ordinary rig/clip inspection still admits 64. Metadata is bounded
to 256 KiB per file. Combined geometry remains bounded to 64 MiB.

## GPU captures and frame budget

Muted headless Chrome uses WebGPU on Apple Metal. Frame 14 shows OFF
**5.324 cm**, ON **0.000 cm** at the meter's 0.001 cm resolution. All five
layer-group toggles change pixels; restoring them restores the complete
GPU frame exactly. Two runs at each viewport have identical hashes.

| State | Desktop 1280 × 800 | Phone viewport 390 × 844 |
| --- | --- | --- |
| All on | `0c813705133643c11b7ef2edda6b4ba052ecea7eb75c6c654ead4895031dab2f` | `745e6feabfa0ddd3662042978f0d331ad4fb1a037074cd295efc62b7ea3fc2e1` |
| Base off | `246080e1281efd5bfeb6bbcc6be1f26e41c26fb5a0d36c3293390b7bdb1918ea` | `69d4dc731b23cddeb9915d670c4e59686cbd8387161e4ecc58508b62a8a3c088` |
| Override off | `c19781729b0280add8e8fe4b3724864ea514d4a04d419ffc6b0ed5576e4c5225` | `3a3b717cbfeb29dc337103178c237c97308d494eed371520dfbcd56583c2bfc8` |
| Additive off | `50886ed12642eb168f8df0c7496e235b2c47fca6c9f0ff980a9e12f8402e0cb7` | `fc74fa11f1d7d58488827568e08758cb395c9ab0670b52f0dce982da49056801` |
| Face off | `ea2384965ba08d56aa74a666834c437369e23270e536c302217adbe2b7926721` | `40973766c61a8fc190209ac21a5f7fe32983b0c4f78ef9725cc8e905727aff20` |
| Fix-ups off | `6d0532bf9958d31b91e4ededa86bff794adfcfc1aae6903f142fd211e5be114d` | `bc8b03854ce6d1c5f3531aa41c2c3932204fa64ca20e8062a369bf17dfbea92c` |

| Run | Median ms | p95 ms |
| --- | ---: | ---: |
| Desktop first / second | 0.950 / 0.940 | 2.210 / 2.480 |
| Phone viewport first / second | 0.950 / 0.955 | 2.340 / 2.340 |

Each run measures 60 complete held-source frames including GPU completion,
against **33.3 ms**. There are zero browser errors. The release Wasm is
**270,095 bytes**, SHA-256
`d6de239e12e0811b513d1dbd2d6b8c6673829749327bf96ec17f2ee3f747a351`.
The unchanged shader SHA-256 is
`958f6ca21885a9b8577a25c9e0794916819c28dbd41cde24f481d3409660367f`.

## Valley frame preservation

The EFC1 adapter shares envelope, pose and ambient-cycle primitives while
retaining source sampling and unclamped source curve weights. A frozen
private VPK is used for both builds, SHA-256
`bcff9de83ab0fc38ecac6363ccbed5f88681eba418f6fb848d08f0637be6cc3f`.
The initial parent `f4f9589eed` matches all 14 checkpoints after migration.
After rebasing, parent **`150260c5cf45e76f52527f70b32d7052629938c8`**, including
its newer strand LOD and hair lighting, was built before and after changing
only the two shared-layer call sites. All **14/14** J/F/ambient hashes match,
with zero browser errors. Ambient's three source segments, fade and 8-second
schedule remain unchanged. The affected ambient/shared suite passes
**103/103 checks** on this parent.

| Valley Wasm | Bytes | SHA-256 |
| --- | ---: | --- |
| Parent | 97,421 | `5bdaeec0e92ac64f32335a718e7a28098db15c7f258936204e14daf7a7657c47` |
| Shared layers | 97,491 | `45479327a97152dadd9755aaf3768a175232712eddb34a371c816e83b95a287d` |

| Profile / checkpoint | GPU SHA-256, identical before and after |
| --- | --- |
| shot / 0 | `fdd936e0c270be9162a7cefe703ffc8845789a1eb2cd389ecfc995c5c0cf2a12` |
| shot / 1 | `c9b147887f79714a36c9a5b65a2a135c7f875bb54f3008d60a2b173c20449e7c` |
| shot / 2 | `fdd936e0c270be9162a7cefe703ffc8845789a1eb2cd389ecfc995c5c0cf2a12` |
| shot / 3 | `48bfe4a5661239942d63795a66000b6232840429cf1e6fc5b70c24c36c484859` |
| shot / 4 | `20a67aac6b9170a6fdc4ca14bc9a5d731d7a8231db832371e4104e23f98c52d5` |
| face / 0 | `ca53cb698c6afaa1c88056dcae9aca5b539764e042a15b8e5a7e23e1a58e40c8` |
| face / 1 | `7a4e688765e9b0557b72110c23ee345b14c5b4804114f56cc5e7f472ca13aa3b` |
| face / 2 | `7a4005d5ad0488b6d021e96a54d3aac652ac032ef4ac1f64f5920c11a636c4a3` |
| face / 3 | `7a4005d5ad0488b6d021e96a54d3aac652ac032ef4ac1f64f5920c11a636c4a3` |
| face / 4 | `7a4005d5ad0488b6d021e96a54d3aac652ac032ef4ac1f64f5920c11a636c4a3` |
| face / 5 | `ca53cb698c6afaa1c88056dcae9aca5b539764e042a15b8e5a7e23e1a58e40c8` |
| ambient / 0 | `85f3058f05454f7d24a2d375193ca1cf90b640a0f6a4ec3081e5d449bc1d47ab` |
| ambient / 1 | `2ca4089239987dd787a3f42fc6bfa4d0313d1f46712b0a007aeae7090865394a` |
| ambient / 2 | `739a518ad81f15709624a03edeba28780f6f51516b1d0397256b301027b40a7d` |

## Owner action

A physical iPhone test is recorded with exact LAYERS, BASE, OVER, ADD, FACE
and FIX taps in [workspace NEEDS_OWNER.md](https://github.com/AtlantisPleb/workspace/blob/main/NEEDS_OWNER.md),
commits `f5c60d43b0657b9805efdcd64ba7e0691454d9c3` and `77b48f8cbc`. Headless phone viewports
use the Mac GPU. No production deployment was performed. **#12962 remains
open** for exact source rendering.
