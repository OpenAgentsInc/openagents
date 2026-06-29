import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniRetrievalTraceAudience = S.Literals([
  'public',
  'team',
  'operator',
])
export type OmniRetrievalTraceAudience =
  typeof OmniRetrievalTraceAudience.Type

export const OmniRetrievalSelectorKind = S.Literals([
  'embedding_similarity',
  'graph_expansion',
  'hybrid_semantic_graph',
  'manual_pin',
  'semantic_selector',
  'structured_query_plan',
])
export type OmniRetrievalSelectorKind =
  typeof OmniRetrievalSelectorKind.Type

export const OmniRetrievalSelectionState = S.Literals([
  'excluded',
  'selected',
])
export type OmniRetrievalSelectionState =
  typeof OmniRetrievalSelectionState.Type

export const OmniRetrievalFreshness = S.Literals([
  'fresh',
  'recent',
  'stale',
  'unknown',
])
export type OmniRetrievalFreshness = typeof OmniRetrievalFreshness.Type

export const OmniRetrievalExclusionReasonKind = S.Literals([
  'blocked',
  'duplicate',
  'low_similarity',
  'not_relevant',
  'privacy',
  'rights',
  'stale',
])
export type OmniRetrievalExclusionReasonKind =
  typeof OmniRetrievalExclusionReasonKind.Type

export const OmniRetrievalMissingContextKind = S.Literals([
  'contradiction',
  'needed_source',
  'private_source',
  'rights_blocked',
  'stale_memory',
  'unclear_query',
])
export type OmniRetrievalMissingContextKind =
  typeof OmniRetrievalMissingContextKind.Type

export const OmniRetrievalGraphNodeKind = S.Literals([
  'agent',
  'claim',
  'customer',
  'fact',
  'repo',
  'site',
  'source',
  'span',
  'workroom',
])
export type OmniRetrievalGraphNodeKind =
  typeof OmniRetrievalGraphNodeKind.Type

export const OmniRetrievalGraphEdgeKind = S.Literals([
  'contradicts',
  'derived_from',
  'human_confirmed',
  'mentions',
  'requires',
  'supports',
  'supersedes',
])
export type OmniRetrievalGraphEdgeKind =
  typeof OmniRetrievalGraphEdgeKind.Type

export const OmniRetrievalFactState = S.Literals([
  'candidate',
  'contradicted',
  'human_confirmed',
  'rejected',
])
export type OmniRetrievalFactState = typeof OmniRetrievalFactState.Type

export const OmniRetrievalTraceAuthorityBoundary = S.Literals([
  'read_only_retrieval_trace',
])
export type OmniRetrievalTraceAuthorityBoundary =
  typeof OmniRetrievalTraceAuthorityBoundary.Type

export class OmniRetrievalTraceAuthority extends S.Class<OmniRetrievalTraceAuthority>(
  'OmniRetrievalTraceAuthority',
)({
  authorityBoundary: OmniRetrievalTraceAuthorityBoundary,
  noAutonomousSourceFetch: S.Boolean,
  noFactPromotionMutation: S.Boolean,
  noGeneratedSummaryMutation: S.Boolean,
  noGraphMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
}) {}

export class OmniRetrievalSourceHit extends S.Class<OmniRetrievalSourceHit>(
  'OmniRetrievalSourceHit',
)({
  caveatRefs: S.Array(S.String),
  exclusionReasonKind: S.NullOr(OmniRetrievalExclusionReasonKind),
  freshness: OmniRetrievalFreshness,
  provenanceRefs: S.Array(S.String),
  rank: S.Number,
  reasonRef: S.NullOr(S.String),
  rightsRef: S.String,
  scoreBps: S.Number,
  selectionState: OmniRetrievalSelectionState,
  sourceBundleRef: S.String,
  sourceRef: S.String,
  spanRefs: S.Array(S.String),
}) {}

export class OmniRetrievalMissingContextItem extends S.Class<OmniRetrievalMissingContextItem>(
  'OmniRetrievalMissingContextItem',
)({
  kind: OmniRetrievalMissingContextKind,
  labelRef: S.String,
  reasonRef: S.String,
  requiredForRef: S.String,
}) {}

