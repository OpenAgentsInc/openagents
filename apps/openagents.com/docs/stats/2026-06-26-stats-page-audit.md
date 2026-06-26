# Stats Page Audit

Date: 2026-06-26

This note tracks the public `/stats` rollout covering the Khala tokens-served counter, Central-time history, model/provider mix, and public/all viewing mode.

## Status

| Issue | Scope | Status |
| --- | --- | --- |
| #6330 | Central-time token history endpoint dependency | Closed before this slice; `/api/public/khala-tokens-served/history` remains the history source for `/stats`. |
| #6351 | Public model/provider mix endpoint | Implemented by this slice. |
| #6352 | Public `/stats` page | Pending follow-up slice in this rollout. |
| #6353 | Public stats epic | Pending until #6351 and #6352 are both closed. |

## 5a /stats Page

The `/stats` page is the intended public product surface for live Khala usage stats. It should show the network-wide token counter, a Central-time per-day history chart, model/provider mix, and a public/all toggle that defaults to public.

The page must not expose user, team, account, raw provider, raw model, prompt, completion, key, wallet, payment, settlement, or routing material. Any future authenticated or operator-only expansion must stay outside the default public projection.

## 5b Model Mix Endpoint

`GET /api/public/khala-tokens-served/model-mix?window=30d` returns:

```json
{
  "schemaVersion": "openagents.public_khala_model_mix.v1",
  "window": "30d",
  "generatedAt": "2026-06-26T00:00:00.000Z",
  "totalTokens": 0,
  "groups": [
    {
      "family": "glm",
      "label": "GLM family",
      "tokens": 0,
      "reqs": 0,
      "pct": 0
    }
  ],
  "staleness": {
    "mode": "live_at_read"
  }
}
```

Supported windows are `today`, `7d`, `30d`, and `all`; the default is `30d`.

The projection is live-at-read over `token_usage_events`, aggregates input plus output tokens, and excludes exact `demand_kind=internal` rows. It remains public-safe by collapsing provider/model identifiers before serving.

## 5c Canonical Model Grouping

The public grouping vocabulary is intentionally small:

| Family | Label | Matching intent |
| --- | --- | --- |
| `glm` | GLM family | Reap, GLM-4, Z.ai/Zhipu, and Hydralisk GLM routes. |
| `fireworks_deepseek` | Fireworks DeepSeek | Fireworks-hosted or DeepSeek model/provider rows. |
| `pylon_codex` | Pylon-Codex | Pylon-Codex and ChatGPT-Codex rows. |
| `gpt_oss` | GPT-OSS | GPT-OSS model/provider rows. |
| `gemini` | Gemini | Gemini, Google, and Vertex rows. |
| `other` | Other | Public-safe fallback for everything else. |

Percentages are rounded percentages of the public token total. Tests cover family grouping, percentage totals near 100%, internal-row exclusion, and staleness wrapping.

## Public Safety

The model-mix endpoint is aggregate-only and uses the shared public projection redaction posture:

- no per-user, team, account, raw provider, or raw model material;
- no prompt, completion, request body, API key, wallet, payment, settlement, or secret material;
- no payout, settlement, routing, provider, or public-claim authority.
