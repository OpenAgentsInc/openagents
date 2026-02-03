# OpenClaw API auth (API keys)

Managed OpenClaw endpoints live under:

```
https://openagents.com/api/openclaw/*
```

## API key auth (recommended)

Send your OpenAgents API token as a bearer token:

```bash
curl -H "Authorization: Bearer $OPENAGENTS_API_TOKEN" \
  "https://openagents.com/api/openclaw/runtime/status"
```

Optional compatibility header:

```
x-api-key: $OPENAGENTS_API_TOKEN
```

## Internal beta auth (fallback)

Server-to-server calls from `apps/web` may use internal headers:

```
X-OA-Internal-Key: <secret>
X-OA-User-Id: <workos user id>
```

If these headers are present, they take precedence. Otherwise, bearer token auth is used.

## Agent quick signup

Create a headless agent principal and receive an API token in one call:

```bash
curl -X POST "https://openagents.com/api/auth/agent/register" \
  -H "Content-Type: application/json" \
  -d '{"name":"demo-agent"}'
```

Optional gating (if `OA_REGISTER_KEY` is set on the API worker):

```
X-OA-Register-Key: <secret>
```

Response:

```json
{
  "ok": true,
  "data": {
    "user_id": "oa_agent_...",
    "api_token": "...",
    "created": true
  }
}
```
