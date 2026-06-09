import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'
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

export const MdkAgentWalletCommandName = S.Literals([
  'balance',
  'payments',
  'receive',
  'send',
])
export type MdkAgentWalletCommandName = typeof MdkAgentWalletCommandName.Type

export const MdkAgentWalletCommandErrorReason = S.Literals([
  'command_timeout',
  'daemon_unavailable',
  'insufficient_balance',
  'insufficient_outbound_capacity',
  'invalid_json',
  'mnemonic_restore_not_send_ready',
  'payment_failed',
  'reconciliation_mismatch',
  'send_readiness_unknown',
])
export type MdkAgentWalletCommandErrorReason =
  typeof MdkAgentWalletCommandErrorReason.Type

export const MdkAgentWalletCommandArgs = S.Record(
  S.String,
  S.Union([S.Boolean, S.Number, S.Null, S.String]),
)
export type MdkAgentWalletCommandArgs = typeof MdkAgentWalletCommandArgs.Type

export const MdkAgentWalletCommandRequest = S.Struct({
  args: MdkAgentWalletCommandArgs,
  command: MdkAgentWalletCommandName,
  requestRef: S.String,
  timeoutMs: S.Number,
  walletRef: S.String,
})
export type MdkAgentWalletCommandRequest =
  typeof MdkAgentWalletCommandRequest.Type

export const MdkAgentWalletCommandResult = S.Struct({
  durationMs: S.Number,
  exitCode: S.Number,
  stderrDigestRef: S.NullOr(S.String),
  stdout: S.String,
  timedOut: S.Boolean,
})
export type MdkAgentWalletCommandResult =
  typeof MdkAgentWalletCommandResult.Type

export const MdkAgentWalletBalanceOutput = S.Struct({
  balance_sats: S.Number,
})
export type MdkAgentWalletBalanceOutput =
  typeof MdkAgentWalletBalanceOutput.Type

export const MdkAgentWalletReceiveOutput = S.Struct({
  expires_at: S.String,
  invoice: S.String,
  payment_hash: S.String,
})
export type MdkAgentWalletReceiveOutput =
  typeof MdkAgentWalletReceiveOutput.Type

export const MdkAgentWalletSendOutput = S.Struct({
  payment_hash: S.String,
})
export type MdkAgentWalletSendOutput = typeof MdkAgentWalletSendOutput.Type

export const MdkAgentWalletPaymentHistoryEntry = S.Struct({
  amount_sats: S.optionalKey(S.Number),
  amountSats: S.optionalKey(S.Number),
  direction: S.optionalKey(S.String),
  openagents_event_ref: S.optionalKey(S.String),
  payment_hash: S.optionalKey(S.String),
  paymentHash: S.optionalKey(S.String),
  paymentId: S.optionalKey(S.String),
  status: S.String,
})
export type MdkAgentWalletPaymentHistoryEntry =
  typeof MdkAgentWalletPaymentHistoryEntry.Type

export const MdkAgentWalletPaymentsOutput = S.Union([
  S.Array(MdkAgentWalletPaymentHistoryEntry),
  S.Struct({
    payments: S.Array(MdkAgentWalletPaymentHistoryEntry),
  }),
])
export type MdkAgentWalletPaymentsOutput =
  typeof MdkAgentWalletPaymentsOutput.Type

export class MdkAgentWalletCommandError extends S.TaggedErrorClass<MdkAgentWalletCommandError>()(
  'MdkAgentWalletCommandError',
  {
    detailRef: S.String,
    reason: MdkAgentWalletCommandErrorReason,
  },
) {}

export type MdkAgentWalletCommandExecutor = Readonly<{
  run: (
    request: MdkAgentWalletCommandRequest,
  ) => Promise<MdkAgentWalletCommandResult>
}>

export const MdkAgentWalletHomeMode = S.Literals([
  'mnemonic_restore',
  'original_funded_wallet_home',
  'unknown',
])
export type MdkAgentWalletHomeMode = typeof MdkAgentWalletHomeMode.Type

export type MdkAgentWalletAdapterConfig = Readonly<{
  defaultTimeoutMs?: number
  executor: MdkAgentWalletCommandExecutor
  executorRef: string
  walletHomeMode?: MdkAgentWalletHomeMode | undefined
  walletRef: string
}>

export type MdkAgentWalletReadinessRequest = Readonly<{
  minimumSendAmountSats: number
  requireOriginalFundedWalletHome?: boolean | undefined
}>

