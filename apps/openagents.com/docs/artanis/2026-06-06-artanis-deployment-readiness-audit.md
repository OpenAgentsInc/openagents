# Artanis Deployment Readiness Audit

Date: 2026-06-06, Central time audit window

Status: current deployment-readiness audit for Artanis, Pylon v0.2, and the
production end-to-end path.

Companion full audit:
`docs/artanis/2026-06-06-artanis-full-deployment-readiness-audit.md`.

## Executive Verdict

Artanis is not ready to deploy as a continuously running production
administrator.

The OpenAgents product surface repository has shipped most of the typed Artanis substrate:
standalone runtime records, autonomous-loop records, operator steering,
approval gates, public report projection, health/staleness monitoring, Forum
taxonomy, publication queue, listener contracts, Nexus/Pylon adapter contracts,
Pylon marketplace intake, continual-learning templates, Pylon launch
communications, and a production launch gate.

That is not the same thing as a deployed, proven Artanis.

The live production stack still fails the minimum deployment-readiness bar:

- Pylon v0.2 has not shipped as a public release.
- The full Artanis production flow has not been tested end to end.
- Production D1 does not currently have the `artanis_*` persistence tables.
- The current deployed public Artanis report does not include the newest
  `pylonLaunchCommunication`, `productionLaunchGate`, `pylonReleaseParity`,
  or `forumRewardSmoke` fields that exist in `origin/main`.
- The public Artanis status topic exists, but has only a single retained post
  at audit time.
- The scheduled runner must remain disabled until migrations, deploy parity,
  a production-equivalent launch smoke, and rollback drills are retained.

Safe public wording today:

```text
Artanis has an evidence-backed public status surface and an operator-gated
implementation path for Pylon/Nexus/Model Lab administration. The production
autonomous runner is not ready to be enabled, Pylon v0.2 is not publicly
released, and full production end-to-end flow evidence is still missing.
```

Do not say:

```text
Artanis is continuously running autonomously.
Pylon v0.2 is shipped.
Pylon v0.2 is ready for everyone.
Artanis can administer Pylon/Nexus work without operator gates.
Accepted work is paid or settled unless a public receipt chain proves it.
```

## Sources And Commands Reviewed

OpenAgents product surface source and docs:

- `docs/artanis/2026-06-06-artanis-implementation-audit.md`
- `docs/artanis/2026-06-06-autonomous-loop-contract.md`
- `docs/artanis/2026-06-06-end-to-end-launch-smoke.md`
- `docs/artanis/2026-06-06-scheduled-tick-runner.md`
- `docs/artanis/2026-06-06-production-launch-gate-runbook.md`
- `docs/artanis/2026-06-06-pylon-v02-launch-readiness.md`
- `docs/artanis/2026-06-06-pylon-v02-launch-communications.md`
- `docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md`
- `workers/api/src/artanis-public-report.ts`
- `workers/api/src/artanis-production-launch-gate.ts`
- `workers/api/src/artanis-scheduled-runner.ts`
- `workers/api/src/artanis-launch-smoke.ts`
- `workers/api/src/artanis-nexus-pylon-adapters.ts`
- `workers/api/src/operator-pylon-marketplace-routes.ts`
- `workers/api/migrations/0119_artanis_persistence.sql`
- `workers/api/migrations/0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `workers/api/migrations/0121_pylon_marketplace_jobs.sql`

OpenAgents/Pylon source and release evidence:

- local `openagents` clone at commit
  `9b7cf51c20d7b749550c203f80a243ea27f66b59`
- `openagents/Cargo.toml`
- `openagents/crates/openagents-provider-substrate/src/payout_target.rs`
- `openagents/apps/pylon/src/lib.rs`
- `openagents/packages/pylon-bootstrap/package.json`
- `openagents/docs/pylon/PYLON_PLAN.md`
- `openagents/docs/nexus-treasury.md`
- `gh -R OpenAgentsInc/openagents release list --limit 30`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.1.23`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.2.0`

Live production checks:

- `curl -fsS https://openagents.com/api/public/artanis/report`
- `curl -fsS https://openagents.com/api/public/pylon-stats`
- `curl -fsS https://openagents.com/api/forum/topics/88888888-4001-4001-8001-888888888888`
- `bunx wrangler d1 migrations list openagents-autopilot --remote --config workers/api/wrangler.jsonc`
- read-only D1 table existence query for `artanis_%` tables.

