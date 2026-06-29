import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  defineTreasuryPaymentAdapterConformanceSuite,
  treasuryPaymentAdapterConformanceFixtures,
} from './treasury-payment-adapter-conformance.test-support'
import {
  type MdkAgentWalletAdapterConfig,
  type MdkAgentWalletCommandExecutor,
  type MdkAgentWalletCommandRequest,
  type MdkAgentWalletCommandResult,
  checkMdkAgentWalletReadiness,
  checkMdkAgentWalletSendReadiness,
  createMdkAgentWalletReceiveInvoice,
  makeMdkAgentWalletPayoutAdapter,
} from './treasury-payment-mdk-agent-wallet-adapter'

class FakeMdkExecutor implements MdkAgentWalletCommandExecutor {
  calls: Array<MdkAgentWalletCommandRequest> = []
  camelCasePaymentHistory = false
  failSendForAttemptRef: string | null = null
  failureText = 'payment failed'
  invalidJsonCommand: MdkAgentWalletCommandRequest['command'] | null = null
  paymentStatusByEventRef = new Map<string, string>()
  timedOutCommand: MdkAgentWalletCommandRequest['command'] | null = null
  unavailableCommand: MdkAgentWalletCommandRequest['command'] | null = null

  run = async (
    request: MdkAgentWalletCommandRequest,
  ): Promise<MdkAgentWalletCommandResult> => {
    this.calls.push(request)

    if (request.command === this.timedOutCommand) {
      return this.result('', {
        stderrDigestRef: 'error.timeout',
        timedOut: true,
      })
    }

    if (request.command === this.unavailableCommand) {
      return this.result('daemon unavailable', {
        exitCode: 1,
        stderrDigestRef: 'error.daemon_unavailable',
      })
    }

    if (request.command === this.invalidJsonCommand) {
      return this.result('not json')
    }

    if (request.command === 'balance') {
      return this.result(JSON.stringify({ balance_sats: 10_000 }))
    }

    if (request.command === 'receive') {
      return this.result(
        JSON.stringify({
          expires_at: '2026-06-07T09:00:00.000Z',
          invoice: 'lnbc10n1rawinvoicefixture',
          payment_hash: 'raw_receive_payment_hash_fixture',
        }),
      )
    }

    if (request.command === 'send') {
      if (request.args.payoutAttemptRef === this.failSendForAttemptRef) {
        return this.result(this.failureText, {
          exitCode: 1,
          stderrDigestRef: `error.${this.failureText.replaceAll(/\s+/g, '_')}`,
        })
      }

      return this.result(
        JSON.stringify({
          payment_hash: `raw_send_payment_hash_${request.args.payoutAttemptRef}`,
        }),
      )
    }

    const eventRef =
      typeof request.args.eventRef === 'string' ? request.args.eventRef : ''
    const payoutAttemptRef =
      typeof request.args.payoutAttemptRef === 'string'
        ? request.args.payoutAttemptRef
        : eventRef
    const status = this.paymentStatusByEventRef.get(eventRef) ?? 'succeeded'

    return this.result(
      JSON.stringify({
        payments: [
          this.camelCasePaymentHistory
            ? {
                amountSats: 1,
                direction: 'outbound',
                paymentHash: `raw_send_payment_hash_${payoutAttemptRef}`,
                paymentId: `payment_id_${eventRef}`,
                status,
              }
            : {
                amount_sats: 1,
                direction: 'send',
                openagents_event_ref: eventRef,
                payment_hash: `raw_history_payment_hash_${eventRef}`,
                status,
              },
        ],
      }),
    )
  }

  result = (
    stdout: string,
    overrides: Partial<MdkAgentWalletCommandResult> = {},
  ): MdkAgentWalletCommandResult => ({
    durationMs: 25,
    exitCode: 0,
    stderrDigestRef: null,
    stdout,
    timedOut: false,
    ...overrides,
  })
}

const config = (
  executor: FakeMdkExecutor,
  overrides: Partial<MdkAgentWalletAdapterConfig> = {},
): MdkAgentWalletAdapterConfig => ({
  defaultTimeoutMs: 1_000,
  executor,
  executorRef: 'executor.mdk_agent_wallet.mock',
  walletHomeMode: 'original_funded_wallet_home',
  walletRef: 'wallet_ref.mdk_agent_wallet.test',
  ...overrides,
})

