# OpenAgents Open Issues Audit

Date: 2026-06-01

Repo: `OpenAgentsInc/openagents`

Local checkout: `main` at `75324a6ae` (`Resolve canonical X video URLs`)

## Purpose

Audit every currently open issue in `OpenAgentsInc/openagents` and give a
direct resolution path for the remaining Nexus/Pylon work. This audit assumes
the current product direction for Pylon v0.2: Nexus and Pylon production payment
work moves away from Spark wallet and uses LDK-only payment, funding, payout,
registration, release, and recovery paths.

## Current Open Issue Set

`gh issue list --repo OpenAgentsInc/openagents --state open --limit 1000`
reported four open issues:

| Issue | State | Title | Current audit decision |
| --- | --- | --- | --- |
| [#4515](https://github.com/OpenAgentsInc/openagents/issues/4515) | Open | Nexus control API 503 "capacity exhausted" blocks admission, heartbeats, leases, finalize | Still open. Worse live symptom observed: public Nexus currently returns Cloudflare `530` / `1033`. Restore reachability first, then fix and soak the authority concurrency path. |
| [#4510](https://github.com/OpenAgentsInc/openagents/issues/4510) | Open | Add recurring Nexus/Pylon LDK accepted-work proof smoke | Keep open until public Nexus is reachable and a fresh accepted-work smoke passes from current production. The command exists. The current outage prevents honest closure. |
| [#4509](https://github.com/OpenAgentsInc/openagents/issues/4509) | Open | Expand LDK channel liquidity beyond proof scale | Probably code/deployment-complete after the 2026-05-22 release audit, but must be reverified after Nexus reachability is restored. |
| [#4504](https://github.com/OpenAgentsInc/openagents/issues/4504) | Open | [TRACKER] Finish Nexus/Pylon LDK production cleanup and remove Spark | Keep open until #4515 is resolved, #4509 is reverified, and #4510 has a fresh passing receipt. |

Closed child issues under #4504 are already closed on GitHub:

| Issue | Closed | Title |
| --- | --- | --- |
| [#4505](https://github.com/OpenAgentsInc/openagents/issues/4505) | 2026-05-18 | Delete Spark from active Nexus/Pylon source and build graph |
| [#4506](https://github.com/OpenAgentsInc/openagents/issues/4506) | 2026-05-18 | Clear historical payout ledger attention and unsupported-target backlog |
| [#4507](https://github.com/OpenAgentsInc/openagents/issues/4507) | 2026-05-19 | Drain or retire retained training and validator backlog |
| [#4508](https://github.com/OpenAgentsInc/openagents/issues/4508) | 2026-05-19 | Register hosted Pylon v0.2 LDK payout targets at scale |
| [#4511](https://github.com/OpenAgentsInc/openagents/issues/4511) | 2026-05-19 | Move slow funding/provider operations behind durable async status |
| [#4512](https://github.com/OpenAgentsInc/openagents/issues/4512) | 2026-05-19 | Enforce server-side custody and Cloudflare read-only/admin facade boundary |

## Evidence Reviewed

Repo guidance:

- `AGENTS.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Issue data:

- GitHub issue list and issue bodies/comments for #4504, #4509, #4510, and #4515.

Local code and docs:

- `Cargo.toml`
- `apps/nexus-relay/src/durable.rs`
- `scripts/deploy/nexus/03-configure-and-start.sh`
- `scripts/deploy/nexus/14-activate-binary-release.sh`
- `scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh`
- `scripts/deploy/nexus/31-smoke-ldk-accepted-work-proof.sh`
- `docs/nexus-treasury.md`
- `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md`
- `docs/deploy/NEXUS_GCP_RUNBOOK.md`
- `docs/reports/nexus/2026-05-18-current-system-status-audit.md`
- `docs/reports/nexus/2026-05-18-payout-ledger-cleanup-before-after.md`
- `2026-05-22-pylon-v0.2-wallet-security-release-readiness-review.md`

Verification run locally:

```text
bash scripts/deploy/nexus/test-ldk-deploy-invariants.sh
```

Result:

```text
Nexus/Pylon active paths are LDK-only; no retired payment runtime/deploy dependency found.
```

Syntax checks:

```text
bash -n scripts/deploy/nexus/31-smoke-ldk-accepted-work-proof.sh scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh scripts/deploy/nexus/03-configure-and-start.sh
```

Result: passed.

Live read-only Nexus probe on 2026-06-01:

```text
GET https://nexus.openagents.com/healthz
GET https://nexus.openagents.com/api/stats
GET https://nexus.openagents.com/v1/treasury/status
```

Each endpoint returned Cloudflare `530` with body `error code: 1033`.

GCP remediation attempt:

```text
gcloud compute instances describe nexus-mainnet-1 --zone us-central1-a
gcloud compute instances list --filter='name~nexus|name~ldk|name~bitcoind'
```

Both failed because the active `chris@openagents.com` gcloud account requires
interactive reauthentication and this shell cannot prompt. This matches the
known deployment-access blocker documented in
`docs/reports/nexus/2026-05-18-current-system-status-audit.md`.

## Executive Findings

### 1. Public Nexus is currently down at the Cloudflare edge

The most important current finding is not the original `#4515` 503 symptom. The
public host currently returns Cloudflare `530` / `1033` for `/healthz`,
`/api/stats`, and `/v1/treasury/status`.

That makes the public Pylon v0.2 proof surface unavailable. It also prevents
honest closure of #4509 and #4510 because the required live LDK readiness and
accepted-work smoke cannot be rerun against current production.

The repo contract says this state is an emergency. The correct first response is
to restore public Nexus reachability before doing ordinary issue work.

Immediate recovery order:

1. Reauthenticate the human GCP deploy account or activate a dedicated
   noninteractive deploy service account with IAP SSH and instance-reset rights.
2. Inspect `nexus-mainnet-1`.
3. Check `nexus-relay.service`.
4. Check `nexus-cloudflared.service`.
5. Check VM-local origin health:

   ```text
   curl http://127.0.0.1:8080/healthz
   ```

6. If the VM is running but the tunnel is disconnected, restart
   `nexus-cloudflared.service` and verify the tunnel env/token path.
7. If the origin is unhealthy, inspect and restart `nexus-relay.service`.
8. If SSH is blocked or the guest network is wedged, reset `nexus-mainnet-1`.
9. Recheck:

   ```text
   curl https://nexus.openagents.com/healthz
   curl https://nexus.openagents.com/api/stats
   curl https://nexus.openagents.com/v1/treasury/status
   ```

Do not close any issue from the May 22 LDK-ready evidence alone while this edge
outage is present.

### 2. The active Nexus/Pylon payment path is LDK-only

The active-path invariant passed. That supports the Pylon v0.2 direction:
normal Nexus/Pylon production payment work is no longer Spark wallet work.

Important nuance:

- Root `Cargo.toml` excludes `crates/spark` from the workspace.
- The active Nexus/Pylon invariant reports no retired payment runtime/deploy
  dependency in the active paths.
- `Cargo.lock` and `apps/autopilot-deprecated` still contain historical Spark
  material, including `spark-wallet-cli` and `openagents-spark`.

That retained material is not the normal Nexus/Pylon production path. It should
be treated as legacy desktop/audit material unless a separate cleanup issue
deletes or quarantines it. Do not reintroduce Spark as a compatibility rail for
Pylon v0.2.

### 3. #4509 is likely satisfied by later release evidence, but current live proof is blocked

The open #4509 comments from 2026-05-19 said readiness was still
`needs_channels`, with one projected channel and roughly 2,000 sats outbound
against a two-channel / 20,000-sat floor.

The later root-level 2026-05-22 Pylon v0.2 review records a materially different
state after additional work:

- `ldk_readiness.state=ready`
- `ldk_readiness.projected_channel_count=3`
- `ldk_readiness.projected_outbound_capacity_sats=124430`
- minimum floor remained two channels and 20,000 sats outbound
- `payouts_failed_24h=0`
- `payouts_skipped_24h=0`
- `degraded_states=null`

That is enough to say #4509 appears complete by documented evidence. It is not
enough to close it today because public Nexus is currently unreachable and the
readiness state cannot be revalidated live.

Closure path:

1. Restore public Nexus.
2. Run the read-only status check:

   ```text
   curl -fsS https://nexus.openagents.com/v1/treasury/status |
     jq '{active_treasury_provider, active_treasury_rail, ldk_readiness, wallet_runtime_status, payout_loop_health, payouts_failed_24h, payouts_skipped_24h, degraded_states}'
   ```

3. Confirm:
   - provider/rail are `ldk` / `ldk`;
   - readiness is `ready`;
   - projected channel count is at least `2`;
   - projected outbound capacity is at least `20000`;
   - no current LDK failed/attention state.
4. Run `scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh` in read-only
   mode first. Use write mode only when the operator intentionally wants funding
   or channel mutation.
5. Close #4509 only with the fresh status output and artifact path.

### 4. #4510 has a command, but still needs a fresh successful production receipt

`scripts/deploy/nexus/31-smoke-ldk-accepted-work-proof.sh` exists and checks the
right chain:

- live treasury status;
- LDK provider/rail;
- LDK readiness;
- fresh CS336 A1 dispatch in launch mode;
- run detail polling;
- reconciled/rewarded window;
- accepted contribution;
- confirmed and settled payout;
- receipt under `docs/reports/nexus/ldk-accepted-work-smoke-*`.

The issue should remain open until a current-production run passes after public
Nexus is restored.

Closure path:

1. Restore public Nexus.
2. Confirm #4509 readiness.
3. Run:

   ```text
   NEXUS_LDK_ACCEPTED_WORK_ARTIFACT_DIR=docs/reports/nexus/ldk-accepted-work-smoke-$(date -u +%Y%m%dT%H%M%SZ) \
   scripts/deploy/nexus/31-smoke-ldk-accepted-work-proof.sh
   ```

4. Commit the resulting receipt only if it contains no raw secrets, raw payment
   targets, raw invoices, bearer tokens, seeds, or private keys.
5. Close #4510 with the receipt path, run id, window id, contribution id, payout
   key hash/status, amount, and reconciliation status.

### 5. #4515 remains the highest-priority code issue after reachability is restored

The issue report says authority requests intermittently failed with:

```text
503 embedded Nexus control API capacity exhausted
```

The current code still has the same basic pressure point:

- `apps/nexus-relay/src/durable.rs` creates `authority_slots` from
  `NEXUS_RELAY_AUTHORITY_MAX_IN_FLIGHT`.
- `proxy_authority_http_request` uses `try_acquire_owned()`.
- If no permit is immediately available, the relay returns `503` without queueing.
- The embedded authority runtime defaults to `4` worker threads.
- The authority HTTP client has a configurable timeout, but the code default is
  `180000 ms`.

The deploy scripts still default to the original capacity values:

- `NEXUS_RELAY_AUTHORITY_MAX_IN_FLIGHT=256`
- `NEXUS_RELAY_AUTHORITY_TOKIO_WORKER_THREADS=4`

The main deploy runtime env list does not include
`NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS`, and the binary activation script also
does not preserve/write that env. That means the newer timeout knob is not a
complete production control until the deploy scripts carry it.

Resolution path:

1. Restore public reachability first. A 530/1033 outage masks the 503 issue.
2. Add `NEXUS_RELAY_AUTHORITY_HTTP_TIMEOUT_MS` to:
   - `RUNTIME_ENV_VARS` in `scripts/deploy/nexus/03-configure-and-start.sh`;
   - the generated runtime env file in `03-configure-and-start.sh`;
   - `scripts/deploy/nexus/14-activate-binary-release.sh`.
3. Set production defaults to a bounded value closer to operator SLOs. The
   historical 180-second budget is a protective ceiling, not acceptable normal
   behavior.
4. Increase authority runtime worker threads for hosted production. Keep the
   value explicit in deploy receipts.
5. Replace immediate `try_acquire_owned()` rejection with bounded waiting:
   - short queue wait for normal bursts;
   - clear timeout;
   - `Retry-After` on overload;
   - per-path logging/metrics for permit wait and downstream duration.
6. Add authority endpoint latency metrics and slow-handler attribution. The root
   bug is not only the semaphore. Long-held permits mean slow or stuck authority
   handlers need to be named.
7. Add a production-safe authority-capacity smoke. The existing provider
   heartbeat dry-run is served inside `nexus-relay` before the authority permit,
   so it does not prove the embedded control plane can accept heartbeats. Add a
   read-only or dry-run endpoint behind `nexus-control`, then test it through the
   relay under bounded concurrency.
8. Deploy through the scripted registry path.
9. Soak with:
   - `/healthz`;
   - `/api/stats`;
   - `/v1/treasury/status`;
   - provider admission;
   - provider heartbeat;
   - training run lease;
   - validator challenge finalize;
   - the #4510 accepted-work smoke.

Close #4515 only after the public edge is stable and the authority control-plane
workflow no longer returns capacity-exhausted 503s under a representative Pylon
load.

## Suggested Issue Resolution Order

### Phase 0: Restore public Nexus

Owner: operator with GCP deploy access.

Outcome:

- `https://nexus.openagents.com/healthz` returns 200.
- `https://nexus.openagents.com/api/stats` returns 200.
- `https://nexus.openagents.com/v1/treasury/status` returns 200.
- The outage cause is recorded in a root-level or `docs/reports/nexus/` incident
  receipt.

Blocker:

- This shell cannot currently use `gcloud` to inspect or reset
  `nexus-mainnet-1` because the active human account requires interactive
  reauthentication.

### Phase 1: Reverify LDK v0.2 readiness (#4509)

Owner: Nexus/Pylon operator.

Outcome:

- Fresh treasury status proves `ldk` / `ldk`.
- LDK readiness is `ready`.
- Channel count and outbound capacity meet the two-channel / 20,000-sat floor.
- No current LDK payout attention.

Then close #4509.

### Phase 2: Prove accepted-work payment loop (#4510)

Owner: Nexus/Pylon operator.

Outcome:

- `31-smoke-ldk-accepted-work-proof.sh` passes against current production.
- Receipt includes run id, window id, contribution id, payout key/status, amount,
  and settled reconciliation.

Then close #4510.

### Phase 3: Fix authority capacity (#4515)

Owner: Nexus relay/control implementer.

Outcome:

- Public Nexus remains reachable.
- Control API no longer fails admission, heartbeats, run leases, or validator
  finalize with capacity-exhausted 503s under expected Pylon load.
- Deploy scripts carry the authority timeout and worker/capacity settings.
- Slow-handler metrics identify future saturation before it becomes a public
  outage.

Then close #4515.

### Phase 4: Close the tracker (#4504)

Owner: issue owner.

Outcome:

- #4509, #4510, and #4515 are closed with fresh evidence.
- Active-path LDK-only invariant still passes.
- No Spark wallet dependency exists in the normal Nexus/Pylon deploy path.
- A short tracker closeout links the incident receipt, LDK readiness proof, and
  accepted-work smoke receipt.

Then close #4504.

## Notes On Spark During Pylon v0.2

Do not route any new Pylon v0.2 work through Spark wallet.

Allowed:

- historical audit docs;
- retained deprecated desktop code pending a separate cleanup;
- old receipt readers only when explicitly needed to interpret historical rows.

Not allowed in normal Nexus/Pylon work:

- Spark funding targets;
- Spark payout targets;
- Spark drains;
- Spark SDK dependencies in Nexus deploy context;
- Spark compatibility flags in Pylon v0.2 registration, payout, recovery, or
  operator flows.

The right closing posture for #4504 is not "Spark still exists nowhere in the
repository." The right posture is "Spark is not part of active Nexus/Pylon
production payment, registration, deployment, release, or recovery behavior, and
any remaining historical material is quarantined outside the active path."

## Residual Risks

- The current public outage means all live-ready conclusions from May 22 need a
  fresh post-restore check.
- `gcloud` deploy access is still an operational single point of friction unless
  a dedicated noninteractive deploy identity is provisioned.
- The authority semaphore fix should not just raise limits. Without
  slow-handler attribution, saturation will recur.
- The accepted-work smoke is powerful and can create production work. It should
  remain a named operator command with explicit artifacts and receipt review.
