# Artanis Deploy Readiness Full Audit

Date: 2026-06-07

Status: current deploy-readiness audit for Artanis, Pylon v0.2, and the live
Pylon/Nexus/Forum/bitcoin proof loop.

## Executive Verdict

Artanis is not ready to deploy as a fully autonomous production administrator.

OpenAgents product surface has a substantial typed implementation path for Artanis: public report
projection, standalone runtime records, loop and tick contracts, health and
staleness projection, operator approval gates, Forum publication/listener
contracts, Nexus/Pylon adapter contracts, Pylon marketplace intake, retained
launch-smoke evidence, production launch gates, and Pylon v0.2 release-parity
projection.

That is still not the same thing as a shipped autonomous Artanis service. The
production system does not yet retain the Artanis runtime rows as the authority
source for the public report. Pylon v0.2.4 now has public GitHub and npm
release artifacts, but the full flow from Artanis task selection through Pylon
work, accepted-work proof, payment, settlement, Forum report, and rollback has
not been tested end to end. #499 therefore freezes new Pylon releases and
broad download/earning claims until the network proves the full operator,
Pylon, job, proof, bitcoin payout, receipt, multi-host smoke, and rollback
sequence.

Direct answers:

- Pylon v0.2.4 release artifacts are published on GitHub and npm.
- Current Pylon network state is `network_not_ready_for_release`.
- The full production flow has not been tested end to end.
- Artanis should not be announced as autonomously administering Pylon/Nexus
  work yet.
- The safe claim is that Artanis has an operator-gated implementation path and
  public evidence surface, with production launch still blocked.

## Safe Current Public Claim

Use this:

```text
Artanis has an evidence-backed public status surface and an operator-gated
implementation path for Pylon, Nexus, Forum, and Model Lab administration.
The autonomous production runner is not ready to be enabled. Pylon v0.2 has
source-level support for the LDK-compatible payout-target contract, and
Pylon v0.2.4 release artifacts are published. New Pylon releases and broad
download/earning claims are frozen because the full production flow has not
been proven end to end. This is not a general readiness claim.
```

Do not say:

```text
Artanis is continuously running autonomously.
Artanis is fully administering Nexus or Pylon production.
Pylon v0.2 is ready for everyone.
Accepted work is paid or settled unless a public receipt chain proves it.
```

## Evidence Reviewed

OpenAgents product surface source and docs reviewed:

- `docs/artanis/2026-06-06-artanis-implementation-audit.md`
- `docs/artanis/2026-06-06-artanis-deployment-readiness-audit.md`
- `docs/artanis/2026-06-06-artanis-full-deployment-readiness-audit.md`
- `docs/artanis/2026-06-06-production-launch-gate-runbook.md`
- `docs/artanis/2026-06-06-pylon-v02-launch-readiness.md`
- `docs/artanis/2026-06-06-pylon-v02-release-parity-evidence.md`
- `workers/api/src/artanis-public-report.ts`
- `workers/api/src/artanis-production-launch-gate.ts`
- `workers/api/src/artanis-production-readiness-verifier.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.ts`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`

OpenAgents/Pylon source and release evidence reviewed:

- local `openagents` clone at
  `9b7cf51c20d7b749550c203f80a243ea27f66b59`
- `openagents/AGENTS.md`
- `openagents/docs/MVP.md`
- `openagents/Cargo.toml`
- `openagents/crates/openagents-provider-substrate/src/payout_target.rs`
- `gh -R OpenAgentsInc/openagents release list --limit 20`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.2.0`
- `gh -R OpenAgentsInc/openagents release list --limit 10`
- `npm view @openagentsinc/pylon dist-tags version versions --json`
- `npm view @openagentsinc/pylon@0.2.4 name version dist.tarball
  dist.integrity bin --json`
