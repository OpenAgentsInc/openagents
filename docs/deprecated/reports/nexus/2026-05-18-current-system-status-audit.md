# Current Nexus/Pylon Training, Payout, LDK, and Spark Status Audit

Date: 2026-05-18

Prepared from live Nexus reads, the latest pushed OpenAgents commits, the
closed LDK tracker issues, and the runbooks/reports updated during the
2026-05-17 through 2026-05-18 production-readiness work.

This audit is intentionally direct about what is proven and what is still
dirty. The current LDK accepted-work path is proven. The historical training
and payout backlog is not clean. Spark is not an active Nexus/Pylon production
rail, and the active Nexus/Pylon source plus staged Nexus deploy context are
guarded as LDK-only.

## Evidence Sources

Live reads were taken through the authenticated Nexus operator API without
recording bearer tokens, raw invoices, seeds, or private key material.

Primary live evidence:

- `GET https://nexus.openagents.com/api/stats`
  - snapshot generated at `2026-05-18T15:53:56Z`
- `GET https://nexus.openagents.com/v1/treasury/status`
  - snapshot generated at `2026-05-18T15:53:26Z`
- `POST https://nexus.openagents.com/v1/admin/treasury/operations`
  - `treasury.status`
  - `treasury.listPayments`
  - `treasury.reconcilePayments`
- `GET https://nexus.openagents.com/api/training/runs/run.cs336.a1.ldk-proof-20260518151532`

Relevant pushed commits:

| Commit | Summary |
| --- | --- |
| `18e0b5656` | Document LDK accepted-work production proof |
| `ad27f320b` | Expire stale retained training leases |
| `fabf67c0e` | Pass Psionic repo to training supervisor |
| `a0b7ee99b` | Document paid Pylon intake priority |
| `f149fd3b1` | Prioritize paid Pylon worker intake |
| `9ca92dc61` | Clear validator backlog before worker payout gating |
| `7864cc88a` | Use live LDK channel readiness |
| `ccab010f0` | Require LDK payout targets before readiness |

Relevant issue state:

- No open GitHub issues matched `LDK OR Spark OR Nexus OR Pylon OR training OR
  payout` at the time of this audit.
- The reopened LDK/Nexus/Pylon issues were closed after the production proof:
  `#4485`, `#4486`, `#4488`, `#4495`, `#4499`, `#4502`, `#4503`.
- Earlier Spark/LDK transition issues are also closed: `#4480` through
  `#4494`, `#4496` through `#4501`.

## Executive Status

### Proven Working

- Nexus is reachable publicly.
- Pylon presence is visible in public stats.
- Nexus treasury is active on the `ldk` provider and `ldk` rail.
- LDK Server is configured against Bitcoin mainnet with `bitcoind`.
- The LDK wallet is connected.
- There is one usable LDK channel.
- There is one registered LDK payout-target identity.
- A fresh production training run paid accepted work over LDK and settled.
- Pylon hosted training now uses the packaged Psionic runtime gate.
- Spark is not the active Nexus funding, payout, Pylon registration, API, chat,
  or deploy path.

### Not Clean Yet

- Training has a large historical backlog:
  - `1921` active runs
  - `1404` active windows
  - `1354` pending-validation windows
  - `2450` open validator challenges
  - `2362` queued validator challenges
- Launch health reports `overall_status: bad` because of backlog, payout
  attention, and artifact latency.
- Historical payout ledger state is still dirty:
  - `90` failed payout records
  - `86` accepted-work payout records needing attention
  - `38` failed payouts in the latest 24h stats snapshot
- Only one LDK payout-target identity is registered; `2421` identities still
  require Pylon v0.2 LDK registration.
- The repo still contains historical desktop wallet code and root Cargo lock
  entries outside the normal Nexus/Pylon deploy context. They are not copied,
  compiled, linked, or configured by the standard Nexus image.

The system is ready for the narrow claim "new accepted work can be paid through
LDK." It is not ready for the broader claim "the training/payout ledger is
fully clean at scale."

## 2026-05-18 Active Source Cleanup Update

Issue `#4505` removed the remaining active Nexus/Pylon source references to the
old payment rail from:

- `apps/nexus-control/src`
- `apps/pylon/src`
- `crates/openagents-provider-substrate/src`
- `scripts/deploy/nexus`

