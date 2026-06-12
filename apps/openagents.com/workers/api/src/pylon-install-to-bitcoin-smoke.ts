import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type OpenAgentsMdkAgentWalletSmokeInput,
  OpenAgentsMdkAgentWalletSmokeProjection,
  openAgentsMdkAgentWalletSmokeHasPrivateMaterial,
  planOpenAgentsMdkAgentWalletSmoke,
} from './mdk-agent-wallet-smoke-fixture'

export const PylonInstallToBitcoinSmokeSchemaVersion =
  'omega.pylon_install_to_bitcoin_smoke.v1'

export const PylonInstallToBitcoinSmokeMode = S.Literals([
  'ci_no_spend',
  'live_small_sats',
  'sandbox_fake_payment',
])
export type PylonInstallToBitcoinSmokeMode =
  typeof PylonInstallToBitcoinSmokeMode.Type

export const PylonInstallToBitcoinSmokeStepKind = S.Literals([
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
export type PylonInstallToBitcoinSmokeStepKind =
  typeof PylonInstallToBitcoinSmokeStepKind.Type

export const PylonInstallToBitcoinSmokeStepState = S.Literals([
  'blocked',
  'not_applicable',
  'passed',
  'planned_no_spend',
])
export type PylonInstallToBitcoinSmokeStepState =
  typeof PylonInstallToBitcoinSmokeStepState.Type

export const PylonInstallToBitcoinSmokeStatus = S.Literals([
  'blocked',
  'ci_no_spend_ready',
  'live_settled_bitcoin_ready',
  'sandbox_fake_payment_ready',
])
export type PylonInstallToBitcoinSmokeStatus =
  typeof PylonInstallToBitcoinSmokeStatus.Type

export class PylonInstallToBitcoinSmokeInput extends S.Class<PylonInstallToBitcoinSmokeInput>(
  'PylonInstallToBitcoinSmokeInput',
)({
  acceptedWorkRefs: S.Array(S.String),
  amountSats: S.Number,
  assignmentLeaseExpiresAtIso: S.String,
  assignmentRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  heartbeatRefs: S.Array(S.String),
  installRefs: S.Array(S.String),
  mdkEndpointRef: S.String,
  mode: PylonInstallToBitcoinSmokeMode,
  nowIso: S.String,
  operatorApprovalRefs: S.Array(S.String),
  operatorApprovedLiveSpend: S.Boolean,
  paymentReceiptRefs: S.Array(S.String),
  publicProjectionRefs: S.Array(S.String),
  pylonRefs: S.Array(S.String),
  payoutReadinessRefs: S.Array(S.String),
  registrationRefs: S.Array(S.String),
  routeStateRef: S.String,
  settlementReceiptRefs: S.Array(S.String),
  spendCapSats: S.Number,
  tokenCacheRef: S.String,
  walletHomeMode: S.Literals([
    'mnemonic_restore',
    'original_funded_wallet_home',
    'unknown',
  ]),
  walletHomeRef: S.String,
  walletReadinessRefs: S.Array(S.String),
}) {}

export class PylonInstallToBitcoinSmokeStep extends S.Class<PylonInstallToBitcoinSmokeStep>(
  'PylonInstallToBitcoinSmokeStep',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  guardRefs: S.Array(S.String),
  kind: PylonInstallToBitcoinSmokeStepKind,
  maySpendBitcoin: S.Boolean,
  state: PylonInstallToBitcoinSmokeStepState,
}) {}

export class PylonInstallToBitcoinSmokeProjection extends S.Class<PylonInstallToBitcoinSmokeProjection>(
  'PylonInstallToBitcoinSmokeProjection',
)({
  amountSats: S.Number,
  blockerRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  mdkPaymentPlan: OpenAgentsMdkAgentWalletSmokeProjection,
  mode: PylonInstallToBitcoinSmokeMode,
  paymentClaimAllowed: S.Boolean,
  publicProjectionRefs: S.Array(S.String),
  redactionScanPassed: S.Boolean,
  schemaVersion: S.Literal(PylonInstallToBitcoinSmokeSchemaVersion),
  settledBitcoinClaimAllowed: S.Boolean,
  settlementClaimAllowed: S.Boolean,
  smokeBundleRefs: S.Array(S.String),
  spendCapSats: S.Number,
  status: PylonInstallToBitcoinSmokeStatus,
  steps: S.Array(PylonInstallToBitcoinSmokeStep),
}) {}

export class PylonInstallToBitcoinSmokeUnsafe extends S.TaggedErrorClass<PylonInstallToBitcoinSmokeUnsafe>()(
  'PylonInstallToBitcoinSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredStepKinds: ReadonlyArray<PylonInstallToBitcoinSmokeStepKind> = [
  'install',
  'register',
  'heartbeat',
  'wallet',
  'assignment',
  'closeout',
  'payment',
  'settlement',
  'public_projection',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer|bolt11|bolt12|channel[_-]?monitor|cookie|customer|email|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|invoice|preimage|raw|secret)|payment\.(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|telemetry|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|dataset|email|invoice|log|model|node|payment|payload|payout|prompt|provider|record|release|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const isAllowedWalletHomeRef = (ref: string): boolean =>
  /^wallet_home\.(?:public\.)?mdk_agent_wallet\.(mnemonic_restore|original_funded_wallet_home|unknown)$/.test(
    ref,
  )

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      (!isAllowedWalletHomeRef(ref) &&
        (containsProviderSecretMaterial(ref) ||
          unsafeMaterialPattern.test(ref))) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PylonInstallToBitcoinSmokeUnsafe({
      reason: `${label} contains private, wallet, raw payment, invoice, preimage, payout target, provider, runner, customer, or timestamp material.`,
    })
  }
}