export class OmniRetrievalGraphNode extends S.Class<OmniRetrievalGraphNode>(
  'OmniRetrievalGraphNode',
)({
  caveatRefs: S.Array(S.String),
  humanConfirmationRefs: S.Array(S.String),
  kind: OmniRetrievalGraphNodeKind,
  nodeRef: S.String,
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
}) {}

export class OmniRetrievalGraphEdge extends S.Class<OmniRetrievalGraphEdge>(
  'OmniRetrievalGraphEdge',
)({
  caveatRefs: S.Array(S.String),
  edgeRef: S.String,
  fromNodeRef: S.String,
  humanConfirmationRefs: S.Array(S.String),
  kind: OmniRetrievalGraphEdgeKind,
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
  toNodeRef: S.String,
}) {}

export class OmniRetrievalConfirmedFact extends S.Class<OmniRetrievalConfirmedFact>(
  'OmniRetrievalConfirmedFact',
)({
  caveatRefs: S.Array(S.String),
  factRef: S.String,
  humanConfirmationRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
  state: OmniRetrievalFactState,
}) {}

export class OmniRetrievalTraceRecord extends S.Class<OmniRetrievalTraceRecord>(
  'OmniRetrievalTraceRecord',
)({
  authority: OmniRetrievalTraceAuthority,
  caveatRefs: S.Array(S.String),
  confirmedFacts: S.Array(OmniRetrievalConfirmedFact),
  excludedSources: S.Array(OmniRetrievalSourceHit),
  generatedSummaryRefs: S.Array(S.String),
  graphEdges: S.Array(OmniRetrievalGraphEdge),
  graphNodes: S.Array(OmniRetrievalGraphNode),
  id: S.String,
  missingContext: S.Array(OmniRetrievalMissingContextItem),
  provenanceRefs: S.Array(S.String),
  queryDigestRef: S.String,
  queryIntentRef: S.String,
  redactionPolicyRefs: S.Array(S.String),
  retrievedAtIso: S.String,
  selectedSources: S.Array(OmniRetrievalSourceHit),
  selectorKind: OmniRetrievalSelectorKind,
  selectorModelRef: S.String,
  sourceBundleRefs: S.Array(S.String),
  workroomRef: S.String,
}) {}

export class OmniRetrievalSourceHitProjection extends S.Class<OmniRetrievalSourceHitProjection>(
  'OmniRetrievalSourceHitProjection',
)({
  caveatRefs: S.Array(S.String),
  exclusionReasonKind: S.NullOr(OmniRetrievalExclusionReasonKind),
  exclusionReasonLabel: S.NullOr(S.String),
  freshness: OmniRetrievalFreshness,
  freshnessLabel: S.String,
  provenanceRefs: S.Array(S.String),
  rank: S.Number,
  reasonRef: S.NullOr(S.String),
  rightsRef: S.String,
  scoreBps: S.Number,
  selectionState: OmniRetrievalSelectionState,
  sourceBundleRef: S.String,
  sourceRef: S.String,
  spanRefs: S.Array(S.String),
}) {}

export class OmniRetrievalTraceProjection extends S.Class<OmniRetrievalTraceProjection>(
  'OmniRetrievalTraceProjection',
)({
  audience: OmniRetrievalTraceAudience,
  authority: OmniRetrievalTraceAuthority,
  autonomousSourceFetchAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  confirmedFacts: S.Array(OmniRetrievalConfirmedFact),
  excludedCount: S.Number,
  excludedSources: S.Array(OmniRetrievalSourceHitProjection),
  factPromotionMutationAllowed: S.Boolean,
  generatedSummaryMutationAllowed: S.Boolean,
  generatedSummaryRefs: S.Array(S.String),
  graphEdges: S.Array(OmniRetrievalGraphEdge),
  graphMutationAllowed: S.Boolean,
  graphNodes: S.Array(OmniRetrievalGraphNode),
  humanConfirmedFactCount: S.Number,
  id: S.String,
  missingContext: S.Array(OmniRetrievalMissingContextItem),
  missingContextCount: S.Number,
  provenanceRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  queryDigestRef: S.String,
  queryIntentRef: S.String,
  redactionPolicyRefs: S.Array(S.String),
  retrievedAtDisplay: S.String,
  selectedCount: S.Number,
  selectedSources: S.Array(OmniRetrievalSourceHitProjection),
  selectorKind: OmniRetrievalSelectorKind,
  selectorModelRef: S.String,
  sourceBundleRefs: S.Array(S.String),
  staleSelectedCount: S.Number,
  workroomRef: S.String,
}) {}

