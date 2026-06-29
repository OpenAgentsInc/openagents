import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord } from './artanis-real-small-bitcoin-assignment-smoke'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  MdkPayoutModeGateProjection,
  localMdkAgentWalletBridgePayoutGate,
} from './mdk-payout-mode-gate'
import { OmniProjectionAudience } from './omni-data-classification'
import {
  publicRefTriggersAgentSecretScanner,
  publicScannerSafeRefs,
} from './public-ref-scanner-safety'

export const PylonV02OmegaReleaseGateCheckKind = S.Literals([
  'agents_openapi_current',
  'artifact_proof_upload',
  'artanis_real_small_bitcoin_assignment',
  'artanis_simulated_assignment',
  'assignment_acceptance_status',
  'forum_update_bridge',
  'hosted_mdk_payout_mode',
  'mdk_adapter_mocked_tests',
  'mdk_runtime_boundary',
  'multi_pylon_paid_work_proof',
  'no_native_mdk_worker_runtime',
  'old_google_cloud_nexus_transition',
  'omega_payout_ledger_migration',
  'operator_dashboard',
  'payment_authority_service',
  'public_receipt_page',
  'pylon_registration_heartbeat',
  'pylon_wallet_readiness',
  'real_two_wallet_mdk_movement',
  'settlement_receipts',
  'simulation_adapter_conformance',
])
export type PylonV02OmegaReleaseGateCheckKind =
  typeof PylonV02OmegaReleaseGateCheckKind.Type

export const PylonV02OmegaReleaseGateCheckStatus = S.Literals([
  'blocked',
  'not_required',
  'passed',
  'pending',
])
export type PylonV02OmegaReleaseGateCheckStatus =
  typeof PylonV02OmegaReleaseGateCheckStatus.Type

export const PylonV02OmegaReleaseGateState = S.Literals([
  'blocked',
  'limited_launcher_release_shipped',
  'ready_for_operator_release_review',
])
export type PylonV02OmegaReleaseGateState =
  typeof PylonV02OmegaReleaseGateState.Type

