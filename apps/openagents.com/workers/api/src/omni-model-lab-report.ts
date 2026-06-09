import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniModelLabReportAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniModelLabReportAudience =
  typeof OmniModelLabReportAudience.Type

export const OmniModelLabReportSectionKind = S.Literals([
  'attribution',
  'benchmark_evidence',
  'candidates',
  'marketplace_memory',
  'model_artifacts',
  'promotion_decisions',
  'retained_failures',
  'rollback',
  'training_runs',
])
export type OmniModelLabReportSectionKind =
  typeof OmniModelLabReportSectionKind.Type

export const OmniModelLabReportReadiness = S.Literals([
  'blocked',
  'complete',
  'missing_evidence',
  'partial',
])
export type OmniModelLabReportReadiness =
  typeof OmniModelLabReportReadiness.Type

export const OmniModelLabReportClaimState = S.Literals([
  'blocked',
  'evidence_only',
  'missing_evidence',
  'no_public_claim',
  'promotion_passed_not_deployed',
])
export type OmniModelLabReportClaimState =
  typeof OmniModelLabReportClaimState.Type

export const OmniModelLabReportAuthorityBoundary = S.Literals([
  'read_only_model_lab_report',
])
export type OmniModelLabReportAuthorityBoundary =
  typeof OmniModelLabReportAuthorityBoundary.Type

export class OmniModelLabReportAuthority extends S.Class<OmniModelLabReportAuthority>(
  'OmniModelLabReportAuthority',
)({
  authorityBoundary: OmniModelLabReportAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noEvalExecution: S.Boolean,
  noPaymentSpend: S.Boolean,
  noPayoutMutation: S.Boolean,
  noProviderCall: S.Boolean,
  noPublicClaimMutation: S.Boolean,
  noRawArtifactExport: S.Boolean,
  noReportPublication: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
  noTrainingLaunch: S.Boolean,
}) {}

export class OmniModelLabReportSectionRecord extends S.Class<OmniModelLabReportSectionRecord>(
  'OmniModelLabReportSectionRecord',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  itemRefs: S.Array(S.String),
  kind: OmniModelLabReportSectionKind,
  missingEvidenceRefs: S.Array(S.String),
  readiness: OmniModelLabReportReadiness,
  sectionRef: S.String,
  title: S.String,
}) {}

export class OmniModelLabReportRedactionSummary extends S.Class<OmniModelLabReportRedactionSummary>(
  'OmniModelLabReportRedactionSummary',
)({
  audience: OmniModelLabReportAudience,
  redactedRefCount: S.Number,
  redactionPolicyRefs: S.Array(S.String),
  withheldClassRefs: S.Array(S.String),
}) {}

export class OmniModelLabReportRecord extends S.Class<OmniModelLabReportRecord>(
  'OmniModelLabReportRecord',
)({
  artifactRefs: S.Array(S.String),
  attributionRefs: S.Array(S.String),
  authority: OmniModelLabReportAuthority,
  benchmarkEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimState: OmniModelLabReportClaimState,
  createdAtIso: S.String,
  id: S.String,
  marketplaceMemoryRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  promotionDecisionRefs: S.Array(S.String),
  readiness: OmniModelLabReportReadiness,
  redactionPolicyRefs: S.Array(S.String),
  reportRef: S.String,
  retainedFailureRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  sections: S.Array(OmniModelLabReportSectionRecord),
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
  withheldClassRefs: S.Array(S.String),
}) {}

