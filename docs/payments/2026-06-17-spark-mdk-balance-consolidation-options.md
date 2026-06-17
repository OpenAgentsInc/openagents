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
- Spark backup balance: receive-only backup funds, claimable/credited after
  Spark sync/claim, not exposed as Spark send authority.

## Selected near-term path: unified balance view

Show one contributor-facing wallet summary with separate backed buckets:

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
  exists.

## Consolidation options

1. Unified balance view only.
   This is safest and should ship first. It reads MDK status plus Spark
   `backup-status`, displays one summary, and leaves the two custody/rail states
   explicit.

2. Consented Spark-to-MDK sweep.
   This is the desired single-spendable-balance path. It needs the live
   integration step that actually moves the node's own credited Spark backup
   funds into the node's own MDK wallet, then records a public-safe reconcile
   receipt. The existing `migrate-spark --confirm-sweep` path has the consent
   and projection boundary, but the live transfer must be finished and proven
   before any UI can call Spark funds MDK-spendable.

3. Spark send/withdraw support.
   This would make Spark an active spend rail again. It is a broader authority
   change and should not be the next move unless option 2 is blocked by SDK or
   liquidity constraints. It would require a new promise gate, send-readiness
   preflight, private-material redaction review, and tests proving it does not
   widen accepted-work payout or public payout-target authority.

4. Hosted/operator consolidation.
   The operator could receive or reimburse the backup balance into MDK through a
   separate treasury action. This is operationally useful for recovery but should
   not become the normal product path because it adds custody/support overhead
   and blurs contributor self-custody.

## Recommendation

Ship option 1 immediately, then implement option 2 as the real consolidation
path. Do not implement option 3 unless a separate owner-approved design decides
that Spark should regain spend authority.
