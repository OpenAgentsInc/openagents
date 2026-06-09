# Autopilot-Controlled Pylon Production Earning Proof

Date: 2026-04-23

Repo state: `openagents` `main` at `96295609b` (`Fix Pylon homework earning proof flow`).

Proof root: `/private/tmp/pylon-autopilot-prod-e2e-20260423T042057Z`

## Summary

This proof used the Autopilot Tauri control surface to keep a Pylon worker online, dispatched a bounded CS336 A1 homework/training run through hosted Nexus, validated the worker contribution with a separate Pylon validator process, confirmed accepted-work payout through Treasury, and verified that the worker Pylon wallet balance increased.

The user-facing result is the one we needed to prove: Autopilot can put Pylon online for available paid training work, Nexus can assign that online node a homework run, the local runtime can complete the work, validation can accept it, Treasury can dispatch the accepted-work payment, and the Pylon wallet balance can go up.

## What Ran

The Autopilot-controlled worker used a fresh isolated Pylon home:

- Pylon home: `/private/tmp/pylon-autopilot-prod-e2e-20260423T042057Z/pylon-home`
- Config: `/private/tmp/pylon-autopilot-prod-e2e-20260423T042057Z/pylon-home/config.json`
- Admin listen address: `127.0.0.1:55477`
- Network id: `trainnet.cs336.a1.autopilot-prod.20260423T042057Z`
- Node pubkey: `be83d8974051cf6874e12117d04773cd1d0bb3b98acacac801a98ef0d5bf69e9`
- Settlement destination: `spark1pgssyt9agft907ew09l6kndl59gtguccvpyuv6h90489ct7hm0drz7rzmswm7g`
- Pylon release id reported to Nexus: `openagents.pylon@0.1.9`
- Pylon build version reported to Nexus: `0.1.9`
- Pylon process state from Autopilot control: `running`
- Autopilot desired mode: `online`
- Provider state: `online`
- Eligible products: `2`
- Ready model: `gemma4-e4b-local:latest`

The source binary was built from `main` at `96295609b`, which is newer than the older `pylon-v0.1.9` tag. Do not treat the public `pylon-v0.1.9` GitHub release as containing every source fix in this proof. The release target for the exact behavior proven here is `pylon-v0.1.10` or newer, cut from `96295609b` or later, followed by a fresh npm/bootstrap proof from a fresh Pylon home.

## Production Dispatch

The admin dispatch used the hosted Nexus homework endpoint with a one-run, one-contributor, 25-sat budget and an isolated network id so only the proof worker could claim it.

- Dispatch endpoint: `POST /v1/admin/homework/cs336-a1/dispatch`
- Batch id: `dispatch.cs336.a1.20260423043525.a7483c6a`
- Requested runs: `1`
- Launched runs: `1`
- Failed runs: `0`
- Amount: `25 sats`
- Run id: `run.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390`
- Window id: `window.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390.0001`
- Worker assignment id: `assign.run.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390.window.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390.0001.worker.1.attempt1`

The run detail after validation showed:

- Run status: `running`
- Latest closeout status: `rewarded`
- Accepted contributors: `1`
- Validator open: `0`
- Validator queued: `0`
- Featured window status: `reconciled`
- Featured window total contributions: `1`
- Featured window admitted contributions: `1`
- Featured window accepted contributions: `1`
- Featured window payout eligible: `true`

The run-level status remained `running` because the broader hosted run object can stay open while the relevant proof window is already reconciled and payout eligible. For this proof, the window-level reconciliation, accepted contributor count, Treasury payout record, and wallet receive are the completion criteria.

## Validation And Payout

A separate validator Pylon process used its own isolated home:

- Validator home: `/private/tmp/pylon-autopilot-prod-e2e-20260423T042057Z/validator/pylon-home`
- Validator admin listen address: `127.0.0.1:55880`
- Validator checkpoint address: `127.0.0.1:55881`
- Validator role claims: `validator`
- Validator network: `trainnet.cs336.a1.autopilot-prod.20260423T042057Z`

The first validator pass reported `artifact_incomplete`, which was retryable and consistent with claiming the aggregate challenge before the worker artifact bundle was fully visible through the authority/artifact path. After forcing worker refresh/sync and retrying validator intake, validation completed and the validator closeout progress reported:

- Stage: `paid`
- Acceptance state: `rewarded`
- Accepted outcome id: `accepted.training_window.window.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390.0001`
- Payout state: `confirmed`
- Payout id: `accepted_work:accepted.training_window.window.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390.0001:e5f851f79f0d0d31afde7acb9687ed0c133036bc78459d4bd9504df02b862984:be83d8974051cf6874e12117d04773cd1d0bb3b98acacac801a98ef0d5bf69e9`
- Payout receipt id: `019db8a2-98d2-7890-95e4-6a1d78709a3c`
- Payout reconciliation status: `settled`

Treasury status after the payment showed:

- Wallet runtime status: `connected`
- Treasury wallet balance: `49483 sats`
- Placeholder payout mode: `disabled`
- Recent training payout status for this run: `confirmed`
- Recent training payout reconciliation status for this run: `settled`
- Recent training payout amount: `25 sats`
- Payment id: `019db8a2-98d2-7890-95e4-6a1d78709a3c`

