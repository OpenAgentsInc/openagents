# @openagentsinc/sarah-take-scoreboard

The Sarah Quality Scoreboard (`sarah-take-scoreboard.v1`) — one canonical,
playback-first quality artifact per OAV/media take (SQ-1 #8618, epic #8610).

Every take gets a machine-readable JSON scoreboard plus a rendered Markdown
summary beside the artifact:

- **Input refs** — source clip, script, TTS reference, model versions, render
  command, recipe, commits, artifact URIs.
- **Audio gates** — per-segment STT round-trip, LUFS / true peak, pause
  timing, human prosody verdict + LLM prosody judge score, initialism risk.
- **Video gates** — owner playback verdict (THE gate), A/V sync at
  start/middle/end, crop sharpness, temporal boil, chunk-boundary jerk with
  motion metrics, identity drift, paste-back seam, bad-frame exclusions.
- **Operational gates** — embeds the SQ-8 GPU media-run closeout checklist
  (`docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md`, #8625): render wall/cost,
  artifact-existence check (`object_exists` only — log markers are banned),
  host disposition with reason, GCS index flag, no-secrets attestation.

**Law: no take advances on stills.** The validator refuses `advance: true`
unless owner playback, human prosody, and per-segment STT all PASS and no
gate anywhere is failing.

## CLI

```bash
# validate scoreboard files
bun packages/sarah-take-scoreboard/src/cli.ts validate <file.json...>
bun packages/sarah-take-scoreboard/src/cli.ts validate --dir docs/sarah/scoreboards

# validate + write canonical <takeId>.json, <takeId>.md, and rebuild index.ndjson
bun packages/sarah-take-scoreboard/src/cli.ts emit <file.json...> [--out-dir <dir>]
```

Checked-in scoreboards live under `docs/sarah/scoreboards/` and are validated
by this package's tests in the normal sweep (`bun run test:sarah-take-scoreboard`).
