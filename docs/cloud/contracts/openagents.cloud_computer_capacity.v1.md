# `openagents.cloud_computer_capacity.v1`

Status: Implemented as the admission authority for `openagents.cloud_computer.v1`.

Owning package: `packages/khala-sync-server`

## Authority boundary

The capacity broker creates every logical computer before it evaluates runtime
capacity. A conversation can own at most 30 non-destroyed logical computers.
Creating inventory never calls a provider or creates a runtime lease.

Postgres reservations are the sole admission authority. Provider and quota
adapters write timestamped observations outside the admission transaction.
Cloud resource enumeration can report drift, but it cannot allocate, delete,
or reassign a reservation.

`openagents.compute_quota_routing.v1` remains an older coding-lane routing
contract. Its documented single-slot overrun tolerance does not apply to cloud
computer admission.

The broker takes transaction-scoped global and provider-region locks, then
evaluates one canonical request in a serializable transaction. The transaction
either records one reservation or records a typed queued or refused receipt.
An exact command retry returns the stored receipt. The same command ref with a
different request digest conflicts.

## Initial policy

Normal admission uses the lowest applicable limit from:

- The configured ceiling and the lower private-preview ceiling.
- A fresh provider quota observation after reserving at least 25% headroom.
- Host or cluster allocatable capacity after incident drains.
- Global, provider, region, owner, tenant, conversation, and budget resource
  vectors.

The resource vector covers concurrency, CPU, memory, scratch, maximum duration,
rolling one-minute start rate, and cost across global, provider, region, tenant, owner, and
conversation scopes. Command identity and computer generation add the two
innermost fences. A conversation receives four active `standard`
runtimes by default. An explicitly budgeted high-fan-out command can raise that
limit to eight. A conversation receives at most two active `strong` runtimes.

Cleanup, replacement, and recovery work can consume protected headroom after
normal admission reaches its ceiling. New normal starts fail closed when the
latest provider observation is stale. They remain queued without calling the
provider.

## Fair queue and start storms

The pure scheduler uses deterministic weighted fair queuing across tenant and
conversation keys. It advances each flow by demand divided by weight and uses
stable request refs to break ties. A continuously queued positive-weight flow
therefore advances instead of being starved by one busy tenant.

Each dispatch pass admits no more than the configured start concurrency.
Deterministic bounded jitter spreads provider calls, and every reservation has
a deadline. The durable store expires deadline-bound reservations and returns
their capacity before a later dispatch pass.

## Reconciliation

Provider adapters report lease observations with provider, region, the exact
provider operation ref, logical computer ref, and generation. Reconciliation
records these typed conditions:

- `leaked`: No durable reservation owns the provider lease.
- `missing`: An active durable reservation is absent from provider evidence.
- `double_claimed`: More than one observation claims the same durable identity.
- `generation_mismatch`: Evidence names a different generation.
- `operation_mismatch`: Evidence does not name the reservation's provider
  operation.
- `quarantined`: Provider or host policy excluded the capacity.

Reconciliation records evidence and can release an expired, missing
reservation. It never adopts an unknown lease, changes the generation of a
live reservation, or reassigns another generation's lease.

## Public receipts

Admission receipts contain only stable public refs, queue or admission state,
a closed reason code, and timestamps. They do
not contain cloud resource names, quota payloads, private addresses, regions
selected by callers, credentials, or provider administration handles.

## Implementation and verification

- Migration: `packages/khala-sync-server/migrations/0137_cloud_computer_capacity.sql`
- Policy and scheduler: `packages/khala-sync-server/src/cloud-computer-capacity.ts`
- Durable store and reconciler:
  `packages/khala-sync-server/src/cloud-computer-capacity-store.ts`
- Focused unit, load, race, deadline, and reconciliation tests live beside
  those modules.

Run:

```sh
pnpm --dir packages/khala-sync-server exec vp test --run \
  packages/khala-sync-server/src/cloud-computer-capacity.test.ts \
  packages/khala-sync-server/src/cloud-computer-capacity-store.test.ts
pnpm --dir packages/khala-sync-server run typecheck
```
