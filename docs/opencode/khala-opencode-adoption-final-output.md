# OpenCode Adoption Lane — Planning Memo

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


> Historical planning export from 2026-06-25. For implementation, use the
> authoritative recipe in [`opencode-khala-recipe.md`](./opencode-khala-recipe.md).
> Older doubled-selector examples below are preserved as planning context, not
> the current published path.

> Companion docs: [GTM push](../inference/2026-06-25-khala-inference-gtm-push.md) ·
> [Runbook & audit](../inference/2026-06-25-opencode-khala-runbook-and-audit.md) ·
> [Promise review](../promises/2026-06-25-khala-inference-push-promise-review.md) ·
> [Head-to-head gym](./khala-head-to-head-gym-final-output.md) ·
> [Tool compat audit](./khala-tool-compat-final-output.md)

## Context

**Big Pickle** (with a space) is the main free model in OpenCode — the default
open/free option users reach for without a paid provider. Khala must beat Big
Pickle on cost-per-accepted-outcome and verified-rate to be the natural
default. (See [head-to-head gym doc](./khala-head-to-head-gym-final-output.md)
for the full ladder: Khala vs Big Pickle → Khala vs free/open models → Khala
vs paid frontier.)

### 1. Exact User Recipe (verified against both the live API and the OpenCode provider schema)

The config is valid. OpenCode's `ConfigV2.Provider.Info` schema (at `packages/core/src/config/provider.ts` in the opencode repo) uses `api: { type: "aisdk", package: string, url?, settings? }`. The GTM doc's JSON maps directly: `npm` → `package`, `options` → `settings`. The OpenCode docs (`providers.mdx`) confirm the `@ai-sdk/openai-compatible` pattern matches any `/v1/chat/completions` endpoint.

Direct chat-completions smoke is **verified**: a plain request to `openagents/khala` returned `200`, reported `usage.total_tokens: 399`, and the public counter increased by exactly 399. Full OpenCode tool-loop smoke is gated on the #6232 compatibility fix (content arrays + tool-call payloads; see runbook).

**Operational recipe** (current published repo recipe, includes `tool_call: true` and the clean selector path):

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

Key verification:

| Property | Status |
|---|---|
| Base URL format matches provider schema | Confirmed — `baseURL` maps to `settings.baseURL` in the AISDK provider; the provider schema accepts `Record<string, unknown>` in `settings` |
| `{env:OPENAGENTS_API_KEY}` syntax | Confirmed — OpenCode docs document the `{env:VAR}` syntax |
| Model key `khala` plus `api.id: "openagents/khala"` sends the upstream model id | Confirmed — the AI SDK's `languageModel(api.id)` sends `api.id` as the `model` field in the POST body |
| Direct chat + token accounting | **Verified** — 399-token delta confirmed on public counter |
| Tool-call round-trip | **Gated on #6232** — content arrays and tool-call payloads under test |

### 2. Upstream/Provider Preset Opportunity

The GTM doc flags the **model-key double-doubling problem**: with provider id `openagents` and model key `openagents/khala`, the TUI selector renders `openagents/openagents/khala`. This is confirmed by OpenCode's model-key resolver (`modelKey = `${providerID}/${modelID}`` where `modelID = model.api.id`).

**The fix** (now in the recipe, not a server-side change):

```jsonc
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
```

By using model key `khala` and overriding `api.id` to `openagents/khala`, the TUI selector renders `openagents/khala` (clean!) while the upstream still sends `{"model": "openagents/khala"}` to our API. OpenCode's `packages/core/src/plugin/provider/opencode.ts:125` confirms `if (config.id !== undefined) model.api.id = config.id` — the per-model `api.id` override is supported.

The **provider preset** (upstream PR) is a separate, later item. The one-config
recipe is now published in repo docs; an upstream PR against OpenCode's built-in
provider list can remove the need to hand-write the JSON later.

### 3. Quota/Error UX

The free tier (2,000 requests / 2,500,000 tokens per UTC day) is enforced by `decideFreeTierQuota` in `inference-free-tier-key.ts`. Over-quota requests fall through to the **balance gate**, which returns a **402** with structured JSON:

```json
{"error": "insufficient_credits", "message": "Insufficient credits. Add credits at https://openagents.com/account"}
```

**What to test in OpenCode specifically:**