export class OmniRetrievalTraceUnsafe extends S.TaggedErrorClass<OmniRetrievalTraceUnsafe>()(
  'OmniRetrievalTraceUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY:
  OmniRetrievalTraceAuthority = {
    authorityBoundary: 'read_only_retrieval_trace',
    noAutonomousSourceFetch: true,
    noFactPromotionMutation: true,
    noGeneratedSummaryMutation: true,
    noGraphMutation: true,
    noPublicClaimUpgrade: true,
  }

const freshnessLabelByFreshness: Readonly<Record<OmniRetrievalFreshness, string>> = {
  fresh: 'Fresh',
  recent: 'Recent',
  stale: 'Stale',
  unknown: 'Unknown',
}

const exclusionLabelByReason:
  Readonly<Record<OmniRetrievalExclusionReasonKind, string>> = {
    blocked: 'Blocked',
    duplicate: 'Duplicate',
    low_similarity: 'Low similarity',
    not_relevant: 'Not relevant',
    privacy: 'Privacy',
    rights: 'Rights',
    stale: 'Stale',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRetrievalRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|ad[_-]?hoc[_-]?keyword|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|keyword[_-]?only[_-]?selector|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(key|source|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(archive|auth|connector|customer|email|export|file|invoice|payment|payload|payout|prompt|provider|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|summary[_-]?text|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(bundle\.private|caveat\.private|confirmation\.private|edge\.private|fact\.private|graph\.private|intent\.private|node\.private|policy\.private|provenance\.private|reason\.private|rights\.private|source\.private|span\.private|summary\.private|workroom\.)/i
const teamUnsafeRefPattern =
  /(confirmation\.private|fact\.private|reason\.private|rights\.private|source\.private|span\.private|summary\.private|workroom\.private)/i

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
      unsafeRetrievalRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniRetrievalTraceUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, ad hoc keyword selector, secret, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniRetrievalTraceAudience,
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
  audience: OmniRetrievalTraceAudience,
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
  audience: OmniRetrievalTraceAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return refsForAudience(label, [ref], audience)[0] ?? null
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniRetrievalTraceAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertRankAndScore = (hit: OmniRetrievalSourceHit): void => {
  if (!Number.isInteger(hit.rank) || hit.rank < 1) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval hit rank must be a positive integer.',
    })
  }

  if (!Number.isInteger(hit.scoreBps) || hit.scoreBps < 0 || hit.scoreBps > 10000) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval hit scoreBps must be an integer from 0 through 10000.',
    })
  }
}

const assertHit = (hit: OmniRetrievalSourceHit): void => {
  assertRankAndScore(hit)
  assertSafeRefs('Retrieval hit refs', [
    hit.sourceRef,
    hit.sourceBundleRef,
    hit.rightsRef,
    hit.reasonRef ?? '',
  ])
  assertSafeRefs('Retrieval hit caveat refs', hit.caveatRefs)
  assertSafeRefs('Retrieval hit provenance refs', hit.provenanceRefs)
  assertSafeRefs('Retrieval hit span refs', hit.spanRefs)

  if (hit.provenanceRefs.length === 0) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval hits require provenance refs.',
    })
  }

  if (hit.selectionState === 'selected' && hit.exclusionReasonKind !== null) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Selected retrieval hits cannot carry exclusion reasons.',
    })
  }

  if (
    hit.selectionState === 'excluded' &&
    (hit.exclusionReasonKind === null || hit.reasonRef === null)
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Excluded retrieval hits require exclusion reason kind and reason ref.',
    })
  }
}

