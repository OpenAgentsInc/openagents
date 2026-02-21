# Autopilot iOS Real-Device Codex Handshake Runbook

Status: Active  
Last updated: 2026-02-20  
Owners: iOS + Desktop + Runtime + Web teams

## Purpose

Provide an operator-ready, <=10 minute procedure to validate the iOS real-device handshake path:

`ios/handshake` -> runtime event append -> Khala websocket delivery -> desktop ack -> `desktop/handshake_ack`

This runbook is for the public Laravel runtime APIs:

- `GET /api/runtime/codex/workers`
- `GET /api/runtime/codex/workers/{workerId}`
- `POST /api/runtime/codex/workers/{workerId}/events`
- `POST /api/sync/token` (or `POST /api/khala/token` compatibility path)

## Automated Acceptance Harness (Canonical Gate)

Before manual device validation, run the automated harness suite:

```bash
# Runtime Khala channel replay/resume acceptance coverage
cd apps/runtime
mix test test/openagents_runtime_web/channels/sync_channel_test.exs

# Desktop proto-first Khala frame parsing + handshake retry/watermark harness
cd /Users/christopherdavid/code/openagents
cargo test -p autopilot-desktop runtime_codex_proto::tests

# iOS proto-first handshake decode + ack correlation tests
# Run `AutopilotTests` in Xcode for apps/autopilot-ios/Autopilot/Autopilot.xcodeproj
```

Gate rule:

- Manual runbook execution is only valid if this automated harness passes first.

## 1. Prerequisites

Desktop host:

- `apps/autopilot-desktop/` is available and buildable.
- Desktop has network access to the same OpenAgents environment as iOS.
- Desktop has a valid WorkOS email-code login for runtime sync (Runtime Login pane; CLI fallback available).

iOS device:

- Real device build installed from `apps/autopilot-ios/Autopilot/`.
- iOS app defaults to `https://openagents.com` automatically (no manual base URL entry).
- iOS user signs in in-app via email code (no manual bearer token paste).
- iOS user and desktop user session must resolve to the same worker owner scope.

Operator tools:

- `curl`, `jq`, `rg`, `uuidgen`
- Optional: `kubectl` (runtime logs), `gcloud` (Laravel Cloud Run logs)

## 2. Desktop Launch Configuration

Authenticate desktop with the same WorkOS user scope as iOS:

```bash
cd /Users/christopherdavid/code/openagents
cargo run -p autopilot-desktop
```

In desktop UI:

1. Open hotbar slot `AU` (`Auth`) to show Runtime Login pane.
2. Enter your email, click `Send code`.
3. Enter the verification code, click `Verify`.
4. Confirm status in the pane (`Token: present`, user email shown).

Optional CLI fallback (automation/headless):

```bash
cargo run -p autopilot-desktop -- auth login --email "<you@domain.com>"
```

Expected:

- Desktop starts normally.
- Desktop uses persisted WorkOS-issued runtime token and creates/reattaches runtime worker IDs like `desktopw:<thread-id>`.
- Desktop begins runtime worker heartbeat and Khala websocket sync loop.

Optional overrides (debug/local):

```bash
export OPENAGENTS_RUNTIME_SYNC_BASE_URL="https://openagents.com"
export OPENAGENTS_RUNTIME_SYNC_TOKEN="<override-token>"
export OPENAGENTS_RUNTIME_SYNC_WORKSPACE_REF="desktop://$(pwd)"
export OPENAGENTS_RUNTIME_SYNC_CODEX_HOME_REF="file://$HOME/.codex"
export OPENAGENTS_RUNTIME_SYNC_WORKER_PREFIX="desktopw"
export OPENAGENTS_RUNTIME_SYNC_HEARTBEAT_MS="30000"
```

## 3. iOS Device Configuration

In the iOS Codex handshake screen:

1. Enter your email address and tap `Send Code`.
2. Enter the verification code and tap `Verify`.
3. Tap `Load Workers`.
4. Select the target worker.
5. Tap `Connect Stream`.

Expected:

- `Stream: live`
- Worker snapshot and latest sequence are visible.

## 4. 10-Minute Handshake Procedure

Set shared shell vars for verification commands:

```bash
export OA_BASE="https://openagents.com"
export OA_TOKEN="<ios-or-operator-bearer-token>"
export WORKER_ID="<selected-worker-id>"
export REQUEST_ID="ioshs-$(uuidgen | tr '[:upper:]' '[:lower:]')"
```

Step 1: Confirm worker visibility.

```bash
curl -sS \
  -H "Authorization: Bearer $OA_TOKEN" \
  -H "x-request-id: ${REQUEST_ID}-list" \
  "$OA_BASE/api/runtime/codex/workers?limit=100" | jq .
```

Step 2: Confirm snapshot for selected worker.

```bash
curl -sS \
  -H "Authorization: Bearer $OA_TOKEN" \
  -H "x-request-id: ${REQUEST_ID}-snapshot" \
  "$OA_BASE/api/runtime/codex/workers/$WORKER_ID" | jq .
```

Step 3: In iOS app, verify stream status is `live`, then tap `Send Handshake`.

Expected UI:

- `Handshake: waiting ack (...)` then `success (...)` within 30s.

Step 4: Confirm Khala live updates contain both handshake request and desktop ack.

Preferred checks:

1. iOS Debug tab `Recent Events` shows `ios/handshake` then matching `desktop/handshake_ack`.
2. Desktop logs show handshake ingestion followed by emitted ack.

## 5. Expected Timeline

1. iOS emits `POST /api/runtime/codex/workers/{workerId}/events` with:
   - `event_type=worker.event`
   - `source=autopilot-ios`
   - `method=ios/handshake`
   - `handshake_id`, `device_id`, `occurred_at`