The active API now rejects unsupported legacy provider-target registrations with
provider-neutral wording, funding responses expose `provider_target` and
`provider_invoice` instead of legacy rail-specific field names, and treasury
migration tests classify old payout records as `retired_payout_record` rather
than preserving a live rail.

Verification for this cleanup:

```bash
rg -n 'spark|Spark|SPARK|breez-sdk-spark|spark-wallet|openagents-spark' \
  apps/nexus-control apps/pylon crates/openagents-provider-substrate \
  scripts/deploy/nexus apps/nexus-relay/deploy/Cargo.nexus.lock

bash scripts/deploy/nexus/test-ldk-deploy-invariants.sh
cargo check -p openagents-provider-substrate -p pylon -p nexus-control
```

Expected result: the `rg` command returns no active-path rows, the deploy
invariant reports the Nexus/Pylon paths are LDK-only, and the targeted Cargo
check completes.

## 2026-05-19 Public Status and Work-Materialization Guard Update

Follow-up stabilization moved the normal public treasury read path to the
redacted cached treasury snapshot. `GET /v1/treasury/status` should now return
the latest public status projection without rebuilding full internal payout
rows, payout-target rows, operation rows, or beneficiary debug rows inline.
Operator refreshes and background wallet refresh update that cache; row-level
diagnostics belong on authenticated projection/export surfaces.

The same update also changed hosted CS336 work creation to be opt-in. Standard
deploys now default both of these environment variables to `false`:

```text
NEXUS_CONTROL_CS336_HOMEWORK_AUTO_DISPATCH_ENABLED=false
NEXUS_CONTROL_CS336_HOMEWORK_LEASE_AUTO_LAUNCH_ENABLED=false
```

The first flag controls timer-driven homework dispatch. The second controls
the older worker lease-claim fallback that created hosted starter work when no
scheduled default-network run existed. Operators should use explicit admin
dispatches or named smoke runners for LDK payout proofs, then leave both
automatic paths disabled during normal production idle.

## 2026-05-18 Payout Ledger Cleanup Update

Issue `#4506` added an explicit treasury cleanup/reporting command for the
historical payout ledger:

```bash
nexus-control treasury payout-ledger-cleanup \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-dry-run.json \
  --json

nexus-control treasury payout-ledger-cleanup \
  --apply \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-apply.json \
  --json
```

The cleanup separates current LDK payout health from retained historical rows.
Failed rows with old provider-style or unknown targets are not retryable and
are reported under `retired_historical_*` counters. Failed rows with
LDK-compatible targets remain current LDK attention or retryable pending state.

The new status fields to watch are:

- `current_ldk_failed_payout_count`
- `current_ldk_attention_payout_count`
- `retired_historical_payout_count`
- `retired_historical_accepted_work_payout_count`
- `retired_historical_payout_sats`

The detailed before/after report is
`docs/reports/nexus/2026-05-18-payout-ledger-cleanup-before-after.md`.

## Current Pylon and Training Status

Live public stats reported:

| Metric | Value |
| --- | ---: |
| Pylons online now | `72` |
| Sellable Pylons online now | `72` |
| Inference-ready Pylons online now | `70` |
| Reported host clusters online now | `22` |
| Training nodes admitted | `338` |
| Training nodes online | `14` |
| Training runs active | `1921` |
| Training windows active | `1404` |
| Training windows pending validation | `1354` |
| Validator challenges open | `2450` |
| Validator challenges queued | `2362` |
| Training accepted closeouts | `375` |
| Training payout-eligible closeouts | `372` |
| Training artifact failures open | `0` |

The public training launch health reported:

```text
overall_status: bad
active_runs: 1921
run_backlog_slots: 33
pending_validation_windows: 1354
validator_challenges_open: 2450
validator_challenges_queued: 2362
accepted_work_pending_payout_count: 0
accepted_work_attention_payout_count: 86
payouts_failed_24h: 38
payouts_skipped_24h: 0
resolver_lookup_latency_p95_ms: 5623
signed_access_latency_p95_ms: 1227
active_alert_count: 5
critical_alert_count: 1
```

Active alerts:

- `run_backlog`: `33` worker slots remain unfilled across `1921` active runs.
- `validator_backlog`: `1354` pending windows, `2450` open challenges, `2362`
  queued challenges.
- `payout_lag`: `86` accepted-work payouts need attention.
- `resolver_latency`: resolver p95 was `5623 ms` over `64` samples.
- `signed_access_latency`: signed-access p95 was `1227 ms` over `64` samples.

Interpretation:

- The fresh proof run shows the new targeted path works.
- The broad training queue still carries old retained windows and old payout
  attention.
- Future proof runs should stay targeted with `training intake --run-id
  <run_id>` until the old queue is drained or retired.

## LDK Treasury and Channel Status

Live treasury status reported:

| Field | Value |
| --- | --- |
| `active_treasury_provider` | `ldk` |
| `active_treasury_rail` | `ldk` |
| `treasury_enabled` | `true` |
| `wallet_runtime_status` | `connected` |
| `payout_loop_health` | `idle` |
| `payout_loop_runtime_status` | `idle` |
| `ldk_server_configured` | `true` |
| `ldk_network` | `bitcoin` |
| `ldk_chain_backend` | `bitcoind` |
| `wallet_balance_sats` | `3843` |
| `wallet_total_onchain_balance_sats` | `1843` |
| `wallet_spendable_onchain_balance_sats` | `1843` |
| `wallet_lightning_balance_sats` | `2817` |
| `wallet_sync_lag_ms` | `476674` |
| `last_wallet_sync_at_unix_ms` | `1779119129827` (`2026-05-18T15:45:29Z`) |

LDK readiness:

| Field | Value |
| --- | ---: |
| `state` | `ready` |
| `registered_payout_target_count` | `1` |
| `projected_channel_count` | `1` |
| `projected_inbound_capacity_sats` | `2000` |
| `projected_outbound_capacity_sats` | `3843` |
| `recent_failed_payment_count_24h` | `1` |
| `recent_no_route_count_24h` | `0` |
| `recent_insufficient_balance_count_24h` | `0` |

Admin `treasury.status` reported:

```text
provider: ldk
node_id: 0306a9fdbcafe756b6fc3a84b7f98d4e6d1832e165e2649a1ad73bfd1c4079c0ab
network: bitcoin
chain_backend: bitcoind
current_best_block_height: 949961
balances.total_onchain_sats: 1843
balances.spendable_onchain_sats: 1843
balances.lightning_sats: 2817
balances.usable_sats: 3843
channels_count: 1
```

The single channel was:

```text
channel_id: 38736559810786082177012781675491527640
peer_node_id: 022374ea32063d5a7ffab2b237b40fdf25ba73dc885998c868002bf1551d62f010
status: usable
outbound_capacity_sats: 2000
inbound_capacity_sats: 0
```

Interpretation:

- LDK is the active provider and rail.
- The node is connected and has usable channel capacity for small proof-scale
  payouts.
- Capacity is not production-scale. It is enough to prove the path, not enough
  to support broad Pylon rollout.
- The channel projection fields are currently sufficient for readiness, but the
  direction labels need continued scrutiny. The runbook already notes that the
  readiness projection is sourced from live provider channel data and represents
  usable payout capacity toward registered Pylons despite older field names.

## Payout Status

Live payout metrics reported:

| Metric | Value |
| --- | ---: |
| `payouts_confirmed_24h` | `1` |
| `payouts_failed_24h` | `38` |
| `payouts_skipped_24h` | `0` |
| `payout_sats_paid_24h` | `25` |
| `payout_sats_paid_total` | `1624628` |
| `accepted_work_payout_sats_paid_24h` | `25` |
| `accepted_work_payout_sats_paid_total` | `86668` |
| `weak_device_accepted_work_payout_sats_paid_24h` | `25` |
| `weak_device_accepted_work_payout_sats_paid_total` | `3144` |
| `tracked_payment_backlog_count` | `9` |
| `backlog_total` | `90` |
| `backlog_retryable` | `0` |

Failure reasons in the last 24h:

| Reason | Count | Total sats |
| --- | ---: | ---: |
| `retired_unpayable_non_ldk_payout_record` | `9` | `171` |
| `treasury_provider_error:ldk:invalid_request:unsupported_ldk_payment_target_kind:unknown` | `29` | `1245` |

Ledger summary:

```text
reconciliation_status: attention_required
payout_record_count: 375
pending_payout_count: 0
confirmed_payout_count: 285
failed_payout_count: 90
skipped_payout_count: 0
attention_payout_count: 90
missing_payout_target_count: 0
accepted_work_pending_payout_count: 0
accepted_work_confirmed_payout_count: 285
accepted_work_attention_payout_count: 86
```

Interpretation:

- New LDK accepted-work payout is proven and settled.
- There are no pending accepted-work payouts in the current summary.
- The ledger still has failed historical records and failed unsupported-target
  attempts. Those should not be represented as new LDK payment failure for the
  proof run, but they do keep the broader payout ledger in attention state.

## Production Accepted-Work Proof

The proof run is:

```text
run: run.cs336.a1.ldk-proof-20260518151532
window: window.cs336.a1.ldk-proof-20260518151532.0001
worker: pylon-gcp-1
validator: pylon-gcp-3
contribution_id: cf7c70416d7265f948fa78ee1e2f94b7bf03ef5975449e8eb89d244816b300d0
```

Payout record from live run detail:

```text
payout_key: accepted_work:accepted.training_window.window.cs336.a1.ldk-proof-20260518151532.0001:cf7c70416d7265f948fa78ee1e2f94b7bf03ef5975449e8eb89d244816b300d0:98bf27e2ea7b89451ce27323a7b4570b72141a7c13d0eb740672f6a3994e7bb4
amount_sats: 25
status: confirmed
reconciliation_status: settled
payout_class: accepted_work
payout_basis: aggregation_weight
work_class: small_model_local_training
progress_class: model_update
accepted_outcome_id: accepted.training_window.window.cs336.a1.ldk-proof-20260518151532.0001
share_bps: 10000
weight_basis: local_steps
weight_value: 4
weak_device_bearing: true
progress_bearing: true
created_at_unix_ms: 1779117606215
updated_at_unix_ms: 1779117702000
```

The proof report is
`docs/reports/nexus/2026-05-18-ldk-accepted-work-production-proof.md`.

## Psionic Runtime and Hosted Pylon Packaging

The important bug fixed during this work was not a Lightning payment bug. It
was a hosted runtime packaging bug.

Bad state:

- `/usr/local/bin/pylon` was updated.
- `/var/lib/pylon/psionic` remained stale at Psionic revision
  `09b71872b24a934228f61c28e65e3aa544025f54`.
- Validators failed with:

```text
failed to resolve machine runtime identity:
failed to resolve psionic repo root: No such file or directory
```

Corrected state:

- Hosted Pylon binary commit: `ad27f320b`
- Pylon binary SHA-256:
  `e839dd7857f2e8f7ddaaabb32a17c7415c3d5b773107282047b381cb0f6e0e16`
- Packaged Psionic runtime revision: `55e4b66f`
- Psionic runtime archive SHA-256:
  `2444877f67ed8f1d396b6a999dcb21272d99d2735bd7f65eda465e72f517108f`
- Hosted `psionic-train` SHA-256:
  `76c60acaf0dc9837c5679d92e9b404339d59a6cbd47b9bc5c9d2c19a60d29b67`

Runbook rule:

- Updating the Pylon binary alone is not sufficient.
- Any hosted training deploy must verify both `/usr/local/bin/pylon` and
  `/var/lib/pylon/psionic`.
- A payout proof does not count until worker and validator use the packaged
  runtime expected by the release.

## Spark Status

Spark is not an active Nexus/Pylon payment rail.

Current active-state facts:

- `NEXUS_TREASURY_PROVIDER=spark` is not valid in the active provider parser.
- Nexus standard funding target creation goes through LDK.
- Nexus accepted-work payout dispatch goes through LDK.
- Pylon v0.2 registration is LDK-compatible; Spark-only targets are ineligible
  for new paid work.
- Normal Nexus deploy staging omits Spark packages.
- No Spark drain was performed and no Spark drain mode should be reintroduced.

Current source-state facts from verification:

- Searches for active drain/provider selectors in `apps/nexus-control/src`,
  `crates`, and `scripts/deploy/nexus` returned no active
  `Spark.*Drain`, `spark_.*drain`, `NEXUS_SPARK_.*DRAIN`,
  `NEXUS_TREASURY_PROVIDER=spark`, or `provider=spark` hits.