const supportRefs = (
  sourceRefs: ReadonlyArray<string>,
  spanRefs: ReadonlyArray<string>,
  humanConfirmationRefs: ReadonlyArray<string>,
): number =>
  sourceRefs.length + spanRefs.length + humanConfirmationRefs.length

const assertNode = (node: OmniRetrievalGraphNode): void => {
  assertSafeRefs('Retrieval graph node refs', [
    node.nodeRef,
    ...node.humanConfirmationRefs,
    ...node.sourceRefs,
    ...node.spanRefs,
  ])
  assertSafeRefs('Retrieval graph node caveat refs', node.caveatRefs)

  if (
    supportRefs(
      node.sourceRefs,
      node.spanRefs,
      node.humanConfirmationRefs,
    ) === 0
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason:
        'Retrieval graph nodes require source refs, span refs, or human confirmation refs.',
    })
  }
}

const assertEdge = (
  edge: OmniRetrievalGraphEdge,
  nodeRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Retrieval graph edge refs', [
    edge.edgeRef,
    edge.fromNodeRef,
    edge.toNodeRef,
    ...edge.humanConfirmationRefs,
    ...edge.sourceRefs,
    ...edge.spanRefs,
  ])
  assertSafeRefs('Retrieval graph edge caveat refs', edge.caveatRefs)

  if (!nodeRefs.has(edge.fromNodeRef) || !nodeRefs.has(edge.toNodeRef)) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval graph edges must reference nodes in the same trace.',
    })
  }

  if (
    supportRefs(
      edge.sourceRefs,
      edge.spanRefs,
      edge.humanConfirmationRefs,
    ) === 0
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason:
        'Retrieval graph edges require source refs, span refs, or human confirmation refs.',
    })
  }
}

const assertFact = (fact: OmniRetrievalConfirmedFact): void => {
  assertSafeRefs('Retrieval confirmed fact refs', [
    fact.factRef,
    ...fact.humanConfirmationRefs,
    ...fact.sourceRefs,
    ...fact.spanRefs,
  ])
  assertSafeRefs('Retrieval confirmed fact caveat refs', fact.caveatRefs)

  if (
    supportRefs(
      fact.sourceRefs,
      fact.spanRefs,
      fact.humanConfirmationRefs,
    ) === 0
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason:
        'Retrieval facts require source refs, span refs, or human confirmation refs.',
    })
  }

  if (
    fact.state === 'human_confirmed' &&
    fact.humanConfirmationRefs.length === 0
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Human-confirmed facts require human confirmation refs.',
    })
  }
}

