import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonInstallToBitcoinSmokeInput,
  PylonInstallToBitcoinSmokeProjection,
  PylonInstallToBitcoinSmokeUnsafe,
  planPylonInstallToBitcoinSmoke,
  pylonInstallToBitcoinSmokeHasPrivateMaterial,
} from './pylon-install-to-bitcoin-smoke'

const baseInput = (
  overrides: Partial<PylonInstallToBitcoinSmokeInput> = {},
): PylonInstallToBitcoinSmokeInput =>
  new PylonInstallToBitcoinSmokeInput({
    acceptedWorkRefs: ['accepted_work.public.install_to_bitcoin.echo'],
    amountSats: 1,
    assignmentLeaseExpiresAtIso: '2026-06-08T23:05:00.000Z',
    assignmentRefs: ['assignment.public.install_to_bitcoin.echo'],
    closeoutRefs: ['closeout.public.install_to_bitcoin.accepted'],
    heartbeatRefs: ['heartbeat.public.install_to_bitcoin.fresh'],
    installRefs: ['install.public.pylon.latest.launcher_0_2_5'],
    mdkEndpointRef: 'endpoint.openagents.pylon.install_to_bitcoin.l402',
    mode: 'ci_no_spend',
    nowIso: '2026-06-08T23:00:00.000Z',
    operatorApprovalRefs: [],
    operatorApprovedLiveSpend: false,
    paymentReceiptRefs: [],
    publicProjectionRefs: ['projection.public.install_to_bitcoin.bundle'],
    pylonRefs: ['pylon.public.install_to_bitcoin.local'],
    payoutReadinessRefs: ['payout_readiness.public.install_to_bitcoin.ready'],
    registrationRefs: ['registration.public.install_to_bitcoin.local'],
    routeStateRef: 'route_state.public.install_to_bitcoin.payment_required',
    settlementReceiptRefs: [],
    spendCapSats: 10,
    tokenCacheRef: 'token_cache.public.install_to_bitcoin.redacted',
    walletHomeMode: 'unknown',
    walletHomeRef: 'wallet_home.mdk_agent_wallet.unknown',
    walletReadinessRefs: ['wallet_readiness.public.install_to_bitcoin.ready'],
    ...overrides,
  })

