# Sarah Quality Scoreboard (SQ-1 / #8618)

Date: 2026-07-09
Issue: OpenAgentsInc/openagents#8618 (epic #8610)

Every OAV / media take gets **one** machine-readable scoreboard beside the
artifact. Stills alone never advance a take — **playback verdict + temporal
evidence** decide.

## Schema

`schemaVersion`: `openagents.sarah.quality_scoreboard.v1`

See `apps/sarah/fixtures/quality-scoreboard.example.json` and validator
`apps/sarah/scripts/quality-scoreboard.mjs`.

### Sections

| Section | Required fields |
|---|---|
| `inputs` | sourceClip, script, ttsRef, modelVersions{}, renderCommand, recipe, commits[], artifactUris[] |
| `audio` | sttRoundTrip (pass/fail/skip), lufs?, dbtp?, pauseTimingNotes?, prosodyVerdict (`pending\|pass\|fail`), initialismRisk (`low\|med\|high`) |
| `video` | playbackVerdict (`pending\|pass\|fail` — **required; stills-only forbidden**), avSync `{start,mid,end}`, temporalBoilNotes?, chunkBoundaryJerk?, identityDrift?, seamNotes?, badFrameExclusions[] |
| `ops` | wallTimeSec?, gpuCostUsd?, artifactExistenceOk (true), hostDisposition |
| `advance` | `eligible` boolean derived: only true when playbackVerdict=pass AND artifactExistenceOk AND no hard audio fail |

### Cultural law

```
if (playbackVerdict !== "pass") advance.eligible = false
// stills_ok alone cannot set eligible true
```

## Operator flow

```bash
cp apps/sarah/fixtures/quality-scoreboard.example.json /tmp/scoreboard-<take>.json
# fill gates after playback review
bun apps/sarah/scripts/quality-scoreboard.mjs --scoreboard /tmp/scoreboard-<take>.json
```

Pair with SQ-8 closeout (`docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md`) for host/cost.