- clean `npx -y @openagentsinc/pylon@latest ... --no-launch --json` smoke
- `gh -R OpenAgentsInc/openagents issue list --state open --limit 40`
- `gh -R OpenAgentsInc/openagents pr list --state open --limit 30`

Live production checks reviewed:

- `curl -fsS https://openagents.com/api/public/artanis/report`
- `curl -fsS https://openagents.com/api/public/pylon-stats`
- `curl -fsS https://openagents.com/api/forum/topics/88888888-4001-4001-8001-888888888888`
- `bunx wrangler d1 migrations list openagents-autopilot --remote --config workers/api/wrangler.jsonc`
- `bunx wrangler d1 execute openagents-autopilot --remote --config workers/api/wrangler.jsonc --command "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'artanis_%' ORDER BY name;"`

## Pylon v0.2 Release Artifact Status

Pylon v0.2.4 release artifacts are now published.

Observed evidence:

- GitHub Releases lists `pylon-v0.2.4` as the latest Pylon release.
- npm `@openagentsinc/pylon` has `latest` set to `0.2.4`.
- `npm view @openagentsinc/pylon@0.2.4 name version dist.tarball
  dist.integrity bin --json` returns the public npm tarball and integrity:
  `sha512-SXZNpqswgyaeVFrzY9P0Pn4dYy51hWjJBf9cH+z0b83pqHGx74Pp8E9Nzk8KMdH+iLpuZtdGWKAQ3SiY4Kw0bA==`.
- A clean package-resolved `npx -y @openagentsinc/pylon@latest` smoke without
  local package-directory authority reports `version: 0.2.4`,
  `tagName: pylon-v0.2.4`, `installMethod: release_asset`, and an offline
  ready runtime status when launched with `--skip-model-download`,
  `--skip-diagnostics`, and `--no-launch`.

What does exist:

- `PYLON_PAYMENT_TARGET_VERSION_V0_2 =
  "pylon-payment-target/v0.2"` exists in provider substrate source.
- `LDK_PAYMENT_TARGET_CAPABILITY_V0_2 =
  "ldk_payment_target_v0_2"` exists in provider substrate source.
- Source-level tests and docs exist around the v0.2 LDK-compatible payout
  target model.
- OpenAgents product surface has a Pylon v0.2 release-parity evidence projection that correctly
  separates source support from release, platform, eligibility, accepted-work,
  paid-work, and settlement claims.

Conclusion:

```text
Pylon v0.2.4 release artifacts are public and npm/GitHub are aligned. This
does not prove general platform readiness, autonomous Artanis readiness,
accepted-work payout readiness, or settlement readiness. The current network
state is `network_not_ready_for_release`.
```

## Full Flow Test Status

The full flow has not been tested end to end.

2026-06-07 update: #485 added the first repeatable proof-chain checker for the
Artanis/Pylon bridge. That checker can classify one assignment as complete only
when the same assignment id appears across dispatch, Pylon accepted work,
artifact/proof evidence, payment evidence, settlement evidence, public receipt
evidence, real bitcoin movement, and terminal settlement. It is evidence-only:
it cannot dispatch work, mutate Pylons, create receipts, spend bitcoin, settle
payments, or publish Pylon v0.2. #486 added the operator proof-run route around
the settlement bridge, #487 added the multi-Pylon release gate, and #488
publishes that proof state through `pylonOpenAgents product surfaceReleaseGate` in
`/api/public/artanis/report`, an OpenAgents product surface release-gate panel on `/artanis`, and
the Artanis Nexus/Pylon Forum release-work-log bridge. #491 then retained a
second distinct Pylon proof, moving the OpenAgents product surface release gate to
`ready_for_operator_release_review` while keeping release, wallet-spend,
settlement-mutation, provider-mutation, and public-claim-upgrade authority
false. #492 verified that this current state is visible in
`/api/public/artanis/report`, the rendered `/artanis` page, and the Artanis
Pylon release work-log Forum topic. #493 added the retained release-review
record and rollback plan at
`docs/nexus/2026-06-07-pylon-v02-release-review-record.md`; the decision stays
`ready_for_operator_release_review`, not general availability approval and not
autonomous Artanis approval.