export type MdkAgentWalletReadinessProjection = Readonly<{
  balanceRef: string
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  executorRef: string
  homeModeRef: string
  receiveReady: boolean
  ready: boolean
  sendReady: boolean
  walletRef: string
}>

export type MdkAgentWalletReceiveInvoiceProjection = Readonly<{
  expiresAt: string
  invoiceRef: string
  paymentHashRef: string
  walletRef: string
}>

const textEncoder = new TextEncoder()
const defaultTimeoutMs = 10_000
const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const unsafeRefPattern =
  /(@|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|raw[_-]?(invoice|payment|wallet)|secret|wallet[_-]?(config|mnemonic|secret|state))/i

const stableRef = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:/-]+/g, '_').slice(0, 160)

const ensureStableRef = (label: string, value: string): void => {
  if (!stableRefPattern.test(value) || unsafeRefPattern.test(value)) {
    throw new MdkAgentWalletCommandError({
      detailRef: `detail.mdk_agent_wallet.${label}.unsafe_ref`,
      reason: 'payment_failed',
    })
  }
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    textEncoder.encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const redactedMaterialRef = async (
  prefix: string,
  rawMaterial: string,
): Promise<string> => `${prefix}.${(await sha256Hex(rawMaterial)).slice(0, 32)}`

const classifyCommandFailure = (
  result: MdkAgentWalletCommandResult,
): MdkAgentWalletCommandErrorReason => {
  const text = `${result.stdout} ${result.stderrDigestRef ?? ''}`.toLowerCase()

  if (result.timedOut || text.includes('timeout')) {
    return 'command_timeout'
  }

  if (text.includes('insufficient')) {
    if (text.includes('outbound') || text.includes('capacity')) {
      return 'insufficient_outbound_capacity'
    }

    return 'insufficient_balance'
  }

  if (text.includes('daemon') || text.includes('unavailable')) {
    return 'daemon_unavailable'
  }

  return 'payment_failed'
}

const commandError = (
  reason: MdkAgentWalletCommandErrorReason,
  detailRef: string,
): MdkAgentWalletCommandError =>
  new MdkAgentWalletCommandError({ detailRef, reason })

const treasuryError = (
  error: MdkAgentWalletCommandError,
): TreasuryPaymentAuthorityError =>
  new TreasuryPaymentAuthorityError({
    message: `MDK agent wallet command failed: ${error.reason}`,
    reason:
      error.reason === 'insufficient_outbound_capacity' ||
      error.reason === 'mnemonic_restore_not_send_ready' ||
      error.reason === 'send_readiness_unknown'
        ? 'stale_or_absent_wallet_readiness'
        : 'adapter_unavailable',
  })

const commandRequest = (
  config: MdkAgentWalletAdapterConfig,
  command: MdkAgentWalletCommandName,
  requestRef: string,
  args: MdkAgentWalletCommandArgs = {},
): MdkAgentWalletCommandRequest => {
  ensureStableRef('wallet_ref', config.walletRef)
  ensureStableRef('executor_ref', config.executorRef)
  ensureStableRef('request_ref', requestRef)

  return {
    args,
    command,
    requestRef,
    timeoutMs: config.defaultTimeoutMs ?? defaultTimeoutMs,
    walletRef: config.walletRef,
  }
}

const runMdkAgentWalletCommand = (
  config: MdkAgentWalletAdapterConfig,
  command: MdkAgentWalletCommandName,
  requestRef: string,
  args: MdkAgentWalletCommandArgs = {},
): Effect.Effect<MdkAgentWalletCommandResult, MdkAgentWalletCommandError> =>
  Effect.gen(function* () {
    const request = commandRequest(config, command, requestRef, args)
    const result = yield* Effect.tryPromise({
      try: () => config.executor.run(request),
      catch: error =>
        error instanceof MdkAgentWalletCommandError
          ? error
          : commandError(
              String(error).toLowerCase().includes('timeout')
                ? 'command_timeout'
                : 'daemon_unavailable',
              `detail.mdk_agent_wallet.${command}.executor_error`,
            ),
    })

    if (result.timedOut || result.exitCode !== 0) {
      return yield* commandError(
        classifyCommandFailure(result),
        `detail.mdk_agent_wallet.${command}.failed`,
      )
    }

    return result
  })

const parseMdkJson = <A>(
  schema: S.Decoder<A>,
  stdout: string,
  detailRef: string,
): Effect.Effect<A, MdkAgentWalletCommandError> =>
  Effect.try({
    try: () => parseJsonWithSchema(schema, stdout),
    catch: () => commandError('invalid_json', detailRef),
  })

const runJson = <A>(
  config: MdkAgentWalletAdapterConfig,
  command: MdkAgentWalletCommandName,
  requestRef: string,
  schema: S.Decoder<A>,
  args: MdkAgentWalletCommandArgs = {},
): Effect.Effect<A, MdkAgentWalletCommandError> =>
  Effect.gen(function* () {
    const result = yield* runMdkAgentWalletCommand(
      config,
      command,
      requestRef,
      args,
    )

    return yield* parseMdkJson(
      schema,
      result.stdout,
      `detail.mdk_agent_wallet.${command}.invalid_json`,
    )
  })

const amountToMdkSats = (
  intent: NexusTreasuryPayoutIntentRecord,
): Effect.Effect<number, MdkAgentWalletCommandError> => {
  if (
    intent.amount.asset !== 'bitcoin' ||
    intent.amount.denomination !== 'bitcoin_millisatoshi' ||
    intent.amount.amountMinorUnits <= 0 ||
    intent.amount.amountMinorUnits % 1_000 !== 0
  ) {
    return Effect.fail(
      commandError(
        'payment_failed',
        'detail.mdk_agent_wallet.unsupported_amount',
      ),
    )
  }

  return Effect.succeed(intent.amount.amountMinorUnits / 1_000)
}

const paymentsArray = (
  output: MdkAgentWalletPaymentsOutput,
): ReadonlyArray<MdkAgentWalletPaymentHistoryEntry> =>
  Array.isArray(output)
    ? output
    : (
        output as {
          readonly payments: ReadonlyArray<MdkAgentWalletPaymentHistoryEntry>
        }
      ).payments

const entryPaymentHash = (
  entry: MdkAgentWalletPaymentHistoryEntry,
): string | undefined => entry.payment_hash ?? entry.paymentHash

const reconciliationStatusByMdkStatus = (
  status: string,
): NexusTreasuryPayoutReconciliationEventRecord['status'] | undefined => {
  const normalized = status.toLowerCase()

  if (['pending', 'processing', 'sent_pending'].includes(normalized)) {
    return 'observed'
  }

  if (
    ['complete', 'completed', 'paid', 'settled', 'succeeded'].includes(
      normalized,
    )
  ) {
    return 'matched'
  }

  if (['duplicate', 'replayed'].includes(normalized)) {
    return 'replayed'
  }

  if (['failed', 'stale_pending'].includes(normalized)) {
    return 'rejected'
  }

  return undefined
}

const publicProjectionJson = (state: string): string =>
  JSON.stringify({
    adapter: 'mdk_agent_wallet',
    commandBoundary: 'agent_wallet_cli',
    moneyMovement: 'adapter_reported',
    rawMaterialStored: false,
    state,
  })

const balanceReadinessRef = (ready: boolean): string =>
  ready
    ? 'balance.mdk_agent_wallet.minimum_satisfied'
    : 'balance.mdk_agent_wallet.minimum_not_satisfied'

const walletHomeMode = (
  config: MdkAgentWalletAdapterConfig,
): MdkAgentWalletHomeMode => config.walletHomeMode ?? 'unknown'

const walletHomeModeRef = (mode: MdkAgentWalletHomeMode): string =>
  `wallet_home.mdk_agent_wallet.${mode}`

const walletHomeModeBlockerRefs = (
  mode: MdkAgentWalletHomeMode,
): ReadonlyArray<string> =>
  mode === 'original_funded_wallet_home'
    ? []
    : mode === 'mnemonic_restore'
      ? ['blocker.mdk_agent_wallet.mnemonic_restore_not_send_ready']
      : ['blocker.mdk_agent_wallet.original_wallet_home_unverified']

export const checkMdkAgentWalletSendReadiness = (
  config: MdkAgentWalletAdapterConfig,
  request: MdkAgentWalletReadinessRequest,
): Effect.Effect<
  MdkAgentWalletReadinessProjection,
  MdkAgentWalletCommandError
> =>
  Effect.gen(function* () {
    const mode = walletHomeMode(config)
    const homeBlockerRefs =
      request.requireOriginalFundedWalletHome === false
        ? []
        : walletHomeModeBlockerRefs(mode)

    if (homeBlockerRefs.length > 0) {
      return {
        balanceRef: 'balance.mdk_agent_wallet.not_checked',
        blockerRefs: homeBlockerRefs,
        caveatRefs: [
          'caveat.mdk_agent_wallet.balance_is_not_send_readiness',
          'caveat.mdk_agent_wallet.receive_ready_is_not_send_ready',
          'caveat.mdk_agent_wallet.mnemonic_restore_does_not_prove_outbound_capacity',
        ],
        executorRef: config.executorRef,
        homeModeRef: walletHomeModeRef(mode),
        receiveReady: false,
        ready: false,
        sendReady: false,
        walletRef: config.walletRef,
      }
    }

    const balance = yield* runJson(
      config,
      'balance',
      'request.mdk_agent_wallet.balance',
      MdkAgentWalletBalanceOutput,
    )
    const balanceReady = balance.balance_sats >= request.minimumSendAmountSats
    const blockerRefs = balanceReady
      ? []
      : ['blocker.mdk_agent_wallet.insufficient_balance']

    return {
      balanceRef: balanceReadinessRef(balanceReady),
      blockerRefs,
      caveatRefs: [
        'caveat.mdk_agent_wallet.balance_is_necessary_not_sufficient_for_send',
        'caveat.mdk_agent_wallet.outbound_capacity_must_survive_restore_boundary',
      ],
      executorRef: config.executorRef,
      homeModeRef: walletHomeModeRef(mode),
      receiveReady: balanceReady,
      ready: balanceReady,
      sendReady: balanceReady,
      walletRef: config.walletRef,
    }
  })

export const checkMdkAgentWalletReadiness = checkMdkAgentWalletSendReadiness

const requireMdkAgentWalletSendReadiness = (
  config: MdkAgentWalletAdapterConfig,
  request: MdkAgentWalletReadinessRequest,
): Effect.Effect<
  MdkAgentWalletReadinessProjection,
  MdkAgentWalletCommandError
> =>
  Effect.gen(function* () {
    const readiness = yield* checkMdkAgentWalletSendReadiness(config, request)

    if (!readiness.sendReady) {
      const reason = readiness.blockerRefs.includes(
        'blocker.mdk_agent_wallet.mnemonic_restore_not_send_ready',
      )
        ? 'mnemonic_restore_not_send_ready'
        : readiness.blockerRefs.includes(
              'blocker.mdk_agent_wallet.original_wallet_home_unverified',
            )
          ? 'send_readiness_unknown'
          : 'insufficient_balance'

      return yield* commandError(
        reason,
        `detail.mdk_agent_wallet.send_readiness.${reason}`,
      )
    }

    return readiness
  })

export const createMdkAgentWalletReceiveInvoice = (
  config: MdkAgentWalletAdapterConfig,
  amountSats: number,
): Effect.Effect<
  MdkAgentWalletReceiveInvoiceProjection,
  MdkAgentWalletCommandError
> =>
  Effect.gen(function* () {
    const output = yield* runJson(
      config,
      'receive',
      `request.mdk_agent_wallet.receive.${amountSats}`,
      MdkAgentWalletReceiveOutput,
      { amountSats },
    )

    return {
      expiresAt: output.expires_at,
      invoiceRef: yield* Effect.promise(() =>
        redactedMaterialRef(
          'invoice.redacted.mdk_agent_wallet',
          output.invoice,
        ),
      ),
      paymentHashRef: yield* Effect.promise(() =>
        redactedMaterialRef(
          'payment.redacted.mdk_agent_wallet',
          output.payment_hash,
        ),
      ),
      walletRef: config.walletRef,
    }
  })

export const makeMdkAgentWalletPayoutAdapter = (
  config: MdkAgentWalletAdapterConfig,
): TreasuryPaymentAuthorityAdapter => ({
  adapterKind: 'mdk_agent_wallet',
  dispatch: input =>
    Effect.gen(function* () {
      const amountSats = yield* amountToMdkSats(input.intent).pipe(
        Effect.mapError(treasuryError),
      )
      yield* requireMdkAgentWalletSendReadiness(config, {
        minimumSendAmountSats: amountSats,
      }).pipe(Effect.mapError(treasuryError))
      const sendOutput = yield* runJson(
        config,
        'send',
        `request.mdk_agent_wallet.send.${stableRef(input.attempt.idempotencyKeyHash)}`,
        MdkAgentWalletSendOutput,
        {
          amountSats,
          destinationRef: input.intent.payoutTargetRef,
          payoutAttemptRef: input.attempt.payoutAttemptRef,
          payoutIntentRef: input.intent.payoutIntentRef,
        },
      ).pipe(Effect.mapError(treasuryError))
      const paymentRef = yield* Effect.promise(() =>
        redactedMaterialRef(
          'payment.redacted.mdk_agent_wallet',
          sendOutput.payment_hash,
        ),
      )

      return {
        ...input.attempt,
        adapterKind: 'mdk_agent_wallet',
        adapterAttemptRef: `adapter_attempt.mdk_agent_wallet.${stableRef(input.attempt.idempotencyKeyHash)}`,
        metadataRefs: [
          ...new Set([
            ...input.attempt.metadataRefs,
            'metadata.nexus.mdk_agent_wallet.dispatch.sent',
          ]),
        ],
        publicProjectionJson: publicProjectionJson('dispatch_reported'),
        redactedPaymentRef: paymentRef,
        status: 'dispatched',
      } satisfies NexusTreasuryPayoutAttemptRecord
    }),
  preview: input =>
    Effect.gen(function* () {
      const amountSats = yield* amountToMdkSats(input.intent).pipe(
        Effect.mapError(treasuryError),
      )
      const readiness = yield* checkMdkAgentWalletReadiness(config, {
        minimumSendAmountSats: amountSats,
      }).pipe(Effect.mapError(treasuryError))

      return {
        adapterKind: 'mdk_agent_wallet',
        amount: input.intent.amount,
        dispatchAllowed: readiness.ready,
        payoutIntentRef: input.intent.payoutIntentRef,
        payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
        policySnapshotRef: input.intent.policySnapshotRef,
        spendCap: input.intent.spendCap,
      } satisfies TreasuryPaymentAuthorityPayoutPreview
    }),
  reconcile: input =>
    Effect.gen(function* () {
      const output = yield* runJson(
        config,
        'payments',
        `request.mdk_agent_wallet.payments.${stableRef(input.event.eventRef)}`,
        MdkAgentWalletPaymentsOutput,
        {
          eventRef: input.event.eventRef,
          payoutAttemptRef: input.event.payoutAttemptRef,
          payoutIntentRef: input.event.payoutIntentRef,
        },
      ).pipe(Effect.mapError(treasuryError))
      const entries = yield* Effect.all(
        paymentsArray(output).map(entry =>
          Effect.gen(function* () {
            const paymentHash = entryPaymentHash(entry)
            const paymentRef =
              paymentHash === undefined
                ? null
                : yield* Effect.promise(() =>
                    redactedMaterialRef(
                      'payment.redacted.mdk_agent_wallet',
                      paymentHash,
                    ),
                  )

            return { entry, paymentRef }
          }),
        ),
      )
      const match = entries.find(
        item =>
          item.entry.openagents_event_ref === input.event.eventRef ||
          item.paymentRef === input.event.externalEventRef ||
          item.paymentRef === input.event.resultRef ||
          (item.paymentRef !== null &&
            input.event.metadataRefs.includes(item.paymentRef)),
      )

      if (match === undefined) {
        return yield* treasuryError(
          commandError(
            'reconciliation_mismatch',
            'detail.mdk_agent_wallet.payments.no_matching_event',
          ),
        )
      }

      const status = reconciliationStatusByMdkStatus(match.entry.status)

      if (status === undefined) {
        return yield* treasuryError(
          commandError(
            'reconciliation_mismatch',
            'detail.mdk_agent_wallet.payments.status_unknown',
          ),
        )
      }

      return {
        ...input.event,
        adapterKind: 'mdk_agent_wallet',
        metadataRefs: [
          ...new Set([
            ...input.event.metadataRefs,
            `metadata.nexus.mdk_agent_wallet.reconciliation.${stableRef(match.entry.status)}`,
          ]),
        ],
        publicProjectionJson: publicProjectionJson(
          `reconciliation_${stableRef(match.entry.status)}`,
        ),
        resultRef:
          match.paymentRef ??
          `result.mdk_agent_wallet.${stableRef(match.entry.status)}`,
        status,
      } satisfies NexusTreasuryPayoutReconciliationEventRecord
    }),
})
