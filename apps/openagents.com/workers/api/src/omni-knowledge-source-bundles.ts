import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  OmniDataClassification,
  OmniTrustTier,
} from './omni-data-classification'

export const OmniKnowledgeProjectionAudience = S.Literals([
  'public',
  'team',
  'operator',
])
export type OmniKnowledgeProjectionAudience =
  typeof OmniKnowledgeProjectionAudience.Type

export const OmniKnowledgeSourceKind = S.Literals([
  'connector_read',
  'data_package',
  'file',
  'link',
  'repo_ref',
  'table',
  'transcript',
])
export type OmniKnowledgeSourceKind = typeof OmniKnowledgeSourceKind.Type

export const OmniKnowledgeSourceRightsState = S.Literals([
  'customer_supplied',
  'internal_only',
  'licensed',
  'public',
  'revoked',
  'unknown',
])
export type OmniKnowledgeSourceRightsState =
  typeof OmniKnowledgeSourceRightsState.Type

export const OmniKnowledgeDigestAlgorithm = S.Literals([
  'blake3',
  'sha256',
])
export type OmniKnowledgeDigestAlgorithm =
  typeof OmniKnowledgeDigestAlgorithm.Type

export const OmniKnowledgeSpanKind = S.Literals([
  'code',
  'file_range',
  'page',
  'row',
  'table_cell',
  'transcript',
])
export type OmniKnowledgeSpanKind = typeof OmniKnowledgeSpanKind.Type

export const OmniKnowledgeBundleAuthorityBoundary = S.Literals([
  'read_only_knowledge_source_bundle',
])
export type OmniKnowledgeBundleAuthorityBoundary =
  typeof OmniKnowledgeBundleAuthorityBoundary.Type

export class OmniKnowledgeBundleAuthority extends S.Class<OmniKnowledgeBundleAuthority>(
  'OmniKnowledgeBundleAuthority',
)({
  authorityBoundary: OmniKnowledgeBundleAuthorityBoundary,
  noConnectorMutation: S.Boolean,
  noGeneratedSummaryMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRawSourceArchiveCopy: S.Boolean,
  noRightsMutation: S.Boolean,
}) {}

export class OmniKnowledgeSourceRecord extends S.Class<OmniKnowledgeSourceRecord>(
  'OmniKnowledgeSourceRecord',
)({
  caveatRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  digestAlgorithm: OmniKnowledgeDigestAlgorithm,
  digestRef: S.String,
  locatorRef: S.String,
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  rightsState: OmniKnowledgeSourceRightsState,
  sourceKind: OmniKnowledgeSourceKind,
  sourceRef: S.String,
  titleRef: S.String,
  trustTier: OmniTrustTier,
}) {}

export class OmniKnowledgeExtractedSpanRecord extends S.Class<OmniKnowledgeExtractedSpanRecord>(
  'OmniKnowledgeExtractedSpanRecord',
)({
  byteEnd: S.NullOr(S.Number),
  byteStart: S.NullOr(S.Number),
  caveatRefs: S.Array(S.String),
  codeSymbolRef: S.NullOr(S.String),
  columnRefs: S.Array(S.String),
  contentDigestRef: S.String,
  dataClassification: OmniDataClassification,
  excerptRef: S.String,
  factCandidateRefs: S.Array(S.String),
  id: S.String,
  lineEnd: S.NullOr(S.Number),
  lineStart: S.NullOr(S.Number),
  pageNumber: S.NullOr(S.Number),
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  rowEnd: S.NullOr(S.Number),
  rowStart: S.NullOr(S.Number),
  selectorRef: S.NullOr(S.String),
  sourceRef: S.String,
  spanKind: OmniKnowledgeSpanKind,
  timeEndMs: S.NullOr(S.Number),
  timeStartMs: S.NullOr(S.Number),
  trustTier: OmniTrustTier,
}) {}

