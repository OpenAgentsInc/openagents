import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Array as Arr, Schema as S } from 'effect'

import type { PylonGepaMetricCallCoordinatorImport } from './pylon-gepa-metric-call-assignments'

export const ProbeGepaStage0NoSpendCampaignState = S.Literals([
  'blocked',
  'green',
])
export type ProbeGepaStage0NoSpendCampaignState =
  typeof ProbeGepaStage0NoSpendCampaignState.Type

export const ProbeGepaStage0NoSpendDashboardState = S.Literals([
  'blocked',
  'green',
])
export type ProbeGepaStage0NoSpendDashboardState =
  typeof ProbeGepaStage0NoSpendDashboardState.Type

export const ProbeGepaStage0NoSpendCampaignProjection = S.Struct({
  acceptedCloseoutRefs: S.Array(S.String),
  acceptedCount: S.Number,
  artifactRefs: S.Array(S.String),
  artanisSummaryRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  campaignRef: S.String,
  caveatRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  dashboardState: ProbeGepaStage0NoSpendDashboardState,
  modelTrainingClaimAllowed: S.Boolean,
  noSpendCampaignClaimAllowed: S.Boolean,
  noSpendEvidenceRefs: S.Array(S.String),
  paidCampaignClaimAllowed: S.Boolean,
  paidModesBlocked: S.Boolean,
  probeCloseoutImportRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  psionicImportDryRunRefs: S.Array(S.String),
  publicSafeBundleRefs: S.Array(S.String),
  pylonRefs: S.Array(S.String),
  rejectedCloseoutRefs: S.Array(S.String),
  rejectedCount: S.Number,
  requestedPaymentMode: S.Literal('unpaid_smoke'),
  resourceUsageRefs: S.Array(S.String),
  runtimeCandidateActivationAllowed: S.Boolean,
  settlementClaimAllowed: S.Boolean,
  state: ProbeGepaStage0NoSpendCampaignState,
  terminalBenchScoreClaimAllowed: S.Boolean,
  verifierResultRefs: S.Array(S.String),
})
export type ProbeGepaStage0NoSpendCampaignProjection =
  typeof ProbeGepaStage0NoSpendCampaignProjection.Type

export type ProbeGepaStage0NoSpendCampaignInput = Readonly<{
  artanisSummaryRefs?: ReadonlyArray<string> | undefined
  campaignRef: string
  coordinatorImports: ReadonlyArray<PylonGepaMetricCallCoordinatorImport>
  probeCloseoutImportRefs?: ReadonlyArray<string> | undefined
  psionicImportDryRunRefs?: ReadonlyArray<string> | undefined
  requestedPaymentMode?: 'unpaid_smoke' | undefined
}>

export class ProbeGepaStage0NoSpendCampaignUnsafe extends S.TaggedErrorClass<ProbeGepaStage0NoSpendCampaignUnsafe>()(
  'ProbeGepaStage0NoSpendCampaignUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeProjection = S.decodeUnknownSync(
  ProbeGepaStage0NoSpendCampaignProjection,
)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|trace|wallet)|prompt[_-]?(raw|text|full)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|fixture|invoice|log|payment|payload|prompt|provider|repo|runner|run[_-]?log|source|state|telemetry|text|trace|training|usage|webhook|weights)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|terminal_bench_score|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(
    ref =>
      !safeRefPattern.test(ref) ||
      containsProviderSecretMaterial(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ProbeGepaStage0NoSpendCampaignUnsafe({
      reason: `${label} must be public-safe refs without raw benchmark data, raw prompts, raw traces, provider payloads, customer data, wallet/payment material, model weights, private repo/source refs, secrets, or timestamps.`,
    })
  }

  return normalized
}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  Arr.isReadonlyArrayNonEmpty(refs)

const refsFromImports = (
  imports: ReadonlyArray<PylonGepaMetricCallCoordinatorImport>,
  selector: (
    coordinatorImport: PylonGepaMetricCallCoordinatorImport,
  ) => ReadonlyArray<string>,
): ReadonlyArray<string> => uniqueRefs(imports.flatMap(selector))

