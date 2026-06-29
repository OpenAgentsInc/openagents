# Pylon Join-Lifecycle Ladder (issue #4848, Pluralis roadmap P0.1)

Date: 2026-06-12

Issue: `OpenAgentsInc/openagents#4848` — funnel join-lifecycle ladder:
typed Pylon contributor states.

Roadmap: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
item P0.1.

Source: the Pluralis Research six-step staged contributor join in the
agora startup sequence (read-only workspace reference lane
`projects/pluralis/repos/agora`, `docs/agora-system/startup-sequence.md`:
authorization → state download → join queue → Sync Phase 1 → Sync
Phase 2 → active, with `sync_phase` published as a routable record and
`max_allowed_stale` triggering re-entry through the same ramp).

Claim discipline: this is contract and projection work only. The ladder
describes what a device's funnel evidence supports; it makes no live
device, payment, or settlement claims. The funnel currently reports the
fleet largely dark and this ladder does not pretend otherwise.

## What shipped

- `workers/api/src/pylon-join-lifecycle.ts` — the typed state machine,
  reason-coded transitions, receipt-compatible transition events, and
  the public-safe ladder projection over capacity-funnel records.
- `workers/api/src/pylon-capacity-funnel-live-routes.ts` — the live
  public funnel route (`/api/public/pylon-capacity-funnel`) now carries
  a `joinLifecycleLadder` block with one entry per funnel record, built
  from the same audience-redacted funnel projections.
- `workers/api/src/pylon-join-lifecycle.test.ts` — legal/illegal
  transitions, reason-code requirements, the back edge, mapping table,
  and projection privacy shape.

The existing dark-capacity reason taxonomy is untouched. The ladder is
additive: the same evidence the funnel already reason-codes on the dark
side is read as a position on the bright side.

## The state machine

States: `registered`, `qualified`, `state_synced`, `warmup`, `active`,
plus the back-edge states `lagged` and `sync_reentry`.

```
registered ──qualification_gate_passed──► qualified
qualified ──durable_seal_digest_synced──► state_synced
state_synced ──warmup_started───────────► warmup
warmup ──shadow_work_verified───────────► active
active ──beyond_max_allowed_stale───────► lagged          (back edge)
lagged ──sync_reentry_started───────────► sync_reentry    (back edge)
sync_reentry ──reentry_seal_digest_synced► state_synced   (re-ramp)
```

Every legal transition carries exactly one reason code from the closed
set (prefix `join_lifecycle.public.`); a transition with a mismatched
reason code is rejected with a typed `reason_code_mismatch` error and
an edge outside the table is rejected with `illegal_transition`. Each
applied transition emits a receipt-compatible event
(`capacityRef`, `fromState`, `toState`, `reasonCode`, `receiptRef`,
`occurredAtIso`) with all timestamps passed in by the caller — the
module never reads a clock.

A lagged device is neither rejected nor trusted: it re-enters at
`state_synced` through `sync_reentry` and re-ramps through the same
`warmup` path, mirroring the Pluralis two-phase re-integration.

Per the standing projection rule, the record's public projection JSON
is rebuilt inside the transition function — projections rebuild on
state transitions, not on registration events.

### State meanings

| State | Meaning |
|---|---|
| `registered` | Present in the registry; nothing proven beyond registration. |
| `qualified` | Passed the device qualification gates (heartbeat fresh, client version, capability, wallet readiness, benchmark evidence). |
| `state_synced` | Holds the last durable seal digest / assigned state (P1.2 bootstrap-from-durable-seal rail). |
| `warmup` | Doing shadow/unmerged work that is verified but not yet merged or paid (P1.1 shadow-window ramp rail). |
| `active` | Merged, paid work classes; full participant. |
| `lagged` | Fell beyond `max_allowed_stale` (P0.2 contract field); off the ladder. |
| `sync_reentry` | Re-ramping; one re-sync away from rejoining at `state_synced`. |

Ladder ranks: `registered` 0, `qualified` 1, `state_synced` 2,
`warmup` 3, `active` 4; back-edge states are negative (`lagged` -2,
`sync_reentry` -1).

## Funnel mapping table

`joinLifecycleStateForFunnel` maps the funnel's existing taxonomy onto
ladder states. Non-dark funnel stages map directly:

| Funnel stage | Ladder state |
|---|---|
| `registered` | `registered` |
| `benchmarked` | `qualified` |
| `eligible` | `qualified` |
| `assigned` | `state_synced` |
| `running` | `warmup` |
| `artifact_producing` | `warmup` |
| `accepted` | `active` |
| `paid` | `active` |
| `settled` | `active` |

Dark rows map through their reason codes:

| Dark-capacity reason ref | Ladder state |
|---|---|
| `dark_capacity.public.never_heartbeated` | `registered` |
| `dark_capacity.public.version_incompatible` | `registered` |
| `dark_capacity.public.capability_missing` | `registered` |
| `dark_capacity.public.wallet_not_ready` | `registered` |
| `dark_capacity.public.stale_heartbeat` | `lagged` |
| `dark_capacity.public.assignment_expired` | `lagged` |
| `dark_capacity.public.closeout_missing` | `lagged` |
| `dark_capacity.public.assignment_declined` | `qualified` |
| `dark_capacity.public.no_assignments_offered` | `qualified` |
| unknown / reasonless | `registered` |

When a dark row carries multiple reasons, the row claims the weakest
supported rung. `sync_reentry` is deliberately absent from the mapping:
the funnel cannot observe re-ramping yet, so that state is reachable
only through an explicit `lagged → sync_reentry` transition, never
inferred from a funnel snapshot.

## Public-safe projection

The ladder block on the live funnel route is derived from the funnel's
own audience-redacted projections, so the ladder never sees more than
the funnel already shows that audience. Entries carry only the
synthetic public capacity ref, the state, its label, and the ladder
rank; refs are screened against the join-lifecycle privacy guard (no
device identifiers, wallet/payment material, or raw timestamps), with
the module's own closed `join_lifecycle.public.*` taxonomy allowlisted
before the substring scan (the `wallet_not_ready` lesson from the live
funnel 500 of 2026-06-11).