const assertTraceRecord = (record: OmniRetrievalTraceRecord): void => {
  if (
    record.authority.noAutonomousSourceFetch !== true ||
    record.authority.noFactPromotionMutation !== true ||
    record.authority.noGeneratedSummaryMutation !== true ||
    record.authority.noGraphMutation !== true ||
    record.authority.noPublicClaimUpgrade !== true
  ) {
    throw new OmniRetrievalTraceUnsafe({
      reason:
        'Retrieval traces must remain read-only and cannot fetch sources, promote facts, mutate summaries, mutate graphs, or upgrade public claims.',
    })
  }

  assertSafeRefs('Retrieval trace refs', [
    record.id,
    record.queryDigestRef,
    record.queryIntentRef,
    record.selectorModelRef,
    record.workroomRef,
  ])
  assertSafeRefs('Retrieval trace caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Retrieval trace generated summary refs',
    record.generatedSummaryRefs,
  )
  assertSafeRefs('Retrieval trace provenance refs', record.provenanceRefs)
  assertSafeRefs(
    'Retrieval trace redaction policy refs',
    record.redactionPolicyRefs,
  )
  assertSafeRefs('Retrieval trace source bundle refs', record.sourceBundleRefs)

  if (record.provenanceRefs.length === 0) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval traces require provenance refs.',
    })
  }

  if (record.sourceBundleRefs.length === 0) {
    throw new OmniRetrievalTraceUnsafe({
      reason: 'Retrieval traces require source bundle refs.',
    })
  }

  record.selectedSources.forEach(hit => {
    if (hit.selectionState !== 'selected') {
      throw new OmniRetrievalTraceUnsafe({
        reason: 'selectedSources must contain selected hits only.',
      })
    }
    assertHit(hit)
  })
  record.excludedSources.forEach(hit => {
    if (hit.selectionState !== 'excluded') {
      throw new OmniRetrievalTraceUnsafe({
        reason: 'excludedSources must contain excluded hits only.',
      })
    }
    assertHit(hit)
  })
  record.missingContext.forEach(item =>
    assertSafeRefs('Retrieval missing context refs', [
      item.labelRef,
      item.reasonRef,
      item.requiredForRef,
    ]),
  )
  record.graphNodes.forEach(assertNode)

  const nodeRefs = new Set(record.graphNodes.map(node => node.nodeRef))

  record.graphEdges.forEach(edge => assertEdge(edge, nodeRefs))
  record.graphNodes.forEach(node => {
    if (node.kind === 'fact') {
      const hasFact = record.graphEdges.some(
        edge => edge.fromNodeRef === node.nodeRef || edge.toNodeRef === node.nodeRef,
      )

      if (!hasFact) {
        throw new OmniRetrievalTraceUnsafe({
          reason: 'Fact graph nodes must be connected by at least one edge.',
        })
      }
    }
  })
  record.confirmedFacts.forEach(assertFact)
}

const hitProjection = (
  hit: OmniRetrievalSourceHit,
  audience: OmniRetrievalTraceAudience,
): OmniRetrievalSourceHitProjection | null => {
  const sourceRef = refsForAudience(
    'Retrieval hit source refs',
    [hit.sourceRef],
    audience,
  )[0]
  const sourceBundleRef = refsForAudience(
    'Retrieval hit source bundle refs',
    [hit.sourceBundleRef],
    audience,
  )[0]
  const rightsRef = refsForAudience(
    'Retrieval hit rights refs',
    [hit.rightsRef],
    audience,
  )[0]

  if (
    sourceRef === undefined ||
    sourceBundleRef === undefined ||
    rightsRef === undefined
  ) {
    return null
  }

  return {
    caveatRefs: refsForAudience('Retrieval hit caveat refs', hit.caveatRefs, audience),
    exclusionReasonKind: hit.exclusionReasonKind,
    exclusionReasonLabel:
      hit.exclusionReasonKind === null
        ? null
        : exclusionLabelByReason[hit.exclusionReasonKind],
    freshness: hit.freshness,
    freshnessLabel: freshnessLabelByFreshness[hit.freshness],
    provenanceRefs: refsForAudience(
      'Retrieval hit provenance refs',
      hit.provenanceRefs,
      audience,
    ),
    rank: hit.rank,
    reasonRef: nullableRefForAudience(
      'Retrieval hit reason refs',
      hit.reasonRef,
      audience,
    ),
    rightsRef,
    scoreBps: hit.scoreBps,
    selectionState: hit.selectionState,
    sourceBundleRef,
    sourceRef,
    spanRefs: refsForAudience('Retrieval hit span refs', hit.spanRefs, audience),
  }
}

const graphNodeProjection = (
  node: OmniRetrievalGraphNode,
  audience: OmniRetrievalTraceAudience,
): OmniRetrievalGraphNode | null => {
  const nodeRef = refsForAudience(
    'Retrieval graph node refs',
    [node.nodeRef],
    audience,
  )[0]

  if (nodeRef === undefined) {
    return null
  }

  return {
    ...node,
    caveatRefs: refsForAudience(
      'Retrieval graph node caveat refs',
      node.caveatRefs,
      audience,
    ),
    humanConfirmationRefs: refsForAudience(
      'Retrieval graph node confirmation refs',
      node.humanConfirmationRefs,
      audience,
    ),
    nodeRef,
    sourceRefs: refsForAudience(
      'Retrieval graph node source refs',
      node.sourceRefs,
      audience,
    ),
    spanRefs: refsForAudience(
      'Retrieval graph node span refs',
      node.spanRefs,
      audience,
    ),
  }
}

