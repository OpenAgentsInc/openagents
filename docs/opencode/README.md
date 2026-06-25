# OpenCode/Khala Docs

Planning memos and session exports from the five concurrent Khala/OpenCode
planning sessions run on 2026-06-25. These sessions were read-only planning
rounds against the Khala inference GTM push strategy
([`docs/inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md)).
The north-star metric is **tokens served per day**; the work below routes
internal agents through Khala (Pillar 1), lands ecosystem-tool integrations
starting with OpenCode (Pillar 2), and benchmarks Khala on a ladder of
opponents (Pillar 3).

## Current Recipe

Use this `opencode.json` in a project root, or in
`~/.config/opencode/opencode.json` for global use:

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
          "tool_call": true,
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

Mint a free key with `POST https://openagents.com/api/keys/free` and export the
returned `credential.token` as `OPENAGENTS_API_KEY`. The free tier is currently
2,000 requests / 2,500,000 tokens per UTC day per key; over-quota requests should
surface a legible 402.

Verification before public promotion: OpenCode must complete a tool-calling
coding task, stream tokens normally, increment the public Khala tokens-served
counter by the recorded usage delta, and show a readable 402/quota error.

## Sessions

| Session title | Final output | One-line description |
|---|---|---|
| `khala-opencode-adoption` | [`khala-opencode-adoption-final-output.md`](./khala-opencode-adoption-final-output.md) | Exacts the OpenCode config recipe, model-selector path, and "what to test" checklist for the first external tool integration. |
| `khala-tool-compat` | [`khala-tool-compat-final-output.md`](./khala-tool-compat-final-output.md) | Audits the gateway's API compatibility against OpenCode's tool-calling, streaming, content arrays, and error surfaces; flags bug #6232. |
| `khala-head-to-head-gym` | [`khala-head-to-head-gym-final-output.md`](./khala-head-to-head-gym-final-output.md) | Designs the benchmarking gym (GYM) ladder — Khala vs BigPickle, free models, then paid frontier — on cost-per-accepted-outcome, verified-rate, and latency. |
| `opencode-usage-audit-inventory` | [`opencode-usage-audit-inventory-final-output.md`](./opencode-usage-audit-inventory-final-output.md) | Catalogs every OpenCode surface touched by the integration (auth, provider schema, config files, tool registry, model limits). |
| `khala-internal-dogfood` | [`khala-internal-dogfood-final-output.md`](./khala-internal-dogfood-final-output.md) | Routes qa-runner, OpenCode, Autopilot, Raynor, Probe, and Verse inference through Khala — Pillar 1's concrete dogfood pipeline. |

## Raw Exports

The direct OpenCode SQLite exports are in [`raw/`](./raw/):

- `sessions.json`
- `session_messages.json`
- `messages.json`
- `parts.json`
- `session_inputs.json`
- `todos.json`
