# 180. Observe, meter and price

> **Status: unimplemented — design draft.** Nothing here runs yet.

A service that other people depend on needs metrics, audit logs and a usage record for each request. This chapter adds all three and a pricing policy.

The pricing policy lives in an `#authority` file. Settlement stays in the Bitcoin wallet, outside Psionic.

## What this chapter covers

- **Service metrics.** Volume, input and output lengths, status counts, latency percentiles, replica count, utilization and queue depth.
- **Audit logs.** Logs record changes to configuration, builds and policies. They record no user content by default.
- **Usage records.** Each request produces a usage record bound to its execution evidence: model identity, backend, profile and token counts.
- **A pricing policy.** Prices are a Coax `#authority` file with authored checks. *Inference Engineering* treats cost as a total, not a reverse-engineered token price, and this chapter does the same.
- **Settlement outside Psionic.** The usage record goes to the Bitcoin wallet, as the voice meter already does. Psionic does not move money.

## What it builds

- Planned metric, audit and usage record schemas in `proto/`.
- A planned pricing policy file with zero-survivor mutation checks.

## Dependencies

- [Chapter 18](ch018-time-a-decision.md): per-request timing.
- [Chapter 22](ch022-run-it-live.md): private usage records from the first live deployment.
- [Chapter 26](ch026-check-the-mesh.md): signed receipts and recomputable pool totals.
- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md): the server and its execution evidence.
- The wallet that settles payments is outside Psionic.

## Done means

- Every completed request has exactly one usage record, and its token counts match the tokens the engine processed.
- A test request with marked content leaves no trace of that content in metrics or logs.
- The pricing policy produces the stated price for each fixture, and mutation testing leaves zero survivors.
- A reader can recompute service totals from the usage records.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
