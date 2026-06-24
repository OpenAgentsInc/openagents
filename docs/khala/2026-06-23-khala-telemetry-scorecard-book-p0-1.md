# Khala request-telemetry scorecard — closing book P0-1 / Open Questions #1–2

*2026-06-23. This note ties the production Khala request-telemetry schema to the
inference-engineering book's P0-1 ("make the Khala scorecard production-complete")
and resolves Open Questions #1 (canonical schema) and #2 (block-vs-receipt split).*

## What the book asked for (P0-1)

The book's core lesson is **measure the request lifecycle before optimizing it**.
P0-1 lists the fields the receipt/manifest must preserve: prompt/completion/total
tokens; cached input tokens where the provider exposes them; TTFT; inter-token
latency or perceived TPS; total wall-clock; the provider / gateway-overhead /
verifier / settlement time split where available; queue and batch wait; request
class (interactive stream | async job | verifier run | batch); route, provider,
served model, region, cache-affinity key hash, and fallback reason; verification
class + executed verdict + scalar reward; and cost basis, price, margin bucket,
settlement state, and blocker refs.

## What shipped

A typed, public-safe Effect Schema —
`apps/openagents.com/workers/api/src/inference/khala-telemetry.ts`
(`openagents.khala.telemetry.v1`) — covering every P0-1 field, plus the gateway
emission of what we can measure NOW into the `openagents` response block and the
dereferenceable inference receipt.

### Open Question #1 — the canonical schema

`KhalaTelemetryRecord` is the canonical full lifecycle record; `KhalaTelemetryBlock`
is the small immediate summary projected from it. Both are public-safe by
construction: no prompt, completion, chain-of-thought, raw account/session key,
amount, destination, or payment material — only token counts, durations, neutral
classifiers, public refs, the cache-affinity key **hash** (one-way FNV-1a, never
the raw key), and a coarse margin **bucket** (never the raw margin).

### Open Question #2 — block-vs-receipt split (RESOLVED)

- **Immediate `openagents` block (`telemetry`)** — small: request class,
  prompt/completion/total tokens, TTFT, total wall-clock, verification class +
  executed verdict + scalar reward, and a `detailRef` pointer.
- **Dereferenceable receipt detail (`KhalaTelemetryRecord`)** — the depth: the
  provider/gateway/verifier/settlement time split, inter-token latency / perceived
  TPS, queue and batch wait, region, cache-affinity hash, fallback reason, cached
  input tokens, cost basis / price / economics state / margin bucket / settlement
  state / blocker refs. Reached via `/api/public/inference/receipts/<ref>`.

The immediate block stays small; the receipt carries the depth.

## Honest measured-vs-`not_measured` discipline

Every numeric is either a real measured number or the explicit `not_measured`
sentinel. A field is never fabricated and never a misleading `0`. `not_measured`
("no measurement exists") and a measured `0` are different products — the same
discipline as the M8 metric table
(`2026-06-23-khala-head-to-head-m8-status.md`).

Measured NOW: tokens (provider `usage`, receipt-first); total wall-clock (gateway
edge); TTFT + inter-token latency + perceived TPS on the **true-streaming** path
(first content delta and EOF are observable there); request class; route /
provider / served model; verification class + executed verdict + scalar reward
(reusing the existing `khala-code` verifier verdict — no parallel grader).

Honestly `not_measured` today: cached input tokens unless the provider reports
them; the provider/gateway/verifier/settlement time split where the route has no
per-stage timer yet; queue / batch wait on the chat path; region, cache-affinity
hash, and fallback reason when the route has not reported them; cost basis,
price, and margin bucket when the economics inputs are not present on the
receipt. The builder now adds public-safe blocker refs automatically for these
absences (`provider_time_not_measured`, `gateway_overhead_not_measured`,
`verifier_time_not_measured`, `settlement_time_not_measured`,
`region_not_measured`, `fallback_reason_not_reported`,
`economics_not_measured`) so a reader can distinguish "not wired" from "zero."

Economics also has a typed state separate from settlement:

| `economicsState` | Meaning |
| --- | --- |
| `measured` | cost basis and price are measured values |
| `simulated` | economics came from staging/fixture/simulation evidence and must not be treated as live money |
| `pending` | accepted-outcome economics are waiting on the gated settlement path |
| `not_measured` | no public-safe economics measurement exists for this receipt |

## How it feeds M8 + the coordinator reward

This closes the M8 "tokens / cost / verification telemetry are `not_measured`"
gap by making the request lifecycle a typed, dereferenceable artifact. M8
manifests can read measured tokens + latency + verdict instead of treating them
as afterthoughts, and the learned coordinator's reward inputs (accepted outcome
per sat and per second) read from a stable schema rather than ad-hoc fields.

## References

- Schema: `apps/openagents.com/workers/api/src/inference/khala-telemetry.ts`
- Gateway emission: `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
  (the `openagents` block + `khalaReceiptForResult`)
- Spec section: [`khala.md`](khala.md) → "Request-telemetry scorecard"
- M8 status: [`2026-06-23-khala-head-to-head-m8-status.md`](2026-06-23-khala-head-to-head-m8-status.md)
- Tests: `khala-telemetry.test.ts` (schema/builder) +
  `chat-completions-routes.test.ts` ("telemetry scorecard" describe — measured
  fields, honest sentinels, and receipt dereference)
