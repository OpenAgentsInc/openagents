# openagents-audio

Private Bun/Effect Cloud Run gateway for persistent voice. It accepts authenticated AUDIO-1 binary WebSocket frames at `/v1/stream`, validates exact owner/device/thread/session/generation and sequence, and bridges bounded LINEAR16 chunks to Google Speech-to-Text V2 `StreamingRecognize` with `chirp_3`.

The Google adapter uses ADC/workload identity. Credentials, raw audio, and transcript text are never logged. Logs contain only event names and generation numbers; production metrics must remain ref-only. The service owns transcription delivery, not commands, Sync, retention, or raw-audio storage.

## Runtime contract

- `GET /health` is an unauthenticated liveness endpoint.
- `/v1/stream` is a binary WebSocket endpoint. It requires a service-issued HMAC grant in `X-OpenAgents-Audio-Grant` (or a WebSocket query fallback), bound to the exact voice identity and expiring within 15 minutes. Cloud Run IAM's `Authorization` identity token remains a separate outer gate.
- Audio messages are capped at 15,360 bytes, the stricter current Google VAD/streaming guidance; AUDIO-1's 24 KB transport maximum is only an outer bound.
- Queues are bounded, ACK and gap frames are explicit, duplicate audio is delivery-idempotent, stale generations close, and provider streams rotate after four minutes of 16 kHz mono PCM.
- Reconnect never assumes the same Cloud Run instance. The client resumes from its durable server-side/session authority; replay cannot open Google recognition or publish another final.

## Verification

```sh
bun run --cwd apps/openagents-audio test
bun run --cwd apps/openagents-audio typecheck
bun run --cwd apps/openagents-audio build:cloudrun
```

The live smoke is intentionally gated because it incurs Google STT use. Deploy with the repository automation gcloud configuration, mint a short-lived test grant through the application authority, then send a consented non-sensitive PCM fixture. Never put a token, transcript, or audio bytes in logs or issue comments.

Official constraints used by this implementation: Google STT streaming is gRPC-only; Chirp 3 supports V2 streaming, interim/final results and VAD; Cloud Run WebSockets are bounded by the request timeout and reconnect is not guaranteed to reach the same instance.

## Retained-audio storage

Private, receipt-gated audio retention for AUDIO-3. Transport frames are
coalesced upstream into bounded segments; this service accepts only segments
covered by an active explicit retained-session receipt. It envelope-encrypts
media before a private GCS write and keeps only exact manifests and audit
receipts in Cloud SQL.

The Cloud SQL schema is
`packages/khala-sync-server/migrations/0064_audio_retention.sql`. The
production bucket must have public-access prevention enforced, uniform
bucket-level access, media object versioning disabled, a lifecycle rule matching
the policy TTL, and a dedicated service identity limited to object
create/get/delete/list. Cloud SQL backups retain manifests and audit receipts,
not media objects.
Neither this service nor its SQL schema produces a public or signed URL.

Run `bun run test` and `bun run typecheck`. The gated GCS/Cloud SQL smoke is
documented in `docs/deploy/openagents-audio-retention.md`.
