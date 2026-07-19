# Sarah take scoreboards (`sarah-take-scoreboard.v1`)

One canonical, playback-first quality artifact per OAV/media take
(SQ-1 #8618, epic #8610). Schema, validator, renderer, and the `score-take`
CLI live in `packages/sarah-take-scoreboard`.

**Law: no take advances on stills.** The owner playback verdict plus temporal
evidence decide. The validator refuses `advance: true` unless owner playback,
human prosody, and per-segment STT all PASS and no gate is failing. Context:
`sarah_reply_enhanced` passed stills QA and failed owner playback. Opener
library v1 passed stills, jerk metrics, and word-level STT and failed owner
playback on all 10 clips (#8610).

## Layout

- `<takeId>.json` — the canonical machine-readable scoreboard
- `<takeId>.md` — rendered summary (generated, do not hand-edit)
- `index.ndjson` — one line per take, rebuilt by `score-take emit`

## Workflow

```bash
# validate everything checked in here (also runs in the test sweep)
bun packages/sarah-take-scoreboard/src/cli.ts validate --dir docs/sarah/scoreboards

# add or update a take: edit <takeId>.json, then regenerate md + index
bun packages/sarah-take-scoreboard/src/cli.ts emit docs/sarah/scoreboards/<takeId>.json
```

The operational-gates section embeds the SQ-8 GPU media-run closeout
checklist (`docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md`, #8625): artifact-existence
checks only (`object_exists`, never log markers), explicit host disposition
with reason, GCS index updated, cost estimate, and the no-secrets attestation.

## Current corpus (retrofit, 2026-07-09)

| Take | Owner playback | Note |
| --- | --- | --- |
| `sarah-reply-v1` | pending | OAV-1 proof baseline. Audio defects sent to OAV-3 |
| `sarah-reply-hq` | pending | v1 audio defects fixed. Enhancement baseline |
| `sarah-reply-enhanced` | **FAIL** | choppy/plastic in motion after passing stills QA |
| `sarah-reply-v3` | pending | tamed GFPGAN. Measured smoothest take (jerk 0.15) |
| `sarah-reply-latentsync` | pending | best articulation. 16-Frame chunk-boundary hitch |
| `sarah-openers-v1` | **FAIL** | all 10 clips failed playback. Openers-v2 lanes running |

## Openers v2 (2026-07-10, per-clip)

| Take | Owner playback | Note |
| --- | --- | --- |
| `openers-v2-opener-01-hello` | **PASS** (batch, 2026-07-10) | judge 7/10 audio — PLATEAU across 60+ re-roll candidates. Stronger TTS tier is the escalation |
| `openers-v2-opener-02-welcome-back` | **PASS** (batch, 2026-07-10) | superseded lane: r2 fixes the 'did'→'do' slur |
| `openers-v2-opener-02-welcome-back-r2` | pending | **9/10 VERBATIM audio** (`w2-slow92-s31415`). Fresh Hallo2 render |
| `openers-v2-opener-03-good-question` | **PASS** (batch, 2026-07-10) | judge 7/10 audio — PLATEAU across 60+ re-roll candidates. Stronger TTS tier is the escalation |
| `openers-v2-opener-04-got-it` | **PASS** (batch, 2026-07-10) | judge 9/10 audio. Most expressive render. STT verbatim pass |
| `openers-v2-opener-05-show-you` | **PASS** (named "close to shippable") | judge 7/10 audio. Kept unchanged for A/B vs r2 |
| `openers-v2-opener-05-show-you-r2` | pending | **8/10 audio** (`w2-pb-s777`, brisker read). Fresh Hallo2 render |

Owner batch verdict 2026-07-10 (verbatim): "those v2s are much better -
opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed
in that direction." `advance` stays false everywhere pending per-take
playback + the sharpness tier.

All takes: Hallo2 (MIT) still-animation renders of judged winner wavs.
`*-sr.mp4` variants are research-only (S-Lab license chain on `video_sr.py`),
and FLAIR was evaluated and REJECTED as the permissive upscaler (S-Lab
CodeFormer aux in its sampling loop, 512² output cap, ~33 min/s runtime) —
raw 512² is the ship tier. Desktop copies: `~/Desktop/sarah-openers-v2/`.
Audio re-roll provenance: `gs://…/openers-v2/bakeoff-r2/`.
