# 181. Survive failures

> **Status: unimplemented — design draft.** Nothing here runs yet.

Hardware failure is the normal case at scale. This chapter makes the service keep answering when a device, a machine or a region fails.

It covers detection, isolation, retries and failover, plus two protections that are easy to forget: data-residency routing and protection of the weights.

## What this chapter covers

- **Health checks.** Each replica reports versioned health from [chapter 22](ch022-run-it-live.md). A failed check removes it from routing.
- **Cordoning.** A failing device or machine is cordoned: it takes no new work, and its in-flight work drains or fails over.
- **Idempotent retries.** A retried request carries the same request identifier, so a duplicate never produces two usage records.
- **Failover.** Active-active and active-passive failover between machines or regions, with the switch time measured.
- **Data residency.** Requests with a residency constraint route only to machines in the allowed region, and fail closed if none is available.
- **Isolation of tool execution.** Tools run apart from the model process, so a tool fault cannot take down a replica.
- **Weight protection.** Weight files are verified on load, and a mismatch stops serving.

## What it builds

- Planned health, cordon and failover policies in Coax.

## Dependencies

- [Chapter 22](ch022-run-it-live.md): health checks and restart supervision.
- [Chapter 177](ch177-route-queue-and-scale-replicas.md): the router and replica set.
- [Chapter 179](ch179-ship-without-downtime.md): draining.
- [Chapter 180](ch180-observe-meter-and-price.md): usage records that retries must not duplicate.
- Regions and providers are owner decisions recorded in `NEEDS_OWNER.md`.

## Done means

- Fault injection kills a device, a process and a machine. The service keeps answering, and [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) reports the error rate and recovery time for each.
- A retried request produces one answer and one usage record.
- A residency-constrained request never reaches a machine outside its region, and fails closed when none is available.
- A weight file with one changed byte stops the replica before it serves.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
