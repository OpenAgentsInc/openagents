# OpenCode Token Counter Attribution Plan

> Worker 07 of 10. Source: `docs/inference/2026-06-25-khala-inference-gtm-push.md`.

## Goal

Prove that OpenCode-served inference increments the public Khala tokens-served counter, and attribute tokens by tool/client (OpenCode, qa-runner, Autopilot, etc.) without conflating internal dogfood with external adoption.

## 1. Baseline: How the Counter Works (live)

- `ServedTokensRecorder` fires synchronously after every `/v1/chat/completions` completion, recording the `usage.total_tokens` from the upstream response.
- Public endpoints: `GET /api/public/khala-tokens-served` (aggregate), `/history` (per-day buckets).
- Already verified under 24-wide concurrent stress — the recorder is monotonic and durable.
- The counter **cannot** distinguish traffic sources today — it aggregates everything.

## 2. Proving OpenCode Increments the Counter

The acceptance check is straightforward:

| Step | Action | Expected |
|---|---|---|
| 1 | Read counter: `curl https://openagents.com/api/public/khala-tokens-served` | Record `N` |
| 2 | Execute a known-completion OpenCode session against `openagents/khala` (single user message, 1 tool call) | Session completes |
| 3 | Read counter again | Record `N' >= N + tokens_used` |
| 4 | Read `/history?window=7d&bucket=day` | Today's bucket increased by `tokens_used` |

The existing OpenCode smoke checklist in the adoption runbook already includes this. The plan is:

1. **Automate the check.** Write a script that: mints a free key, reads counter, runs a single-turn OpenCode task (`opencode run --format json -m openagents/khala -p "say hello"`), reads counter again, asserts delta ≥ 0 and matches `usage.total_tokens` from the OpenCode JSON output.
2. **Run it as a pre-publication gate.** Before publishing the OpenCode recipe, this script must pass. Add it to `apps/openagents.com/workers/api/src/inference/` as `verify-opencode-counter.ts`.
3. **Run it as a recurring CI check.** Once the recipe is live, pin it as a weekly cron or CI step so counter drift is caught immediately.

## 3. Attributing Tokens by Tool/Client (Without Vanity Metrics)

### The problem

The public counter is a single aggregate. If OpenCode serves 10k tokens and qa-runner serves 90k, the counter says 100k — and it is impossible to tell how much is the ecosystem tool (OpenCode) vs internal dogfood (qa-runner). The GTM push doc is explicit: "keep [internal vs external] distinguishable in our analytics so we never imply external traction we do not have."

### The solution: per-client tagging at the gateway

Add an **optional, server-enforced `client` tag** on the inference request. This is not a user-facing parameter — it is set by our own systems and optionally by ecosystem integrations.

**Implementation sketch:**

1. **Request header** — `X-Khala-Client: opencode | qa-runner | autopilot | probe | forum | sites | external`. The gateway reads this header on the request path in `chat-completions.ts`.
2. **Telemetry record** — `KhalaTelemetryRecord` in `khala-telemetry.ts` currently has `requestClass`, `tokens`, `TTFT`, etc. but **no consumer field**. Add `consumer: string` (default `"external"` if header absent).
3. **Ledger table** — The token-usage ledger (`token-usage-ledger.ts`) already records per-request rows. Add a `consumer` column to the persisted row.
4. **Attribution endpoint** — A new internal-only endpoint `GET /api/internal/khala-tokens-by-client?window=7d&bucket=day` returns `{ consumer: string, tokens: number }[]`. This is **not** on the public counter — it is our analytics surface.
5. **Dogfood dashboard** — Query this endpoint daily. The public counter stays aggregate; the internal dashboard breaks out the split.

### Honesty rules (anti-vanity guardrails)

| Rule | Enforcement |
|---|---|
| The public counter is **always** aggregate, never broken out by source | No per-source query parameters on public endpoints |
| External attribution claims ("OpenCode served X tokens") must come from the internal attribution endpoint, **not** from subtracting dogfood from aggregate | The dogfood dashboard is the single source of truth for per-tool numbers |
| If a tool's `X-Khala-Client` header is missing/untrusted, it counts as `"external"` | Default is safe — under-attribution is honest, over-attribution is not |
| Internal dogfood traffic uses registered `oa_agent_` tokens, not free-tier mint keys | Free-tier tokens are indistinguishable from external; registered tokens carry operator metadata for the attribution header |

### Rollout sequence

1. **Add `consumer` field** to `KhalaTelemetryRecord` and the ledger persistence layer.
2. **Add `X-Khala-Client` header read** in the gateway request path.
3. **Build internal attribution endpoint** + dashboard.
4. **Tag all internal systems** (qa-runner, Autopilot, Probe) with their consumer id.
5. **Tag OpenCode recipe** — Publish the config with a note: if you set `X-Khala-Client: opencode` in your custom HTTP client, we count your tokens separately. (Not required for the recipe to work.)
6. **Verify delta discipline.** Run the automated check from §2 with the client header set. Confirm the public counter increments AND the attribution endpoint records it correctly.

## 4. What We May Claim (and How)

| Claim | Data source | Honest framing |
|---|---|---|
| "Khala served N tokens total today" | Public counter | Aggregate, includes internal + external |
| "OpenCode sessions served M tokens through Khala" | Attribution endpoint | Only if `X-Khala-Client: opencode` header was present; else say "at least" |
| "X external developers tried Khala via OpenCode" | Count distinct `oa_agent_` keys used in OpenCode sessions | Not derivable from token counts alone; needs key-level attribution |
| "Khala tokens grew W% week-over-week" | Public history endpoint | Aggregate; internal growth is real growth but should note if mostly dogfood |

The public-facing `/khala` page shows the aggregate counter. The per-tool numbers stay in our analytics. Never publish a per-tool number without also publishing the methodology (which header, what window, internal-only caveats if any).

## 5. Summary of Work Items

| # | Item | Owner |
|---|---|---|
| 1 | Write `verify-opencode-counter.ts` automated counter-delta script | Agent session |
| 2 | Add `consumer` field to `KhalaTelemetryRecord` schema | Gateway team |
| 3 | Add `X-Khala-Client` header read in chat-completions request path | Gateway team |
| 4 | Add `consumer` column to token-usage ledger persistence | Ledger team |
| 5 | Build `GET /api/internal/khala-tokens-by-client` endpoint | API team |
| 6 | Tag all internal systems with their consumer id | Each system owner |
| 7 | Add `X-Khala-Client: opencode` to the published OpenCode recipe | Docs team |
| 8 | Run acceptance script as pre-publication gate before publishing OpenCode recipe | QA |

## Status Summary

Written `docs/opencode/opencode-token-counter-attribution-plan.md`. The plan defines three layers: (1) an automated counter-delta acceptance script to prove OpenCode traffic hits the public counter; (2) per-client tagging via `X-Khala-Client` header + new `consumer` field on the telemetry and ledger records, with a strictly internal attribution endpoint; and (3) honesty guardrails that keep the public counter aggregate-only and default to `"external"` (under-attribution) when a client identity is missing. No other files edited. Not committed. Not pushed.
