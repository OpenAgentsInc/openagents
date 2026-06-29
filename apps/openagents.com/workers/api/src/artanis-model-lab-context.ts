import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  OmniBenchmarkCloudProjection,
  OmniBenchmarkCloudRecord,
  projectOmniBenchmarkCloud,
} from './omni-model-lab-benchmark-cloud'
import {
  OmniModelArtifactProjection,
  OmniModelArtifactRecord,
  projectOmniModelArtifact,
} from './omni-model-lab-model-artifact'
import {
  OmniModelLabEvidenceGraphProjection,
  OmniModelLabEvidenceGraphRecord,
  projectOmniModelLabEvidenceGraph,
} from './omni-model-lab-evidence-graph'
import {
  OmniModelLabReportProjection,
  OmniModelLabReportRecord,
  projectOmniModelLabReport,
} from './omni-model-lab-report'
import {
  OmniModelLabRetainedFailureLoopProjection,
  OmniModelLabRetainedFailureLoopRecord,
  projectOmniModelLabRetainedFailureLoop,
} from './omni-model-lab-retained-failure-loop'
import {
  OmniPromotionDecisionLedgerRecord,
  OmniPromotionDecisionProjection,
  projectOmniPromotionDecisionLedger,
} from './omni-model-lab-promotion-decision'
import {
  OmniTrainingRunProjection,
  OmniTrainingRunRecord,
  projectOmniTrainingRun,
} from './omni-model-lab-training-run'

export const ArtanisModelLabContextAudience = S.Literals([
  'private_loop',
  'public_artanis',
  'public_forum',
])
export type ArtanisModelLabContextAudience =
  typeof ArtanisModelLabContextAudience.Type

export const ArtanisModelLabContextReadiness = S.Literals([
  'blocked',
  'missing_evidence',
  'partial',
  'ready',
])
export type ArtanisModelLabContextReadiness =
  typeof ArtanisModelLabContextReadiness.Type

export const ArtanisModelLabOperatorActionKind = S.Literals([
  'draft_eval_rerun',
  'draft_public_forum_summary',
  'inspect_retained_failure',
  'request_missing_contracts',
  'request_missing_evidence',
  'request_operator_promotion_review',
])
export type ArtanisModelLabOperatorActionKind =
  typeof ArtanisModelLabOperatorActionKind.Type

export class ArtanisModelLabOperatorActionDraft extends S.Class<ArtanisModelLabOperatorActionDraft>(
  'ArtanisModelLabOperatorActionDraft',
)({
  actionRef: S.String,
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  kind: ArtanisModelLabOperatorActionKind,
  publicReportRefs: S.Array(S.String),
  requiresOperatorApproval: S.Boolean,
  summary: S.String,
  targetRefs: S.Array(S.String),
}) {}

export class ArtanisModelLabContextRecord extends S.Class<ArtanisModelLabContextRecord>(
  'ArtanisModelLabContextRecord',
)({
  agentId: S.String,
  benchmarkCloud: S.NullOr(OmniBenchmarkCloudRecord),
  caveatRefs: S.Array(S.String),
  contextRef: S.String,
  createdAtIso: S.String,
  evidenceGraph: S.NullOr(OmniModelLabEvidenceGraphRecord),
  loopGoalRef: S.String,
  modelArtifacts: S.Array(OmniModelArtifactRecord),
  privateEvidenceRefs: S.Array(S.String),
  promotionDecisionLedger: S.NullOr(OmniPromotionDecisionLedgerRecord),
  publicReport: S.NullOr(OmniModelLabReportRecord),
  retainedFailureLoop: S.NullOr(OmniModelLabRetainedFailureLoopRecord),
  trainingRuns: S.Array(OmniTrainingRunRecord),
  updatedAtIso: S.String,
}) {}

