# OpenCode Free Tier 402 Playbook

> Khala free-tier inference: key minting, quota behavior, over-quota errors, and
> paid fallback — what OpenCode users and operators need to know.

## Free Key Minting

```
POST https://openagents.com/api/keys/free
```

Returns a normal `oa_agent_` bearer key. No signup, no payment instrument. The key
unlocks inference on `openagents/khala` at:

```
POST https://openagents.com/api/v1/chat/completions
```

The response includes the bearer key at `credential.token`. Store that key in
OpenCode via `/connect -> Other -> id "openagents"` (stored at
`~/.local/share/opencode/auth.json`) or export as `OPENAGENTS_API_KEY`. The provider
config in `~/.config/opencode/opencode.json`:

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
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  },
  "model": "openagents/khala"
}
```

## Daily Quota (Per-Key, Per-UTC-Day)

| Resource | Limit |
|----------|-------|
| Requests per day | **2,000** |
| Tokens per day | **2,500,000** |

Both counters reset at 00:00 UTC. The free tier is a **research preview** — enough
for a real "try it" session (a few coding turns) but not for sustained daily use.

## Over-Quota Behavior — 402 Payment Required

When a request would exceed either limit, the gateway returns HTTP **402 Payment
Required** with a JSON body shaped identically to the OpenAI API error format:

```json
{
  "error": {
    "message": "Free tier quota exceeded. Add credits at https://openagents.com/account or wait for UTC-day reset.",
    "type": "insufficient_quota",
    "code": "quota_exceeded",
    "param": null
  }
}
```

No tokens are consumed for a rejected 402 — only the request counter (1 request)
is debited if the request passes auth but fails the quota check on the write path.
*Current behavior: the 402 is returned before any model inference, so the request
counter IS debited but the token counter is NOT.*

## Paid/Credit Fallback

Over-quota requests fall through to the **normal balance + 402 gate**. If the key
has a positive credit balance, the request proceeds and is billed at the standard
Khala rate (see pricing model docs). If the balance is also zero or negative, the
same 402 shape is returned with a different message pointing to the credits page.

To add credits: visit `https://openagents.com/account` (payment methods, BTC
discount, credit purchase — owner-gated; not yet a public product promise).

## What OpenCode Should Show

When Khala returns 402:

| Symptom | What happens in OpenCode |
|---------|--------------------------|
| `402` with `quota_exceeded` | OpenCode's AI SDK provider (`@ai-sdk/openai-compatible`) surfaces the error message. The user sees: *"Free tier quota exceeded. Add credits at https://openagents.com/account or wait for UTC-day reset."* The TUI shows a red error banner; the session continues but the next tool call fails with the same error. |
| UTC-day reset | A key that was over-quota at 23:59 UTC is auto-replenished at 00:00 UTC. The next request succeeds without any config change. |
| No credits + over-quota | Same 402 shape with a message directing to the credits page. No fallback to a different model — OpenCode must either wait for reset or the user must add credits. |

Operators: the free-tier quota counters live in the `oa_agent_` key record in the
inference free-tier module (`inference-free-tier-key.ts`). The reset is a
UTC-midnight cron or on-read zero-check — verify which by reading the module.
Logs show `FREE_TIER_QUOTA_EXCEEDED` events with the key prefix, current
request/token counts, and limit.

---

**Status:** File created at `docs/opencode/opencode-free-tier-402-playbook.md`.
Single-focused playbook covering free key minting, 2,000-request/2.5M-token daily quota,
over-quota 402 JSON shape, paid balance fallback, and OpenCode error display.
