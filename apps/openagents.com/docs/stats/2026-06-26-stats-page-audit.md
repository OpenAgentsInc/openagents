# Stats Page Audit

Date: 2026-06-26

This note tracks the public `/stats` rollout covering the Khala tokens-served counter, Central-time history, model/provider mix, and public/all viewing mode.

## Status

| Issue | Scope | Status |
| --- | --- | --- |
| #6330 | Central-time token history endpoint dependency | Closed before this slice; `/api/public/khala-tokens-served/history` remains the history source for `/stats`. |
| #6351 | Public model/provider mix endpoint | Closed in commit `8c853f2df9`; `/api/public/khala-tokens-served/model-mix` is live for the page. |
| #6352 | Public `/stats` page | Closed in commit `a282066552`; `/stats` renders the live Khala counter, America/Chicago day chart, and aggregate model-family mix. |
| #6353 | Public stats epic | Closed after #6330, #6351, and #6352 were all closed and `/stats` returned HTTP 200. |

## 5a /stats Page

The `/stats` page is the public product surface for live Khala usage stats. It shows the network-wide token counter, an America/Chicago per-day history chart, and model/provider mix from the aggregate public endpoints:

- `GET /api/public/khala-tokens-served`
- `GET /api/public/khala-tokens-served/history?bucket=day&timezone=America%2FChicago&window=30d`
- `GET /api/public/khala-tokens-served/model-mix?window=30d`

The page must not expose user, team, account, raw provider, raw model, prompt, completion, key, wallet, payment, settlement, or routing material. Any future authenticated or operator-only expansion must stay outside the default public projection.

### Visual Rendering Contract

The live Khala tokens-served total must remain inside its stat panel as the
counter grows. The number uses card-width-aware sizing and a no-wrap display so
nine-digit values such as `285,022,051` fit in the three-column `/stats`
dashboard without horizontal page overflow.

The per-day history chart must show visible day/value labels in addition to the
SVG bars and screen-reader text fallback. The compact `/stats` chart displays
the latest contiguous America/Chicago daily run, capped at four days, so sparse
older buckets such as an isolated `2026-06-11` event do not appear beside the
current `2026-06-24` onward run. Each displayed day appears as a compact
`MM/DD` + value cell; the peak day is highlighted so the daily maximum is
obvious without hovering. Bar columns and value cells share the same grid column
so the visible day/value number sits directly under its bar. The latest
America/Chicago day may add a black, green-striped end-of-day projection segment
above the observed green segment; it is calculated from the public
`generatedAt` timestamp's Central-time elapsed day seconds through midnight and
is labeled as an `EOD` estimate, not counted as observed tokens.

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
