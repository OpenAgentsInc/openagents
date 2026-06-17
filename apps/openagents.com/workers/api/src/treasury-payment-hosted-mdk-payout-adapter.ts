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

export type HostedMdkRpcClient = Readonly<{
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

export type HostedMdkPayoutAdapterConfig = Readonly<{
  accessToken?: string | undefined
  baseUrl?: string | undefined
  client?: HostedMdkRpcClient | undefined
  fetch?: typeof fetch
  maxSinglePayoutSats?: number | undefined
  providerRef?: string | undefined
  resolveDestination: HostedMdkPayoutDestinationResolver
  waitTimeoutMs?: number | undefined
}>

const textEncoder = new TextEncoder()
const defaultBaseUrl = 'https://moneydevkit.com/rpc'
const defaultMaxSinglePayoutSats = 25_000
const defaultWaitTimeoutMs = 15_000
const hostedMdkChunkIdempotencyMetadataPrefix =
  'metadata.nexus.hosted_mdk.chunk_idempotency.'
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const unsafeRefPattern =
  /(@|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|raw[_-]?(destination|invoice|payment|wallet)|secret|wallet[_-]?(config|mnemonic|secret|state))/i
const controlCharacterPattern = /[\u0000-\u001f\u007f]/
const reusableDestinationPattern = /(^lno1|^lnurl|\S+@\S+)/i

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

type HostedMdkPayoutChunk = Readonly<{
  amountSats: number
  idempotencyKey: string
  index: number
  total: number
}>

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

const maxSinglePayoutSats = (config: HostedMdkPayoutAdapterConfig): number => {
  const configured = config.maxSinglePayoutSats

  return typeof configured === 'number' &&
    Number.isInteger(configured) &&
    configured > 0
    ? configured
    : defaultMaxSinglePayoutSats
}

const destinationCanBeChunked = (destination: string): boolean =>
  reusableDestinationPattern.test(destination.trim())

const chunkAmounts = (
  amountSats: number,
  maxChunkSats: number,
): ReadonlyArray<number> =>
  amountSats <= maxChunkSats
    ? [amountSats]
    : [maxChunkSats, ...chunkAmounts(amountSats - maxChunkSats, maxChunkSats)]

const chunkIdempotencyKey = async (
  baseIdempotencyKeyHash: string,
  chunkIndex: number,
  chunkTotal: number,
  amountSats: number,
): Promise<string> =>
  `hash.hosted_mdk_payout_chunk.${(
    await sha256Hex(
      [
        'hosted-mdk-payout-chunk',
        baseIdempotencyKeyHash,
        String(chunkIndex),
        String(chunkTotal),
        String(amountSats),
      ].join(':'),
    )
  ).slice(0, 64)}`

const payoutChunks = (
  config: HostedMdkPayoutAdapterConfig,
  amountSats: number,
  destination: string,
  baseIdempotencyKeyHash: string,
): Effect.Effect<ReadonlyArray<HostedMdkPayoutChunk>> =>
  Effect.promise(async () => {
    const maxChunkSats = maxSinglePayoutSats(config)
    const amounts =
      amountSats > maxChunkSats && destinationCanBeChunked(destination)
        ? chunkAmounts(amountSats, maxChunkSats)
        : [amountSats]
    const total = amounts.length
    const keys = await Promise.all(
      amounts.map((chunkAmountSats, index) =>
        total === 1
          ? Promise.resolve(baseIdempotencyKeyHash)
          : chunkIdempotencyKey(
              baseIdempotencyKeyHash,
              index + 1,
              total,
              chunkAmountSats,
            ),
      ),
    )

    return amounts.map((chunkAmountSats, index) => ({
      amountSats: chunkAmountSats,
      idempotencyKey: keys[index]!,
      index: index + 1,
      total,
    }))
  })

const chunkMetadataRefs = (
  chunks: ReadonlyArray<HostedMdkPayoutChunk>,
  maxChunkSats: number,
): ReadonlyArray<string> =>
  chunks.length <= 1
    ? []
    : [
        'metadata.nexus.hosted_mdk.dispatch.chunked',
        `metadata.nexus.hosted_mdk.chunk_count.${chunks.length}`,
        `metadata.nexus.hosted_mdk.max_single_payout_sats.${maxChunkSats}`,
        ...chunks.flatMap(chunk => [
          `${hostedMdkChunkIdempotencyMetadataPrefix}${chunk.idempotencyKey}`,
          `metadata.nexus.hosted_mdk.chunk_amount_sats.${chunk.index}.${chunk.amountSats}`,
        ]),
      ]

const chunkIdempotencyKeysFromMetadata = (
  metadataRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  metadataRefs
    .filter(ref => ref.startsWith(hostedMdkChunkIdempotencyMetadataPrefix))
    .map(ref => ref.slice(hostedMdkChunkIdempotencyMetadataPrefix.length))
    .filter(ref => stableRefPattern.test(ref) && !unsafeRefPattern.test(ref))

const clientForConfig = (
  config: HostedMdkPayoutAdapterConfig,
  accessToken: string,
): HostedMdkRpcClient => {
  if (config.client !== undefined) {
    return config.client
  }

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
  details: Readonly<Record<string, number>> = {},
): string =>
  JSON.stringify({
    adapter: 'hosted_mdk',
    commandBoundary: 'moneydevkit_hosted_rpc',
    ...details,
    moneyMovement: 'real_bitcoin',
    rawMaterialStored: false,
    state,
  })

const paymentMaterialForDispatch = (
  result: ProgrammaticPayoutResult,
): string => result.paymentHash ?? result.paymentId

const paymentMaterialForDispatchResults = (
  results: ReadonlyArray<ProgrammaticPayoutResult>,
): string => results.map(paymentMaterialForDispatch).join(':')

const paymentMaterialForReconciliation = (
  result: WaitForPayoutResultOutput,
  fallback: string,
): string => result.paymentHash ?? fallback

const reconciliationStatusForResults = (
  results: ReadonlyArray<WaitForPayoutResultOutput>,
): NexusTreasuryPayoutReconciliationEventRecord['status'] =>
  results.some(result => result.status === 'FAILED')
    ? 'rejected'
    : results.some(result => result.status === 'REQUESTED')
      ? 'observed'
      : 'matched'

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
      const chunks = yield* payoutChunks(
        config,
        amountSats,
        destination,
        input.attempt.idempotencyKeyHash,
      )
      const results = yield* Effect.forEach(
        chunks,
        chunk =>
          Effect.tryPromise({
            catch: error =>
              classifyHostedMdkError(error, 'Hosted MDK dispatch'),
            try: () =>
              client.checkout.programmaticPayout({
                amountSats: chunk.amountSats,
                destination,
                idempotencyKey: chunk.idempotencyKey,
              }),
          }),
        { concurrency: 1 },
      )
      const paymentRef = yield* Effect.promise(() =>
        redactedMaterialRef(
          'payment.redacted.hosted_mdk',
          paymentMaterialForDispatchResults(results),
        ),
      )
      const maxChunkSats = maxSinglePayoutSats(config)

      return {
        ...input.attempt,
        adapterKind: 'hosted_mdk',
        adapterAttemptRef:
          `adapter_attempt.hosted_mdk.${stableRef(input.attempt.idempotencyKeyHash)}`,
        metadataRefs: [
          ...new Set([
            ...input.attempt.metadataRefs,
            'metadata.nexus.hosted_mdk.dispatch.accepted',
            ...chunkMetadataRefs(chunks, maxChunkSats),
          ]),
        ],
        publicProjectionJson: publicProjectionJson('dispatch_reported', {
          chunkCount: chunks.length,
          maxSinglePayoutSats: maxChunkSats,
        }),
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
      const chunkIdempotencyKeys = chunkIdempotencyKeysFromMetadata(
        input.event.metadataRefs,
      )
      const idempotencyKeys =
        chunkIdempotencyKeys.length === 0
          ? [input.event.idempotencyKeyHash]
          : chunkIdempotencyKeys
      const results = yield* Effect.forEach(
        idempotencyKeys,
        idempotencyKey =>
          Effect.tryPromise({
            catch: error =>
              classifyHostedMdkError(error, 'Hosted MDK reconcile'),
            try: () =>
              client.checkout.waitForPayoutResult({
                idempotencyKey,
                timeoutMs: config.waitTimeoutMs ?? defaultWaitTimeoutMs,
              }),
          }),
        { concurrency: 1 },
      )
      const status = reconciliationStatusForResults(results)
      const resultMaterial = results
        .map((result, index) =>
          paymentMaterialForReconciliation(
            result,
            idempotencyKeys[index] ?? input.event.externalEventRef,
          ),
        )
        .join(':')
      const resultRef = status === 'observed'
        ? `result.hosted_mdk.requested.${stableRef(input.event.idempotencyKeyHash)}`
        : yield* Effect.promise(() =>
            redactedMaterialRef('payment.redacted.hosted_mdk', resultMaterial),
          )
      const providerRef = config.providerRef ?? input.event.providerRef
      const statusRefs = results.map(
        result =>
          `metadata.nexus.hosted_mdk.reconciliation.${result.status.toLowerCase()}`,
      )

      return {
        ...input.event,
        adapterKind: 'hosted_mdk',
        metadataRefs: [
          ...new Set([
            ...input.event.metadataRefs,
            ...statusRefs,
          ]),
        ],
        providerRef,
        publicProjectionJson: publicProjectionJson(
          `reconciliation_${status}`,
          { chunkCount: idempotencyKeys.length },
        ),
        resultRef,
        status,
      } satisfies NexusTreasuryPayoutReconciliationEventRecord
    }),
})
