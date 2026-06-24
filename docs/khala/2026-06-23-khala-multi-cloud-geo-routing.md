# Khala Multi-Cloud And Geo-Aware Routing

Issue: OpenAgents #6093  
Status: first slice complete; broader geo-residency enforcement remains staged.

Khala already routes across multiple supply families: Vertex, Fireworks,
partner passthrough, and the future OpenAgents/Pylon serving fabric. The
inference-engineering book's multi-cloud guidance applies here as a control
plane rule, not as a new live provider rollout: the control plane should publish
placement and health signals, while workload-plane gateways continue serving
from their last known local plan if that control plane is degraded.

## First Slice

The gateway now has a typed routing-metadata path beside the existing overflow
dispatcher:

- `dispatchWithOverflowWithMetadata` preserves the existing retryable-overflow
  behavior and returns the served adapter, the primary adapter, the first
  fallback reason, and optional lane signals.
- The old `dispatchWithOverflow` remains as a value-only compatibility wrapper.
- `routingSignals` is an injected, inert-by-default oracle. When it is absent,
  routing behavior is unchanged and receipts record honest `not_measured`
  sentinels instead of fabricated health or region values.
- When a signal is available, Khala receipts carry:
  - `routing.provider_health_score` in `[0, 1]`;
  - `routing.region`;
  - `routing.fallback_reason`, or `null` when the primary lane served.
- The canonical `openagents.khala.telemetry.v1` record also has
  `providerHealthScore` alongside the existing `region` and `fallbackReason`
  fields, with `provider_health_score_not_measured` as the honest blocker when
  no measured score exists.

This is intentionally not a live active-active traffic migration. It makes the
receipt surface dereferenceable and typed so a later control-plane snapshot can
be wired without changing the public receipt contract.

## Active-Active Failover Contract

For compatible chat-completions lanes, failover is active-active at the gateway
contract level:

1. The planner resolves an ordered candidate list for the requested model.
2. The gateway skips lanes without registered adapters.
3. A retryable lane failure, such as rate limit, overload, upstream 5xx, or
   transport fault, overflows to the next viable lane after bounded backoff.
4. A non-retryable failure surfaces immediately and does not double-dispatch.
5. The served lane is what metering, public receipt attribution, and routing
   metadata record.

The fallback reason is derived from the typed provider error (`kind` first,
then HTTP status, then a neutral retryable provider error). No raw upstream
error body, prompt, key, or customer identifier is exposed.

## Region And Data-Residency Controls

The first slice records region when a lane or control-plane snapshot discloses
it. Account-level residency enforcement is staged behind the same typed shape:

- account policy resolves an allowed region set or residency class;
- the planner filters or demotes lanes that cannot satisfy that policy;
- the cache-affinity pin policy refuses a warm lane when privacy or region
  rules forbid reuse;
- the receipt records the served region and a fallback reason when a compliant
  fallback lane served.

Until that account policy is wired, the gateway must not claim residency
compliance. It reports `region: "not_measured"` and keeps
`region_not_measured` in the telemetry blockers when the lane did not expose a
region.

## Degraded Control Plane Scenario

The workload plane must continue serving when the central control plane is
impaired:

- The current gateway has a local static model plan and local adapter registry,
  so it can keep serving compatible requests without a live placement service.
- The optional `routingSignals` oracle is additive. If a health/region snapshot
  is unavailable, dispatch still uses the locally resolved plan and receipts
  fall back to sentinels.
- Cache-aware routing is also inert without its oracle; a missing warm-lane
  view leaves the cheapest viable plan unchanged.
- A future global capacity view should be distributed as a last-known-good
  snapshot to workload planes. Snapshot absence or staleness must degrade
  receipt metadata quality, not request availability.

## Not Yet Done

- No live global capacity service was added.
- No account-level residency policy UI/API was armed.
- No production traffic, deploy, spend, Pylon registration, or provider secret
  changed.
- No claim is made that every provider discloses a region or health score today.
