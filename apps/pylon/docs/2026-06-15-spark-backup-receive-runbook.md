# Pylon Spark Backup Receive Runbook

Date: 2026-06-15

Issue: [#5078](https://github.com/OpenAgentsInc/openagents/issues/5078)

This runbook is the **owner activation path** for the Pylon Spark backup
receive rail. It is the end-to-end procedure the owner follows to flip the
scoped product promise

`payments.offline_receive_spark_fallback.v1`

from blocked to green. The code (slices 1-3 of #5078) is landed; the only
remaining step is the owner-gated live integration smoke described below.

Read alongside:

- `apps/pylon/docs/2026-06-15-spark-backup-receive-fallback-audit.md` (the spec)
- `apps/pylon/docs/legacy-spark-wallet-migration.md` (the consent model)

## What this rail is (and is not)

Spark is reintroduced ONLY as a **receive-only backup target** for when the
primary MDK rail is offline and cannot create a receive request.

- It is **receive-only**. There is NO Spark send/payout path.
- The `migrate-spark --confirm-sweep` reconcile moves the node's **OWN**
  received Spark backup funds into the node's **OWN** MDK wallet, under
  explicit consent. It is a reconcile, **NOT** a payout, **NOT** a send to a
  third party, and **NOT** accepted-work settlement.
- `PayoutTargetKind` and `admitPayoutTarget` are unchanged. Spark gains **no**
  public payout-target authority.
- It is **inert by default**: nothing runs and no SDK code loads until the
  backup is opt-in enabled AND a Breez/Spark credential plus a local wallet
  seed are present. With anything missing the node behaves exactly as before.

## State and storage (Pylon home, private)

- config: `<pylon-home>/wallet/spark-backup/config.json`
- SDK storage: `<pylon-home>/wallet/spark-backup/sdk/`
- cached local target: `<pylon-home>/wallet/spark-backup/receive-target.json`
  (mode `0600`; holds raw wallet-operable receive material)
- reconciliation ledger: existing Pylon ledger events
  (`backup-receive-selected`, `spark-backup-reconcile-swept`)

## Step 1 — set the Breez/Spark API key (local, never committed)

The adapter reads the API key from the first of these env vars that is set,
in order:

1. `OPENAGENTS_SPARK_API_KEY`
2. `BREEZ_API_KEY`
3. `PYLON_SPARK_BACKUP_API_KEY`

Setting one of these **overrides** the embedded default. Set it in the node's
local environment only (for example the local `.secrets/` env file the node
sources, or the shell that launches Pylon). Do NOT commit it, do NOT paste it
into issues, logs, the Forum, or any public projection. A real key value is
intentionally NOT recorded in this runbook.

The Spark network defaults to `mainnet`; set `PYLON_SPARK_BACKUP_NETWORK=regtest`
for a regtest test wallet.

**A manual key is NOT required (#5078):** an embedded, owner-authorized default
Breez/Spark service key is compiled in, so once opt-in is enabled the receive
backup is credential-ready out-of-box and `backup-status` reports
`address-ready` (or `helper-unavailable` if the node cannot reach the Spark
network) — not `credential-missing`. Inert-by-default is enforced by the
`PYLON_SPARK_BACKUP_ENABLED` flag, NOT by key presence. The env vars above only
override the embedded key. (`credential-missing` now appears only if the
embedded key were ever stripped from the build.)

## Step 2 — opt in

```sh
export PYLON_SPARK_BACKUP_ENABLED=1
```

This flag is the master opt-in. With it unset (the default) the rail is fully
disabled regardless of any key.

The wallet seed is the node's existing local identity mnemonic
(`<pylon-home>/identity.mnemonic`). It is read only to seed the SDK closure
in-process; it is never returned, logged, or emitted.

## Step 3 — offline-recipient receive smoke

Goal: prove that when MDK cannot create a receive request, the node still
hands out a local Spark backup target, and that public output carries only
redacted refs.

0. Check the unified local wallet summary:

   ```sh
   pylon wallet status
   ```

   The `unifiedBalance` block shows `mdkSpendableSats`,
   `sparkBackupCreditedSats`, `sparkBackupClaimableSats`,
   `sparkBackupPendingSweepSats`, and `totalVisibleSats`. The total is visible
   value, not one spendable MDK balance; Spark backup sats remain non-MDK-
   spendable until a later consented sweep records a reconcile receipt.

1. Confirm enablement and helper readiness (no funds, no movement):

   ```sh
   pylon wallet backup-status
   ```

   Expected when enabled + credential + helper are ready: `state: "address-ready"`
   (or `cached-address-ready` if the helper is offline but a target is cached),
   with a redacted `receiveTargetRef` of the form
   `wallet.backup.spark_address.<digest>`. No raw address appears.

2. Generate / view the local target (LOCAL/PRIVATE terminal only):

   ```sh
   pylon wallet backup-receive --kind spark-address --show-local-target
   ```

   Without `--show-local-target` the raw target is withheld and only refs are
   printed. With the flag the raw Spark address prints to the local terminal
   labelled `LOCAL/PRIVATE` and is cached at `receive-target.json` (mode 0600).
   It is never posted to any network or public projection.

3. Simulate MDK offline and route a receive through the fallback:

   ```sh
   pylon wallet receive --amount 1000
   ```

   With MDK reachable this returns the MDK `wallet.receive.*` ref. With MDK in
   an offline/unavailable class AND the backup enabled, it returns
   `rail: "spark_backup"`, a redacted `wallet.backup_receive.<digest>`
   receipt, and `rawTargetAvailableLocally: true`. An MDK validation/user
   error does NOT switch rails.

4. Send test sats to the local Spark address, then re-run `backup-status` to
   watch the funds move through detected / pending / credited. `backup-status`
   surfaces the sweep recommendation (`recommendSparkSweep`): once a balance is
   credited it reports `sweep-to-mdk-recommended` and
   `action.wallet.spark_backup.run_migrate_spark_with_consent`.

## Step 4 — `migrate-spark` sweep + reconcile (consented)

This is the receive-side reconcile half: move the node's OWN received Spark
backup funds into its OWN MDK wallet.

1. Dry-run probe (no consent, no movement):

   ```sh
   pylon wallet migrate-spark --sweep
   ```

   With a detected balance and no consent this returns `state: "consent-required"`
   and `blocker.wallet.spark_backup.sweep_consent_required`. It refuses to move
   funds.

2. Prepare the MDK destination so it can receive (the node's own MDK wallet
   online and able to create a receive request).

3. Execute under explicit consent:

   ```sh
   pylon wallet migrate-spark --confirm-sweep --destination-ready
   ```

   `--confirm-sweep` is the required explicit consent flag. `--destination-ready`
   asserts the MDK destination is ready. On success the helper claims the
   node's unclaimed deposits and the swept amount is recorded with a redacted
   reconcile receipt `receipt.pylon.spark_backup_reconcile.<digest>` (also
   appended to the local ledger as `spark-backup-reconcile-swept`). State
   becomes `swept-to-mdk`.

   Without `--destination-ready` the sweep refuses with
   `state: "sweep-failed"` and `blocker.wallet.spark_backup.mdk_destination_not_ready`;
   funds stay untouched. Settlement is NEVER marked as completed on a failed or
   unconsented sweep.

The legacy-balance migration preflight (old Spark/Breez wallet recovery via the
12-word phrase) remains the default `migrate-spark` behavior when the sweep
flags are absent. See `legacy-spark-wallet-migration.md`.

## Public-projection redaction policy

Allowed public refs only:

- `wallet.backup.spark_address.<digest>`
- `wallet.backup_receive.<digest>`
- `receipt.pylon.spark_backup_reconcile.<digest>`
- blocker refs such as `blocker.wallet.spark_backup.credential_missing`,
  `blocker.wallet.spark_backup.helper_unavailable`,
  `blocker.wallet.spark_backup.sync_unavailable`,
  `blocker.wallet.spark_backup.sweep_consent_required`,
  `blocker.wallet.spark_backup.mdk_destination_not_ready`

Never emitted to public projections, the Forum, assignment closeouts, public
stats, the wallet-readiness post, this runbook, tests, or stdout (outside an
explicit `--show-local-target` local terminal print):

- raw Spark addresses / invoices / payment requests
- raw BOLT11 invoices, payment hashes, preimages
- mnemonics / wallet seeds
- Breez / Spark API keys
- SDK storage paths or local wallet home paths
- raw helper stdout/stderr

`assertPublicProjectionSafe` enforces this on every projection and ledger
record; the wallet tests assert raw Spark material is rejected.

## Receive-only boundary (non-negotiable)

- No Spark send to third parties. The only fund movement is the consented
  sweep of the node's OWN received funds into its OWN MDK wallet.
- `PayoutTargetKind` does not gain `spark_address`; `admitPayoutTarget` is
  unchanged; there is no public payout-target authority for Spark.
- Funds remain non-settled until Spark sync plus the consented reconcile.

## Remaining owner-gated item: live integration smoke

All three code slices of #5078 are landed and covered by mock-backed tests.
The remaining step is owner-gated and cannot run without a real credential:

1. Set a real Breez/Spark API key (Step 1) and `PYLON_SPARK_BACKUP_ENABLED=1`.
2. Run the offline-recipient receive smoke (Step 3) against a real Spark
   wallet, sending real test sats.
3. Run the consented `migrate-spark --confirm-sweep --destination-ready`
   reconcile (Step 4) and confirm the MDK balance after the sweep.
4. Confirm public output across the smoke contains only refs and
   blocker/action refs.

Completing that live smoke is what flips
`payments.offline_receive_spark_fallback.v1` green.