export class OmniKnowledgeSourceBundleRecord extends S.Class<OmniKnowledgeSourceBundleRecord>(
  'OmniKnowledgeSourceBundleRecord',
)({
  authority: OmniKnowledgeBundleAuthority,
  bundleRef: S.String,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  generatedSummaryRefs: S.Array(S.String),
  id: S.String,
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  sources: S.Array(OmniKnowledgeSourceRecord),
  spans: S.Array(OmniKnowledgeExtractedSpanRecord),
  titleRef: S.String,
  updatedAtIso: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class OmniKnowledgeSourceProjection extends S.Class<OmniKnowledgeSourceProjection>(
  'OmniKnowledgeSourceProjection',
)({
  caveatRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  digestAlgorithm: OmniKnowledgeDigestAlgorithm,
  digestRef: S.String,
  locatorRef: S.String,
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  rightsState: OmniKnowledgeSourceRightsState,
  sourceKind: OmniKnowledgeSourceKind,
  sourceRef: S.String,
  titleRef: S.String,
  trustTier: OmniTrustTier,
}) {}

export class OmniKnowledgeSpanProjection extends S.Class<OmniKnowledgeSpanProjection>(
  'OmniKnowledgeSpanProjection',
)({
  caveatRefs: S.Array(S.String),
  codeSymbolRef: S.NullOr(S.String),
  columnRefs: S.Array(S.String),
  contentDigestRef: S.String,
  dataClassification: OmniDataClassification,
  excerptRef: S.String,
  factCandidateRefs: S.Array(S.String),
  id: S.String,
  locatorLabel: S.String,
  provenanceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  rightsRefs: S.Array(S.String),
  selectorRef: S.NullOr(S.String),
  sourceRef: S.String,
  spanKind: OmniKnowledgeSpanKind,
  trustTier: OmniTrustTier,
}) {}

export class OmniKnowledgeSourceBundleProjection extends S.Class<OmniKnowledgeSourceBundleProjection>(
  'OmniKnowledgeSourceBundleProjection',
)({
  audience: OmniKnowledgeProjectionAudience,
  authority: OmniKnowledgeBundleAuthority,
  bundleRef: S.String,
  caveatRefs: S.Array(S.String),
  connectorMutationAllowed: S.Boolean,
  generatedSummaryMutationAllowed: S.Boolean,
  generatedSummaryRefs: S.Array(S.String),
  id: S.String,
  provenanceRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  rawSourceArchiveCopyAllowed: S.Boolean,
  redactionPolicyRefs: S.Array(S.String),
  rightsMutationAllowed: S.Boolean,
  rightsRefs: S.Array(S.String),
  sourceCount: S.Number,
  sources: S.Array(OmniKnowledgeSourceProjection),
  spanCount: S.Number,
  spans: S.Array(OmniKnowledgeSpanProjection),
  titleRef: S.String,
  updatedAtDisplay: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class OmniKnowledgeSourceBundleUnsafe extends S.TaggedErrorClass<OmniKnowledgeSourceBundleUnsafe>()(
  'OmniKnowledgeSourceBundleUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY:
  OmniKnowledgeBundleAuthority = {
    authorityBoundary: 'read_only_knowledge_source_bundle',
    noConnectorMutation: true,
    noGeneratedSummaryMutation: true,
    noPublicClaimUpgrade: true,
    noRawSourceArchiveCopy: true,
    noRightsMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeKnowledgeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(key|source|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(archive|auth|connector|customer|email|export|file|invoice|payment|payload|payout|prompt|provider|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|summary[_-]?text|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(bundle\.private|caveat\.private|digest\.private|excerpt\.private|fact\.private|locator\.private|policy\.private|provenance\.private|rights\.private|source\.private|span\.private|summary\.private|title\.private|workroom\.)/i
const teamUnsafeRefPattern =
  /(digest\.private|excerpt\.private|locator\.private|rights\.private|source\.private|span\.private|summary\.private|workroom\.private)/i

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
      unsafeKnowledgeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, secret, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniKnowledgeProjectionAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniKnowledgeProjectionAudience,
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
  audience: OmniKnowledgeProjectionAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return refsForAudience(label, [ref], audience)[0] ?? null
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniKnowledgeProjectionAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertPositiveInteger = (
  label: string,
  value: number | null,
): void => {
  if (value === null) {
    return
  }

  if (!Number.isInteger(value) || value < 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: `${label} must be a non-negative integer when present.`,
    })
  }
}

const assertOrderedRange = (
  label: string,
  start: number | null,
  end: number | null,
): void => {
  assertPositiveInteger(`${label} start`, start)
  assertPositiveInteger(`${label} end`, end)

  if (start !== null && end !== null && end < start) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: `${label} end must be greater than or equal to start.`,
    })
  }
}

const requireRange = (
  span: OmniKnowledgeExtractedSpanRecord,
  label: string,
  start: number | null,
  end: number | null,
): void => {
  if (start === null || end === null) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: `${span.spanKind} spans require ${label} start and end.`,
    })
  }

  assertOrderedRange(`${span.spanKind} ${label}`, start, end)
}