describe('Pylon install-to-bitcoin launch smoke', () => {
  test('keeps CI no-spend mode complete without any spend authority', () => {
    const projection = planPylonInstallToBitcoinSmoke(baseInput())

    expect(
      S.decodeUnknownSync(PylonInstallToBitcoinSmokeProjection)(projection),
    ).toEqual(projection)
    expect(projection.status).toBe('ci_no_spend_ready')
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.settledBitcoinClaimAllowed).toBe(false)
    expect(projection.steps.map(step => step.kind)).toEqual([
      'install',
      'register',
      'heartbeat',
      'wallet',
      'assignment',
      'closeout',
      'payment',
      'settlement',
      'public_projection',
    ])
    expect(
      projection.steps.find(step => step.kind === 'payment'),
    ).toMatchObject({
      maySpendBitcoin: false,
      state: 'planned_no_spend',
    })
    expect(projection.smokeBundleRefs).toEqual([
      'accepted_work.public.install_to_bitcoin.echo',
      'assignment.public.install_to_bitcoin.echo',
      'closeout.public.install_to_bitcoin.accepted',
      'heartbeat.public.install_to_bitcoin.fresh',
      'install.public.pylon.latest.launcher_0_2_5',
      'payout_readiness.public.install_to_bitcoin.ready',
      'projection.public.install_to_bitcoin.bundle',
      'pylon.public.install_to_bitcoin.local',
      'registration.public.install_to_bitcoin.local',
      'wallet_readiness.public.install_to_bitcoin.ready',
    ])
    expect(pylonInstallToBitcoinSmokeHasPrivateMaterial(projection)).toBe(false)
  })

  test('allows sandbox fake-payment evidence while blocking settled bitcoin claims', () => {
    const projection = planPylonInstallToBitcoinSmoke(
      baseInput({
        mode: 'sandbox_fake_payment',
        paymentReceiptRefs: ['payment_receipt.public.fake_sandbox.redacted'],
        settlementReceiptRefs: ['settlement.public.fake_sandbox.redacted'],
      }),
    )

    expect(projection.status).toBe('sandbox_fake_payment_ready')
    expect(projection.paymentClaimAllowed).toBe(true)
    expect(projection.settlementClaimAllowed).toBe(true)
    expect(projection.settledBitcoinClaimAllowed).toBe(false)
    expect(projection.mdkPaymentPlan.status).toBe('documentation_only')
    expect(projection.steps.some(step => step.maySpendBitcoin)).toBe(false)
  })

  test('opens live small-sats mode only with cap, approval, original wallet home, and receipts', () => {
    const projection = planPylonInstallToBitcoinSmoke(
      baseInput({
        mode: 'live_small_sats',
        operatorApprovalRefs: [
          'approval.public.install_to_bitcoin.live_small_sats',
        ],
        operatorApprovedLiveSpend: true,
        paymentReceiptRefs: ['payment_receipt.public.live_small_sats.redacted'],
        settlementReceiptRefs: [
          'settlement.public.live_small_sats.receipt_recorded',
        ],
        walletHomeMode: 'original_funded_wallet_home',
        walletHomeRef:
          'wallet_home.mdk_agent_wallet.original_funded_wallet_home',
      }),
    )

    expect(projection.status).toBe('live_settled_bitcoin_ready')
    expect(projection.liveWalletSpendAllowed).toBe(true)
    expect(projection.settledBitcoinClaimAllowed).toBe(true)
    expect(projection.mdkPaymentPlan.status).toBe('ready_for_signet')
    expect(
      projection.steps.find(step => step.kind === 'payment'),
    ).toMatchObject({
      maySpendBitcoin: true,
      state: 'passed',
    })
  })

  test('blocks live mode on missing approval, stale lease, payout readiness, or receipts', () => {
    const projection = planPylonInstallToBitcoinSmoke(
      baseInput({
        assignmentLeaseExpiresAtIso: '2026-06-08T22:59:59.000Z',
        mode: 'live_small_sats',
        operatorApprovedLiveSpend: false,
        payoutReadinessRefs: [],
        walletHomeMode: 'mnemonic_restore',
        walletHomeRef: 'wallet_home.mdk_agent_wallet.mnemonic_restore',
      }),
    )

    expect(projection.status).toBe('blocked')
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual([
      'blocker.pylon_install_to_bitcoin.assignment_lease_stale',
      'blocker.pylon_install_to_bitcoin.mdk_payment_plan_not_ready',
      'blocker.pylon_install_to_bitcoin.mdk_send_readiness_missing',
      'blocker.pylon_install_to_bitcoin.operator_approval_missing',
      'blocker.pylon_install_to_bitcoin.payment_receipt_missing',
      'blocker.pylon_install_to_bitcoin.payout_readiness_missing',
      'blocker.pylon_install_to_bitcoin.settlement_receipt_missing',
    ])
    expect(
      projection.steps.find(step => step.kind === 'assignment')?.blockerRefs,
    ).toContain('blocker.pylon_install_to_bitcoin.assignment_lease_stale')
  })

  test('rejects raw wallet, invoice, preimage, payout target, and timestamp material', () => {
    for (const input of [
      baseInput({ walletHomeRef: '/Users/private/.mdk-wallet/config.json' }),
      baseInput({ paymentReceiptRefs: ['lnbc10n1rawinvoice'] }),
      baseInput({ settlementReceiptRefs: ['payment_preimage.secret_value'] }),
      baseInput({ payoutReadinessRefs: ['payout_address.bc1qprivate'] }),
      baseInput({ installRefs: ['install.public.2026-06-08T23:00:00Z'] }),
    ]) {
      expect(() => planPylonInstallToBitcoinSmoke(input)).toThrow(
        PylonInstallToBitcoinSmokeUnsafe,
      )
    }
  })
})
