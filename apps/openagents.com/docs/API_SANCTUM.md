# OpenAgents API (Sanctum)

This app now exposes a token-authenticated API under `/api` using Laravel Sanctum.

## Auth model

- Guard: `auth:sanctum`
- Token type: Sanctum personal access token (`Authorization: Bearer <token>`)
- Required setup in app:
  - `php artisan install:api`
  - `HasApiTokens` on `App\Models\User`
  - `routes/api.php` protected by `auth:sanctum`
  - `bootstrap/app.php` with `$middleware->statefulApi()` for SPA/session compatibility

## Bootstrap Signup (Local/Testing Only)

A programmatic signup endpoint is available only in local/testing environments when enabled by env:

- `POST /api/auth/register`

Guardrails:
- Disabled by default (`OA_API_SIGNUP_ENABLED=false`).
- Optional domain allowlist via `OA_API_SIGNUP_ALLOWED_DOMAINS` (comma-separated).
- Route is not registered outside `local`/`testing` app environments.

Request body example:

```json
{
  "email": "creator.openagents.com",
  "name": "Creator",
  "tokenName": "staging-e2e",
  "createAutopilot": true,
  "autopilotDisplayName": "Creator Agent"
}
```

Response includes a one-time bearer token under `data.token` plus the created/resolved user and optional autopilot.

## Endpoints

### Identity

- `GET /api/me`

### Token management

- `GET /api/tokens`
- `POST /api/tokens`
- `DELETE /api/tokens/current`
- `DELETE /api/tokens/{tokenId}`
- `DELETE /api/tokens`

### Chat

- `GET /api/chats`
- `POST /api/chats`
- `GET /api/chats/{conversationId}`
- `GET /api/chats/{conversationId}/messages`
- `GET /api/chats/{conversationId}/runs`
- `GET /api/chats/{conversationId}/runs/{runId}/events`
- `POST /api/chats/{conversationId}/stream`
- `POST /api/chat/stream` (query param fallback: `conversationId`)

### User settings

- `GET /api/settings/profile`
- `PATCH /api/settings/profile`
- `DELETE /api/settings/profile`

### L402 data surfaces

- `GET /api/l402/wallet`
- `GET /api/l402/transactions`
- `GET /api/l402/transactions/{eventId}`
- `GET /api/l402/paywalls`
- `GET /api/l402/settlements`
- `GET /api/l402/deployments`
- `POST /api/l402/paywalls` (admin-only)
- `PATCH /api/l402/paywalls/{paywallId}` (admin-only)
- `DELETE /api/l402/paywalls/{paywallId}` (admin-only)

Optional query for wallet/transactions/paywalls/settlements/deployments:
- `autopilot=<id-or-handle>` to scope analytics to one owned autopilot.
- Unknown autopilot => `404`, non-owned autopilot => `403`.

### Agent Payments (Spark)

Primary endpoints:

- `GET /api/agent-payments/wallet`
- `POST /api/agent-payments/wallet`
- `GET /api/agent-payments/balance`
- `POST /api/agent-payments/invoice`
- `POST /api/agent-payments/pay`
- `POST /api/agent-payments/send-spark`

Backward-compatible aliases:

- `GET /api/agents/me/wallet`
- `POST /api/agents/me/wallet`
- `GET /api/agents/me/balance`
- `POST /api/payments/invoice`
- `POST /api/payments/pay`
- `POST /api/payments/send-spark`

## Production token creation

Use the existing ops command:

```bash
php artisan ops:create-api-token chris@openagents.com "my-token" --abilities='*' --expires-days=30
```

For Cloud Run, we execute that command via the `openagents-migrate` job by overriding args:

```bash
gcloud run jobs execute openagents-migrate \
  --region us-central1 \
  --args="artisan,ops:create-api-token,chris@openagents.com,my-token,--abilities=*,--expires-days=30" \
  --wait
```

Then read execution logs to retrieve the one-time plaintext token.

## Maintenance-mode testing note

If production is in maintenance mode, API requests also return `503` until bypassed.

Current bypass flow:
1. Visit maintenance secret route to set `laravel_maintenance` cookie.
2. Call API with both cookie + bearer token.

Example:

```bash
curl -I https://openagents.com/<maintenance-secret>
curl -H "Authorization: Bearer <token>" https://openagents.com/api/me
```

## Validation coverage

Pest coverage for API/Sanctum lives in:

- `tests/Feature/Api/V1/AuthenticationAndTokensTest.php`
- `tests/Feature/Api/V1/ChatApiTest.php`
- `tests/Feature/Api/V1/L402ApiTest.php`
- `tests/Feature/Api/V1/AgentPaymentsApiTest.php`
- `tests/Feature/SparkWalletInvoicePayerTest.php`
- `tests/Feature/Api/V1/ProfileApiTest.php`
- `tests/Feature/CreateApiTokenCommandTest.php`

## OpenAPI

This API is documented with auto-generated OpenAPI output at:

- `/openapi.json`

Generation source is controller attributes + `app/OpenApi/*` factory classes.
