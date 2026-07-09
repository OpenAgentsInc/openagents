# Sarah Quality Scoreboard (SQ-1 / #8618)

Date: 2026-07-09
Issue: OpenAgentsInc/openagents#8618 (epic #8610)

Every OAV / media take gets **one** machine-readable scoreboard beside the
artifact. Stills alone never advance a take — **playback verdict + temporal
evidence** decide.

## Canonical implementation

`schemaVersion`: `sarah-take-scoreboard.v1`

- Schema + validator + Markdown renderer + `score-take` CLI:
  `packages/sarah-take-scoreboard` (Effect Schema; tests run in the sweep as
  `test:sarah-take-scoreboard`).
- Checked-in scoreboards (JSON + rendered md + `index.ndjson`):
  `docs/sarah/scoreboards/` — see its README for the workflow and the current
  retrofit corpus (sarah_reply v1/hq/enhanced/v3/latentsync, opener library
  v1).

An earlier same-day draft (`openagents.sarah.quality_scoreboard.v1`, an
`apps/sarah/scripts/quality-scoreboard.mjs` validator) was consolidated into
this package so there is exactly one scoreboard schema; nothing had been
recorded against the draft schema when it was removed.

## Sections

| Section | Fields |
|---|---|
| `inputRefs` | sourceClip, script, ttsReference, modelVersions{}, renderCommand?, recipe, commits[], artifactUris[] |
| `audioGates` | sttRoundTrip {status, perSegment, transcriber?}, loudnessLufs?, truePeakDbtp?, pauseTiming, prosody {humanVerdict, llmJudgeScore? 1-10, llmJudgeModel?}, initialismRisk? |
| `videoGates` | ownerPlaybackVerdict (`pass\|fail\|pending` — THE gate), avSync {start,middle,end}, cropSharpness, temporalBoil, chunkBoundaryJerk {status, metrics?}, identityDrift, pasteBackSeam, badFrameExclusions[] |
| `operationalGates` | renderWallSeconds?, gpuType?, costEstimateUsd?, artifactExistenceCheck (`object_exists` only), hostDisposition {status, reason}, gcsIndexUpdated, noSecretsInArtifacts, closeoutReceipt? |
| `advance` | boolean — validator refuses `true` unless owner playback PASS + human prosody PASS + per-segment STT PASS and no failing gate |

## Cultural law (validator-enforced)

- `stills_cannot_advance`: `advance: true` without an owner playback PASS is
  rejected. Stills, motion statistics, and word-level STT never advance a
  take (the enhanced take and opener library v1 both passed those and failed
  owner playback).
- `prosody_gate_required` / `stt_gate_required`: human prosody verdict and
  per-segment STT must PASS to advance.
- `left_running_without_reason`, `secret_pattern`: SQ-8 operational laws.

## Operator flow

```bash
# copy an existing record as a template, edit gates after playback review
cp docs/sarah/scoreboards/sarah-reply-v3.json docs/sarah/scoreboards/<take>.json

# validate + write canonical JSON, rendered md, and rebuild index.ndjson
bun packages/sarah-take-scoreboard/src/cli.ts emit docs/sarah/scoreboards/<take>.json
```

Pair with the SQ-8 closeout (`docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md`) for
host/cost receipts; the scoreboard's operational-gates section embeds that
checklist.
