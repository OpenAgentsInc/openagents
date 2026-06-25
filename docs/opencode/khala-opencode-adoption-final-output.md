Here is the planning memo for the OpenCode adoption lane.

---

## OpenCode Adoption Lane — Planning Memo

### 1. Exact User Recipe (verified against both the live API and the OpenCode provider schema)

The config is valid. OpenCode's `ConfigV2.Provider.Info` schema (at `packages/core/src/config/provider.ts` in the opencode repo) uses `api: { type: "aisdk", package: string, url?, settings? }`. The GTM doc's JSON maps directly: `npm` → `package`, `options` → `settings`. The OpenCode docs (`providers.mdx`) confirm the `@ai-sdk/openai-compatible` pattern matches any `/v1/chat/completions` endpoint.

**Finalized recipe:**

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
        "openagents/khala": {
          "name": "Khala",
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  }
}
```

Key verification:

| Property | Status |
|---|---|
| Base URL format matches provider schema | Confirmed — `baseURL` maps to `settings.baseURL` in the AISDK provider; the provider schema accepts `Record<string, unknown>` in `settings` |
| `{env:OPENAGENTS_API_KEY}` syntax | Confirmed — OpenCode docs document the `{env:VAR}` syntax |
| Model key `openagents/khala` sent upstream as model id | Confirmed — the AI SDK's `languageModel(api.id)` sends `api.id` as the `model` field in the POST body |

### 2. Upstream/Provider Preset Opportunity

The GTM doc flags the **model-key double-doubling problem**: with provider id `openagents` and model key `openagents/khala`, the TUI selector renders `openagents/openagents/khala`. This is confirmed by OpenCode's model-key resolver (`modelKey = `${providerID}/${modelID}`` where `modelID = model.api.id`).

**The fix** (which should go in the recipe, not a server-side change):

```jsonc
"models": {
  "khala": {
    "name": "Khala",
    "api": {
      "id": "openagents/khala"
    },
    "limit": { "context": 128000, "output": 65536 }
  }
}
```

By using model key `khala` and overriding `api.id` to `openagents/khala`, the TUI selector renders `openagents/khala` (clean!) while the upstream still sends `{"model": "openagents/khala"}` to our API. OpenCode's `packages/core/src/plugin/provider/opencode.ts:125` confirms `if (config.id !== undefined) model.api.id = config.id` — the per-model `api.id` override is supported.

The **provider preset** (upstream PR) is a separate, later item. We should first publish the one-config recipe, verify it works end-to-end in a live OpenCode session, and then create an upstream PR against the opencode repo's built-in provider list. That PR would add `openagents` as a hardcoded provider in `packages/opencode/src/provider/provider.ts` or similar, removing the need to hand-write the JSON.

### 3. Quota/Error UX

The free tier (200 req / 200k tokens per UTC day) is enforced by `decideFreeTierQuota` in `inference-free-tier-key.ts`. Over-quota requests fall through to the **balance gate**, which returns a **402** with structured JSON:

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

**Risk**: OpenCode may not handle a 402 gracefully in all paths (model listing vs. chat completion). Need to test and possibly document a workaround (e.g., "if you see a 402, top up or wait for your daily quota to reset").

### 4. Token-Counter Acceptance Checks

The public counter (`GET /api/public/khala-tokens-served` and `/history`) is already live, verified, and wired through `token-usage-ledger.ts`. The `ServedTokensRecorder` fires after every completion (including free-tier zero-debit ones via `withFreeTierKhala`). Acceptance checklist:

| Check | Method |
|---|---|
| Tokens increment on a free-tier request | Read counter before and after one completion |
| Tokens increment on the public history endpoint | Same pattern against `/history?window=7d&bucket=day` |
| OpenCode session tokens appear | Run a short OpenCode session against Khala, then check counter |
| Counter survives concurrent requests | Already verified under 24-wide concurrent stress (per GTM doc) |
| Idempotency on retry | Same request_id does not double-count |

### 5. Docs/Runbook Updates Needed

No `docs/inference/runbook*` files exist. The inference README covers architecture but not adoption recipes. Required additions:

| Document | What it needs |
|---|---|
| `docs/inference/2026-06-25-khala-opencode-adoption-runbook.md` (new) | Full runbook: the exact config (with the `api.id` fix above), `curl` command to mint a key, step-by-step OpenCode `/connect` flow, quota behavior, what to test, troubleshooting |
| `docs/inference/README.md` | Add a section "Ecosystem Adoption" linking to the new runbook, and a checklist row for OpenCode in the status table |
| `apps/openagents.com/AGENTS.md` or a companion doc | Add the one-config recipe and the "what to test" checklist so future agent sessions can verify OpenCode integration without re-researching |
| `/khala` page copy (public) | Once through copy gate: "Point OpenCode at Khala with this one-config recipe" — keep it short, link to the full runbook |

The runbook should also document the **model-key doubling fix** prominently, so anyone publishing the recipe gets the clean `openagents/khala` selector rather than the doubled form.

### 6. Sequence / Gating

In order of execution:

1. **Verify** — Run a live OpenCode session against the endpoint with the fixed recipe (model key `khala`, api.id `openagents/khala`). Test tool-calling, streaming, all quota states, and the counter increment.
2. **Write runbook** — `docs/inference/2026-06-25-khala-opencode-adoption-runbook.md` with the exact recipe, verbatim curl for key minting, and the test checklist.
3. **Update README** — Link from `docs/inference/README.md` to the new runbook.
4. **Update AGENTS.md** — Add the one-config recipe and checklist so agents can reproduce.
5. **Promise-register** — Confirm the runbook recipe matches `inference.khala_free_openai_compatible_api.v1` (proposed) safeCopy before public copy.
6. **Public copy** — Only after all above pass. The `/khala` page and any social copy reference the runbook.
7. **Upstream PR** (optional, later) — Add `openagents` as a built-in OpenCode provider.

### Summary of Open Items

| Item | Resolution |
|---|---|
| Model key `openagents/khala` → doubled selector | Use `api.id` override in model config (fixed recipe above) |
| 402 handling in OpenCode | Test and document; may need an OpenCode issue if unhandled |
| Upstream preset PR | Deferred until the one-config recipe is verified |
| Promise gate for public copy | Owner-gated; runbook is internal until green |