- A staged Nexus build context was created with
  `scripts/deploy/nexus/stage-build-context.sh`; searching that staged context
  for `openagents-spark`, `breez-sdk-spark`, `spark-wallet`,
  `name = "spark"`, and `breez/spark-sdk` returned no rows.
- The repository still contains `crates/spark` and Spark entries in the root
  `Cargo.lock`. Those are historical/deprecated repository artifacts and are
  not in the staged Nexus production build context.

Historical Spark timing/failure facts from the transition audit:

- Spark funding-target calls timed out at `10` seconds, `20` seconds,
  `180` seconds, and even `600` seconds across different reports.
- The 2026-05-15 incident was made worse by a public recovery proxy that still
  had a stale `12s` upstream timeout while the VM-local relay could complete
  the same operation.
- The recovery proxy timeout was moved into the same budget class as the relay
  and Nexus-control wallet timeouts, but this is a protective ceiling, not an
  acceptable product latency target.
- Spark wallet history could return empty payments even when balances changed.
- Spark leaf scans could leave nominal funds unusable because leaves were
  `SplitLocked` or `TransferLocked`.
- Relay logs previously showed `Failed to select leaves:
  TreeServiceError(InsufficientFunds)` even when nominal balance existed.

Conclusion:

- Spark should remain historical audit data only.
- Do not add Spark recovery, fallback, drain, payout, registration, or invoice
  code back into the normal Nexus/Pylon path.
- If old Spark records must be exported, export from existing provider-neutral
  records; do not rebuild production Nexus with Spark SDKs.

## Funding and Invoice Status

Current operator funding path:

- `POST /v1/treasury/funding-target`
- provider: LDK
- expected material: BOLT11 invoice
- confirmation proof: spendable balance change, provider payment lookup, or
  accepted-work payout settlement

Important rule:

- Invoice creation is not payment proof.
- A funding-target HTTP success is not payment proof.
- A cached balance movement without provider confirmation is not enough.
- Payment proof requires LDK provider state or downstream settled payout state.

The funding runbook now treats the `180000 ms` relay budget as a protective
ceiling. It is not an acceptable UI or operator latency target.

## Deployment and Release State

Current release process for Nexus/Pylon payment work:

- Nexus and Pylon payment work is LDK-only.
- The normal Nexus image must not copy, compile, or link Spark packages.
- Use the scripted registry-image path, not VM-local ad hoc Docker images.
- Verify public Nexus reachability after deploy.
- Verify `/v1/treasury/status` and any task-specific payout/receipt proof.

Important recent deployment/runtime facts:

- The production LDK proof used OpenAgents/Pylon commit `ad27f320b`.
- The proof documentation was committed and pushed as `18e0b5656`.
- The temporary Psionic runtime build worktree used to package
  `55e4b66f` was removed after the proof.
- 2026-05-19 deploy access check: the current local human `gcloud` account
  requires interactive reauthentication, while the active
  `nexus-mainnet@openagentsgemini.iam.gserviceaccount.com` account is a runtime
  identity, not a deploy identity. It can mint access tokens, but it cannot
  enable project services, submit Cloud Build jobs through the
  `openagentsgemini_cloudbuild` bucket, list Cloud Build jobs, read project IAM,
  or use IAP SSH against `nexus-mainnet-1`. Until the human deploy account is
  reauthenticated or a dedicated deploy service account has image-lane or
  binary-lane permissions, production Nexus code deployment is blocked.
- Old untracked JSON deploy/build receipts remain in
  `docs/reports/nexus/`. They were not staged as part of the proof closeout
  because they are noisy historical artifacts, not the current audit.

## Files and Docs Touched

Docs updated or added during this closeout:

- `docs/2026-05-15-ldk-nexus-treasury-transition-audit.md`
- `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md`
- `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`
- `docs/reports/nexus/2026-05-16-ldk-operator-docs-closeout.md`
- `docs/reports/nexus/2026-05-18-ldk-accepted-work-production-proof.md`
- This audit:
  `docs/reports/nexus/2026-05-18-current-system-status-audit.md`

Recent code areas touched by the commits in scope:

- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/treasury.rs`
- `apps/pylon/src/lib.rs`
- `scripts/deploy/nexus/29-install-pylon-psionic-runtime.sh`
- `scripts/release/pylon-binary-release.sh`

## Current Risks

### 1. Historical Training Backlog

The broad training queue is still large. The targeted proof is valid, but it
does not mean the retained queue is healthy.

Operational response:

- Use targeted proof runs for payment validation.
- Drain, retire, or explicitly quarantine old retained windows.
- Keep validator backlog from blocking fresh paid proof windows.

### 2. Historical Payout Attention

The accepted-work LDK proof is settled, but the ledger still has many failed
historical records.

Operational response:

- Separate new LDK proof status from historical payout-ledger cleanup.
- Add a report that classifies each failed payout bucket as retired,
  unsupported target, retryable, or operator-retained.
- Do not retry old unsupported non-LDK targets through LDK.

### 3. Thin LDK Capacity

The 2026-05-19 hosted Pylon rollout moved the GCP proof fleet from one
registered LDK payout-target identity to seven registered LDK payout-target
identities. Each hosted Pylon now has a unique BOLT12 payout destination
configured through `scripts/deploy/nexus/30-register-hosted-pylon-ldk-targets.sh`.

That improves admission coverage, but it does not solve Lightning liquidity.
The active LDK readiness state still requires channels/rebalancing before this
can support broad paid-work volume.

Latest 2026-05-19 check:

| Field | Value |
| --- | ---: |
| active treasury provider | `ldk` |
| active treasury rail | `ldk` |
| LDK readiness | `needs_channels` |
| projected channel count | `1` |
| minimum ready channel count | `2` |
| projected outbound capacity | `2,000 sats` |
| minimum ready outbound capacity | `20,000 sats` |
| wallet balance | `3,843 sats` |

The guarded production readiness smoke still exits before funding-target or
write operations. The recurring accepted-work proof smoke writes a failed
receipt with `reason=ldk_readiness_not_ready` and does not launch fresh work.
This is the correct behavior while the active node is below the channel and
outbound-capacity floor.

Operational response:

- Keep hosted Pylon v0.2 target registration at seven or higher.
- Add channel/liquidity capacity before broader paid-work rollout.
- Keep watch on no-route and insufficient-balance counters.

### 4. Spark Code Still Exists Outside Production Context

`crates/spark` and Spark dependencies still exist in the repository and
`Cargo.lock`.

Operational response:

- Keep deploy-context guards in place.
- Remove or quarantine deprecated Spark crate/code when no old desktop or
  receipt-inspection path needs it.
- Do not interpret root `Cargo.lock` hits as active Nexus deployment hits.
  The staged Nexus context is the relevant deployment truth.

### 5. Funding Target Latency

The current LDK funding path is better than Spark, but the operator runbook
still has long protective timeouts.

Operational response:

- Move slow provider operations behind durable async operation rows.
- Keep interactive endpoints bounded.
- Track p50/p95 invoice creation latency separately from public proxy errors.

## Recommended Next Work

1. Add or run a ledger cleanup report for the `90` failed payout records and
   `86` accepted-work attention records.
2. Drain or retire the retained training backlog so normal launch health can
   move out of `bad`.
3. Expand LDK channel capacity beyond proof scale.
4. Keep a recurring proof smoke:
   - fresh targeted training run;
   - worker claim;
   - validator closeout;
   - rewarded window;
   - confirmed and settled LDK payout.
5. Remove dormant Spark repo artifacts once old receipt inspection and
   deprecated desktop surfaces no longer require them.
6. Keep Cloudflare/web surfaces as read-only or admin facades only. LDK node,
   `bitcoind`, custody, payment events, and backups stay on server
   infrastructure.

## Bottom Line

The current system has crossed the critical LDK production proof line:

- LDK is active.
- The wallet is connected.
- The channel is usable.
- A fresh Pylon training run paid accepted work through LDK and settled.
- Spark is not an active payment rail.

The system is still carrying historical training and payout debt:

- large retained-run and validator backlog;
- old failed payout records;
- old non-LDK or unsupported payout targets;
- dormant Spark source artifacts outside the normal deploy context.

Treat the LDK path as proven for new accepted work. Treat the historical
training/payout ledger as a separate cleanup and scale-readiness project.
