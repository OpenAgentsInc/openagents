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

6. Treasury-to-agent payout chunking.
   **Implemented in #5179 for hosted-MDK accepted-work payouts.** When the
   operator payout destination is reusable (Spark Lightning Address, LNURL, or a
   BOLT12-style offer), hosted-MDK payouts above 25,000 sats are split into
   sequential chunks. Each chunk gets a deterministic hash idempotency key, and
   reconciliation waits on every chunk idempotency key before claiming the
   accepted-work payout is settled. Fixed BOLT11 invoices are not chunked because
   they are amount-bound. The accepted-work payout route resolves the pylon owner
   agent's saved Forum tip-recipient Spark Lightning Address when the operator
   request does not supply a private destination. Public projections expose only
   redacted payment refs, chunk count, public-safe metadata refs, and settlement
   state.

7. Recipient-attributed treasury ledger.
   **Implemented in #5180 for direct treasury/tips-buffer payout accounting.**
   `treasury_transactions` now stores a public-safe recipient ref (or
   destination-hash fallback), optional keyed owed refs/amounts, and a
   recipient-confirmed state separate from treasury-side `settled`. Operator
   reports can pull owed vs settled-sent vs confirmed-received totals by
   recipient and flag over-send when settled sent exceeds keyed owed. Recipient
   confirmation is a separate operator action backed by public-safe receipt or
   balance evidence; it does not expose raw destination, invoice, preimage,
   payment hash, or wallet material.

8. MDK scoped out of the agent-balance path.
   **Implemented in #5181.** Pylon no longer probes MDK to decide the primary
   agent wallet status, heartbeats do not spawn an MDK wallet probe by default,
   paid assignment admission no longer gates on local MDK send readiness, and
   tip-recipient self-claims publish a Spark Lightning Address without minting a
   local MDK BOLT 12 offer. Forum readiness, tip ladder, sweeps, Artanis spend,
   and x-claim dispatch now accept a registered Spark Lightning Address as the
   preferred public recipient destination. Legacy BOLT 12 remains readable only
   for rows that do not have a Spark Lightning Address; it is not a fallback once
   a Spark destination exists. MDK remains intentionally alive for customer
   checkouts and operator/treasury rails.

## Recommendation

The product path is option 4: Spark is the single agent balance. Option 2 remains
a local recovery/compatibility path when a user explicitly wants to move credited
Spark funds into MDK, and option 3 is the direct spend path for the primary Spark
balance. Option 6 is the treasury delivery policy for larger agent payouts while
Spark Lightning Address is the normal recipient endpoint. Option 7 is the
operator accounting layer that distinguishes sent from recipient-confirmed
received. Option 8 is the guardrail that keeps MDK out of contributor-facing
balance/readiness decisions while preserving MDK where it belongs:
checkouts and treasury.
