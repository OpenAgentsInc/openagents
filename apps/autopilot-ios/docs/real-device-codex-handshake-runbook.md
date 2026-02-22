# Real Device Codex WGPUI Validation Runbook

Status: active (`OA-IOS-WGPUI-CODEX-009`)

## Preconditions

1. Date/time synchronized on desktop + iOS device.
2. Desktop app running with Codex worker online.
3. iOS app build includes current Rust client-core artifact.
4. Test account can complete email-code auth on `https://openagents.com`.

## Deterministic test flow

1. `Auth`
- From iOS, send email code and verify.
- Capture API result IDs: `session_id`, token issuance timestamp.

2. `Worker selection`
- Load workers from iOS.
- Confirm selected worker matches expected desktop/shared preference.
- Capture selected `worker_id`.

3. `Stream connect + handshake`
- Connect Khala stream from iOS.
- Send iOS handshake.
- Wait for desktop ack.
- Capture `handshake_id` and ack latency (ms).

4. `Thread start + message turn`
- Send first user message from iOS (turn start).
- Capture `request_id`, resulting `thread_id`, `turn_id`.

5. `Interrupt`
- Trigger interrupt while turn is active.
- Capture interrupt `request_id`, outcome status.

6. `Reconnect + resume`
- Force transient disconnect (toggle network or background/foreground cycle).
- Reconnect stream and verify replay/resume from watermark.
- Confirm no duplicated applied events after reconnect.

## Expected protocol event sequence

Expected order (allowing extra heartbeat frames between steps):

1. `worker.event` with `method=ios/handshake`
2. `worker.event` with `method=desktop/handshake_ack`
3. `worker.request` for `method=turn/start`
4. `worker.response` for `method=turn/start` (or terminal `worker.error`)
5. `worker.request` for `method=turn/interrupt`
6. `worker.response` for `method=turn/interrupt` (or terminal `worker.error`)
7. After reconnect: replay/update batch resumes strictly after prior watermark; no duplicated terminal receipts

## Pass criteria

1. Auth succeeds without stale-response race acceptance.
2. Worker selection uses expected desktop/shared ranking.
3. Handshake ack matches the sent `handshake_id`.
4. Turn start and interrupt terminal receipts reconcile once.
5. Reconnect resumes from watermark without duplicate UI side effects.

## Required evidence in issue comment

Include direct links/attachments with:

1. build SHA and test date
2. `worker_id`, `handshake_id`, `thread_id`, `turn_id`, request IDs
3. ordered event log snippet showing expected protocol sequence
4. reconnect/resume proof (watermark before/after + no-duplicate confirmation)
5. pass/fail summary with any deviations
