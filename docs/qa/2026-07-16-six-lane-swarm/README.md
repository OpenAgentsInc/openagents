# QA-1 six-lane swarm — 2026-07-16 current-main/production run

- Issue: [#8906](https://github.com/OpenAgentsInc/openagents/issues/8906)
- Base: `46ea6c28f04b0fc29df482227a2c8e87b16968c5`
- Run: `qa.six-lane.20260716T150054760Z`
- Window: `2026-07-16T15:00:54.760Z`–`2026-07-16T15:01:08.866Z`
- Production target: `https://openagents.com`
- Verdict: **five lanes passed. One confirmed production finding**

This first QA-1 execution combines current-main contract tests with read-only
production probes. No authenticated mutation, payment, spend, deployment, or
credential read occurred.

## Results

| Lane                | Current-main evidence                                                                            | Production evidence                                                                                    | Verdict                                |
| ------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ | -------------------------------------- |
| Web routes          | 51 files / 232 tests passed                                                                      | `/`, `/forum`, and `/promises` returned their expected HTML surfaces                                   | pass                                   |
| Desktop shell       | QA-3's `qa:swarm:desktop` gate built Desktop and compared all 16 states                          | deterministic current-main pixel oracle. No production target                                          | pass: zero drift                       |
| Mobile              | 22 files / 126 tests passed                                                                      | no public deployed mobile target is declared                                                           | pass at the current-main contract rung |
| API / OpenAPI       | 3 files / 31 tests passed                                                                        | `/api/openapi.json` returned `200`, OpenAPI `3.1.0`, and advertised `GET /api/public/product-promises` | pass                                   |
| Payments / promises | 2 files / 27 tests passed                                                                        | the advertised promise registry returned `404`                                                         | **confirmed finding**                  |
| Sync                | client: 27 files passed, 1 skipped / 195 tests passed, 3 skipped. API: 2 files / 48 tests passed | expected `405` for GET. Malformed POST failed closed with typed `protocol_version_unsupported` `400`   | pass                                   |

The machine-readable run, per-lane log hashes, probe timestamps/statuses, and
the unchanged QA-3 Desktop receipt are in [`evidence/run.json`](./evidence/run.json).
The adjacent logs are the exact stdout/stderr inputs to those hashes.

## Confirmed finding and triage

| Severity | Surface                         | Reproduction                                                     | Evidence                                                                                                                            | Disposition                                                            |
| -------- | ------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| high     | public product-promise registry | `curl -sS -i https://openagents.com/api/public/product-promises` | three pre-triage repetitions and the canonical run returned `404 {"error":"not_found"}` while live OpenAPI advertised the GET route | filed [#8912](https://github.com/OpenAgentsInc/openagents/issues/8912) |

Before filing, the result was reproduced three times, crossed against live
OpenAPI, and searched against open and closed issues with `product-promises`,
`product promises`, and `promise registry`. No active duplicate existed. No
other issue was filed.

## What did not become a finding

An initial recipe calibration accidentally forwarded focused filenames after a
package script's `--`, broadening three lanes into overlapping monorepo runs.
Their Postgres-port collisions and unrelated failures were orchestration noise,
not verified product findings. The canonical registry uses root-relative test
paths. Each corrected command was reproduced green before the dated run above.
Calibration artifacts were discarded and are not counted here.

Bounds remain explicit: Desktop is deterministic real-shell pixel proof, not a
signed installed-app session. Mobile has no declared production target. And the
Sync probe is unauthenticated/fail-closed only, not a credentialed cross-device
session.

## Repeat the swarm

From a clean repository root on Node 24 with dependencies installed:

```sh
node scripts/qa/six-lane-swarm.mjs \
  --registry docs/qa/swarm/six-lane-registry.json \
  --out runs/qa-six-lane/$(date -u +%Y-%m-%dT%H%M%SZ) \
  --base-sha $(git rev-parse HEAD)
```

The runner validates that all six lanes appear exactly once, starts them
concurrently, runs production probes beside local commands, stores one log per
lane, preserves the QA-3 receipt, and writes
`openagents.qa.six-lane-run.v1`. It never files issues automatically.

Before filing a future finding: independently rerun the smallest exact repro,
exclude infrastructure failure, search the live issue ledger, and file only a
confirmed result. Plausible but unverified observations stay labeled
`unverified` in the dated report.
