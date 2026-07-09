# Sarah take scoreboards (`sarah-take-scoreboard.v1`)

One canonical, playback-first quality artifact per OAV/media take
(SQ-1 #8618, epic #8610). Schema, validator, renderer, and the `score-take`
CLI live in `packages/sarah-take-scoreboard`.

**Law: no take advances on stills.** The owner playback verdict plus temporal
evidence decide. The validator refuses `advance: true` unless owner playback,
human prosody, and per-segment STT all PASS and no gate is failing. Context:
`sarah_reply_enhanced` passed stills QA and failed owner playback; opener
library v1 passed stills, jerk metrics, and word-level STT and failed owner
playback on all 10 clips (#8610).

## Layout

- `<takeId>.json` — the canonical machine-readable scoreboard
- `<takeId>.md` — rendered summary (generated; do not hand-edit)
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
| `sarah-reply-v1` | pending | OAV-1 proof baseline; audio defects sent to OAV-3 |
| `sarah-reply-hq` | pending | v1 audio defects fixed; enhancement baseline |
| `sarah-reply-enhanced` | **FAIL** | choppy/plastic in motion after passing stills QA |
| `sarah-reply-v3` | pending | tamed GFPGAN; measured smoothest take (jerk 0.15) |
| `sarah-reply-latentsync` | pending | best articulation; 16-frame chunk-boundary hitch |
| `sarah-openers-v1` | **FAIL** | all 10 clips failed playback; openers-v2 lanes running |
