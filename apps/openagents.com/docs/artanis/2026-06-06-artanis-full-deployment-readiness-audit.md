# Artanis Full Deployment Readiness Audit

Date: 2026-06-06 America/Chicago, 2026-06-07 UTC evidence window

Status: current deployment-readiness audit for Artanis, Pylon v0.2, and the
first production flow.

## Executive Verdict

Artanis is not ready to deploy as an autonomous production administrator.

OpenAgents product surface has implemented a large amount of the typed substrate needed for
Artanis: public identity, public report aggregation, standalone runtime and
loop records, health/staleness projection, operator approval gates, Forum
taxonomy, publication queue, listener/triage contracts, D1 persistence
schemas, Nexus/Pylon adapter contracts, Pylon marketplace intake, continual
learning templates, production launch gates, and retained-smoke evidence
models.

That is still not a deployed, proven Artanis. The production deployment cannot
yet prove that Artanis is running a scheduled loop, retaining its own rows,
publishing and listening through the Forum from the scheduled runner, routing
real Pylon jobs, or administering payouts.

Direct answers:

- Pylon v0.2 has not shipped.
- The full production flow has not been tested end to end.
- Artanis should not be announced as continuously running autonomously.
- The safe public claim is that Artanis has an operator-gated implementation
  path and public evidence surface, with production launch still blocked.

## Safe Current Public Copy

Use this:

```text
Artanis has an evidence-backed public status surface and an operator-gated
implementation path for Pylon, Nexus, Forum, and Model Lab administration.
The production autonomous runner is not ready to be enabled. Pylon v0.2 source
support exists, but Pylon v0.2 has not shipped as a public release and the
full production flow has not been proven end to end.
```

Do not say:

```text
Artanis is continuously running autonomously.
Artanis is a fully autonomous production administrator.
Pylon v0.2 is shipped.
Pylon v0.2 is ready for everyone.
Accepted work is paid or settled without public receipt chains.
```

## Evidence Reviewed

Local OpenAgents product surface source and docs:

- `workers/api/src/artanis-public-report.ts`
- `workers/api/src/artanis-production-launch-gate.ts`
- `workers/api/src/artanis-production-readiness-verifier.ts`
- `workers/api/src/artanis-retained-launch-smoke.ts`
- `workers/api/src/artanis-forum-verification.ts`
- `workers/api/src/artanis-pylon-v02-readiness.ts`
- `workers/api/src/artanis-pylon-v02-launch-communications.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.ts`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`
- `docs/artanis/2026-06-06-artanis-implementation-audit.md`
- `docs/artanis/2026-06-06-artanis-deployment-readiness-audit.md`
- `docs/artanis/2026-06-06-production-launch-gate-runbook.md`
- `docs/artanis/2026-06-06-pylon-v02-launch-readiness.md`
- `docs/artanis/2026-06-06-pylon-v02-launch-communications.md`
- `docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md`

Local `openagents` source and release evidence:

- `openagents` clone at
  `9b7cf51c20d7b749550c203f80a243ea27f66b59`
- `openagents/Cargo.toml`
- `openagents/crates/openagents-provider-substrate/src/payout_target.rs`
- `openagents/apps/pylon/src/lib.rs`
- `openagents/docs/pylon/PYLON_PLAN.md`
- `openagents/docs/nexus-treasury.md`
- `gh -R OpenAgentsInc/openagents release list --limit 10`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.2.0`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.1.23`

Live production checks:

- `curl -fsS https://openagents.com/api/public/artanis/report`
- `curl -fsS https://openagents.com/api/public/pylon-stats`
- `curl -fsS https://openagents.com/api/forum/topics/88888888-4001-4001-8001-888888888888`
- `bunx wrangler d1 migrations list openagents-autopilot --remote --config workers/api/wrangler.jsonc`
- read-only D1 query for `sqlite_master` table names matching `artanis_%`

## Pylon v0.2 Shipping Status

Pylon v0.2 has not shipped.

Observed facts:

- GitHub Releases for `OpenAgentsInc/openagents` list `pylon-v0.1.23` as the
  latest Pylon release, published on 2026-05-15.
- `gh -R OpenAgentsInc/openagents release view pylon-v0.2.0` returns
  `release not found`.
- `openagents/Cargo.toml` still reports workspace package version `0.1.23`.
- The `pylon-v0.1.23` release contains only the Apple Silicon macOS archive
  and checksum:
  - `pylon-v0.1.23-darwin-arm64.tar.gz`
  - `pylon-v0.1.23-darwin-arm64.tar.gz.sha256`
- The live Pylon stats endpoint is live, but recent clients still report
  v0.1-era versions such as `pylon/0.1.1`, `pylon/0.1.12`, and
  `pylon/0.1.23`.