const inputRefGroups = (
  input: PylonInstallToBitcoinSmokeInput,
): ReadonlyArray<ReadonlyArray<string>> => [
  input.acceptedWorkRefs,
  input.assignmentRefs,
  input.closeoutRefs,
  input.heartbeatRefs,
  input.installRefs,
  input.operatorApprovalRefs,
  input.paymentReceiptRefs,
  input.publicProjectionRefs,
  input.pylonRefs,
  input.payoutReadinessRefs,
  input.registrationRefs,
  input.settlementReceiptRefs,
  input.walletReadinessRefs,
  [
    input.mdkEndpointRef,
    input.routeStateRef,
    input.tokenCacheRef,
    input.walletHomeRef,
  ],
]

const assertInputSafe = (input: PylonInstallToBitcoinSmokeInput): void => {
  for (const [index, refs] of inputRefGroups(input).entries()) {
    assertSafeRefs(`Pylon install-to-bitcoin smoke ref group ${index}`, refs)
  }
}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  uniqueRefs(refs).length > 0

const stepFor = (
  kind: PylonInstallToBitcoinSmokeStepKind,
  evidenceRefs: ReadonlyArray<string>,
  guardRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
  state: PylonInstallToBitcoinSmokeStepState,
  maySpendBitcoin = false,
): PylonInstallToBitcoinSmokeStep =>
  new PylonInstallToBitcoinSmokeStep({
    blockerRefs: uniqueRefs(blockerRefs),
    evidenceRefs: uniqueRefs(evidenceRefs),
    guardRefs: uniqueRefs(guardRefs),
    kind,
    maySpendBitcoin,
    state,
  })

const blockerIfMissing = (
  condition: boolean,
  blockerRef: string,
): ReadonlyArray<string> => (condition ? [] : [blockerRef])

const leaseActive = (input: PylonInstallToBitcoinSmokeInput): boolean =>
  Date.parse(input.assignmentLeaseExpiresAtIso) > Date.parse(input.nowIso)

