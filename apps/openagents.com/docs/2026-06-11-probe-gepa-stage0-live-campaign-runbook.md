# Probe GEPA Stage 0 Live Campaign Runbook

Date: 2026-06-11

Issue: <https://github.com/OpenAgentsInc/openagents/issues/4667>

Promise context: `pylon.compute_revenue_modes.v1`

## Boundary

Stage 0 is unpaid campaign evidence only. It must not carry payment receipts,
settlement receipts, payout claims, Terminal-Bench score claims, model-training
claims, or runtime-candidate activation claims.

The green decision is owned by
`workers/api/src/probe-gepa-stage0-no-spend-campaign.ts`. The live smoke script
only verifies public-safe retained refs against that gate.

## Preconditions

- At least two distinct real Pylon workers are active and fresh.
- Each selected Pylon advertises `cap.gepa.retained.v1`.
- Synthetic smoke refs such as `pylon.codex.live_smoke.*`,
  `pylon.codex.packaged_*_smoke.*`, loopback refs, demo refs, and fixture refs
  are excluded.
- Accepted and rejected closeouts will both be retained.
- Probe closeout import refs, Psionic import dry-run refs, and Artanis summary
  refs are public-safe and retained.

Check public fleet readiness:

```bash
cd apps/openagents.com/workers/api
bun run smoke:probe-gepa-stage0 -- --preflight
```

Limit the check to specific Pylons:

```bash
bun run smoke:probe-gepa-stage0 -- \
  --preflight \
  --pylon-ref pylon.public.stage0.real_one \
  --pylon-ref pylon.public.stage0.real_two
```

The preflight must return `state: "green"` before the live run can honestly
start.

## Live Run

1. Create one unpaid Stage 0 assignment per selected Pylon using the controlled
   operator assignment path. The assignment route must remain `unpaid_smoke`
   and no-spend.
2. On each selected real Pylon, run the local assignment worker so it polls the
   live assignment, accepts it, executes the bounded no-spend work, and retains
   artifact/proof/closeout refs.
3. Operator closeout must accept at least one submitted worker bundle and reject
   at least one submitted worker bundle with public-safe rejection refs.
4. Import the retained closeout refs into Probe and retain the
   `probeCloseoutImportRefs`.
5. Run the Psionic import in dry-run mode only and retain the
   `psionicImportDryRunRefs`.
6. Publish or retain an Artanis public summary ref for the campaign.
7. Build a bundle JSON with this shape:

```json
{
  "campaignRef": "campaign.public.probe_gepa_stage0.live_multi_pylon",
  "coordinatorImports": [],
  "probeCloseoutImportRefs": [],
  "psionicImportDryRunRefs": [],
  "artanisSummaryRefs": []
}
```

The `coordinatorImports` entries are the public-safe
`omega.pylon_gepa_metric_call_coordinator_import.v1` records produced from the
retained assignment closeouts.

Verify the retained bundle and live preflight together:

```bash
bun run smoke:probe-gepa-stage0 -- \
  --bundle /path/to/ignored/stage0-live-bundle.json \
  --preflight \
  --pylon-ref pylon.public.stage0.real_one \
  --pylon-ref pylon.public.stage0.real_two
```

Only `state: "green"` from that command is Stage 0 campaign evidence.

## Current Recheck

On 2026-06-11, `GET https://openagents.com/api/pylons` showed live
assignment-ready Pylons, but none of the currently fresh public non-synthetic
Pylons advertised `cap.gepa.retained.v1`. The GEPA-capable refs visible in the
public list were previous synthetic smoke registrations. The live Stage 0
campaign therefore remains blocked until two real GEPA-capable workers are
online and can produce retained accepted and rejected closeout bundles.