const assertSourceRecord = (source: OmniKnowledgeSourceRecord): void => {
  assertSafeRefs('Knowledge source refs', [
    source.sourceRef,
    source.locatorRef,
    source.digestRef,
    source.titleRef,
  ])
  assertSafeRefs('Knowledge source caveat refs', source.caveatRefs)
  assertSafeRefs('Knowledge source provenance refs', source.provenanceRefs)
  assertSafeRefs(
    'Knowledge source redaction policy refs',
    source.redactionPolicyRefs,
  )
  assertSafeRefs('Knowledge source rights refs', source.rightsRefs)

  if (source.provenanceRefs.length === 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Knowledge sources require provenance refs.',
    })
  }

  if (source.rightsRefs.length === 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Knowledge sources require rights refs.',
    })
  }

  if (source.rightsState === 'revoked') {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Revoked source rights cannot be projected as active sources.',
    })
  }

  if (
    source.sourceRef.includes('generated_summary') ||
    source.locatorRef.includes('generated_summary')
  ) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason:
        'Generated summaries must be linked through generatedSummaryRefs, not modeled as source records.',
    })
  }
}

const assertSpanRecord = (
  span: OmniKnowledgeExtractedSpanRecord,
  sourceRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Knowledge extracted span refs', [
    span.id,
    span.sourceRef,
    span.contentDigestRef,
    span.excerptRef,
    span.selectorRef ?? '',
    span.codeSymbolRef ?? '',
  ])
  assertSafeRefs('Knowledge span caveat refs', span.caveatRefs)
  assertSafeRefs('Knowledge span column refs', span.columnRefs)
  assertSafeRefs('Knowledge span fact candidate refs', span.factCandidateRefs)
  assertSafeRefs('Knowledge span provenance refs', span.provenanceRefs)
  assertSafeRefs(
    'Knowledge span redaction policy refs',
    span.redactionPolicyRefs,
  )
  assertSafeRefs('Knowledge span rights refs', span.rightsRefs)

  if (!sourceRefs.has(span.sourceRef)) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Extracted spans must reference a source in the same bundle.',
    })
  }

  if (span.provenanceRefs.length === 0 || span.rightsRefs.length === 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Extracted spans require provenance and rights refs.',
    })
  }

  assertOrderedRange('byte range', span.byteStart, span.byteEnd)
  assertOrderedRange('line range', span.lineStart, span.lineEnd)
  assertOrderedRange('row range', span.rowStart, span.rowEnd)
  assertOrderedRange('time range', span.timeStartMs, span.timeEndMs)
  assertPositiveInteger('page number', span.pageNumber)

  switch (span.spanKind) {
    case 'code':
      requireRange(span, 'line', span.lineStart, span.lineEnd)
      break
    case 'file_range':
      if (
        (span.lineStart === null || span.lineEnd === null) &&
        (span.byteStart === null || span.byteEnd === null)
      ) {
        throw new OmniKnowledgeSourceBundleUnsafe({
          reason: 'file_range spans require either line or byte ranges.',
        })
      }
      break
    case 'page':
      if (span.pageNumber === null || span.pageNumber < 1) {
        throw new OmniKnowledgeSourceBundleUnsafe({
          reason: 'page spans require a positive pageNumber.',
        })
      }
      break
    case 'row':
      requireRange(span, 'row', span.rowStart, span.rowEnd)
      break
    case 'table_cell':
      requireRange(span, 'row', span.rowStart, span.rowEnd)
      if (span.columnRefs.length === 0) {
        throw new OmniKnowledgeSourceBundleUnsafe({
          reason: 'table_cell spans require column refs.',
        })
      }
      break
    case 'transcript':
      requireRange(span, 'time', span.timeStartMs, span.timeEndMs)
      break
  }
}

