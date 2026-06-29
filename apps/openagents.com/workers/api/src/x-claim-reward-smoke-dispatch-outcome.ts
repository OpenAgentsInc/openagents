import type { XClaimRewardTreasuryDispatchSummary } from './x-claim-reward-treasury-dispatcher'

const DispatchNotEnabledReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_not_enabled'
const NotExactlyOneSettledReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_not_exactly_one_settled'
const RewardFailedReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_reward_failed'
const PaymentStillPendingReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_payment_still_pending'
const QueueNotDrainedReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_queue_not_drained'
const RunSkippedReasonRef =
  'reason.public.x_claim_reward_smoke_dispatch_run_skipped'

export type XClaimRewardSmokeDispatchOutcomeCheck = Readonly<{
  name: string
  ok: boolean
  reasonRef: string | null
}>

export type XClaimRewardSmokeDispatchOutcomeReport = Readonly<{
  blockingReasonRefs: ReadonlyArray<string>
  checks: ReadonlyArray<XClaimRewardSmokeDispatchOutcomeCheck>
  /**
   * Whether the worker-side dispatch run completed the first live single-reward
   * smoke cleanly: dispatch enabled, exactly one reward settled, nothing failed
   * or left pending, and the queue fully drained. `true` means the operator may
   * proceed to the per-row post-settlement receipt audit.
   */
  ok: boolean
  /**
   * Public-safe echo of the run counters. These are already aggregate counts and
   * skip-reason refs — never invoices, payment ids, destinations, or preimages.
   */
  outcomeSummary: Readonly<{
    failed: number
    pending: number
    polled: number
    requested: number
    settled: number
    skippedReasonRefs: ReadonlyArray<string>
  }>
}>

/**
 * Audits the {@link XClaimRewardTreasuryDispatchSummary} returned by
 * `runXClaimRewardTreasuryDispatch` after the first live single-reward smoke on
 * the worker-side (flag-gated) dispatch path.
 *
 * This is a pure, public-safe gate that complements the per-row post-settlement
 * receipt audit: the row audit confirms the *settled record* is clean, while
 * this confirms the *run that produced it* did exactly the bounded smoke and
 * nothing more — dispatch was enabled, exactly one reward settled, no reward
 * failed, no payment was left pending, the dispatch queue drained, and the run
 * skipped nothing (e.g. liquidity or daily-cap stops). It moves no funds and
 * only reads the summary's existing aggregate counters and skip-reason refs.
 */
export const assertXClaimRewardSmokeDispatchOutcome = (
  summary: XClaimRewardTreasuryDispatchSummary,
): XClaimRewardSmokeDispatchOutcomeReport => {
  const queueDrained =
    summary.pending === 0 &&
    summary.stats.pendingPaymentCount === 0 &&
    summary.stats.requestedDispatchCount === 0

  const checks: ReadonlyArray<XClaimRewardSmokeDispatchOutcomeCheck> = [
    {
      name: 'dispatch_run_enabled',
      ok: summary.stats.enabled,
      reasonRef: summary.stats.enabled ? null : DispatchNotEnabledReasonRef,
    },
    {
      name: 'exactly_one_settled',
      ok: summary.settled === 1,
      reasonRef: summary.settled === 1 ? null : NotExactlyOneSettledReasonRef,
    },
    {
      name: 'no_reward_failed',
      ok: summary.failed === 0,
      reasonRef: summary.failed === 0 ? null : RewardFailedReasonRef,
    },
    {
      name: 'no_payment_pending',
      ok: summary.pending === 0,
      reasonRef: summary.pending === 0 ? null : PaymentStillPendingReasonRef,
    },
    {
      name: 'dispatch_queue_drained',
      ok: queueDrained,
      reasonRef: queueDrained ? null : QueueNotDrainedReasonRef,
    },
    {
      name: 'no_skipped_reasons',
      ok: summary.skippedReasonRefs.length === 0,
      reasonRef:
        summary.skippedReasonRefs.length === 0 ? null : RunSkippedReasonRef,
    },
  ]

  const blockingReasonRefs = Array.from(
    new Set(
      checks
        .filter(check => !check.ok && check.reasonRef !== null)
        .map(check => check.reasonRef as string),
    ),
  )

  return {
    blockingReasonRefs,
    checks,
    ok: blockingReasonRefs.length === 0,
    outcomeSummary: {
      failed: summary.failed,
      pending: summary.pending,
      polled: summary.polled,
      requested: summary.requested,
      settled: summary.settled,
      skippedReasonRefs: summary.skippedReasonRefs,
    },
  }
}