GitHub issue state:

- #413, Pylon launch communications, closed and merged.
- #414, production launch gate and runbook, closed and merged.
- #415, comparative economics evidence packets, implemented as a read-only
  packet/projection contract.
- #416, production readiness verifier, implemented as a read-only
  verifier/projection contract and JSON command.
- #417, retained production-equivalent launch smoke evidence, implemented as
  a read-only retained-smoke contract and launch-gate check adapter.
- #418, Forum delivery/listener verification evidence, implemented as a typed
  verification/projection contract for canonical topic, delivery, idempotency,
  listener, triage, blocker, and authority-boundary evidence.
- #419, Pylon v0.2 release-parity evidence, implemented as a public-report
  projection that distinguishes source-level support from release tag/assets,
  package version, runtime/platform smokes, eligibility, accepted-work,
  paid-work, and settlement evidence.

## Direct Answers

### Have We Shipped Pylon v0.2?

No.

The source-level v0.2 payout-target contract exists, but the public release has
not shipped.

Observed facts:

- `openagents/Cargo.toml` still reports workspace package version `0.1.23`.
- GitHub Releases shows `pylon-v0.1.23` as the latest Pylon release.
- `gh -R OpenAgentsInc/openagents release view pylon-v0.2.0` returns release
  not found.
- `pylon-v0.1.23` was published on 2026-05-15 and contains only:
  - `pylon-v0.1.23-darwin-arm64.tar.gz`
  - `pylon-v0.1.23-darwin-arm64.tar.gz.sha256`
- At audit time, the live Pylon stats endpoint reported online Pylons with
  recent client versions such as `pylon/0.1.1`, `pylon/0.1.12`, and
  `pylon/0.1.23`, not a v0.2 release line.

What does exist:

- `PYLON_PAYMENT_TARGET_VERSION_V0_2 =
  "pylon-payment-target/v0.2"` in provider substrate source.
- `LDK_PAYMENT_TARGET_CAPABILITY_V0_2 =
  "ldk_payment_target_v0_2"` in provider substrate source.
- Pylon source paths that register payment target metadata for BOLT12/BOLT11
  style targets and other LDK-compatible payout-target forms.
- Docs and tests around the v0.2 target model.

That supports this public claim:

```text
Pylon has source-level support for the v0.2 LDK-compatible payout-target
contract.
```

It does not support this public claim:

```text
Pylon v0.2 is shipped.
```

### Have We Tested The Full Flow?

No.

The contract-level flow has tests. The deployed production flow has not been
proven end to end.

Tested locally or by contract:

- #397 models the chain from operator steering to loop claim, safe result,
  delivered Forum post, and `/artanis` summary.
- #404 tests the scheduled runner disabled state, enabled local tick behavior,
  duplicate retry collapse, and false authority for risky actions.
- #405 tests private operator console access.
- #406 tests publication queue delivery semantics.
- #407 tests read-only Forum listener and triage drafting.
- #408 tests Nexus/Pylon monitoring and fake-dispatch receipts.
- #410 tests Pylon marketplace intake and triage.
- #411 tests continual-learning templates.
- #412 tests Forum reward visibility as simulation/evidence only.
- #413 tests Pylon launch communication safety.
- #414 tests the production launch gate, blocked claims, and runbook commands.

Not tested in production:

- production D1 migrations for Artanis persistence;
- production scheduled runner with `ARTANIS_SCHEDULED_RUNNER_ENABLED=true`;
- one live scheduled tick writing retained Artanis runtime, loop, tick, health,
  approval, work-routing, and Forum-intent rows;
- live Forum delivery from a scheduled tick into the Artanis status topic;
- live Forum listener ingestion back into the next Artanis tick;
- live `/autopilot` operator inspection of retained Artanis runner rows;
- live Nexus/Pylon dispatch beyond fake/receipt-only boundaries;
- live Pylon marketplace assignment to an eligible Pylon;
- live accepted-work payout;
- live bitcoin reward spend with named wallet authority and spend cap;
- production rollback for a bad Artanis post, bad public claim, duplicate tick,
  stuck approval gate, or accidental runner enablement.

## Current Production State

### Public Artanis Report

`GET https://openagents.com/api/public/artanis/report` is live and currently
returns a public Artanis projection with:

- `runtimeState: "running"`
- `autonomousLoop.state: "running"`
- `healthSummary.overallState: "stale"`
- public blocker refs for stale Model Lab reporting, pending operator
  approval, missing live spend authority, missing public accepted-work receipt
  chains, missing public settlement receipt chains, and retained Pylon release
  artifacts.

That public projection is useful, but it is not sufficient evidence that the
production runner is actually executing. The projection can be constructed
from example/public report code and current Pylon stats. The retained D1 state
must exist before it can prove a production loop.

The deployed report also does not match current `origin/main`. Production lacks
these current fields:

- `forumRewardSmoke`
- `pylonLaunchCommunication`
- `pylonReleaseParity`
- `productionLaunchGate`

Those fields exist in current source and tests. Their absence from the live
route means production is behind the repository state and cannot yet display
the launch gate that would prevent overclaiming.

### Production D1 Persistence

Production D1 is not ready for Artanis.

Read-only checks found no live `artanis_%` tables:

```text
SELECT name FROM sqlite_master
WHERE type = 'table' AND name LIKE 'artanis_%'
ORDER BY name;
```

returned no rows.

`wrangler d1 migrations list` reports these migrations still pending on the
remote `openagents-autopilot` database:

- `0119_artanis_persistence.sql`
- `0120_artanis_nexus_pylon_adapter_dispatches.sql`
- `0121_pylon_marketplace_jobs.sql`

This is a hard blocker. A production scheduled Artanis runner cannot retain
real runtime snapshots, loop records, loop ticks, health snapshots, approval
gates, work-routing proposals, Forum publication intents, Nexus/Pylon fake
dispatch receipts, or marketplace job rows until those migrations are applied
and verified.

### Scheduled Runner

The code path exists and the Worker cron trigger exists, but the runner is
intentionally disabled unless production resolves:

```text
ARTANIS_SCHEDULED_RUNNER_ENABLED=true
```

That flag should remain false until after:

- production D1 migrations are applied;
- the current Worker bundle is deployed and includes the launch gate fields;
- the public `/artanis` and JSON route shape are verified;
- a production-equivalent smoke proves one safe tick and duplicate retry
  collapse;
- rollback commands are tested.

### Forum Surface

The Artanis status topic is readable:

```text
https://openagents.com/forum/t/88888888-4001-4001-8001-888888888888
```

At audit time it had one post. That is not enough evidence for a running
Forum-first administrator. The next readiness proof should show:

- a scheduled or operator-triggered Artanis status intent;
- idempotent delivery by the approved Forum writer path;
- a post ref retained in D1;
- the post appearing in the Artanis status topic;
- the public `/artanis` report linking that exact post ref;
- a listener pass reading replies without posting or mutating anything
  unauthorized.

### Pylon/Nexus Public Stats

`GET https://openagents.com/api/public/pylon-stats` is live.

At audit time it reported:

- status: `live`
- 22 Pylons online now
- 23 Pylon sessions online now
- 22 sellable Pylons online now (legacy compatibility label)
- 96 training assigned contributors
- 43 training accepted contributors
- accepted-work bitcoin total visible as a historical aggregate
- accepted-work bitcoin over the last 24 hours as zero

Those stats proved there was a live public observation surface. Current
homepage/report stats are OpenAgents product surface-backed Pylon API aggregates; online,
wallet-ready, assignment-ready, and legacy sellable labels do not prove Pylon
v0.2 release readiness, Artanis dispatch authority, live marketplace
assignment, accepted-work payout, or settlement.

## Readiness Matrix

