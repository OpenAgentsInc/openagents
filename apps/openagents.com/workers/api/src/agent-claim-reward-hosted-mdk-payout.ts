import { Effect, Schema as S } from 'effect'

import {
  AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
  type AgentClaimRewardLedgerRecord,
  projectAgentClaimRewardPublicReceipt,
} from './agent-claim-reward-ledger'
import {
  type MdkPayoutModeGateInput,
  projectMdkPayoutModeGate,
} from './mdk-payout-mode-gate'

export const AgentClaimRewardHostedMdkPayoutErrorReason = S.Literals([
  'reward_not_payable',
  'payout_mode_blocked',
  'destination_unavailable',
  'provider_unavailable',
  'settlement_unmatched',
])
export type AgentClaimRewardHostedMdkPayoutErrorReason =
  typeof AgentClaimRewardHostedMdkPayoutErrorReason.Type

export class AgentClaimRewardHostedMdkPayoutError extends S.TaggedErrorClass<AgentClaimRewardHostedMdkPayoutError>()(
  'AgentClaimRewardHostedMdkPayoutError',
  {
    message: S.String,
    reason: AgentClaimRewardHostedMdkPayoutErrorReason,
  },
) {}

export type AgentClaimRewardHostedMdkProgrammaticPayoutResult = Readonly<{
  paymentHash?: string | undefined
  paymentId: string
  status: 'REQUESTED' | 'SUCCESS' | 'FAILED'
}>

export type AgentClaimRewardHostedMdkWaitResult = Readonly<{
  paymentHash?: string | undefined
  paymentId: string
  status: 'REQUESTED' | 'SUCCESS' | 'FAILED'
}>

export type AgentClaimRewardHostedMdkClient = Readonly<{
  programmaticPayout: (
    input: Readonly<{
      amountSats: number
      destination: string
      idempotencyKey: string
    }>,
  ) => Promise<AgentClaimRewardHostedMdkProgrammaticPayoutResult>
  waitForPayoutResult: (
    input: Readonly<{
      idempotencyKey: string
      paymentId: string
      timeoutMs: number
    }>,
  ) => Promise<AgentClaimRewardHostedMdkWaitResult>
}>

export type AgentClaimRewardHostedMdkPayoutConfig = Readonly<{
  client: AgentClaimRewardHostedMdkClient
  gate: MdkPayoutModeGateInput
  resolveDestination: (
    record: AgentClaimRewardLedgerRecord,
  ) => Effect.Effect<string, AgentClaimRewardHostedMdkPayoutError>
  waitTimeoutMs?: number | undefined
}>

const defaultWaitTimeoutMs = 15_000
const rawDestinationUnsafePattern = /[\u0000-\u001f\u007f]/

const ensurePayableReward = (
  record: AgentClaimRewardLedgerRecord,
): Effect.Effect<
  AgentClaimRewardLedgerRecord,
  AgentClaimRewardHostedMdkPayoutError
> => {
  if (
    record.amountSats !== AGENT_CLAIM_X_REWARD_AMOUNT_SATS ||
    !['approved', 'payout_intent_created'].includes(record.state)
  ) {
    return Effect.fail(
      new AgentClaimRewardHostedMdkPayoutError({
        message:
          'Agent claim reward must be approved before hosted MDK payout dispatch.',
        reason: 'reward_not_payable',
      }),
    )
  }

  return Effect.succeed(record)
}

const ensureHostedMdkGate = (
  input: MdkPayoutModeGateInput,
): Effect.Effect<void, AgentClaimRewardHostedMdkPayoutError> => {
  const gate = projectMdkPayoutModeGate(input)

  if (
    gate.activeMode !== 'hosted_mdk_direct_payout' ||
    !gate.livePayoutClaimAllowed
  ) {
    return Effect.fail(
      new AgentClaimRewardHostedMdkPayoutError({
        message: `Hosted MDK claim reward payout is blocked: ${gate.blockerRefs.join(', ')}`,
        reason: 'payout_mode_blocked',
      }),
    )
  }

  return Effect.void
}

