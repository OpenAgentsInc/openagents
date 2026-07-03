import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  OmniDataClassification,
  OmniProjectionAudience,
  OmniTrustTier,
  canProjectOmniClassifiedRecord,
} from './omni-data-classification'
import {
  type OmniKnowledgeExtractedSpanRecord,
  type OmniKnowledgeSourceRecord,
  OmniKnowledgeSourceBundleRecord,
  projectOmniKnowledgeSourceBundle,
} from './omni-knowledge-source-bundles'

// ---------------------------------------------------------------------------
// Business fulfillment context library (BF-3.3 / #8086).
//
// A per-workspace grounded-memory contract that composes ingested corpus source
// bundles (BF-3.1) with structured intake facts (BF-1.2). It is deliberately
// read-only: fulfillment workflows retrieve against this record and must cite
// its source/span/fact refs before a deliverable can be considered publishable.
// ---------------------------------------------------------------------------

export const BusinessContextLibraryReadiness = S.Literals([
  'blocked',
  'ready',
])
export type BusinessContextLibraryReadiness =
  typeof BusinessContextLibraryReadiness.Type

export const BusinessContextFactSourceKind = S.Literals([
  'corpus_extraction',
  'operator_confirmation',
  'prefilled_workspace_seeded_memory',
  'structured_intake_spec',
])
export type BusinessContextFactSourceKind =
  typeof BusinessContextFactSourceKind.Type

export const BusinessContextFactKind = S.Literals([
  'approval_requirement',
  'audience',
  'constraint',
  'goal',
  'offer',
  'pain',
  'system_of_record',
  'vertical',
])
export type BusinessContextFactKind = typeof BusinessContextFactKind.Type

export const BusinessContextFactState = S.Literals([
  'candidate',
  'extracted',
  'human_confirmed',
  'rejected',
  'superseded',
])
export type BusinessContextFactState = typeof BusinessContextFactState.Type

export const BusinessContextLibraryAuthorityBoundary = S.Literals([
  'read_only_workspace_context_library',
])
export type BusinessContextLibraryAuthorityBoundary =
  typeof BusinessContextLibraryAuthorityBoundary.Type

export class BusinessContextLibraryAuthority extends S.Class<BusinessContextLibraryAuthority>(
  'BusinessContextLibraryAuthority',
)({
  authorityBoundary: BusinessContextLibraryAuthorityBoundary,
  noAutonomousIngestion: S.Boolean,
  noDeliverablePublishWithoutCitations: S.Boolean,
  noExternalSend: S.Boolean,
  noFactMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSpendAuthority: S.Boolean,
}) {}

export class BusinessContextFactRecord extends S.Class<BusinessContextFactRecord>(
  'BusinessContextFactRecord',
)({
  caveatRefs: S.Array(S.String),
  classificationCaveatRef: S.String,
  dataClassification: OmniDataClassification,
  factKind: BusinessContextFactKind,
  factRef: S.String,
  intakeSpecRef: S.NullOr(S.String),
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  sourceKind: BusinessContextFactSourceKind,
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
  state: BusinessContextFactState,
  trustTier: OmniTrustTier,
  updatedAtIso: S.String,
}) {}

