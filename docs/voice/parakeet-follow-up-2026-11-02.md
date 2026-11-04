# Parakeet: the bounded follow-up pass

This is the historical 2026-11-02 stopping-point record. The later status is
below; the earlier statements about work outside this pass are not a current
list of open issues.

The owner limited this pass to three issues, then a stop: default local
transcription (#12666), bounded long-file transcription and subtitles
(#12620), and Mac input-device selection (#12671). The broader
[Parakeet epic](https://github.com/OpenAgentsInc/openagents/issues/12614)
remains open.

Completed and pushed all three selected issues, then stopped. The Mac device
change is `5fcbbb6861`; the default CLI and file change is `3049f25f3b`.

| Issue | Delivered behavior and evidence |
| --- | --- |
| #12666 | Default local Parakeet transcription, explicit legacy compatibility and early model/backend mismatch refusal. Current rebuilt CLI: 21 transport/refusal cases and exact original-weight short recording. [File/CLI qualification](../verification/parakeet-file-cli-2026-11-02/README.md). |
| #12620 | Exact file windows, global resampling, versioned JSON and coarse SRT/VTT cues. Seven policy groups, eleven native cases under plain/ASan/UBSan, standard build, original-weight long/seam fixtures and interruption controls pass in the same file/CLI record. |
| #12671 | Persisted CoreAudio UID selection, explicit OS-default choice and safe route-change cancellation. [Device qualification](../verification/coder-dictation-devices-2026-11-02/README.md): 409 policy cases, native boundary/host sanitizer checks, full app and recorded-audio no-send run. Physical acceptance remains in `NEEDS_OWNER.md`. |

The file reference compares four new independent-window profiles with pinned
NeMo on the owned GPU. A 180-second-plus-three-sample fixture checks nine
complete requests. A separate sentence crosses an actual window boundary;
normalized words survive in that example, with punctuation/case differences.
These finite fixtures are not representative population or general seam
quality measurements.

An independent blocked-output test caught a cancellation hang in libc's
buffered write path. The fix uses interruptible POSIX stdout writes and an
explicit nonzero cancellation result. The original failure and the corrected
run remain separate; cancellation after output begins may leave partial bytes,
while cancellation before publication emits no transcript.

The [shared toolchain record](../verification/parakeet-follow-up-2026-11-02/toolchain.json)
pins the focused development compiler and reused resident-model build. Normal
full-bootstrap/release qualification is separate. No full mutation run or
unchanged model-corpus replay was used for this pass.

The work uses separate agent worktrees. The main checkout is not an editing,
build or test directory. Existing dated model and app qualifications retain
their original identities; this pass checks changed code and new consumer
paths rather than repeating the complete model or mutation gates.

The other seven follow-ups remain outside this pass: representative dictation
quality and release latency (#12665), microphone CLI (#12667), terminal
composer (#12668), Verse/Aethyr provider (#12669), vocabulary and formatting
(#12670), Linux capture and inference (#12672), and genuine incremental ASR
(#12673). Independent file windows are not cache-aware streaming. TDT emission
positions are not acoustic word alignment.


The CLI entry-point change leaves the app/library translation unit byte-for-byte
unchanged under its actual compile macros; the
[library continuity check](../verification/parakeet-follow-up-2026-11-02/library-continuity.json)
records that boundary. Existing model qualification was reused with its own
source/producer identities. The file record separately preserves the earlier
large-file model witness and the corrected publication-effect qualification.

Physical microphone, permission and input-device acceptance remain owner-only
in [NEEDS_OWNER.md](../../NEEDS_OWNER.md). The tested Mac app is preserved at
`/tmp/openagents-parakeet-devices-20261102/OpenAgents.app`; the final CLI/native
binaries are at `/tmp/parakeet-file-cli-final-20261102/`.

All implementation workers stopped. Their worktrees are Git-clean. The managed
archive tool refused cleanup of pinned/protected worker worktrees; no pin was
changed and that protection was not bypassed. No main-checkout edits, builds or
tests were used for this pass.

## Subsequent status, 2026-11-03

#12665, #12667, #12668, #12669, #12670 and #12672 are now closed: Linux
evaluation/optimizations, microphone CLI, terminal insertion, Verse/Aethyr
recognition, CLI vocabulary/formatting and Linux CUDA support have landed.
#12673 now has the separate Linux streaming qualification below. The [epic](https://github.com/OpenAgentsInc/openagents/issues/12614)
has been reconciled with those issue states.

The Mac host now consumes #12694's silence-padding plan. A separate Mac
worktree was built and checked over SSH: exact payload/cancellation and native
sanitizer checks, an original-weight 30.46 s recording, and the actual rebuilt
app's offscreen insertion/Undo/no-send fixture pass. See the
[Mac consumer record](../verification/dictation-eval-2026-11-02/README.md#mac-consumer-adoption-2026-11-03).

Coder for Mac still needs vocabulary/code-formatting adoption, CPU rescoring
measurements and actual release-to-composer timing. Physical microphone,
permission/device and terminal acceptance remain in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
The separate [incremental checkpoint/profile](../../psionic/docs/parakeet.md#incremental-follow-up-12673)
is implemented and qualified for #12673 on Linux CUDA: real incremental
features, persistent FastConformer/RNNT state, partials, finalization and
reset/cancel/order refusal. The explicit file-backed Coax consumer admits only
one final through Aethyr's existing media/turn gate. Thirteen original-weight
cases and the bounded native API are recorded in the
[incremental qualification](../verification/incremental-asr-2026-11-03/README.md).
The Linux CLI now adopts it explicitly with `openagents dictate --stream`
(#12745): partials while capture continues, one guarded final on stop and
responsive cancellation. The actual launcher, recorded capture and recorder
subprocess are checked in the
[consumer record](../verification/dictate-stream-2026-11-03/README.md).
Mac/Metal streaming and composer/Verse partial UI adoption remain engineering
work. Physical Linux input acceptance is in NEEDS_OWNER.md. The default
dictation provider and chapter 153's numerical profile remain unchanged.
