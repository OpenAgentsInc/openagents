# OpenCode Provider Config & Model Selector

> Focused note for engineers wiring `openagents/khala` in OpenCode.

## OpenAI-Compatible Base URL

OpenCode's custom provider config uses `@ai-sdk/openai-compatible`, which POSTs to `<baseURL>/chat/completions`.

```
baseURL: "https://openagents.com/api/v1"
```

This hits our live `POST /api/v1/chat/completions`, model `openagents/khala`.

## The Model Key vs `api.id` Distinction

OpenCode resolves the in-TUI model selector path as:

```
providerId / model.api.id
```

Two separate concepts:

| Field | Purpose | Example |
|---|---|---|
| JSON **key** under `models` | TUI model key segment | `openagents/khala` or `khala` |
| `api.id` (optional override) | What gets sent as `"model"` upstream | defaults to the JSON key |

If you set model key `openagents/khala` with no `api.id` override, the selector renders `openagents/openagents/khala` (doubled) — correct upstream (sends `{"model": "openagents/khala"}`) but ugly UX.

## Doubled Selector Problem — Disassembly

| Config form | TUI selector | Upstream `model` field |
|---|---|---|
| model key `openagents/khala`, no `api.id` | `openagents/openagents/khala` | `openagents/khala` ✅ |
| model key `khala`, no `api.id` | `openagents/khala` ✅ | `khala` ❌ (wrong model id) |

Neither naive form works cleanly.

## Clean Recipe

Override `api.id` explicitly to decouple the TUI display key from the upstream model id:

```jsonc
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
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  },
  "model": "openagents/khala"
}
```

| Property | Value | Why |
|---|---|---|
| Model JSON **key** | `khala` | Selector reads `openagents/khala` — clean |
| `api.id` override | `openagents/khala` | Upstream sends `{"model": "openagents/khala"}` — correct |
| `model` (default) | `openagents/khala` | OpenCode's default model selection |

Verified: OpenCode's `packages/core/src/plugin/provider/opencode.ts:125` applies `config.id` when `api.id` is set (`if (config.id !== undefined) model.api.id = config.id`).

## Key Minting

```bash
curl -X POST https://openagents.com/api/keys/free
# returns {"key": "oa_agent_..."}
export OPENAGENTS_API_KEY="oa_agent_..."
```

Free tier: 200 requests / 200,000 tokens per UTC day per key. Over-quota → 402 `insufficient_credits`.

## Summary

| Problem | Cause | Fix |
|---|---|---|
| Double segment `openagents/openagents/khala` in TUI | model key matches API model id | Use short model key + `api.id` override |
| Wrong upstream model id | Short model key with no override | Set `api.id` to `"openagents/khala"` |
| Both right | — | Recipe above |

**Status:** Internal recipe verified against OpenCode provider schema. Tool-call compatibility (issue #6232 — content arrays, tool-call deltas) deployed but needs production smoke before public publication. No changes to other files. No commits. No pushes.
