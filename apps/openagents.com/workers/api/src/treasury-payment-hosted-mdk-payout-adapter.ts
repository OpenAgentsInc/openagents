import { ORPCError, createORPCClient } from '@orpc/client'
import { RPCLink } from '@orpc/client/fetch'
import type {
  ProgrammaticPayoutResult,
  WaitForPayoutResultOutput,
} from '@moneydevkit/api-contract'
import { Effect } from 'effect'

import type {
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityAdapter,
  type TreasuryPaymentAuthorityPayoutPreview,
} from './treasury-payment-authority'

export type HostedMdkPayoutDestinationResolverInput = Readonly<{
  attempt: NexusTreasuryPayoutAttemptRecord
  intent: NexusTreasuryPayoutIntentRecord
}>

export type HostedMdkPayoutDestinationResolver = (
  input: HostedMdkPayoutDestinationResolverInput,
) => Effect.Effect<string, TreasuryPaymentAuthorityError>

export type HostedMdkPayoutAdapterConfig = Readonly<{
  accessToken?: string | undefined
  baseUrl?: string | undefined
  fetch?: typeof fetch
  providerRef?: string | undefined
  resolveDestination: HostedMdkPayoutDestinationResolver
  waitTimeoutMs?: number | undefined
}>

type HostedMdkRpcClient = Readonly<{
  checkout: Readonly<{
    programmaticPayout: (input: Readonly<{
      amountSats: number
      destination: string
      idempotencyKey: string
    }>) => Promise<ProgrammaticPayoutResult>
    waitForPayoutResult: (input: Readonly<{
      idempotencyKey?: string | undefined
      paymentId?: string | undefined
      timeoutMs?: number | undefined
    }>) => Promise<WaitForPayoutResultOutput>
  }>
}>

const textEncoder = new TextEncoder()
const defaultBaseUrl = 'https://moneydevkit.com/rpc'
const defaultWaitTimeoutMs = 15_000
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const unsafeRefPattern =
  /(@|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|raw[_-]?(destination|invoice|payment|wallet)|secret|wallet[_-]?(config|mnemonic|secret|state))/i
const controlCharacterPattern = /[\u0000-\u001f\u007f]/

const stableRef = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:/-]+/g, '_').slice(0, 160)

const ensureStableRef = (
  label: string,
  value: string,
): TreasuryPaymentAuthorityError | undefined =>
  !stableRefPattern.test(value) || unsafeRefPattern.test(value)
    ? new TreasuryPaymentAuthorityError({
        message: `Hosted MDK ${label} must be a stable redacted reference.`,
        reason: 'adapter_unavailable',
      })
    : undefined

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', textEncoder.encode(value))

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const redactedMaterialRef = async (
  prefix: string,
  rawMaterial: string,
): Promise<string> =>
  `${prefix}.${(await sha256Hex(rawMaterial)).slice(0, 32)}`

const amountToMdkSats = (
  intent: NexusTreasuryPayoutIntentRecord,
): Effect.Effect<number, TreasuryPaymentAuthorityError> => {
  if (
    intent.amount.asset !== 'bitcoin' ||
    intent.amount.denomination !== 'bitcoin_millisatoshi' ||
    intent.amount.amountMinorUnits <= 0 ||
    intent.amount.amountMinorUnits % 1_000 !== 0
  ) {
    return Effect.fail(
      new TreasuryPaymentAuthorityError({
        message: 'Hosted MDK payout requires a positive whole-bitcoin-sat amount.',
        reason: 'malformed_payout_amount',
      }),
    )
  }

  return Effect.succeed(intent.amount.amountMinorUnits / 1_000)
}

const configuredAccessToken = (
  config: HostedMdkPayoutAdapterConfig,
): Effect.Effect<string, TreasuryPaymentAuthorityError> =>
  config.accessToken === undefined || config.accessToken.trim() === ''
    ? Effect.fail(
        new TreasuryPaymentAuthorityError({
          message: 'Hosted MDK payout access token is not configured.',
          reason: 'adapter_unavailable',
        }),
      )
    : Effect.succeed(config.accessToken)

const ensureDestination = (
  destination: string,
): Effect.Effect<string, TreasuryPaymentAuthorityError> => {
  const trimmed = destination.trim()

  if (
    trimmed === '' ||
    trimmed.length > 4096 ||
    controlCharacterPattern.test(trimmed)
  ) {
    return Effect.fail(
      new TreasuryPaymentAuthorityError({
        message: 'Hosted MDK payout destination is invalid.',
        reason: 'malformed_payout_target',
      }),
    )
  }

  return Effect.succeed(trimmed)
}

const clientForConfig = (
  config: HostedMdkPayoutAdapterConfig,
  accessToken: string,
): HostedMdkRpcClient => {
  const link = new RPCLink({
    ...(config.fetch === undefined ? {} : { fetch: config.fetch }),
    headers: () => ({ 'x-api-key': accessToken }),
    url: config.baseUrl ?? defaultBaseUrl,
  })

  return createORPCClient(link) as unknown as HostedMdkRpcClient
}

