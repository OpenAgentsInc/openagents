import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import type {
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TreasuryPaymentAuthorityAdapter,
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityPayoutPreview,
} from './treasury-payment-authority'

export type SparkTreasuryPayoutDestinationResolverInput = Readonly<{
  attempt: NexusTreasuryPayoutAttemptRecord
  intent: NexusTreasuryPayoutIntentRecord
}>

export type SparkTreasuryPayoutDestinationResolver = (
  input: SparkTreasuryPayoutDestinationResolverInput,
) => Effect.Effect<string, TreasuryPaymentAuthorityError>

export type SparkTreasuryPayoutAdapterConfig = Readonly<{
  fetchTreasury?: ContainerPathFetch | undefined
  providerRef?: string | undefined
  resolveDestination: SparkTreasuryPayoutDestinationResolver
}>

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const unsafeRefPattern =
  /(@|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mnemonic|payment[_-]?(hash|preimage)=|preimage|raw[_-]?(destination|invoice|payment|wallet)|secret|spark[_-]?(address|invoice|request)|wallet[_-]?(config|mnemonic|secret|state))/i
const controlCharacterPattern = /[\u0000-\u001f\u007f]/

const stableRef = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:/-]+/g, '_').slice(0, 160)

const ensureStableRef = (
  label: string,
  value: string,
): TreasuryPaymentAuthorityError | undefined =>
  !stableRefPattern.test(value) || unsafeRefPattern.test(value)
    ? new TreasuryPaymentAuthorityError({
        message: `Spark treasury ${label} must be a stable redacted reference.`,
        reason: 'adapter_unavailable',
      })
    : undefined

const amountToSats = (
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
        message:
          'Spark treasury payout requires a positive whole-bitcoin-sat amount.',
        reason: 'malformed_payout_amount',
      }),
    )
  }

  return Effect.succeed(intent.amount.amountMinorUnits / 1_000)
}

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
        message: 'Spark treasury payout destination is invalid.',
        reason: 'malformed_payout_target',
      }),
    )
  }

  return Effect.succeed(trimmed)
}

const classifySparkTreasuryError = (
  error: unknown,
  operation: string,
): TreasuryPaymentAuthorityError => {
  if (error instanceof TreasuryPaymentAuthorityError) {
    return error
  }

  const message = error instanceof Error ? error.message : String(error)
  const normalized = message.toLowerCase()

  return new TreasuryPaymentAuthorityError({
    message: `${operation} failed: ${
      normalized.includes('insufficient')
        ? 'spark_treasury_insufficient_spendable_balance'
        : normalized.includes('unconfigured')
          ? 'spark_treasury_unconfigured'
          : 'spark_treasury_error'
    }`,
    reason: 'adapter_unavailable',
  })
}

const publicProjectionJson = (
  state: string,
  details: Readonly<Record<string, number | string | null>> = {},
): string =>
  JSON.stringify({
    adapter: 'spark_treasury',
    commandBoundary: 'openagents_treasury_container_spark_sdk',
    moneyMovement: 'real_bitcoin',
    rawMaterialStored: false,
    ...details,
    state,
  })

const paymentRefFromPayload = (
  payload: Record<string, unknown>,
  fallback: string,
): string =>
  typeof payload.paymentRef === 'string' &&
  stableRefPattern.test(payload.paymentRef) &&
  !unsafeRefPattern.test(payload.paymentRef)
    ? payload.paymentRef
    : `payment.redacted.spark_treasury.${stableRef(fallback)}`

export const makeSparkTreasuryPayoutAdapter = (
  config: SparkTreasuryPayoutAdapterConfig,
): TreasuryPaymentAuthorityAdapter => ({
  adapterKind: 'spark_treasury',
  dispatch: input =>
    Effect.gen(function* () {
      const refError = ensureStableRef(
        'attempt idempotency key',
        input.attempt.idempotencyKeyHash,
      )

      if (refError !== undefined) {
        return yield* refError
      }

      if (config.fetchTreasury === undefined) {
        return yield* new TreasuryPaymentAuthorityError({
          message: 'Spark treasury container is not configured.',
          reason: 'adapter_unavailable',
        })
      }

      const amountSats = yield* amountToSats(input.intent)
      const destination = yield* config
        .resolveDestination(input)
        .pipe(Effect.flatMap(ensureDestination))
      const payload = yield* Effect.tryPromise({
        catch: error =>
          classifySparkTreasuryError(error, 'Spark treasury dispatch'),
        try: async () => {
          const response = await config.fetchTreasury!('/spark/pay', {
            body: JSON.stringify({
              amountSat: amountSats,
              destination,
              idempotencyKey: input.attempt.idempotencyKeyHash,
            }),
            method: 'POST',
          })
          const parsed = (await response.json().catch(() => ({}))) as Record<
            string,
            unknown
          >

          if (!response.ok || parsed.status !== 'succeeded') {
            throw new TreasuryPaymentAuthorityError({
              message:
                typeof parsed.error === 'string'
                  ? parsed.error
                  : 'spark_treasury_pay_failed',
              reason: 'adapter_unavailable',
            })
          }

          return parsed
        },
      })
      const paymentRef = paymentRefFromPayload(
        payload,
        input.attempt.idempotencyKeyHash,
      )
      const method =
        typeof payload.method === 'string' ? stableRef(payload.method) : null

      return {
        ...input.attempt,
        adapterKind: 'spark_treasury',
        adapterAttemptRef: `adapter_attempt.spark_treasury.${stableRef(input.attempt.idempotencyKeyHash)}`,
        metadataRefs: [
          ...new Set([
            ...input.attempt.metadataRefs,
            'metadata.nexus.spark_treasury.dispatch.accepted',
            ...(method === null
              ? []
              : [`metadata.nexus.spark_treasury.method.${method}`]),
          ]),
        ],
        publicProjectionJson: publicProjectionJson('dispatch_reported', {
          method,
        }),
        redactedPaymentRef: paymentRef,
        status: 'dispatched',
      } satisfies NexusTreasuryPayoutAttemptRecord
    }),
  preview: input =>
    Effect.gen(function* () {
      if (config.fetchTreasury === undefined) {
        return yield* new TreasuryPaymentAuthorityError({
          message: 'Spark treasury container is not configured.',
          reason: 'adapter_unavailable',
        })
      }
      yield* amountToSats(input.intent)

      return {
        adapterKind: 'spark_treasury',
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

      const providerRef = config.providerRef ?? input.event.providerRef

      return {
        ...input.event,
        adapterKind: 'spark_treasury',
        metadataRefs: [
          ...new Set([
            ...input.event.metadataRefs,
            'metadata.nexus.spark_treasury.reconciliation.matched',
          ]),
        ],
        providerRef,
        publicProjectionJson: publicProjectionJson('reconciliation_matched'),
        resultRef: input.event.externalEventRef,
        status: 'matched',
      } satisfies NexusTreasuryPayoutReconciliationEventRecord
    }),
})
