# EP212 Laravel Rehearsal Gate (Deterministic + Live + UI)

This runbook defines the release gate for EP212 recording readiness on the Laravel stack (`apps/openagents.com`). The gate is mandatory before recording the demo. Recording is blocked until the deterministic matrix passes, live smoke passes, and UI checklist passes, with artifacts logged in the rehearsal status file.

## 1) Deterministic Gate (Local + CI)

The deterministic gate is implemented in `apps/openagents.com/tests/Feature/Ep212RehearsalGateTest.php` and is designed to run without external dependencies.

Run locally:

```bash
cd apps/openagents.com
composer test:ep212
composer test:ep212:junit
```

The matrix covers four EP212-critical paths:

1. paid success
2. cached repeat
3. blocked pre-payment
4. approval lifecycle

Artifact output from `composer test:ep212:junit` is written to:

- `apps/openagents.com/output/ep212-rehearsal/ep212-deterministic-junit.xml`

CI gate wiring:

- `apps/openagents.com/.github/workflows/tests.yml`
- Step: `EP212 deterministic rehearsal gate`
- Artifact upload: `ep212-deterministic-junit-php<version>`

## 2) Production Live Smoke Checklist (API + Chat)

Use this for rehearsal against `https://openagents.com` before recording.

### 2.1 Preconditions

1. You have a valid Sanctum token for a rehearsal user.
2. Rehearsal user has wallet funding available for at least one paid call.
3. Domain allowlist and spend caps are configured for the intended EP212 endpoints.
4. If maintenance mode is enabled, your browser/API client has maintenance bypass cookie.

### 2.2 API Baseline Checks

```bash
BASE_URL="https://openagents.com"
TOKEN="<sanctum-token>"

curl -sS -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/me"
curl -sS -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/l402/wallet"
curl -sS -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/l402/transactions?limit=10"
```

Expected: authenticated identity, wallet payload, and transactions payload return `200`.

### 2.3 API Chat Fallback Flow (if UI capture fails)

Create thread:

```bash
CONV_ID=$(curl -sS -X POST -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/chats" | jq -r '.data.id')
echo "$CONV_ID"
```

Send prompt via stream endpoint:

```bash
curl -N -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$BASE_URL/api/chats/$CONV_ID/stream" \
  -d '{"messages":[{"id":"m1","role":"user","content":"Use the sats4ai L402 text generation endpoint. Max spend 100 sats. Ask me to approve before paying. Question: what is one short fact about Bitcoin?"}]}'
```

Run the same request again for cache-repeat verification (same endpoint, same payload). Then run a blocked path prompt:

```text
Use the expensive L402 endpoint with max spend 100 sats and tell me the result.
```

Verify outcomes via API:

```bash
curl -sS -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/l402/transactions?limit=20" | jq .
```

Expected in recent entries:

1. one paid completion
2. one cached completion
3. one blocked pre-payment result

## 3) UI Rehearsal Checklist (openagents.com)

Run in browser on `https://openagents.com`.

1. Start a chat and send this exact prompt:

```text
Use the sats4ai L402 text generation endpoint. Max spend 100 sats. Ask me to approve before paying. Question: what is one short fact about Bitcoin?
```

2. Confirm approval intent UI is shown before payment.
3. Click approve and confirm the payment-complete state appears with proof reference.
4. Send the same prompt again and confirm cached behavior (no second payment).
5. Send blocked-case prompt:

```text
Use the expensive L402 endpoint with max spend 100 sats and report the result.
```

6. Confirm explicit pre-payment block reason is shown.
7. Open and verify L402 pages reflect the same run outcomes:
   - `/l402`
   - `/l402/transactions`
   - `/l402/paywalls`
   - `/l402/deployments`

## 4) Recording Prompts and Fallbacks (Do Not Drift)

Use only the exact prompts in this runbook for recording consistency. If the chat UI stream capture is unstable during recording, use the API fallback flow from section 2.3 and then show `/api/l402/transactions` plus `/l402/transactions` as authoritative receipt views.

## 5) Artifact and Log Recording Requirement

After every rehearsal run, update:

- `docs/lightning/status/20260217-ep212-laravel-rehearsal-gate-log.md`

Include:

1. deterministic gate command result
2. artifact paths or CI artifact names
3. live smoke pass/fail notes
4. UI checklist pass/fail notes
5. links to relevant run/deploy logs if used

No EP212 recording should be made unless the latest log entry shows all gate sections as passed.
