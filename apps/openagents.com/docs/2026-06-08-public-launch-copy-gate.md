# Public Launch Copy Gate

Issue: [#569](https://github.com/OpenAgentsInc/openagents/issues/569)

Public launch copy is now machine-checkable before it can claim tomorrow-launch
promises as live.

## Covered Surfaces

The gate accepts copy surfaces for:

- `AGENTS.md`;
- manifests;
- OpenAPI descriptions;
- Forum seed copy;
- Artanis public summaries;
- launch announcements;
- pages, templates, and dashboards.

## Unsafe Phrase Areas

The denylist covers affirmative claims for:

- broad Pylon earning;
- full GEPA network live;
- Qwen 3.6 remote fine-tuning live;
- provider capacity marketplace live;
- referral sats streams;
- hosted MDK direct payouts;
- creator spendable settlement;
- unbounded Artanis autonomy.

Prohibition language such as "do not claim X" is allowed. Affirmative launch
copy still fails unless the matching evidence gate is green and the surface
carries a matching evidence ref.

## Gate Inputs

Each checked surface supplies:

- `surfaceRef`;
- `kind`;
- `text`;
- `evidenceRefs`.

Each evidence gate supplies:

- `gateRef`;
- `state`;
- `unsafeCopyAllowed`;
- `blockerRefs`.

Stale health blocks green state even if an evidence gate is otherwise ready.

## Verification

Regression coverage lives in
`workers/api/src/public-launch-copy-gate.test.ts`. The test suite checks
synthetic overclaims and scans current `docs/live`, capability manifest,
OpenAPI, Forum seed copy, and Artanis summary fixtures.