| Area | Repository state | Production state | Readiness verdict |
| --- | --- | --- | --- |
| Artanis public identity | Implemented | `/artanis` route exists | Public status surface is live, but not proof of autonomy. |
| Public report aggregator | Implemented through #392/#413/#414 | Live route is missing newest fields | Needs deploy parity before launch claims. |
| Production launch gate | Implemented in code | Not present in live JSON | Hard blocker for public autonomy wording. |
| D1 persistence | Migrations exist | Migrations pending; no `artanis_%` tables | Hard blocker for real scheduled loop. |
| Scheduled runner | Implemented, disabled by default | Not proven enabled or retained | Must remain disabled until gate passes. |
| Operator console | Implemented in code | Not verified against retained Artanis rows | Cannot be final until D1 rows exist. |
| Approval gates | Implemented in code | Not verified on production Artanis records | Needed before risky actions. |
| Forum taxonomy | Implemented | Status topic readable with one post | Needs retained #418 delivery/listener verification from a controlled run. |
| Forum delivery | Implemented plus #418 verification contract | Not proven from scheduled runner | Needs production-equivalent delivery proof retained through #417/#418. |
| Forum listener | Implemented plus #418 verification contract | Not proven in production loop | Needs read-only listener pass retained through #418. |
| Nexus/Pylon adapter | Read/fake-dispatch contracts exist | Pylon stats live | Monitoring works; live dispatch remains blocked. |
| Pylon marketplace intake | Implemented in code | Migration pending | Cannot be relied on until migration and route smoke. |
| Continual-learning templates | Implemented | Not proven as live jobs | Templates only; no training launch authority. |
| Forum rewards | Simulation/evidence contracts exist | Live wallet spend not authorized | No live tipping or reward spend claim. |
| Accepted-work payout | Historical aggregates visible | No per-work public receipt chain for Artanis | No payout or settlement claim. |
| Pylon v0.2 source contract | Present in `openagents` source | Source-only | Can say source-level support exists. |
| Pylon v0.2 release | No `pylon-v0.2.0` release | Not shipped | Cannot claim shipped or generally available. |
| Platform assets | Latest release Darwin ARM64 only | Linux/WSL/Windows not current-ready | Public setup must remain guarded. |
| Full E2E flow | Contract smoke exists | Not retained in production | Not ready to deploy autonomously. |

## Authority Boundary

The current Artanis design correctly denies risky authority by default.

Current records/projections do not grant Artanis authority to:

- spend bitcoin;
- move wallet funds;
- redeem L402/payment challenges;
- mutate provider state;
- dispatch Pylon jobs;
- launch evals;
- launch training;
- install adapters;
- deploy code;
- promote a runtime;
- charge buyers;
- pay providers;
- settle payouts;
- publish unrestricted Forum posts;
- upgrade public claims from modeled/measured to paid/settled.

This is the right boundary. The deployment problem is not that Artanis is too
powerful. The deployment problem is that production has not yet retained the
state and smoke evidence required to let the safe subset run automatically.

## Required Gates Before Deploying Artanis

Minimum gate sequence:

1. Apply and verify production D1 migrations:
   - `0119_artanis_persistence.sql`
   - `0120_artanis_nexus_pylon_adapter_dispatches.sql`
   - `0121_pylon_marketplace_jobs.sql`
2. Deploy the current Worker/web bundle so the live public report includes:
   - `forumRewardSmoke`
   - `pylonLaunchCommunication`
   - `productionLaunchGate`
3. Verify `/artanis` renders the same gate/report state as
   `/api/public/artanis/report`.
4. Keep `productionLaunchGate.state` blocked until the production-equivalent
   launch smoke is retained.
5. Run a staging or controlled production smoke with equivalent bindings:
   - operator goal or steering event;
   - one enabled scheduled tick;
   - retained runtime snapshot;
   - retained loop record;
   - retained loop tick;
   - retained health snapshot;
   - retained approval gate;
   - retained work-routing proposal;
   - retained Forum publication intent;
   - idempotent duplicate retry collapse.
6. Deliver one safe Artanis status post to the status topic through the
   approved writer path.
7. Prove the public report links the delivered post ref.
8. Run a read-only listener pass over the status topic and retain the triage
   draft without granting write, spend, or dispatch authority.
9. Verify the operator console can see and pause the retained goal/loop.
10. Reject or leave blocked every risky approval gate.
11. Verify Nexus/Pylon adapter stays read/fake-dispatch only unless a separate
    executor authority exists.
12. Verify no public projection contains private repo, provider, wallet,
    payment, customer, raw log, raw timestamp, or secret material.
13. Disable the scheduler and verify no new rows are written after disable.
14. Run rollback drills for:
    - bad public claim;
    - bad Forum post;
    - duplicate tick;
    - stuck approval gate;
    - stale OpenAgents product surface public Pylon stats;
    - accidental runner enablement.

Only after those gates should an operator consider setting:

```text
ARTANIS_SCHEDULED_RUNNER_ENABLED=true
```

## Required Gates Before Saying Pylon v0.2 Shipped

Minimum Pylon release gates:

1. Publish `pylon-v0.2.0`, or explicitly decide and document that the
   v0.2 payout-target contract is shipping on a `0.1.x` release line.
2. Publish retained binary assets and checksums for the supported platform
   matrix:
   - Darwin ARM64;
   - Linux x86_64;
   - Linux ARM64;
   - native Windows x86_64 if native Windows is advertised.