const assertBundleRecord = (
  bundle: OmniKnowledgeSourceBundleRecord,
): void => {
  if (
    bundle.authority.noConnectorMutation !== true ||
    bundle.authority.noGeneratedSummaryMutation !== true ||
    bundle.authority.noPublicClaimUpgrade !== true ||
    bundle.authority.noRawSourceArchiveCopy !== true ||
    bundle.authority.noRightsMutation !== true
  ) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason:
        'Knowledge source bundles must remain read-only and cannot mutate connectors, generated summaries, public claims, raw archives, or rights.',
    })
  }

  assertSafeRefs('Knowledge bundle refs', [
    bundle.id,
    bundle.bundleRef,
    bundle.titleRef,
  ])
  assertSafeRefs('Knowledge bundle caveat refs', bundle.caveatRefs)
  assertSafeRefs(
    'Knowledge bundle generated summary refs',
    bundle.generatedSummaryRefs,
  )
  assertSafeRefs('Knowledge bundle provenance refs', bundle.provenanceRefs)
  assertSafeRefs(
    'Knowledge bundle redaction policy refs',
    bundle.redactionPolicyRefs,
  )
  assertSafeRefs('Knowledge bundle rights refs', bundle.rightsRefs)
  assertSafeRefs('Knowledge bundle workroom refs', bundle.workroomRefs)

  if (bundle.sources.length === 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Knowledge source bundles require at least one source.',
    })
  }

  if (bundle.provenanceRefs.length === 0 || bundle.rightsRefs.length === 0) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason: 'Knowledge source bundles require provenance and rights refs.',
    })
  }

  bundle.sources.forEach(assertSourceRecord)

  const sourceRefs = new Set(bundle.sources.map(source => source.sourceRef))

  bundle.spans.forEach(span => assertSpanRecord(span, sourceRefs))
}

const locatorLabel = (span: OmniKnowledgeExtractedSpanRecord): string => {
  switch (span.spanKind) {
    case 'code':
      return `lines ${span.lineStart}-${span.lineEnd}`
    case 'file_range':
      if (span.lineStart !== null && span.lineEnd !== null) {
        return `lines ${span.lineStart}-${span.lineEnd}`
      }

      return `bytes ${span.byteStart}-${span.byteEnd}`
    case 'page':
      return `page ${span.pageNumber}`
    case 'row':
      return `rows ${span.rowStart}-${span.rowEnd}`
    case 'table_cell':
      return `rows ${span.rowStart}-${span.rowEnd}`
    case 'transcript':
      return `time ${span.timeStartMs}-${span.timeEndMs}ms`
  }
}

const sourceProjection = (
  source: OmniKnowledgeSourceRecord,
  audience: OmniKnowledgeProjectionAudience,
): OmniKnowledgeSourceProjection | null => {
  const digestRef = refsForAudience(
    'Knowledge source digest refs',
    [source.digestRef],
    audience,
  )[0]
  const locatorRef = refsForAudience(
    'Knowledge source locator refs',
    [source.locatorRef],
    audience,
  )[0]
  const sourceRef = refsForAudience(
    'Knowledge source refs',
    [source.sourceRef],
    audience,
  )[0]
  const titleRef = refsForAudience(
    'Knowledge source title refs',
    [source.titleRef],
    audience,
  )[0]

  if (
    digestRef === undefined ||
    locatorRef === undefined ||
    sourceRef === undefined ||
    titleRef === undefined
  ) {
    return null
  }

  return {
    caveatRefs: refsForAudience(
      'Knowledge source caveat refs',
      source.caveatRefs,
      audience,
    ),
    dataClassification: source.dataClassification,
    digestAlgorithm: source.digestAlgorithm,
    digestRef,
    locatorRef,
    provenanceRefs: refsForAudience(
      'Knowledge source provenance refs',
      source.provenanceRefs,
      audience,
    ),
    redactionPolicyRefs: refsForAudience(
      'Knowledge source redaction refs',
      source.redactionPolicyRefs,
      audience,
    ),
    rightsRefs: refsForAudience(
      'Knowledge source rights refs',
      source.rightsRefs,
      audience,
    ),
    rightsState: source.rightsState,
    sourceKind: source.sourceKind,
    sourceRef,
    titleRef,
    trustTier: source.trustTier,
  }
}