export class BusinessContextLibraryRecord extends S.Class<BusinessContextLibraryRecord>(
  'BusinessContextLibraryRecord',
)({
  authority: BusinessContextLibraryAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  id: S.String,
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  retrievalTraceRefs: S.Array(S.String),
  sourceBundles: S.Array(OmniKnowledgeSourceBundleRecord),
  structuredFacts: S.Array(BusinessContextFactRecord),
  titleRef: S.String,
  updatedAtIso: S.String,
  workspaceRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class BusinessContextFactProjection extends S.Class<BusinessContextFactProjection>(
  'BusinessContextFactProjection',
)({
  caveatRefs: S.Array(S.String),
  classificationCaveatRef: S.String,
  dataClassification: OmniDataClassification,
  factKind: BusinessContextFactKind,
  factRef: S.String,
  intakeSpecRef: S.NullOr(S.String),
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  sourceKind: BusinessContextFactSourceKind,
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
  state: BusinessContextFactState,
  trustTier: OmniTrustTier,
  updatedAtDisplay: S.String,
}) {}

export class BusinessContextLibraryProjection extends S.Class<BusinessContextLibraryProjection>(
  'BusinessContextLibraryProjection',
)({
  audience: OmniProjectionAudience,
  authority: BusinessContextLibraryAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  corpusSourceCount: S.Number,
  corpusSpanCount: S.Number,
  deliverablePublishRequiresCitations: S.Boolean,
  externalSendAllowed: S.Boolean,
  factMutationAllowed: S.Boolean,
  groundingFactRefs: S.Array(S.String),
  groundingSourceRefs: S.Array(S.String),
  groundingSpanRefs: S.Array(S.String),
  id: S.String,
  provenanceRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  readiness: BusinessContextLibraryReadiness,
  redactionPolicyRefs: S.Array(S.String),
  retrievalRequiredForFulfillment: S.Boolean,
  retrievalTraceRefs: S.Array(S.String),
  sourceBundleRefs: S.Array(S.String),
  spendAuthorityAllowed: S.Boolean,
  structuredFactCount: S.Number,
  structuredFacts: S.Array(BusinessContextFactProjection),
  titleRef: S.String,
  updatedAtDisplay: S.String,
  workspaceRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class BusinessContextCitation extends S.Class<BusinessContextCitation>(
  'BusinessContextCitation',
)({
  citationRef: S.String,
  factRefs: S.Array(S.String),
  provenanceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
}) {}

export class BusinessContextDeliverableAssertion extends S.Class<BusinessContextDeliverableAssertion>(
  'BusinessContextDeliverableAssertion',
)({
  assertionRef: S.String,
  citationRefs: S.Array(S.String),
}) {}

export class BusinessContextDeliverableRecord extends S.Class<BusinessContextDeliverableRecord>(
  'BusinessContextDeliverableRecord',
)({
  assertionRefs: S.Array(BusinessContextDeliverableAssertion),
  caveatRefs: S.Array(S.String),
  citations: S.Array(BusinessContextCitation),
  deliverableRef: S.String,
  generatedArtifactRefs: S.Array(S.String),
  retrievalTraceRefs: S.Array(S.String),
  reviewRefs: S.Array(S.String),
  workflowRef: S.String,
  workspaceRef: S.String,
}) {}

export class BusinessContextDeliverableGroundingDecision extends S.Class<BusinessContextDeliverableGroundingDecision>(
  'BusinessContextDeliverableGroundingDecision',
)({
  blockerRefs: S.Array(S.String),
  citationRefs: S.Array(S.String),
  deliverableRef: S.String,
  groundedAssertionCount: S.Number,
  groundedFactRefs: S.Array(S.String),
  groundedSourceRefs: S.Array(S.String),
  groundedSpanRefs: S.Array(S.String),
  publishAllowed: S.Boolean,
  reasonRef: S.String,
  retrievalTraceRefs: S.Array(S.String),
  ungroundedAssertionRefs: S.Array(S.String),
  ungroundedCitationRefs: S.Array(S.String),
  workspaceRef: S.String,
}) {}

export class BusinessContextLibraryUnsafe extends S.TaggedErrorClass<BusinessContextLibraryUnsafe>()(
  'BusinessContextLibraryUnsafe',
  { reason: S.String },
) {}

export const BUSINESS_CONTEXT_LIBRARY_READ_ONLY_AUTHORITY:
  BusinessContextLibraryAuthority = {
    authorityBoundary: 'read_only_workspace_context_library',
    noAutonomousIngestion: true,
    noDeliverablePublishWithoutCitations: true,
    noExternalSend: true,
    noFactMutation: true,
    noPublicClaimUpgrade: true,
    noSpendAuthority: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeContextRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(contact|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(archive|auth|connector|customer|email|export|file|invoice|payment|payload|payout|prompt|provider|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|summary[_-]?text|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(bundle\.(customer|private)|caveat\.(customer|private)|context\.(customer|private)|fact\.(customer|private)|intake\.(customer|private)|policy\.(customer|private)|provenance\.(customer|private)|rights\.(customer|private)|source\.(customer|private)|span\.(customer|private)|summary\.(customer|private)|title\.(customer|private)|workspace\.|workroom\.)/i
const customerUnsafeRefPattern =
  /(context\.operator|fact\.operator|policy\.operator|source\.operator|span\.operator|summary\.operator|workspace\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(context\.private|fact\.private|intake\.private|policy\.private|rights\.private|source\.private|span\.private|summary\.private|workspace\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

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
    throw new BusinessContextLibraryUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, secret, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniProjectionAudience,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniProjectionAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const nullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: OmniProjectionAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return refsForAudience(label, [ref], audience)[0] ?? null
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniProjectionAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const classifiedProjectionAllowed = (
  record: Readonly<{
    classificationCaveatRef: string
    dataClassification: OmniDataClassification
    trustTier: OmniTrustTier
  }>,
  audience: OmniProjectionAudience,
): boolean =>
  canProjectOmniClassifiedRecord(
    {
      classificationCaveatRef: record.classificationCaveatRef,
      dataClassification: record.dataClassification,
      trustTier: record.trustTier,
    },
    audience,
  )

const sourceProjectionAllowed = (
  source: OmniKnowledgeSourceRecord,
  audience: OmniProjectionAudience,
): boolean =>
  classifiedProjectionAllowed(
    {
      classificationCaveatRef: 'classification_caveat.source_bundle',
      dataClassification: source.dataClassification,
      trustTier: source.trustTier,
    },
    audience,
  )

const spanProjectionAllowed = (
  span: OmniKnowledgeExtractedSpanRecord,
  audience: OmniProjectionAudience,
): boolean =>
  classifiedProjectionAllowed(
    {
      classificationCaveatRef: 'classification_caveat.source_bundle_span',
      dataClassification: span.dataClassification,
      trustTier: span.trustTier,
    },
    audience,
  )

const businessContextAuthorityIsReadOnly = (
  authority: BusinessContextLibraryAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_workspace_context_library' &&
  authority.noAutonomousIngestion &&
  authority.noDeliverablePublishWithoutCitations &&
  authority.noExternalSend &&
  authority.noFactMutation &&
  authority.noPublicClaimUpgrade &&
  authority.noSpendAuthority

const factSupportCount = (fact: BusinessContextFactRecord): number =>
  fact.sourceRefs.length +
  fact.spanRefs.length +
  fact.provenanceRefs.length +
  (fact.intakeSpecRef === null ? 0 : 1)

const factIsPublishableGrounding = (
  fact: BusinessContextFactRecord,
): boolean =>
  (fact.state === 'extracted' || fact.state === 'human_confirmed') &&
  fact.trustTier !== 'blocked' &&
  fact.provenanceRefs.length > 0 &&
  factSupportCount(fact) > 0

const assertFactRecord = (fact: BusinessContextFactRecord): void => {
  assertSafeRefs('Business context fact refs', [
    fact.classificationCaveatRef,
    fact.factRef,
    fact.intakeSpecRef ?? '',
    ...fact.caveatRefs,
    ...fact.provenanceRefs,
    ...fact.redactionPolicyRefs,
    ...fact.rightsRefs,
    ...fact.sourceRefs,
    ...fact.spanRefs,
  ])

  if (fact.provenanceRefs.length === 0) {
    throw new BusinessContextLibraryUnsafe({
      reason: 'Business context facts require provenance refs.',
    })
  }

  if (fact.sourceKind === 'structured_intake_spec' && fact.intakeSpecRef === null) {
    throw new BusinessContextLibraryUnsafe({
      reason: 'Structured intake facts require an intakeSpecRef.',
    })
  }

  if (
    (fact.state === 'extracted' || fact.state === 'human_confirmed') &&
    factSupportCount(fact) === 0
  ) {
    throw new BusinessContextLibraryUnsafe({
      reason:
        'Extracted or human-confirmed business context facts require source, span, intake, or provenance support.',
    })
  }
}

const assertKnowledgeBundleSafe = (
  bundle: OmniKnowledgeSourceBundleRecord,
  nowIso: string,
): void => {
  try {
    projectOmniKnowledgeSourceBundle(bundle, 'operator', nowIso)
  } catch (error) {
    throw new BusinessContextLibraryUnsafe({
      reason:
        error instanceof Error
          ? `Knowledge source bundle is not usable by the context library: ${error.message}`
          : 'Knowledge source bundle is not usable by the context library.',
    })
  }
}

const assertLibraryRecord = (
  record: BusinessContextLibraryRecord,
  nowIso: string,
): void => {
  if (!businessContextAuthorityIsReadOnly(record.authority)) {
    throw new BusinessContextLibraryUnsafe({
      reason:
        'Business context libraries must remain read-only and cannot autonomously ingest, mutate facts, send externally, spend, publish without citations, or upgrade public claims.',
    })
  }

  assertSafeRefs('Business context library refs', [
    record.id,
    record.titleRef,
    record.workspaceRef,
    ...record.blockerRefs,
    ...record.caveatRefs,
    ...record.provenanceRefs,
    ...record.redactionPolicyRefs,
    ...record.retrievalTraceRefs,
    ...record.workroomRefs,
  ])
  record.sourceBundles.forEach(bundle => assertKnowledgeBundleSafe(bundle, nowIso))
  record.structuredFacts.forEach(assertFactRecord)
}

const assertDeliverableRecord = (
  deliverable: BusinessContextDeliverableRecord,
): void => {
  assertSafeRefs('Business context deliverable refs', [
    deliverable.deliverableRef,
    deliverable.workflowRef,
    deliverable.workspaceRef,
    ...deliverable.caveatRefs,
    ...deliverable.generatedArtifactRefs,
    ...deliverable.retrievalTraceRefs,
    ...deliverable.reviewRefs,
  ])
  deliverable.assertionRefs.forEach(assertion => {
    assertSafeRefs('Business context deliverable assertion refs', [
      assertion.assertionRef,
      ...assertion.citationRefs,
    ])
  })
  deliverable.citations.forEach(citation => {
    assertSafeRefs('Business context citation refs', [
      citation.citationRef,
      ...citation.factRefs,
      ...citation.provenanceRefs,
      ...citation.sourceRefs,
      ...citation.spanRefs,
    ])

    if (
      citation.factRefs.length +
      citation.sourceRefs.length +
      citation.spanRefs.length ===
      0
    ) {
      throw new BusinessContextLibraryUnsafe({
        reason: 'Business context citations require fact, source, or span refs.',
      })
    }

    if (citation.provenanceRefs.length === 0) {
      throw new BusinessContextLibraryUnsafe({
        reason: 'Business context citations require provenance refs.',
      })
    }
  })
}

const corpusSourceRefs = (
  record: BusinessContextLibraryRecord,
): ReadonlyArray<string> =>
  uniqueRefs(
    record.sourceBundles.flatMap(bundle =>
      bundle.sources.map(source => source.sourceRef),
    ),
  )

const corpusSpanRefs = (
  record: BusinessContextLibraryRecord,
): ReadonlyArray<string> =>
  uniqueRefs(
    record.sourceBundles.flatMap(bundle =>
      bundle.spans.map(span => span.id),
    ),
  )

const corpusBundleRefs = (
  record: BusinessContextLibraryRecord,
): ReadonlyArray<string> =>
  uniqueRefs(record.sourceBundles.map(bundle => bundle.bundleRef))

const visibleCorpusSources = (
  record: BusinessContextLibraryRecord,
  audience: OmniProjectionAudience,
): ReadonlyArray<string> =>
  refsForAudience(
    'Business context corpus source refs',
    record.sourceBundles.flatMap(bundle =>
      bundle.sources
        .filter(source => sourceProjectionAllowed(source, audience))
        .map(source => source.sourceRef),
    ),
    audience,
  )

const visibleCorpusSpans = (
  record: BusinessContextLibraryRecord,
  audience: OmniProjectionAudience,
): ReadonlyArray<string> =>
  refsForAudience(
    'Business context corpus span refs',
    record.sourceBundles.flatMap(bundle =>
      bundle.spans
        .filter(span => spanProjectionAllowed(span, audience))
        .map(span => span.id),
    ),
    audience,
  )

const projectFact = (
  fact: BusinessContextFactRecord,
  audience: OmniProjectionAudience,
  nowIso: string,
): BusinessContextFactProjection | null => {
  if (!classifiedProjectionAllowed(fact, audience)) {
    return null
  }

  const factRef = refsForAudience(
    'Business context fact refs',
    [fact.factRef],
    audience,
  )[0]

  if (factRef === undefined) {
    return null
  }

  return new BusinessContextFactProjection({
    caveatRefs: refsForAudience(
      'Business context fact caveat refs',
      fact.caveatRefs,
      audience,
    ),
    classificationCaveatRef: primaryRefForAudience(
      'Business context fact classification refs',
      fact.classificationCaveatRef,
      audience,
      'classification_caveat.redacted',
    ),
    dataClassification: fact.dataClassification,
    factKind: fact.factKind,
    factRef,
    intakeSpecRef: nullableRefForAudience(
      'Business context fact intake refs',
      fact.intakeSpecRef,
      audience,
    ),
    provenanceRefs: refsForAudience(
      'Business context fact provenance refs',
      fact.provenanceRefs,
      audience,
    ),
    redactionPolicyRefs: refsForAudience(
      'Business context fact redaction refs',
      fact.redactionPolicyRefs,
      audience,
    ),
    rightsRefs: refsForAudience(
      'Business context fact rights refs',
      fact.rightsRefs,
      audience,
    ),
    sourceKind: fact.sourceKind,
    sourceRefs: refsForAudience(
      'Business context fact source refs',
      fact.sourceRefs,
      audience,
    ),
    spanRefs: refsForAudience(
      'Business context fact span refs',
      fact.spanRefs,
      audience,
    ),
    state: fact.state,
    trustTier: fact.trustTier,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      fact.updatedAtIso,
      nowIso,
    ),
  })
}

const libraryBlockers = (
  record: BusinessContextLibraryRecord,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...record.blockerRefs,
    ...(record.sourceBundles.length === 0
      ? ['blocker.business_context_library.corpus_missing']
      : []),
    ...(record.structuredFacts.filter(factIsPublishableGrounding).length === 0
      ? ['blocker.business_context_library.intake_facts_missing']
      : []),
  ])

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(item => [...stringValues(item)])
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => [...stringValues(item)])
  }

  return []
}

const projectionHasPrivateMaterial = (
  projection: BusinessContextLibraryProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience as OmniProjectionAudience)

  return (
    unsafeContextRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const projectBusinessContextLibrary = (
  record: BusinessContextLibraryRecord,
  audience: OmniProjectionAudience,
  nowIso: string,
): BusinessContextLibraryProjection => {
  assertLibraryRecord(record, nowIso)

  const blockers = libraryBlockers(record)
  const structuredFacts = record.structuredFacts
    .map(fact => projectFact(fact, audience, nowIso))
    .filter((fact): fact is BusinessContextFactProjection => fact !== null)
  const projection = new BusinessContextLibraryProjection({
    audience,
    authority: BUSINESS_CONTEXT_LIBRARY_READ_ONLY_AUTHORITY,
    blockerRefs: refsForAudience(
      'Business context library blocker refs',
      blockers,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Business context library caveat refs',
      record.caveatRefs,
      audience,
    ),
    corpusSourceCount: corpusSourceRefs(record).length,
    corpusSpanCount: corpusSpanRefs(record).length,
    deliverablePublishRequiresCitations: true,
    externalSendAllowed: false,
    factMutationAllowed: false,
    groundingFactRefs: refsForAudience(
      'Business context grounding fact refs',
      record.structuredFacts
        .filter(factIsPublishableGrounding)
        .map(fact => fact.factRef),
      audience,
    ),
    groundingSourceRefs: visibleCorpusSources(record, audience),
    groundingSpanRefs: visibleCorpusSpans(record, audience),
    id: primaryRefForAudience(
      'Business context library id refs',
      record.id,
      audience,
      'context_library.redacted',
    ),
    provenanceRefs: refsForAudience(
      'Business context library provenance refs',
      record.provenanceRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    readiness: blockers.length === 0 ? 'ready' : 'blocked',
    redactionPolicyRefs: refsForAudience(
      'Business context library redaction refs',
      record.redactionPolicyRefs,
      audience,
    ),
    retrievalRequiredForFulfillment: true,
    retrievalTraceRefs: refsForAudience(
      'Business context library retrieval trace refs',
      record.retrievalTraceRefs,
      audience,
    ),
    sourceBundleRefs: refsForAudience(
      'Business context library source bundle refs',
      corpusBundleRefs(record),
      audience,
    ),
    spendAuthorityAllowed: false,
    structuredFactCount: record.structuredFacts.length,
    structuredFacts,
    titleRef: primaryRefForAudience(
      'Business context library title refs',
      record.titleRef,
      audience,
      'title.redacted',
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workspaceRef: primaryRefForAudience(
      'Business context library workspace refs',
      record.workspaceRef,
      audience,
      'redacted.workspace',
    ),
    workroomRefs: refsForAudience(
      'Business context library workroom refs',
      record.workroomRefs,
      audience,
    ),
  })

  if (projectionHasPrivateMaterial(projection)) {
    throw new BusinessContextLibraryUnsafe({
      reason:
        'Business context library projection contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, secret, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

const citationGroundingRefs = (
  citation: BusinessContextCitation,
): ReadonlyArray<string> => [
  ...citation.factRefs,
  ...citation.sourceRefs,
  ...citation.spanRefs,
]

const citationIsKnown = (
  citation: BusinessContextCitation,
  indexes: Readonly<{
    factRefs: ReadonlySet<string>
    sourceRefs: ReadonlySet<string>
    spanRefs: ReadonlySet<string>
  }>,
): boolean =>
  citation.factRefs.every(ref => indexes.factRefs.has(ref)) &&
  citation.sourceRefs.every(ref => indexes.sourceRefs.has(ref)) &&
  citation.spanRefs.every(ref => indexes.spanRefs.has(ref)) &&
  citationGroundingRefs(citation).length > 0 &&
  citation.provenanceRefs.length > 0

export const evaluateBusinessContextDeliverableGrounding = (
  input: Readonly<{
    deliverable: BusinessContextDeliverableRecord
    library: BusinessContextLibraryRecord
    nowIso: string
  }>,
): BusinessContextDeliverableGroundingDecision => {
  assertLibraryRecord(input.library, input.nowIso)
  assertDeliverableRecord(input.deliverable)

  const publishableFactRefs = uniqueRefs(
    input.library.structuredFacts
      .filter(factIsPublishableGrounding)
      .map(fact => fact.factRef),
  )
  const indexes = {
    factRefs: new Set(publishableFactRefs),
    sourceRefs: new Set(corpusSourceRefs(input.library)),
    spanRefs: new Set(corpusSpanRefs(input.library)),
  }
  const citationsByRef = new Map<string, BusinessContextCitation>(
    input.deliverable.citations.map(citation => [
      citation.citationRef,
      citation,
    ]),
  )
  const unknownCitationRefs = uniqueRefs(
    input.deliverable.assertionRefs.flatMap(assertion =>
      assertion.citationRefs.filter(ref => !citationsByRef.has(ref)),
    ),
  )
  const ungroundedCitationRefs = uniqueRefs([
    ...input.deliverable.citations
      .filter(citation => !citationIsKnown(citation, indexes))
      .map(citation => citation.citationRef),
    ...unknownCitationRefs,
  ])
  const ungroundedCitationSet = new Set(ungroundedCitationRefs)
  const ungroundedAssertionRefs = uniqueRefs(
    input.deliverable.assertionRefs
      .filter(assertion =>
        assertion.citationRefs.length === 0 ||
        assertion.citationRefs.some(ref => ungroundedCitationSet.has(ref)),
      )
      .map(assertion => assertion.assertionRef),
  )
  const libraryRefs = libraryBlockers(input.library)
  const blockerRefs = uniqueRefs([
    ...libraryRefs,
    ...(input.deliverable.workspaceRef !== input.library.workspaceRef
      ? ['blocker.business_context_library.workspace_mismatch']
      : []),
    ...(input.deliverable.retrievalTraceRefs.length === 0
      ? ['blocker.business_context_deliverable.retrieval_trace_missing']
      : []),
    ...(input.deliverable.citations.length === 0
      ? ['blocker.business_context_deliverable.citations_missing']
      : []),
    ...(input.deliverable.assertionRefs.length === 0
      ? ['blocker.business_context_deliverable.assertions_missing']
      : []),
    ...(unknownCitationRefs.length > 0
      ? ['blocker.business_context_deliverable.assertion_citation_missing']
      : []),
    ...(ungroundedCitationRefs.length > 0
      ? ['blocker.business_context_deliverable.ungrounded_citation']
      : []),
  ])
  const publishAllowed = blockerRefs.length === 0

  return new BusinessContextDeliverableGroundingDecision({
    blockerRefs,
    citationRefs: uniqueRefs(
      input.deliverable.citations.map(citation => citation.citationRef),
    ),
    deliverableRef: input.deliverable.deliverableRef,
    groundedAssertionCount:
      input.deliverable.assertionRefs.length - ungroundedAssertionRefs.length,
    groundedFactRefs: uniqueRefs(
      input.deliverable.citations.flatMap(citation =>
        citation.factRefs.filter(ref => indexes.factRefs.has(ref)),
      ),
    ),
    groundedSourceRefs: uniqueRefs(
      input.deliverable.citations.flatMap(citation =>
        citation.sourceRefs.filter(ref => indexes.sourceRefs.has(ref)),
      ),
    ),
    groundedSpanRefs: uniqueRefs(
      input.deliverable.citations.flatMap(citation =>
        citation.spanRefs.filter(ref => indexes.spanRefs.has(ref)),
      ),
    ),
    publishAllowed,
    reasonRef: publishAllowed
      ? 'reason.business_context_deliverable.grounded'
      : 'reason.business_context_deliverable.blocked',
    retrievalTraceRefs: uniqueRefs(input.deliverable.retrievalTraceRefs),
    ungroundedAssertionRefs,
    ungroundedCitationRefs,
    workspaceRef: input.deliverable.workspaceRef,
  })
}

export const exampleBusinessContextLibrary =
  (): BusinessContextLibraryRecord => ({
    authority: BUSINESS_CONTEXT_LIBRARY_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.context_library_requires_citations'],
    createdAtIso: '2026-07-02T12:00:00.000Z',
    id: 'context_library.workspace.legal_ops_demo',
    provenanceRefs: ['provenance.public.context_library_fixture'],
    redactionPolicyRefs: ['policy.public.redacted_archive_only'],
    retrievalTraceRefs: ['retrieval_trace.workspace.legal_ops_demo.latest'],
    sourceBundles: [
      {
        authority: {
          authorityBoundary: 'read_only_knowledge_source_bundle',
          noConnectorMutation: true,
          noGeneratedSummaryMutation: true,
          noPublicClaimUpgrade: true,
          noRawSourceArchiveCopy: true,
          noRightsMutation: true,
        },
        bundleRef: 'bundle.customer.legal_ops_corpus',
        caveatRefs: ['caveat.customer.customer_supplied_corpus'],
        createdAtIso: '2026-07-02T12:00:00.000Z',
        generatedSummaryRefs: [],
        id: 'knowledge_bundle.customer.legal_ops_corpus',
        provenanceRefs: ['provenance.customer.drive_read_receipt'],
        redactionPolicyRefs: ['policy.customer.redacted_before_inference'],
        rightsRefs: ['rights.customer.read_only_workspace_corpus'],
        sources: [
          {
            caveatRefs: ['caveat.customer.customer_supplied_template'],
            dataClassification: 'customer',
            digestAlgorithm: 'sha256',
            digestRef: 'digest.customer.formation_template',
            locatorRef: 'locator.customer.formation_template',
            provenanceRefs: ['provenance.customer.drive_read_receipt'],
            redactionPolicyRefs: ['policy.customer.redacted_before_inference'],
            rightsRefs: ['rights.customer.read_only_workspace_corpus'],
            rightsState: 'customer_supplied',
            sourceKind: 'file',
            sourceRef: 'source.customer.formation_template',
            titleRef: 'title.customer.formation_template',
            trustTier: 'reviewed',
          },
        ],
        spans: [
          {
            byteEnd: null,
            byteStart: null,
            caveatRefs: ['caveat.customer.template_span'],
            codeSymbolRef: null,
            columnRefs: [],
            contentDigestRef: 'digest.customer.formation_template_scope_span',
            dataClassification: 'customer',
            excerptRef: 'excerpt.customer.formation_template_scope',
            factCandidateRefs: ['fact.customer.scope_requires_review'],
            id: 'span.customer.formation_template_scope',
            lineEnd: 20,
            lineStart: 10,
            pageNumber: null,
            provenanceRefs: ['provenance.customer.drive_read_receipt'],
            redactionPolicyRefs: ['policy.customer.redacted_before_inference'],
            rightsRefs: ['rights.customer.read_only_workspace_corpus'],
            rowEnd: null,
            rowStart: null,
            selectorRef: 'selector.customer.formation_template_scope',
            sourceRef: 'source.customer.formation_template',
            spanKind: 'file_range',
            timeEndMs: null,
            timeStartMs: null,
            trustTier: 'reviewed',
          },
        ],
        titleRef: 'title.customer.legal_ops_corpus',
        updatedAtIso: '2026-07-02T12:05:00.000Z',
        workroomRefs: ['workroom.customer.legal_ops_demo'],
      },
    ],
    structuredFacts: [
      {
        caveatRefs: ['caveat.customer.structured_intake'],
        classificationCaveatRef: 'classification_caveat.customer.intake_fact',
        dataClassification: 'customer',
        factKind: 'vertical',
        factRef: 'fact.customer.vertical.legal_ops',
        intakeSpecRef: 'intake.customer.legal_ops_demo.spec',
        provenanceRefs: ['provenance.customer.intake_spec_confirmed'],
        redactionPolicyRefs: ['policy.customer.redacted_before_inference'],
        rightsRefs: ['rights.customer.intake_use_for_fulfillment'],
        sourceKind: 'structured_intake_spec',
        sourceRefs: [],
        spanRefs: [],
        state: 'human_confirmed',
        trustTier: 'reviewed',
        updatedAtIso: '2026-07-02T12:05:00.000Z',
      },
    ],
    titleRef: 'title.customer.legal_ops_context_library',
    updatedAtIso: '2026-07-02T12:05:00.000Z',
    workspaceRef: 'workspace.customer.legal_ops_demo',
    workroomRefs: ['workroom.customer.legal_ops_demo'],
  })