What does exist:

- `PYLON_PAYMENT_TARGET_VERSION_V0_2 =
  "pylon-payment-target/v0.2"` in `openagents` provider substrate source.
- `LDK_PAYMENT_TARGET_CAPABILITY_V0_2 =
  "ldk_payment_target_v0_2"` in `openagents` provider substrate source.
- Source and docs describing LDK-compatible payout target registration,
  BOLT12/BOLT11 style target support, wallet telemetry, and accepted-work
  payout eligibility.
- OpenAgents product surface Pylon readiness and launch-communication projections that correctly
  separate source support from release, platform, eligibility, accepted-work,
  paid, and settled claims.

Conclusion:

```text
Pylon has source-level support for the v0.2 LDK-compatible payout-target
contract. Pylon v0.2 has not shipped as a public release.
```

## Pylon v0.2 Release-Parity Gate

The new release-parity gate should be treated as the authoritative checklist
before anyone says Pylon v0.2 is shipped or generally available.

Required evidence:

- release tag `pylon-v0.2.0`;
- retained release assets and checksums for Apple Silicon macOS, Linux, WSL
  Ubuntu, and native Windows;
- package version evidence matching `0.2.0`;
- runtime first-boot smoke on the release artifact;
- platform smokes for Linux, Apple Silicon macOS, native Windows, and WSL
  Ubuntu;
- eligibility telemetry for upgraded Pylons;
- LDK-compatible payment target registration refs;
- accepted-work proof refs;
- paid-work receipt refs;
- settlement receipt refs.

Until those refs exist, public report projection must keep:

- `releaseReady = false`;
- `platformReady = false`;
- `eligibilityReady = false`;
- `shippedClaimAllowed = false`;
- `generalAvailabilityClaimAllowed = false`;
- `acceptedWorkClaimAllowed = false`;
- `paidClaimAllowed = false`;
- `settledClaimAllowed = false`.

## Full Flow Test Status

The full production flow has not been tested end to end.

Contract-level and local tests exist for:

- public Artanis report aggregation;
- standalone runtime projection;
- autonomous loop records;
- operator steering and approval gates;
- health/staleness projection;
- Forum taxonomy, publication queue, delivery, listener, and delivery/listener
  verification;
- Nexus/Pylon adapter contracts and fake-dispatch receipts;
- Pylon marketplace intake and triage;
- continual-learning templates;
- Pylon v0.2 readiness and launch communication;
- production launch gate and runbook commands;
- retained production-equivalent smoke evidence;
- release-parity evidence projection.

Missing live proof:

- production D1 migrations applied and verified;
- one scheduled Artanis tick using production-equivalent bindings;
- retained runtime, loop, tick, health, approval, work-routing, publication,
  dispatch, marketplace, and verification rows;
- live Forum delivery from the scheduled runner into the canonical Artanis
  status topic;
- read-only Forum listener pass feeding back into the next tick;
- `/autopilot` operator inspection of retained Artanis rows;
- public `/api/public/artanis/report` sourced from retained rows rather than
  example projections;
- production launch-gate verifier passing after deployment parity;
- Pylon v0.2 release artifact install and runtime smoke;
- Pylon marketplace assignment to an eligible Pylon;
- accepted-work closeout proof;
- paid-work receipt;
- settlement receipt;
- rollback drill for bad posts, false public claims, duplicate ticks, stuck
  approvals, and accidental runner enablement.

## Production Deployment State

### Public Artanis Report

`GET https://openagents.com/api/public/artanis/report` is live and currently
returns a public Artanis projection with runtime and health state. At audit
time it reported:

- `runtimeState: "running"`;
- `healthSummary.overallState: "stale"`;
- no live `pylonReleaseParity` field;
- no live `productionLaunchGate` field.

The absence of those current-source fields means production is behind the
repository state. Deploy parity is required before the public route can enforce
the newer launch-gate and release-parity blockers.

### Pylon Stats

`GET https://openagents.com/api/public/pylon-stats` is live and reports active
Pylon network data. At audit time it reported a live feed with:

- 20 Pylons online now;
- 24 Pylons seen in the last 24 hours;
- 21 online Pylon sessions;
- 20 sellable Pylons online now (legacy compatibility label);
- recent client versions from the v0.1 line.

This proves that the public stats surface is useful. Current homepage/report
stats are OpenAgents product surface-backed Pylon API aggregates; online, wallet-ready,
assignment-ready, and legacy sellable labels do not prove Pylon v0.2 release
readiness, accepted work, payout, or settlement.

### D1 Persistence

Remote D1 is not ready for Artanis.

Read-only query:

```sql
SELECT name
FROM sqlite_master
WHERE type = 'table' AND name LIKE 'artanis_%'
ORDER BY name;
```

returned no rows.