const mdkSmokeInputFor = (
  input: PylonInstallToBitcoinSmokeInput,
): OpenAgentsMdkAgentWalletSmokeInput => ({
  amountBitcoinSatoshis: Math.max(0, Math.trunc(input.amountSats)),
  endpointRef: input.mdkEndpointRef,
  mode:
    input.mode === 'sandbox_fake_payment'
      ? 'fake_sandbox'
      : input.mode === 'live_small_sats'
        ? 'signet'
        : 'live_blocked',
  operatorApprovedPayment: input.operatorApprovedLiveSpend,
  routeStateRef: input.routeStateRef,
  spendCapBitcoinSatoshis: Math.max(0, Math.trunc(input.spendCapSats)),
  tokenCacheRef: input.tokenCacheRef,
  walletHomeMode: input.walletHomeMode,
  walletHomeRef: input.walletHomeRef,
})

const modeGuardRefs = (
  input: PylonInstallToBitcoinSmokeInput,
): ReadonlyArray<string> => [
  `mode.pylon_install_to_bitcoin.${input.mode}`,
  `spend_cap.bitcoin_satoshis.${Math.max(0, Math.trunc(input.spendCapSats))}`,
  ...(input.operatorApprovedLiveSpend
    ? input.operatorApprovalRefs
    : ['blocker.pylon_install_to_bitcoin.operator_approval_missing']),
]

const liveBlockerRefs = (
  input: PylonInstallToBitcoinSmokeInput,
  mdkPaymentPlan: OpenAgentsMdkAgentWalletSmokeProjection,
): ReadonlyArray<string> => [
  ...blockerIfMissing(
    input.amountSats <= input.spendCapSats,
    'blocker.pylon_install_to_bitcoin.spend_cap_exceeded',
  ),
  ...blockerIfMissing(
    input.operatorApprovedLiveSpend,
    'blocker.pylon_install_to_bitcoin.operator_approval_missing',
  ),
  ...blockerIfMissing(
    input.walletHomeMode === 'original_funded_wallet_home',
    'blocker.pylon_install_to_bitcoin.mdk_send_readiness_missing',
  ),
  ...blockerIfMissing(
    mdkPaymentPlan.status === 'ready_for_signet',
    'blocker.pylon_install_to_bitcoin.mdk_payment_plan_not_ready',
  ),
  ...blockerIfMissing(
    hasRefs(input.paymentReceiptRefs),
    'blocker.pylon_install_to_bitcoin.payment_receipt_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.settlementReceiptRefs),
    'blocker.pylon_install_to_bitcoin.settlement_receipt_missing',
  ),
]

const commonBlockerRefs = (
  input: PylonInstallToBitcoinSmokeInput,
): ReadonlyArray<string> => [
  ...blockerIfMissing(
    hasRefs(input.installRefs),
    'blocker.pylon_install_to_bitcoin.install_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.registrationRefs) && hasRefs(input.pylonRefs),
    'blocker.pylon_install_to_bitcoin.registration_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.heartbeatRefs),
    'blocker.pylon_install_to_bitcoin.heartbeat_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.walletReadinessRefs),
    'blocker.pylon_install_to_bitcoin.wallet_readiness_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.assignmentRefs),
    'blocker.pylon_install_to_bitcoin.assignment_missing',
  ),
  ...blockerIfMissing(
    leaseActive(input),
    'blocker.pylon_install_to_bitcoin.assignment_lease_stale',
  ),
  ...blockerIfMissing(
    hasRefs(input.closeoutRefs) && hasRefs(input.acceptedWorkRefs),
    'blocker.pylon_install_to_bitcoin.accepted_closeout_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.payoutReadinessRefs),
    'blocker.pylon_install_to_bitcoin.payout_readiness_missing',
  ),
  ...blockerIfMissing(
    hasRefs(input.publicProjectionRefs),
    'blocker.pylon_install_to_bitcoin.public_projection_missing',
  ),
]

const paymentStateFor = (
  input: PylonInstallToBitcoinSmokeInput,
  blockers: ReadonlyArray<string>,
): PylonInstallToBitcoinSmokeStepState =>
  blockers.length > 0
    ? 'blocked'
    : input.mode === 'ci_no_spend'
      ? 'planned_no_spend'
      : 'passed'

const settlementStateFor = paymentStateFor