2. Runtime appends durable worker event (`seq=N`) and exposes it on Khala topic `runtime.codex_worker_events`.
3. Desktop Khala websocket loop reads event `seq=N`, matches handshake envelope, emits ack event:
   - `source=autopilot-desktop`
   - `method=desktop/handshake_ack`
   - same `handshake_id`
4. Runtime appends ack event (`seq=N+1`) and delivers it to iOS over Khala websocket.
5. iOS marks handshake success only when matching `handshake_id` ack is observed.

## 6. Correlation and Trace Lookup

Use explicit correlation headers in manual API calls:

```bash
export TRACEPARENT="00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01"
export TRACESTATE="openagents=ios-handshake"
export XRID="ioshs-$(uuidgen | tr '[:upper:]' '[:lower:]')"
```

Example correlated ingest request:

```bash
HANDSHAKE_ID="hs-$(date +%s)"
curl -i -sS -X POST \
  -H "Authorization: Bearer $OA_TOKEN" \
  -H "Content-Type: application/json" \
  -H "traceparent: $TRACEPARENT" \
  -H "tracestate: $TRACESTATE" \
  -H "x-request-id: $XRID" \
  "$OA_BASE/api/runtime/codex/workers/$WORKER_ID/events" \
  -d "{
    \"event\": {
      \"event_type\": \"worker.event\",
      \"payload\": {
        \"source\": \"autopilot-ios\",
        \"method\": \"ios/handshake\",
        \"handshake_id\": \"$HANDSHAKE_ID\",
        \"device_id\": \"device_runbook\",
        \"occurred_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
      }
    }
  }"
```

Correlate by `x-request-id` and `handshake_id`:

Kubernetes runtime logs:

```bash
kubectl -n <RUNTIME_NAMESPACE> logs -l app=runtime --since=30m \
  | rg "$XRID|$HANDSHAKE_ID|desktop/handshake_ack|ios/handshake"
```

Laravel Cloud Run logs:

```bash
gcloud logging read \
  "resource.type=cloud_run_revision AND resource.labels.service_name=openagents-web AND textPayload:$XRID" \
  --project openagentsgemini --limit 50 --format='value(timestamp,textPayload)'
```

Khala sequence correlation:

- Khala update `watermark` / payload `seq` equals runtime worker event `seq`.
- Confirm `ios/handshake` appears at `seq=N` and `desktop/handshake_ack` at `seq>N`.

## 7. Troubleshooting Matrix

| Symptom | Likely cause | Confirm | Deterministic fix |
|---|---|---|---|
| `401` on worker APIs | invalid/expired bearer token | `curl` list workers returns `401` | refresh token, re-save iOS auth token, retry `Load Workers` |
| `403` on snapshot/events/token | ownership mismatch | snapshot/events/token returns `forbidden` | use same principal scope for iOS token and desktop sync token |
| handshake timeout (no ack in 30s) | desktop not consuming Khala updates or not synced worker | iOS shows waiting timeout; no `desktop/handshake_ack` in recent events | restart desktop with `OPENAGENTS_RUNTIME_SYNC_*`, verify worker ID, reconnect stream |
| `410 stale_cursor` | watermark below retention floor | Khala `sync:error` code `stale_cursor` | reset local watermark and replay from latest snapshot |
| repeated duplicate handshake acks missing | handshake already acked by desktop dedupe | stream already contains same `handshake_id` ack | generate a new `handshake_id` (normal behavior) |

## 8. Release Validation Checklist (Pass/Fail)

- [ ] Automated harness gate passed (`sync_channel_test.exs`, desktop `runtime_codex_proto::tests`, iOS `AutopilotTests`).
- [ ] Desktop authenticated via `autopilot-desktop -- auth login` (or equivalent valid runtime sync token source).
- [ ] iOS app can load workers and select target worker.
- [ ] iOS stream enters `live` state before handshake attempt.
- [ ] Sending handshake emits waiting state then success within 30s.
- [ ] Recent events show both `ios/handshake` and `desktop/handshake_ack` with matching `handshake_id`.
- [ ] `desktop/handshake_ack` `seq` is strictly greater than `ios/handshake` `seq`.
- [ ] No `401/403/409/410` errors during happy path.
- [ ] Correlation data captured (`x-request-id`, optional `traceparent`, `handshake_id`, Khala seq/watermark values).

Release decision rule:

- PASS only if all checklist items are true.
- FAIL if any item is false; open follow-up issue with captured correlation IDs and exact failing step.

## 9. Dry-Run Notes (2026-02-20)

Dry-run type: operator procedure walkthrough + contract verification (non-device simulation).

Validation notes:

1. API paths and handshake envelope fields were validated against regression suites:
   - `cd apps/runtime && mix test test/openagents_runtime_web/channels/sync_channel_test.exs`
   - `cd apps/openagents.com && php artisan test --filter=RuntimeCodexWorkersApiTest`
   - Result snapshot:
     - runtime suite: `10 tests, 0 failures`
     - Laravel suite: `9 passed (41 assertions)`
2. The checklist steps map directly to implemented iOS/desktop handshake methods:
   - iOS request: `method=ios/handshake`
   - desktop ack: `method=desktop/handshake_ack`
3. Timebox estimate from walkthrough: ~8 minutes excluding token acquisition.

## 10. References

- `apps/autopilot-ios/docs/codex-connection-roadmap.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `apps/runtime/docs/OBSERVABILITY.md`
- `apps/runtime/docs/OPERATIONS_ALERTING.md`
- `docs/autopilot/testing/PROD_E2E_TESTING.md` (legacy trace-retrieval background)
- `docs/autopilot/testing/TRACE_RETRIEVAL.md` (legacy trace-retrieval background)