export class PylonV02OmegaReleaseGateAuthority extends S.Class<PylonV02OmegaReleaseGateAuthority>(
  'PylonV02OmegaReleaseGateAuthority',
)({
  oldGoogleCloudNexusRequired: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class PylonV02OmegaReleaseGateCheck extends S.Class<PylonV02OmegaReleaseGateCheck>(
  'PylonV02OmegaReleaseGateCheck',
)({
  blockerRefs: S.Array(S.String),
  checkKind: PylonV02OmegaReleaseGateCheckKind,
  description: S.String,
  docsRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  issueRefs: S.Array(S.String),
  required: S.Boolean,
  routeRefs: S.Array(S.String),
  status: PylonV02OmegaReleaseGateCheckStatus,
  testRefs: S.Array(S.String),
  title: S.String,
}) {}

export class PylonV02OmegaReleaseGateRecord extends S.Class<PylonV02OmegaReleaseGateRecord>(
  'PylonV02OmegaReleaseGateRecord',
)({
  agentRef: S.String,
  authority: PylonV02OmegaReleaseGateAuthority,
  checks: S.Array(PylonV02OmegaReleaseGateCheck),
  gateRef: S.String,
  releaseRef: S.String,
  runbookRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonV02OmegaReleaseGateProjection extends S.Class<PylonV02OmegaReleaseGateProjection>(
  'PylonV02OmegaReleaseGateProjection',
)({
  agentRef: S.String,
  audience: OmniProjectionAudience,
  blockerRefs: S.Array(S.String),
  canAnnouncePylonV02AcceptedWork: S.Boolean,
  canAnnouncePylonV02Payments: S.Boolean,
  canAnnouncePylonV02Release: S.Boolean,
  canAnnouncePylonV02Settlement: S.Boolean,
  checkCount: S.Number,
  checkRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failedOrPendingRequiredCount: S.Number,
  gateRef: S.String,
  hostedMdkDirectPayoutClaimAllowed: S.Boolean,
  missingRequiredCheckRefs: S.Array(S.String),
  multiPylonObservedDistinctPylonCount: S.Number,
  multiPylonObservedPylonRefs: S.Array(S.String),
  multiPylonPaidWorkProofComplete: S.Boolean,
  multiPylonProofRefs: S.Array(S.String),
  multiPylonRequiredDistinctPylonCount: S.Number,
  oldGoogleCloudNexusRequired: S.Boolean,
  optionalTransitionEvidenceRefs: S.Array(S.String),
  payoutModeGate: MdkPayoutModeGateProjection,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  releaseCreationAllowedByThisRecord: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  releaseRef: S.String,
  requiredCheckCount: S.Number,
  requiredPassedCount: S.Number,
  runbookRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  stageSummaryRefs: S.Array(S.String),
  state: PylonV02OmegaReleaseGateState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class PylonV02OmegaReleaseGateUnsafe extends S.TaggedErrorClass<PylonV02OmegaReleaseGateUnsafe>()(
  'PylonV02OmegaReleaseGateUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_V02_OMEGA_RELEASE_GATE_NO_AUTHORITY: PylonV02OmegaReleaseGateAuthority =
  new PylonV02OmegaReleaseGateAuthority({
    oldGoogleCloudNexusRequired: false,
    providerMutationAllowed: false,
    publicClaimUpgradeAllowed: false,
    releasePublicationAllowed: false,
    settlementMutationAllowed: false,
    walletSpendAllowed: false,
  })

const requiredCheckKinds: ReadonlyArray<PylonV02OmegaReleaseGateCheckKind> = [
  'agents_openapi_current',
  'artifact_proof_upload',
  'artanis_real_small_bitcoin_assignment',
  'artanis_simulated_assignment',
  'assignment_acceptance_status',
  'forum_update_bridge',
  'hosted_mdk_payout_mode',
  'mdk_adapter_mocked_tests',
  'mdk_runtime_boundary',
  'multi_pylon_paid_work_proof',
  'no_native_mdk_worker_runtime',
  'omega_payout_ledger_migration',
  'operator_dashboard',
  'payment_authority_service',
  'public_receipt_page',
  'pylon_registration_heartbeat',
  'pylon_wallet_readiness',
  'real_two_wallet_mdk_movement',
  'settlement_receipts',
  'simulation_adapter_conformance',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#{}-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|command[_-]?output[_-]?raw|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|node[_-]?(telemetry|raw|private)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[_-]?raw|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|telemetry|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|dataset|email|invoice|log|model|node|payment|payload|payout|prompt|provider|record|release|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|raw[_-]?payout[_-]?target|release[_-]?command[_-]?output|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const publicUnsafeRefPattern =
  /(^|[.:/_-])(operator|private|raw|secret)([.:/_-]|$)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const refsForAudience = (
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  const safe = uniqueRefs(refs)

  if (audience === 'operator' || audience === 'private') {
    return publicScannerSafeRefs('evidence.public.pylon_v0_2.omega_gate', safe)
  }

  return publicScannerSafeRefs(
    'evidence.public.pylon_v0_2.omega_gate',
    safe.filter(ref => !publicUnsafeRefPattern.test(ref)),
  )
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      containsProviderSecretMaterial(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PylonV02OmegaReleaseGateUnsafe({
      reason: `${label} contains private, secret, wallet, payment, raw command, raw timestamp, provider, payout target, customer, source, or credential material.`,
    })
  }
}

const assertAuthority = (
  authority: PylonV02OmegaReleaseGateAuthority,
): void => {
  if (
    authority.oldGoogleCloudNexusRequired !== false ||
    authority.providerMutationAllowed !== false ||
    authority.publicClaimUpgradeAllowed !== false ||
    authority.releasePublicationAllowed !== false ||
    authority.settlementMutationAllowed !== false ||
    authority.walletSpendAllowed !== false
  ) {
    throw new PylonV02OmegaReleaseGateUnsafe({
      reason:
        'Pylon v0.2 Omega release gate evidence cannot require old Google Cloud Nexus, mutate providers, publish releases, spend wallet funds, settle payouts, or upgrade public claims.',
    })
  }
}

const assertRecordSafe = (record: PylonV02OmegaReleaseGateRecord): void => {
  assertAuthority(record.authority)

  if (record.agentRef !== 'agent_artanis') {
    throw new PylonV02OmegaReleaseGateUnsafe({
      reason:
        'Pylon v0.2 Omega release gates must be administered by agent_artanis.',
    })
  }

  assertSafeRefs('Pylon v0.2 Omega release gate refs', [
    record.agentRef,
    record.gateRef,
    record.releaseRef,
    ...record.runbookRefs,
    ...record.checks.flatMap(check => [
      check.checkKind,
      check.status,
      ...check.blockerRefs,
      ...check.docsRefs,
      ...check.evidenceRefs,
      ...check.issueRefs,
      ...check.routeRefs,
      ...check.testRefs,
    ]),
  ])

  if (
    stringValues({ ...record, updatedAtIso: 'redacted' }).some(
      value =>
        containsProviderSecretMaterial(value) ||
        rawTimestampPattern.test(value),
    )
  ) {
    throw new PylonV02OmegaReleaseGateUnsafe({
      reason:
        'Pylon v0.2 Omega release gate records cannot expose provider secret material or raw timestamps outside timestamp fields.',
    })
  }
}

const checkStatusRef = (check: PylonV02OmegaReleaseGateCheck): string =>
  `gate.public.pylon_v0_2.omega.${check.checkKind}.${check.status}`

const missingCheckRef = (
  checkKind: PylonV02OmegaReleaseGateCheckKind,
): string => `missing.public.pylon_v0_2.omega_gate.${checkKind}`

const requiredChecks = (
  record: PylonV02OmegaReleaseGateRecord,
): ReadonlyArray<PylonV02OmegaReleaseGateCheck> =>
  record.checks.filter(check => check.required)

const missingRequiredCheckRefs = (
  record: PylonV02OmegaReleaseGateRecord,
): ReadonlyArray<string> => {
  const present = new Set(requiredChecks(record).map(check => check.checkKind))

  return requiredCheckKinds
    .filter(checkKind => !present.has(checkKind))
    .map(missingCheckRef)
}

const failedRequiredChecks = (
  record: PylonV02OmegaReleaseGateRecord,
): ReadonlyArray<PylonV02OmegaReleaseGateCheck> =>
  requiredChecks(record).filter(check => check.status !== 'passed')

const multiPylonRequiredDistinctPylonCount = 2
const publicPylonRefPattern = /^pylon\.public\.[A-Za-z0-9_.:/#{}-]+$/
const publicReceiptRefPattern =
  /^receipt\.[A-Za-z0-9_.:/#{}-]*settlement[A-Za-z0-9_.:/#{}-]*$/

const multiPylonCheck = (
  record: PylonV02OmegaReleaseGateRecord,
): PylonV02OmegaReleaseGateCheck | undefined =>
  record.checks.find(check => check.checkKind === 'multi_pylon_paid_work_proof')

const multiPylonObservedPylonRefs = (
  check: PylonV02OmegaReleaseGateCheck | undefined,
): ReadonlyArray<string> =>
  uniqueRefs(
    (check?.evidenceRefs ?? []).filter(ref => publicPylonRefPattern.test(ref)),
  )

const multiPylonReceiptRefs = (
  check: PylonV02OmegaReleaseGateCheck | undefined,
): ReadonlyArray<string> =>
  uniqueRefs(
    (check?.evidenceRefs ?? []).filter(ref =>
      publicReceiptRefPattern.test(ref),
    ),
  )

const multiPylonDefectRefs = (
  check: PylonV02OmegaReleaseGateCheck | undefined,
): ReadonlyArray<string> => {
  if (check === undefined || check.status !== 'passed') {
    return []
  }

  const pylonRefs = multiPylonObservedPylonRefs(check)
  const receiptRefs = multiPylonReceiptRefs(check)
  const simulationOnly = check.evidenceRefs.some(ref => /simulation/i.test(ref))

  return uniqueRefs([
    ...(pylonRefs.length < multiPylonRequiredDistinctPylonCount
      ? ['blocker.public.pylon_v0_2.multi_pylon.distinct_pylon_count_missing']
      : []),
    ...(receiptRefs.length < multiPylonRequiredDistinctPylonCount
      ? ['blocker.public.pylon_v0_2.multi_pylon.terminal_settlement_missing']
      : []),
    ...(simulationOnly
      ? ['blocker.public.pylon_v0_2.multi_pylon.simulation_only']
      : []),
  ])
}

const multiPylonProofComplete = (
  check: PylonV02OmegaReleaseGateCheck | undefined,
): boolean =>
  check?.status === 'passed' && multiPylonDefectRefs(check).length === 0

const publicProjectionStrings = (value: unknown): ReadonlyArray<string> =>
  stringValues(value)

const assertProjectionSafe = (
  projection: PylonV02OmegaReleaseGateProjection,
): void => {
  const unsafe = publicProjectionStrings(projection).find(
    value =>
      containsProviderSecretMaterial(value) ||
      publicRefTriggersAgentSecretScanner(value) ||
      unsafeRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

  if (unsafe !== undefined) {
    throw new PylonV02OmegaReleaseGateUnsafe({
      reason:
        'Pylon v0.2 Omega release gate projection contains private, secret, wallet, payment, raw command, raw timestamp, provider, payout target, customer, source, or credential material.',
    })
  }
}

export const projectPylonV02OmegaReleaseGate = (
  record: PylonV02OmegaReleaseGateRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonV02OmegaReleaseGateProjection => {
  assertRecordSafe(record)

  const missingRefs = missingRequiredCheckRefs(record)
  const failedChecks = failedRequiredChecks(record)
  const multiPylon = multiPylonCheck(record)
  const multiPylonDefects = multiPylonDefectRefs(multiPylon)
  const multiPylonPylonRefs = multiPylonObservedPylonRefs(multiPylon)
  const multiPylonProofRefs = refsForAudience(
    multiPylon?.evidenceRefs ?? [],
    audience,
  )
  const isMultiPylonProofComplete = multiPylonProofComplete(multiPylon)
  const requiredPassedCount = requiredChecks(record).filter(
    check => check.status === 'passed',
  ).length
  const requiredCheckCount = requiredCheckKinds.length
  const failedOrPendingRequiredCount =
    failedChecks.length + missingRefs.length + multiPylonDefects.length
  const releaseGatePassed = failedOrPendingRequiredCount === 0
  const payoutModeGate = localMdkAgentWalletBridgePayoutGate({
    operatorApproved: true,
    sendReady: true,
    walletHomeMode: 'original_funded_wallet_home',
  })
  const evidenceRefs = refsForAudience(
    record.checks.flatMap(check => check.evidenceRefs),
    audience,
  )
  const blockerRefs = uniqueRefs([
    ...missingRefs,
    ...multiPylonDefects,
    ...failedChecks.flatMap(check => check.blockerRefs),
  ])
  const optionalTransitionEvidenceRefs = refsForAudience(
    record.checks
      .filter(check => !check.required)
      .flatMap(check => check.evidenceRefs),
    audience,
  )
  const projection = new PylonV02OmegaReleaseGateProjection({
    agentRef: record.agentRef,
    audience,
    blockerRefs,
    canAnnouncePylonV02AcceptedWork: releaseGatePassed,
    canAnnouncePylonV02Payments:
      releaseGatePassed && payoutModeGate.livePayoutClaimAllowed,
    canAnnouncePylonV02Release: releaseGatePassed,
    canAnnouncePylonV02Settlement: releaseGatePassed,
    checkCount: record.checks.length,
    checkRefs: uniqueRefs(record.checks.map(checkStatusRef)),
    evidenceRefs,
    failedOrPendingRequiredCount,
    gateRef: record.gateRef,
    hostedMdkDirectPayoutClaimAllowed:
      payoutModeGate.hostedDirectPayoutClaimAllowed,
    missingRequiredCheckRefs: missingRefs,
    multiPylonObservedDistinctPylonCount: multiPylonPylonRefs.length,
    multiPylonObservedPylonRefs: refsForAudience(multiPylonPylonRefs, audience),
    multiPylonPaidWorkProofComplete: isMultiPylonProofComplete,
    multiPylonProofRefs,
    multiPylonRequiredDistinctPylonCount,
    oldGoogleCloudNexusRequired: record.authority.oldGoogleCloudNexusRequired,
    optionalTransitionEvidenceRefs,
    payoutModeGate,
    providerMutationAllowed: record.authority.providerMutationAllowed,
    publicClaimUpgradeAllowed: record.authority.publicClaimUpgradeAllowed,
    releaseCreationAllowedByThisRecord: false,
    releasePublicationAllowed: record.authority.releasePublicationAllowed,
    releaseRef: record.releaseRef,
    requiredCheckCount,
    requiredPassedCount,
    runbookRefs: refsForAudience(record.runbookRefs, audience),
    settlementMutationAllowed: record.authority.settlementMutationAllowed,
    stageSummaryRefs: uniqueRefs(record.checks.map(checkStatusRef)),
    state: releaseGatePassed ? 'limited_launcher_release_shipped' : 'blocked',
    stateLabel: releaseGatePassed
      ? 'Pylon v0.2 package launcher is shipped with listed platform and authority limits'
      : 'Pylon v0.2 OpenAgents Nexus release gate is blocked',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletSpendAllowed: record.authority.walletSpendAllowed,
  })

  assertProjectionSafe(projection)

  return projection
}

const passed = (
  checkKind: PylonV02OmegaReleaseGateCheckKind,
  title: string,
  description: string,
  refs: {
    blockerRefs?: ReadonlyArray<string>
    docsRefs?: ReadonlyArray<string>
    evidenceRefs?: ReadonlyArray<string>
    issueRefs?: ReadonlyArray<string>
    routeRefs?: ReadonlyArray<string>
    testRefs?: ReadonlyArray<string>
  },
): PylonV02OmegaReleaseGateCheck =>
  new PylonV02OmegaReleaseGateCheck({
    blockerRefs: [...(refs.blockerRefs ?? [])],
    checkKind,
    description,
    docsRefs: [...(refs.docsRefs ?? [])],
    evidenceRefs: [...(refs.evidenceRefs ?? [])],
    issueRefs: [...(refs.issueRefs ?? [])],
    required: true,
    routeRefs: [...(refs.routeRefs ?? [])],
    status: 'passed',
    testRefs: [...(refs.testRefs ?? [])],
    title,
  })

const optional = (
  checkKind: PylonV02OmegaReleaseGateCheckKind,
  title: string,
  description: string,
  refs: {
    docsRefs?: ReadonlyArray<string>
    evidenceRefs?: ReadonlyArray<string>
    issueRefs?: ReadonlyArray<string>
    routeRefs?: ReadonlyArray<string>
    testRefs?: ReadonlyArray<string>
  },
): PylonV02OmegaReleaseGateCheck =>
  new PylonV02OmegaReleaseGateCheck({
    blockerRefs: [],
    checkKind,
    description,
    docsRefs: [...(refs.docsRefs ?? [])],
    evidenceRefs: [...(refs.evidenceRefs ?? [])],
    issueRefs: [...(refs.issueRefs ?? [])],
    required: false,
    routeRefs: [...(refs.routeRefs ?? [])],
    status: 'not_required',
    testRefs: [...(refs.testRefs ?? [])],
    title,
  })

export const currentPylonV02OmegaReleaseGateRecord =
  (): PylonV02OmegaReleaseGateRecord => {
    const issue438Smoke = issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord()

    return new PylonV02OmegaReleaseGateRecord({
      agentRef: 'agent_artanis',
      authority: PYLON_V02_OMEGA_RELEASE_GATE_NO_AUTHORITY,
      checks: [
        passed(
          'omega_payout_ledger_migration',
          'Omega payout ledger migration deployed',
          'D1 stores payout intents, attempts, reconciliation events, and payment authority receipts.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md',
            ],
            evidenceRefs: [
              'receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
            ],
            issueRefs: ['issue.github.431'],
            testRefs: ['workers/api/src/nexus-treasury-payout-ledger.test.ts'],
          },
        ),
        passed(
          'payment_authority_service',
          'Payment authority service deployed',
          'TreasuryPaymentAuthority enforces readiness, spend caps, approval, idempotency, and dispatch receipts.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-artanis-payment-backed-dispatch-gates.md',
            ],
            evidenceRefs: [
              'authority.public.treasury_payment_authority.deployed',
            ],
            issueRefs: ['issue.github.428'],
            testRefs: ['workers/api/src/treasury-payment-authority.test.ts'],
          },
        ),
        passed(
          'simulation_adapter_conformance',
          'Simulation adapter conformance green',
          'The fake/simulation payout adapter still satisfies the payout adapter contract.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-nexus-simulation-payout-adapter.md',
            ],
            evidenceRefs: [
              'test.public.treasury_payment_adapter_conformance.simulation',
            ],
            issueRefs: ['issue.github.427'],
            testRefs: [
              'workers/api/src/treasury-payment-adapter-conformance.test-support.ts',
              'workers/api/src/pylon-marketplace-payout-flow.test.ts',
            ],
          },
        ),
        passed(
          'mdk_adapter_mocked_tests',
          'MDK adapter mocked tests green',
          'The MDK agent-wallet adapter passes mocked readiness, dispatch, idempotency, and reconciliation coverage.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-mdk-agent-wallet-payout-adapter-runbook.md',
            ],
            evidenceRefs: ['test.public.mdk_agent_wallet_adapter.mocked_green'],
            issueRefs: ['issue.github.431'],
            testRefs: [
              'workers/api/src/treasury-payment-mdk-agent-wallet-adapter.test.ts',
            ],
          },
        ),
        passed(
          'mdk_runtime_boundary',
          'Live MDK runtime boundary explicit',
          'Issue 434 selected a Worker-safe MDK-compatible route boundary with native node-control behavior kept outside Worker code.',
          {
            docsRefs: [
              'docs/mdk/2026-06-07-moneydevkit-local-source-audit.md',
              'docs/mdk/2026-06-07-omega-mdk-setup-audit.md',
            ],
            evidenceRefs: [
              'runtime.public.mdk.worker_safe_route_boundary.issue_434',
              'runtime.public.mdk.native_node_control.outside_worker',
            ],
            issueRefs: ['issue.github.434'],
            testRefs: [
              'workers/api/src/hosted-mdk-client.test.ts',
              'workers/api/src/site-commerce-routes.test.ts',
              'workers/api/src/site-checkout-return.test.ts',
              'workers/api/src/site-mdk-webhooks.test.ts',
            ],
          },
        ),
        passed(
          'hosted_mdk_payout_mode',
          'Hosted MDK payout mode declared',
          'Hosted MDK direct programmatic payout remains disabled; Pylon settlement evidence is explicitly scoped to the local MDK agent-wallet bridge.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-pylon-accepted-work-payout-hosted-mdk-smoke.md',
              'docs/nexus/2026-06-08-mdk-agent-wallet-send-readiness-preflight.md',
            ],
            evidenceRefs: [
              'blocker.mdk.hosted_programmatic_payouts_disabled',
              'evidence.mdk_agent_wallet.local_bridge_authority_recorded',
              'evidence.mdk_agent_wallet.send_readiness_preflight_ready',
              'evidence.mdk_agent_wallet.bridge_material_redaction_checked',
            ],
            issueRefs: ['issue.github.556'],
            testRefs: [
              'workers/api/src/mdk-payout-mode-gate.test.ts',
              'workers/api/src/mdk-agent-wallet-smoke-fixture.test.ts',
              'workers/api/src/pylon-v02-omega-release-gate.test.ts',
            ],
          },
        ),
        passed(
          'no_native_mdk_worker_runtime',
          'No native MDK node runtime in Worker',
          'Omega Worker code must not import native lightning-js or host createMoneyDevKitNode directly.',
          {
            docsRefs: ['docs/mdk/2026-06-07-moneydevkit-local-source-audit.md'],
            evidenceRefs: [
              'policy.public.no_native_mdk_lightning_js_in_worker',
            ],
            issueRefs: ['issue.github.432', 'issue.github.434'],
            testRefs: [
              'workers/api/src/treasury-payment-mdk-agent-wallet-adapter.test.ts',
            ],
          },
        ),
        passed(
          'real_two_wallet_mdk_movement',
          'Real two-wallet MDK movement proof green',
          'The issue 431 smoke moved a small amount of bitcoin between isolated MDK wallets through Omega authority and public-safe receipts.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md',
            ],
            evidenceRefs: [
              'receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
            ],
            issueRefs: ['issue.github.431'],
            routeRefs: [
              'route:/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
              'route:/api/public/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
            ],
          },
        ),
        passed(
          'pylon_registration_heartbeat',
          'Pylon registration and heartbeat green',
          'Registered agents can register and heartbeat Pylon refs through the Omega API.',
          {
            docsRefs: [
              'docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md',
            ],
            evidenceRefs: [
              'api.public.pylon.registration_heartbeat',
              'npm.package.openagentsinc_pylon.0_2_5',
              'pylon.issue505.npm.20260608035130',
              'pylon.issue505.archnpm.20260608035227',
            ],
            issueRefs: ['issue.github.420', 'issue.github.505'],
            routeRefs: [
              'route:/api/pylons',
              'route:/api/pylons/{pylonRef}/heartbeat',
            ],
            testRefs: ['workers/api/src/pylon-api-routes.test.ts'],
          },
        ),
        passed(
          'pylon_wallet_readiness',
          'Pylon wallet readiness green',
          'Pylon wallet readiness records use public-safe readiness buckets and do not expose exact balances or wallet material.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-mdk-two-wallet-smoke-prerequisites.md',
              'docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md',
            ],
            evidenceRefs: [
              'wallet_readiness.public.bucketed.minimum_satisfied',
              'wallet_readiness.public.issue505.local_npm_launcher.ready',
              'wallet_readiness.public.issue505.arch_linux_npm_launcher.ready',
            ],
            issueRefs: ['issue.github.436', 'issue.github.505'],
            routeRefs: ['route:/api/pylons/{pylonRef}/wallet-readiness'],
            testRefs: [
              'workers/api/src/pylon-wallet-liquidity-readiness.test.ts',
            ],
          },
        ),
        passed(
          'assignment_acceptance_status',
          'Assignment acceptance and status green',
          'Pylon marketplace job and provider lifecycle contracts retain assignment, acceptance, progress, and status evidence.',
          {
            evidenceRefs: [
              'assignment.public.pylon_marketplace.lifecycle_tested',
            ],
            issueRefs: ['issue.github.421'],
            testRefs: [
              'workers/api/src/pylon-marketplace-jobs.test.ts',
              'workers/api/src/pylon-provider-job-lifecycle.test.ts',
            ],
          },
        ),
        passed(
          'artifact_proof_upload',
          'Artifact and proof upload green',
          'Pylon assignment records can reference public-safe artifacts and proof manifests without storing private payloads.',
          {
            evidenceRefs: [
              'artifact.public.pylon_assignment.redacted_manifest',
            ],
            issueRefs: ['issue.github.421'],
            testRefs: [
              'workers/api/src/pylon-marketplace-jobs.test.ts',
              'workers/api/src/pylon-accepted-work-proof-links.test.ts',
            ],
          },
        ),
        passed(
          'settlement_receipts',
          'Settlement receipts green',
          'Settlement receipts distinguish dispatch acceptance, terminal result, and public-safe bitcoin movement evidence.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-nexus-pylon-visibility-runbook.md',
              'docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md',
              'docs/nexus/2026-06-07-artanis-real-small-bitcoin-assignment-smoke-evidence.md',
            ],
            evidenceRefs: [
              'receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
              issue438Smoke.settlementReceiptRef,
            ],
            issueRefs: [
              'issue.github.429',
              'issue.github.431',
              'issue.github.438',
            ],
            testRefs: [
              'workers/api/src/nexus-pylon-visibility-routes.test.ts',
              'workers/api/src/pylon-settlement-bridge.test.ts',
            ],
          },
        ),
        passed(
          'artanis_simulated_assignment',
          'Artanis simulated assignment green',
          'Artanis can model and publish evidence for Nexus/Pylon assignment, progress, payout, and settlement lifecycle events in simulation.',
          {
            docsRefs: ['docs/artanis/2026-06-07-nexus-pylon-forum-bridge.md'],
            evidenceRefs: [
              'simulation.public.artanis_nexus_pylon_assignment.green',
            ],
            issueRefs: ['issue.github.408', 'issue.github.430'],
            testRefs: [
              'workers/api/src/artanis-nexus-pylon-adapters.test.ts',
              'workers/api/src/artanis-nexus-pylon-forum-bridge.test.ts',
            ],
          },
        ),
        passed(
          'artanis_real_small_bitcoin_assignment',
          'Artanis real small-bitcoin assignment green',
          'Issue 438 retained an Artanis-administered small-bitcoin assignment through assignment, accepted work, artifact proof, payment authority, reconciliation, settlement receipt, and Forum update intent refs.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-artanis-real-small-bitcoin-assignment-smoke-evidence.md',
              'docs/nexus/2026-06-07-mdk-two-wallet-smoke-evidence.md',
            ],
            evidenceRefs: [
              issue438Smoke.smokeRef,
              issue438Smoke.assignmentRef,
              ...issue438Smoke.acceptedWorkRefs,
              ...issue438Smoke.artifactProofRefs,
              issue438Smoke.payoutTargetApprovalRef,
              issue438Smoke.payoutIntentRef,
              issue438Smoke.payoutAttemptRef,
              issue438Smoke.reconciliationEventRef,
              issue438Smoke.paymentAuthorityReceiptRef,
              issue438Smoke.settlementReceiptRef,
              ...issue438Smoke.duplicateDispatchEvidenceRefs,
              ...issue438Smoke.forumUpdateRefs,
            ],
            issueRefs: ['issue.github.438'],
            routeRefs: [
              issue438Smoke.receiptPageRouteRef,
              issue438Smoke.receiptApiRouteRef,
            ],
            testRefs: [
              'workers/api/src/artanis-real-small-bitcoin-assignment-smoke.test.ts',
              'workers/api/src/pylon-v02-omega-release-gate.test.ts',
              'workers/api/src/nexus-pylon-visibility-routes.test.ts',
            ],
          },
        ),
        passed(
          'multi_pylon_paid_work_proof',
          'Multi-Pylon paid-work proof green',
          'At least two distinct Pylons now have complete paid-work proof traces with terminal public settlement receipt evidence.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-artanis-pylon-proof-trace-checker.md',
              'docs/nexus/2026-06-07-artanis-pylon-operator-proof-run.md',
              'docs/nexus/2026-06-07-pylon-v02-omega-release-gate-runbook.md',
              'docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md',
            ],
            evidenceRefs: [
              issue438Smoke.assignmentRef,
              issue438Smoke.pylonRef,
              issue438Smoke.settlementReceiptRef,
              'artanis-mdk-bridge-8b378373002501f3e896dcd3',
              'pylon.public.artanis.bridge.8b378373',
              'receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
              'proof.public.mdk_agent_wallet.real_bitcoin_moved.8b378373002501f3e896dcd3',
            ],
            issueRefs: ['issue.github.487', 'issue.github.505'],
            routeRefs: ['route:/api/operator/nexus-pylon/proof-runs'],
            testRefs: [
              'workers/api/src/artanis-pylon-proof-trace.test.ts',
              'workers/api/src/pylon-v02-omega-release-gate.test.ts',
            ],
          },
        ),
        passed(
          'public_receipt_page',
          'Public-safe receipt page green',
          'The live public receipt page/API serves persisted issue 431 and issue 438 real-bitcoin projections without private payment material.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-artanis-real-small-bitcoin-assignment-smoke-evidence.md',
            ],
            evidenceRefs: [
              'receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
              issue438Smoke.settlementReceiptRef,
            ],
            issueRefs: [
              'issue.github.429',
              'issue.github.431',
              'issue.github.438',
            ],
            routeRefs: [
              'route:/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
              'route:/api/public/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
              issue438Smoke.receiptPageRouteRef,
              issue438Smoke.receiptApiRouteRef,
            ],
            testRefs: ['workers/api/src/nexus-pylon-visibility-routes.test.ts'],
          },
        ),
        passed(
          'operator_dashboard',
          'Operator dashboard green',
          'Operators can inspect Nexus/Pylon payout and receipt state through the Omega operator surface.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-nexus-pylon-visibility-runbook.md',
            ],
            evidenceRefs: ['operator.public.nexus_pylon_visibility.dashboard'],
            issueRefs: ['issue.github.429'],
            routeRefs: [
              'route:/api/operator/nexus-pylon/receipts/{receiptRef}',
            ],
            testRefs: ['workers/api/src/nexus-pylon-visibility-routes.test.ts'],
          },
        ),
        passed(
          'forum_update_bridge',
          'Forum update bridge green',
          'Artanis/Nexus/Pylon lifecycle events map to public-safe Forum intents and the Pylon release work-log topic.',
          {
            docsRefs: ['docs/artanis/2026-06-07-nexus-pylon-forum-bridge.md'],
            evidenceRefs: [
              'topic.public.forum.artanis.pylon_release_work_log',
              ...issue438Smoke.forumUpdateRefs,
            ],
            issueRefs: ['issue.github.430', 'issue.github.438'],
            routeRefs: ['route:/forum/t/88888888-4004-4004-8004-888888888888'],
            testRefs: [
              'workers/api/src/artanis-nexus-pylon-forum-bridge.test.ts',
            ],
          },
        ),
        passed(
          'agents_openapi_current',
          'AGENTS.md and OpenAPI current',
          'The public agent instructions and OpenAPI describe current Pylon, Nexus/Pylon receipt, Forum, Sites, and MDK boundaries.',
          {
            docsRefs: ['docs/live/AGENTS.md'],
            evidenceRefs: ['doc.public.openagents.agents_md.current'],
            issueRefs: ['issue.github.432'],
            routeRefs: ['route:/AGENTS.md', 'route:/api/openapi.json'],
            testRefs: [
              'workers/api/src/openagents-agent-onboarding-routes.test.ts',
            ],
          },
        ),
        optional(
          'old_google_cloud_nexus_transition',
          'Old Google Cloud Nexus transition evidence',
          'Old Google Cloud Nexus health is transition context only and is not required for normal Pylon v0.2 release classification.',
          {
            docsRefs: [
              'docs/nexus/2026-06-07-nexus-google-cloud-friction-and-cloudflare-effect-rebuild-audit.md',
            ],
            evidenceRefs: [
              'transition.public.old_google_cloud_nexus.not_release_gate',
            ],
            issueRefs: ['issue.github.432'],
          },
        ),
      ],
      gateRef: 'gate.public.pylon_v0_2.omega_nexus_release',
      releaseRef: 'release.public.openagents.pylon_npm_launcher_0_2_5',
      runbookRefs: [
        'docs/nexus/2026-06-07-pylon-v02-omega-release-gate-runbook.md',
        'docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md',
      ],
      updatedAtIso: '2026-06-08T03:58:00.000Z',
    })
  }

export const readyPylonV02OmegaReleaseGateRecord =
  (): PylonV02OmegaReleaseGateRecord =>
    new PylonV02OmegaReleaseGateRecord({
      ...currentPylonV02OmegaReleaseGateRecord(),
      checks: currentPylonV02OmegaReleaseGateRecord().checks.map(check =>
        check.checkKind === 'multi_pylon_paid_work_proof'
          ? passed(
              'multi_pylon_paid_work_proof',
              'Multi-Pylon paid-work proof green',
              'At least two distinct Pylons have complete paid-work proof traces with terminal settlement receipt evidence.',
              {
                docsRefs: check.docsRefs,
                evidenceRefs: check.evidenceRefs,
                issueRefs: ['issue.github.487'],
                routeRefs: check.routeRefs,
                testRefs: check.testRefs,
              },
            )
          : check,
      ),
    })