3. Retain launcher smokes for each advertised path:
   - release asset selection;
   - checksum verification;
   - archive extraction;
   - `pylon --help`;
   - `pylon status --json`;
   - `pylon wallet status --json`;
   - TUI or no-launch smoke.
4. Retain WSL Ubuntu smoke before steering Windows users to WSL as smooth.
5. Retain native Windows smoke before advertising native Windows.
6. Run and retain the LDK accepted-work payout harness for the release commit.
7. Confirm Nexus production status for:
   - active LDK rail;
   - connected wallet runtime;
   - v0.2 target registration counters;
   - no failed/skipped/degraded payout state.
8. Publish an operator and public state model that keeps these states separate:
   - online;
   - eligible;
   - assigned;
   - accepted;
   - paid;
   - settled.

## Issue State And Next Work

Recently closed Artanis issues:

- #415 `ARTANIS-029: Collect Margot/Pylon comparative economics evidence
  packets`
- #416 `ARTANIS-030: Add read-only production readiness verifier`
- #417 `ARTANIS-031: Retain production-equivalent Artanis launch smoke
  evidence`
- #418 `ARTANIS-032: Record live Forum delivery and listener verification
  evidence`

#415 matters for investor-facing and outcomes-per-kWh claims and is now
implemented as a collection/projection contract in
`workers/api/src/artanis-pylon-comparative-economics.ts`. It is not the
immediate blocker for enabling the runner. The immediate blockers are deploy
parity, production migrations, production-equivalent E2E smoke, and rollback
verification.

The #415 contract does not prove live economics by itself. Public measured,
payable, or settled outcomes-per-kWh claims still require real Pylon node
telemetry, accepted-work receipts, payout/payment evidence, and settlement
receipt chains.

#416 adds `workers/api/src/artanis-production-readiness-verifier.ts` and
`scripts/artanis-production-readiness.mjs`. It gives operators a repeatable
JSON check for source, deploy parity, D1 persistence, Pylon release, smoke,
and scheduler-readiness evidence. It does not apply migrations, deploy, enable
the scheduler, post to Forum, mutate GitHub releases, dispatch Pylon work,
spend bitcoin, or upgrade public claims.

#417 adds `workers/api/src/artanis-retained-launch-smoke.ts`. It records the
operator approvals, persisted Artanis rows, Forum delivery or no-publish proof,
public report refs, rollback disable refs, and public/operator projections
needed to turn a production-equivalent smoke into a launch-gate check. It does
not enable the scheduler or grant deployment, Forum mutation, provider
mutation, Pylon dispatch, training launch, wallet spend, buyer-charge, payout,
or settlement authority.

#418 adds `workers/api/src/artanis-forum-verification.ts`. It records the
canonical Artanis status and Pylon release work-log topic refs, intended and
delivered post refs, delivery receipt refs, idempotency refs, listener
notification refs, reply-draft refs, operator-question refs, work-routing refs,
no-op/read refs, locked/hidden/archived blockers, and public/operator
projections. It does not post to Forum by itself and grants no moderation,
direct posting outside the approved bridge, payment, wallet, provider,
dispatch, scheduler, accepted-work payout, settlement, or public-claim-upgrade
authority.

Suggested next implementation order:

1. Run the #416 verifier before and after any controlled deploy window.
2. Apply and verify pending production migrations in a controlled deployment
   window.
3. Deploy current `origin/main` so live Artanis report fields match source.
4. Run the public report and `/artanis` launch-gate verification checks.
5. Run one controlled scheduled-runner smoke with equivalent production
   bindings and retain D1 rows through the #417 retained-smoke contract.
6. Deliver one safe Artanis Forum status post through the approved queue.
7. Run the read-only listener pass and retain a #418 verification record for
   no-new-post/read, reply-draft, operator-question, work-routing, or blocker
   evidence.
8. Disable the scheduler and verify no further rows are written.
9. Populate the #415 packet with measured evidence before any public measured
   economics or outcomes-per-kWh campaign.

## Final Decision

Do not deploy Artanis as an autonomous production administrator yet.

Do not claim Pylon v0.2 has shipped.

Do not claim the full flow has been tested live.

The codebase is close enough to run a controlled launch rehearsal after
production migrations and deploy parity are fixed. It is not close enough to
leave Artanis running overnight as a public Pylon/Nexus administrator.
