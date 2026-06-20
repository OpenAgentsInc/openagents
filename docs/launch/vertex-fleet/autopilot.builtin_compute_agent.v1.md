# autopilot.builtin_compute_agent.v1 — metered token-ceiling path

Promise: `autopilot.builtin_compute_agent.v1` (state: **yellow** — unchanged).

## Blocker advanced

`blocker.product_promises.openagents_compute_metering_live_smoke_missing`
(partial — still listed; see "What remains").

## What was missing

The built-in hosted-compute (Gemini) grant broker
(`apps/openagents.com/workers/api/src/builtin-compute-agent-grant.ts`)
advertised a `dailyTokenCeiling` in every grant but **only enforced the daily
session count**. Nothing measured or bounded actual token consumption, so the
"metered/bounded compute path" the promise's green gate requires did not exist
in code — a grant kept issuing even after a user burned through the day's token
ceiling, as long as free sessions remained.

## What was built

- `evaluateBuiltinComputeAgentTokenBudget(...)` — a pure, fail-closed decision
  over already-observed token usage vs. the daily ceiling. Negative counters
  clamp to 0; a non-finite counter is treated as fully spent so a malformed
  meter can never silently widen access to the shared, owner-funded key.
- `BuiltinComputeAgentStore.sumTokensSince(...)` + its D1 implementation, which
  sums `total_tokens` from `token_usage_events` scoped to the built-in-compute
  producer/source-route only (a user's other keyed Gemini usage is never
  counted against this free tier).
- `executeBuiltinComputeAgentGrant(...)` now denies with
  `reason: 'tokens'` when the metered ceiling is spent (even with free sessions
  left) and reports `tokensRemaining` on issued grants.
- The grant route (`provider-account-service-routes.ts`) surfaces `reason`
  and `tokensRemaining` in its 429 `builtin_agent_quota_exhausted` response.
- Tests in `builtin-compute-agent-grant.test.ts` cover the token-ceiling
  denial, partial-budget grant, exact-ceiling boundary, and malformed-counter
  clamping.

No secrets, keys, or raw prompts are read, returned, or logged. No promise
state was changed.

## What remains (blocker NOT cleared)

- A **live from-install metered smoke**: a signed/notarized recut going online
  and recording non-zero metered `token_usage_events` against this ceiling,
  with public evidence. This change provides the enforcement primitive the
  smoke would exercise; it does not perform the smoke.
- Wiring the same budget gate into the inference (`generateContent`) path so
  per-request token spend is rejected once the ceiling is hit, distinguishing a
  built-in-compute actor from a key-bearing Gemini user.
- The `builtin_compute_agent_signed_recut_missing` and
  `builtin_compute_agent_live_from_install_smoke_missing` blockers are
  untouched by this change.