const statusFor = (
  input: PylonInstallToBitcoinSmokeInput,
  blockers: ReadonlyArray<string>,
): PylonInstallToBitcoinSmokeStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (input.mode === 'live_small_sats') {
    return 'live_settled_bitcoin_ready'
  }

  if (input.mode === 'sandbox_fake_payment') {
    return 'sandbox_fake_payment_ready'
  }

  return 'ci_no_spend_ready'
}

const smokeBundleRefsFor = (
  input: PylonInstallToBitcoinSmokeInput,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...input.installRefs,
    ...input.pylonRefs,
    ...input.registrationRefs,
    ...input.heartbeatRefs,
    ...input.walletReadinessRefs,
    ...input.payoutReadinessRefs,
    ...input.assignmentRefs,
    ...input.closeoutRefs,
    ...input.acceptedWorkRefs,
    ...input.paymentReceiptRefs,
    ...input.settlementReceiptRefs,
    ...input.publicProjectionRefs,
  ])

export const pylonInstallToBitcoinSmokeHasPrivateMaterial = (
  value: unknown,
): boolean => {
  if (
    value !== null &&
    typeof value === 'object' &&
    'mdkPaymentPlan' in value
  ) {
    const { mdkPaymentPlan, ...rest } = value as {
      readonly mdkPaymentPlan: unknown
      readonly [key: string]: unknown
    }

    return (
      pylonInstallToBitcoinSmokeHasPrivateMaterial(rest) ||
      openAgentsMdkAgentWalletSmokeHasPrivateMaterial(mdkPaymentPlan)
    )
  }

  const json = JSON.stringify(value)

  return (
    containsProviderSecretMaterial(json) ||
    unsafeMaterialPattern.test(json) ||
    rawTimestampPattern.test(json)
  )
}

