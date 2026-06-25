# OpenCode to Khala Recipe

> Authoritative recipe for issue #6239. This is repo documentation, not public
> marketing copy. Public site copy still follows the Khala promise gate.

## What You Get

- OpenAI-compatible base URL: `https://openagents.com/api/v1`
- Model: `openagents/khala`
- Free key endpoint: `POST https://openagents.com/api/keys/free`
- Free tier: 2,000 requests/day and 2,500,000 tokens/day per key, reset at
  UTC midnight

## 1. Mint A Free Key

```sh
curl -fsS -X POST https://openagents.com/api/keys/free
```

The response includes `credential.token`. It starts with `oa_agent_` and is only
shown once, so save it locally:

```sh
export OPENAGENTS_API_KEY="oa_agent_..."
```

Do not commit the token or paste it into issue comments, traces, or shared logs.

## 2. Configure OpenCode

Put this in your project or user `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "openagents": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "OpenAgents",
      "options": {
        "baseURL": "https://openagents.com/api/v1",
        "apiKey": "{env:OPENAGENTS_API_KEY}"
      },
      "models": {
        "khala": {
          "name": "Khala",
          "api": {
            "id": "openagents/khala"
          },
          "tool_call": true,
          "limit": {
            "context": 128000,
            "output": 65536
          }
        }
      }
    }
  },
  "model": "openagents/khala"
}
```

## Selector Decision

OpenCode displays models as `providerId/modelKey`.

Use:

- Provider id: `openagents`
- JSON model key: `khala`
- Upstream model id: `api.id = "openagents/khala"`
- OpenCode selector/default model: `openagents/khala`

Do not use a JSON model key of `openagents/khala` for the published recipe. That
makes OpenCode display `openagents/openagents/khala`. It still sends the right
upstream model id, but it is ambiguous for users and no longer needed because
OpenCode supports the per-model `api.id` override.

## 3. Smoke Test

From any repo that contains `docs/faq/khala-inference-quickstart.md`:

```sh
opencode run \
  --pure \
  --agent plan \
  --model openagents/khala \
  --dir /path/to/repo \
  --dangerously-skip-permissions \
  'Use tools to read docs/faq/khala-inference-quickstart.md. Reply with exactly: base=<base-url>; model=<model-id>'
```

Expected final answer:

```txt
base=https://openagents.com/api/v1; model=openagents/khala
```

## Verification Checklist

- Tool calling: OpenCode uses a file-read tool and completes the task.
- Streaming: the TUI streams the answer without malformed SSE or dropped
  tool-call deltas.
- Token counter: read `GET /api/public/khala-tokens-served` before and after
  the run; the counter increases after the OpenCode request.
- 402/quota path: after a key exhausts either 2,000 requests/day or 2,500,000
  tokens/day, Khala returns HTTP 402 with a readable OpenAI-style error body.
  OpenCode should surface the message instead of crashing.

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---|---|---|
| `401` or auth error | `OPENAGENTS_API_KEY` is unset or not an `oa_agent_` token | Export the `credential.token` from the free key response |
| Model selector shows `openagents/openagents/khala` | The JSON model key is `openagents/khala` | Use model key `khala` with `api.id: "openagents/khala"` |
| `402` quota error | The free key is over daily request or token quota | Wait for UTC-day reset or use a funded key |
| Tool loop stalls | Running an old gateway build or stale OpenCode config | Confirm this recipe is installed and rerun the smoke checklist |