The remote migration list still shows pending:

- `0119_artanis_persistence.sql`
- `0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `0121_pylon_marketplace_jobs.sql`

This blocks a real scheduled production loop. Without those tables, Artanis
cannot durably retain production runtime snapshots, loop records, ticks,
health snapshots, approval gates, work-routing proposals, Forum publication
intents, Nexus/Pylon dispatch receipts, or marketplace job rows.

### Forum Status Topic

The canonical Artanis status topic is reachable:

```text
https://openagents.com/forum/t/88888888-4001-4001-8001-888888888888
```

At audit time it contained one seed/status post. That proves the topic exists.
It does not prove scheduled-runner delivery, listener ingestion, triage, or
closed-loop operation.

## Readiness Matrix

| Area | Source status | Production status | Verdict |
| --- | --- | --- | --- |
| Artanis public identity | Implemented | Live public routes exist | Usable, not proof of autonomy |
| Public report aggregator | Implemented and extended | Live route behind current source | Needs deploy parity |
| Production launch gate | Implemented in source | Not visible in live report | Needs deploy parity and live verifier |
| Release-parity gate | Implemented in source | Not visible in live report | Needs deploy parity and real refs |
| D1 Artanis persistence | Migrations exist | Pending remotely; no tables | Hard blocker |
| Scheduled runner | Implemented disabled-by-default | Not enabled/proven | Hard blocker |
| Forum delivery | Implemented by contract | Not proven from scheduled runner | Needs live retained proof |
| Forum listener | Implemented read-only by contract | Not proven in loop | Needs live retained proof |
| Operator console | Implemented by contract | Needs retained-row inspection | Blocked by D1/deploy parity |
| Nexus/Pylon adapters | Contracts and fake receipts exist | No live dispatch proof | Operator-gated only |
| Pylon marketplace intake | Implemented by contract | Needs D1 and live job run | Not production-proven |
| Continual-learning templates | Implemented | No live training job launched by Artanis | Template only |
| Forum bitcoin rewards | Simulation/projection only | No approved live spend cap | Simulation only |
| Accepted-work payouts | Modeled/projection only | No public receipt chain | Cannot claim paid |
| Settlement | Modeled/projection only | No public settlement chain | Cannot claim settled |
| Pylon v0.2 source support | Present in source | Not released | Source-only |
| Pylon v0.2 release | No release found | Not shipped | Hard blocker |

## Deployment Gate

Before enabling Artanis in production:

1. Deploy the current OpenAgents product surface source so `/api/public/artanis/report` includes
   `productionLaunchGate` and `pylonReleaseParity`.
2. Apply and verify remote D1 migrations 0119, 0120, and 0121.
3. Confirm `artanis_%` tables exist remotely.
4. Keep `ARTANIS_SCHEDULED_RUNNER_ENABLED=false`.
5. Run the read-only production readiness verifier.
6. Execute a production-equivalent retained smoke with explicit operator
   approval and equivalent bindings.
7. Verify one scheduled-runner tick writes retained rows.
8. Verify Forum delivery to the canonical Artanis status topic.
9. Verify read-only listener ingestion and triage draft generation.
10. Verify `/autopilot` operator inspection of the retained rows.
11. Verify `/api/public/artanis/report` projects from retained evidence and
    blocks false autonomy and Pylon shipped claims.
12. Run rollback drills for publication, dispatch, public-claim, and
    payment/reward mistakes.
13. Only then consider enabling the scheduled runner, still with operator
    approvals for risky actions.

Before saying Pylon v0.2 shipped:

1. Publish a real `pylon-v0.2.0` release.
2. Retain release assets and checksums for all required platforms.
3. Update package version evidence to `0.2.0`.
4. Run and retain runtime/platform smokes.
5. Retain eligibility telemetry and payment target registration refs.
6. Complete and retain accepted-work, paid-work, and settlement receipt refs.
7. Confirm the Artanis release-parity projection allows shipped/general
   availability claims.

## Immediate Priorities

1. Ship deploy parity for current OpenAgents product surface source.
2. Apply Artanis D1 migrations in production.
3. Run the read-only production readiness verifier.
4. Run a retained production-equivalent smoke.
5. Prove Forum delivery and listener feedback from the scheduled loop.
6. Keep Pylon v0.2 claims source-only until the release-parity gate passes.

## Final Answer

Artanis should not be deployed as a live autonomous administrator yet. Pylon
v0.2 has not shipped. The full production flow has not been tested end to end.

The correct next move is not a public autonomy launch. It is a controlled
deploy-parity and retained-smoke window that proves the public report,
production launch gate, release-parity gate, D1 persistence, scheduled runner,
Forum delivery/listener loop, operator inspection, and rollback procedures in
production-equivalent conditions.
