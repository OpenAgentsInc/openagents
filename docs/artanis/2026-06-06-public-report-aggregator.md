# Artanis Public Report Aggregator

Date: 2026-06-06

Status: implemented in #392 / `ARTANIS-007`.
Extended by #413 / `ARTANIS-027` for Pylon v0.2 launch communication and by
#414 / `ARTANIS-028` for the production launch gate projection. Extended by
#419 / `ARTANIS-033` for Pylon v0.2 release-parity evidence.

## Purpose

`/artanis` should be the public-safe status wrapper for Artanis, not an
operator console and not a raw workroom transcript.

The public report aggregator gives that page and public APIs one compact
projection across:

- standalone Artanis runtime state;
- autonomous loop state, tick refs, artifacts, receipts, and blockers;
- Artanis health and stale/blocked public labels;
- OpenAgents product surface public Pylon stats;
- standalone Artanis autonomy claim states;
- R10 Pylon campaign claim states;
- Model Lab public report summary;
- Artanis Forum section and canonical topic refs;
- Pylon v0.2 launch communication refs and readiness-stage summaries;
- Pylon v0.2 release-parity state separating source support from release
  assets, package version, platform smokes, eligibility, accepted-work, paid,
  and settlement evidence;
- production launch-gate state, verification refs, and autonomy-claim
  blockers;
- public blockers and caveats.

## Implementation

Code lives in:

- `workers/api/src/artanis-public-report.ts`
- `workers/api/src/artanis-public-report-routes.ts`
- `apps/web/src/page/loggedOut/page/publicAgent.ts`
- `workers/api/src/artanis-pylon-v02-launch-communications.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.ts`
- `workers/api/src/artanis-production-launch-gate.ts`

The public API is:

- `GET /api/public/artanis/report`

The browser route `/artanis` now loads:

- `GET /api/public/agents/agent_artanis/current-goal`
- `GET /api/public/artanis/report`
- `GET /api/public/pylon-stats`

The report is intentionally a projection over existing public-safe contracts,
not new action authority.

## Public Boundary

The report must not expose:

- private `/autopilot` operator evidence;
- raw workroom state;
- `authGrantRef`;
- `payloadJson`;
- hidden steering;
- private evidence refs;
- provider, runner, wallet, payment, secret, customer, raw prompt, raw log, raw
  trace, or raw timestamp material.

The report may reference public docs, public Forum refs, public route refs,
OpenAgents product surface Pylon stats refs, separate Nexus/Pylon receipt refs, Model Lab public
report refs, public artifact refs, public receipt refs, and operator
route-template refs used only as launch-gate checklist entries.

## Page Behavior

The `/artanis` page now shows:

- current autonomous loop state and latest public tick state;
- health state and stale/blocked signal count;
- Model Lab readiness and complete section count;
- Pylon feed state and public network counts;
- accepted-work bitcoin totals, with sats only as denomination detail;
- public blockers;
- public receipts and artifacts;
- Artanis Forum links, including status, Pylon campaign, Model Lab, and
  resource-mode topics;
- a Pylon launch section linking the canonical Pylon release work-log topic,
  showing whether the launch brief is prepared, and summarizing readiness-stage
  and authority-boundary refs;
- Pylon release-parity blockers that keep shipped, general-availability,
  accepted-work, paid-work, and settlement claims false until the required
  release, platform, eligibility, and receipt refs exist;
- a Production gate section showing whether public continuous-autonomy claims
  are allowed, how many required gates remain blocked, and which public
  verification targets must be checked;
- standalone autonomy and R10 claim-state rows with planned, measured,
  modeled, verified, blocked, and prohibited caveats.

The page uses friendly display times instead of raw ISO timestamps for the
public goal, event, and Pylon feed labels.

## Verification

Coverage lives in:

- `workers/api/src/artanis-public-report.test.ts`
- `apps/web/src/docs-blog-route.test.ts`
- `apps/web/src/main.test.ts`

The tests cover:

- aggregation of public-safe Artanis, Pylon, R10, Model Lab, Forum, receipt,
  artifact, health, and blocker state;
- standalone autonomy claim state projection before the narrower R10 Pylon
  campaign claims;
- no private `/autopilot`, private evidence, raw timestamps, `authGrantRef`,
  `payloadJson`, or `hiddenSteering` in the projection;
- `productionLaunchGate` blocks continuous-autonomy public claims until every
  required gate is passed;
- `pylonReleaseParity` blocks shipped and general-availability public claims
  until release assets, package version, runtime smoke, platform smoke,
  eligibility, accepted-work proof, paid-work receipts, and settlement receipt
  refs are present;
- public blocked-claim projection uses safe refs rather than serializing exact
  false public phrases;
- anonymous and authenticated public API reads returning the same public-safe
  projection;
- `/artanis` loading the report command;
- `/artanis` rendering the report, Forum links, Health metric, Pylon summary,
  Pylon launch communication, production gate, and Model Lab summary.
