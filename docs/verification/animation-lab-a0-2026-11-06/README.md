# Animation Lab A0 verification

Issue [#12965](https://github.com/OpenAgentsInc/openagents/issues/12965).
The [format reference](../../verse/anim-documents-v1.md) defines the six
document types and their admission limits.

## Admission

Each row is a complete entry-source mutation gate (`--root-only`), with all
dependency checks retained. No equivalent mutants are waived. Scene
admission ran as four shards, merged by the compiler. Each command finished
within the repository's 15-minute check limit.

| Unit | Killed | Survived |
| --- | ---: | ---: |
| Common JSON, names and pack ranges | 339 | 0 |
| Rig, skin, face maps and sparse morph bindings | 434 | 0 |
| Clip tracks, timing, events and provenance | 246 | 0 |
| Performance layers and solver declarations | 157 | 0 |
| Pose edits | 90 | 0 |
| Scene, paths, cameras, lights and sound cues | 397 | 0 |
| Cut and legacy media adapters | 244 | 0 |
| Compiled FRG1 face-rig sections | 300 | 0 |
| Shared ECH1 source adapter | 165 | 0 |
| **Total** | **2,372** | **0** |

Checks include made fixtures at exact capacities and one item beyond,
malformed names/counts, cyclic skeletons, nonfinite binary samples,
out-of-pack ranges, invalid references, duplicate bindings and times that
collapse after conversion to binary32. Rejected documents leave the previous
output unchanged. The `.cut` parser moved without arithmetic changes; the
Labs CLI's 105 checks pass. The frame-check tool's 136 checks pass.

## Private conversion

The converter admitted 334 joints, 24 body clips, 20 face clips, 181 shapes,
125 curve tracks and 16 facial bone tracks. It compares the admitted
rest/bind values, every body/face sample and all 564,903 sparse morph records
against their source words: **zero differing bits**. Source floats and
quantized skin weights are retained, including signed zero.

The private binary is 65,321,636 bytes:

```text
pack    e68a1cc8946b7ddc72dbe7bdbc425af39fab3c78f7aba54cc86d1460f830b019
receipt 604ade269ad88fc472e257a113f951f256f43da651d3d49eb652b6cc77ed9f88
```

Only the converter, made test data, counts and hashes are committed.
Licensed payloads, the export documents and all review images remain under
`~/.openagents/aethyrforge/animation-lab/`.

## Valley frame preservation

The initial comparison used parent `09a4a84221` and the frozen 794,477,568-byte
pack `9c285e9ffa59b850e93337c4089be86a39e7c54f52cda51419fe86eac7ef4074`.
Its original 91,877-byte module is
`c1c833463b2de270a3d87d00053a7dcf4aa6b3f4bbd2593b007e1970495291ae`.
Moving ECH1 admission into shared code produced 91,864 bytes,
`4adb737d370b15c993a7ac726046d626a9b7349ff6732707a5364a63b9f32a0d`.

Muted headless Chrome matched the previously recorded GPU crops exactly:

| Checkpoint | SHA-256 |
| --- | --- |
| J TF0290 frame 1125, and the exact step-back | `4a026affb2876d05204d9434afadc669633065a9be25f242e06a0f0e2dfaacdd` |
| Adjacent source frame | `2270ca438782b5629e24c1c2430f8b2992c2d1471e797a6f18e1851efa95f711` |
| Coupled body/face/camera playback | `b3231fdc4b4651b2e00aeae895644c5b63ba61f9fbbfbf55bee852c47f97df67` |
| J phone viewport, 390 × 844 on this Mac | `c07cb5c869e98c6d3df023a9c96b7195b9d7800f83eea3301bb72d0ab540b40c` |
| F neutral at 756 × 413 | `5fa92268795292afb5ae5e2d6caf3b8b54de9635a235428f168b76858027e731` |
| F frame 12 | `a95fccb4e949e641a00a783e5301a82ab6a0758af3cf28ed43727e18e71325af` |
| F frame 28, including pause | `afbdc13b6883518129dcd47051af930e045aa68e2aa77b0b59de09c65cc30be1` |

The F check also restores the exact neutral hash after clearing the face.
The J check verifies the existing ambient segment and fade while moving.
There were no browser errors in either frame comparison.

### Rebase onto the new source camera and groom

Upstream subsequently landed the source camera template, full Slay groom,
lighting changes and Replay button. A0 was rebased onto `aecc49ed29`.
The admission units and their Coax dependencies were unchanged by that rebase,
so their passing mutation gates above were retained.

For a comparison with that exact parent, the baseline build temporarily used
the parent's inline `echo-rig-policy.coax` in this task's worktree. All other
Valley sources, WGSL and host files were the parent's. The committed shared
adapter was then restored before the migrated build. No other worktree or
shared private-pack alias was changed.

| Build | Bytes | Wasm SHA-256 |
| --- | ---: | --- |
| Upstream parent | 92,929 | `b51e500cdc8a1c18b1b98742eb8863053adabab3cc806c393f243099583365f6` |
| A0 shared adapter | 92,924 | `e1cf7ca10e9fa94115f8b07a9bdb773cbc427ddd03186b5672d37a806faa2b7c` |

Both use the 1,023,634,576-byte combined private pack
`5807c40f1c56dd67727bc7a72605049e6ca533c177251ab9b8c156fd0560dc0c`.
The upstream baseline has these crop hashes:

| Checkpoint | SHA-256 |
| --- | --- |
| J frame 1125 and step-back | `0184111aa4a534a0f4d9b86efb49c318ffa3b1babe5ff5b3609ca73fc880e274` |
| J adjacent frame | `2a6159736b0ecaa55f43e4e93f626e71e14fb962bf48dff05dce7853b564af69` |
| J advancing playback | `1956d50b765e456fed340ddc8e59b333673d9548a82d8a841d392222ffcc79cd` |
| J phone viewport | `ef3a961eec864f53dee4e91178262582863473ddaea9c1789b89e496b25522c5` |
| F neutral and reset | `a18f96a08c6a946be4fcc1fc17b729d744e3ca1b89f89541639c98822cbb7a1a` |
| F frame 12 | `f51fbd4685aff3a8227da98a49895b24f26ee02296d6d402d551a678de5c6689` |
| F frame 28 and pause | `c0f86a278c4e8509a1d28593f348afde617a257f2e8eba12e1b1077fed6fc92c` |

**Both migrated comparisons pass:** every crop above matches the recorded
parent, including the phone viewport, with no browser errors. The first
phone attempt captured before the resize settled; its
[failure](phone-resize-red.log) is retained. The tool now waits for the
resize notification and two paused producer frames. It matches the original
parent hash without changing that baseline. No renderer or host behavior was
changed to resolve the capture race.

The final rebase also retains main's later lighting and native-host work
through `bb61c1ff9a`. It does not change the admission units or their Coax
dependencies. The before/after hashes above apply to `aecc49ed29` and its
lighting; subsequent lighting qualification remains in #12962's record.

The original recovery probes timed out reacquiring a GPU device before this
change. Frame preservation does not requalify GPU recovery, cinematic parity
or mobile hardware. The current full-groom recovery limitation is tracked in
[#12975](https://github.com/OpenAgentsInc/openagents/issues/12975).
[#12962](https://github.com/OpenAgentsInc/openagents/issues/12962) stays open.

The capture tool can explicitly record an upstream baseline and compare the
migration with it, including when upstream changes the source pack:

```sh
coax/web/tool.sh coax/web/anim-document-frames.coax shot URL PRIVATE_OUTPUT --record BASELINE.json
coax/web/tool.sh coax/web/anim-document-frames.coax shot URL PRIVATE_OUTPUT --compare BASELINE.json
```

Use `face` for the F checkpoints. Baseline receipts record the loaded Wasm
hash and viewport as well as the crop hashes. Comparisons require the same
viewport and every crop; the producer hashes can differ after a code move.

## Rendering the converted documents

Follow-up qualification on `42920e800b` closes the document-to-runtime path.
`anim-echo-documents --compile` admits the rig and 44 clips and rebuilds the
ECH1/EFC1 streams from those records. The compatibility backend supplies no
animation samples from the template. Its limits are documented in the format
reference. A made-fixture check changes a bind value and face clip name and
verifies both changes in the emitted runtime stream. The complete converter
suite passes **247/247** checks; the unchanged admission gates above retain
**2,372 killed, zero survivors**.

The ordered receipt/rig/clip input digest is
`3c773cb5444183258613ecfb43f6bd72dd97addd0eb85abaffd749613a4b95c2`.
The compiled VPK has **1,023,634,576 bytes**, SHA-256
`5807c40f1c56dd67727bc7a72605049e6ca533c177251ab9b8c156fd0560dc0c`.
It is byte-identical to the current source VPK. The private output is
`~/.openagents/aethyrforge/animation-lab/a0-runtime/valley-from-documents.pack`.
Only this task's new `animlab-a0-documents` alias points to it; the shared
Valley alias was not changed.

Muted headless Chrome loaded that emitted package through
`/worlds/valley-canyon?pack=animlab-a0-documents`. The baseline used
`animlab-a0-current` with the same `bb61c1ff9a` lighting, host, viewport and
`e1cf7ca1…faa2b7c` Wasm. All 11 captured crops match. J's frame 1125 and
step-back hash is
`4bdccc51f7cf6774aff62730856c990cc1700b74dcba600232d89e1b52c19888`;
the adjacent frame is
`09d62e352e49babf506e8ce15bbefde259b9be40fee01a3b9708e60e37860b8b`;
advancing playback is
`52de9411a12a312107d071f2938d278bbb5aba42cacb939600be27a2ade4a8f8`;
and the phone viewport is
`bb9528187d2c5c0b82aa456cb516a29802f5efab995a783ac82710a38cbf6e30`.
F retains the neutral, frame 12 and frame 28 hashes in the previous table,
including pause and neutral reset. Both comparisons report no browser errors.
Raw hash-only receipts and the compile/check logs are committed alongside
this record. Captured images and licensed documents remain private.

## Scope

A0 supplies the documents, admission, binary conversion and shared source
adapters. It does not implement stations, a new retargeter, layer solvers,
imports from new source formats or a scene renderer. Those remain in A1–A8.
There was no production deployment and no owner action was needed for A0.