const spanProjection = (
  span: OmniKnowledgeExtractedSpanRecord,
  audience: OmniKnowledgeProjectionAudience,
): OmniKnowledgeSpanProjection | null => {
  const contentDigestRef = refsForAudience(
    'Knowledge span content digest refs',
    [span.contentDigestRef],
    audience,
  )[0]
  const excerptRef = refsForAudience(
    'Knowledge span excerpt refs',
    [span.excerptRef],
    audience,
  )[0]
  const id = refsForAudience('Knowledge span id refs', [span.id], audience)[0]
  const sourceRef = refsForAudience(
    'Knowledge span source refs',
    [span.sourceRef],
    audience,
  )[0]

  if (
    contentDigestRef === undefined ||
    excerptRef === undefined ||
    id === undefined ||
    sourceRef === undefined
  ) {
    return null
  }

  return {
    caveatRefs: refsForAudience(
      'Knowledge span caveat refs',
      span.caveatRefs,
      audience,
    ),
    codeSymbolRef: nullableRefForAudience(
      'Knowledge span code symbol refs',
      span.codeSymbolRef,
      audience,
    ),
    columnRefs: refsForAudience(
      'Knowledge span column refs',
      span.columnRefs,
      audience,
    ),
    contentDigestRef,
    dataClassification: span.dataClassification,
    excerptRef,
    factCandidateRefs: refsForAudience(
      'Knowledge span fact candidate refs',
      span.factCandidateRefs,
      audience,
    ),
    id,
    locatorLabel: locatorLabel(span),
    provenanceRefs: refsForAudience(
      'Knowledge span provenance refs',
      span.provenanceRefs,
      audience,
    ),
    redactionPolicyRefs: refsForAudience(
      'Knowledge span redaction refs',
      span.redactionPolicyRefs,
      audience,
    ),
    rightsRefs: refsForAudience(
      'Knowledge span rights refs',
      span.rightsRefs,
      audience,
    ),
    selectorRef: nullableRefForAudience(
      'Knowledge span selector refs',
      span.selectorRef,
      audience,
    ),
    sourceRef,
    spanKind: span.spanKind,
    trustTier: span.trustTier,
  }
}