const requiredBlockers = (
  input: Readonly<{
    acceptedCloseoutRefs: ReadonlyArray<string>
    acceptedCount: number
    artifactRefs: ReadonlyArray<string>
    artanisSummaryRefs: ReadonlyArray<string>
    assignmentRefs: ReadonlyArray<string>
    closeoutRefs: ReadonlyArray<string>
    noSpendEvidenceRefs: ReadonlyArray<string>
    probeCloseoutImportRefs: ReadonlyArray<string>
    proofBundleRefs: ReadonlyArray<string>
    psionicImportDryRunRefs: ReadonlyArray<string>
    pylonRefs: ReadonlyArray<string>
    rejectedCloseoutRefs: ReadonlyArray<string>
    rejectedCount: number
    resourceUsageRefs: ReadonlyArray<string>
    verifierResultRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(input.pylonRefs.length < 2
    ? ['blocker.public.probe_gepa_stage0.multiple_pylons_missing']
    : []),
  ...(!hasRefs(input.assignmentRefs)
    ? ['blocker.public.probe_gepa_stage0.assignment_refs_missing']
    : []),
  ...(input.acceptedCount === 0
    ? ['blocker.public.probe_gepa_stage0.accepted_closeout_missing']
    : []),
  ...(input.rejectedCount === 0
    ? ['blocker.public.probe_gepa_stage0.rejected_closeout_missing']
    : []),
  ...(!hasRefs(input.closeoutRefs)
    ? ['blocker.public.probe_gepa_stage0.closeout_refs_missing']
    : []),
  ...(!hasRefs(input.probeCloseoutImportRefs)
    ? ['blocker.public.probe_gepa_stage0.probe_closeout_import_missing']
    : []),
  ...(!hasRefs(input.acceptedCloseoutRefs)
    ? ['blocker.public.probe_gepa_stage0.accepted_refs_missing']
    : []),
  ...(!hasRefs(input.rejectedCloseoutRefs)
    ? ['blocker.public.probe_gepa_stage0.rejection_refs_missing']
    : []),
  ...(!hasRefs(input.artifactRefs)
    ? ['blocker.public.probe_gepa_stage0.artifact_refs_missing']
    : []),
  ...(!hasRefs(input.proofBundleRefs)
    ? ['blocker.public.probe_gepa_stage0.proof_bundle_refs_missing']
    : []),
  ...(!hasRefs(input.noSpendEvidenceRefs)
    ? ['blocker.public.probe_gepa_stage0.no_spend_evidence_missing']
    : []),
  ...(!hasRefs(input.resourceUsageRefs)
    ? ['blocker.public.probe_gepa_stage0.resource_usage_refs_missing']
    : []),
  ...(!hasRefs(input.verifierResultRefs)
    ? ['blocker.public.probe_gepa_stage0.verifier_result_refs_missing']
    : []),
  ...(!hasRefs(input.psionicImportDryRunRefs)
    ? ['blocker.public.probe_gepa_stage0.psionic_import_dry_run_missing']
    : []),
  ...(!hasRefs(input.artanisSummaryRefs)
    ? ['blocker.public.probe_gepa_stage0.artanis_summary_missing']
    : []),
]

const caveatRefs = [
  'caveat.public.probe_gepa_stage0.no_spend_modes_only',
  'caveat.public.probe_gepa_stage0.no_settlement_copy',
  'caveat.public.probe_gepa_stage0.no_public_benchmark_score_claim',
  'caveat.public.probe_gepa_stage0.no_model_training_claim',
  'caveat.public.probe_gepa_stage0.no_runtime_candidate_activation',
]

export const projectProbeGepaStage0NoSpendCampaign = (
  input: ProbeGepaStage0NoSpendCampaignInput,
): ProbeGepaStage0NoSpendCampaignProjection => {
  const campaignRef =
    assertSafeRefs('Stage 0 campaign ref', [input.campaignRef])[0] ??
    'campaign.public.probe_gepa_stage0.redacted'
  const coordinatorImports = input.coordinatorImports
  const requestedPaymentMode = input.requestedPaymentMode ?? 'unpaid_smoke'

  if (requestedPaymentMode !== 'unpaid_smoke') {
    throw new ProbeGepaStage0NoSpendCampaignUnsafe({
      reason: 'Stage 0 campaign requests must use unpaid_smoke mode.',
    })
  }

  coordinatorImports.forEach(coordinatorImport => {
    const allowedTerminalNoSpendMode =
      (coordinatorImport.closeoutDecision === 'accepted' &&
        coordinatorImport.paymentMode === 'unpaid_smoke') ||
      (coordinatorImport.closeoutDecision === 'rejected' &&
        coordinatorImport.paymentMode === 'rejected_no_pay')

    if (!allowedTerminalNoSpendMode) {
      throw new ProbeGepaStage0NoSpendCampaignUnsafe({
        reason:
          'Stage 0 no-spend campaigns require unpaid_smoke accepted imports and rejected_no_pay rejected imports.',
      })
    }

    if (
      hasRefs(coordinatorImport.paymentReceiptRefs) ||
      hasRefs(coordinatorImport.settlementReceiptRefs) ||
      coordinatorImport.payableWorkClaimAllowed ||
      coordinatorImport.settledBitcoinPayoutClaimAllowed
    ) {
      throw new ProbeGepaStage0NoSpendCampaignUnsafe({
        reason:
          'Stage 0 no-spend campaigns cannot carry payment, payable, or settlement evidence.',
      })
    }

    if (
      coordinatorImport.closeoutDecision === 'accepted' &&
      !coordinatorImport.acceptedWorkClaimAllowed
    ) {
      throw new ProbeGepaStage0NoSpendCampaignUnsafe({
        reason:
          'Accepted Stage 0 closeouts require accepted-work artifact, proof, closeout, verifier, and resource refs.',
      })
    }
  })

  const assignmentRefs = assertSafeRefs(
    'Stage 0 assignment refs',
    coordinatorImports.map(
      coordinatorImport => coordinatorImport.assignmentRef,
    ),
  )
  const pylonRefs = assertSafeRefs(
    'Stage 0 Pylon refs',
    uniqueRefs(
      coordinatorImports.flatMap(coordinatorImport =>
        coordinatorImport.workerRef === null
          ? []
          : [coordinatorImport.workerRef],
      ),
    ),
  )
  const acceptedImports = coordinatorImports.filter(
    coordinatorImport => coordinatorImport.closeoutDecision === 'accepted',
  )
  const rejectedImports = coordinatorImports.filter(
    coordinatorImport => coordinatorImport.closeoutDecision === 'rejected',
  )
  const acceptedCloseoutRefs = assertSafeRefs(
    'Stage 0 accepted closeout refs',
    refsFromImports(
      acceptedImports,
      coordinatorImport => coordinatorImport.closeoutResultRefs,
    ),
  )
  const rejectedCloseoutRefs = assertSafeRefs(
    'Stage 0 rejected closeout refs',
    refsFromImports(
      rejectedImports,
      coordinatorImport => coordinatorImport.closeoutResultRefs,
    ),
  )
  const closeoutRefs = assertSafeRefs(
    'Stage 0 closeout refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.closeoutResultRefs,
    ),
  )
  const artifactRefs = assertSafeRefs(
    'Stage 0 artifact refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.artifactRefs,
    ),
  )
  const proofBundleRefs = assertSafeRefs(
    'Stage 0 proof bundle refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.proofBundleRefs,
    ),
  )
  const resourceUsageRefs = assertSafeRefs(
    'Stage 0 resource usage refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.resourceUsageRefs,
    ),
  )
  const verifierResultRefs = assertSafeRefs(
    'Stage 0 verifier result refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.verifierResultRefs,
    ),
  )
  const noSpendEvidenceRefs = assertSafeRefs(
    'Stage 0 no-spend evidence refs',
    refsFromImports(
      coordinatorImports,
      coordinatorImport => coordinatorImport.noSpendEvidenceRefs,
    ),
  )
  const probeCloseoutImportRefs = assertSafeRefs(
    'Stage 0 Probe closeout import refs',
    input.probeCloseoutImportRefs ?? [],
  )
  const psionicImportDryRunRefs = assertSafeRefs(
    'Stage 0 Psionic import dry-run refs',
    input.psionicImportDryRunRefs ?? [],
  )
  const artanisSummaryRefs = assertSafeRefs(
    'Stage 0 Artanis summary refs',
    input.artanisSummaryRefs ?? [],
  )
  const blockerRefs = requiredBlockers({
    acceptedCloseoutRefs,
    acceptedCount: acceptedImports.length,
    artifactRefs,
    artanisSummaryRefs,
    assignmentRefs,
    closeoutRefs,
    noSpendEvidenceRefs,
    probeCloseoutImportRefs,
    proofBundleRefs,
    psionicImportDryRunRefs,
    pylonRefs,
    rejectedCloseoutRefs,
    rejectedCount: rejectedImports.length,
    resourceUsageRefs,
    verifierResultRefs,
  })
  const state: ProbeGepaStage0NoSpendCampaignState = hasRefs(blockerRefs)
    ? 'blocked'
    : 'green'
  const publicSafeBundleRefs = assertSafeRefs(
    'Stage 0 public-safe bundle refs',
    [
      campaignRef,
      ...assignmentRefs,
      ...closeoutRefs,
      ...artifactRefs,
      ...proofBundleRefs,
      ...resourceUsageRefs,
      ...verifierResultRefs,
      ...noSpendEvidenceRefs,
      ...probeCloseoutImportRefs,
      ...psionicImportDryRunRefs,
      ...artanisSummaryRefs,
    ],
  )

  return decodeProjection({
    acceptedCloseoutRefs,
    acceptedCount: acceptedImports.length,
    artifactRefs,
    artanisSummaryRefs,
    assignmentRefs,
    blockerRefs,
    campaignRef,
    caveatRefs,
    closeoutRefs,
    dashboardState: state,
    modelTrainingClaimAllowed: false,
    noSpendCampaignClaimAllowed: state === 'green',
    noSpendEvidenceRefs,
    paidCampaignClaimAllowed: false,
    paidModesBlocked: true,
    probeCloseoutImportRefs,
    proofBundleRefs,
    psionicImportDryRunRefs,
    publicSafeBundleRefs,
    pylonRefs,
    rejectedCloseoutRefs,
    rejectedCount: rejectedImports.length,
    requestedPaymentMode,
    resourceUsageRefs,
    runtimeCandidateActivationAllowed: false,
    settlementClaimAllowed: false,
    state,
    terminalBenchScoreClaimAllowed: false,
    verifierResultRefs,
  })
}

export const probeGepaStage0NoSpendCampaignHasPrivateMaterial = (
  projection: ProbeGepaStage0NoSpendCampaignProjection,
): boolean => {
  const publicValues = [
    projection.campaignRef,
    projection.dashboardState,
    projection.requestedPaymentMode,
    projection.state,
    ...projection.acceptedCloseoutRefs,
    ...projection.artifactRefs,
    ...projection.artanisSummaryRefs,
    ...projection.assignmentRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.closeoutRefs,
    ...projection.noSpendEvidenceRefs,
    ...projection.probeCloseoutImportRefs,
    ...projection.proofBundleRefs,
    ...projection.psionicImportDryRunRefs,
    ...projection.publicSafeBundleRefs,
    ...projection.pylonRefs,
    ...projection.rejectedCloseoutRefs,
    ...projection.resourceUsageRefs,
    ...projection.verifierResultRefs,
  ]

  return publicValues.some(
    value =>
      containsProviderSecretMaterial(value) ||
      unsafeRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )
}
