# Autostream Settlement Visibility Capture

Date: 2026-06-19

Issue: #5438

This records the first public visibility/replay capture of a Tassadar
auto-stream settlement sequence. It is evidence capture only: no new settlement,
payout, accepted-work, provider, wallet, deployment, or public-claim authority
was added.

## Boundaries

- The public timeline shows `trace_submitted -> verification_verified ->
  real_bitcoin_moved -> settlement_recorded` for
  `training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4`.
- The public receipt dereferences with `movementMode: real_bitcoin`,
  `realBitcoinMoved: true`, and `settlement.state: settled`.
- The Worker path documents the no-operator-POST verdict hook in
  `apps/openagents.com/workers/api/src/tassadar-trace-contribution-routes.ts`
  and wires it in `apps/openagents.com/workers/api/src/index.ts`.
- The generated replay source refs still include
  `operator_approval.tassadar.autostream.worker`; treat that as the owner/gate
  evidence for the real-money rail, not as a broader autonomous-spend claim.
- Current source-lag rows still report stale source families. This capture
  records the visible public projection and its caveats.

## Public Sequence

Captured from:

```sh
curl -sS 'https://openagents.com/api/public/activity-timeline?from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80'
```

| Time (UTC) | Kind | Cursor/Event |
| --- | --- | --- |
| 2026-06-18T13:47:40.322Z | `trace_submitted` | `2026-06-18T13:47:40.322Z:training_trace:event.public.trace_submitted.contribution.tassadar_executor_trace.assignment.artanis_admin.20260616123648.kernel_trace` |
| 2026-06-18T13:47:40.412Z | `verification_verified` | `2026-06-18T13:47:40.412Z:training_verification:event.public.verification_verified.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4` |
| 2026-06-18T13:47:40.555Z | `real_bitcoin_moved` | `2026-06-18T13:47:40.555Z:settlement_receipt:event.public.real_bitcoin_moved.receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker` |
| 2026-06-18T13:47:40.555Z | `settlement_recorded` | `2026-06-18T13:47:40.555Z:settlement_receipt:event.public.settlement_recorded.receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker` |

Receipt refs:

- `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker`
- `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker`

## Receipt Dereference

Validation command:

```sh
curl -sS -o /tmp/openagents-5438-receipt.json -w '%{http_code} %{content_type}\n' \
  'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker'
```

Observed:

```json
{
  "httpStatus": 200,
  "schemaVersion": "openagents.nexus_pylon.public_receipt.v1",
  "receiptKind": "settlement_recorded",
  "movementMode": "real_bitcoin",
  "realBitcoinMoved": true,
  "settlement": {
    "providerRef": "provider.spark_treasury",
    "settlementMutationAllowed": false,
    "state": "settled",
    "stateLabel": "Settled",
    "walletReadinessStateLabel": "Receive ready"
  }
}
```

## Generated Replay

Validation command:

```sh
curl -sS 'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80'
```

Observed:

- `bundleRef`: `proof_replay_bundle.public_activity.73e66071`
- `schemaVersion`: `proof_replay_bundle.v1`
- `generatedFrom.authority`: `evidence_presentation_only`
- Replay events at seconds 42, 48, 54, and 60 match the timeline sequence.
- Source refs include the public challenge route, public receipt route, payout
  intent/attempt refs, reconciliation ref, and
  `operator_approval.tassadar.autostream.worker`.

## Rendered Clip

Rendered locally from the generated replay route:

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-clip.mjs \
  --bundle-url 'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80' \
  --start 42 \
  --duration 24 \
  --fps 1 \
  --width 640 \
  --height 360 \
  --camera zap_focus \
  --out /tmp/openagents-5438-autostream.mp4
```

Observed render outputs:

```json
{
  "bundleRef": "proof_replay_bundle.public_activity.73e66071",
  "frameCount": 24,
  "durationSecond": 24,
  "fps": 1,
  "renderer": "playwright-chromium-screenshot-plus-ffmpeg",
  "runLocation": "local_or_ci_render_box_with_bun_node_headless_chromium_and_ffmpeg_not_cloudflare_worker",
  "webglState": "available",
  "mp4ByteSize": 252982,
  "mp4Sha256": "aa2689c520bca782681b3cc4db9478aba5d06ac29fd7c03e29cb436b00ffda8b"
}
```

A local clip manifest was produced at
`/tmp/openagents-5438-autostream.mp4.clip-manifest.json` and copied to
`docs/launch/2026-06-19-autostream-settlement-clip-manifest.json` with:

- `schemaVersion`: `openagents.replay_clip_manifest.v1`
- `jobRef`:
  `replay_clip_job.autostream_10c3b01b_visibility_capture.issue_5438`
- `claimScope`: `evidence_presentation_only`
- `source.kind`: `timeline_range`
- `source.fromCursor`: the `trace_submitted` cursor above
- `source.toCursor`: the `settlement_recorded` cursor above
- `artifacts[0].storageUrl`:
  `local:replay-clips/replay_clip_job.autostream_10c3b01b_visibility_capture.issue_5438/openagents-5438-autostream.mp4`
- `caveatRefs`: includes
  `needs_owner.replay_clip.r2_bucket_not_provisioned` because this issue did
  not upload to R2.

## Validation Commands

```sh
bun install --frozen-lockfile
curl -sS 'https://openagents.com/api/public/activity-timeline?from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80'
curl -sS 'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80'
curl -sS 'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker'
node spike/replay-r1/render-clip.mjs --bundle-url 'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80' --start 42 --duration 24 --fps 1 --width 640 --height 360 --camera zap_focus --out /tmp/openagents-5438-autostream.mp4
git diff --check
```