const ensureDestination = (
  destination: string,
): Effect.Effect<string, AgentClaimRewardHostedMdkPayoutError> => {
  const trimmed = destination.trim()

  if (
    trimmed === '' ||
    trimmed.length > 4096 ||
    rawDestinationUnsafePattern.test(trimmed)
  ) {
    return Effect.fail(
      new AgentClaimRewardHostedMdkPayoutError({
        message: 'Hosted MDK claim reward destination is unavailable.',
        reason: 'destination_unavailable',
      }),
    )
  }

  return Effect.succeed(trimmed)
}

const redactedProviderRef = (value: string): string =>
  `hosted_mdk:${value.replaceAll(/[^A-Za-z0-9_.:-]+/g, '_').slice(0, 120)}`

export const dispatchAgentClaimRewardHostedMdkPayout = (
  record: AgentClaimRewardLedgerRecord,
  config: AgentClaimRewardHostedMdkPayoutConfig,
): Effect.Effect<
  AgentClaimRewardLedgerRecord,
  AgentClaimRewardHostedMdkPayoutError
> =>
  Effect.gen(function* () {
    yield* ensurePayableReward(record)
    yield* ensureHostedMdkGate(config.gate)
    const destination = yield* config
      .resolveDestination(record)
      .pipe(Effect.flatMap(ensureDestination))
    const result = yield* Effect.tryPromise({
      catch: error =>
        new AgentClaimRewardHostedMdkPayoutError({
          message: error instanceof Error ? error.message : String(error),
          reason: 'provider_unavailable',
        }),
      try: () =>
        config.client.programmaticPayout({
          amountSats: AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
          destination,
          idempotencyKey: record.idempotencyKey,
        }),
    })

    return {
      ...record,
      dispatchAttemptRef: redactedProviderRef(result.paymentId),
      payoutIntentRef:
        record.payoutIntentRef ?? `claim_reward_payout_intent:${record.id}`,
      state: result.status === 'FAILED' ? 'rejected' : 'dispatched',
    }
  })

export const settleAgentClaimRewardHostedMdkPayout = (
  record: AgentClaimRewardLedgerRecord,
  config: AgentClaimRewardHostedMdkPayoutConfig,
): Effect.Effect<
  AgentClaimRewardLedgerRecord,
  AgentClaimRewardHostedMdkPayoutError
> =>
  Effect.gen(function* () {
    if (record.state !== 'dispatched' || record.dispatchAttemptRef === null) {
      return yield* new AgentClaimRewardHostedMdkPayoutError({
        message: 'Agent claim reward has not been dispatched.',
        reason: 'reward_not_payable',
      })
    }

    const paymentId = record.dispatchAttemptRef

    yield* ensureHostedMdkGate(config.gate)
    const result = yield* Effect.tryPromise({
      catch: error =>
        new AgentClaimRewardHostedMdkPayoutError({
          message: error instanceof Error ? error.message : String(error),
          reason: 'provider_unavailable',
        }),
      try: () =>
        config.client.waitForPayoutResult({
          idempotencyKey: record.idempotencyKey,
          paymentId,
          timeoutMs: config.waitTimeoutMs ?? defaultWaitTimeoutMs,
        }),
    })

    if (result.status !== 'SUCCESS') {
      return yield* new AgentClaimRewardHostedMdkPayoutError({
        message: `Hosted MDK claim reward settlement is ${result.status}.`,
        reason: 'settlement_unmatched',
      })
    }

    return {
      ...record,
      settlementRef: redactedProviderRef(
        result.paymentHash ?? result.paymentId,
      ),
      state: 'settled',
    }
  })

export const projectHostedMdkClaimRewardReceipt = (
  record: AgentClaimRewardLedgerRecord,
) => ({
  ...projectAgentClaimRewardPublicReceipt(record),
  payoutMode: 'hosted_mdk_direct_payout' as const,
})
