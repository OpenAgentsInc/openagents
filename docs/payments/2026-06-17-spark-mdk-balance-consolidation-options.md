# Spark backup balance and MDK consolidation options

Date: 2026-06-17

## Context

The rc.12 Spark backup receive path is now proven for the scoped offline-receive
claim: a payout to a Spark-backed Lightning Address can land while the primary
MDK wallet is not accepting inbound, and `backup-claim` / `backup-status` can
show the recipient-visible credited Spark backup balance.

That is not the same as one spendable MDK balance. Today the contributor can see
two balances:

- MDK primary wallet balance: the normal receive/spend rail.
- Spark backup balance: backup funds claimable/credited after Spark sync/claim;
  now directly spendable only through the explicit consented Spark send rail
  added in #5177.

## Implemented near-term path: unified balance view (#5168)

Pylon now shows one contributor-facing wallet summary with separate backed
buckets in `pylon wallet status`, the local control wallet-status response, and
the operator snapshot:

- `mdkSpendableSats`
- `sparkBackupCreditedSats`
- `sparkBackupClaimableSats`
- `sparkBackupPendingSweepSats`
- `totalVisibleSats = mdkSpendableSats + sparkBackupCreditedSats + sparkBackupClaimableSats`

This solves the product confusion without changing fund authority. It must label
the total as visible/claimable value, not one spendable MDK balance. Public and
operator surfaces should preserve the existing caveats:

- receive readiness is not send readiness;
- Spark backup receive is not accepted-work settlement authority;
- Spark backup balance is not MDK spendable until a real sweep/transfer receipt
  exists;
- direct Spark send is a separate spend rail, not an MDK balance merge.

The summary carries `caveat.wallet.total_visible_is_not_single_spendable_balance`
and `caveat.wallet.spark_backup_is_not_mdk_spendable_until_sweep_receipt` so
callers can display the aggregate without implying one spendable MDK balance.

## Consolidation options

1. Unified balance view only.
   **Done in #5168.** It reads MDK status plus Spark
   `backup-status`, displays one summary, and leaves the two custody/rail states
   explicit.

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

4. Hosted/operator consolidation.
   The operator could receive or reimburse the backup balance into MDK through a
   separate treasury action. This is operationally useful for recovery but should
   not become the normal product path because it adds custody/support overhead
   and blurs contributor self-custody.

## Recommendation

Option 1 and option 2 remain the low-confusion product path: show the unified
view first, then use the consented sweep to consolidate credited Spark backup
funds into MDK when the user wants one MDK-spendable balance. Option 3 is now
available for direct Spark withdrawals when the user explicitly wants to spend
from Spark without first sweeping to MDK.