const graphEdgeProjection = (
  edge: OmniRetrievalGraphEdge,
  visibleNodeRefs: ReadonlySet<string>,
  audience: OmniRetrievalTraceAudience,
): OmniRetrievalGraphEdge | null => {
  const edgeRef = refsForAudience(
    'Retrieval graph edge refs',
    [edge.edgeRef],
    audience,
  )[0]
  const fromNodeRef = refsForAudience(
    'Retrieval graph edge from node refs',
    [edge.fromNodeRef],
    audience,
  )[0]
  const toNodeRef = refsForAudience(
    'Retrieval graph edge to node refs',
    [edge.toNodeRef],
    audience,
  )[0]

  if (
    edgeRef === undefined ||
    fromNodeRef === undefined ||
    toNodeRef === undefined ||
    !visibleNodeRefs.has(fromNodeRef) ||
    !visibleNodeRefs.has(toNodeRef)
  ) {
    return null
  }

  return {
    ...edge,
    caveatRefs: refsForAudience(
      'Retrieval graph edge caveat refs',
      edge.caveatRefs,
      audience,
    ),
    edgeRef,
    fromNodeRef,
    humanConfirmationRefs: refsForAudience(
      'Retrieval graph edge confirmation refs',
      edge.humanConfirmationRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Retrieval graph edge source refs',
      edge.sourceRefs,
      audience,
    ),
    spanRefs: refsForAudience(
      'Retrieval graph edge span refs',
      edge.spanRefs,
      audience,
    ),
    toNodeRef,
  }
}