| Scenario | Expected Behavior |
|---|---|
| Within quota | Normal streaming completion |
| Over request quota | 402 — OpenCode should show the error message cleanly, not crash |
| Over token quota | Same 402 path |
| Exhausted and then reset next UTC day | Next request within quota succeeds |
| Mint a second key from same IP | Allowed up to 25/day; 26th gets 429 `free_key_mint_rate_limited` |

**Risk**: OpenCode may not handle a 402 gracefully in all paths (model listing vs. chat completion). Need to test and possibly document a workaround (e.g., "if you see a 402, top up or wait for your daily quota to reset"). Big Pickle (the default free OpenCode model) has no quota gate — Khala must make the quota ceiling legible, or users who hit the 2,000-request/2.5M-token ceiling will silently fall back to Big Pickle rather than top up.

### 4. Token-Counter Acceptance Checks

The public counter (`GET /api/public/khala-tokens-served` and `/history`) is already live, verified, and wired through `token-usage-ledger.ts`. The `ServedTokensRecorder` fires after every completion (including free-tier zero-debit ones via `withFreeTierKhala`). Acceptance checklist:

| Check | Method |
|---|---|
| Tokens increment on a free-tier request | Read counter before and after one completion |
| Tokens increment on the public history endpoint | Same pattern against `/history?window=7d&bucket=day` |
| OpenCode session tokens appear | Run a short OpenCode session against Khala, then check counter |
| Counter survives concurrent requests | Already verified under 24-wide concurrent stress (per GTM doc) |
| Idempotency on retry | Same request_id does not double-count |

### 5. Docs/Runbook Updates — Status

The runbook already exists at `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md`. Remaining gaps:

| Document | What it needs |
|---|---|
| `docs/opencode/opencode-khala-recipe.md` | **Exists.** Canonical config with model key `khala`, `api.id: "openagents/khala"`, free-key instructions, smoke command, token-counter check, and 402/quota checklist. |
| `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md` | **Exists.** Contains the operational smoke, key minting curl, selector decision, #6232 fix coverage, regression test list, and publication checklist. |
| `docs/inference/README.md` | Add a section "Ecosystem Adoption" linking to the runbook, and mention Big Pickle as the main free OpenCode model that sets the comparison bar. |
| `apps/openagents.com/AGENTS.md` or a companion doc | Add the one-config recipe and the "what to test" checklist so future agent sessions can verify OpenCode integration without re-researching. |
| `/khala` page copy (public) | Once through copy gate: "Point OpenCode at Khala with this one-config recipe" — keep it short, link to the full runbook. |

The runbook documents the **model-key doubling** (`openagents/openagents/khala` in the TUI selector) only as historical context. The published recipe uses the cleaner `api.id` override.

### 6. Sequence / Gating

In order of execution:

1. **Ship #6232** — Deploy the content-array + tool-call payload fix from clean `origin/main`. Production smoke the fix before any published recipe.
2. **Rerun OpenCode smoke** — With the fix live, run the runbook smoke against a dedicated `oa_agent_` key. Confirm tool calls, streaming, token counter movement, and all quota states. **Also smoke against Big Pickle** to establish the baseline tool-call experience users will compare Khala to.
3. **Update the runbook** — `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md` tracks the clean selector and smoke checklist.
4. **Update README** — Link from `docs/inference/README.md` and `docs/opencode/README.md` to the runbook and recipe.
5. **Update AGENTS.md** — Add the one-config recipe and checklist so agents can reproduce.
6. **Promise-register** — Confirm the runbook recipe matches `inference.khala_free_openai_compatible_api.v1` (proposed) safeCopy before public copy.
7. **Public copy** — Only after all above pass. The `/khala` page and any social copy reference the runbook.
8. **Upstream PR** (optional, later) — Add `openagents` as a built-in OpenCode provider.

### Summary of Open Items

| Item | Resolution |
|---|---|
| Model key `openagents/khala` → doubled selector | Resolved: use model key `khala`, `api.id: "openagents/khala"`, and selector `openagents/khala` |
| 402 handling in OpenCode | Test and document; may need an OpenCode issue if unhandled. Big Pickle has no quota gate — users hitting Khala's free ceiling will compare unfavorably |
| Big Pickle baseline | **Missing from this doc** — added. Khala must beat Big Pickle on cost-per-accepted-outcome and verified-rate. Add a GYM ladder rung comparing Khala vs Big Pickle on the OpenCode coding-agent surface |
| Runbook already exists | `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md` points to `docs/opencode/opencode-khala-recipe.md` |
| Upstream preset PR | Deferred until the one-config recipe is verified |
| Promise gate for public copy | Owner-gated; runbook is internal until green |
