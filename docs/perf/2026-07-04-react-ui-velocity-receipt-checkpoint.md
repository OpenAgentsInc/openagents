# React-Era UI Velocity Receipt Checkpoint

Date: 2026-07-04
Issue: [#8351](https://github.com/OpenAgentsInc/openagents/issues/8351)
Epic: [#8339](https://github.com/OpenAgentsInc/openagents/issues/8339)
Baseline: [`2026-07-04-foldkit-ui-velocity-baseline.md`](./2026-07-04-foldkit-ui-velocity-baseline.md)
Metric contract: [`docs/fable/2026-07-03-bf-7-2-locked-business-factory-metrics.md`](../fable/2026-07-03-bf-7-2-locked-business-factory-metrics.md)

This is the TS-10b checkpoint, not the final React-era velocity comparison.
The issue requires the TS-10a method to be rerun after TS-2 and the first
TS-7 phase have merged with at least 30 days of React-era UI PRs. That window
does not exist yet on 2026-07-04.

## Eligibility

The later dependency anchor is TS-7 phase 1:

- TS-2: `388a03340a3498aeb75773aaeef3487048fc9027`, merged
  `2026-07-04T21:09:20Z`
- TS-7 phase 1: `011e374ee06aed3d54a5e2a531842dfe51c89a11`, merged
  `2026-07-04T21:36:04Z`

The first honest 30-day React-era row can be measured at or after
`2026-08-03T21:36:04Z`. A full 30-day plus 60-day table using the same
two-window shape as TS-10a is not fully React-era until
`2026-09-02T21:36:04Z`.

## Surface Definition

The file-surface roles remain the TS-10a roles: OpenAgents web app surface
plus Khala Code desktop surface. For the React era they map to:

- `apps/openagents.com/apps/start/`
- `clients/khala-code-desktop/`

This intentionally excludes `clients/khala-mobile/` and shared token/package
work. Those are important ONE-UI work, but TS-10a did not count mobile or shared
package edits; adding them here would cherry-pick a wider React-era sample.

## Reusable Method

The TS-10a Markdown snippet is now extracted as:

```sh
bun run perf:ui-velocity -- \
  --ref <react-era-cutoff-ref> \
  --cutoff <react-era-cutoff-iso> \
  --paths apps/openagents.com/apps/start,clients/khala-code-desktop \
  --era-start 2026-07-04T21:36:04Z \
  --require-era-days 30 \
  --window-days 30
```

For the full TS-10a-shaped two-window table after the 60-day window matures:

```sh
bun run perf:ui-velocity -- \
  --ref <react-era-cutoff-ref> \
  --cutoff <react-era-cutoff-iso> \
  --paths apps/openagents.com/apps/start,clients/khala-code-desktop \
  --era-start 2026-07-04T21:36:04Z \
  --require-era-days 60 \
  --window-days 30,60
```

The extracted command was checked against the TS-10a Foldkit baseline and
reproduced the published headline values: 131 UI PRs, 60 web-path PRs, 72
desktop-path PRs, 586 direct/no-PR UI commits, median cycle time 12.4 minutes,
average 43.5 minutes, and P75 39.0 minutes.

## Current Receipt

Command:

```sh
bun run perf:ui-velocity -- \
  --ref 2f15f14899bd351e14628bf54fa82a4241a1d25a \
  --cutoff 2026-07-04T22:49:12Z \
  --paths apps/openagents.com/apps/start,clients/khala-code-desktop \
  --era-start 2026-07-04T21:36:04Z \
  --require-era-days 30 \
  --window-days 30,60
```

Output:

```json
{
  "schema": "openagents.ui_velocity_receipt.v1",
  "measurementState": "not_eligible",
  "repo": "OpenAgentsInc/openagents",
  "ref": "2f15f14899bd351e14628bf54fa82a4241a1d25a",
  "cutoff": "2026-07-04T22:49:12.000Z",
  "pathFilters": [
    "apps/openagents.com/apps/start/",
    "clients/khala-code-desktop/"
  ],
  "windows": [],
  "eligibility": {
    "eraStart": "2026-07-04T21:36:04.000Z",
    "requiredAgeDays": 30,
    "actualAgeDays": 0.1,
    "earliestEligibleCutoff": "2026-08-03T21:36:04.000Z",
    "reason": "The React-era comparison must not run until the required trailing window is wholly after the React-era dependency anchor."
  }
}
```

## Review Minutes

The TS-10a caveat still applies. BF-7.2 review minutes are ledgered
`business_factory.review_minutes.v1` values from accepted-outcome economics
rows, not GitHub review latency or elapsed wall time. If the ledger join to UI
PR refs is still absent at the eligible cutoff, TS-10b must keep review minutes
`not_measured` rather than substituting a proxy.

## Status

#8351 remains open. The reproducible command and checkpoint are merged so the
future comparison can be run without method drift, but the actual acceptance
criteria cannot be truthfully satisfied until the React-era window matures.