defineTreasuryPaymentAdapterConformanceSuite({
  adapterKind: 'mdk_agent_wallet',
  makeSubject: fixtures => {
    const executor = new FakeMdkExecutor()

    executor.paymentStatusByEventRef.set(
      fixtures.pendingEvent.eventRef,
      'pending',
    )
    executor.paymentStatusByEventRef.set(
      fixtures.succeededEvent.eventRef,
      'succeeded',
    )
    executor.paymentStatusByEventRef.set(
      fixtures.failedEvent.eventRef,
      'failed',
    )
    executor.paymentStatusByEventRef.set(
      fixtures.duplicateEvent.eventRef,
      'duplicate',
    )
    executor.paymentStatusByEventRef.set(
      fixtures.stalePendingEvent.eventRef,
      'stale_pending',
    )

    return {
      adapter: makeMdkAgentWalletPayoutAdapter(config(executor)),
      expected: {
        rejectedDispatchStatus: 'dispatched',
      },
    }
  },
  name: 'mdk_agent_wallet',
})

describe('MDK agent-wallet payout adapter boundary', () => {
  test('reports wallet readiness without exposing the exact balance', async () => {
    const executor = new FakeMdkExecutor()
    const readiness = await Effect.runPromise(
      checkMdkAgentWalletReadiness(config(executor), {
        minimumSendAmountSats: 1,
      }),
    )

    expect(readiness.ready).toBe(true)
    expect(readiness.sendReady).toBe(true)
    expect(readiness.receiveReady).toBe(true)
    expect(readiness.balanceRef).toBe(
      'balance.mdk_agent_wallet.minimum_satisfied',
    )
    expect(readiness.homeModeRef).toBe(
      'wallet_home.mdk_agent_wallet.original_funded_wallet_home',
    )
    expect(readiness.blockerRefs).toEqual([])
    expect(JSON.stringify(readiness)).not.toContain('10000')
  })

  test('blocks mnemonic-restore mode before balance or send preflight', async () => {
    const executor = new FakeMdkExecutor()
    const restoreConfig = config(executor, {
      walletHomeMode: 'mnemonic_restore',
    })
    const readiness = await Effect.runPromise(
      checkMdkAgentWalletSendReadiness(restoreConfig, {
        minimumSendAmountSats: 1,
      }),
    )

    expect(readiness).toMatchObject({
      balanceRef: 'balance.mdk_agent_wallet.not_checked',
      blockerRefs: ['blocker.mdk_agent_wallet.mnemonic_restore_not_send_ready'],
      homeModeRef: 'wallet_home.mdk_agent_wallet.mnemonic_restore',
      ready: false,
      sendReady: false,
    })
    expect(executor.calls.map(call => call.command)).toEqual([])

    const fixtures =
      treasuryPaymentAdapterConformanceFixtures('mdk_agent_wallet')
    await expect(
      Effect.runPromise(
        makeMdkAgentWalletPayoutAdapter(restoreConfig).dispatch({
          attempt: fixtures.attempt,
          intent: fixtures.intent,
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining('mnemonic_restore_not_send_ready'),
      reason: 'stale_or_absent_wallet_readiness',
    })
    expect(executor.calls.map(call => call.command)).toEqual([])
  })

  test('redacts receive invoices and payment hashes before returning refs', async () => {
    const executor = new FakeMdkExecutor()
    const invoice = await Effect.runPromise(
      createMdkAgentWalletReceiveInvoice(config(executor), 1),
    )

    expect(invoice.invoiceRef).toMatch(/^invoice\.redacted\.mdk_agent_wallet\./)
    expect(invoice.paymentHashRef).toMatch(
      /^payment\.redacted\.mdk_agent_wallet\./,
    )
    expect(JSON.stringify(invoice)).not.toMatch(
      /lnbc|raw_receive_payment_hash_fixture/i,
    )
    expect(executor.calls[0]?.command).toBe('receive')
  })

  test('reconciles camelCase agent-wallet payment history by redacted payment ref', async () => {
    const executor = new FakeMdkExecutor()
    executor.camelCasePaymentHistory = true
    const fixtures =
      treasuryPaymentAdapterConformanceFixtures('mdk_agent_wallet')
    const adapter = makeMdkAgentWalletPayoutAdapter(config(executor))
    const dispatched = await Effect.runPromise(
      adapter.dispatch({
        attempt: fixtures.attempt,
        intent: fixtures.intent,
      }),
    )
    const event = {
      ...fixtures.succeededEvent,
      externalEventRef:
        dispatched.redactedPaymentRef ?? 'payment.redacted.none',
      metadataRefs:
        dispatched.redactedPaymentRef === null
          ? fixtures.succeededEvent.metadataRefs
          : [
              ...fixtures.succeededEvent.metadataRefs,
              dispatched.redactedPaymentRef,
            ],
    }
    const reconciled = await Effect.runPromise(adapter.reconcile({ event }))

    expect(reconciled.status).toBe('matched')
    expect(reconciled.resultRef).toBe(dispatched.redactedPaymentRef)
  })

  test('classifies command timeout, daemon unavailable, invalid JSON, insufficient balance, payment failure, and reconciliation mismatch', async () => {
    const timeout = new FakeMdkExecutor()
    timeout.timedOutCommand = 'balance'
    await expect(
      Effect.runPromise(
        checkMdkAgentWalletReadiness(config(timeout), {
          minimumSendAmountSats: 1,
        }),
      ),
    ).rejects.toMatchObject({
      reason: 'command_timeout',
    })

    const unavailable = new FakeMdkExecutor()
    unavailable.unavailableCommand = 'balance'
    await expect(
      Effect.runPromise(
        checkMdkAgentWalletReadiness(config(unavailable), {
          minimumSendAmountSats: 1,
        }),
      ),
    ).rejects.toMatchObject({
      reason: 'daemon_unavailable',
    })

    const invalidJson = new FakeMdkExecutor()
    invalidJson.invalidJsonCommand = 'receive'
    await expect(
      Effect.runPromise(
        createMdkAgentWalletReceiveInvoice(config(invalidJson), 1),
      ),
    ).rejects.toMatchObject({
      reason: 'invalid_json',
    })

    const fixtures =
      treasuryPaymentAdapterConformanceFixtures('mdk_agent_wallet')
    const insufficient = new FakeMdkExecutor()
    insufficient.failSendForAttemptRef = fixtures.attempt.payoutAttemptRef
    insufficient.failureText = 'insufficient balance'
    await expect(
      Effect.runPromise(
        makeMdkAgentWalletPayoutAdapter(config(insufficient)).dispatch({
          attempt: fixtures.attempt,
          intent: fixtures.intent,
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining('insufficient_balance'),
      reason: 'adapter_unavailable',
    })

    const noOutboundCapacity = new FakeMdkExecutor()
    noOutboundCapacity.failSendForAttemptRef = fixtures.attempt.payoutAttemptRef
    noOutboundCapacity.failureText =
      'insufficient outbound capacity: required 1000msat, available 0msat'
    await expect(
      Effect.runPromise(
        makeMdkAgentWalletPayoutAdapter(config(noOutboundCapacity)).dispatch({
          attempt: fixtures.attempt,
          intent: fixtures.intent,
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining('insufficient_outbound_capacity'),
      reason: 'stale_or_absent_wallet_readiness',
    })

    const failed = new FakeMdkExecutor()
    failed.failSendForAttemptRef = fixtures.attempt.payoutAttemptRef
    failed.failureText = 'payment failed'
    await expect(
      Effect.runPromise(
        makeMdkAgentWalletPayoutAdapter(config(failed)).dispatch({
          attempt: fixtures.attempt,
          intent: fixtures.intent,
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining('payment_failed'),
      reason: 'adapter_unavailable',
    })

    const mismatch = new FakeMdkExecutor()
    mismatch.paymentStatusByEventRef.clear()
    mismatch.run = async request =>
      request.command === 'payments'
        ? mismatch.result(JSON.stringify({ payments: [] }))
        : FakeMdkExecutor.prototype.run.call(mismatch, request)

    await expect(
      Effect.runPromise(
        makeMdkAgentWalletPayoutAdapter(config(mismatch)).reconcile({
          event: fixtures.succeededEvent,
        }),
      ),
    ).rejects.toMatchObject({
      message: expect.stringContaining('reconciliation_mismatch'),
      reason: 'adapter_unavailable',
    })
  })
})