export const planPylonInstallToBitcoinSmoke = (
  input: PylonInstallToBitcoinSmokeInput,
): PylonInstallToBitcoinSmokeProjection => {
  assertInputSafe(input)

  const mdkPaymentPlan = planOpenAgentsMdkAgentWalletSmoke(
    mdkSmokeInputFor(input),
  )
  const commonBlockers = commonBlockerRefs(input)
  const paymentBlockers =
    input.mode === 'live_small_sats'
      ? liveBlockerRefs(input, mdkPaymentPlan)
      : []
  const blockerRefs = uniqueRefs([...commonBlockers, ...paymentBlockers])
  const liveReady = input.mode === 'live_small_sats' && blockerRefs.length === 0

  const steps = [
    stepFor(
      'install',
      input.installRefs,
      ['guard.pylon_install_to_bitcoin.fresh_launcher_install'],
      blockerIfMissing(
        hasRefs(input.installRefs),
        'blocker.pylon_install_to_bitcoin.install_missing',
      ),
      hasRefs(input.installRefs) ? 'passed' : 'blocked',
    ),
    stepFor(
      'register',
      [...input.registrationRefs, ...input.pylonRefs],
      ['guard.pylon_install_to_bitcoin.public_registration_ref_required'],
      blockerIfMissing(
        hasRefs(input.registrationRefs) && hasRefs(input.pylonRefs),
        'blocker.pylon_install_to_bitcoin.registration_missing',
      ),
      hasRefs(input.registrationRefs) && hasRefs(input.pylonRefs)
        ? 'passed'
        : 'blocked',
    ),
    stepFor(
      'heartbeat',
      input.heartbeatRefs,
      ['guard.pylon_install_to_bitcoin.fresh_heartbeat_required'],
      blockerIfMissing(
        hasRefs(input.heartbeatRefs),
        'blocker.pylon_install_to_bitcoin.heartbeat_missing',
      ),
      hasRefs(input.heartbeatRefs) ? 'passed' : 'blocked',
    ),
    stepFor(
      'wallet',
      input.walletReadinessRefs,
      ['guard.pylon_install_to_bitcoin.mdk_send_readiness_preflight'],
      blockerIfMissing(
        hasRefs(input.walletReadinessRefs),
        'blocker.pylon_install_to_bitcoin.wallet_readiness_missing',
      ),
      hasRefs(input.walletReadinessRefs) ? 'passed' : 'blocked',
    ),
    stepFor(
      'assignment',
      input.assignmentRefs,
      ['guard.pylon_install_to_bitcoin.non_stale_assignment_lease'],
      [
        ...blockerIfMissing(
          hasRefs(input.assignmentRefs),
          'blocker.pylon_install_to_bitcoin.assignment_missing',
        ),
        ...blockerIfMissing(
          leaseActive(input),
          'blocker.pylon_install_to_bitcoin.assignment_lease_stale',
        ),
      ],
      hasRefs(input.assignmentRefs) && leaseActive(input)
        ? 'passed'
        : 'blocked',
    ),
    stepFor(
      'closeout',
      [...input.closeoutRefs, ...input.acceptedWorkRefs],
      ['guard.pylon_install_to_bitcoin.accepted_work_closeout_required'],
      blockerIfMissing(
        hasRefs(input.closeoutRefs) && hasRefs(input.acceptedWorkRefs),
        'blocker.pylon_install_to_bitcoin.accepted_closeout_missing',
      ),
      hasRefs(input.closeoutRefs) && hasRefs(input.acceptedWorkRefs)
        ? 'passed'
        : 'blocked',
    ),
    stepFor(
      'payment',
      input.paymentReceiptRefs,
      modeGuardRefs(input),
      paymentBlockers.filter(ref => ref.includes('payment')),
      paymentStateFor(input, paymentBlockers),
      liveReady,
    ),
    stepFor(
      'settlement',
      input.settlementReceiptRefs,
      ['guard.pylon_install_to_bitcoin.public_settlement_receipt_required'],
      paymentBlockers.filter(ref => ref.includes('settlement')),
      settlementStateFor(input, paymentBlockers),
    ),
    stepFor(
      'public_projection',
      input.publicProjectionRefs,
      ['guard.pylon_install_to_bitcoin.public_receipt_projection_required'],
      blockerIfMissing(
        hasRefs(input.publicProjectionRefs),
        'blocker.pylon_install_to_bitcoin.public_projection_missing',
      ),
      hasRefs(input.publicProjectionRefs) ? 'passed' : 'blocked',
    ),
  ]

  const projection = new PylonInstallToBitcoinSmokeProjection({
    amountSats: Math.max(0, Math.trunc(input.amountSats)),
    blockerRefs,
    liveWalletSpendAllowed: liveReady,
    mdkPaymentPlan,
    mode: input.mode,
    paymentClaimAllowed:
      input.mode === 'sandbox_fake_payment'
        ? hasRefs(input.paymentReceiptRefs)
        : liveReady && hasRefs(input.paymentReceiptRefs),
    publicProjectionRefs: uniqueRefs(input.publicProjectionRefs),
    redactionScanPassed: true,
    schemaVersion: PylonInstallToBitcoinSmokeSchemaVersion,
    settledBitcoinClaimAllowed: liveReady,
    settlementClaimAllowed:
      liveReady ||
      (input.mode === 'sandbox_fake_payment' &&
        hasRefs(input.settlementReceiptRefs)),
    smokeBundleRefs: smokeBundleRefsFor(input),
    spendCapSats: Math.max(0, Math.trunc(input.spendCapSats)),
    status: statusFor(input, blockerRefs),
    steps,
  })

  if (
    requiredStepKinds.some(
      kind => !projection.steps.some(step => step.kind === kind),
    )
  ) {
    throw new PylonInstallToBitcoinSmokeUnsafe({
      reason:
        'Pylon install-to-bitcoin smoke projection is missing a required step.',
    })
  }

  if (pylonInstallToBitcoinSmokeHasPrivateMaterial(projection)) {
    throw new PylonInstallToBitcoinSmokeUnsafe({
      reason:
        'Pylon install-to-bitcoin smoke projection contains private or raw payment material.',
    })
  }

  return projection
}