const confirmedFactProjection = (
  fact: OmniRetrievalConfirmedFact,
  audience: OmniRetrievalTraceAudience,
): OmniRetrievalConfirmedFact | null => {
  const factRef = refsForAudience(
    'Retrieval fact refs',
    [fact.factRef],
    audience,
  )[0]

  if (factRef === undefined) {
    return null
  }

  return {
    ...fact,
    caveatRefs: refsForAudience(
      'Retrieval fact caveat refs',
      fact.caveatRefs,
      audience,
    ),
    factRef,
    humanConfirmationRefs: refsForAudience(
      'Retrieval fact confirmation refs',
      fact.humanConfirmationRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Retrieval fact source refs',
      fact.sourceRefs,
      audience,
    ),
    spanRefs: refsForAudience(
      'Retrieval fact span refs',
      fact.spanRefs,
      audience,
    ),
  }
}

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
  projection: OmniRetrievalTraceProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeRetrievalRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectOmniRetrievalTrace = (
  record: OmniRetrievalTraceRecord,
  audience: OmniRetrievalTraceAudience,
  nowIso: string,
): OmniRetrievalTraceProjection => {
  assertTraceRecord(record)

  const selectedSources = record.selectedSources
    .map(hit => hitProjection(hit, audience))
    .filter((hit): hit is OmniRetrievalSourceHitProjection => hit !== null)
    .sort((left, right) => left.rank - right.rank)
  const excludedSources = record.excludedSources
    .map(hit => hitProjection(hit, audience))
    .filter((hit): hit is OmniRetrievalSourceHitProjection => hit !== null)
    .sort((left, right) => left.rank - right.rank)
  const graphNodes = record.graphNodes
    .map(node => graphNodeProjection(node, audience))
    .filter((node): node is OmniRetrievalGraphNode => node !== null)
  const visibleNodeRefs = new Set(graphNodes.map(node => node.nodeRef))
  const graphEdges = record.graphEdges
    .map(edge => graphEdgeProjection(edge, visibleNodeRefs, audience))
    .filter((edge): edge is OmniRetrievalGraphEdge => edge !== null)
  const confirmedFacts = record.confirmedFacts
    .map(fact => confirmedFactProjection(fact, audience))
    .filter((fact): fact is OmniRetrievalConfirmedFact => fact !== null)
  const projection: OmniRetrievalTraceProjection = {
    audience,
    authority: OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY,
    autonomousSourceFetchAllowed: false,
    caveatRefs: refsForAudience(
      'Retrieval trace caveat refs',
      record.caveatRefs,
      audience,
    ),
    confirmedFacts,
    excludedCount: excludedSources.length,
    excludedSources,
    factPromotionMutationAllowed: false,
    generatedSummaryMutationAllowed: false,
    generatedSummaryRefs: refsForAudience(
      'Retrieval trace generated summary refs',
      record.generatedSummaryRefs,
      audience,
    ),
    graphEdges,
    graphMutationAllowed: false,
    graphNodes,
    humanConfirmedFactCount: confirmedFacts.filter(
      fact => fact.state === 'human_confirmed',
    ).length,
    id: primaryRefForAudience(
      'Retrieval trace id refs',
      record.id,
      audience,
      'retrieval_trace.redacted',
    ),
    missingContext: record.missingContext.map(item => ({
      ...item,
      labelRef: primaryRefForAudience(
        'Retrieval missing context label refs',
        item.labelRef,
        audience,
        'label.redacted',
      ),
      reasonRef: primaryRefForAudience(
        'Retrieval missing context reason refs',
        item.reasonRef,
        audience,
        'reason.redacted',
      ),
      requiredForRef: primaryRefForAudience(
        'Retrieval missing context required refs',
        item.requiredForRef,
        audience,
        'required.redacted',
      ),
    })),
    missingContextCount: record.missingContext.length,
    provenanceRefs: refsForAudience(
      'Retrieval trace provenance refs',
      record.provenanceRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    queryDigestRef: primaryRefForAudience(
      'Retrieval trace query digest refs',
      record.queryDigestRef,
      audience,
      'query_digest.redacted',
    ),
    queryIntentRef: primaryRefForAudience(
      'Retrieval trace query intent refs',
      record.queryIntentRef,
      audience,
      'query_intent.redacted',
    ),
    redactionPolicyRefs: refsForAudience(
      'Retrieval trace redaction policy refs',
      record.redactionPolicyRefs,
      audience,
    ),
    retrievedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.retrievedAtIso,
      nowIso,
    ),
    selectedCount: selectedSources.length,
    selectedSources,
    selectorKind: record.selectorKind,
    selectorModelRef: primaryRefForAudience(
      'Retrieval trace selector model refs',
      record.selectorModelRef,
      audience,
      'selector_model.redacted',
    ),
    sourceBundleRefs: refsForAudience(
      'Retrieval trace source bundle refs',
      record.sourceBundleRefs,
      audience,
    ),
    staleSelectedCount: selectedSources.filter(
      hit => hit.freshness === 'stale',
    ).length,
    workroomRef: primaryRefForAudience(
      'Retrieval trace workroom refs',
      record.workroomRef,
      audience,
      'redacted.workroom',
    ),
  }

  if (projectionHasPrivateMaterial(projection)) {
    throw new OmniRetrievalTraceUnsafe({
      reason:
        'Retrieval trace projection contains private customer, provider, wallet, payment, raw source, raw transcript, raw text, private repo, generated summary text, ad hoc keyword selector, secret, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniRetrievalTrace = (): OmniRetrievalTraceRecord => ({
  authority: OMNI_RETRIEVAL_TRACE_READ_ONLY_AUTHORITY,
  caveatRefs: ['caveat.public.retrieval_trace_not_claim'],
  confirmedFacts: [
    {
      caveatRefs: ['caveat.public.operator_confirmation'],
      factRef: 'fact.public.otec_power_compute_context',
      humanConfirmationRefs: ['confirmation.public.operator_review_1'],
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: ['span.public.transcript_230_intro'],
      state: 'human_confirmed',
    },
  ],
  excludedSources: [
    {
      caveatRefs: ['caveat.public.stale_source'],
      exclusionReasonKind: 'stale',
      freshness: 'stale',
      provenanceRefs: ['provenance.public.semantic_selector'],
      rank: 3,
      reasonRef: 'reason.public.stale_memory',
      rightsRef: 'rights.public.web_citation_allowed',
      scoreBps: 6200,
      selectionState: 'excluded',
      sourceBundleRef: 'bundle.public.otec_research_sources',
      sourceRef: 'source.public.old_otec_brief',
      spanRefs: ['span.public.old_otec_brief'],
    },
  ],
  generatedSummaryRefs: ['summary.public.otec_retrieval_context'],
  graphEdges: [
    {
      caveatRefs: ['caveat.public.source_support'],
      edgeRef: 'edge.public.otec_claim_supported_by_transcript',
      fromNodeRef: 'node.public.claim.otec_power_compute',
      humanConfirmationRefs: [],
      kind: 'supports',
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: ['span.public.transcript_230_intro'],
      toNodeRef: 'node.public.source.transcript_230',
    },
    {
      caveatRefs: ['caveat.public.operator_confirmation'],
      edgeRef: 'edge.public.operator_confirmed_claim',
      fromNodeRef: 'node.public.claim.otec_power_compute',
      humanConfirmationRefs: ['confirmation.public.operator_review_1'],
      kind: 'human_confirmed',
      sourceRefs: [],
      spanRefs: [],
      toNodeRef: 'node.public.fact.operator_confirmed',
    },
  ],
  graphNodes: [
    {
      caveatRefs: ['caveat.public.claim_needs_receipts'],
      humanConfirmationRefs: [],
      kind: 'claim',
      nodeRef: 'node.public.claim.otec_power_compute',
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: ['span.public.transcript_230_intro'],
    },
    {
      caveatRefs: [],
      humanConfirmationRefs: [],
      kind: 'source',
      nodeRef: 'node.public.source.transcript_230',
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: [],
    },
    {
      caveatRefs: ['caveat.public.operator_confirmation'],
      humanConfirmationRefs: ['confirmation.public.operator_review_1'],
      kind: 'fact',
      nodeRef: 'node.public.fact.operator_confirmed',
      sourceRefs: [],
      spanRefs: [],
    },
  ],
  id: 'retrieval_trace.public.otec_context_1',
  missingContext: [
    {
      kind: 'needed_source',
      labelRef: 'label.public.need_facility_power_source',
      reasonRef: 'reason.public.facility_power_not_measured',
      requiredForRef: 'claim.public.facility_power',
    },
  ],
  provenanceRefs: ['provenance.public.semantic_selector'],
  queryDigestRef: 'query_digest.public.otec_context',
  queryIntentRef: 'intent.public.otec_context_for_site_revision',
  redactionPolicyRefs: ['policy.public.redacted_archive_only'],
  retrievedAtIso: '2026-06-06T22:25:00.000Z',
  selectedSources: [
    {
      caveatRefs: ['caveat.public.transcript_context'],
      exclusionReasonKind: null,
      freshness: 'fresh',
      provenanceRefs: ['provenance.public.semantic_selector'],
      rank: 1,
      reasonRef: null,
      rightsRef: 'rights.public.openagents_repo',
      scoreBps: 9300,
      selectionState: 'selected',
      sourceBundleRef: 'bundle.public.otec_research_sources',
      sourceRef: 'source.public.openagents_transcript_230',
      spanRefs: ['span.public.transcript_230_intro'],
    },
    {
      caveatRefs: ['caveat.public.repo_context'],
      exclusionReasonKind: null,
      freshness: 'recent',
      provenanceRefs: ['provenance.public.semantic_selector'],
      rank: 2,
      reasonRef: null,
      rightsRef: 'rights.public.openagents_repo',
      scoreBps: 8500,
      selectionState: 'selected',
      sourceBundleRef: 'bundle.public.otec_research_sources',
      sourceRef: 'source.public.openagents_otec_site_commit',
      spanRefs: ['span.public.otec_site_renderer'],
    },
  ],
  selectorKind: 'hybrid_semantic_graph',
  selectorModelRef: 'selector_model.public.embedding_graph_v1',
  sourceBundleRefs: ['bundle.public.otec_research_sources'],
  workroomRef: 'workroom.public.otec_research',
})