const classifyHostedMdkError = (
  error: unknown,
  operation: string,
): TreasuryPaymentAuthorityError => {
  if (error instanceof TreasuryPaymentAuthorityError) {
    return error
  }

  if (error instanceof ORPCError) {
    const data = error.data as { code?: string } | undefined
    const code = data?.code ?? error.code
    const normalized = `${code} ${error.message}`.toLowerCase()
    const detail = normalized.includes('insufficient')
      ? 'hosted_mdk_insufficient_liquidity'
      : normalized.includes('disabled')
        ? 'hosted_mdk_programmatic_payouts_disabled'
        : normalized.includes('limit')
          ? 'hosted_mdk_limit_exceeded'
          : 'hosted_mdk_rpc_error'

    return new TreasuryPaymentAuthorityError({
      message: `${operation} failed: ${detail}`,
      reason: 'adapter_unavailable',
    })
  }

  const message = error instanceof Error ? error.message : String(error)

  return new TreasuryPaymentAuthorityError({
    message: `${operation} failed: ${message}`,
    reason: 'adapter_unavailable',
  })
}

const publicProjectionJson = (
  state: string,
): string =>
  JSON.stringify({
    adapter: 'hosted_mdk',
    commandBoundary: 'moneydevkit_hosted_rpc',
    moneyMovement: 'real_bitcoin',
    rawMaterialStored: false,
    state,
  })

const reconciliationStatusByHostedMdkStatus:
  Readonly<
    Record<
      WaitForPayoutResultOutput['status'],
      NexusTreasuryPayoutReconciliationEventRecord['status']
    >
  > = {
    FAILED: 'rejected',
    REQUESTED: 'observed',
    SUCCESS: 'matched',
  }

const paymentMaterialForDispatch = (
  result: ProgrammaticPayoutResult,
): string => result.paymentHash ?? result.paymentId

const paymentMaterialForReconciliation = (
  result: WaitForPayoutResultOutput,
  fallback: string,
): string => result.paymentHash ?? fallback

export const makeHostedMdkPayoutAdapter = (
  config: HostedMdkPayoutAdapterConfig,
): TreasuryPaymentAuthorityAdapter => ({
  adapterKind: 'hosted_mdk',
  dispatch: input =>
    Effect.gen(function* () {
      const refError = ensureStableRef(
        'attempt idempotency key',
        input.attempt.idempotencyKeyHash,
      )

      if (refError !== undefined) {
        return yield* refError
      }

      const accessToken = yield* configuredAccessToken(config)
      const amountSats = yield* amountToMdkSats(input.intent)
      const destination = yield* config.resolveDestination(input).pipe(
        Effect.flatMap(ensureDestination),
      )
      const client = clientForConfig(config, accessToken)
      const result = yield* Effect.tryPromise({
        catch: error => classifyHostedMdkError(error, 'Hosted MDK dispatch'),
        try: () =>
          client.checkout.programmaticPayout({
            amountSats,
            destination,
            idempotencyKey: input.attempt.idempotencyKeyHash,
          }),
      })
      const paymentRef = yield* Effect.promise(() =>
        redactedMaterialRef(
          'payment.redacted.hosted_mdk',
          paymentMaterialForDispatch(result),
        ),
      )

      return {
        ...input.attempt,
        adapterKind: 'hosted_mdk',
        adapterAttemptRef:
          `adapter_attempt.hosted_mdk.${stableRef(input.attempt.idempotencyKeyHash)}`,
        metadataRefs: [
          ...new Set([
            ...input.attempt.metadataRefs,
            'metadata.nexus.hosted_mdk.dispatch.accepted',
          ]),
        ],
        publicProjectionJson: publicProjectionJson('dispatch_reported'),
        redactedPaymentRef: paymentRef,
        status: 'dispatched',
      } satisfies NexusTreasuryPayoutAttemptRecord
    }),
  preview: input =>
    Effect.gen(function* () {
      yield* configuredAccessToken(config)
      yield* amountToMdkSats(input.intent)

      return {
        adapterKind: 'hosted_mdk',
        amount: input.intent.amount,
        dispatchAllowed: true,
        payoutIntentRef: input.intent.payoutIntentRef,
        payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
        policySnapshotRef: input.intent.policySnapshotRef,
        spendCap: input.intent.spendCap,
      } satisfies TreasuryPaymentAuthorityPayoutPreview
    }),
  reconcile: input =>
    Effect.gen(function* () {
      const refError = ensureStableRef('event ref', input.event.eventRef)

      if (refError !== undefined) {
        return yield* refError
      }

      const accessToken = yield* configuredAccessToken(config)
      const client = clientForConfig(config, accessToken)
      const result = yield* Effect.tryPromise({
        catch: error => classifyHostedMdkError(error, 'Hosted MDK reconcile'),
        try: () =>
          client.checkout.waitForPayoutResult({
            idempotencyKey: input.event.idempotencyKeyHash,
            timeoutMs: config.waitTimeoutMs ?? defaultWaitTimeoutMs,
          }),
      })
      const resultMaterial = paymentMaterialForReconciliation(
        result,
        input.event.externalEventRef,
      )
      const resultRef = result.status === 'REQUESTED'
        ? `result.hosted_mdk.requested.${stableRef(input.event.idempotencyKeyHash)}`
        : yield* Effect.promise(() =>
            redactedMaterialRef('payment.redacted.hosted_mdk', resultMaterial),
          )
      const providerRef = config.providerRef ?? input.event.providerRef

      return {
        ...input.event,
        adapterKind: 'hosted_mdk',
        metadataRefs: [
          ...new Set([
            ...input.event.metadataRefs,
            `metadata.nexus.hosted_mdk.reconciliation.${result.status.toLowerCase()}`,
          ]),
        ],
        providerRef,
        publicProjectionJson: publicProjectionJson(
          `reconciliation_${result.status.toLowerCase()}`,
        ),
        resultRef,
        status: reconciliationStatusByHostedMdkStatus[result.status],
      } satisfies NexusTreasuryPayoutReconciliationEventRecord
    }),
})
