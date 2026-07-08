# Token And Cost Budgeting Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #14 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should track context size, model output, user-requested
token budgets, provider usage, cost estimates, rate-limit state, and
public-safe usage reporting.

## Target

Build a budgeting system that separates three different questions:

- How full is the model context window?
- How much work has this run spent against a user or assignment budget?
- What did the provider usage likely cost?

Those are related, but not the same. Keeping them separate prevents bad
decisions such as compacting based on output-only usage, treating cache reads
as fresh spend, or reporting cost without knowing the model price tier.

## User-Visible Capability

The user should be able to see:

- Context remaining.
- Current turn or run usage.
- Requested token budget progress.
- Estimated cost when pricing is known.
- Unknown-cost warnings when pricing is missing.
- Rate-limit or quota blockers.
- Whether a long run stopped because it reached a budget, context boundary, or
  external limit.

Usage display should be helpful without exposing provider payloads or private
prompts.

## Core Design

Define a `UsageBudgetService` that ingests model, tool, task, and adapter
usage events and projects context, budget, and cost views.

Suggested service boundary:

```ts
interface UsageBudgetService {
  record(event: UsageEvent): Effect.Effect<void, UsageError>
  snapshot(scope: UsageScope): Effect.Effect<UsageSnapshot, UsageError>
  estimate(request: UsageEstimateRequest): Effect.Effect<UsageEstimate, UsageError>
  shouldStop(request: BudgetDecisionRequest): Effect.Effect<BudgetDecision, UsageError>
}
```

The model gateway should emit provider usage. The context assembler should emit
context estimates. The task supervisor should emit task and external-adapter
usage. The budget service should combine them.

## Usage Dimensions

Track dimensions independently:

- Input tokens.
- Output tokens.
- Cache write tokens.
- Cache read tokens.
- Tool-call usage.
- Server-side tool requests.
- Estimated context-window tokens.
- Final context-window tokens from the last provider response.
- Model-specific max output reservation.
- Wall-clock time.
- Process runtime.
- External-adapter spend.
- Payment or credit state.
- User-requested token target.

Context-window estimates are for fitting prompts. Cost estimates are for
billing. User budgets are for run control.

## Budget Types

Supported budgets:

- Per-turn max model output.
- Per-run token target.
- Per-task token target.
- Per-assignment total budget.
- Per-provider retry budget.
- Per-tool output budget.
- Context-window threshold.
- Cost ceiling.
- Wall-clock timeout.

Each budget should declare whether crossing it stops immediately, pauses for
approval, compacts, asks the user, or continues with a warning.

## Event Shape

Usage events should include:

- `usage.provider_response_recorded`
- `usage.context_estimated`
- `usage.cost_estimated`
- `usage.budget_created`
- `usage.budget_progressed`
- `usage.budget_threshold_crossed`
- `usage.max_output_escalated`
- `usage.rate_limit_observed`
- `usage.quota_blocked`
- `usage.unknown_pricing_observed`
- `usage.snapshot_projected`

Every event should include run id, optional turn/task/tool refs, model target,
provider ref, sequence, generatedAt, and visibility.

## Cost Model

Pricing should be model and provider specific:

- Input token price.
- Output token price.
- Cache-write token price.
- Cache-read token price.
- Server-side tool request price.
- Fast-path or premium-mode multiplier.
- Unknown-pricing fallback and warning.

Unknown pricing should not silently become zero. Use a conservative fallback
for local estimates and record that pricing was unknown.

## Context Measurement

Use a canonical context-size estimator:

- Prefer usage from the last provider response when available.
- Add estimated tokens for messages appended after that response.
- Include cache tokens when measuring context-window fullness if the provider
  reports them as occupying the prompt.
- Handle streamed or split assistant records from a single response as one
  provider response.
- Use rough estimates only when exact provider usage is unavailable.

Never use output-only token count for context-limit thresholds.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for usage and budget state.
- `Schema` for usage events, pricing records, and budget decisions.
- `Ref` for active run counters.
- `Stream` for live usage updates to the UI.
- `Layer` for pricing tables, provider usage parsers, and test fixtures.
- `Schedule` for rate-limit retry and budget reminders.

Usage events should be append-only. Snapshots should be projections.

## Safety Rules

- Do not expose private prompt text in usage reports.
- Do not claim exact cost when the model price is unknown.
- Do not auto-escalate max output indefinitely.
- Do not retry past a retry budget.
- Do not continue a budgeted run past the stop threshold unless the policy says
  to ask or continue.
- Do not count synthetic messages as real provider usage.
- Do not mix context-window headroom with monetary spend.
- Public receipts should report usage summaries and refs, not raw provider
  payloads.

## Tests

Minimum regression coverage:

- Parse a user-requested token budget.
- Track provider input, output, cache-read, and cache-write tokens.
- Estimate context size after new tool results are appended.
- Trigger warning, compaction, and stop thresholds independently.
- Calculate cost with known pricing.
- Mark unknown pricing and use fallback estimate.
- Avoid counting synthetic messages as provider usage.
- Stop a run at a user-requested token target.
- Retry a rate-limit fixture within retry budget.
- Produce a public-safe usage projection.

## OpenAgents Translation Notes

When promoted, map budgets to OpenAgents assignment budgets, usage ledgers,
payment refs, closeout receipts, and operator projections. Verify current issue
state before claiming any budgeting path is live.

## Decision

Token counting, context headroom, and cost estimation should be separate typed
projections over one usage event stream. The runtime should make budget stops
explicit rather than hiding them inside model prose.