const projectionHasPrivateMaterial = (
  projection: OmniKnowledgeSourceBundleProjection,
): boolean => {
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
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeKnowledgeRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectOmniKnowledgeSourceBundle = (
  bundle: OmniKnowledgeSourceBundleRecord,
  audience: OmniKnowledgeProjectionAudience,
  nowIso: string,
): OmniKnowledgeSourceBundleProjection => {
  assertBundleRecord(bundle)

  const sources = bundle.sources
    .map(source => sourceProjection(source, audience))
    .filter((source): source is OmniKnowledgeSourceProjection =>
      source !== null,
    )
  const spans = bundle.spans
    .map(span => spanProjection(span, audience))
    .filter((span): span is OmniKnowledgeSpanProjection => span !== null)
  const projection: OmniKnowledgeSourceBundleProjection = {
    audience,
    authority: OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY,
    bundleRef: primaryRefForAudience(
      'Knowledge bundle refs',
      bundle.bundleRef,
      audience,
      'bundle.redacted',
    ),
    caveatRefs: refsForAudience('Knowledge bundle caveat refs', bundle.caveatRefs, audience),
    connectorMutationAllowed: false,
    generatedSummaryMutationAllowed: false,
    generatedSummaryRefs: refsForAudience(
      'Knowledge bundle generated summary refs',
      bundle.generatedSummaryRefs,
      audience,
    ),
    id: primaryRefForAudience(
      'Knowledge bundle id refs',
      bundle.id,
      audience,
      'knowledge_bundle.redacted',
    ),
    provenanceRefs: refsForAudience(
      'Knowledge bundle provenance refs',
      bundle.provenanceRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    rawSourceArchiveCopyAllowed: false,
    redactionPolicyRefs: refsForAudience(
      'Knowledge bundle redaction policy refs',
      bundle.redactionPolicyRefs,
      audience,
    ),
    rightsMutationAllowed: false,
    rightsRefs: refsForAudience('Knowledge bundle rights refs', bundle.rightsRefs, audience),
    sourceCount: sources.length,
    sources,
    spanCount: spans.length,
    spans,
    titleRef: primaryRefForAudience(
      'Knowledge bundle title refs',
      bundle.titleRef,
      audience,
      'title.redacted',
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      bundle.updatedAtIso,
      nowIso,
    ),
    workroomRefs: refsForAudience(
      'Knowledge bundle workroom refs',
      bundle.workroomRefs,
      audience,
    ),
  }

  if (projectionHasPrivateMaterial(projection)) {
    throw new OmniKnowledgeSourceBundleUnsafe({
      reason:
        'Knowledge source bundle projection contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, secret, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniKnowledgeSourceBundle =
  (): OmniKnowledgeSourceBundleRecord => ({
    authority: OMNI_KNOWLEDGE_SOURCE_BUNDLE_READ_ONLY_AUTHORITY,
    bundleRef: 'bundle.public.otec_research_sources',
    caveatRefs: ['caveat.public.source_quotes_not_generated_claims'],
    createdAtIso: '2026-06-06T22:00:00.000Z',
    generatedSummaryRefs: ['summary.public.otec_research_brief'],
    id: 'knowledge_bundle.public.otec_research_sources',
    provenanceRefs: ['provenance.public.operator_reviewed_import'],
    redactionPolicyRefs: ['policy.public.redacted_archive_only'],
    rightsRefs: ['rights.public.web_citation_allowed'],
    sources: [
      {
        caveatRefs: ['caveat.public.public_web_source'],
        dataClassification: 'public',
        digestAlgorithm: 'sha256',
        digestRef: 'digest.public.openagents_transcript_230',
        locatorRef: 'locator.public.github_transcript_230',
        provenanceRefs: ['provenance.public.github_raw_fetch'],
        redactionPolicyRefs: ['policy.public.quote_limit'],
        rightsRefs: ['rights.public.openagents_repo'],
        rightsState: 'public',
        sourceKind: 'transcript',
        sourceRef: 'source.public.openagents_transcript_230',
        titleRef: 'title.public.open_letter_to_agents',
        trustTier: 'reviewed',
      },
      {
        caveatRefs: ['caveat.public.repo_ref_snapshot'],
        dataClassification: 'public',
        digestAlgorithm: 'sha256',
        digestRef: 'digest.public.otec_site_commit',
        locatorRef: 'locator.public.github_openagents_commit',
        provenanceRefs: ['provenance.public.github_commit'],
        redactionPolicyRefs: ['policy.public.no_private_repo_refs'],
        rightsRefs: ['rights.public.openagents_repo'],
        rightsState: 'public',
        sourceKind: 'repo_ref',
        sourceRef: 'source.public.openagents_otec_site_commit',
        titleRef: 'title.public.otec_site_commit',
        trustTier: 'reviewed',
      },
    ],
    spans: [
      {
        byteEnd: null,
        byteStart: null,
        caveatRefs: ['caveat.public.transcript_context'],
        codeSymbolRef: null,
        columnRefs: [],
        contentDigestRef: 'digest.public.span.transcript_230_intro',
        dataClassification: 'public',
        excerptRef: 'excerpt.public.transcript_230_intro',
        factCandidateRefs: ['fact.public.pay_people_with_receipts'],
        id: 'span.public.transcript_230_intro',
        lineEnd: null,
        lineStart: null,
        pageNumber: null,
        provenanceRefs: ['provenance.public.github_raw_fetch'],
        redactionPolicyRefs: ['policy.public.quote_limit'],
        rightsRefs: ['rights.public.openagents_repo'],
        rowEnd: null,
        rowStart: null,
        selectorRef: 'selector.public.transcript_230_intro',
        sourceRef: 'source.public.openagents_transcript_230',
        spanKind: 'transcript',
        timeEndMs: 120000,
        timeStartMs: 0,
        trustTier: 'reviewed',
      },
      {
        byteEnd: null,
        byteStart: null,
        caveatRefs: ['caveat.public.code_context'],
        codeSymbolRef: 'symbol.public.otec_site_renderer',
        columnRefs: [],
        contentDigestRef: 'digest.public.span.otec_site_renderer',
        dataClassification: 'public',
        excerptRef: 'excerpt.public.otec_site_renderer',
        factCandidateRefs: ['fact.public.otec_site_revision_source'],
        id: 'span.public.otec_site_renderer',
        lineEnd: 88,
        lineStart: 42,
        pageNumber: null,
        provenanceRefs: ['provenance.public.github_commit'],
        redactionPolicyRefs: ['policy.public.no_private_repo_refs'],
        rightsRefs: ['rights.public.openagents_repo'],
        rowEnd: null,
        rowStart: null,
        selectorRef: 'selector.public.otec_site_renderer',
        sourceRef: 'source.public.openagents_otec_site_commit',
        spanKind: 'code',
        timeEndMs: null,
        timeStartMs: null,
        trustTier: 'reviewed',
      },
    ],
    titleRef: 'title.public.otec_research_sources',
    updatedAtIso: '2026-06-06T22:25:00.000Z',
    workroomRefs: ['workroom.public.otec_research'],
  })
