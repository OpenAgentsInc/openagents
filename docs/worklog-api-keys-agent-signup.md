# Worklog: API keys + agent quick signup

## 2026-02-02 23:48:03 -0600
- Read spec + checklist and reviewed legacy Convex apiTokens implementation.
- Added Convex schema for users + api_tokens in apps/web; ported apiTokens functions and user helpers.
- Added Convex control HTTP endpoints for token resolution and agent registration.
- Implemented Rust API token resolution (Authorization Bearer / x-api-key), agent quick signup endpoint, and OpenClaw auth fallback to API tokens.
- Added basic IP-based rate limiting for agent registration and optional register-key gating.
- Documented OpenClaw API token auth and agent register endpoint.

### Files changed/added
- `apps/web/convex/schema.ts`
- `apps/web/convex/lib/users.ts`
- `apps/web/convex/users.ts`
- `apps/web/convex/apiTokens.ts`
- `apps/web/convex/control_auth.ts`
- `apps/web/convex/http.ts`
- `apps/api/src/lib.rs`
- `apps/api/src/openclaw/http.rs`
- `apps/api/docs/openclaw-auth.md`
- `apps/api/docs/README.md`
- `apps/api/README.md`

### Endpoints added/modified
- Convex (apps/web):
  - `POST /control/auth/resolve-token`
  - `POST /control/auth/agent/register`
- Rust API (apps/api):
  - `POST /auth/agent/register`
  - `/openclaw/*` now accepts `Authorization: Bearer <OPENAGENTS_API_TOKEN>` or `x-api-key` (internal headers still supported)

### Env / config
- Convex (apps/web): `OA_CONTROL_KEY` (required for control endpoints)
- API worker (apps/api):
  - `CONVEX_SITE_URL`
  - `CONVEX_CONTROL_KEY`
  - `OA_REGISTER_KEY` (optional; requires `X-OA-Register-Key` header when set)

### Curl examples
```bash
# Agent quick signup (public)
curl -X POST "https://openagents.com/api/auth/agent/register" \
  -H "Content-Type: application/json" \
  -d '{"name":"demo-agent"}'

# Token-based OpenClaw status
curl -H "Authorization: Bearer $OPENAGENTS_API_TOKEN" \
  "https://openagents.com/api/openclaw/runtime/status"

# Convex control resolve-token (internal)
curl -X POST "$CONVEX_SITE_URL/control/auth/resolve-token" \
  -H "x-oa-control-key: $CONVEX_CONTROL_KEY" \
  -H "Content-Type: application/json" \
  -d '{"token":"<OPENAGENTS_API_TOKEN>"}'
```

### Commands run
- `date '+%Y-%m-%d %H:%M:%S %z'`

### Verification
- Pending (see checklist; run build/tests before release).

### Notes / TODO
- Convex deployment required for new schema + control endpoints.
- Consider stronger rate limiting / CAPTCHA for `/auth/agent/register`.

## 2026-02-02 23:56:21 -0600
### Commands run
- `npm -C apps/api run build`

### Verification
- `npm -C apps/api run build` (succeeded; warnings about unused fields in `ResolvedApiToken` and `RuntimeError`).

## 2026-02-03 00:00:20 -0600
### Commands run
- `npm -C apps/api run build`

### Verification
- `npm -C apps/api run build` (succeeded; warning about unused fields in `RuntimeError` from existing code).