export class OmniModelLabReportProjection extends S.Class<OmniModelLabReportProjection>(
  'OmniModelLabReportProjection',
)({
  adapterInstallAllowed: S.Boolean,
  artifactRefs: S.Array(S.String),
  attributionRefs: S.Array(S.String),
  audience: OmniModelLabReportAudience,
  authority: OmniModelLabReportAuthority,
  benchmarkEvidenceRefs: S.Array(S.String),
  blockedSectionCount: S.Number,
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimState: OmniModelLabReportClaimState,
  completeSectionCount: S.Number,
  createdAtDisplay: S.String,
  evalExecutionAllowed: S.Boolean,
  id: S.String,
  marketplaceMemoryRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  missingSectionCount: S.Number,
  partialSectionCount: S.Number,
  paymentSpendAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  promotionDecisionRefs: S.Array(S.String),
  providerCallAllowed: S.Boolean,
  publicClaimMutationAllowed: S.Boolean,
  rawArtifactExportAllowed: S.Boolean,
  readiness: OmniModelLabReportReadiness,
  redaction: OmniModelLabReportRedactionSummary,
  reportPublicationAllowed: S.Boolean,
  reportRef: S.String,
  retainedFailureRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Boolean,
  sectionCount: S.Number,
  sections: S.Array(OmniModelLabReportSectionRecord),
  settlementMutationAllowed: S.Boolean,
  trainingLaunchAllowed: S.Boolean,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniModelLabReportUnsafe extends S.TaggedErrorClass<OmniModelLabReportUnsafe>()(
  'OmniModelLabReportUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY:
  OmniModelLabReportAuthority = {
    authorityBoundary: 'read_only_model_lab_report',
    noAdapterInstall: true,
    noEvalExecution: true,
    noPaymentSpend: true,
    noPayoutMutation: true,
    noProviderCall: true,
    noPublicClaimMutation: true,
    noRawArtifactExport: true,
    noReportPublication: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
    noTrainingLaunch: true,
  }

const requiredSectionKinds: ReadonlyArray<OmniModelLabReportSectionKind> = [
  'attribution',
  'benchmark_evidence',
  'candidates',
  'marketplace_memory',
  'model_artifacts',
  'promotion_decisions',
  'retained_failures',
  'rollback',
  'training_runs',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeReportRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|email|fixture|input|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(artifact\.private|attribution\.private|benchmark\.private|blocker\.private|candidate\.private|caveat\.private|decision\.private|eval\.private|evidence\.private|failure\.private|marketplace\.private|policy\.private|promotion\.private|report\.private|rollback\.private|section\.private|source\.|training\.private|withheld\.private)/i
const agentUnsafeRefPattern =
  /(artifact\.private|attribution\.private|benchmark\.private|blocker\.private|candidate\.private|decision\.private|eval\.private|failure\.private|marketplace\.private|policy\.private|promotion\.private|report\.private|rollback\.private|section\.private|source\.private|training\.private|withheld\.private)/i
const customerUnsafeRefPattern = agentUnsafeRefPattern

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeReportRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniModelLabReportUnsafe({
      reason: `${label} contains private prompts, source archives, datasets, provider payloads, raw artifacts, model weights, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniModelLabReportAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniModelLabReportAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: OmniModelLabReportAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const redactedRefCountForAudience = (
  refs: ReadonlyArray<string>,
  audience: OmniModelLabReportAudience,
): number => {
  const pattern = audienceUnsafePattern(audience)

  if (pattern === null) {
    return 0
  }

  return uniqueRefs(refs).filter(ref => pattern.test(ref)).length
}

const assertReadOnlyAuthority = (
  authority: OmniModelLabReportAuthority,
): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noEvalExecution !== true ||
    authority.noPaymentSpend !== true ||
    authority.noPayoutMutation !== true ||
    authority.noProviderCall !== true ||
    authority.noPublicClaimMutation !== true ||
    authority.noRawArtifactExport !== true ||
    authority.noReportPublication !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true ||
    authority.noTrainingLaunch !== true
  ) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Model Lab reports are read-only exports and cannot train, run evals, call providers, install adapters, export raw artifacts, publish reports, spend money, promote runtime behavior, pay out, settle, or mutate public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniModelLabReportUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const duplicateRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  uniqueRefs(refs.filter((ref, index) => refs.indexOf(ref) !== index))

const assertSection = (section: OmniModelLabReportSectionRecord): void => {
  assertSafeRefs('Model Lab report section ref', [section.sectionRef])
  assertSafeRefs('Model Lab report section title', [section.title])
  assertSafeRefs('Model Lab report section item refs', section.itemRefs)
  assertSafeRefs('Model Lab report section evidence refs', section.evidenceRefs)
  assertSafeRefs(
    'Model Lab report section missing evidence refs',
    section.missingEvidenceRefs,
  )
  assertSafeRefs('Model Lab report section caveat refs', section.caveatRefs)
  assertSafeRefs('Model Lab report section blocker refs', section.blockerRefs)

  if (
    section.readiness === 'complete' &&
    (!hasAny(section.itemRefs) || !hasAny(section.evidenceRefs))
  ) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Complete Model Lab report sections require items and evidence.',
    })
  }

  if (
    section.readiness === 'partial' &&
    (!hasAny(section.itemRefs) ||
      (!hasAny(section.missingEvidenceRefs) && !hasAny(section.caveatRefs)))
  ) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Partial Model Lab report sections require items plus missing evidence or caveats.',
    })
  }

  if (
    section.readiness === 'missing_evidence' &&
    !hasAny(section.missingEvidenceRefs)
  ) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Missing-evidence report sections require missing evidence refs.',
    })
  }

  if (section.readiness === 'blocked' && !hasAny(section.blockerRefs)) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Blocked Model Lab report sections require blocker refs.',
    })
  }
}

const assertReport = (report: OmniModelLabReportRecord): void => {
  assertReadOnlyAuthority(report.authority)
  assertValidIso('createdAtIso', report.createdAtIso)
  assertValidIso('updatedAtIso', report.updatedAtIso)
  assertSafeRefs('Model Lab report id', [report.id])
  assertSafeRefs('Model Lab report ref', [report.reportRef])
  assertSafeRefs('Model Lab report artifact refs', report.artifactRefs)
  assertSafeRefs('Model Lab report attribution refs', report.attributionRefs)
  assertSafeRefs(
    'Model Lab report benchmark evidence refs',
    report.benchmarkEvidenceRefs,
  )
  assertSafeRefs('Model Lab report blocker refs', report.blockerRefs)
  assertSafeRefs('Model Lab report candidate refs', report.candidateRefs)
  assertSafeRefs('Model Lab report caveat refs', report.caveatRefs)
  assertSafeRefs(
    'Model Lab report marketplace memory refs',
    report.marketplaceMemoryRefs,
  )
  assertSafeRefs(
    'Model Lab report missing evidence refs',
    report.missingEvidenceRefs,
  )
  assertSafeRefs(
    'Model Lab report promotion decision refs',
    report.promotionDecisionRefs,
  )
  assertSafeRefs(
    'Model Lab report redaction policy refs',
    report.redactionPolicyRefs,
  )
  assertSafeRefs(
    'Model Lab report retained failure refs',
    report.retainedFailureRefs,
  )
  assertSafeRefs('Model Lab report rollback refs', report.rollbackRefs)
  assertSafeRefs('Model Lab report training run refs', report.trainingRunRefs)
  assertSafeRefs('Model Lab report withheld class refs', report.withheldClassRefs)
  report.sections.forEach(assertSection)

  if (!hasAny(report.sections)) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Model Lab reports require sections.',
    })
  }

  const sectionKinds = report.sections.map(section => section.kind)
  const sectionRefs = report.sections.map(section => section.sectionRef)

  if (hasAny(duplicateRefs(sectionKinds)) || hasAny(duplicateRefs(sectionRefs))) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Model Lab reports cannot contain duplicate section kinds or section refs.',
    })
  }

  if (requiredSectionKinds.some(kind => !sectionKinds.includes(kind))) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Model Lab reports require retained failures, candidates, model artifacts, training runs, benchmark evidence, promotion decisions, rollback, attribution, and marketplace memory sections.',
    })
  }

  if (
    report.readiness === 'complete' &&
    (report.sections.some(section => section.readiness !== 'complete') ||
      hasAny(report.missingEvidenceRefs) ||
      hasAny(report.blockerRefs))
  ) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Complete Model Lab reports require complete sections and no missing evidence or blockers.',
    })
  }

  if (
    report.readiness === 'partial' &&
    (!hasAny(report.missingEvidenceRefs) ||
      report.sections.every(section => section.readiness === 'complete'))
  ) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Partial Model Lab reports require missing evidence and at least one non-complete section.',
    })
  }

  if (
    report.readiness === 'missing_evidence' &&
    !hasAny(report.missingEvidenceRefs)
  ) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Missing-evidence Model Lab reports require missing evidence refs.',
    })
  }

  if (report.readiness === 'blocked' && !hasAny(report.blockerRefs)) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Blocked Model Lab reports require blocker refs.',
    })
  }

  if (
    report.claimState === 'promotion_passed_not_deployed' &&
    (!hasAny(report.promotionDecisionRefs) || !hasAny(report.caveatRefs))
  ) {
    throw new OmniModelLabReportUnsafe({
      reason:
        'Promotion-passed report claims require promotion decision refs and no-deploy caveats.',
    })
  }

  if (
    report.claimState === 'missing_evidence' &&
    !hasAny(report.missingEvidenceRefs)
  ) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Missing-evidence claim state requires missing evidence refs.',
    })
  }

  if (report.claimState === 'blocked' && !hasAny(report.blockerRefs)) {
    throw new OmniModelLabReportUnsafe({
      reason: 'Blocked claim state requires blocker refs.',
    })
  }

  if (report.claimState === 'no_public_claim' && !hasAny(report.caveatRefs)) {
    throw new OmniModelLabReportUnsafe({
      reason: 'No-public-claim report state requires caveat refs.',
    })
  }
}

const redactSection = (
  section: OmniModelLabReportSectionRecord,
  audience: OmniModelLabReportAudience,
): OmniModelLabReportSectionRecord => ({
  ...section,
  blockerRefs: refsForAudience(
    'Model Lab report section blocker refs',
    section.blockerRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Model Lab report section caveat refs',
    section.caveatRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Model Lab report section evidence refs',
    section.evidenceRefs,
    audience,
  ),
  itemRefs: refsForAudience(
    'Model Lab report section item refs',
    section.itemRefs,
    audience,
  ),
  missingEvidenceRefs: refsForAudience(
    'Model Lab report section missing evidence refs',
    section.missingEvidenceRefs,
    audience,
  ),
  sectionRef: refForAudience(
    'Model Lab report section ref',
    section.sectionRef,
    audience,
    'section.redacted.model_lab_report',
  ),
})

const allRefsForReport = (
  report: OmniModelLabReportRecord,
): ReadonlyArray<string> => [
  report.id,
  report.reportRef,
  ...report.artifactRefs,
  ...report.attributionRefs,
  ...report.benchmarkEvidenceRefs,
  ...report.blockerRefs,
  ...report.candidateRefs,
  ...report.caveatRefs,
  ...report.marketplaceMemoryRefs,
  ...report.missingEvidenceRefs,
  ...report.promotionDecisionRefs,
  ...report.redactionPolicyRefs,
  ...report.retainedFailureRefs,
  ...report.rollbackRefs,
  ...report.trainingRunRefs,
  ...report.withheldClassRefs,
  ...report.sections.flatMap(section => [
    section.sectionRef,
    ...section.blockerRefs,
    ...section.caveatRefs,
    ...section.evidenceRefs,
    ...section.itemRefs,
    ...section.missingEvidenceRefs,
  ]),
]

export const projectOmniModelLabReport = (
  report: OmniModelLabReportRecord,
  audience: OmniModelLabReportAudience,
  nowIso: string,
): OmniModelLabReportProjection => {
  assertReport(report)

  return {
    adapterInstallAllowed: !report.authority.noAdapterInstall,
    artifactRefs: refsForAudience(
      'Model Lab report artifact refs',
      report.artifactRefs,
      audience,
    ),
    attributionRefs: refsForAudience(
      'Model Lab report attribution refs',
      report.attributionRefs,
      audience,
    ),
    audience,
    authority: report.authority,
    benchmarkEvidenceRefs: refsForAudience(
      'Model Lab report benchmark evidence refs',
      report.benchmarkEvidenceRefs,
      audience,
    ),
    blockedSectionCount: report.sections.filter(
      section => section.readiness === 'blocked',
    ).length,
    blockerRefs: refsForAudience(
      'Model Lab report blocker refs',
      report.blockerRefs,
      audience,
    ),
    candidateRefs: refsForAudience(
      'Model Lab report candidate refs',
      report.candidateRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Model Lab report caveat refs',
      report.caveatRefs,
      audience,
    ),
    claimState: report.claimState,
    completeSectionCount: report.sections.filter(
      section => section.readiness === 'complete',
    ).length,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      report.createdAtIso,
      nowIso,
    ),
    evalExecutionAllowed: !report.authority.noEvalExecution,
    id: refForAudience(
      'Model Lab report id',
      report.id,
      audience,
      'model-lab-report.redacted',
    ),
    marketplaceMemoryRefs: refsForAudience(
      'Model Lab report marketplace memory refs',
      report.marketplaceMemoryRefs,
      audience,
    ),
    missingEvidenceRefs: refsForAudience(
      'Model Lab report missing evidence refs',
      report.missingEvidenceRefs,
      audience,
    ),
    missingSectionCount: report.sections.filter(
      section => section.readiness === 'missing_evidence',
    ).length,
    partialSectionCount: report.sections.filter(
      section => section.readiness === 'partial',
    ).length,
    paymentSpendAllowed: !report.authority.noPaymentSpend,
    payoutMutationAllowed: !report.authority.noPayoutMutation,
    promotionDecisionRefs: refsForAudience(
      'Model Lab report promotion decision refs',
      report.promotionDecisionRefs,
      audience,
    ),
    providerCallAllowed: !report.authority.noProviderCall,
    publicClaimMutationAllowed: !report.authority.noPublicClaimMutation,
    rawArtifactExportAllowed: !report.authority.noRawArtifactExport,
    readiness: report.readiness,
    redaction: {
      audience,
      redactedRefCount: redactedRefCountForAudience(
        allRefsForReport(report),
        audience,
      ),
      redactionPolicyRefs: refsForAudience(
        'Model Lab report redaction policy refs',
        report.redactionPolicyRefs,
        audience,
      ),
      withheldClassRefs: refsForAudience(
        'Model Lab report withheld class refs',
        report.withheldClassRefs,
        audience,
      ),
    },
    reportPublicationAllowed: !report.authority.noReportPublication,
    reportRef: refForAudience(
      'Model Lab report ref',
      report.reportRef,
      audience,
      'report.redacted.model_lab',
    ),
    retainedFailureRefs: refsForAudience(
      'Model Lab report retained failure refs',
      report.retainedFailureRefs,
      audience,
    ),
    rollbackRefs: refsForAudience(
      'Model Lab report rollback refs',
      report.rollbackRefs,
      audience,
    ),
    runtimePromotionAllowed: !report.authority.noRuntimePromotion,
    sectionCount: report.sections.length,
    sections: report.sections.map(section => redactSection(section, audience)),
    settlementMutationAllowed: !report.authority.noSettlementMutation,
    trainingLaunchAllowed: !report.authority.noTrainingLaunch,
    trainingRunRefs: refsForAudience(
      'Model Lab report training run refs',
      report.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      report.updatedAtIso,
      nowIso,
    ),
  }
}

const projectionStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStringValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStringValues)
  }

  return []
}

export const omniModelLabReportProjectionHasPrivateMaterial = (
  projection: OmniModelLabReportProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      unsafeReportRefPattern.test(value) || rawTimestampPattern.test(value),
  )

const completeSection = (
  kind: OmniModelLabReportSectionKind,
  itemRefs: ReadonlyArray<string>,
  evidenceRefs: ReadonlyArray<string>,
): OmniModelLabReportSectionRecord => ({
  blockerRefs: [],
  caveatRefs: [`caveat.public.${kind}_evidence_only`],
  evidenceRefs,
  itemRefs,
  kind,
  missingEvidenceRefs: [],
  readiness: 'complete',
  sectionRef: `section.public.${kind}`,
  title: kind,
})

export const exampleOmniModelLabReport = (): OmniModelLabReportRecord => ({
  artifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
  attributionRefs: ['attribution.public.autopilot_site_revision_quality'],
  authority: OMNI_MODEL_LAB_REPORT_READ_ONLY_AUTHORITY,
  benchmarkEvidenceRefs: ['benchmark.public.autopilot_coding_cloud_v1'],
  blockerRefs: [],
  candidateRefs: ['candidate.public.autopilot_lora_v2'],
  caveatRefs: [
    'caveat.public.model_lab_report_evidence_only',
    'caveat.public.promotion_passed_but_not_deployed',
  ],
  claimState: 'promotion_passed_not_deployed',
  createdAtIso: '2026-06-07T00:20:00.000Z',
  id: 'report.public.model_lab_autopilot_v2',
  marketplaceMemoryRefs: ['marketplace.public.autopilot_margin_memory'],
  missingEvidenceRefs: [],
  promotionDecisionRefs: ['decision.public.autopilot_lora_v2_passed'],
  readiness: 'complete',
  redactionPolicyRefs: ['policy.public.model_lab_report_redaction'],
  reportRef: 'report.public.model_lab_autopilot_v2',
  retainedFailureRefs: ['retained_failure.public.site_revision_images'],
  rollbackRefs: ['rollback.public.autopilot_lora_v1_restore'],
  sections: [
    completeSection(
      'retained_failures',
      ['retained_failure.public.site_revision_images'],
      ['evidence.public.retained_failure_summary'],
    ),
    completeSection(
      'candidates',
      ['candidate.public.autopilot_lora_v2'],
      ['evidence.public.candidate_summary'],
    ),
    completeSection(
      'model_artifacts',
      ['artifact.public.autopilot_lora_candidate_v2'],
      ['evidence.public.artifact_digest_summary'],
    ),
    completeSection(
      'training_runs',
      ['training.public.autopilot_lora_v2_imported'],
      ['evidence.public.training_run_summary'],
    ),
    completeSection(
      'benchmark_evidence',
      ['benchmark.public.autopilot_coding_cloud_v1'],
      ['evidence.public.benchmark_cloud_summary'],
    ),
    completeSection(
      'promotion_decisions',
      ['decision.public.autopilot_lora_v2_passed'],
      ['evidence.public.promotion_decision_summary'],
    ),
    completeSection(
      'rollback',
      ['rollback.public.autopilot_lora_v1_restore'],
      ['evidence.public.rollback_ready_summary'],
    ),
    completeSection(
      'attribution',
      ['attribution.public.autopilot_site_revision_quality'],
      ['evidence.public.attribution_summary'],
    ),
    completeSection(
      'marketplace_memory',
      ['marketplace.public.autopilot_margin_memory'],
      ['evidence.public.marketplace_memory_summary'],
    ),
  ],
  trainingRunRefs: ['training.public.autopilot_lora_v2_imported'],
  updatedAtIso: '2026-06-07T00:26:00.000Z',
  withheldClassRefs: ['withheld.public.artifact_binaries_and_private_inputs'],
})
