# OpenCode Token Counter Attribution Plan

> Worker 07 of 10. Source: `docs/inference/2026-06-25-khala-inference-gtm-push.md`.

## Goal

Prove that OpenCode-served inference increments the public Khala tokens-served counter, and attribute tokens by tool/client (OpenCode, qa-runner, Autopilot, etc.) without conflating internal dogfood with external adoption.

## 1. Baseline: How the Counter Works (live)

- `ServedTokensRecorder` fires synchronously after every `/v1/chat/completions` completion, recording the `usage.total_tokens` from the upstream response.
- Public endpoints: `GET /api/public/khala-tokens-served` (aggregate), `/history` (per-day buckets).
- Already verified under 24-wide concurrent stress — the recorder is monotonic and durable.
- The public counter intentionally aggregates everything. The gateway also accepts
  optional safe attribution headers and stores them in
  `token_usage_events.safe_metadata_json`; owner-gated per-tool rollups are F1
  (#6252), not public counter dimensions.

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

The gateway accepts optional public-safe attribution tags on the inference
request. These are set by first-party systems and by recipes where the client can
send custom headers.

**Implementation sketch:**

1. **Request headers** — the route reads:
   - `x-openagents-demand-kind: external | internal | unlabeled`
   - `x-openagents-demand-source: ecosystem | dogfood | ...`
   - `x-openagents-client: opencode | aider | qa-runner | ...`
2. **Ledger metadata** — `ServedTokensRecorder` writes these values into
   `safeMetadata` as `demandKind`, `demandSource`, and `demandClient`.
3. **Attribution endpoint** — F1 (#6252) owns the owner-gated aggregate
   analytics split over these safe metadata fields. This is **not** on the
   public counter.
4. **Dogfood dashboard** — Query the F1 analytics split daily. The public
   counter stays aggregate; the internal dashboard breaks out the split.

### Honesty rules (anti-vanity guardrails)

| Rule | Enforcement |
|---|---|
| The public counter is **always** aggregate, never broken out by source | No per-source query parameters on public endpoints |
| External attribution claims ("OpenCode served X tokens") must come from the internal attribution endpoint, **not** from subtracting dogfood from aggregate | The dogfood dashboard is the single source of truth for per-tool numbers |
| If a tool's attribution header is missing/untrusted, it is treated as unlabeled aggregate traffic | Default is safe — under-attribution is honest, over-attribution is not |
| Internal dogfood traffic uses registered `oa_agent_` tokens, not free-tier mint keys | Free-tier tokens are indistinguishable from external; registered tokens carry operator metadata for the attribution header |

### Rollout sequence

1. **Use the existing request headers** in first-party clients that can set them.
2. **Build internal attribution endpoint** + dashboard in F1 (#6252).
3. **Tag all internal systems** (qa-runner, Autopilot, Probe) with their demand
   kind/source/client.
4. **Tag ecosystem recipes when possible** — AI SDK and LangChain JS can set
   custom headers; Aider, Cline, and Continue should use fresh per-tool keys
   until F1 can roll up request metadata.
5. **Verify delta discipline.** Run the automated check from §2 with headers set
   where supported. Confirm the public counter increments and, once F1 exists,
   the attribution endpoint records the split.

## 4. What We May Claim (and How)

| Claim | Data source | Honest framing |
|---|---|---|
| "Khala served N tokens total today" | Public counter | Aggregate, includes internal + external |
| "OpenCode sessions served M tokens through Khala" | Attribution endpoint or fresh-key test window | Only if headers/key scope bind the traffic to OpenCode; else say "at least" |
| "X external developers tried Khala via OpenCode" | Count distinct `oa_agent_` keys used in OpenCode sessions | Not derivable from token counts alone; needs key-level attribution |
| "Khala tokens grew W% week-over-week" | Public history endpoint | Aggregate; internal growth is real growth but should note if mostly dogfood |

The public-facing `/khala` page shows the aggregate counter. The per-tool numbers stay in our analytics. Never publish a per-tool number without also publishing the methodology (which header, what window, internal-only caveats if any).

## 5. Summary of Work Items

| # | Item | Owner |
|---|---|---|
| 1 | Write `verify-opencode-counter.ts` automated counter-delta script | Agent session |
| 2 | Use `x-openagents-demand-kind`, `x-openagents-demand-source`, and `x-openagents-client` in clients that support headers | Each integration owner |
| 3 | Build owner-gated per-tool rollups over safe metadata | F1 / API team |
| 4 | Tag all internal systems with demand kind/source/client | Each system owner |
| 5 | Use fresh per-tool keys for tools that cannot set headers | Docs / QA |
| 6 | Run acceptance script as pre-publication gate before publishing recipes | QA |

## Status Summary

Current status: the served-token recorder already stores safe request attribution
metadata when callers send `x-openagents-demand-kind`,
`x-openagents-demand-source`, and `x-openagents-client`. The remaining F1 work is
the owner-gated analytics split over those fields. The public counter remains
aggregate-only.