The public run detail still surfaced a `treasury_degraded` caveat for `wallet_snapshot_stale`, and Treasury status surfaced a `snapshot_stale` continuity warning. That warning did not block this proof: the runtime wallet was connected, placeholder payouts were disabled, the accepted-work payout record was confirmed and settled, and the worker wallet received the sats.

## Wallet Balance Proof

The worker wallet balance was captured before dispatch and after payout:

```json
{
  "before_total_sats": 0,
  "after_total_sats": 25,
  "delta_total_sats": 25,
  "before": {
    "spark_sats": 0,
    "lightning_sats": 0,
    "onchain_sats": 0,
    "total_sats": 0
  },
  "after": {
    "spark_sats": 25,
    "lightning_sats": 0,
    "onchain_sats": 0,
    "total_sats": 25
  }
}
```

Wallet history also showed the concrete receive:

```json
{
  "payment_id": "019db8a2-98d2-7890-95e4-6a1d78709a3c",
  "direction": "receive",
  "status": "completed",
  "amount_sats": 25,
  "fees_sats": 0,
  "method": "spark"
}
```

This is the decisive balance proof. The worker Pylon wallet went from `0 sats` to `25 sats`, and the receive payment id matches the Treasury accepted-work payout receipt id.

## Autopilot Surface Proof

The final Autopilot control projection showed:

- Status: `Homework paid`
- Online stage: `ready`
- Intake stage: `ready`
- Closeout stage: `paid`
- Payout stage: `dispatched`
- Current run id: `run.cs336.a1.autopilot-prod-20260423T042057Z_20260423043525_a7483c6a_0001.20260423043525.65bb3390`
- Recent closeout acceptance state: `rewarded`
- Recent closeout stage: `paid`
- Recent worker issues after final sync: none

The Autopilot worker-side cache still described the payout stage as `dispatched` / `pending_confirmation` after local sync, while the validator closeout, Treasury record, and wallet history already showed `confirmed`, `settled`, and `completed`. That is an observation about the worker-local projection lag, not a payout failure. The user-facing status was already `Homework paid`, and the wallet balance/history confirmed receipt.

## Code Fixes Proven

This proof exercised the fixes in `96295609b`:

- Pylon now creates a default local Spark payout destination during the long-lived serve path when the config has no payout destination, instead of coming online without a usable payment target.
- Pylon validator replay now reuses an existing retained content-addressed snapshot if a mutable local source path drifted between attempts, avoiding retry-time artifact mismatch.
- Autopilot only shows `Homework paid` when closeout/payout evidence is terminal enough, and stale historical issues no longer override the current paid proof projection.

The focused verification that accompanied those fixes passed before this live proof:

- `cargo test -p pylon config_set_updates_payout_destination`
- `cargo test -p pylon default_payout_destination_uses_wallet_spark_address`
- `cargo test -p pylon snapshot_training_retained_artifact_binding`
- `cargo check -p pylon`
- `cargo check -p autopilot`
- `cargo test -p autopilot --lib`
- `scripts/autopilot/tauri-control-smoke.sh --homework-handshake --timeout-ms 600000`

## Reproduction Notes

The operator runbook remains `docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`. For this Autopilot-controlled variant, the important differences are:

- Start Autopilot Tauri with an isolated control manifest and isolated Pylon home.
- Put the Autopilot-managed Pylon process into `online` mode with `autopilotctl-tauri`.
- Verify the Pylon process advertises online state, a payout destination, and eligible products before dispatch.
- Dispatch one bounded homework run with `only_online: true`, a short run slug, a 25-sat budget, and the isolated `network_id`.
- Wait for the worker contribution to seal before starting or forcing the validator path.
- If the validator claims too early and reports `artifact_incomplete`, force worker `training refresh` / `training sync` once, then retry validator intake.
- Confirm completion with four independent surfaces: Nexus run detail, validator closeout progress, Treasury recent payout, and worker wallet balance/history.

Do not record bearer tokens, wallet mnemonics, Spark API keys, or raw private service credentials in proof artifacts.

## Remaining Follow-Up

The system works in production for this bounded proof. The next cleanup is release and projection hardening, not core payout repair:

- `pylon-v0.1.10` was published from `8b814d800b6f4291892a1bcc835fb34a2b91fee1` as a GitHub release with a `darwin-arm64` archive and as npm package `@openagentsinc/pylon@0.1.10`. A fresh `@openagentsinc/pylon@0.1.10` production proof then ran from a new Pylon home, received hosted homework work, reconciled one accepted contribution, produced payout receipt `019db8c1-6639-7751-a717-cee14dd2012e`, and increased the released worker wallet from `0` to `25` sats. The release receipt is `docs/reports/nexus/20260423-050434-pylon-v0.1.10-release.json`.
- Make worker-local payout projection catch up from completed wallet receive evidence so Autopilot can display `confirmed` instead of only `dispatched` after the wallet has already received the sats.
- Reduce or repair the Treasury `wallet_snapshot_stale` continuity warning so operator dashboards do not show scary degraded caveats when the wallet runtime is connected and payouts are actually settling.
- Prefer admin/API or Autopilot control endpoints over standalone `pylon training intake` commands while a long-lived Pylon process is running, to avoid confusing file-backed state with a different in-memory supervisor process.