export class ArtanisModelLabContextProjection extends S.Class<ArtanisModelLabContextProjection>(
  'ArtanisModelLabContextProjection',
)({
  agentId: S.String,
  audience: ArtanisModelLabContextAudience,
  benchmarkCloud: S.NullOr(OmniBenchmarkCloudProjection),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  consumedContractRefs: S.Array(S.String),
  contextRef: S.String,
  createdAtDisplay: S.String,
  evidenceGraph: S.NullOr(OmniModelLabEvidenceGraphProjection),
  loopGoalRef: S.String,
  missingContractRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  modelArtifacts: S.Array(OmniModelArtifactProjection),
  operatorNextActions: S.Array(ArtanisModelLabOperatorActionDraft),
  privateEvidenceRefs: S.Array(S.String),
  promotionDecisionLedger: S.NullOr(OmniPromotionDecisionProjection),
  publicForumSummaryReportRefs: S.Array(S.String),
  publicPromotionClaimRefs: S.Array(S.String),
  publicReport: S.NullOr(OmniModelLabReportProjection),
  readiness: ArtanisModelLabContextReadiness,
  retainedFailureLoop: S.NullOr(OmniModelLabRetainedFailureLoopProjection),
  trainingRuns: S.Array(OmniTrainingRunProjection),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisModelLabContextUnsafe extends S.TaggedErrorClass<ArtanisModelLabContextUnsafe>()(
  'ArtanisModelLabContextUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeContextRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|email|fixture|input|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisModelLabContextUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeContextRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisModelLabContextUnsafe({
      reason: `${label} contains private prompts, source archives, datasets, provider payloads, raw artifacts, model weights, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const privateAudience = (
  audience: ArtanisModelLabContextAudience,
): boolean => audience === 'private_loop'

const modelLabAudience = (
  audience: ArtanisModelLabContextAudience,
): 'operator' | 'public' => (privateAudience(audience) ? 'operator' : 'public')

const missingContractsForContext = (
  context: ArtanisModelLabContextRecord,
): ReadonlyArray<string> => {
  const missing: Array<string> = []

  if (context.retainedFailureLoop === null) {
    missing.push('contract.public.model_lab.retained_failure_loop')
  }

  if (context.modelArtifacts.length === 0) {
    missing.push('contract.public.model_lab.model_artifact')
  }

  if (context.trainingRuns.length === 0) {
    missing.push('contract.public.model_lab.training_run')
  }

  if (context.evidenceGraph === null) {
    missing.push('contract.public.model_lab.evidence_graph')
  }

  if (context.benchmarkCloud === null) {
    missing.push('contract.public.model_lab.benchmark_cloud')
  }

  if (context.promotionDecisionLedger === null) {
    missing.push('contract.public.model_lab.promotion_decision')
  }

  if (context.publicReport === null) {
    missing.push('contract.public.model_lab.public_report')
  }

  return missing
}

const consumedContractsForContext = (
  context: ArtanisModelLabContextRecord,
): ReadonlyArray<string> =>
  uniqueRefs([
    context.retainedFailureLoop === null
      ? null
      : 'contract.public.model_lab.retained_failure_loop',
    context.modelArtifacts.length === 0
      ? null
      : 'contract.public.model_lab.model_artifact',
    context.trainingRuns.length === 0
      ? null
      : 'contract.public.model_lab.training_run',
    context.evidenceGraph === null
      ? null
      : 'contract.public.model_lab.evidence_graph',
    context.benchmarkCloud === null
      ? null
      : 'contract.public.model_lab.benchmark_cloud',
    context.promotionDecisionLedger === null
      ? null
      : 'contract.public.model_lab.promotion_decision',
    context.publicReport === null
      ? null
      : 'contract.public.model_lab.public_report',
  ].filter((ref): ref is string => ref !== null))

const allProjectionStrings = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(allProjectionStrings)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(allProjectionStrings)
  }

  return []
}

const projectionHasPrivateMaterial = (value: unknown): boolean =>
  allProjectionStrings(value).some(
    item =>
      unsafeContextRefPattern.test(item) || rawTimestampPattern.test(item),
  )

const errorReason = (error: unknown): string => {
  if (
    typeof error === 'object' &&
    error !== null &&
    'reason' in error &&
    typeof error.reason === 'string'
  ) {
    return error.reason
  }

  if (error instanceof Error && error.message !== '') {
    return error.message
  }

  return 'Model Lab evidence projection failed.'
}

const projectModelLabEvidence = <A>(
  label: string,
  project: () => A,
): A => {
  try {
    return project()
  } catch (error) {
    throw new ArtanisModelLabContextUnsafe({
      reason: `${label} rejected unsafe or invalid evidence: ${errorReason(error)}`,
    })
  }
}

const assertNoFalseAuthority = (
  projection: ArtanisModelLabContextProjection,
): void => {
  const serialized = JSON.stringify(projection)

  for (const forbidden of [
    '"adapterInstallAllowed":true',
    '"adapterInstallationAllowed":true',
    '"benchmarkLaunchAllowed":true',
    '"evalExecutionAllowed":true',
    '"marketplaceRankMutationAllowed":true',
    '"modelDeploymentAllowed":true',
    '"modelTrainingLaunchAllowed":true',
    '"modelTrainingMutationAllowed":true',
    '"modelTrainingStartAllowed":true',
    '"paymentSpendAllowed":true',
    '"payoutMutationAllowed":true',
    '"providerCallAllowed":true',
    '"providerMutationAllowed":true',
    '"publicClaimMutationAllowed":true',
    '"publicClaimUpgradeAllowed":true',
    '"rawArtifactExportAllowed":true',
    '"rawBenchmarkInputCopyAllowed":true',
    '"rawDatasetCopyAllowed":true',
    '"rawWeightCopyAllowed":true',
    '"reportPublicationAllowed":true',
    '"rollbackExecutionAllowed":true',
    '"routeMutationAllowed":true',
    '"routingMutationAllowed":true',
    '"runtimePromotionAllowed":true',
    '"settlementMutationAllowed":true',
    '"trainingLaunchAllowed":true',
  ]) {
    if (serialized.includes(forbidden)) {
      throw new ArtanisModelLabContextUnsafe({
        reason:
          'Artanis Model Lab context cannot expose eval, training, provider, adapter, runtime, route, report-publication, payment, payout, settlement, rollback, raw export, or public-claim mutation authority.',
      })
    }
  }
}

const blockerRefsForContext = (
  context: ArtanisModelLabContextRecord,
  report: OmniModelLabReportProjection | null,
  benchmark: OmniBenchmarkCloudProjection | null,
  promotion: OmniPromotionDecisionProjection | null,
): ReadonlyArray<string> => {
  const blockers = [
    ...missingContractsForContext(context).map(contractRef =>
      `blocker.public.artanis.missing_${contractRef.replace(/^contract\.public\.model_lab\./, '')}`,
    ),
    ...(report?.blockerRefs ?? []),
    ...(report?.readiness === 'blocked'
      ? ['blocker.public.artanis.model_lab_report_blocked']
      : []),
    ...(report?.readiness === 'missing_evidence'
      ? ['blocker.public.artanis.model_lab_missing_evidence']
      : []),
    ...(benchmark?.promotionBlocked === true
      ? ['blocker.public.artanis.benchmark_promotion_blocked']
      : []),
    ...(promotion?.claimState === 'blocked'
      ? ['blocker.public.artanis.promotion_decision_blocked']
      : []),
  ]

  return uniqueRefs(blockers)
}

const missingEvidenceRefsForContext = (
  report: OmniModelLabReportProjection | null,
  evidenceGraph: OmniModelLabEvidenceGraphProjection | null,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...(report?.missingEvidenceRefs ?? []),
    ...(evidenceGraph?.staleEvidenceRefs ?? []),
  ])

const readinessForContext = (
  missingContracts: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
  missingEvidenceRefs: ReadonlyArray<string>,
  report: OmniModelLabReportProjection | null,
): ArtanisModelLabContextReadiness => {
  if (hasAny(blockerRefs)) {
    return 'blocked'
  }

  if (hasAny(missingContracts) || hasAny(missingEvidenceRefs)) {
    return 'missing_evidence'
  }

  if (report === null || report.readiness === 'partial') {
    return 'partial'
  }

  if (report.readiness === 'complete') {
    return 'ready'
  }

  return 'partial'
}

const operatorActionsForContext = (
  context: ArtanisModelLabContextRecord,
  report: OmniModelLabReportProjection | null,
  readiness: ArtanisModelLabContextReadiness,
  missingContractRefs: ReadonlyArray<string>,
  missingEvidenceRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
): ReadonlyArray<ArtanisModelLabOperatorActionDraft> => {
  const actions: Array<ArtanisModelLabOperatorActionDraft> = []
  const reportRefs = report === null ? [] : [report.reportRef]

  if (hasAny(missingContractRefs)) {
    actions.push({
      actionRef: 'action.public.artanis.request_missing_model_lab_contracts',
      blockerRefs,
      evidenceRefs: [],
      kind: 'request_missing_contracts',
      publicReportRefs: reportRefs,
      requiresOperatorApproval: false,
      summary: 'Request the missing Model Lab contracts before claiming progress.',
      targetRefs: missingContractRefs,
    })
  }

  if (hasAny(missingEvidenceRefs) || readiness === 'blocked') {
    actions.push({
      actionRef: 'action.public.artanis.request_model_lab_missing_evidence',
      blockerRefs,
      evidenceRefs: missingEvidenceRefs,
      kind: 'request_missing_evidence',
      publicReportRefs: reportRefs,
      requiresOperatorApproval: false,
      summary: 'Ask the operator or Model Lab owner for missing evidence.',
      targetRefs: uniqueRefs([...missingEvidenceRefs, ...blockerRefs]),
    })
  }

  if (
    context.retainedFailureLoop !== null &&
    context.retainedFailureLoop.evalReruns.length === 0
  ) {
    actions.push({
      actionRef: 'action.public.artanis.draft_eval_rerun',
      blockerRefs: [],
      evidenceRefs: context.retainedFailureLoop.retainedFailures.flatMap(
        failure => failure.evidenceRefs,
      ),
      kind: 'draft_eval_rerun',
      publicReportRefs: reportRefs,
      requiresOperatorApproval: true,
      summary: 'Draft an operator-reviewed eval rerun from retained failures.',
      targetRefs: context.retainedFailureLoop.retainedFailures.map(
        failure => failure.failureRef,
      ),
    })
  }

  if (context.retainedFailureLoop !== null) {
    actions.push({
      actionRef: 'action.public.artanis.inspect_retained_failures',
      blockerRefs: [],
      evidenceRefs: context.retainedFailureLoop.retainedFailures.flatMap(
        failure => failure.evidenceRefs,
      ),
      kind: 'inspect_retained_failure',
      publicReportRefs: reportRefs,
      requiresOperatorApproval: false,
      summary: 'Inspect retained failures as private Artanis context.',
      targetRefs: context.retainedFailureLoop.retainedFailures.map(
        failure => failure.failureRef,
      ),
    })
  }

  if (report !== null && report.readiness === 'complete') {
    actions.push({
      actionRef: 'action.public.artanis.draft_model_lab_forum_summary',
      blockerRefs: [],
      evidenceRefs: report.sections.flatMap(section => section.evidenceRefs),
      kind: 'draft_public_forum_summary',
      publicReportRefs: [report.reportRef],
      requiresOperatorApproval: false,
      summary: 'Draft a public Forum summary from the Model Lab public report.',
      targetRefs: [report.reportRef],
    })
  }

  if (report?.claimState === 'promotion_passed_not_deployed') {
    actions.push({
      actionRef: 'action.public.artanis.request_promotion_review',
      blockerRefs: [],
      evidenceRefs: report.promotionDecisionRefs,
      kind: 'request_operator_promotion_review',
      publicReportRefs: [report.reportRef],
      requiresOperatorApproval: true,
      summary:
        'Ask an operator to review promotion evidence before any runtime change.',
      targetRefs: report.promotionDecisionRefs,
    })
  }

  return actions
}

const assertContextRecord = (context: ArtanisModelLabContextRecord): void => {
  assertValidIso('context.createdAtIso', context.createdAtIso)
  assertValidIso('context.updatedAtIso', context.updatedAtIso)
  assertSafeRefs('Artanis Model Lab context agent id', [context.agentId])
  assertSafeRefs('Artanis Model Lab context ref', [context.contextRef])
  assertSafeRefs('Artanis Model Lab loop goal ref', [context.loopGoalRef])
  assertSafeRefs('Artanis Model Lab caveat refs', context.caveatRefs)
  assertSafeRefs(
    'Artanis Model Lab private evidence refs',
    context.privateEvidenceRefs,
  )

  if (context.agentId !== 'agent_artanis') {
    throw new ArtanisModelLabContextUnsafe({
      reason: 'Artanis Model Lab contexts must use agent_artanis.',
    })
  }
}

export const projectArtanisModelLabContext = (
  context: ArtanisModelLabContextRecord,
  audience: ArtanisModelLabContextAudience,
  nowIso: string,
): ArtanisModelLabContextProjection => {
  assertContextRecord(context)

  const detailAudience = modelLabAudience(audience)
  const publicReport = context.publicReport === null
    ? null
    : projectModelLabEvidence('Model Lab public report', () =>
        projectOmniModelLabReport(context.publicReport!, 'public', nowIso),
      )
  const retainedFailureLoop = privateAudience(audience) &&
      context.retainedFailureLoop !== null
    ? projectModelLabEvidence('Model Lab retained-failure loop', () =>
        projectOmniModelLabRetainedFailureLoop(
          context.retainedFailureLoop!,
          detailAudience,
          nowIso,
        ),
      )
    : null
  const modelArtifacts = privateAudience(audience)
    ? context.modelArtifacts.map(artifact =>
        projectModelLabEvidence('Model artifact', () =>
          projectOmniModelArtifact(artifact, detailAudience, nowIso),
        ),
      )
    : []
  const trainingRuns = privateAudience(audience)
    ? context.trainingRuns.map(run =>
        projectModelLabEvidence('Training run', () =>
          projectOmniTrainingRun(run, detailAudience, nowIso),
        ),
      )
    : []
  const evidenceGraph = privateAudience(audience) &&
      context.evidenceGraph !== null
    ? projectModelLabEvidence('Model Lab evidence graph', () =>
        projectOmniModelLabEvidenceGraph(
          context.evidenceGraph!,
          detailAudience,
          nowIso,
        ),
      )
    : null
  const benchmarkCloud = privateAudience(audience) &&
      context.benchmarkCloud !== null
    ? projectModelLabEvidence('Benchmark Cloud evidence', () =>
        projectOmniBenchmarkCloud(context.benchmarkCloud!, detailAudience, nowIso),
      )
    : null
  const promotionDecisionLedger = privateAudience(audience) &&
      context.promotionDecisionLedger !== null
    ? projectModelLabEvidence('Promotion decision ledger', () =>
        projectOmniPromotionDecisionLedger(
          context.promotionDecisionLedger!,
          detailAudience,
          nowIso,
        ),
      )
    : null
  const missingContractRefs = uniqueRefs(missingContractsForContext(context))
  const blockerRefs = blockerRefsForContext(
    context,
    publicReport,
    benchmarkCloud,
    promotionDecisionLedger,
  )
  const missingEvidenceRefs = missingEvidenceRefsForContext(
    publicReport,
    evidenceGraph,
  )
  const readiness = readinessForContext(
    missingContractRefs,
    blockerRefs,
    missingEvidenceRefs,
    publicReport,
  )
  const publicReportRefs = publicReport === null ? [] : [publicReport.reportRef]
  const publicPromotionClaimRefs =
    readiness === 'ready' &&
    publicReport?.claimState === 'promotion_passed_not_deployed'
      ? [publicReport.reportRef]
      : []
  const operatorNextActions = privateAudience(audience)
    ? operatorActionsForContext(
        context,
        publicReport,
        readiness,
        missingContractRefs,
        missingEvidenceRefs,
        blockerRefs,
      )
    : []

  const projection: ArtanisModelLabContextProjection = {
    agentId: context.agentId,
    audience,
    benchmarkCloud,
    blockerRefs,
    caveatRefs: uniqueRefs(context.caveatRefs),
    consumedContractRefs: consumedContractsForContext(context),
    contextRef: context.contextRef,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      context.createdAtIso,
      nowIso,
    ),
    evidenceGraph,
    loopGoalRef: context.loopGoalRef,
    missingContractRefs,
    missingEvidenceRefs,
    modelArtifacts,
    operatorNextActions,
    privateEvidenceRefs: privateAudience(audience)
      ? uniqueRefs(context.privateEvidenceRefs)
      : [],
    promotionDecisionLedger,
    publicForumSummaryReportRefs: publicReportRefs,
    publicPromotionClaimRefs,
    publicReport,
    readiness,
    retainedFailureLoop,
    trainingRuns,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      context.updatedAtIso,
      nowIso,
    ),
  }

  assertNoFalseAuthority(projection)

  if (projectionHasPrivateMaterial(projection)) {
    throw new ArtanisModelLabContextUnsafe({
      reason:
        'Artanis Model Lab context projection contains private prompts, source archives, provider payloads, customer data, secrets, payment/wallet material, private repos, raw logs, raw traces, raw timestamps, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const artanisModelLabContextProjectionHasPrivateMaterial = (
  projection: ArtanisModelLabContextProjection,
): boolean => projectionHasPrivateMaterial(projection)

export const exampleArtanisModelLabContext = (
  records: {
    benchmarkCloud: OmniBenchmarkCloudRecord
    evidenceGraph: OmniModelLabEvidenceGraphRecord
    modelArtifact: OmniModelArtifactRecord
    promotionDecisionLedger: OmniPromotionDecisionLedgerRecord
    publicReport: OmniModelLabReportRecord
    retainedFailureLoop: OmniModelLabRetainedFailureLoopRecord
    trainingRun: OmniTrainingRunRecord
  },
): ArtanisModelLabContextRecord => ({
  agentId: 'agent_artanis',
  benchmarkCloud: records.benchmarkCloud,
  caveatRefs: ['caveat.public.artanis_model_lab_context_evidence_only'],
  contextRef: 'context.public.artanis.model_lab.autopilot_loop',
  createdAtIso: '2026-06-07T01:40:00.000Z',
  evidenceGraph: records.evidenceGraph,
  loopGoalRef: 'goal.public.artanis.pylon_model_lab',
  modelArtifacts: [records.modelArtifact],
  privateEvidenceRefs: ['evidence.public.model_lab.operator_context_packet'],
  promotionDecisionLedger: records.promotionDecisionLedger,
  publicReport: records.publicReport,
  retainedFailureLoop: records.retainedFailureLoop,
  trainingRuns: [records.trainingRun],
  updatedAtIso: '2026-06-07T01:50:00.000Z',
})
