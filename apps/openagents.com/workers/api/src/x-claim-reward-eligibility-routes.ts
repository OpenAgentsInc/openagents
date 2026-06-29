// Public-safe x_claim_reward eligibility read path (issue #4754,
// instance 8 — the limit case — of the projection-staleness epic
// #4751). The eligibility write happens when an owner completes X
// verification (agents.x_claim_reward.v1); before this module nothing
// served it: an eligible owner could not point at any public surface
// that says so, and the live-dispatch smoke could not cite WHO it
// dispatched to.
//
// The projection composes live at read (no stored copy can go stale),
// carries generatedAt plus its declared staleness contract, exposes the
// promise's own four-state lifecycle separation, and redacts every
// identity field to a digest ref so no X handle, owner id, or agent
// user id leaves the ledger.
import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { notFound } from '@openagentsinc/sync-worker'

import type {
  AgentOwnerClaimStore,
  XClaimRewardRecord,
  XClaimRewardState,
} from './agent-owner-claim-routes'
import { sha256Hex } from './agent-registration'
import { methodNotAllowed, noStoreJsonResponse, serverError } from './http/responses'

type HttpResponse = globalThis.Response

export const X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT =
  'projection.x_claim_reward_eligibility.v1'

const LIST_LIMIT = 200

// The promise's four-state separation (agents.x_claim_reward.v1
// safeCopy): eligibility, operator-approved dispatch, treasury
// dispatch, and settlement are separate states. failed/refused are
// terminal exits, not lifecycle stages.
export const X_CLAIM_REWARD_LIFECYCLE = [
  'eligible',
  'operator_approved',
  'dispatched',
  'settled',
] as const

export type XClaimRewardLifecycleStage =
  (typeof X_CLAIM_REWARD_LIFECYCLE)[number]

export const xClaimRewardLifecycleStage = (
  state: XClaimRewardState,
): XClaimRewardLifecycleStage | 'failed' | 'refused' =>
  state === 'dispatch_requested' ? 'operator_approved' : state

// The promise's own non-spendable caveat wording
// (agents.x_claim_reward.v1 authorityBoundary / unsafeCopy).
const NON_SPENDABLE_CAVEAT =
  'Reward eligibility is a promotional campaign state, not Forum tip settlement, accepted-work payout, Treasury authority, or spendable balance. Dispatch requires the operator admin gate, and only a settled state with receipts proves payment.'

const stalenessContract = {
  composition: 'live_at_read',
  maxStalenessSeconds: 0,
  rebuildsOn: ['x_claim_reward_state_transition'],
} as const

const redactedRef = async (prefix: string, value: string): Promise<string> =>
  `${prefix}.sha256.${(await sha256Hex(value)).slice(0, 16)}`

export type XClaimRewardEligibilityProjection = Readonly<{
  agentRef: string | null
  amountSats: number
  createdAt: string
  evidenceRefCount: number
  lifecycle: typeof X_CLAIM_REWARD_LIFECYCLE
  lifecycleStage: XClaimRewardLifecycleStage | 'failed' | 'refused'
  nonSpendableCaveat: string
  ownerRef: string
  receiptRef: string
  rewardId: string
  state: XClaimRewardState
  stateReasonRef: string | null
  treasuryPaymentAttached: boolean
  updatedAt: string
  xAccountRef: string
}>

export const xClaimRewardEligibilityProjection = async (
  reward: XClaimRewardRecord,
): Promise<XClaimRewardEligibilityProjection> => ({
  agentRef:
    reward.agentUserId === null
      ? null
      : await redactedRef('agent', reward.agentUserId),
  amountSats: reward.amountSats,
  createdAt: reward.createdAt,
  evidenceRefCount: reward.evidenceRefs.length,
  lifecycle: X_CLAIM_REWARD_LIFECYCLE,
  lifecycleStage: xClaimRewardLifecycleStage(reward.state),
  nonSpendableCaveat: NON_SPENDABLE_CAVEAT,
  ownerRef: await redactedRef('owner', reward.ownerUserId),
  receiptRef: reward.receiptRef,
  rewardId: reward.id,
  state: reward.state,
  stateReasonRef: reward.stateReasonRef,
  treasuryPaymentAttached: reward.treasuryPaymentId !== null,
  updatedAt: reward.updatedAt,
  xAccountRef: await redactedRef('x_account', reward.xAccountRef),
})

const projectionLeaksPrivateMaterial = (value: unknown): boolean =>
  containsProviderSecretMaterial(JSON.stringify(value))

const safeProjectionResponse = (value: unknown): HttpResponse =>
  projectionLeaksPrivateMaterial(value)
    ? serverError()
    : noStoreJsonResponse(value)

type XClaimRewardEligibilityStore = Pick<
  AgentOwnerClaimStore,
  | 'listXClaimRewards'
  | 'readXClaimRewardById'
  | 'readXClaimRewardByReceiptRef'
>

export type XClaimRewardEligibilityRouteDependencies = Readonly<{
  nowIso: () => string
  store: XClaimRewardEligibilityStore
}>

export const xClaimRewardEligibilityListResponse = async (
  dependencies: XClaimRewardEligibilityRouteDependencies,
  request: Request,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const rewards = await dependencies.store.listXClaimRewards(LIST_LIMIT)
  const projections = await Promise.all(
    rewards.map(xClaimRewardEligibilityProjection),
  )
  const counts: Record<string, number> = {
    dispatched: 0,
    eligible: 0,
    failed: 0,
    operator_approved: 0,
    refused: 0,
    settled: 0,
  }
  for (const projection of projections) {
    counts[projection.lifecycleStage] =
      (counts[projection.lifecycleStage] ?? 0) + 1
  }

  return safeProjectionResponse({
    contractVersion: X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT,
    counts,
    generatedAt: dependencies.nowIso(),
    lifecycle: X_CLAIM_REWARD_LIFECYCLE,
    nonSpendableCaveat: NON_SPENDABLE_CAVEAT,
    rewards: projections,
    staleness: stalenessContract,
  })
}

// A reward resolves by its id or by its public receipt ref — both are
// returned to the owner at verification time, so the eligible party can
// always cite a ref this surface resolves.
export const xClaimRewardEligibilityStatusResponse = async (
  dependencies: XClaimRewardEligibilityRouteDependencies,
  request: Request,
  rewardRef: string,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const reward =
    (await dependencies.store.readXClaimRewardById(rewardRef)) ??
    (await dependencies.store.readXClaimRewardByReceiptRef(rewardRef))

  if (reward === undefined) {
    return notFound()
  }

  return safeProjectionResponse({
    contractVersion: X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT,
    generatedAt: dependencies.nowIso(),
    reward: await xClaimRewardEligibilityProjection(reward),
    staleness: stalenessContract,
  })
}
