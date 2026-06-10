# Artanis Nexus/Pylon Admin Adapters

Date: 2026-06-06

Issue: #408 / `ARTANIS-022`

Status: implemented as a schema/projection contract, fake dispatch adapter,
and D1 receipt persistence path.

## Purpose

Artanis needs a safe boundary for administering Nexus/Pylon work. The first
implementation lets Artanis summarize OpenAgents product surface public Pylon fleet state, record
approval-gated dispatch proposals, call the intended Nexus/Pylon route through
a fake test adapter, and persist the resulting public receipt.

This does not enable live Pylon job dispatch. The live adapter remains blocked
until the production launch gate and target-specific authority checks exist.

## Adapter Surfaces

The v1 boundary covers these surfaces:

- stats
- provider inventory
- Pylon readiness
- job offers
- job assignments
- run status
- artifacts
- acceptance
- payout and settlement caveats

Public projections use OpenAgents product surface public Pylon stats and public-safe refs only.
Nexus/Pylon receipt refs remain separate evidence for dispatch, payout, and
settlement paths.
Operator projections can carry private evidence refs by reference inside
`/autopilot`, but the same contract rejects raw provider, runner, wallet,
payment, customer, private repo, secret, raw log, raw artifact, raw dataset,
and raw timestamp material.

## Dispatch Boundary

Dispatch records carry:

- proposal refs
- marketplace job refs
- job kind
- resource mode
- estimated cost refs
- spend-limit refs
- acceptance criteria refs
- approval gate refs
- authority receipt refs
- provider eligibility refs
- intended Nexus and Pylon route refs
- run status refs
- public receipt refs

Approved records require an effective `pylon_job_dispatch` approval gate,
authority receipt refs, and provider eligibility refs. The only executable path
in this issue is the fake adapter path. It records the route refs that would be
called and returns a public receipt/run-status ref, then
`saveArtanisNexusPylonAdapterDispatch` persists the dispatch record through
the Artanis D1 persistence layer with `executableAuthority: false`.

Live dispatch remains false:

- no live Pylon job dispatch
- no provider mutation
- no wallet spend
- no payment spend
- no settlement mutation
- no training launch
- no deployment
- no runtime promotion

## Public Projection

Public `/artanis` and Forum projections can show:

- fleet state
- online, wallet-ready, and assignment-ready Pylon counts
- training contributor counts
- public Pylon refs
- adapter surface names
- dispatch proposal refs
- public cost, spend-limit, acceptance, caveat, blocker, receipt, and run
  status refs
- friendly display times

They do not show:

- operator detail refs
- private evidence refs
- authority receipt refs
- idempotency keys
- raw timestamps
- raw provider/runner/customer/wallet/payment/log/artifact material

## Verification

Coverage lives in `workers/api/src/artanis-nexus-pylon-adapters.test.ts`.

The tests prove:

- public-safe fleet monitoring from OpenAgents product surface public Pylon stats;
- unavailable stats become public blockers;
- adapter surface coverage includes stats, provider inventory, readiness,
  offers, assignments, run status, artifacts, acceptance, and payout/settlement
  caveats;
- approval-gated fake dispatch calls the intended Nexus and Pylon route refs;
- fake dispatch receipts persist through D1 and retry idempotently;
- live dispatch, missing approval/eligibility, expired approval, and unsafe
  private/provider/wallet/payment/raw refs fail closed.
