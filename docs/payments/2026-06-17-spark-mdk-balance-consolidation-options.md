# Spark backup balance and MDK consolidation options

Date: 2026-06-17

## Context

The rc.12 Spark receive path is now proven for the scoped offline-receive claim:
a payout to a Spark-backed Lightning Address can land while MDK is not accepting
inbound, and `backup-claim` / `backup-status` can show the recipient-visible
credited Spark balance.

After #5178, the contributor-facing product model is no longer two balances:

- Spark is the primary agent wallet and the one displayed agent balance.
- MDK is auxiliary for treasury/checkouts and legacy compatibility, not part of
  the agent balance.
- Spark claimable HTLCs remain a pending receive state until `backup-claim`
  credits them into the Spark balance.

## Implemented primary path: Spark as the single agent balance (#5178)

Pylon now reports the Spark balance as the primary balance in `pylon wallet
status`, the local control wallet-status response, readiness/heartbeat, and the
operator snapshot:

- `primaryRail = spark`
- `primaryBalanceSats = Spark credited balance`
- `primarySpendableSats = Spark credited balance`
- `totalVisibleSats = Spark credited balance`
- `mdkSpendableSats`
- `sparkBackupCreditedSats`
- `sparkBackupClaimableSats`
- `sparkBackupPendingSweepSats = 0`

For the Spark-primary projection, `mdkSpendableSats` is intentionally null. Local
MDK status may still be checked for legacy/sweep/treasury paths, but the amount
is excluded from the public agent-balance projection.

Public and operator surfaces preserve these caveats:

- receive readiness is not send readiness;
- Spark receive is not accepted-work settlement authority;
- claimable Spark HTLCs are not credited balance until `backup-claim` succeeds;
- MDK is excluded from the agent primary balance.

The summary carries `caveat.wallet.mdk_excluded_from_agent_primary_balance` and,
when needed, `caveat.wallet.spark_claimable_htlcs_require_backup_claim`.

## Consolidation options

1. Unified balance view only.
   **Historical/interim (#5168).** It read MDK status plus Spark
   `backup-status` and displayed separate buckets. This reduced confusion but
   still left two agent-facing balances.

2. Consented Spark-to-MDK sweep.
   **Implemented in #5169.** `migrate-spark --confirm-sweep
   --destination-ready` now creates a fresh local MDK receive target, pays it
   from the node's own credited Spark backup balance through a private
   sweep-only SDK adapter, then verifies the MDK balance increased before it
   emits `receipt.pylon.spark_backup_reconcile.<digest>`. Until that verified
   receipt exists, the projection stays `sweep-pending-mdk-credit` or
   `sweep-failed` and the funds are not described as MDK-spendable.

3. Spark send/withdraw support.
   **Implemented in #5177.** `wallet send --rail spark --confirm-send` pays from
   the node's own Spark backup wallet to a raw BOLT11/Spark payment request or
   Lightning Address/LNURL-pay destination. The adapter follows the previously
   working Spark flow (`prepareSendPayment` -> `sendPayment`) for payment
   requests and the SDK LNURL flow (`parse` -> `prepareLnurlPay` -> `lnurlPay`)
   for Lightning Addresses. Public output and ledger rows contain only digest
   refs, amount/fee, method, and status. This does not widen accepted-work payout
   authority and does not claim the Spark balance is MDK-spendable.

4. Spark-primary agent balance.
   **Implemented in #5178.** The Pylon status, heartbeat/readiness, local control
   wallet status, and operator snapshot now source the agent-facing balance from
   Spark. MDK balance is omitted from the agent balance and left to
   treasury/checkouts/legacy paths.

5. Hosted/operator consolidation.
   The operator could receive or reimburse the backup balance into MDK through a
   separate treasury action. This is operationally useful for recovery but should
   not become the normal product path because it adds custody/support overhead
   and blurs contributor self-custody.

## Recommendation

The product path is option 4: Spark is the single agent balance. Option 2 remains
a local recovery/compatibility path when a user explicitly wants to move credited
Spark funds into MDK, and option 3 is the direct spend path for the primary Spark
balance.
