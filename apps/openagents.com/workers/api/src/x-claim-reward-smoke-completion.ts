import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import {
  assertXClaimRewardSmokeDispatchOutcome,
  type XClaimRewardSmokeDispatchOutcomeReport,
} from './x-claim-reward-smoke-dispatch-outcome'
import {
  buildXClaimRewardSmokeTransitionRequest,
  type XClaimRewardSmokeTransitionProposal,
  type XClaimRewardSmokeTransitionRequest,
} from './x-claim-reward-smoke-receipt-audit'
import type { XClaimRewardTreasuryDispatchSummary } from './x-claim-reward-treasury-dispatcher'

const DispatchRunNotCleanReasonRef =
  'reason.public.x_claim_reward_smoke_completion_dispatch_run_not_clean'
const SettledRowNotReadyReasonRef =
  'reason.public.x_claim_reward_smoke_completion_settled_row_not_ready'

export type XClaimRewardSmokeCompletionInput = Readonly<{
  /**
   * The settled reward row produced by the smoke. Inspected by the per-row
   * post-settlement audit and the transition-request builder.
   */
  reward: XClaimRewardRecord
  /**
   * The summary returned by `runXClaimRewardTreasuryDispatch` for the flag-gated
   * worker-side run that produced the settled row. Inspected by the run-level
   * outcome auditor.
   */
  summary: XClaimRewardTreasuryDispatchSummary
}>

export type XClaimRewardSmokeCompletionReport = Readonly<{
  blockingReasonRefs: ReadonlyArray<string>
  /**
   * The run-level outcome audit of the worker-side dispatch summary: confirms the
   * run did exactly the bounded single-reward smoke (dispatch on, exactly one
   * settled, nothing failed/pending, queue drained, no skips).
   */
  dispatchOutcome: XClaimRewardSmokeDispatchOutcomeReport
  /**
   * Whether BOTH the run-level outcome audit and the per-row transition proposal
   * passed. `true` means the operator may submit `transitionRequest` to
   * `POST /api/operator/product-promises/transitions`.
   */
  ready: boolean
  /**
   * The per-row post-settlement audit plus the assembled transition proposal.
   */
  transitionProposal: XClaimRewardSmokeTransitionProposal
  /**
   * The public-safe `POST /api/operator/product-promises/transitions` body — only
   * emitted when BOTH the run-level and row-level gates pass; `null` otherwise.
   * Building it flips no promise state and moves no funds.
   */
  transitionRequest: XClaimRewardSmokeTransitionRequest | null
}>

/**
 * Composite go/no-go gate for the live single-reward X-claim dispatch smoke.
 *
 * The run-level outcome auditor inspects the dispatch *summary* and the
 * transition-request builder inspects the settled *row*, but until now nothing
 * required BOTH before proposing the green flip. That left a hole: a worker-side
 * run that settled the wrong number of rewards, left a payment pending, or
 * skipped on liquidity/daily-cap could still produce a green transition proposal
 * as long as the single inspected row happened to look clean.
 *
 * This pure, public-safe gate closes that hole. It runs
 * {@link assertXClaimRewardSmokeDispatchOutcome} on the run summary AND
 * {@link buildXClaimRewardSmokeTransitionRequest} on the settled row, and only
 * emits the transition request when BOTH pass. It moves no funds and flips no
 * promise state — the registry route still re-evaluates blockers and the green
 * flip still requires owner sign-off.
 */
export const assertXClaimRewardSmokeCompletion = (
  input: XClaimRewardSmokeCompletionInput,
): XClaimRewardSmokeCompletionReport => {
  const dispatchOutcome = assertXClaimRewardSmokeDispatchOutcome(input.summary)
  const transitionProposal = buildXClaimRewardSmokeTransitionRequest(
    input.reward,
  )

  const blockingReasonRefs = Array.from(
    new Set([
      ...(dispatchOutcome.ok ? [] : [DispatchRunNotCleanReasonRef]),
      ...dispatchOutcome.blockingReasonRefs,
      ...(transitionProposal.ready ? [] : [SettledRowNotReadyReasonRef]),
      ...transitionProposal.blockingReasonRefs,
    ]),
  )

  const ready = dispatchOutcome.ok && transitionProposal.ready

  return {
    blockingReasonRefs,
    dispatchOutcome,
    ready,
    transitionProposal,
    transitionRequest: ready ? transitionProposal.transitionRequest : null,
  }
}
