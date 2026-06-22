# Visibility + Replay Operations Runbook

Date: 2026-06-19

This runbook covers operating the public visibility/replay stack: activity
timeline, SSE stream, SpacetimeDB projection bridge, browser replay surfaces,
render-box clips, and R2 clip outputs.

All surfaces here are observation/projection/retrieval only. They grant no
settlement, payout, accepted-work, deployment, provider, wallet, or public-claim
authority.

## Source Of Truth

Authoritative state stays in the `openagents.com` Worker and its private
stores. Public routes are projections:

| Surface | Source of truth | Projection |
| --- | --- | --- |
| Activity timeline | Worker/D1 public-safe reducers for Pylon, training, verification, settlement, Forum, Artanis, and capacity | `GET /api/public/activity-timeline` and `GET /api/public/activity-timeline/stream` |
| Live Tassadar scene | `GET /api/public/tassadar-run-summary` plus optional SpacetimeDB world rows | `/tassadar` browser view |
| Proof replay bundle | Public Worker replay resolver | `/api/public/proof-replays`, `/api/public/tassadar-replays/first-real-settlement` |
| SpacetimeDB world | Projection from public timeline/run summary | the deleted legacy bridge scripts |
| Replay clip jobs | Worker/D1 clip-job records | `GET|POST /api/public/replay-clips` after owner integration |
| Replay clip bytes | Owner-provisioned R2 bucket/public host | `openagents.replay_clip_manifest.v1` artifact URLs |

Projection contracts must carry `projection_staleness.v1`, source refs,
blocker refs, caveat refs, and public-safe redaction. A fresh `generatedAt`
does not prove every source family is current; inspect `sourceLag`.

## Normal Checks

Run HTTP projection checks:

```sh
bun run --cwd apps/openagents.com smoke:tassadar:live-page
bun run --cwd apps/openagents.com smoke:activity:proof-links
```

Run owned-infra freshness checks. This is the scheduled alerting command; do
not move it into GitHub Actions as the primary monitor:

```sh
node apps/openagents.com/scripts/visibility-freshness-smoke.mjs \
  --base-url https://openagents.com
```

Run browser/canvas checks:

```sh
bun run --cwd apps/openagents.com smoke:visibility:browser
```

For local UI verification against live public APIs:

```sh
cd apps/openagents.com/apps/web
bun run dev -- --host 127.0.0.1 --port 5173 --force

bun run --cwd apps/openagents.com smoke:visibility:browser -- \
  --base-url http://127.0.0.1:5173 \
  --api-base-url https://openagents.com
```

## Timeline API

Fetch current state:

```sh
curl -s 'https://openagents.com/api/public/activity-timeline?limit=20' \
  | jq '{generatedAt, nextCursor, sourceLag, events: [.events[] | {ts, kind, sourceKind, eventRef, sourceRefs, blockerRefs, caveatRefs}]}'
```

Resume with a cursor:

```sh
curl -s "https://openagents.com/api/public/activity-timeline?since=${cursor}&limit=20"
```

Generated replay bundles require bounded time input:

```sh
curl -s 'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T18:00:00.000Z&to=2026-06-18T18:05:00.000Z&limit=100' \
  | jq '{bundleRef, generatedFrom, sourceRefs, caveatRefs, gaps}'
```

Failure routing:

| Symptom | First check | Expected action |
| --- | --- | --- |
| Empty or old timeline | `generatedAt`, `sourceLag[]` | Identify stale `sourceKind`; inspect its public route/source refs. |
| `projection_gap` event | `blockerRefs`, `caveatRefs` | Treat as honest gap; do not invent events. |
| Proof links fail | `smoke:activity:proof-links` | Fix URL derivation or missing public dereference route. |
| Private material concern | route body + redaction tests | Remove the projection; never publish raw traces, invoices, keys, tokens, or local paths. |

## SSE Stream

Tail the same public timeline shape:

```sh
curl -N 'https://openagents.com/api/public/activity-timeline/stream?limit=20'
```

The stream emits an `activity_timeline_meta` frame followed by event frames with
`id` equal to the timeline cursor. Reconnect with `since=<last id>` or
`Last-Event-ID`; the query parameter wins when both are present.

If SSE fails, use the `x-openagents-polling-fallback` response header or poll
`/api/public/activity-timeline?since=...`. SSE is a convenience projection, not
the authority.

## SpacetimeDB Bridge

Build a dry-run reducer plan from the public activity timeline:

```sh
bun deleted legacy bridge script \
  --json \
  --limit 50
```

Apply to the owned VM only from an operator-approved environment:

```sh
bun deleted legacy bridge script \
  --apply-vm \
  --limit 50
```

The bridge projects public events into world rows. It is not a settlement,
training, receipt, or claim authority. If a bad projection is suspected:

1. Stop the scheduler or stop running `--apply-vm`.
2. Save the current timeline envelope and generated plan for evidence.
3. Re-run `--json` against a known-good saved envelope with `--source-file`.
4. Apply the corrected public projection only after the source refs and reducer
   plan are understood.

## Render Worker And R2 Outputs

The Cloudflare Worker owns clip-job records only. It must not render frames,
run Playwright, run ffmpeg, or hold R2 upload secrets. Rendering belongs on an
owned render box.

Local render:

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-job.mjs \
  --job spike/replay-r1/job.example.json \
  --out spike/replay-r1/out/clip.mp4
```

R2 upload mode is owner-gated. Required render-box environment variables:

```sh
R2_REPLAY_CLIPS_BUCKET
R2_REPLAY_CLIPS_PUBLIC_HOST
R2_REPLAY_CLIPS_ACCOUNT_ID
R2_REPLAY_CLIPS_ACCESS_KEY_ID
R2_REPLAY_CLIPS_SECRET_ACCESS_KEY
R2_REPLAY_CLIPS_PREFIX # optional
```

Do not print secret values. Missing bucket credentials fail closed with
`needs_owner.replay_clip.r2_bucket_not_provisioned`.

Regression render smoke:

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-regression-smoke.mjs \
  --out /tmp/openagents-replay-clip-regression \
  --duration 1 \
  --fps 1 \
  --width 640 \
  --height 360
```

This checks nonblank WebGL frames, camera-path differences, generated timeline
replay rendering, and clip manifests with public-safe source/caveat refs.

## Rollback

| Area | Rollback |
| --- | --- |
| Web/Worker deploy | Re-deploy the last known-good `origin/main` using `docs/DEPLOYMENT.md`; include `wrangler deploy --assets ../../apps/web/dist` so UI and Worker match. |
| Timeline projection bug | Disable affected projection read path or ship a revert; keep honest `projection_gap` output rather than fabricating events. |
| SSE regression | Fall back to polling; fix stream route separately. |
| SpacetimeDB bridge bug | Stop `--apply-vm`; replay a known-good saved envelope or wait for corrected timeline source. |
| Render-box failure | Stop the render worker; queued clip jobs remain evidence-only records. |
| R2 artifact issue | Remove or stop advertising the broken public manifest URL; re-upload from the render manifest after credentials and public host are verified. |

## Deployment Pointer

The deployment hub is `docs/DEPLOYMENT.md`. Update that hub whenever this
runbook's command names, owner-gated boundaries, or release/deploy mechanics
change.