Contract-level and local evidence exists for these pieces:

- Artanis public report projection.
- Standalone runtime and autonomous loop records.
- Operator approval gates and steering contracts.
- Health/staleness projection.
- Forum taxonomy, publication queue, listener contracts, and delivery/listener
  verification contracts.
- Nexus/Pylon adapter contracts and fake-dispatch receipts.
- Pylon marketplace intake and triage.
- Continual-learning templates.
- Production launch-gate modeling.
- Retained production-equivalent launch-smoke schema.
- Pylon v0.2 launch communication and release-parity projections.

Missing production proof:

- production D1 migrations applied for Artanis persistence;
- one scheduled Artanis tick through production-equivalent bindings;
- retained runtime, loop, tick, health, approval, work-routing, publication,
  adapter-dispatch, marketplace, and verification rows;
- live Forum delivery from the scheduled runner into the canonical Artanis
  status topic;
- read-only Forum listener pass feeding back into the next Artanis tick;
- `/autopilot` operator inspection of retained Artanis rows;
- public `/api/public/artanis/report` sourced from retained rows and including
  the current launch-gate, Pylon release-parity, and Pylon v0.2 OpenAgents product surface
  release-gate fields;
- Pylon v0.2 release artifact install and runtime smoke;
- Pylon marketplace assignment to an eligible v0.2 Pylon;
- accepted-work closeout proof;
- paid-work receipt;
- settlement receipt;
- rollback drill for bad posts, false public claims, duplicate ticks, stuck
  approvals, and accidental runner enablement.

## Production Artanis State

The live public report route exists:

```text
https://openagents.com/api/public/artanis/report
```

At audit time, it returned:

- `runtimeState: "running"`;
- `autonomousLoop.state: "running"`;
- `autonomousLoop.tickCount: 1`;
- `healthSummary.overallState: "stale"`;
- `productionLaunchGate: null`;
- `pylonReleaseParity: null`;
- `pylonLaunchCommunication: null`;
- `forumRewardSmoke: null`.

This is not sufficient production evidence. It shows a public projection, not
a fully deployed, retained, scheduled Artanis loop. The missing fields also
prove production is behind the current source and cannot yet display the newer
deployment blockers that are supposed to prevent overclaiming.

The canonical Artanis Forum topic exists:

```text
https://openagents.com/api/forum/topics/88888888-4001-4001-8001-888888888888
```

At audit time, it had one retained post: the seeded canonical status-thread
message from Artanis. That does not prove scheduled publication, live listener
ingestion, operator review, or loop closeout.

## Production Pylon State

The live public Pylon stats route exists:

```text
https://openagents.com/api/public/pylon-stats
```

At audit time, it reported:

- `available: true`;
- `status: "live"`;
- 22 Pylons online now;
- 24 Pylons seen in the last 24 hours;
- 23 Pylon sessions online now;
- 22 sellable Pylons online now (legacy compatibility label);
- total Nexus payout bitcoin-denominated value of 1,654,303 satoshis;
- accepted-work payout total of 29,675 satoshis;
- accepted-work payout in the last 24 hours of 0 satoshis;
- recent client versions in the v0.1 line.

These historical sellable and payout values were Nexus-era observations.
Current homepage/report stats are OpenAgents product surface-backed Pylon API aggregates; online,
wallet-ready, assignment-ready, and legacy sellable labels do not prove
accepted work, payout, or settlement.

This proves the Pylon stats surface is live and useful. It does not prove
Pylon v0.2 release readiness or the Artanis-administered accepted-work loop.

## Production D1 State

Production D1 is not ready for Artanis.

Remote migration check shows the following migrations still pending:

- `0119_artanis_persistence.sql`
- `0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `0121_pylon_marketplace_jobs.sql`

Remote read-only table check:

```sql
SELECT name
FROM sqlite_master
WHERE type = 'table' AND name LIKE 'artanis_%'
ORDER BY name;
```

returned no rows.

This is a hard launch blocker. Without those tables, Artanis cannot retain
real runtime snapshots, loop records, tick records, health snapshots, approval
gates, work-routing proposals, Forum publication intents, Nexus/Pylon adapter
dispatch receipts, or Pylon marketplace job records.

## Open OpenAgents Release Blockers

Open issues in `OpenAgentsInc/openagents` directly relevant to Pylon v0.2 and
Artanis deployment:

- #4515, Nexus control API 503 capacity exhaustion. This blocks reliable
  admission, heartbeats, run leases, and validator-challenge finalize paths.
- #4510, recurring Nexus/Pylon LDK accepted-work proof smoke. This is required
  before claiming the accepted-work payout loop is continuously tested.
- #4509, expand LDK channel liquidity beyond proof scale. This is required
  before relying on real bitcoin settlement beyond tiny proof-scale runs.
- #4504, tracker for Nexus/Pylon LDK production cleanup and Spark removal.
  This remains open until the cleanup, liquidity, and proof-smoke work is
  integrated and proven.

Open PRs in `OpenAgentsInc/openagents`:

- #4547, Unified Issue Resolution, not draft, merge state clean.
- #4546, concurrent proof fleet/run-detail fetches and route probes, draft,
  merge state clean.
- #4545, throttle proof fleet diagnostics, not draft, merge state clean.
- #4519, #4515 root-cause diagnosis, not draft, merge state clean.
- #4518, Pylon TUI fast clean exit, not draft, merge state clean.
- #4517, Pylon TUI node-panel sizing, not draft, merge state clean.
- #4516, training-coordination retry jitter, not draft, merge state clean.

The presence of clean PRs does not mean the release is ready. Those changes
still need to be reviewed, merged to `main`, tested from the integrated state,
and followed by the actual Pylon v0.2 build, release assets, live proof, and
operator runbook execution.

## Deployment Readiness Matrix

| Area | Current status | Deploy meaning |
| --- | --- | --- |
| Artanis identity and public status surface | Partially live | Good enough for a caveated public status page, not autonomous operation |
| Artanis D1 persistence | Not live | Hard blocker |
| Scheduled Artanis runner | Not proven in production | Hard blocker |
| Operator approval gates | Implemented by contract | Must be proven against retained rows before enabling autonomous ticks |
| Forum publication | Seed topic exists, scheduled publication unproven | Hard blocker for public coordination claims |
| Forum listener | Contract exists, live loop unproven | Hard blocker for closed-loop Forum coordination |
| Nexus/Pylon adapter dispatch | Contract exists, fake-dispatch evidence only | Hard blocker for real administration claims |
| Pylon marketplace jobs | Contract exists, remote tables pending | Hard blocker for live assignment |
| Pylon v0.2.4 release artifacts | Published | GitHub and npm artifacts are aligned; this does not prove full release-gate readiness |
| Pylon network release freeze | Active | #499 records `network_not_ready_for_release`; no new release/latest move or broad download/earning claim until #500-#505 close honestly |
| Multi-platform Pylon artifact smoke | Partially proven | #490 proves local macOS arm64 clean package-resolved no-launch and forwarded status smokes; Linux, WSL Ubuntu, native Windows, and reachable second-host evidence remain blockers for general availability |
| LDK liquidity | Open issue #4509 | Hard blocker for reliable real-bitcoin settlement |
| Accepted-work recurring smoke | Open issue #4510 | Hard blocker for continuous payout proof |
| Real bitcoin payment/settlement loop | Not proven for Artanis v0.2 launch | Hard blocker |
| Multi-Pylon paid-work proof | Proven for release review | #491 proves two distinct public Pylon refs with complete public-safe terminal settlement evidence; this is not autonomous runtime proof |
| Public release-gate publication | Verified | #492 verifies the public report, rendered `/artanis` page, and Artanis Forum release work-log show the current ready-for-operator-review state without granting authority |
| Release-review record and rollback plan | Ready for review | #493 records the current non-approval decision plus rollback commands for package, GitHub release, public copy, Forum, scheduler/tick, and receipt-projection mistakes |
| Rollback drills | Not proven | Hard blocker |

## Minimum Launch Sequence

Do this before enabling Artanis as a production administrator:

1. Deploy current OpenAgents product surface source so `/api/public/artanis/report` exposes
   `productionLaunchGate`, `pylonReleaseParity`, `pylonLaunchCommunication`,
   and `forumRewardSmoke`.
2. Apply and verify remote D1 migrations:
   `0119_artanis_persistence.sql`,
   `0120_artanis_nexus_pylon_adapter_dispatches.sql`, and
   `0121_pylon_marketplace_jobs.sql`.
3. Run the Artanis production readiness verifier and confirm it blocks launch
   until retained runtime evidence exists.
4. Enable one operator-controlled production-equivalent Artanis tick, not an
   unbounded schedule.
5. Verify retained D1 rows for runtime, loop, tick, health, approval,
   work-routing, Forum publication intent, dispatch, marketplace, and verifier
   evidence.
6. Verify one public Forum publication from Artanis and one read-only listener
   pass that feeds the next tick.
7. Merge the required OpenAgents/Pylon release blockers on `main` and close or
   explicitly supersede the related PRs.
8. Keep the #499 Pylon network release freeze active; do not cut a new
   package release, move npm `latest`, or publish broad download/earning copy.
9. Smoke Pylon v0.2 release artifacts on Apple Silicon macOS, Linux, WSL
   Ubuntu, and native Windows. #490 retained clean package-resolved macOS arm64
   no-launch and forwarded status smokes, but broader host coverage is still
   required.
10. Register an eligible Pylon v0.2 LDK-compatible payout target.
11. Run a local proof-runtime accepted-work flow.
12. Run a production accepted-work flow with real bitcoin funding.
13. Verify accepted-work closeout, payment receipt, wallet balance increase,
   and settlement receipt.
14. Run rollback drills for bad Forum post, false public claim, duplicate tick,
   stuck approval, and accidental runner enablement.
15. Only then unfreeze release work, announce Artanis as live, and describe
    the Pylon network as ready for paid work.

## What Can Ship Before Full Artanis

These can ship safely if copy stays precise:

- public Artanis status and blocker reporting;
- operator-only Artanis inspection;
- Pylon v0.2 source-support docs;
- Pylon v0.2 readiness checklist;
- Forum topic for Artanis coordination;
- Pylon stats display;
- release-parity gates that block overclaiming;
- manual operator-run launch-smoke records.

These cannot be claimed yet:

- autonomous Artanis production administration;
- Pylon v0.2 general availability;
- general Pylon v0.2 availability;
- automatically paid accepted-work loop;
- settlement certainty;
- continuous Forum-coordinated work administration.

## Final Assessment

Artanis is in late implementation, not deployment.

The engineering direction is sound: OpenAgents product surface now has enough typed gate, evidence,
and public-report machinery to prevent many false claims. The remaining
blockers are not cosmetic. They are the core proof requirements for the system:
retained production rows, deployed launch-gate parity, Pylon v0.2 release
artifacts, live Pylon eligibility, real accepted-work execution, real bitcoin
payment, settlement receipts, and rollback evidence.

Until those are complete, the correct deployment posture is:

```text
Keep Artanis operator-gated. Keep the public report caveated. Do not enable an
unbounded scheduled runner. Do not announce Pylon v0.2 as ready for everyone.
Do not claim the full flow works or tell people to download Pylon to earn
bitcoin until the #499 network-readiness freeze checklist passes.
```
