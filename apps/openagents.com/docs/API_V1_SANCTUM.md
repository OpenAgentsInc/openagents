# OpenAgents API v1 (Sanctum)

This app now exposes a token-authenticated API under `/api/v1` using Laravel Sanctum.

## Auth model

- Guard: `auth:sanctum`
- Token type: Sanctum personal access token (`Authorization: Bearer <token>`)
- Required setup in app:
  - `php artisan install:api`
  - `HasApiTokens` on `App\Models\User`
  - `routes/api.php` protected by `auth:sanctum`
  - `bootstrap/app.php` with `$middleware->statefulApi()` for SPA/session compatibility

## Endpoints

### Identity / Admin

- `GET /api/v1/me`
- `GET /api/v1/admin/status` (requires `admin` middleware)

### Token management

- `GET /api/v1/tokens`
- `POST /api/v1/tokens`
- `DELETE /api/v1/tokens/current`
- `DELETE /api/v1/tokens/{tokenId}`
- `DELETE /api/v1/tokens`

### Chat

- `GET /api/v1/chats`
- `POST /api/v1/chats`
- `GET /api/v1/chats/{conversationId}`
- `GET /api/v1/chats/{conversationId}/messages`
- `GET /api/v1/chats/{conversationId}/runs`
- `GET /api/v1/chats/{conversationId}/runs/{runId}/events`
- `POST /api/v1/chats/{conversationId}/stream`
- `POST /api/v1/chat/stream` (query param fallback: `conversationId`)

### User settings

- `GET /api/v1/settings/profile`
- `PATCH /api/v1/settings/profile`
- `DELETE /api/v1/settings/profile`

### L402 data surfaces

- `GET /api/v1/l402/wallet`
- `GET /api/v1/l402/transactions`
- `GET /api/v1/l402/transactions/{eventId}`
- `GET /api/v1/l402/paywalls`
- `GET /api/v1/l402/settlements`
- `GET /api/v1/l402/deployments`

### Agent Payments (Spark)

Primary endpoints:

- `GET /api/v1/agent-payments/wallet`
- `POST /api/v1/agent-payments/wallet`
- `GET /api/v1/agent-payments/balance`
- `POST /api/v1/agent-payments/invoice`
- `POST /api/v1/agent-payments/pay`
- `POST /api/v1/agent-payments/send-spark`

Backward-compatible aliases:

- `GET /api/v1/agents/me/wallet`
- `POST /api/v1/agents/me/wallet`
- `GET /api/v1/agents/me/balance`
- `POST /api/v1/payments/invoice`
- `POST /api/v1/payments/pay`
- `POST /api/v1/payments/send-spark`

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
curl -H "Authorization: Bearer <token>" https://openagents.com/api/v1/me
```

## Validation coverage

Pest coverage for API/Sanctum lives in:

- `tests/Feature/Api/V1/AuthenticationAndTokensTest.php`
- `tests/Feature/Api/V1/ChatApiTest.php`
- `tests/Feature/Api/V1/L402ApiTest.php`
- `tests/Feature/Api/V1/AgentPaymentsApiTest.php`
- `tests/Feature/SparkWalletInvoicePayerTest.php`
- `tests/Feature/Api/V1/AdminAndProfileApiTest.php`
- `tests/Feature/CreateApiTokenCommandTest.php`

## OpenAPI

This API is documented with auto-generated OpenAPI output at:

- `/openapi.json`

Generation source is controller attributes + `app/OpenApi/*` factory classes.
